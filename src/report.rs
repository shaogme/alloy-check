use colored::*;
use std::path::PathBuf;

use serde::Serialize;
use Severity::{Error, Warning};

/// 表示诊断信息的严重程度。
#[derive(Debug, Clone, Serialize)]
pub enum Severity {
    /// 错误级别，通常会导致检查失败。
    Error,
    /// 警告级别，提示潜在问题但不一定会导致失败。
    Warning,
}

/// 表示一条诊断信息，包含位置、消息和代码。
#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    /// 发生问题的文件路径。
    pub file: PathBuf,
    /// 所在行号（从 1 开始）。
    pub line: usize,
    /// 所在列号（从 1 开始）。
    pub column: usize,
    /// 详细的描述消息。
    pub message: String,
    /// 严重程度。
    pub severity: Severity,
    /// 诊断代码（如 "PATH001"）。
    pub code: String,
    /// 可选的修复建议。
    pub suggestion: Option<String>,
}

impl Diagnostic {
    /// 创建一条错误级别的诊断信息。
    pub fn error(file: PathBuf, line: usize, column: usize, code: &str, message: &str) -> Self {
        Self {
            file,
            line,
            column,
            message: message.to_string(),
            severity: Error,
            code: code.to_string(),
            suggestion: None,
        }
    }

    /// 创建一条警告级别的诊断信息。
    pub fn warning(file: PathBuf, line: usize, column: usize, code: &str, message: &str) -> Self {
        Self {
            file,
            line,
            column,
            message: message.to_string(),
            severity: Warning,
            code: code.to_string(),
            suggestion: None,
        }
    }

    /// 为诊断信息添加修复建议。
    pub fn with_suggestion(mut self, suggestion: &str) -> Self {
        self.suggestion = Some(suggestion.to_string());
        self
    }
}

/// 包含多个诊断信息的报告。
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// 所有的诊断信息列表。
    pub diagnostics: Vec<Diagnostic>,
}

impl Default for Report {
    fn default() -> Self {
        Self::new()
    }
}

impl Report {
    /// 创建一个新的空报告。
    pub fn new() -> Self {
        Self {
            diagnostics: Vec::new(),
        }
    }

    /// 向报告中添加一条诊断信息。
    pub fn add(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    /// 检查报告中是否包含任何错误级别的诊断。
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| matches!(d.severity, Error))
    }

    /// 将所有诊断信息输出。
    pub fn write_human(&self, mut writer: impl std::io::Write) -> std::io::Result<()> {
        for diag in &self.diagnostics {
            let severity_label = match diag.severity {
                Error => "error".red().bold(),
                Warning => "warning".yellow().bold(),
            };

            writeln!(
                writer,
                "{}[{}]: {}\n  --> {}:{}:{}\n",
                severity_label,
                diag.code.cyan(),
                diag.message.bold(),
                diag.file.display(),
                diag.line,
                diag.column
            )?;

            if let Some(suggestion) = &diag.suggestion {
                writeln!(writer, "  {} {}\n", "help:".blue().bold(), suggestion)?;
            }
        }

        if !self.diagnostics.is_empty() {
            let error_count = self
                .diagnostics
                .iter()
                .filter(|d| matches!(d.severity, Error))
                .count();
            let warning_count = self
                .diagnostics
                .iter()
                .filter(|d| matches!(d.severity, Warning))
                .count();

            writeln!(
                writer,
                "Summary: {} errors, {} warnings",
                error_count.to_string().red().bold(),
                warning_count.to_string().yellow().bold()
            )?;
        }
        Ok(())
    }

    /// 将由诊断信息序列化为 RON 格式并输出
    pub fn write_ron(&self, mut writer: impl std::io::Write) -> std::io::Result<()> {
        let options = ron::ser::PrettyConfig::new()
            .depth_limit(2)
            .separate_tuple_members(true)
            .enumerate_arrays(true);
        match ron::ser::to_string_pretty(self, options) {
            Ok(s) => writeln!(writer, "{}", s),
            Err(e) => writeln!(writer, "Failed to serialize report to RON: {}", e),
        }
    }
}
