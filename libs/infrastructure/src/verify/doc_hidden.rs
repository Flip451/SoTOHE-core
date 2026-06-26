//! Verify gate: detect `#[doc(hidden)]` attribute declarations (all forms).
//!
//! Uses the shared `scan_workspace_rust_sources` scanner from `super::syn_scan`
//! so that file discovery, `syn::parse_file`, and `#[cfg(test)]` / `#[test]`
//! exclusion are handled centrally.

use std::path::Path;

use domain::verify::{VerifyFinding, VerifyOutcome};

use super::syn_scan::{SynScanContext, scan_workspace_rust_sources};

/// Verify that no `#[doc(hidden)]` attribute (in any of its recognized forms)
/// is present in the workspace source files covered by
/// `architecture-rules.json` `layers[]`.
///
/// # Detection forms (IN-02)
///
/// 1. Outer direct: `#[doc(hidden)]`
/// 2. Inner direct: `#![doc(hidden)]`
/// 3. Combined doc args (order-agnostic): `#[doc(hidden, alias = "x")]` /
///    `#[doc(alias = "x", hidden)]`
/// 4. `cfg_attr`-wrapped outer: `#[cfg_attr(pred, doc(hidden))]` /
///    `#[cfg_attr(pred, doc(hidden, ...))]`
/// 5. `cfg_attr`-wrapped inner: `#![cfg_attr(pred, doc(hidden))]`
///
/// `#[doc = "..."]` name-value doc comments are **never** flagged.
/// Test-scoped items (`#[cfg(test)]` / `#[test]`) are excluded (IN-03).
///
/// # Errors
///
/// Returns error-level findings for each attribute occurrence, including the
/// root-relative file path and 1-based source line (CN-05).
pub fn verify(root: &Path) -> VerifyOutcome {
    scan_workspace_rust_sources(root, detect_doc_hidden)
}

// ─────────────────────────────────────────────────────────────────────────────
// Detection callback
// ─────────────────────────────────────────────────────────────────────────────

/// Detection callback: inspects the attributes on a single AST node and
/// returns an error finding for each `#[doc(hidden)]` form found.
///
/// The `ctx.line` and `ctx.node_kind` fields provide context for the node;
/// the per-attribute source line is preferred when span information is available.
fn detect_doc_hidden(ctx: SynScanContext) -> Vec<VerifyFinding> {
    let mut findings = Vec::new();
    let rel = ctx.relative_path.display();

    for attr in &ctx.attrs {
        // Prefer the attribute's own source line; fall back to the node's line.
        let attr_line = attr.pound_token.spans[0].start().line;
        let line = if attr_line > 0 { attr_line } else { ctx.line };

        if is_doc_hidden_attr(attr) {
            findings.push(VerifyFinding::error(format!(
                "{rel}:{line} [{node_kind}]: `#[doc(hidden)]` attribute is forbidden \
                 (ADR 2026-06-26-0810-prohibit-doc-hidden-attribute)",
                node_kind = ctx.node_kind,
            )));
        } else if is_cfg_attr_with_doc_hidden(attr) {
            findings.push(VerifyFinding::error(format!(
                "{rel}:{line} [{node_kind}]: `#[cfg_attr(..., doc(hidden))]` attribute is \
                 forbidden (ADR 2026-06-26-0810-prohibit-doc-hidden-attribute)",
                node_kind = ctx.node_kind,
            )));
        }
    }

    findings
}

