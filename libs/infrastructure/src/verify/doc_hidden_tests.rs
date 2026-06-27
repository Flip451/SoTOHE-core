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

// ── PR10-PR12: sibling file-backed cfg(test) module exclusion ────────────────

/// PR10: `foo.rs` in the same directory declares
/// `#[cfg(test)] #[path = "foo_tests.rs"] mod tests;`.
/// `foo_tests.rs` contains a `#[doc(hidden)]` fixture — the file is not a
/// canonical module entry point (`mod.rs` / `lib.rs` / `main.rs`), yet the
/// sibling reference must cause it to be excluded from scanning.
#[test]
fn test_ignores_doc_hidden_in_sibling_file_backed_cfg_test_mod() {
    let tmp = TempDir::new().unwrap();
    write_arch_rules(tmp.path());
    // lib.rs is clean and does not reference foo or foo_tests.
    write_src(tmp.path(), "lib.rs", "pub fn clean() {}\n");
    // foo.rs declares foo_tests.rs as its test module via a sibling #[path].
    write_src(
        tmp.path(),
        "foo.rs",
        concat!(
            "pub fn foo_fn() {}\n",
            "#[cfg(test)]\n",
            "#[path = \"foo_tests.rs\"]\n",
            "mod tests;\n",
        ),
    );
    // foo_tests.rs has a #[doc(hidden)] fixture: must not be flagged.
    write_src(tmp.path(), "foo_tests.rs", "#[doc(hidden)]\npub fn x() {}\n");

    let outcome = verify(tmp.path());
    assert!(
        outcome.is_ok(),
        "doc(hidden) in sibling #[cfg(test)] #[path] test module must not be flagged: {:?}",
        outcome.findings()
    );
}

/// PR11: both `foo.rs` and `bar.rs` declare
/// `#[cfg(test)] #[path = "shared_tests.rs"] mod tests;`.
/// All references to `shared_tests.rs` are `cfg(test)`, so any `#[doc(hidden)]`
/// fixture inside it must still be excluded.
#[test]
fn test_ignores_doc_hidden_in_shared_test_file_referenced_by_multiple_cfg_test_siblings() {
    let tmp = TempDir::new().unwrap();
    write_arch_rules(tmp.path());
    write_src(tmp.path(), "lib.rs", "pub fn clean() {}\n");
    write_src(
        tmp.path(),
        "foo.rs",
        concat!(
            "pub fn foo_fn() {}\n",
            "#[cfg(test)]\n",
            "#[path = \"shared_tests.rs\"]\n",
            "mod tests;\n",
        ),
    );
    write_src(
        tmp.path(),
        "bar.rs",
        concat!(
            "pub fn bar_fn() {}\n",
            "#[cfg(test)]\n",
            "#[path = \"shared_tests.rs\"]\n",
            "mod tests;\n",
        ),
    );
    // shared_tests.rs has a #[doc(hidden)] fixture: must not be flagged.
    write_src(tmp.path(), "shared_tests.rs", "#[doc(hidden)]\npub fn hidden() {}\n");

    let outcome = verify(tmp.path());
    assert!(
        outcome.is_ok(),
        "doc(hidden) in shared test file referenced only by cfg(test) siblings must not be \
         flagged: {:?}",
        outcome.findings()
    );
}

/// PR12: regression — a file that is NOT referenced as a `cfg(test)` module by
/// any sibling must still be flagged.  The sibling probe must not accidentally
/// exclude production files simply because sibling files exist.
#[test]
fn test_flags_doc_hidden_in_file_not_referenced_as_cfg_test_by_any_sibling() {
    let tmp = TempDir::new().unwrap();
    write_arch_rules(tmp.path());
    write_src(tmp.path(), "lib.rs", "pub fn clean() {}\n");
    // sibling.rs exists but does NOT reference production.rs as a test module.
    write_src(tmp.path(), "sibling.rs", "pub fn sibling_fn() {}\n");
    // production.rs has #[doc(hidden)] and no cfg(test) reference: must be flagged.
    write_src(tmp.path(), "production.rs", "#[doc(hidden)]\npub fn bad() {}\n");

    let outcome = verify(tmp.path());
    assert!(
        outcome.has_errors(),
        "doc(hidden) in file not referenced as cfg(test) by any sibling must still be flagged: \
         {:?}",
        outcome.findings()
    );
}

// ── PR13-PR17: transitive cfg(test) context propagation ──────────────────────

