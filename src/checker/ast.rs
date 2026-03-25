pub mod index;
pub mod utils;
pub mod visitor;

use crate::report::Diagnostic as Diag;
use crate::report::Report;
use crate::workspace::WorkspaceContext as Ctx;
use anyhow::Result;
use rayon::prelude::*;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
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

    let index = build_index(ctx, &paths)?;

    let diags: Vec<_> = paths
        .into_par_iter()
        .flat_map(|path| {
            let mut local_report = Report::new();
            if let Err(e) = process_rs_file(ctx, &mut local_report, &path, &index) {
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

fn build_index(ctx: &Ctx, paths: &[PathBuf]) -> Result<index::SymbolIndex> {
    use std::collections::{HashMap, HashSet};
    use std::sync::Mutex;

    let free_fns = Mutex::new(HashMap::<String, HashSet<String>>::new());
    let inherent_methods = Mutex::new(HashMap::<String, HashSet<String>>::new());
    let trait_methods = Mutex::new(HashSet::<String>::new());

    paths.par_iter().try_for_each(|path| -> Result<()> {
        let pkg = ctx.find_package(path);
        let Some(package) = pkg else {
            return Ok(());
        };
        if ctx.is_ignored(package, path) {
            return Ok(());
        }

        let content = fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("Failed to read file {:?}: {}", path, e))?;
        let syntax = syn::parse_file(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse file {:?}: {}", path, e))?;

        let mut local_fns = HashSet::new();
        let mut local_inherent = HashSet::new();
        let mut local_trait = HashSet::new();
        let mut visitor = index::IndexVisitor {
            free_fns: &mut local_fns,
            inherent_methods: &mut local_inherent,
            trait_methods: &mut local_trait,
        };
        visitor.visit_file(&syntax);

        if !local_fns.is_empty() {
            let mut lock = free_fns
                .lock()
                .map_err(|e| anyhow::anyhow!("Free fns mutex poisoned: {}", e))?;
            lock.entry(package.name.to_string())
                .or_default()
                .extend(local_fns);
        }
        if !local_inherent.is_empty() {
            let mut lock = inherent_methods
                .lock()
                .map_err(|e| anyhow::anyhow!("Inherent methods mutex poisoned: {}", e))?;
            lock.entry(package.name.to_string())
                .or_default()
                .extend(local_inherent);
        }
        if !local_trait.is_empty() {
            let mut lock = trait_methods
                .lock()
                .map_err(|e| anyhow::anyhow!("Trait methods mutex poisoned: {}", e))?;
            lock.extend(local_trait);
        }
        Ok(())
    })?;

    Ok(index::SymbolIndex {
        free_fns: free_fns
            .into_inner()
            .map_err(|e| anyhow::anyhow!("Free fns mutex poisoned: {}", e))?,
        inherent_methods: inherent_methods
            .into_inner()
            .map_err(|e| anyhow::anyhow!("Inherent methods mutex poisoned: {}", e))?,
        trait_methods: trait_methods
            .into_inner()
            .map_err(|e| anyhow::anyhow!("Trait methods mutex poisoned: {}", e))?,
    })
}

fn process_rs_file(
    ctx: &Ctx,
    report: &mut Report,
    path: &Path,
    index: &index::SymbolIndex,
) -> Result<()> {
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

    let pkg = ctx.find_package(path);
    let Some(package) = pkg.filter(|p| !ctx.is_ignored(p, path)) else {
        return Ok(());
    };

    let content = match read_file_content(path, report) {
        Some(c) => c,
        None => return Ok(()),
    };

    check_file_length(path, &content, report);
    parse_and_visit(path, &content, report, package.name.to_string(), index);

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

fn parse_and_visit(
    path: &Path,
    content: &str,
    report: &mut Report,
    package_name: String,
    index: &index::SymbolIndex,
) {
    match syn::parse_file(content) {
        Ok(syntax) => {
            let mut visitor = AstVisitor::new(report, path, content, package_name, index);
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
    if !is_mod {
        return false;
    }

    if !ctx.find_package(path).is_some_and(|p| !ctx.is_ignored(p, path)) {
        return false;
    }

    // 获取相对于工作区根目录的路径
    let rel_path = path.strip_prefix(&ctx.root).unwrap_or(path);
    let p_str = rel_path.to_str().unwrap_or("").replace('\\', "/");
    let components: Vec<&str> = p_str.split('/').collect();

    let in_src = components.iter().any(|&c| c == "src");
    let in_tests = components.iter().any(|&c| c == "tests");
    let in_benches = components.iter().any(|&c| c == "benches");

    // 允许在 tests/ 或 benches/ 下使用，除非路径中包含 src/
    let allowed = (in_tests || in_benches) && !in_src;
    !allowed
}
