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

/// Write a Rust source file at `test-layer/tests/<rel>`.
fn write_tests(root: &Path, rel: &str, content: &str) {
    let path = root.join("test-layer/tests").join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&path, content).unwrap();
}

/// Write a Rust source file at `test-layer/examples/<rel>`.
fn write_examples(root: &Path, rel: &str, content: &str) {
    let path = root.join("test-layer/examples").join(rel);
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

// ── Union field-level attribute scanning ─────────────────────────────────────

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

// ── Local items in function / method bodies ──────────────────────────────────

#[test]
fn test_detects_doc_hidden_on_local_struct_in_fn_body() {
    let tmp = TempDir::new().unwrap();
    setup(tmp.path(), concat!("fn f() {\n", "    #[doc(hidden)]\n", "    struct Local;\n", "}\n"));
    let outcome = verify(tmp.path());
    assert!(
        outcome.has_errors(),
        "expected error for #[doc(hidden)] on local struct in fn body: {:?}",
        outcome.findings()
    );
}

#[test]
fn test_detects_doc_hidden_on_local_fn_in_fn_body() {
    let tmp = TempDir::new().unwrap();
    setup(tmp.path(), concat!("fn f() {\n", "    #[doc(hidden)]\n", "    fn inner() {}\n", "}\n"));
    let outcome = verify(tmp.path());
    assert!(
        outcome.has_errors(),
        "expected error for #[doc(hidden)] on local fn in fn body: {:?}",
        outcome.findings()
    );
}

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
        "expected error for #[doc(hidden)] on local enum in impl method body: {:?}",
        outcome.findings()
    );
}

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
        "expected error for #[doc(hidden)] on local struct in nested if block: {:?}",
        outcome.findings()
    );
}

// ── const / static initializer bodies ────────────────────────────────────────

#[test]
fn test_detects_doc_hidden_on_local_item_in_const_initializer_block() {
    let tmp = TempDir::new().unwrap();
    setup(tmp.path(), "const X: usize = { #[doc(hidden)] struct Hidden; 1 };\n");
    let outcome = verify(tmp.path());
    assert!(
        outcome.has_errors(),
        "expected error for #[doc(hidden)] on local struct in const initializer block: {:?}",
        outcome.findings()
    );
}

#[test]
fn test_detects_doc_hidden_on_local_item_in_static_initializer_block() {
    let tmp = TempDir::new().unwrap();
    setup(tmp.path(), "static Y: usize = { #[doc(hidden)] fn h() {} 0 };\n");
    let outcome = verify(tmp.path());
    assert!(
        outcome.has_errors(),
        "expected error for #[doc(hidden)] on local fn in static initializer block: {:?}",
        outcome.findings()
    );
}

// ── impl / trait associated const initializers ───────────────────────────────

#[test]
fn test_detects_doc_hidden_on_local_item_in_impl_associated_const_initializer() {
    let tmp = TempDir::new().unwrap();
    setup(tmp.path(), "struct S;\nimpl S { const X: usize = { #[doc(hidden)] struct H; 0 }; }\n");
    let outcome = verify(tmp.path());
    assert!(
        outcome.has_errors(),
        "expected error for #[doc(hidden)] on local struct in associated const initializer \
         inside impl block: {:?}",
        outcome.findings()
    );
}

#[test]
fn test_detects_doc_hidden_on_local_item_in_trait_associated_const_default() {
    let tmp = TempDir::new().unwrap();
    setup(tmp.path(), "pub trait T { const Y: usize = { #[doc(hidden)] fn h() {} 0 }; }\n");
    let outcome = verify(tmp.path());
    assert!(
        outcome.has_errors(),
        "expected error for #[doc(hidden)] on local fn in trait associated const default \
         value: {:?}",
        outcome.findings()
    );
}

#[test]
fn test_regression_top_level_const_static_and_impl_fn_body_still_flagged() {
    // Top-level const initializer.
    let tmp = TempDir::new().unwrap();
    setup(tmp.path(), "const X: usize = { #[doc(hidden)] struct Hidden; 1 };\n");
    let outcome = verify(tmp.path());
    assert!(
        outcome.has_errors(),
        "regression: top-level const initializer must still be flagged: {:?}",
        outcome.findings()
    );

    // Top-level static initializer.
    let tmp2 = TempDir::new().unwrap();
    setup(tmp2.path(), "static Y: usize = { #[doc(hidden)] fn h() {} 0 };\n");
    let outcome2 = verify(tmp2.path());
    assert!(
        outcome2.has_errors(),
        "regression: top-level static initializer must still be flagged: {:?}",
        outcome2.findings()
    );

    // impl method body.
    let tmp3 = TempDir::new().unwrap();
    setup(tmp3.path(), "struct S;\nimpl S { fn f() { #[doc(hidden)] struct Local; } }\n");
    let outcome3 = verify(tmp3.path());
    assert!(
        outcome3.has_errors(),
        "regression: impl method body must still be flagged: {:?}",
        outcome3.findings()
    );
}