// ─────────────────────────────────────────────────────────────────────────────
// Attribute detection helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Returns `true` when `attr` is a direct `#[doc(hidden)]` (or inner
/// `#![doc(hidden)]`) attribute in any of the recognized forms including
/// combined doc args (order-agnostic).
///
/// Matches:
/// - `#[doc(hidden)]`
/// - `#![doc(hidden)]`
/// - `#[doc(hidden, alias = "x")]`
/// - `#[doc(alias = "x", hidden)]`
///
/// Explicitly does NOT match `#[doc = "..."]` (name-value doc comments).
fn is_doc_hidden_attr(attr: &syn::Attribute) -> bool {
    if !attr.path().is_ident("doc") {
        return false;
    }
    // Only list-style meta `#[doc(...)]` can contain `hidden`.
    // Name-value `#[doc = "..."]` is parsed as Meta::NameValue and is excluded.
    let syn::Meta::List(meta_list) = &attr.meta else {
        return false;
    };
    doc_list_tokens_contain_hidden(&meta_list.tokens)
}

/// Returns `true` when `attr` is a `#[cfg_attr(pred, doc(hidden))]` or
/// `#![cfg_attr(pred, doc(hidden))]` attribute (any predicate, order-agnostic
/// combined forms inside the doc list).
fn is_cfg_attr_with_doc_hidden(attr: &syn::Attribute) -> bool {
    if !attr.path().is_ident("cfg_attr") {
        return false;
    }
    let syn::Meta::List(meta_list) = &attr.meta else {
        return false;
    };
    cfg_attr_tokens_contain_doc_hidden(&meta_list.tokens)
}

// ─────────────────────────────────────────────────────────────────────────────
// Token-stream helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Returns `true` when the token stream of a `doc(...)` list contains a bare
/// `hidden` identifier.
///
/// Works order-agnostically: `hidden, alias = "x"` and `alias = "x", hidden`
/// both return `true`.
///
/// Only `Ident("hidden")` tokens trigger detection, so `alias = "hidden"`
/// (where `"hidden"` is a `Literal`) is correctly excluded.
fn doc_list_tokens_contain_hidden(tokens: &proc_macro2::TokenStream) -> bool {
    use proc_macro2::TokenTree;
    tokens
        .clone()
        .into_iter()
        .any(|tt| matches!(tt, TokenTree::Ident(ref ident) if ident == "hidden"))
}

