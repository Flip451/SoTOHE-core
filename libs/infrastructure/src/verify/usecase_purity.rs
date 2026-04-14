//! Verify that `libs/usecase/src/` contains no forbidden patterns that violate
//! hexagonal architecture purity.
//!
//! Uses `syn` AST parsing for accurate detection. Comments, string literals,
//! and token boundaries are handled by the parser — no manual text scanning needed.
//! `#[cfg(test)]` and `#[test]` items are automatically excluded.

use std::path::Path;

use domain::verify::{VerifyFinding, VerifyOutcome};
use syn::spanned::Spanned;
use syn::visit::Visit;

const USECASE_SRC_DIR: &str = "libs/usecase/src";

/// Path prefixes forbidden in pure layers (domain, usecase).
/// Any path or use-import starting with these segments is flagged.
/// This list covers the entire std I/O surface — the set is finite and stable.
const FORBIDDEN_PATH_PREFIXES: &[(&str, &str)] = &[
    ("std::fs", "file I/O belongs in infrastructure/CLI"),
    ("std::net", "network I/O belongs in infrastructure"),
    ("std::process", "process management belongs in CLI"),
    ("std::io", "I/O types/traits belong in infrastructure"),
    ("std::env", "environment access belongs in CLI"),
];

/// Paths forbidden in pure layers (prefix match on segments).
/// Implicit external dependencies that should be injected as arguments.
const FORBIDDEN_PATHS: &[(&str, &str)] = &[
    ("chrono::Utc::now", "implicit time dependency; pass timestamps as arguments"),
    ("std::time::SystemTime", "system clock access; pass timestamps as arguments"),
    ("std::time::Instant", "monotonic clock access; pass timestamps as arguments"),
];

/// Macro names forbidden in pure layers.
const FORBIDDEN_MACROS: &[(&str, &str)] = &[
    ("println", "output belongs in CLI"),
    ("eprintln", "output belongs in CLI"),
    ("print", "output belongs in CLI"),
    ("eprint", "output belongs in CLI"),
];

/// Scan a source directory for forbidden patterns that violate hexagonal purity.
/// Reusable across layers (usecase, domain).
///
/// # Errors
///
/// Returns findings for each forbidden pattern found.
pub(crate) fn check_layer_purity(root: &Path, src_dir: &str, layer_label: &str) -> VerifyOutcome {
    let src = root.join(src_dir);
    if !src.is_dir() {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "{layer_label} source directory not found: {src_dir}"
        ))]);
    }

    let mut findings = Vec::new();
    scan_dir(&src, root, &mut findings);
    VerifyOutcome::from_findings(findings)
}

/// Scan `libs/usecase/src/` for forbidden patterns that violate hexagonal purity.
///
/// # Errors
///
/// Returns findings for each forbidden pattern found.
pub fn verify(root: &Path) -> VerifyOutcome {
    check_layer_purity(root, USECASE_SRC_DIR, "Usecase")
}

fn scan_dir(dir: &Path, root: &Path, findings: &mut Vec<VerifyFinding>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            let rel = dir.strip_prefix(root).unwrap_or(dir);
            findings.push(VerifyFinding::error(format!(
                "{}: cannot read directory: {e}",
                rel.to_string_lossy()
            )));
            return;
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                let rel = dir.strip_prefix(root).unwrap_or(dir);
                findings.push(VerifyFinding::error(format!(
                    "{}: cannot read entry: {e}",
                    rel.to_string_lossy()
                )));
                continue;
            }
        };
        let path = entry.path();
        if path.is_dir() {
            scan_dir(&path, root, findings);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    let rel = path.strip_prefix(root).unwrap_or(&path);
                    check_content(&rel.to_string_lossy(), &content, findings);
                }
                Err(e) => {
                    let rel = path.strip_prefix(root).unwrap_or(&path);
                    findings.push(VerifyFinding::error(format!(
                        "{}: cannot read file: {e}",
                        rel.to_string_lossy()
                    )));
                }
            }
        }
    }
}