/// PR13: `lib.rs` has `#[cfg(test)] mod tests;`, `tests.rs` has a plain
/// `mod helpers;`, and `tests/helpers.rs` has `#[doc(hidden)]`.
/// Because `tests.rs` is itself only reachable under `#[cfg(test)]`, the plain
/// `mod helpers;` inside it is transitively test-only, so `tests/helpers.rs`
/// must not be flagged.
#[test]
fn test_ignores_doc_hidden_in_nested_file_backed_test_module() {
    let tmp = TempDir::new().unwrap();
    write_arch_rules(tmp.path());
    write_src(
        tmp.path(),
        "lib.rs",
        concat!("pub fn clean() {}\n", "#[cfg(test)]\n", "mod tests;\n"),
    );
    write_src(tmp.path(), "tests.rs", "mod helpers;\n");
    write_src(tmp.path(), "tests/helpers.rs", "#[doc(hidden)]\npub fn x() {}\n");

    let outcome = verify(tmp.path());
    assert!(
        outcome.is_ok(),
        "doc(hidden) in transitively test-only file must not be flagged (PR13): {:?}",
        outcome.findings()
    );
}

/// PR14: regression — when `lib.rs` has a production `mod tests;` (no
/// `#[cfg(test)]`), `tests.rs` is production, so `tests/helpers.rs` must
/// still be flagged even though `tests.rs` declares `mod helpers;` plainly.
#[test]
fn test_flags_doc_hidden_when_parent_is_production_module() {
    let tmp = TempDir::new().unwrap();
    write_arch_rules(tmp.path());
    write_src(
        tmp.path(),
        "lib.rs",
        // Plain mod tests; — no #[cfg(test)].
        concat!("pub fn clean() {}\n", "mod tests;\n"),
    );
    write_src(tmp.path(), "tests.rs", "mod helpers;\n");
    write_src(tmp.path(), "tests/helpers.rs", "#[doc(hidden)]\npub fn x() {}\n");

    let outcome = verify(tmp.path());
    assert!(
        outcome.has_errors(),
        "doc(hidden) reachable via production mod chain must be flagged (PR14): {:?}",
        outcome.findings()
    );
}

/// PR15: multi-level transitive inheritance — `lib.rs` has
/// `#[cfg(test)] mod a;`, `a.rs` has plain `mod b;`, `a/b.rs` has plain
/// `mod c;`, and `a/b/c.rs` has `#[doc(hidden)]`.
/// All three descendant files are transitively test-only and must not be flagged.
#[test]
fn test_ignores_doc_hidden_in_deeply_nested_transitive_cfg_test_module() {
    let tmp = TempDir::new().unwrap();
    write_arch_rules(tmp.path());
    write_src(tmp.path(), "lib.rs", concat!("pub fn clean() {}\n", "#[cfg(test)]\n", "mod a;\n"));
    write_src(tmp.path(), "a.rs", "mod b;\n");
    write_src(tmp.path(), "a/b.rs", "mod c;\n");
    write_src(tmp.path(), "a/b/c.rs", "#[doc(hidden)]\npub fn hidden() {}\n");

    let outcome = verify(tmp.path());
    assert!(
        outcome.is_ok(),
        "doc(hidden) in deeply nested transitively test-only file must not be flagged (PR15): \
         {:?}",
        outcome.findings()
    );
}

/// PR16: same-directory sibling correctness — `lib.rs` has `#[cfg(test)] mod a;`,
/// `a.rs` has plain `mod b;` (no `#[path]`), and `b.rs` has `#[doc(hidden)]`.
/// In Rust, `mod b;` in `src/a.rs` resolves to `src/a/b.rs` (subdirectory), NOT
/// to sibling `src/b.rs`.  `src/b.rs` is an orphan file and must be flagged.
#[test]
fn test_ignores_doc_hidden_in_same_directory_transitive_cfg_test_module() {
    let tmp = TempDir::new().unwrap();
    write_arch_rules(tmp.path());
    write_src(tmp.path(), "lib.rs", concat!("pub fn clean() {}\n", "#[cfg(test)]\n", "mod a;\n"));
    write_src(tmp.path(), "a.rs", "mod b;\n");
    write_src(tmp.path(), "b.rs", "#[doc(hidden)]\npub fn hidden() {}\n");

    let outcome = verify(tmp.path());
    // `mod b;` in a.rs resolves to src/a/b.rs, not to sibling src/b.rs.
    // b.rs is an orphan and must be flagged (PR16 — corrected after sibling-probe fix).
    assert!(
        outcome.has_errors(),
        "doc(hidden) in sibling b.rs (orphaned — a.rs's mod b; resolves to a/b.rs) must be \
         flagged (PR16): {:?}",
        outcome.findings()
    );
}

/// PR17: same-directory production regression — without `#[cfg(test)]` on the
/// root module, `a.rs` and `b.rs` are production files and the violation must
/// still be reported.
#[test]
fn test_flags_doc_hidden_in_same_directory_production_module_chain() {
    let tmp = TempDir::new().unwrap();
    write_arch_rules(tmp.path());
    write_src(tmp.path(), "lib.rs", concat!("pub fn clean() {}\n", "mod a;\n"));
    write_src(tmp.path(), "a.rs", "mod b;\n");
    write_src(tmp.path(), "b.rs", "#[doc(hidden)]\npub fn hidden() {}\n");

    let outcome = verify(tmp.path());
    assert!(
        outcome.has_errors(),
        "doc(hidden) in same-directory production module chain must be flagged (PR17): {:?}",
        outcome.findings()
    );
}

