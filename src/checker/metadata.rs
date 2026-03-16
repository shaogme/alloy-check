use crate::report::Diagnostic as Diag;
use crate::report::Report;
use crate::workspace::WorkspaceContext as Ctx;
use anyhow::Result;
use cargo_metadata::Package;
use std::path::Path;

/// 验证工作空间中每个包的元数据（如 Edition、描述、许可证）是否符合标准。
pub fn check(ctx: &Ctx, report: &mut Report) -> Result<()> {
    for package in ctx.members() {
        check_package(package, report);
    }
    Ok(())
}

/// 检查单个包的元数据。
fn check_package(pkg: &Package, report: &mut Report) {
    let path = pkg.manifest_path.clone().into_std_path_buf();

    check_edition(pkg, &path, report);
    check_description(pkg, &path, report);
    check_license(pkg, &path, report);
}

/// 检查 Rust 版本是否为 2024 或更高。
fn check_edition(pkg: &Package, path: &Path, report: &mut Report) {
    let ed = pkg.edition.as_str();
    let is_old = if let Ok(year) = ed.parse::<u32>() {
        year < 2024
    } else {
        ed != "2024"
    };

    if is_old {
        let msg = format!(
            "Package `{}` must use edition 2024 or later (currently {})",
            pkg.name, ed
        );
        report.add(
            Diag::error(path.to_path_buf(), 0, 0, "META001", &msg)
                .with_suggestion("Set `edition = \"2024\"` in Cargo.toml"),
        );
    }
}

/// 检查是否配置了非空的包描述。
fn check_description(pkg: &Package, path: &Path, report: &mut Report) {
    let has_desc = pkg
        .description
        .as_ref()
        .is_some_and(|d| !d.trim().is_empty());

    if !has_desc {
        let msg = format!("Package `{}` is missing a description", pkg.name);
        report.add(
            Diag::error(path.to_path_buf(), 0, 0, "META002", &msg)
                .with_suggestion("Add `description = \"...\"` to [package] section"),
        );
    }
}

/// 检查是否配置了有效的许可证。
fn check_license(pkg: &Package, path: &Path, report: &mut Report) {
    let has_license = pkg.license.as_ref().is_some_and(|l| !l.trim().is_empty());

    if !has_license {
        let msg = format!("Package `{}` is missing a license", pkg.name);
        report.add(
            Diag::error(path.to_path_buf(), 0, 0, "META003", &msg)
                .with_suggestion("Add `license = \"MIT\"` or `license = \"Apache-2.0\"`"),
        );
    }
}