// ── New behavior: test-gated items are now flagged (no test exclusion) ────────

/// Without test exclusion, `#[doc(hidden)]` inside a `#[cfg(test)] fn` is flagged.
/// (Test items do not appear in rustdoc paths, so the attribute has no effect,
/// but uniform scanning is simpler than carving out exclusions.)
#[test]
fn test_flags_doc_hidden_inside_cfg_test_fn() {
    let tmp = TempDir::new().unwrap();
    setup(
        tmp.path(),
        concat!("#[cfg(test)]\n", "fn t() {\n", "    #[doc(hidden)]\n", "    struct X;\n", "}\n"),
    );
    let outcome = verify(tmp.path());
    assert!(
        outcome.has_errors(),
        "#[doc(hidden)] inside #[cfg(test)] fn must be flagged (no test exclusion): {:?}",
        outcome.findings()
    );
}

/// Without test exclusion, `#[test] #[doc(hidden)] pub fn x()` is also flagged.
#[test]
fn test_flags_doc_hidden_on_test_fn_attribute() {
    let tmp = TempDir::new().unwrap();
    setup(tmp.path(), "#[test]\n#[doc(hidden)]\npub fn my_test() {}\n");
    let outcome = verify(tmp.path());
    assert!(
        outcome.has_errors(),
        "#[test] #[doc(hidden)] fn must be flagged (no test exclusion): {:?}",
        outcome.findings()
    );
}

// ── BR1/BR2: tests/ and examples/ directories are scanned ────────────────────

/// BR1: `#[doc(hidden)]` in `tests/it.rs` must be flagged (scan broadened to
/// whole layer directory, not just `src/`).
#[test]
fn test_flags_doc_hidden_in_tests_dir() {
    let tmp = TempDir::new().unwrap();
    write_arch_rules(tmp.path());
    // src/ is clean; violation lives in tests/.
    write_src(tmp.path(), "lib.rs", "pub fn ok() {}\n");
    write_tests(tmp.path(), "it.rs", "#[doc(hidden)]\npub fn x() {}\n");
    let outcome = verify(tmp.path());
    assert!(
        outcome.has_errors(),
        "#[doc(hidden)] in tests/it.rs must be flagged: {:?}",
        outcome.findings()
    );
}

/// BR2: `#[doc(hidden)]` in `examples/ex.rs` must be flagged (scan broadened
/// to whole layer directory, not just `src/`).
#[test]
fn test_flags_doc_hidden_in_examples_dir() {
    let tmp = TempDir::new().unwrap();
    write_arch_rules(tmp.path());
    // src/ is clean; violation lives in examples/.
    write_src(tmp.path(), "lib.rs", "pub fn ok() {}\n");
    write_examples(tmp.path(), "ex.rs", "#[doc(hidden)]\npub fn x() {}\n");
    let outcome = verify(tmp.path());
    assert!(
        outcome.has_errors(),
        "#[doc(hidden)] in examples/ex.rs must be flagged: {:?}",
        outcome.findings()
    );
}

// ── RI: raw-identifier spellings are detected ─────────────────────────────────

/// RI1: `#[doc(r#hidden)]` — raw identifier in the doc arg list — must be flagged.
#[test]
fn test_detects_doc_raw_hidden_ident() {
    let tmp = TempDir::new().unwrap();
    setup(tmp.path(), "#[doc(r#hidden)]\npub fn x() {}\n");
    let outcome = verify(tmp.path());
    assert!(outcome.has_errors(), "#[doc(r#hidden)] must be flagged: {:?}", outcome.findings());
}

/// RI2: `#[r#doc(hidden)]` — raw identifier used as the attribute path — must be flagged.
#[test]
fn test_detects_raw_doc_path_attr() {
    let tmp = TempDir::new().unwrap();
    setup(tmp.path(), "#[r#doc(hidden)]\npub fn y() {}\n");
    let outcome = verify(tmp.path());
    assert!(outcome.has_errors(), "#[r#doc(hidden)] must be flagged: {:?}", outcome.findings());
}

/// RI3: `#[cfg_attr(test, doc(r#hidden))]` — raw identifier inside cfg_attr's
/// doc arg list — must be flagged.
#[test]
fn test_detects_cfg_attr_doc_raw_hidden_ident() {
    let tmp = TempDir::new().unwrap();
    setup(tmp.path(), "#[cfg_attr(test, doc(r#hidden))]\npub fn z() {}\n");
    let outcome = verify(tmp.path());
    assert!(
        outcome.has_errors(),
        "#[cfg_attr(test, doc(r#hidden))] must be flagged: {:?}",
        outcome.findings()
    );
}
