use crate::report::Diagnostic as Diag;
use crate::report::Report;
use quote::ToTokens;
use std::path::Path;
use syn::spanned::Spanned;
use syn::visit::{self, Visit};

use super::utils::*;

pub(crate) struct AstVisitor<'a> {
    pub report: &'a mut Report,
    pub current_file: &'a Path,
    pub source_text: &'a str,
    pub in_test_ctx: bool,
    pub in_trait_impl: bool,
    pub in_const_ctx: bool,
}

impl<'a> AstVisitor<'a> {
    /// Creates a new `AstVisitor`.
    pub fn new(report: &'a mut Report, current_file: &'a Path, source_text: &'a str) -> Self {
        Self {
            report,
            current_file,
            source_text,
            in_test_ctx: false,
            in_trait_impl: false,
            in_const_ctx: false,
        }
    }

    fn check_safety_comment(&mut self, span: proc_macro2::Span) {
        let line = span.start().line;
        if line <= 1 {
            return;
        }
        let lines: Vec<&str> = self.source_text.lines().collect();
        let mut found_safety = false;
        for j in (0..line - 1).rev() {
            let l = lines[j].trim();
            if l.is_empty() {
                continue;
            }
            if l.starts_with("//") {
                if l.contains("SAFETY:") {
                    found_safety = true;
                    break;
                }
            } else {
                break;
            }
        }

        if !found_safety {
            let start = span.start();
            self.report.add(
                Diag::error(
                    self.current_file.to_path_buf(),
                    start.line,
                    start.column,
                    "SAFE003",
                    "Missing `// SAFETY:` comment above unsafe block or item.",
                )
                .with_suggestion("Add `// SAFETY: [reason]` to document why this is safe."),
            );
        }
    }

    fn check_function_alias(&mut self, sig: &syn::Signature, block: &syn::Block) {
        let Some(call) = extract_single_call(block) else {
            return;
        };

        let mut param_idents = Vec::new();
        for param in &sig.inputs {
            if let syn::FnArg::Typed(pat_type) = param {
                if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                    param_idents.push(pat_ident.ident.to_string());
                } else {
                    return;
                }
            } else if let syn::FnArg::Receiver(_) = param {
                param_idents.push("self".to_string());
            }
        }

        let mut arg_idents = Vec::new();
        for arg in &call.args {
            if let syn::Expr::Path(expr_path) = arg {
                if let Some(ident) = expr_path.path.get_ident() {
                    arg_idents.push(ident.to_string());
                } else {
                    return;
                }
            } else {
                return;
            }
        }

        if param_idents == arg_idents {
            let s = sig.ident.span().start();
            let msg = format!("Function `{}` is a simple alias wrapper.", sig.ident);
            self.report.add(
                Diag::error(
                    self.current_file.to_path_buf(),
                    s.line,
                    s.column,
                    "FUNC003",
                    &msg,
                )
                .with_suggestion("Remove the alias and use the inner function directly."),
            );
        }
    }

    fn check_function_length(&mut self, sig: &syn::Signature, block: &syn::Block) {
        let i = &sig.ident;
        let lines = count_code_lines(self.source_text, block);
        let pos = i.span().start();

        if lines > 100 {
            let msg = format!("Function `{}` is too long ({} lines, max 100).", i, lines);
            self.report.add(
                Diag::error(
                    self.current_file.to_path_buf(),
                    pos.line,
                    pos.column,
                    "FUNC001",
                    &msg,
                )
                .with_suggestion("Refactor the function into smaller components."),
            );
        } else if lines > 75 {
            let msg = format!("Function `{}` is long ({} lines, max 75).", i, lines);
            self.report.add(
                Diag::warning(
                    self.current_file.to_path_buf(),
                    pos.line,
                    pos.column,
                    "FUNC001",
                    &msg,
                )
                .with_suggestion("Consider refactoring the function into smaller components."),
            );
        } else if lines > 50 {
            let msg = format!(
                "Function `{}` is getting long ({} lines, suggested max 50).",
                i, lines
            );
            self.report.add(
                Diag::warning(
                    self.current_file.to_path_buf(),
                    pos.line,
                    pos.column,
                    "FUNC001",
                    &msg,
                )
                .with_suggestion("Consider breaking down the function earlier."),
            );
        }
    }

    fn check_function_nesting(&mut self, sig: &syn::Signature, block: &syn::Block) {
        let i = &sig.ident;
        let mut v = NestingVisitor::default();
        v.visit_block(block);
        if v.max_depth > 5 {
            let pos = i.span().start();
            let msg = format!(
                "Function `{}` has too much nesting (depth {}, max 5).",
                i, v.max_depth
            );
            self.report.add(
                Diag::error(
                    self.current_file.to_path_buf(),
                    pos.line,
                    pos.column,
                    "FUNC002",
                    &msg,
                )
                .with_suggestion(
                    "Simplify the logic or extract nested blocks into separate functions.",
                ),
            );
        }
    }

    fn check_complexity(&mut self, sig: &syn::Signature, block: &syn::Block) {
        self.check_function_length(sig, block);
        self.check_function_nesting(sig, block);
    }
}