/// PR18: a production `mod tests;` can still point at a module file whose own
/// file-level `#![cfg(test)]` makes every child module declaration test-only.
#[test]
fn test_ignores_doc_hidden_when_parent_module_file_has_inner_cfg_test() {
    let tmp = TempDir::new().unwrap();
    write_arch_rules(tmp.path());
    write_src(tmp.path(), "lib.rs", concat!("pub fn clean() {}\n", "mod tests;\n"));
    write_src(tmp.path(), "tests.rs", "#![cfg(test)]\nmod helpers;\n");
    write_src(tmp.path(), "tests/helpers.rs", "#[doc(hidden)]\npub fn hidden() {}\n");

    let outcome = verify(tmp.path());
    assert!(
        outcome.is_ok(),
        "doc(hidden) under a file-level cfg(test) parent module must not be flagged (PR18): {:?}",
        outcome.findings()
    );
}

// ── LB01-LB05: local items in function / method bodies ──────────────────────

/// LB01: `fn f() { #[doc(hidden)] struct Local; }` — `#[doc(hidden)]` on a
/// local struct declaration inside a function body must be flagged.
#[test]
fn test_detects_doc_hidden_on_local_struct_in_fn_body() {
    let tmp = TempDir::new().unwrap();
    setup(tmp.path(), concat!("fn f() {\n", "    #[doc(hidden)]\n", "    struct Local;\n", "}\n"));
    let outcome = verify(tmp.path());
    assert!(
        outcome.has_errors(),
        "expected error for #[doc(hidden)] on local struct in fn body (LB01): {:?}",
        outcome.findings()
    );
}

/// LB02: `fn f() { #[doc(hidden)] fn inner() {} }` — `#[doc(hidden)]` on a
/// local function declaration inside a function body must be flagged.
#[test]
fn test_detects_doc_hidden_on_local_fn_in_fn_body() {
    let tmp = TempDir::new().unwrap();
    setup(tmp.path(), concat!("fn f() {\n", "    #[doc(hidden)]\n", "    fn inner() {}\n", "}\n"));
    let outcome = verify(tmp.path());
    assert!(
        outcome.has_errors(),
        "expected error for #[doc(hidden)] on local fn in fn body (LB02): {:?}",
        outcome.findings()
    );
}

/// LB03: `#[cfg(test)] fn t() { #[doc(hidden)] struct X; }` — when the
/// enclosing function is `#[cfg(test)]`, local items inside its body must
/// not be flagged (the outer test-gate excludes the entire body).
#[test]
fn test_ignores_doc_hidden_in_local_item_inside_cfg_test_fn() {
    let tmp = TempDir::new().unwrap();
    setup(
        tmp.path(),
        concat!("#[cfg(test)]\n", "fn t() {\n", "    #[doc(hidden)]\n", "    struct X;\n", "}\n"),
    );
    let outcome = verify(tmp.path());
    assert!(
        outcome.is_ok(),
        "#[doc(hidden)] local item inside #[cfg(test)] fn must not be flagged (LB03): {:?}",
        outcome.findings()
    );
}

/// LB04: `fn f() { #[cfg(test)] #[doc(hidden)] struct X; }` — when the local
/// item itself carries `#[cfg(test)]`, it must not be flagged even though the
/// enclosing function is production code.
#[test]
fn test_ignores_doc_hidden_on_local_item_with_cfg_test_attr_in_fn_body() {
    let tmp = TempDir::new().unwrap();
    setup(
        tmp.path(),
        concat!(
            "fn f() {\n",
            "    #[cfg(test)]\n",
            "    #[doc(hidden)]\n",
            "    struct X;\n",
            "}\n"
        ),
    );
    let outcome = verify(tmp.path());
    assert!(
        outcome.is_ok(),
        "#[doc(hidden)] local item with its own #[cfg(test)] must not be flagged (LB04): {:?}",
        outcome.findings()
    );
}

/// LB05: `impl S { fn m(&self) { #[doc(hidden)] enum E {} } }` — `#[doc(hidden)]`
/// on a local item inside an impl method body must be flagged.
#[test]
fn test_detects_doc_hidden_on_local_enum_in_impl_method_body() {
    let tmp = TempDir::new().unwrap();
    setup(
        tmp.path(),
        concat!(
            "struct S;\n",
            "impl S {\n",
            "    fn m(&self) {\n",
            "        #[doc(hidden)]\n",
            "        enum E {}\n",
            "    }\n",
            "}\n"
        ),
    );
    let outcome = verify(tmp.path());
    assert!(
        outcome.has_errors(),
        "expected error for #[doc(hidden)] on local enum in impl method body (LB05): {:?}",
        outcome.findings()
    );
}

