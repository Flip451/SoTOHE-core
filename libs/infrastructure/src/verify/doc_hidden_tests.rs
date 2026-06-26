//! Tests for [`doc_hidden`] (split out to keep the main module under the line guideline).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]

use std::path::Path;

use tempfile::TempDir;

use super::*;

// ── Fixture helpers ──────────────────────────────────────────────────────────

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

// ── AC-01: clean source passes ───────────────────────────────────────────────

#[test]
fn test_clean_source_passes() {
    let tmp = TempDir::new().unwrap();
    setup(tmp.path(), "pub struct Foo;\nimpl Foo { pub fn bar() {} }\n");
    let outcome = verify(tmp.path());
    assert!(outcome.is_ok(), "unexpected findings: {:?}", outcome.findings());
    assert!(outcome.findings().is_empty());
}

// ── AC-04: all 5 forms detected on pub items ─────────────────────────────────

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

// ── AC-04: inner attribute forms ─────────────────────────────────────────────

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

// ── CN-02: visibility-agnostic detection ──────────────────────────────────────

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

// ── AC-08: impl block itself is flagged ──────────────────────────────────────

#[test]
fn test_detects_doc_hidden_on_impl_block() {
    let tmp = TempDir::new().unwrap();
    setup(tmp.path(), "struct Foo;\n#[doc(hidden)]\nimpl Foo { pub fn x() {} }\n");
    let outcome = verify(tmp.path());
    assert!(outcome.has_errors(), "expected error for #[doc(hidden)] on impl block");
}

// ── Fields and variants ───────────────────────────────────────────────────────

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
    setup(tmp.path(), "pub trait MyTrait {\n    #[doc(hidden)]\n    fn hidden_method(&self);\n}\n");
    let outcome = verify(tmp.path());
    assert!(outcome.has_errors(), "expected error for #[doc(hidden)] on trait associated item");
}

// ── AC-03: test-gated items excluded ─────────────────────────────────────────

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
    write_src(tmp.path(), "test_helpers.rs", "#![cfg(test)]\n#[doc(hidden)]\npub fn helper() {}\n");
    // Also write a clean lib.rs so the layer dir exists and is scanned.
    write_src(tmp.path(), "lib.rs", "pub fn ok() {}\n");
    let outcome = verify(tmp.path());
    assert!(outcome.is_ok(), "file with #![cfg(test)] must be skipped: {:?}", outcome.findings());
}

// ── Not-detected: name-value doc comment form ────────────────────────────────

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

// ── Error cases ───────────────────────────────────────────────────────────────

#[test]
fn test_missing_arch_rules_returns_error_finding() {
    let tmp = TempDir::new().unwrap();
    // No architecture-rules.json.
    let outcome = verify(tmp.path());
    assert!(outcome.has_errors(), "missing arch rules must produce an error finding");
}

// ── AC-06: multi-layer scanning ───────────────────────────────────────────────

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

// ── IN-03/AC-03: #[path] cfg(test) mod exclusion ────────────────────────────

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

// ── PR4-PR6: union field-level attribute scanning ────────────────────────────

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

// ── AC-07: scanner callback replaceability ────────────────────────────────────

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

// ── PR7-PR9: dual-reference (production + cfg-test) handling ─────────────────

/// PR7: `lib.rs` declares BOTH `#[cfg(test)] #[path = "shared.rs"] mod test_shared;`
/// AND a production `mod shared;`.  Because a non-`cfg(test)` module declaration
/// also references `shared.rs`, the file must be treated as production code and
/// any `#[doc(hidden)]` inside it must be flagged.
#[test]
fn test_flags_doc_hidden_when_file_has_both_cfg_test_path_attr_and_production_mod() {
    let tmp = TempDir::new().unwrap();
    write_arch_rules(tmp.path());
    write_src(
        tmp.path(),
        "lib.rs",
        concat!(
            "pub fn clean() {}\n",
            "#[cfg(test)]\n",
            "#[path = \"shared.rs\"]\n",
            "mod test_shared;\n",
            "mod shared;\n",
        ),
    );
    write_src(tmp.path(), "shared.rs", "#[doc(hidden)]\npub fn x() {}\n");

    let outcome = verify(tmp.path());
    assert!(
        outcome.has_errors(),
        "#[doc(hidden)] in file with both cfg_test #[path] and production mod must be flagged: \
         {:?}",
        outcome.findings()
    );
}

/// PR8: `lib.rs` declares only `#[cfg(test)] mod tests;` with no production
/// reference to `tests.rs`.  Regression: the fix must not break pure cfg-test
/// ident-based exclusion — the file must still be skipped.
#[test]
fn test_ignores_doc_hidden_when_only_cfg_test_ident_references_file() {
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
        "pure cfg-test ident mod with no production ref must still exclude the file \
         (regression): {:?}",
        outcome.findings()
    );
}

/// PR9: `lib.rs` declares only `#[cfg(test)] #[path = "shared.rs"] mod test_shared;`
/// with no production reference to `shared.rs`.  Regression: the fix must not
/// break pure cfg-test `#[path]`-based exclusion — the file must still be skipped.
#[test]
fn test_ignores_doc_hidden_when_only_cfg_test_path_attr_references_file() {
    let tmp = TempDir::new().unwrap();
    write_arch_rules(tmp.path());
    write_src(
        tmp.path(),
        "lib.rs",
        concat!(
            "pub fn clean() {}\n",
            "#[cfg(test)]\n",
            "#[path = \"shared.rs\"]\n",
            "mod test_shared;\n",
        ),
    );
    write_src(tmp.path(), "shared.rs", "#[doc(hidden)]\npub fn hidden_helper() {}\n");

    let outcome = verify(tmp.path());
    assert!(
        outcome.is_ok(),
        "pure cfg-test #[path] mod with no production ref must still exclude the file \
         (regression): {:?}",
        outcome.findings()
    );
}