impl<'ast> Visit<'ast> for AstVisitor<'_> {
    fn visit_file(&mut self, i: &'ast syn::File) {
        let mut seen_use = false;
        let mut seen_other = false;
        for item in &i.items {
            match item {
                syn::Item::Mod(m) if !has_test_attr(&m.attrs) => {
                    if seen_use || seen_other {
                        let s = item.span().start();
                        self.report.add(
                            Diag::error(
                                self.current_file.to_path_buf(),
                                s.line,
                                s.column,
                                "PATH002",
                                "`mod` statements must be placed before `use` and other items.",
                            )
                            .with_suggestion("Move this `mod` statement further up."),
                        );
                    }
                }
                syn::Item::Use(_) => {
                    seen_use = true;
                    if seen_other {
                        let s = item.span().start();
                        self.report.add(Diag::error(self.current_file.to_path_buf(), s.line, s.column, "PATH002", "`use` statements must be placed before other implementation items.")
                            .with_suggestion("Move this `use` statement further up (after `mod` but before other items)."));
                    }
                }
                _ => {
                    seen_other = true;
                }
            }
        }
        visit::visit_file(self, i);
    }

    fn visit_block(&mut self, i: &'ast syn::Block) {
        let mut first_non_use = false;
        for stmt in &i.stmts {
            if let syn::Stmt::Item(syn::Item::Use(_)) = stmt {
                if first_non_use {
                    let s = stmt.span().start();
                    self.report.add(
                        Diag::error(
                            self.current_file.to_path_buf(),
                            s.line,
                            s.column,
                            "PATH002",
                            "`use` statements must be at the top of the block.",
                        )
                        .with_suggestion("Move this `use` statement to the top of the block."),
                    );
                }
            } else {
                first_non_use = true;
            }
        }
        visit::visit_block(self, i);
    }

    fn visit_path(&mut self, i: &'ast syn::Path) {
        if i.leading_colon.is_none() && i.segments.len() > 1 {
            let mut prefix_parts = Vec::new();
            for (idx, seg) in i.segments.iter().enumerate() {
                if idx == i.segments.len() - 1 {
                    break;
                }
                prefix_parts.push(seg.ident.to_string());
            }
            let prefix_path = prefix_parts.join("::");

            if !prefix_path.is_empty() && !is_allowed_path_prefix(&prefix_path) {
                let limit = 15;
                let module_limit = 20;

                let is_longer_allowed = !is_reserved_prefix(&prefix_path);
                let current_limit = if is_longer_allowed {
                    module_limit
                } else {
                    limit
                };

                if prefix_path.len() > current_limit {
                    let s = i.span().start();
                    let msg = format!(
                        "Path prefix `{}` exceeds {} characters.",
                        prefix_path, current_limit
                    );
                    self.report.add(
                        Diag::error(
                            self.current_file.to_path_buf(),
                            s.line,
                            s.column,
                            "PATH001",
                            &msg,
                        )
                        .with_suggestion("Import the item with a `use` statement instead."),
                    );
                }
            }
        }
        visit::visit_path(self, i);
    }

    fn visit_expr_method_call(&mut self, i: &'ast syn::ExprMethodCall) {
        let m = i.method.to_string();
        if (m == "unwrap" || m == "expect") && !self.in_test_ctx && !self.in_const_ctx {
            let s = i.method.span().start();
            let msg = format!("Use of `.{}` is prohibited in non-test code.", m);
            self.report.add(
                Diag::error(
                    self.current_file.to_path_buf(),
                    s.line,
                    s.column,
                    "SAFE001",
                    &msg,
                )
                .with_suggestion("Handle error gracefully using `Result` or `Option`."),
            );
        }
        visit::visit_expr_method_call(self, i);
    }

    fn visit_macro(&mut self, i: &'ast syn::Macro) {
        let path_str = i.path.to_token_stream().to_string().replace(" ", "");
        let is_panic = path_str == "panic" || path_str == "std::panic" || path_str == "core::panic";
        let is_unreachable = path_str == "unreachable"
            || path_str == "std::unreachable"
            || path_str == "core::unreachable";
        let is_todo = path_str == "todo" || path_str == "std::todo" || path_str == "core::todo";
        let is_unimplemented = path_str == "unimplemented"
            || path_str == "std::unimplemented"
            || path_str == "core::unimplemented";

        if (is_panic || is_unreachable || is_todo || is_unimplemented)
            && !self.in_test_ctx
            && !self.in_const_ctx
        {
            let s = i.path.span().start();
            self.report.add(
                Diag::error(
                    self.current_file.to_path_buf(),
                    s.line,
                    s.column,
                    "SAFE002",
                    "Use of `panic!` or related panicking macros is prohibited.",
                )
                .with_suggestion("Return a `Result::Err` instead."),
            );
        }
        visit::visit_macro(self, i);
    }

    fn visit_expr_unsafe(&mut self, i: &'ast syn::ExprUnsafe) {
        self.check_safety_comment(i.span());
        visit::visit_expr_unsafe(self, i);
    }

    fn visit_item_mod(&mut self, i: &'ast syn::ItemMod) {
        let prev = self.in_test_ctx;
        if has_test_attr(&i.attrs) {
            self.in_test_ctx = true;
        }
        visit::visit_item_mod(self, i);
        self.in_test_ctx = prev;
    }

    fn visit_item_fn(&mut self, i: &'ast syn::ItemFn) {
        let prev = self.in_test_ctx;
        if has_test_attr(&i.attrs) {
            self.in_test_ctx = true;
        }
        if i.sig.unsafety.is_some() {
            self.check_safety_comment(i.span());
        }
        self.check_function_alias(&i.sig, &i.block);
        self.check_complexity(&i.sig, &i.block);
        if !self.in_test_ctx {
            check_id_length(self.report, self.current_file, &i.sig.ident, false);
        }
        check_doc(
            self.report,
            self.current_file,
            &i.vis,
            &i.attrs,
            &i.sig.ident,
            "function",
        );
        visit::visit_item_fn(self, i);
        self.in_test_ctx = prev;
    }

    fn visit_item_const(&mut self, i: &'ast syn::ItemConst) {
        let prev = self.in_const_ctx;
        self.in_const_ctx = true;
        visit::visit_item_const(self, i);
        self.in_const_ctx = prev;
    }

    fn visit_item_static(&mut self, i: &'ast syn::ItemStatic) {
        let prev = self.in_const_ctx;
        self.in_const_ctx = true;
        visit::visit_item_static(self, i);
        self.in_const_ctx = prev;
    }

    fn visit_item_impl(&mut self, i: &'ast syn::ItemImpl) {
        let prev = self.in_trait_impl;
        if i.trait_.is_some() {
            self.in_trait_impl = true;
        }
        if i.unsafety.is_some() {
            self.check_safety_comment(i.span());
        }
        visit::visit_item_impl(self, i);
        self.in_trait_impl = prev;
    }

    fn visit_impl_item_fn(&mut self, i: &'ast syn::ImplItemFn) {
        let prev = self.in_test_ctx;
        if has_test_attr(&i.attrs) {
            self.in_test_ctx = true;
        }
        if i.sig.unsafety.is_some() {
            self.check_safety_comment(i.span());
        }
        if !self.in_trait_impl {
            self.check_function_alias(&i.sig, &i.block);
            if !self.in_test_ctx {
                check_id_length(self.report, self.current_file, &i.sig.ident, true);
            }
        }
        self.check_complexity(&i.sig, &i.block);
        check_doc(
            self.report,
            self.current_file,
            &i.vis,
            &i.attrs,
            &i.sig.ident,
            "method",
        );
        visit::visit_impl_item_fn(self, i);
        self.in_test_ctx = prev;
    }

    fn visit_item(&mut self, i: &'ast syn::Item) {
        match i {
            syn::Item::Struct(it) => check_doc(
                self.report,
                self.current_file,
                &it.vis,
                &it.attrs,
                &it.ident,
                "struct",
            ),
            syn::Item::Enum(it) => check_doc(
                self.report,
                self.current_file,
                &it.vis,
                &it.attrs,
                &it.ident,
                "enum",
            ),
            syn::Item::Trait(it) => {
                if it.unsafety.is_some() {
                    self.check_safety_comment(it.span());
                }
                check_doc(
                    self.report,
                    self.current_file,
                    &it.vis,
                    &it.attrs,
                    &it.ident,
                    "trait",
                );
            }
            syn::Item::Type(it) => check_doc(
                self.report,
                self.current_file,
                &it.vis,
                &it.attrs,
                &it.ident,
                "type alias",
            ),
            _ => {}
        }
        visit::visit_item(self, i);
    }
}