/// LB06: `fn f() { if cond { #[doc(hidden)] struct X; } }` — local items
/// nested under expression/control-flow blocks must be flagged.
#[test]
fn test_detects_doc_hidden_on_local_struct_in_nested_if_block() {
    let tmp = TempDir::new().unwrap();
    setup(
        tmp.path(),
        concat!(
            "fn f() {\n",
            "    if true {\n",
            "        #[doc(hidden)]\n",
            "        struct X;\n",
            "    }\n",
            "}\n"
        ),
    );
    let outcome = verify(tmp.path());
    assert!(
        outcome.has_errors(),
        "expected error for #[doc(hidden)] on local struct in nested if block (LB06): {:?}",
        outcome.findings()
    );
}

/// LB07: local items nested in expression/control-flow blocks keep their own
/// `#[cfg(test)]` pruning.
#[test]
fn test_ignores_doc_hidden_on_cfg_test_local_item_inside_nested_if_block() {
    let tmp = TempDir::new().unwrap();
    setup(
        tmp.path(),
        concat!(
            "fn f() {\n",
            "    if true {\n",
            "        #[cfg(test)]\n",
            "        #[doc(hidden)]\n",
            "        struct X;\n",
            "    }\n",
            "}\n"
        ),
    );
    let outcome = verify(tmp.path());
    assert!(
        outcome.is_ok(),
        "#[doc(hidden)] cfg(test) local item inside nested if block must not be flagged (LB07): {:?}",
        outcome.findings()
    );
}

// ── PR21-PR23: leading ./ in #[path] attributes ──────────────────────────────

/// PR21: `#[cfg(test)] #[path = "./helpers.rs"] mod test_helpers;` — a leading
/// `./` in the path attribute must not prevent the target file from being
/// classified as test-only.
#[test]
fn test_ignores_doc_hidden_in_cfg_test_mod_with_cur_dir_prefix_path_attr() {
    let tmp = TempDir::new().unwrap();
    write_arch_rules(tmp.path());
    write_src(
        tmp.path(),
        "lib.rs",
        concat!(
            "pub fn clean() {}\n",
            "#[cfg(test)]\n",
            "#[path = \"./helpers.rs\"]\n",
            "mod test_helpers;\n",
        ),
    );
    write_src(tmp.path(), "helpers.rs", "#[doc(hidden)]\npub fn hidden_helper() {}\n");

    let outcome = verify(tmp.path());
    assert!(
        outcome.is_ok(),
        "#[path = \"./helpers.rs\"] cfg(test) mod must exclude the pointed file (PR21): {:?}",
        outcome.findings()
    );
}

/// PR22: `#[cfg(test)] #[path = "./subdir/helpers.rs"] mod test_helpers;` — a
/// leading `./` combined with a subdirectory path must also be handled correctly.
#[test]
fn test_ignores_doc_hidden_in_cfg_test_mod_with_cur_dir_prefix_subdir_path_attr() {
    let tmp = TempDir::new().unwrap();
    write_arch_rules(tmp.path());
    write_src(
        tmp.path(),
        "lib.rs",
        concat!(
            "pub fn clean() {}\n",
            "#[cfg(test)]\n",
            "#[path = \"./subdir/helpers.rs\"]\n",
            "mod test_helpers;\n",
        ),
    );
    write_src(tmp.path(), "subdir/helpers.rs", "#[doc(hidden)]\npub fn hidden_helper() {}\n");

    let outcome = verify(tmp.path());
    assert!(
        outcome.is_ok(),
        "#[path = \"./subdir/helpers.rs\"] cfg(test) mod must exclude the pointed file (PR22): \
         {:?}",
        outcome.findings()
    );
}

/// PR23: regression — `#[cfg(test)] #[path = "helpers.rs"] mod test_helpers;`
/// (no leading `./`) must continue to work exactly as before the PR21/PR22 fix.
#[test]
fn test_ignores_doc_hidden_in_cfg_test_mod_with_bare_path_attr_regression() {
    let tmp = TempDir::new().unwrap();
    write_arch_rules(tmp.path());
    write_src(
        tmp.path(),
        "lib.rs",
        concat!(
            "pub fn clean() {}\n",
            "#[cfg(test)]\n",
            "#[path = \"helpers.rs\"]\n",
            "mod test_helpers;\n",
        ),
    );
    write_src(tmp.path(), "helpers.rs", "#[doc(hidden)]\npub fn hidden_helper() {}\n");

    let outcome = verify(tmp.path());
    assert!(
        outcome.is_ok(),
        "#[path = \"helpers.rs\"] (no ./) cfg(test) mod must still exclude the file (PR23 \
         regression): {:?}",
        outcome.findings()
    );
}

// ── PR24-PR26: non-root module with subdirectory #[path] reference ───────────

