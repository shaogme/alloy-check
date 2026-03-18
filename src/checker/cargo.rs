use crate::report::Diagnostic as Diag;
use crate::report::Report;
use crate::workspace::WorkspaceContext as Ctx;
use anyhow::Result;
use anyhow::anyhow;
use serde_json::Value;
use std::io::BufRead;
use std::io::BufReader;
use std::path::Path;
use std::process::Command;
use std::process::Stdio;

/// 运行 `cargo fmt --check` 并将结果添加到报告中。
pub fn check_fmt(ctx: &Ctx, report: &mut Report) -> Result<()> {
    let output = Command::new("cargo")
        .args(["fmt", "--all", "--", "--check"])
        .current_dir(&ctx.root)
        .output()?;

    if !output.status.success() {
        report.add(Diag::error(
            ctx.root.clone(),
            0,
            0,
            "FMT001",
            "Code is not formatted correctly. Run `cargo fmt --all` to fix.",
        ));
    }
    Ok(())
}

/// 运行 `cargo check`。
///
/// - **Human 模式**：执行 `RUSTFLAGS="-D warnings" cargo check --all-targets`，
///   将 stderr 直接透传到终端。若命令失败则立即返回 `Err`，调用方应中止后续步骤。
/// - **JSON 模式**：执行 `cargo check --all-targets --message-format=json`，
///   解析 JSON 输出并将诊断写入 `report`。
pub fn check_cargo(ctx: &Ctx, report: &mut Report, json_mode: bool) -> Result<()> {
    let mut args = vec!["check".to_string(), "--all-targets".to_string()];
    push_feature_args(ctx, &mut args);

    if json_mode {
        args.push("--message-format=json".to_string());
        run_cargo_json(ctx, report, args, "CHECK")
    } else {
        // cargo check 不支持 `-- -D warnings`，需通过 RUSTFLAGS 传入
        run_cargo_passthrough(ctx, args, Some("-D warnings"))
    }
}

/// 运行 `cargo clippy`。
///
/// - **Human 模式**：执行 `cargo clippy --all-targets -- -D warnings`，
///   将 stderr 直接透传到终端。若命令失败则立即返回 `Err`，调用方应中止后续步骤。
/// - **JSON 模式**：执行 `cargo clippy --all-targets --message-format=json -- -D warnings`，
///   解析 JSON 输出并将诊断写入 `report`。
pub fn check_clippy(ctx: &Ctx, report: &mut Report, json_mode: bool) -> Result<()> {
    let mut args = vec!["clippy".to_string(), "--all-targets".to_string()];
    push_feature_args(ctx, &mut args);

    if json_mode {
        args.push("--message-format=json".to_string());
        args.extend(["--".to_string(), "-D".to_string(), "warnings".to_string()]);
        run_cargo_json(ctx, report, args, "CLIPPY")
    } else {
        args.extend(["--".to_string(), "-D".to_string(), "warnings".to_string()]);
        run_cargo_passthrough(ctx, args, None)
    }
}

// ──────────────────────────────── 内部工具函数 ────────────────────────────────

/// 将 features 参数追加到 args 列表中（如果有配置）。
fn push_feature_args(ctx: &Ctx, args: &mut Vec<String>) {
    if ctx.all_features {
        args.push("--all-features".to_string());
    } else if !ctx.features.is_empty() {
        args.push("--features".to_string());
        args.push(ctx.features.join(","));
    }
}

/// Human 模式：直接运行 cargo 命令，stderr 透传，若失败返回 Err。
///
/// `extra_rustflags`：若指定，则追加到 `RUSTFLAGS` 环境变量（适用于不支持 `-- <flag>` 的子命令，如 `cargo check`）。
fn run_cargo_passthrough(
    ctx: &Ctx,
    args: Vec<String>,
    extra_rustflags: Option<&str>,
) -> Result<()> {
    let mut cmd = Command::new("cargo");
    cmd.args(&args)
        .current_dir(&ctx.root)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    if let Some(flags) = extra_rustflags {
        // 保留环境中已有的 RUSTFLAGS，追加新 flag
        let existing = std::env::var("RUSTFLAGS").unwrap_or_default();
        let merged = if existing.is_empty() {
            flags.to_string()
        } else {
            format!("{existing} {flags}")
        };
        cmd.env("RUSTFLAGS", merged);
    }

    let status = cmd.status()?;

    if status.success() {
        Ok(())
    } else {
        Err(anyhow!(
            "cargo {} failed with exit code {}",
            args.first().map(String::as_str).unwrap_or("?"),
            status.code().unwrap_or(-1)
        ))
    }
}

/// JSON 模式：运行 cargo 命令并解析其 JSON 输出流，填充 report。
fn run_cargo_json(ctx: &Ctx, report: &mut Report, args: Vec<String>, prefix: &str) -> Result<()> {
    let mut child = Command::new("cargo")
        .args(&args)
        .current_dir(&ctx.root)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("Failed to capture stdout"))?;
    let reader = BufReader::new(stdout);

    for line in reader.lines() {
        process_json_line(&line?, ctx, report, prefix)?;
    }

    child.wait()?;
    Ok(())
}

/// 处理单行 JSON 输出。
fn process_json_line(line: &str, ctx: &Ctx, report: &mut Report, prefix: &str) -> Result<()> {
    let value: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => return Ok(()),
    };

    if value["reason"] != "compiler-message" {
        return Ok(());
    }

    let msg = &value["message"];
    let lvl = msg["level"].as_str().unwrap_or("");

    if lvl == "warning" || lvl == "error" {
        process_compiler_message(msg, ctx, report, prefix, lvl);
    }

    Ok(())
}

/// 处理编译器消息。
fn process_compiler_message(msg: &Value, ctx: &Ctx, report: &mut Report, prefix: &str, lvl: &str) {
    let text = msg["message"].as_str().unwrap_or("Unknown error");
    let code = msg["code"]["code"].as_str().unwrap_or("GENERIC");

    let Some(spans) = msg["spans"].as_array() else {
        return;
    };

    for span in spans {
        if !span["is_primary"].as_bool().unwrap_or(false) {
            continue;
        }

        add_diagnostic_from_span(span, ctx, report, prefix, code, text, lvl);
    }
}

/// 从 span 信息中提取位置并添加诊断。
fn add_diagnostic_from_span(
    span: &Value,
    ctx: &Ctx,
    report: &mut Report,
    prefix: &str,
    code: &str,
    text: &str,
    lvl: &str,
) {
    let file_name = span["file_name"].as_str().unwrap_or("");
    let line = span["line_start"].as_u64().unwrap_or(0) as usize;
    let col = span["column_start"].as_u64().unwrap_or(0) as usize;

    let abs_path = ctx.root.join(file_name);

    if is_ignored(ctx, &abs_path) {
        return;
    }

    let code_str = format!("{}-{}", prefix, code);
    let mut diag = if lvl == "warning" {
        Diag::warning(abs_path, line, col, &code_str, text)
    } else {
        Diag::error(abs_path, line, col, &code_str, text)
    };

    if let Some(suggest) = span["suggested_replacement"].as_str() {
        diag = diag.with_suggestion(&format!("Try replacing with: `{}`", suggest));
    }

    report.add(diag);
}

/// 检查文件是否应被忽略。
fn is_ignored(ctx: &Ctx, path: &Path) -> bool {
    ctx.find_package(path)
        .is_some_and(|pkg| ctx.is_ignored(pkg, path))
}