fn check_content(rel_path: &str, content: &str, findings: &mut Vec<VerifyFinding>) {
    let file = match syn::parse_file(content) {
        Ok(f) => f,
        Err(e) => {
            findings.push(VerifyFinding::warning(format!("{rel_path}: failed to parse: {e}")));
            return;
        }
    };

    // Pass 1: collect use-import aliases from all top-level use statements.
    // This makes alias resolution order-independent (Rust imports are unordered).
    let mut collector = UseCollector { aliases: Vec::new() };
    syn::visit::visit_file(&mut collector, &file);

    // Pass 2: check forbidden patterns with pre-collected aliases.
    let mut visitor = PurityVisitor {
        findings: Vec::new(),
        rel_path: rel_path.to_string(),
        imported_aliases: collector.aliases,
    };
    syn::visit::visit_file(&mut visitor, &file);
    findings.extend(visitor.findings);
}

// ---------------------------------------------------------------------------
// AST visitor
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Pass 1: collect use-import aliases
// ---------------------------------------------------------------------------

struct UseCollector {
    aliases: Vec<(String, Vec<String>)>,
}

impl UseCollector {
    fn collect_use_tree(&mut self, segments: &[String], tree: &syn::UseTree) {
        match tree {
            syn::UseTree::Path(p) => {
                let mut new_seg = segments.to_vec();
                new_seg.push(p.ident.to_string());
                self.collect_use_tree(&new_seg, &p.tree);
            }
            syn::UseTree::Name(n) => {
                let name = n.ident.to_string();
                if name == "self" {
                    // `use std::process::{self}` → alias "process" → ["std", "process"]
                    if let Some(last) = segments.last() {
                        self.aliases.push((last.clone(), segments.to_vec()));
                    }
                } else {
                    let mut full = segments.to_vec();
                    full.push(name.clone());
                    self.aliases.push((name, full));
                }
            }
            syn::UseTree::Glob(_) => {
                // `use std::process::*` → record the prefix as a "glob alias" so that
                // any child name (e.g., `Command`) is resolved against it.
                // We store ("*", segments) and handle it specially in check_segments.
                self.aliases.push(("*".to_string(), segments.to_vec()));
            }
            syn::UseTree::Group(g) => {
                for item in &g.items {
                    self.collect_use_tree(segments, item);
                }
            }
            syn::UseTree::Rename(r) => {
                let name = r.ident.to_string();
                if name == "self" {
                    // `use std::process::{self as proc}` → alias "proc" → ["std", "process"]
                    self.aliases.push((r.rename.to_string(), segments.to_vec()));
                } else {
                    let mut full = segments.to_vec();
                    full.push(name);
                    self.aliases.push((r.rename.to_string(), full));
                }
            }
        }
    }
}

impl<'ast> Visit<'ast> for UseCollector {
    fn visit_item(&mut self, item: &'ast syn::Item) {
        if has_test_attr(item_attrs(item)) {
            return;
        }
        syn::visit::visit_item(self, item);
    }

    fn visit_impl_item(&mut self, item: &'ast syn::ImplItem) {
        if has_test_attr(impl_item_attrs(item)) {
            return;
        }
        syn::visit::visit_impl_item(self, item);
    }

    fn visit_trait_item(&mut self, item: &'ast syn::TraitItem) {
        if has_test_attr(trait_item_attrs(item)) {
            return;
        }
        syn::visit::visit_trait_item(self, item);
    }

    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        self.collect_use_tree(&[], &item.tree);
    }
}

// ---------------------------------------------------------------------------
// Pass 2: check forbidden patterns
// ---------------------------------------------------------------------------

struct PurityVisitor {
    findings: Vec<VerifyFinding>,
    rel_path: String,
    /// Maps local name → (full forbidden path, reason) for use-imported forbidden modules.
    /// E.g. `use std::process;` adds ("process", ["std", "process"]) so that
    /// `process::Command::new(...)` can be resolved to `std::process::Command::new`.
    imported_aliases: Vec<(String, Vec<String>)>,
}

impl PurityVisitor {
    fn report(&mut self, span: proc_macro2::Span, pattern: &str, reason: &str) {
        let line = span.start().line;
        self.findings.push(VerifyFinding::error(format!(
            "{}:{}: `{}` found — {}",
            self.rel_path, line, pattern, reason
        )));
    }