/// PR24: `foo.rs` (not lib.rs) in `src/` declares
/// `#[cfg(test)] #[path = "fixtures/helpers.rs"] mod tests;`.
/// `src/fixtures/helpers.rs` contains `#[doc(hidden)]`.
/// The probe must walk ancestor directories of `src/fixtures/helpers.rs` and
/// find the sibling `foo.rs` — the file must be excluded from scanning.
#[test]
fn test_ignores_doc_hidden_in_cfg_test_mod_declared_in_non_root_sibling_with_subdir_path() {
    let tmp = TempDir::new().unwrap();
    write_arch_rules(tmp.path());
    write_src(tmp.path(), "lib.rs", "pub fn clean() {}\n");
    write_src(
        tmp.path(),
        "foo.rs",
        concat!(
            "pub fn foo_fn() {}\n",
            "#[cfg(test)]\n",
            "#[path = \"fixtures/helpers.rs\"]\n",
            "mod tests;\n",
        ),
    );
    write_src(tmp.path(), "fixtures/helpers.rs", "#[doc(hidden)]\npub fn hidden_helper() {}\n");

    let outcome = verify(tmp.path());
    assert!(
        outcome.is_ok(),
        "doc(hidden) in cfg(test) file declared via subdirectory #[path] from a non-root \
         sibling must not be flagged (PR24): {:?}",
        outcome.findings()
    );
}

/// PR25: multi-level subdirectory — `foo.rs` in `src/` declares
/// `#[cfg(test)] #[path = "fixtures/inner/helpers.rs"] mod tests;`.
/// `src/fixtures/inner/helpers.rs` contains `#[doc(hidden)]`.
/// The ancestor probe must recurse past two subdirectory levels and still find
/// `foo.rs` as the cfg(test) parent — the file must be excluded.
#[test]
fn test_ignores_doc_hidden_in_cfg_test_mod_declared_in_non_root_sibling_with_multi_level_subdir_path()
 {
    let tmp = TempDir::new().unwrap();
    write_arch_rules(tmp.path());
    write_src(tmp.path(), "lib.rs", "pub fn clean() {}\n");
    write_src(
        tmp.path(),
        "foo.rs",
        concat!(
            "pub fn foo_fn() {}\n",
            "#[cfg(test)]\n",
            "#[path = \"fixtures/inner/helpers.rs\"]\n",
            "mod tests;\n",
        ),
    );
    write_src(
        tmp.path(),
        "fixtures/inner/helpers.rs",
        "#[doc(hidden)]\npub fn hidden_helper() {}\n",
    );

    let outcome = verify(tmp.path());
    assert!(
        outcome.is_ok(),
        "doc(hidden) in cfg(test) file declared via multi-level subdirectory #[path] from a \
         non-root sibling must not be flagged (PR25): {:?}",
        outcome.findings()
    );
}

/// PR26: regression — the ancestor sibling probe must not cause false negatives.
/// `foo.rs` exists in `src/` but does NOT declare any cfg(test) reference to
/// `src/fixtures/helpers.rs`.  The file still has `#[doc(hidden)]` and must be
/// flagged, because no cfg(test) module declaration covers it.
#[test]
fn test_flags_doc_hidden_when_ancestor_sibling_does_not_reference_file_as_cfg_test() {
    let tmp = TempDir::new().unwrap();
    write_arch_rules(tmp.path());
    write_src(tmp.path(), "lib.rs", "pub fn clean() {}\n");
    // foo.rs exists in src/ but does not reference fixtures/helpers.rs.
    write_src(tmp.path(), "foo.rs", "pub fn foo_fn() {}\n");
    write_src(tmp.path(), "fixtures/helpers.rs", "#[doc(hidden)]\npub fn hidden_helper() {}\n");

    let outcome = verify(tmp.path());
    assert!(
        outcome.has_errors(),
        "doc(hidden) in file with no cfg(test) ancestor reference must still be flagged \
         (PR26 regression): {:?}",
        outcome.findings()
    );
}

/// PR27: `src/lib.rs` has `#[doc(hidden)]` AND a sibling `bypass.rs` declares
/// `#[cfg(test)] #[path = "lib.rs"] mod fake_root;`.
/// Before the fix the crate root was misclassified as test-only (cfg_test_ref=true,
/// prod_ref=false) and the finding was silently dropped.  After the fix the root is
/// always treated as production and the violation must be flagged.
#[test]
fn test_flags_doc_hidden_in_lib_rs_when_sibling_has_cfg_test_path_ref_to_lib() {
    let tmp = TempDir::new().unwrap();
    write_arch_rules(tmp.path());
    write_src(tmp.path(), "lib.rs", "#[doc(hidden)]\npub fn x() {}\n");
    write_src(
        tmp.path(),
        "bypass.rs",
        concat!("#[cfg(test)]\n", "#[path = \"lib.rs\"]\n", "mod fake_root;\n",),
    );

    let outcome = verify(tmp.path());
    assert!(
        outcome.has_errors(),
        "doc(hidden) in lib.rs must be flagged even when a sibling declares \
         #[cfg(test)] #[path = \"lib.rs\"] mod … (PR27): {:?}",
        outcome.findings()
    );
}

