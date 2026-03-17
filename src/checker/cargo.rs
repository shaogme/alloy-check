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

/// 运行 `cargo clippy` 并解析其 JSON 输出。
pub fn check_clippy(ctx: &Ctx, report: &mut Report) -> Result<()> {
    let mut args = vec![
        "clippy".to_string(),
        "--all-targets".to_string(),
        "--message-format=json".to_string(),
    ];

    if ctx.all_features {
        args.push("--all-features".to_string());
    }
    if !ctx.features.is_empty() {
        args.push("--features".to_string());
        args.push(ctx.features.join(" "));
    }

    args.extend(["--".to_string(), "-D".to_string(), "warnings".to_string()]);
    run_cargo_json(ctx, report, args, "CLIPPY")
}

/// 运行 `cargo check` 并解析其 JSON 输出。
pub fn check_cargo(ctx: &Ctx, report: &mut Report) -> Result<()> {
    let mut args = vec![
        "check".to_string(),
        "--all-targets".to_string(),
        "--message-format=json".to_string(),
    ];

    if ctx.all_features {
        args.push("--all-features".to_string());
    }
    if !ctx.features.is_empty() {
        args.push("--features".to_string());
        args.push(ctx.features.join(" "));
    }

    run_cargo_json(ctx, report, args, "CHECK")
}

/// 运行 cargo 命令并处理其 JSON 输出。
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
