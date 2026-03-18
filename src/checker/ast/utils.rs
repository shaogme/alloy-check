use crate::report::Diagnostic as Diag;
use crate::report::Report;
use quote::ToTokens;
use std::path::Path;
use syn::visit::{self, Visit};

pub(crate) fn check_id_length(report: &mut Report, file: &Path, id: &syn::Ident, is_method: bool) {
    let name = id.to_string();
    let len = name.len();
    let s = id.span().start();

    let (warn_limit, err_limit) = if is_method { (20, 25) } else { (25, 30) };

    if len > err_limit {
        let msg = format!(
            "Identifier `{}` is too long ({} chars, max {}).",
            name, len, err_limit
        );
        report.add(
            Diag::error(file.to_path_buf(), s.line, s.column, "ID001", &msg)
                .with_suggestion("Rename the item to a shorter, more concise name."),
        );
    } else if len > warn_limit {
        let msg = format!(
            "Identifier `{}` is long ({} chars, suggested max {}).",
            name, len, warn_limit
        );
        report.add(
            Diag::warning(file.to_path_buf(), s.line, s.column, "ID001", &msg)
                .with_suggestion("Consider renaming the item to a shorter name."),
        );
    }
}

pub(crate) fn check_doc(
    report: &mut Report,
    file: &Path,
    vis: &syn::Visibility,
    attrs: &[syn::Attribute],
    id: &syn::Ident,
    kind: &str,
) {
    if !matches!(vis, syn::Visibility::Public(_)) {
        return;
    }

    let has_doc = attrs.iter().any(|a| a.path().is_ident("doc"));

    let might_gen_doc = attrs.iter().any(|a| {
        let path_str = a.path().to_token_stream().to_string().replace(" ", "");
        ![
            "derive",
            "cfg",
            "allow",
            "warn",
            "deny",
            "test",
            "inline",
            "must_use",
            "repr",
            "non_exhaustive",
            "default",
        ]
        .contains(&path_str.as_str())
    });

    if !has_doc && !might_gen_doc {
        let s = id.span().start();
        let msg = format!("Public {} `{}` is missing doc comments.", kind, id);
        report.add(
            Diag::error(file.to_path_buf(), s.line, s.column, "DOC001", &msg)
                .with_suggestion("Add `///` documentation above the item."),
        );
    }
}

pub(crate) fn is_allowed_path_prefix(path: &str) -> bool {
    path.starts_with("self")
}

pub(crate) fn is_reserved_prefix(path: &str) -> bool {
    ["std", "core", "alloc", "crate", "super", "self"]
        .iter()
        .any(|&p| path.starts_with(p))
}

pub(crate) fn has_test_attr(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|a| {
        let path = a.path();
        if path.is_ident("test") || path.is_ident("rstest") || path.is_ident("test_case") {
            return true;
        }
        // Handle #[tokio::test], #[async_std::test], #[test_log::test], etc.
        if let Some(last) = path.segments.last()
            && (last.ident == "test" || last.ident == "test_case")
        {
            return true;
        }
        if path.is_ident("cfg") {
            let mut is_test = false;
            let _ = a.parse_nested_meta(|meta| {
                if meta.path.is_ident("test") {
                    is_test = true;
                }
                Ok(())
            });
            return is_test;
        }
        false
    })
}

#[derive(Default)]
pub(crate) struct NestingVisitor {
    current_depth: usize,
    pub(crate) max_depth: usize,
}

impl<'ast> Visit<'ast> for NestingVisitor {
    fn visit_block(&mut self, i: &'ast syn::Block) {
        self.current_depth += 1;
        self.max_depth = self.max_depth.max(self.current_depth);
        visit::visit_block(self, i);
        self.current_depth -= 1;
    }
}

pub(crate) fn extract_single_call(block: &syn::Block) -> Option<&syn::ExprCall> {
    let mut call_expr = None;
    let mut non_use_stmts = 0;

    for stmt in &block.stmts {
        if let syn::Stmt::Item(syn::Item::Use(_)) = stmt {
            continue;
        }
        if non_use_stmts == 0
            && let syn::Stmt::Expr(e, _) = stmt
        {
            call_expr = Some(e);
        }
        non_use_stmts += 1;
    }

    let expr = if non_use_stmts == 1 { call_expr } else { None };

    let inner_expr = match expr {
        Some(syn::Expr::Return(ret)) => ret.expr.as_deref(),
        Some(e) => Some(e),
        None => None,
    };

    if let Some(syn::Expr::Call(call)) = inner_expr {
        Some(call)
    } else {
        None
    }
}

pub(crate) fn count_code_lines(source_text: &str, block: &syn::Block) -> usize {
    let start = block.brace_token.span.join().start().line;
    let end = block.brace_token.span.join().end().line;
    let lines_text: Vec<&str> = source_text.lines().collect();
    let mut lines = 0;
    for j in start..end.saturating_sub(1) {
        if j < lines_text.len() {
            let l = lines_text[j].trim();
            if !l.is_empty() && !l.starts_with("//") && !l.starts_with("/*") && !l.starts_with('*')
            {
                lines += 1;
            }
        }
    }
    lines
}