/// PR28: `src/main.rs` has `#[doc(hidden)]` AND a sibling declares
/// `#[cfg(test)] #[path = "main.rs"] mod fake_main;`.
/// The crate root must not be misclassified as test-only; the violation must be flagged.
#[test]
fn test_flags_doc_hidden_in_main_rs_when_sibling_has_cfg_test_path_ref_to_main() {
    let tmp = TempDir::new().unwrap();
    write_arch_rules(tmp.path());
    write_src(tmp.path(), "main.rs", "#[doc(hidden)]\npub fn x() {}\n");
    write_src(
        tmp.path(),
        "sibling.rs",
        concat!("#[cfg(test)]\n", "#[path = \"main.rs\"]\n", "mod fake_main;\n",),
    );

    let outcome = verify(tmp.path());
    assert!(
        outcome.has_errors(),
        "doc(hidden) in main.rs must be flagged even when a sibling declares \
         #[cfg(test)] #[path = \"main.rs\"] mod … (PR28): {:?}",
        outcome.findings()
    );
}

/// PR29: regression — a non-root file referenced only via `#[cfg(test)] mod` in
/// `lib.rs` must still be classified as test-only and NOT flagged.
/// The crate-root fix must not affect the existing test-helper exclusion logic.
#[test]
fn test_non_root_file_cfg_test_only_ref_from_lib_rs_remains_test_only() {
    let tmp = TempDir::new().unwrap();
    write_arch_rules(tmp.path());
    // lib.rs (crate root) only references helpers.rs via cfg(test) — no prod ref.
    write_src(
        tmp.path(),
        "lib.rs",
        concat!(
            "pub fn clean() {}\n",
            "#[cfg(test)]\n",
            "#[path = \"helpers.rs\"]\n",
            "mod test_helpers;\n",
        ),
    );
    write_src(tmp.path(), "helpers.rs", "#[doc(hidden)]\npub fn hidden_helper() {}\n");

    let outcome = verify(tmp.path());
    assert!(
        outcome.is_ok(),
        "doc(hidden) in a non-root file referenced only via cfg(test) mod must not be \
         flagged (PR29 regression): {:?}",
        outcome.findings()
    );
}

/// PR30: a same-directory sibling file that declares `#[cfg(test)] mod b;` (no
/// `#[path]`) must NOT exclude `src/b.rs` from scanning.  In Rust, `mod b;` in
/// `src/a.rs` resolves to `src/a/b.rs`, not to sibling `src/b.rs`.  Without the
/// fix the sibling probe incorrectly propagates the `cfg(test)` gate to `b.rs` and
/// silently drops the `#[doc(hidden)]` finding.
#[test]
fn test_flags_doc_hidden_in_b_rs_when_sibling_has_cfg_test_mod_b_without_path() {
    let tmp = TempDir::new().unwrap();
    write_arch_rules(tmp.path());
    write_src(tmp.path(), "lib.rs", "pub mod a;\n");
    // a.rs has a cfg(test)-gated bare `mod b;` (no #[path]).
    // In Rust this resolves to src/a/b.rs, not to sibling src/b.rs.
    write_src(tmp.path(), "a.rs", "pub fn a_fn() {}\n#[cfg(test)]\nmod b;\n");
    // b.rs has a violation.
    write_src(tmp.path(), "b.rs", "#[doc(hidden)]\npub fn x() {}\n");

    let outcome = verify(tmp.path());
    assert!(
        outcome.has_errors(),
        "#[doc(hidden)] in b.rs must be flagged; sibling `mod b;` (no #[path]) in a.rs must \
         not exclude b.rs (PR30): {:?}",
        outcome.findings()
    );
}

/// PR32: `const X: usize = { #[doc(hidden)] struct Hidden; 1 };` — `#[doc(hidden)]`
/// on a local struct inside a const initializer block must be flagged.
#[test]
fn test_detects_doc_hidden_on_local_item_in_const_initializer_block() {
    let tmp = TempDir::new().unwrap();
    setup(tmp.path(), "const X: usize = { #[doc(hidden)] struct Hidden; 1 };\n");
    let outcome = verify(tmp.path());
    assert!(
        outcome.has_errors(),
        "expected error for #[doc(hidden)] on local struct in const initializer block (PR32): \
         {:?}",
        outcome.findings()
    );
}

/// PR33: `static Y: usize = { #[doc(hidden)] fn h() {} 0 };` — `#[doc(hidden)]`
/// on a local function inside a static initializer block must be flagged.
#[test]
fn test_detects_doc_hidden_on_local_item_in_static_initializer_block() {
    let tmp = TempDir::new().unwrap();
    setup(tmp.path(), "static Y: usize = { #[doc(hidden)] fn h() {} 0 };\n");
    let outcome = verify(tmp.path());
    assert!(
        outcome.has_errors(),
        "expected error for #[doc(hidden)] on local fn in static initializer block (PR33): \
         {:?}",
        outcome.findings()
    );
}

// ── PR35-PR36: #[path] file disambiguation (foo.rs vs foo/mod.rs) ────────────