    /// Check a list of path segments against forbidden prefixes and paths.
    /// Also resolves imported aliases (e.g., `use std::process;` makes
    /// `process::Command` resolve to `std::process::Command`).
    /// Returns `true` if a finding was reported.
    fn check_segments(&mut self, segments: &[String], span: proc_macro2::Span) -> bool {
        // Direct match
        if self.check_segments_direct(segments, span) {
            return true;
        }
        // Try resolving via imported aliases. Clone to avoid borrow conflict.
        let aliases = self.imported_aliases.clone();
        if let Some(first) = segments.first() {
            for (alias, full_path) in &aliases {
                // Named alias: first segment matches alias name
                if alias != "*" && first == alias {
                    let mut resolved = full_path.clone();
                    resolved.extend_from_slice(segments.get(1..).unwrap_or_default());
                    if self.check_segments_direct(&resolved, span) {
                        return true;
                    }
                }
            }
        }
        // Glob aliases: `use std::process::*` makes `Command` resolve to
        // `std::process::Command`. Only apply to single-segment paths (bare identifiers
        // like `Command`), not multi-segment paths (like `local::foo`) which are already
        // module-qualified and not glob-imported names.
        if segments.len() == 1 {
            for (alias, full_path) in &aliases {
                if alias == "*" {
                    let mut resolved = full_path.clone();
                    resolved.extend_from_slice(segments);
                    if self.check_segments_direct(&resolved, span) {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn check_segments_direct(&mut self, segments: &[String], span: proc_macro2::Span) -> bool {
        for &(prefix, reason) in FORBIDDEN_PATH_PREFIXES {
            if matches_prefix(segments, prefix) {
                self.report(span, &format!("{prefix}::"), reason);
                return true;
            }
        }
        for &(pattern, reason) in FORBIDDEN_PATHS {
            if matches_prefix(segments, pattern) {
                self.report(span, pattern, reason);
                return true;
            }
        }
        false
    }

    /// Walk a use-tree, accumulating path segments and checking at each level.
    /// Also records imported aliases so that short-form paths like
    /// `process::Command` can be resolved to `std::process::Command`.
    fn walk_use_tree(&mut self, segments: &[String], tree: &syn::UseTree) {
        match tree {
            syn::UseTree::Path(p) => {
                let mut new_seg = segments.to_vec();
                new_seg.push(p.ident.to_string());
                if self.check_segments(&new_seg, p.ident.span()) {
                    return; // prefix matched — don't recurse further
                }
                self.walk_use_tree(&new_seg, &p.tree);
            }
            syn::UseTree::Name(n) => {
                let mut full = segments.to_vec();
                full.push(n.ident.to_string());
                self.check_segments(&full, n.ident.span());
            }
            syn::UseTree::Glob(g) => {
                self.check_segments(segments, g.span());
            }
            syn::UseTree::Group(g) => {
                for item in &g.items {
                    self.walk_use_tree(segments, item);
                }
            }
            syn::UseTree::Rename(r) => {
                let mut full = segments.to_vec();
                full.push(r.ident.to_string());
                self.check_segments(&full, r.ident.span());
            }
        }
    }
}

impl<'ast> Visit<'ast> for PurityVisitor {
    fn visit_item(&mut self, item: &'ast syn::Item) {
        if has_test_attr(item_attrs(item)) {
            return; // skip #[cfg(test)] and #[test] items entirely
        }
        syn::visit::visit_item(self, item);
    }

    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        self.walk_use_tree(&[], &item.tree);
        // Don't call syn::visit::visit_item_use — we walked the tree manually
    }

    fn visit_path(&mut self, path: &'ast syn::Path) {
        let segments = path_segments(path);
        self.check_segments(&segments, path.span());
        syn::visit::visit_path(self, path);
    }

    fn visit_impl_item(&mut self, item: &'ast syn::ImplItem) {
        if has_test_attr(impl_item_attrs(item)) {
            return;
        }
        syn::visit::visit_impl_item(self, item);
    }

    fn visit_trait_item(&mut self, item: &'ast syn::TraitItem) {
        if has_test_attr(trait_item_attrs(item)) {
            return;
        }
        syn::visit::visit_trait_item(self, item);
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        let macro_name = path_segments(&mac.path).join("::");
        for &(name, reason) in FORBIDDEN_MACROS {
            if macro_name == name {
                self.report(mac.span(), &format!("{name}!"), reason);
                break;
            }
        }
        // Try to parse macro arguments to catch forbidden patterns inside
        // common macros like `dbg!(std::fs::read("x"))` or `assert!(cond, "{}", val)`.
        // First try as a single expression, then as a comma-separated expression list.
        if let Ok(expr) = mac.parse_body::<syn::Expr>() {
            syn::visit::visit_expr(self, &expr);
        } else if let Ok(exprs) = mac.parse_body_with(
            syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated,
        ) {
            for expr in &exprs {
                syn::visit::visit_expr(self, expr);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn path_segments(path: &syn::Path) -> Vec<String> {
    path.segments.iter().map(|s| s.ident.to_string()).collect()
}

/// Check if `segments` starts with the segments of `prefix_str` (split by `::`).
fn matches_prefix(segments: &[String], prefix_str: &str) -> bool {
    let prefix: Vec<&str> = prefix_str.split("::").collect();
    segments.len() >= prefix.len()
        && segments.iter().zip(prefix.iter()).all(|(a, b)| a.as_str() == *b)
}

fn item_attrs(item: &syn::Item) -> &[syn::Attribute] {
    match item {
        syn::Item::Const(i) => &i.attrs,
        syn::Item::Enum(i) => &i.attrs,
        syn::Item::ExternCrate(i) => &i.attrs,
        syn::Item::Fn(i) => &i.attrs,
        syn::Item::ForeignMod(i) => &i.attrs,
        syn::Item::Impl(i) => &i.attrs,
        syn::Item::Macro(i) => &i.attrs,
        syn::Item::Mod(i) => &i.attrs,
        syn::Item::Static(i) => &i.attrs,
        syn::Item::Struct(i) => &i.attrs,
        syn::Item::Trait(i) => &i.attrs,
        syn::Item::TraitAlias(i) => &i.attrs,
        syn::Item::Type(i) => &i.attrs,
        syn::Item::Union(i) => &i.attrs,
        syn::Item::Use(i) => &i.attrs,
        _ => &[],
    }
}

fn trait_item_attrs(item: &syn::TraitItem) -> &[syn::Attribute] {
    match item {
        syn::TraitItem::Const(i) => &i.attrs,
        syn::TraitItem::Fn(i) => &i.attrs,
        syn::TraitItem::Type(i) => &i.attrs,
        syn::TraitItem::Macro(i) => &i.attrs,
        _ => &[],
    }
}

fn impl_item_attrs(item: &syn::ImplItem) -> &[syn::Attribute] {
    match item {
        syn::ImplItem::Const(i) => &i.attrs,
        syn::ImplItem::Fn(i) => &i.attrs,
        syn::ImplItem::Type(i) => &i.attrs,
        syn::ImplItem::Macro(i) => &i.attrs,
        _ => &[],
    }
}

fn has_test_attr(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        // #[test]
        if attr.path().is_ident("test") {
            return true;
        }
        // #[cfg(test)]
        if attr.path().is_ident("cfg") {
            let mut found = false;
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("test") {
                    found = true;
                }
                Ok(())
            });
            return found;
        }
        false
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn setup_usecase_file(root: &Path, rel: &str, content: &str) {
        let path = root.join(USECASE_SRC_DIR).join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, content).unwrap();
    }

    // --- Detection tests ---

    #[test]
    fn test_detects_std_fs_usage() {
        let tmp = TempDir::new().unwrap();
        setup_usecase_file(
            tmp.path(),
            "workflow.rs",
            "fn foo() { let _ = std::fs::read(\"x\"); }\n",
        );
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
        assert!(!outcome.findings().is_empty());
        assert!(outcome.findings()[0].to_string().contains("std::fs::"));
    }

    #[test]
    fn test_detects_chrono_utc_now() {
        let tmp = TempDir::new().unwrap();
        setup_usecase_file(tmp.path(), "workflow.rs", "fn now() { let _ = chrono::Utc::now(); }\n");
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
        assert!(!outcome.findings().is_empty());
        assert!(outcome.findings()[0].to_string().contains("chrono::Utc::now"));
    }

    #[test]
    fn test_detects_println() {
        let tmp = TempDir::new().unwrap();
        setup_usecase_file(tmp.path(), "workflow.rs", "fn foo() { println!(\"hi\"); }\n");
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
        assert!(!outcome.findings().is_empty());
        assert!(outcome.findings()[0].to_string().contains("println!"));
    }

    #[test]
    fn test_detects_eprintln() {
        let tmp = TempDir::new().unwrap();
        setup_usecase_file(tmp.path(), "workflow.rs", "fn foo() { eprintln!(\"err\"); }\n");
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
        assert!(!outcome.findings().is_empty());
        assert!(outcome.findings()[0].to_string().contains("eprintln!"));
    }

    #[test]
    fn test_detects_process_command() {
        let tmp = TempDir::new().unwrap();
        setup_usecase_file(
            tmp.path(),
            "workflow.rs",
            "fn run() { std::process::Command::new(\"ls\"); }\n",
        );
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
        assert!(!outcome.findings().is_empty());
        assert!(outcome.findings()[0].to_string().contains("std::process::"));
    }

    #[test]
    fn test_detects_use_import_of_forbidden_module() {
        let tmp = TempDir::new().unwrap();
        setup_usecase_file(tmp.path(), "workflow.rs", "use std::fs;\nfn f() {}\n");
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
        assert!(!outcome.findings().is_empty());
        assert!(outcome.findings()[0].to_string().contains("std::fs::"));
    }

    #[test]
    fn test_detects_use_import_of_forbidden_item() {
        let tmp = TempDir::new().unwrap();
        setup_usecase_file(tmp.path(), "workflow.rs", "use std::fs::read_to_string;\nfn f() {}\n");
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
        assert!(!outcome.findings().is_empty());
        assert!(outcome.findings()[0].to_string().contains("std::fs::"));
    }

    #[test]
    fn test_multiple_violations_in_one_file() {
        let tmp = TempDir::new().unwrap();
        setup_usecase_file(
            tmp.path(),
            "bad.rs",
            "fn a() { println!(\"x\"); }\nfn b() { eprintln!(\"y\"); }\n",
        );
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
        assert_eq!(outcome.findings().len(), 2);
    }

    // --- False-positive prevention tests ---

    #[test]
    fn test_clean_usecase_passes() {
        let tmp = TempDir::new().unwrap();
        setup_usecase_file(
            tmp.path(),
            "workflow.rs",
            "pub fn execute() -> Result<(), String> { Ok(()) }\n",
        );
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
        assert!(outcome.findings().is_empty());
    }

    #[test]
    fn test_ignores_test_module() {
        let tmp = TempDir::new().unwrap();
        setup_usecase_file(
            tmp.path(),
            "workflow.rs",
            "pub fn clean() {}\n\n\
             #[cfg(test)]\nmod tests {\n    fn t() { println!(\"ok\"); }\n}\n",
        );
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
        assert!(outcome.findings().is_empty());
    }

    #[test]
    fn test_ignores_test_function() {
        let tmp = TempDir::new().unwrap();
        setup_usecase_file(
            tmp.path(),
            "workflow.rs",
            "pub fn clean() {}\n\n\
             #[test]\nfn test_something() { println!(\"ok\"); }\n",
        );
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
        assert!(outcome.findings().is_empty());
    }

    #[test]
    fn test_file_starting_with_cfg_test_is_skipped() {
        let tmp = TempDir::new().unwrap();
        setup_usecase_file(
            tmp.path(),
            "test_only.rs",
            "#[cfg(test)]\nmod tests {\n    fn t() { println!(\"ok\"); }\n}\n",
        );
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
        assert!(outcome.findings().is_empty());
    }

    #[test]
    fn test_ignores_comment_lines() {
        let tmp = TempDir::new().unwrap();
        setup_usecase_file(
            tmp.path(),
            "workflow.rs",
            "// std::fs::read is not allowed here\n\
             /// Use println! for debugging\n\
             pub fn clean() {}\n",
        );
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
        assert!(outcome.findings().is_empty());
    }

    #[test]
    fn test_ignores_block_comment() {
        let tmp = TempDir::new().unwrap();
        setup_usecase_file(
            tmp.path(),
            "workflow.rs",
            "/*\nprintln!(\"debug\")\n*/\npub fn clean() {}\n",
        );
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
        assert!(outcome.findings().is_empty());
    }

    #[test]
    fn test_ignores_inline_block_comment() {
        let tmp = TempDir::new().unwrap();
        setup_usecase_file(
            tmp.path(),
            "workflow.rs",
            "fn f() { let _ = 1; /* println!(\"debug\") */ }\n",
        );
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
        assert!(outcome.findings().is_empty());
    }

    #[test]
    fn test_detects_code_after_inline_block_comment() {
        let tmp = TempDir::new().unwrap();
        setup_usecase_file(
            tmp.path(),
            "workflow.rs",
            "fn f() { /* comment */ std::process::Command::new(\"ls\"); }\n",
        );
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
        assert!(!outcome.findings().is_empty());
        assert!(outcome.findings()[0].to_string().contains("std::process::"));
    }

    #[test]
    fn test_ignores_pattern_in_string_literal() {
        let tmp = TempDir::new().unwrap();
        setup_usecase_file(
            tmp.path(),
            "workflow.rs",
            "fn f() { let _ = \"std::fs::read is forbidden\"; }\n",
        );
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
        assert!(outcome.findings().is_empty());
    }

    #[test]
    fn test_ignores_custom_macro_with_println_suffix() {
        let tmp = TempDir::new().unwrap();
        setup_usecase_file(
            tmp.path(),
            "workflow.rs",
            "macro_rules! myprintln { ($($t:tt)*) => {} }\nfn foo() { myprintln!(\"ok\"); }\n",
        );
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
        assert!(outcome.findings().is_empty());
    }

    #[test]
    fn test_ignores_identifier_with_trailing_chars() {
        let tmp = TempDir::new().unwrap();
        setup_usecase_file(
            tmp.path(),
            "workflow.rs",
            "mod chrono { pub mod Utc { pub fn nowish() {} } }\n\
             fn foo() { chrono::Utc::nowish(); }\n",
        );
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
        assert!(outcome.findings().is_empty());
    }

    // --- Infrastructure tests ---

    #[test]
    fn test_missing_usecase_dir_errors() {
        let tmp = TempDir::new().unwrap();
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_scans_subdirectories() {
        let tmp = TempDir::new().unwrap();
        setup_usecase_file(tmp.path(), "sub/deep.rs", "fn bad() { std::fs::read(\"x\"); }\n");
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
        assert!(!outcome.findings().is_empty());
    }

    #[test]
    fn test_detects_forbidden_call_inside_macro_args() {
        let tmp = TempDir::new().unwrap();
        setup_usecase_file(tmp.path(), "workflow.rs", "fn f() { dbg!(std::fs::read(\"x\")); }\n");
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
        assert!(!outcome.findings().is_empty());
        assert!(outcome.findings().iter().any(|f| f.to_string().contains("std::fs::")));
    }

    #[test]
    fn test_detects_short_form_via_use_import() {
        let tmp = TempDir::new().unwrap();
        setup_usecase_file(
            tmp.path(),
            "workflow.rs",
            "use std::process;\nfn f() { process::Command::new(\"ls\"); }\n",
        );
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
        assert!(!outcome.findings().is_empty());
        assert!(outcome.findings().iter().any(|f| { f.to_string().contains("std::process::") }));
    }

    #[test]
    fn test_detects_renamed_import() {
        let tmp = TempDir::new().unwrap();
        setup_usecase_file(
            tmp.path(),
            "workflow.rs",
            "use std::fs as file_io;\nfn f() { file_io::read(\"x\"); }\n",
        );
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
        // The `use std::fs as file_io` itself is flagged (std::fs prefix match)
        assert!(!outcome.findings().is_empty());
        assert!(outcome.findings().iter().any(|f| f.to_string().contains("std::fs::")));
    }

    #[test]
    fn test_detects_use_self_import() {
        let tmp = TempDir::new().unwrap();
        setup_usecase_file(
            tmp.path(),
            "workflow.rs",
            "use std::process::{self};\nfn f() { process::Command::new(\"ls\"); }\n",
        );
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
        assert!(!outcome.findings().is_empty());
        assert!(outcome.findings().iter().any(|f| f.to_string().contains("std::process::")));
    }

    #[test]
    fn test_detects_glob_import() {
        let tmp = TempDir::new().unwrap();
        setup_usecase_file(
            tmp.path(),
            "workflow.rs",
            "use std::process::*;\nfn f() { Command::new(\"ls\"); }\n",
        );
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors());
        assert!(!outcome.findings().is_empty());
        assert!(outcome.findings().iter().any(|f| f.to_string().contains("std::process::")));
    }

    #[test]
    fn test_ignores_cfg_test_method_in_impl() {
        let tmp = TempDir::new().unwrap();
        setup_usecase_file(
            tmp.path(),
            "workflow.rs",
            "struct Foo;\nimpl Foo {\n    \
             #[cfg(test)]\n    fn helper() { println!(\"test\"); }\n\
             pub fn clean(&self) {}\n}\n",
        );
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok());
        assert!(outcome.findings().is_empty());
    }
}