/// Returns `true` when the token stream of a `cfg_attr(...)` list contains a
/// `doc(...)` nested attribute whose list contains `hidden`.
///
/// The token stream looks like `<pred> , doc ( <doc-args> )` at the top level,
/// where `<pred>` may itself contain commas inside groups (e.g.
/// `any(feature = "a", test)`).  Because `proc_macro2` represents grouped
/// sub-expressions as `TokenTree::Group`, the first `Punct(',')` in the flat
/// token stream is always the separator between the predicate and the
/// attributes — no depth tracking is necessary.
fn cfg_attr_tokens_contain_doc_hidden(tokens: &proc_macro2::TokenStream) -> bool {
    use proc_macro2::TokenTree;

    let mut iter = tokens.clone().into_iter();

    // Skip everything up to and including the first top-level comma.
    let mut found_comma = false;
    for tt in iter.by_ref() {
        if let TokenTree::Punct(ref p) = tt {
            if p.as_char() == ',' {
                found_comma = true;
                break;
            }
        }
    }
    if !found_comma {
        return false;
    }

    // In the remaining tokens, look for the pattern `doc <Group(...)>` where
    // the group's contents contain `hidden`.
    let remaining: Vec<TokenTree> = iter.collect();
    let mut i = 0;
    while i < remaining.len() {
        if let Some(TokenTree::Ident(ident)) = remaining.get(i) {
            if ident == "doc" {
                if let Some(TokenTree::Group(g)) = remaining.get(i + 1) {
                    if g.delimiter() == proc_macro2::Delimiter::Parenthesis
                        && doc_list_tokens_contain_hidden(&g.stream())
                    {
                        return true;
                    }
                }
            }
        }
        i += 1;
    }

    false
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use std::path::Path;

    use tempfile::TempDir;

    use super::*;

    // ── Fixture helpers ─────────────────────────────────────────────────────

    /// Write a minimal `architecture-rules.json` pointing at `test-layer/src`.
    fn write_arch_rules(root: &Path) {
        let rules = serde_json::json!({
            "layers": [
                { "crate": "test-layer", "path": "test-layer", "may_depend_on": [] }
            ]
        });
        std::fs::write(root.join("architecture-rules.json"), rules.to_string()).unwrap();
    }

    /// Write a Rust source file at `test-layer/src/<rel>`.
    fn write_src(root: &Path, rel: &str, content: &str) {
        let path = root.join("test-layer/src").join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, content).unwrap();
    }

    fn setup(root: &Path, content: &str) {
        write_arch_rules(root);
        write_src(root, "lib.rs", content);
    }

    // ── AC-01: clean source passes ──────────────────────────────────────────

    #[test]
    fn test_clean_source_passes() {
        let tmp = TempDir::new().unwrap();
        setup(tmp.path(), "pub struct Foo;\nimpl Foo { pub fn bar() {} }\n");
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok(), "unexpected findings: {:?}", outcome.findings());
        assert!(outcome.findings().is_empty());
    }

    // ── AC-04: all 5 forms detected on pub items ─────────────────────────────

    #[test]
    fn test_detects_outer_doc_hidden_on_pub_item() {
        let tmp = TempDir::new().unwrap();
        setup(tmp.path(), "#[doc(hidden)]\npub fn foo() {}\n");
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors(), "expected error for #[doc(hidden)]");
        assert!(!outcome.findings().is_empty());
        let msg = outcome.findings()[0].to_string();
        assert!(msg.contains("doc(hidden)"), "finding should mention doc(hidden): {msg}");
    }

    #[test]
    fn test_detects_outer_doc_hidden_combined_hidden_first() {
        let tmp = TempDir::new().unwrap();
        setup(tmp.path(), "#[doc(hidden, alias = \"x\")]\npub fn foo() {}\n");
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors(), "expected error for #[doc(hidden, alias)]");
    }

    #[test]
    fn test_detects_outer_doc_hidden_combined_hidden_second() {
        let tmp = TempDir::new().unwrap();
        setup(tmp.path(), "#[doc(alias = \"x\", hidden)]\npub fn foo() {}\n");
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors(), "expected error for #[doc(alias, hidden)]");
    }

    #[test]
    fn test_detects_cfg_attr_wrapped_outer() {
        let tmp = TempDir::new().unwrap();
        setup(tmp.path(), "#[cfg_attr(feature = \"doc-cfg\", doc(hidden))]\npub fn foo() {}\n");
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors(), "expected error for #[cfg_attr(..., doc(hidden))]");
    }

    #[test]
    fn test_detects_cfg_attr_wrapped_doc_hidden_with_extra_args() {
        let tmp = TempDir::new().unwrap();
        setup(tmp.path(), "#[cfg_attr(test, doc(hidden, alias = \"x\"))]\npub fn foo() {}\n");
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors(), "expected error for cfg_attr with doc(hidden, ...)");
    }

    // ── AC-04: inner attribute forms ─────────────────────────────────────────

    #[test]
    fn test_detects_inner_doc_hidden_at_file_level() {
        let tmp = TempDir::new().unwrap();
        // File-level inner attribute — must appear in file.attrs.
        setup(tmp.path(), "#![doc(hidden)]\npub fn foo() {}\n");
        let outcome = verify(tmp.path());
        assert!(
            outcome.has_errors(),
            "expected error for file-level #![doc(hidden)]: {:?}",
            outcome.findings()
        );
    }

    #[test]
    fn test_detects_inner_cfg_attr_doc_hidden_at_file_level() {
        let tmp = TempDir::new().unwrap();
        setup(tmp.path(), "#![cfg_attr(feature = \"doc-cfg\", doc(hidden))]\npub fn foo() {}\n");
        let outcome = verify(tmp.path());
        assert!(
            outcome.has_errors(),
            "expected error for file-level #![cfg_attr(..., doc(hidden))]: {:?}",
            outcome.findings()
        );
    }

    // ── CN-02: visibility-agnostic detection ──────────────────────────────────

    #[test]
    fn test_detects_doc_hidden_on_non_pub_item() {
        let tmp = TempDir::new().unwrap();
        setup(tmp.path(), "#[doc(hidden)]\nfn private_fn() {}\n");
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors(), "expected error for non-pub item with #[doc(hidden)]");
    }

    #[test]
    fn test_detects_doc_hidden_on_pub_crate_item() {
        let tmp = TempDir::new().unwrap();
        setup(tmp.path(), "#[doc(hidden)]\npub(crate) fn crate_fn() {}\n");
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors(), "expected error for pub(crate) item with #[doc(hidden)]");
    }

    // ── AC-08: impl block itself is flagged ──────────────────────────────────

    #[test]
    fn test_detects_doc_hidden_on_impl_block() {
        let tmp = TempDir::new().unwrap();
        setup(tmp.path(), "struct Foo;\n#[doc(hidden)]\nimpl Foo { pub fn x() {} }\n");
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors(), "expected error for #[doc(hidden)] on impl block");
    }

    // ── Fields and variants ──────────────────────────────────────────────────

    #[test]
    fn test_detects_doc_hidden_on_struct_field() {
        let tmp = TempDir::new().unwrap();
        setup(tmp.path(), "pub struct Foo {\n    #[doc(hidden)]\n    pub x: u32,\n}\n");
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors(), "expected error for #[doc(hidden)] on struct field");
    }

    #[test]
    fn test_detects_doc_hidden_on_enum_variant() {
        let tmp = TempDir::new().unwrap();
        setup(tmp.path(), "pub enum Bar {\n    #[doc(hidden)]\n    Hidden,\n}\n");
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors(), "expected error for #[doc(hidden)] on enum variant");
    }

    #[test]
    fn test_detects_doc_hidden_on_trait_associated_item() {
        let tmp = TempDir::new().unwrap();
        setup(
            tmp.path(),
            "pub trait MyTrait {\n    #[doc(hidden)]\n    fn hidden_method(&self);\n}\n",
        );
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors(), "expected error for #[doc(hidden)] on trait associated item");
    }

    // ── AC-03: test-gated items excluded ─────────────────────────────────────

    #[test]
    fn test_ignores_doc_hidden_inside_cfg_test_module() {
        let tmp = TempDir::new().unwrap();
        setup(
            tmp.path(),
            concat!(
                "pub fn clean() {}\n",
                "#[cfg(test)]\n",
                "mod tests {\n",
                "    #[doc(hidden)]\n",
                "    pub fn hidden_in_test() {}\n",
                "}\n"
            ),
        );
        let outcome = verify(tmp.path());
        assert!(
            outcome.is_ok(),
            "doc(hidden) inside #[cfg(test)] mod must not be flagged: {:?}",
            outcome.findings()
        );
    }

    #[test]
    fn test_ignores_doc_hidden_in_file_backed_cfg_test_module() {
        let tmp = TempDir::new().unwrap();
        write_arch_rules(tmp.path());
        write_src(
            tmp.path(),
            "lib.rs",
            concat!("pub fn clean() {}\n", "#[cfg(test)]\n", "mod helpers;\n"),
        );
        write_src(tmp.path(), "helpers.rs", "#[doc(hidden)]\npub fn hidden_helper() {}\n");

        let outcome = verify(tmp.path());
        assert!(
            outcome.is_ok(),
            "doc(hidden) inside file-backed #[cfg(test)] mod must not be flagged: {:?}",
            outcome.findings()
        );
    }

    #[test]
    fn test_ignores_doc_hidden_on_test_fn() {
        let tmp = TempDir::new().unwrap();
        setup(
            tmp.path(),
            concat!("pub fn clean() {}\n", "#[test]\n", "#[doc(hidden)]\n", "fn my_test() {}\n"),
        );
        let outcome = verify(tmp.path());
        assert!(
            outcome.is_ok(),
            "doc(hidden) on #[test] fn must not be flagged: {:?}",
            outcome.findings()
        );
    }

    #[test]
    fn test_ignores_file_with_cfg_test_inner_attr() {
        let tmp = TempDir::new().unwrap();
        write_arch_rules(tmp.path());
        // File-level #![cfg(test)] means the entire file is test-only.
        write_src(
            tmp.path(),
            "test_helpers.rs",
            "#![cfg(test)]\n#[doc(hidden)]\npub fn helper() {}\n",
        );
        // Also write a clean lib.rs so the layer dir exists and is scanned.
        write_src(tmp.path(), "lib.rs", "pub fn ok() {}\n");
        let outcome = verify(tmp.path());
        assert!(
            outcome.is_ok(),
            "file with #![cfg(test)] must be skipped: {:?}",
            outcome.findings()
        );
    }

    // ── Not-detected: name-value doc comment form ────────────────────────────

    #[test]
    fn test_does_not_flag_doc_name_value_comment() {
        let tmp = TempDir::new().unwrap();
        // #[doc = "hidden"] is a doc comment, NOT doc(hidden).
        setup(tmp.path(), "#[doc = \"This item is hidden from users.\"]\npub fn foo() {}\n");
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok(), "#[doc = \"...\"] must not be flagged: {:?}", outcome.findings());
    }

    #[test]
    fn test_does_not_flag_plain_doc_comment() {
        let tmp = TempDir::new().unwrap();
        // Doc line comment `/// hidden` is also name-value form in the AST.
        setup(tmp.path(), "/// This is hidden from view.\npub fn foo() {}\n");
        let outcome = verify(tmp.path());
        assert!(outcome.is_ok(), "doc line comments must not be flagged: {:?}", outcome.findings());
    }

    // ── Error cases ──────────────────────────────────────────────────────────

    #[test]
    fn test_missing_arch_rules_returns_error_finding() {
        let tmp = TempDir::new().unwrap();
        // No architecture-rules.json.
        let outcome = verify(tmp.path());
        assert!(outcome.has_errors(), "missing arch rules must produce an error finding");
    }

    // ── AC-06: multi-layer scanning ──────────────────────────────────────────

    #[test]
    fn test_scans_multiple_layers() {
        let tmp = TempDir::new().unwrap();
        // Two layers in architecture-rules.json.
        let rules = serde_json::json!({
            "layers": [
                { "crate": "layer-a", "path": "layer-a", "may_depend_on": [] },
                { "crate": "layer-b", "path": "layer-b", "may_depend_on": [] }
            ]
        });
        std::fs::write(tmp.path().join("architecture-rules.json"), rules.to_string()).unwrap();
        // layer-a is clean.
        let dir_a = tmp.path().join("layer-a/src");
        std::fs::create_dir_all(&dir_a).unwrap();
        std::fs::write(dir_a.join("lib.rs"), "pub fn ok() {}\n").unwrap();
        // layer-b has a violation.
        let dir_b = tmp.path().join("layer-b/src");
        std::fs::create_dir_all(&dir_b).unwrap();
        std::fs::write(dir_b.join("lib.rs"), "#[doc(hidden)]\npub fn bad() {}\n").unwrap();

        let outcome = verify(tmp.path());
        assert!(outcome.has_errors(), "violation in layer-b must be detected");
    }

    // ── IN-03/AC-03: #[path] cfg(test) mod exclusion ────────────────────────

    /// PR1: `#[cfg(test)] #[path = "shared_helpers.rs"] mod test_helpers;` —
    /// the module ident (`test_helpers`) differs from the file stem
    /// (`shared_helpers`); the file must still be excluded from scanning.
    #[test]
    fn test_ignores_doc_hidden_in_cfg_test_mod_with_path_attr_same_dir() {
        let tmp = TempDir::new().unwrap();
        write_arch_rules(tmp.path());
        write_src(
            tmp.path(),
            "lib.rs",
            concat!(
                "pub fn clean() {}\n",
                "#[cfg(test)]\n",
                "#[path = \"shared_helpers.rs\"]\n",
                "mod test_helpers;\n",
            ),
        );
        write_src(tmp.path(), "shared_helpers.rs", "#[doc(hidden)]\npub fn hidden_helper() {}\n");

        let outcome = verify(tmp.path());
        assert!(
            outcome.is_ok(),
            "#[path = \"shared_helpers.rs\"] cfg(test) mod must exclude the pointed file: {:?}",
            outcome.findings()
        );
    }

    /// PR2: `#[cfg(test)] #[path = "subdir/helpers.rs"] mod test_helpers;` —
    /// multi-component path attribute; file in a subdirectory must be excluded.
    #[test]
    fn test_ignores_doc_hidden_in_cfg_test_mod_with_path_attr_subdir() {
        let tmp = TempDir::new().unwrap();
        write_arch_rules(tmp.path());
        write_src(
            tmp.path(),
            "lib.rs",
            concat!(
                "pub fn clean() {}\n",
                "#[cfg(test)]\n",
                "#[path = \"subdir/helpers.rs\"]\n",
                "mod test_helpers;\n",
            ),
        );
        write_src(tmp.path(), "subdir/helpers.rs", "#[doc(hidden)]\npub fn hidden_helper() {}\n");

        let outcome = verify(tmp.path());
        assert!(
            outcome.is_ok(),
            "#[path = \"subdir/helpers.rs\"] cfg(test) mod must exclude the pointed file: {:?}",
            outcome.findings()
        );
    }

    /// PR2b: `#[cfg(test)] #[path = "fixtures/mod.rs"] mod fixtures;` —
    /// `mod.rs` is the canonical file for the `fixtures` module and must
    /// match the containing directory component, not the literal `mod` stem.
    #[test]
    fn test_ignores_doc_hidden_in_cfg_test_mod_with_path_attr_mod_rs() {
        let tmp = TempDir::new().unwrap();
        write_arch_rules(tmp.path());
        write_src(
            tmp.path(),
            "lib.rs",
            concat!(
                "pub fn clean() {}\n",
                "#[cfg(test)]\n",
                "#[path = \"fixtures/mod.rs\"]\n",
                "mod fixtures;\n",
            ),
        );
        write_src(tmp.path(), "fixtures/mod.rs", "#[doc(hidden)]\npub fn hidden_helper() {}\n");

        let outcome = verify(tmp.path());
        assert!(
            outcome.is_ok(),
            "#[path = \"fixtures/mod.rs\"] cfg(test) mod must exclude the pointed file: {:?}",
            outcome.findings()
        );
    }

    #[test]
    fn test_path_attr_mismatch_does_not_exclude_ident_named_file() {
        let tmp = TempDir::new().unwrap();
        write_arch_rules(tmp.path());
        write_src(
            tmp.path(),
            "lib.rs",
            concat!(
                "pub fn clean() {}\n",
                "#[cfg(test)]\n",
                "#[path = \"other.rs\"]\n",
                "mod shared_helpers;\n",
            ),
        );
        write_src(tmp.path(), "other.rs", "pub fn clean_helper() {}\n");
        write_src(tmp.path(), "shared_helpers.rs", "#[doc(hidden)]\npub fn hidden_helper() {}\n");

        let outcome = verify(tmp.path());
        assert!(
            outcome.has_errors(),
            "mismatched #[path] must not fall back to ident-based exclusion: {:?}",
            outcome.findings()
        );
    }

    #[test]
    fn test_path_attr_parent_dir_component_does_not_alias_scanned_file() {
        let tmp = TempDir::new().unwrap();
        write_arch_rules(tmp.path());
        write_src(
            tmp.path(),
            "lib.rs",
            concat!(
                "pub fn clean() {}\n",
                "#[cfg(test)]\n",
                "#[path = \"../fixtures/shared_helpers.rs\"]\n",
                "mod shared_helpers;\n",
            ),
        );
        write_src(tmp.path(), "fixtures/shared_helpers.rs", "#[doc(hidden)]\npub fn hidden() {}\n");

        let outcome = verify(tmp.path());
        assert!(
            outcome.has_errors(),
            "path attrs with parent components must not alias in-src scanned files: {:?}",
            outcome.findings()
        );
    }

    /// PR3: `#[cfg(test)] mod tests;` (no `#[path]`) — regression guard: the
    /// existing ident-based resolution must continue to work unchanged.
    #[test]
    fn test_file_backed_cfg_test_mod_without_path_attr_regression() {
        let tmp = TempDir::new().unwrap();
        write_arch_rules(tmp.path());
        write_src(
            tmp.path(),
            "lib.rs",
            concat!("pub fn clean() {}\n", "#[cfg(test)]\n", "mod tests;\n"),
        );
        write_src(tmp.path(), "tests.rs", "#[doc(hidden)]\npub fn hidden_in_test() {}\n");

        let outcome = verify(tmp.path());
        assert!(
            outcome.is_ok(),
            "#[cfg(test)] mod without #[path] must still exclude the target file: {:?}",
            outcome.findings()
        );
    }

    // ── PR4-PR6: union field-level attribute scanning ────────────────────────

    /// PR4: field-level `#[doc(hidden)]` on a union field must be flagged.
    #[test]
    fn test_detects_doc_hidden_on_union_field() {
        let tmp = TempDir::new().unwrap();
        setup(tmp.path(), "pub union U { pub x: u32, #[doc(hidden)] pub y: u32 }\n");
        let outcome = verify(tmp.path());
        assert!(
            outcome.has_errors(),
            "expected error for #[doc(hidden)] on union field: {:?}",
            outcome.findings()
        );
    }

    /// PR5: `#[doc(hidden)]` on the union item itself must be flagged (regression).
    #[test]
    fn test_detects_doc_hidden_on_union_item() {
        let tmp = TempDir::new().unwrap();
        setup(tmp.path(), "#[doc(hidden)]\npub union U { pub x: u32 }\n");
        let outcome = verify(tmp.path());
        assert!(
            outcome.has_errors(),
            "expected error for #[doc(hidden)] on union item: {:?}",
            outcome.findings()
        );
    }

    /// PR6: `#[cfg(test)]` union must be excluded — field-level `#[doc(hidden)]`
    /// inside it must not be flagged.
    #[test]
    fn test_ignores_doc_hidden_on_union_field_inside_cfg_test() {
        let tmp = TempDir::new().unwrap();
        setup(tmp.path(), "#[cfg(test)]\nunion U { #[doc(hidden)] x: u32 }\n");
        let outcome = verify(tmp.path());
        assert!(
            outcome.is_ok(),
            "#[doc(hidden)] inside #[cfg(test)] union must not be flagged: {:?}",
            outcome.findings()
        );
    }

    // ── AC-07: scanner callback replaceability ────────────────────────────────

    #[test]
    fn test_scanner_callback_is_replaceable() {
        use super::super::syn_scan::scan_workspace_rust_sources;

        let tmp = TempDir::new().unwrap();
        write_arch_rules(tmp.path());
        // A source file with a `#[deprecated]` attribute.
        write_src(tmp.path(), "lib.rs", "#[deprecated]\npub fn old() {}\n");

        // Use a different callback that detects `#[deprecated]` instead.
        let outcome = scan_workspace_rust_sources(tmp.path(), |ctx| {
            let mut findings = Vec::new();
            for attr in &ctx.attrs {
                if attr.path().is_ident("deprecated") {
                    findings.push(domain::verify::VerifyFinding::error(format!(
                        "{}: #[deprecated] detected",
                        ctx.relative_path.display()
                    )));
                }
            }
            findings
        });
        assert!(
            outcome.has_errors(),
            "custom callback must detect #[deprecated]: {:?}",
            outcome.findings()
        );
    }
}