/// PR35: a `cfg(test)` `#[path]` attribute that points to `foo/mod.rs` must NOT
/// cause `foo.rs` to be treated as test-only.  Both files share the module-path
/// component sequence `["foo"]`, so the old module-path comparison was ambiguous
/// and incorrectly excluded `foo.rs`.  The fix resolves the `#[path]` value to an
/// absolute file path and compares it against the actual target file.
#[test]
fn test_flags_doc_hidden_in_foo_rs_when_cfg_test_path_attr_points_to_foo_mod_rs() {
    let tmp = TempDir::new().unwrap();
    write_arch_rules(tmp.path());
    // lib.rs points its cfg(test) module at foo/mod.rs via #[path].
    write_src(
        tmp.path(),
        "lib.rs",
        concat!(
            "pub fn clean() {}\n",
            "#[cfg(test)]\n",
            "#[path = \"foo/mod.rs\"]\n",
            "mod foo;\n",
        ),
    );
    // foo.rs is a production file distinct from foo/mod.rs; #[doc(hidden)] must be flagged.
    write_src(tmp.path(), "foo.rs", "#[doc(hidden)]\npub fn bad() {}\n");
    // foo/mod.rs is the literal target of the #[path] attribute.
    write_src(tmp.path(), "foo/mod.rs", "pub fn ok() {}\n");

    let outcome = verify(tmp.path());
    assert!(
        outcome.has_errors(),
        "#[doc(hidden)] in foo.rs must be flagged; #[path = \"foo/mod.rs\"] resolves to \
         foo/mod.rs, not foo.rs (PR35): {:?}",
        outcome.findings()
    );
}

/// PR36: regression — `#[cfg(test)] #[path = "foo/mod.rs"] mod foo;` must still
/// exclude `foo/mod.rs` itself (the literal target of the attribute).
#[test]
fn test_ignores_doc_hidden_in_foo_mod_rs_when_cfg_test_path_attr_points_to_foo_mod_rs() {
    let tmp = TempDir::new().unwrap();
    write_arch_rules(tmp.path());
    write_src(
        tmp.path(),
        "lib.rs",
        concat!(
            "pub fn clean() {}\n",
            "#[cfg(test)]\n",
            "#[path = \"foo/mod.rs\"]\n",
            "mod foo;\n",
        ),
    );
    // foo/mod.rs is the direct target of the cfg(test) #[path] attribute; must be excluded.
    write_src(tmp.path(), "foo/mod.rs", "#[doc(hidden)]\npub fn hidden_helper() {}\n");

    let outcome = verify(tmp.path());
    assert!(
        outcome.is_ok(),
        "#[doc(hidden)] in foo/mod.rs must not be flagged when #[path = \"foo/mod.rs\"] is \
         cfg(test) (PR36 regression): {:?}",
        outcome.findings()
    );
}

// ── PR37-PR38: #[path] inside inline mods — basedir accumulation ─────────────

/// PR37: `lib.rs` has `#[cfg(test)] mod tests { #[path = "helpers.rs"] mod helpers; }` +
/// `src/tests/helpers.rs` has a `#[doc(hidden)]` item.
/// rustc resolves `helpers.rs` relative to the inline mod name (`src/tests/`),
/// not the containing file's directory (`src/`).  The file must be excluded.
#[test]
fn test_ignores_doc_hidden_in_path_attr_mod_inside_inline_cfg_test_mod() {
    let tmp = TempDir::new().unwrap();
    write_arch_rules(tmp.path());
    write_src(
        tmp.path(),
        "lib.rs",
        concat!(
            "pub fn clean() {}\n",
            "#[cfg(test)]\n",
            "mod tests {\n",
            "    #[path = \"helpers.rs\"]\n",
            "    mod helpers;\n",
            "}\n",
        ),
    );
    write_src(tmp.path(), "tests/helpers.rs", "#[doc(hidden)]\npub fn hidden_helper() {}\n");

    let outcome = verify(tmp.path());
    assert!(
        outcome.is_ok(),
        "#[path = \"helpers.rs\"] inside inline #[cfg(test)] mod tests must resolve to \
         tests/helpers.rs and be excluded (PR37): {:?}",
        outcome.findings()
    );
}

/// PR38: multi-level inline mods — `lib.rs` has
/// `mod a { mod b { #[cfg(test)] #[path = "h.rs"] mod helpers; } }` +
/// `src/a/b/h.rs` has `#[doc(hidden)]`.
/// rustc accumulates inline mod names for `#[path]` resolution: `h.rs` resolves
/// to `src/a/b/h.rs`.  The file must be excluded.
#[test]
fn test_ignores_doc_hidden_in_path_attr_mod_inside_multi_level_inline_mods() {
    let tmp = TempDir::new().unwrap();
    write_arch_rules(tmp.path());
    write_src(
        tmp.path(),
        "lib.rs",
        concat!(
            "pub fn clean() {}\n",
            "mod a {\n",
            "    mod b {\n",
            "        #[cfg(test)]\n",
            "        #[path = \"h.rs\"]\n",
            "        mod helpers;\n",
            "    }\n",
            "}\n",
        ),
    );
    write_src(tmp.path(), "a/b/h.rs", "#[doc(hidden)]\npub fn hidden() {}\n");

    let outcome = verify(tmp.path());
    assert!(
        outcome.is_ok(),
        "#[path = \"h.rs\"] inside multi-level inline `mod a {{ mod b {{ … }} }}` must \
         resolve to a/b/h.rs and be excluded (PR38): {:?}",
        outcome.findings()
    );
}

