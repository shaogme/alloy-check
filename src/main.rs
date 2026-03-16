mod checker;
mod report;
mod workspace;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use std::process;

use checker::ast;
use checker::cargo;
use checker::metadata;
use report::Report;
use workspace::WorkspaceContext as Ctx;

#[derive(clap::ValueEnum, Clone, Debug, Default, PartialEq)]
enum Format {
    #[default]
    Human,
    Ron,
}

/// Alloy-Check: A tool to enforce strict Rust workspace standards.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the workspace root (defaults to current directory)
    #[arg(short, long, default_value = ".")]
    path: PathBuf,

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,

    /// Output format (human, ron)
    #[arg(short, long, value_enum, default_value_t = Format::Human)]
    format: Format,

    /// Optional path to write output file
    #[arg(short, long)]
    output: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    if let Err(e) = run(&args) {
        eprintln!("Error: {}", e);
        process::exit(1);
    }
    Ok(())
}

fn run(args: &Args) -> Result<()> {
    let mut report = Report::new();

    if args.verbose {
        println!("Checking workspace at: {:?}", args.path);
    }

    // 加载工作空间上下文
    let ctx = Ctx::load(&args.path)?;

    if args.verbose {
        print_ctx_info(&ctx);
    }

    run_all_checks(&ctx, &mut report, args.verbose)?;

    let mut writer: Box<dyn std::io::Write> = if let Some(path) = &args.output {
        colored::control::set_override(false);
        Box::new(std::fs::File::create(path)?)
    } else {
        Box::new(std::io::stdout())
    };

    if args.format == Format::Ron {
        report.write_ron(&mut writer)?;
    } else {
        report.write_human(&mut writer)?;
        if args.output.is_none() && !report.has_errors() {
            println!("Alloy-Check: All checks passed!");
        }
    }

    if report.has_errors() {
        process::exit(1);
    }

    Ok(())
}

fn print_ctx_info(ctx: &Ctx) {
    println!("Found workspace root: {:?}", ctx.root);
    let names: Vec<_> = ctx.members().iter().map(|p| &p.name).collect();
    println!("Workspace members: {:?}", names);
}

fn run_all_checks(ctx: &Ctx, report: &mut Report, verbose: bool) -> Result<()> {
    // Cargo 检查
    if verbose {
        println!("Running cargo fmt check...");
    }
    cargo::check_fmt(ctx, report)?;

    if verbose {
        println!("Running cargo check...");
    }
    cargo::check_cargo(ctx, report)?;

    if verbose {
        println!("Running cargo clippy...");
    }
    cargo::check_clippy(ctx, report)?;

    // 元数据验证
    if verbose {
        println!("Running metadata validation...");
    }
    metadata::check(ctx, report)?;

    // AST 分析
    if verbose {
        println!("Running AST analysis...");
    }
    ast::check(ctx, report)?;

    Ok(())
}
