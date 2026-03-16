pub mod utils;
pub mod visitor;

use crate::report::Diagnostic as Diag;
use crate::report::Report;
use crate::workspace::WorkspaceContext as Ctx;
use anyhow::Result;
use rayon::prelude::*;
use std::fs;
use std::path::Path;
use syn::visit::Visit;
use walkdir::WalkDir;

use visitor::AstVisitor;

/// 遍历工作空间并在所有 Rust 文件上运行 AST 分析。
pub fn check(ctx: &Ctx, report: &mut Report) -> Result<()> {
    let paths: Vec<_> = WalkDir::new(&ctx.root)
        .into_iter()
        .filter_map(|e| e.ok())
        .map(|e| e.path().to_path_buf())
        .filter(|p| p.extension().is_some_and(|s| s == "rs"))
        .collect();

    let diags: Vec<_> = paths
        .into_par_iter()
        .flat_map(|path| {
            let mut local_report = Report::new();
            if let Err(e) = process_rs_file(ctx, &mut local_report, &path) {
                local_report.add(Diag::error(
                    path.clone(),
                    1,
                    1,
                    "IO002",
                    &format!("Failed to process file: {}", e),
                ));
            }
            local_report.diagnostics
        })
        .collect();

    report.diagnostics.extend(diags);
    Ok(())
}

fn process_rs_file(ctx: &Ctx, report: &mut Report, path: &Path) -> Result<()> {
    if is_prohibited_mod_rs(ctx, path) {
        report.add(
            Diag::error(
                path.to_path_buf(),
                1,
                1,
                "PATH003",
                "The use of `mod.rs` is prohibited.",
            )
            .with_suggestion("Rename the file to match the directory name and move it up."),
        );
    }

    if ctx
        .find_package(path)
        .filter(|p| !ctx.is_ignored(p, path))
        .is_none()
    {
        return Ok(());
    }

    let content = match read_file_content(path, report) {
        Some(c) => c,
        None => return Ok(()),
    };

    check_file_length(path, &content, report);
    parse_and_visit(path, &content, report);

    Ok(())
}

fn read_file_content(path: &Path, report: &mut Report) -> Option<String> {
    match fs::read_to_string(path) {
        Ok(c) => Some(c),
        Err(e) => {
            report.add(Diag::error(
                path.to_path_buf(),
                1,
                1,
                "IO001",
                &format!("Failed to read file: {}", e),
            ));
            None
        }
    }
}

fn check_file_length(path: &Path, content: &str, report: &mut Report) {
    let line_count = content.lines().count();
    if line_count > 800 {
        report.add(
            Diag::error(
                path.to_path_buf(),
                1,
                1,
                "FILE001",
                &format!("File is too long ({} lines, max 800).", line_count),
            )
            .with_suggestion("Split the file into smaller modules."),
        );
    } else if line_count > 650 {
        report.add(
            Diag::warning(
                path.to_path_buf(),
                1,
                1,
                "FILE001",
                &format!(
                    "File is getting long ({} lines, suggested max 650).",
                    line_count
                ),
            )
            .with_suggestion("Consider splitting the file into smaller modules."),
        );
    }
}

fn parse_and_visit(path: &Path, content: &str, report: &mut Report) {
    match syn::parse_file(content) {
        Ok(syntax) => {
            let mut visitor = AstVisitor::new(report, path, content);
            visitor.visit_file(&syntax);
        }
        Err(e) => {
            report.add(
                Diag::error(
                    path.to_path_buf(),
                    1,
                    1,
                    "AST001",
                    &format!("Failed to parse Rust source code: {}", e),
                )
                .with_suggestion("Check for syntax errors."),
            );
        }
    }
}

fn is_prohibited_mod_rs(ctx: &Ctx, path: &Path) -> bool {
    let is_mod = path.file_name().and_then(|n| n.to_str()) == Some("mod.rs");
    is_mod
        && ctx
            .find_package(path)
            .is_some_and(|p| !ctx.is_ignored(p, path))
}