// ── PR39-PR41: associated const initializers in impl/trait blocks ────────────

/// PR39: `impl S { const X: usize = { #[doc(hidden)] struct H; 0 }; }` —
/// `#[doc(hidden)]` on a local item inside an associated const initializer must
/// be flagged (mirrors top-level `Item::Const` handling added in PR32).
#[test]
fn test_detects_doc_hidden_on_local_item_in_impl_associated_const_initializer() {
    let tmp = TempDir::new().unwrap();
    setup(tmp.path(), "struct S;\nimpl S { const X: usize = { #[doc(hidden)] struct H; 0 }; }\n");
    let outcome = verify(tmp.path());
    assert!(
        outcome.has_errors(),
        "expected error for #[doc(hidden)] on local struct in associated const initializer \
         inside impl block (PR39): {:?}",
        outcome.findings()
    );
}

/// PR40: `pub trait T { const Y: usize = { #[doc(hidden)] fn h() {} 0 }; }` —
/// `#[doc(hidden)]` on a local item inside a trait associated const default value
/// must be flagged (mirrors top-level `Item::Const` handling added in PR32).
#[test]
fn test_detects_doc_hidden_on_local_item_in_trait_associated_const_default() {
    let tmp = TempDir::new().unwrap();
    setup(tmp.path(), "pub trait T { const Y: usize = { #[doc(hidden)] fn h() {} 0 }; }\n");
    let outcome = verify(tmp.path());
    assert!(
        outcome.has_errors(),
        "expected error for #[doc(hidden)] on local fn in trait associated const default \
         value (PR40): {:?}",
        outcome.findings()
    );
}

/// PR41: regression — top-level const/static initializers and impl method bodies
/// continue to be flagged after the PR39/PR40 refactor.
#[test]
fn test_regression_top_level_const_static_and_impl_fn_body_still_flagged() {
    // Top-level const initializer (originally covered by PR32).
    let tmp = TempDir::new().unwrap();
    setup(tmp.path(), "const X: usize = { #[doc(hidden)] struct Hidden; 1 };\n");
    let outcome = verify(tmp.path());
    assert!(
        outcome.has_errors(),
        "regression (PR41): top-level const initializer must still be flagged: {:?}",
        outcome.findings()
    );

    // Top-level static initializer (originally covered by PR33).
    let tmp2 = TempDir::new().unwrap();
    setup(tmp2.path(), "static Y: usize = { #[doc(hidden)] fn h() {} 0 };\n");
    let outcome2 = verify(tmp2.path());
    assert!(
        outcome2.has_errors(),
        "regression (PR41): top-level static initializer must still be flagged: {:?}",
        outcome2.findings()
    );

    // impl method body (originally covered by earlier rounds).
    let tmp3 = TempDir::new().unwrap();
    setup(tmp3.path(), "struct S;\nimpl S { fn f() { #[doc(hidden)] struct Local; } }\n");
    let outcome3 = verify(tmp3.path());
    assert!(
        outcome3.has_errors(),
        "regression (PR41): impl method body must still be flagged: {:?}",
        outcome3.findings()
    );
}

/// PR31: regression — a same-directory sibling that uses `#[cfg(test)] #[path =
/// "shared_helpers.rs"] mod tests;` must still cause `shared_helpers.rs` to be
/// classified as test-only and excluded from scanning.  The sibling probe must
/// continue to work for `#[path]`-based declarations after the PR30 fix.
#[test]
fn test_sibling_cfg_test_path_attr_still_excludes_target() {
    let tmp = TempDir::new().unwrap();
    write_arch_rules(tmp.path());
    write_src(tmp.path(), "lib.rs", "pub mod a;\n");
    // a.rs references shared_helpers.rs via #[path] under cfg(test).
    write_src(
        tmp.path(),
        "a.rs",
        concat!(
            "pub fn a_fn() {}\n",
            "#[cfg(test)]\n",
            "#[path = \"shared_helpers.rs\"]\n",
            "mod tests;\n",
        ),
    );
    // shared_helpers.rs is a sibling of a.rs and referenced only via cfg(test) #[path].
    write_src(tmp.path(), "shared_helpers.rs", "#[doc(hidden)]\npub fn hidden_helper() {}\n");

    let outcome = verify(tmp.path());
    assert!(
        outcome.is_ok(),
        "shared_helpers.rs referenced only via #[cfg(test)] #[path] must remain excluded \
         after the PR30 sibling-probe fix (PR31 regression): {:?}",
        outcome.findings()
    );
}
