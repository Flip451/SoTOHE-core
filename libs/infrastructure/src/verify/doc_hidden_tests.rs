//! Tests for [`doc_hidden`] (split out to keep the main module under the 400-line guideline).

use std::path::Path;

use tempfile::TempDir;

use super::*;

/// Write a minimal `architecture-rules.json` at `root` that declares one
/// layer with source at `<root>/<layer_rel_path>/src/`.
fn setup_arch_rules(root: &Path, layer_rel_path: &str) {
    let json = format!(
        r#"{{"version":2,"layers":[{{"crate":"test-layer","path":"{layer_rel_path}","may_depend_on":[],"deny_reason":""}}]}}"#
    );
    std::fs::write(root.join("architecture-rules.json"), json).unwrap();
}

/// Write a Rust source file at `<root>/<rel>`, creating parent directories.
fn write_src(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, content).unwrap();
}

// --- AC-02: no violations → ok ---

#[test]
fn no_violations_returns_ok() {
    let tmp = TempDir::new().unwrap();
    setup_arch_rules(tmp.path(), "layer");
    write_src(tmp.path(), "layer/src/lib.rs", "pub fn foo() {}\n");

    let outcome = verify(tmp.path());

    assert!(outcome.is_ok(), "expected ok but got: {:?}", outcome.findings());
    assert!(outcome.findings().is_empty());
}

// --- AC-01 / CN-05: pub + #[doc(hidden)] → error with file path and item name ---

#[test]
fn pub_doc_hidden_fn_returns_error_with_path_and_name() {
    let tmp = TempDir::new().unwrap();
    setup_arch_rules(tmp.path(), "layer");
    write_src(tmp.path(), "layer/src/lib.rs", "#[doc(hidden)]\npub fn foo() {}\n");

    let outcome = verify(tmp.path());

    assert!(outcome.has_errors(), "expected error findings");
    let msg = outcome.findings()[0].to_string();
    assert!(msg.contains("foo"), "item name missing: {msg}");
    assert!(msg.contains("lib.rs"), "file path missing: {msg}");
    assert!(msg.contains("pub + #[doc(hidden)] is forbidden"), "reason missing: {msg}");
}

// --- AC-04 / IN-06: #[cfg(test)] block items not reported ---

#[test]
fn cfg_test_mod_items_not_reported() {
    let tmp = TempDir::new().unwrap();
    setup_arch_rules(tmp.path(), "layer");
    write_src(
        tmp.path(),
        "layer/src/lib.rs",
        "#[cfg(test)]\nmod tests {\n    #[doc(hidden)]\n    pub fn bar() {}\n}\n",
    );

    let outcome = verify(tmp.path());

    assert!(outcome.is_ok(), "expected ok but got: {:?}", outcome.findings());
    assert!(outcome.findings().is_empty());
}

#[test]
fn test_cfg_test_mod_does_not_skip_following_production_items() {
    let tmp = TempDir::new().unwrap();
    setup_arch_rules(tmp.path(), "layer");
    write_src(
        tmp.path(),
        "layer/src/lib.rs",
        "#[cfg(test)]\nmod tests {\n    #[doc(hidden)]\n    pub fn ignored() {}\n}\n#[doc(hidden)]\npub fn visible_after_tests() {}\n",
    );

    let outcome = verify(tmp.path());

    assert!(outcome.has_errors(), "expected production item violation");
    let msg = outcome.findings()[0].to_string();
    assert!(msg.contains("visible_after_tests"), "item name missing: {msg}");
    assert!(msg.contains("pub + #[doc(hidden)] is forbidden"), "reason missing: {msg}");
}

// --- AC-04 / IN-06: #[test] attribute items not reported ---

#[test]
fn test_attr_fn_not_reported() {
    let tmp = TempDir::new().unwrap();
    setup_arch_rules(tmp.path(), "layer");
    write_src(tmp.path(), "layer/src/lib.rs", "#[test]\n#[doc(hidden)]\npub fn baz() {}\n");

    let outcome = verify(tmp.path());

    assert!(outcome.is_ok(), "expected ok but got: {:?}", outcome.findings());
    assert!(outcome.findings().is_empty());
}

// --- OS-04: non-pub #[doc(hidden)] items not reported ---

#[test]
fn doc_hidden_without_pub_not_reported() {
    let tmp = TempDir::new().unwrap();
    setup_arch_rules(tmp.path(), "layer");
    write_src(tmp.path(), "layer/src/lib.rs", "#[doc(hidden)]\nfn private() {}\n");

    let outcome = verify(tmp.path());

    assert!(outcome.is_ok(), "expected ok but got: {:?}", outcome.findings());
    assert!(outcome.findings().is_empty());
}

// --- Additional: pub(crate) + #[doc(hidden)] not reported (OS-04) ---

#[test]
fn pub_crate_doc_hidden_not_reported() {
    let tmp = TempDir::new().unwrap();
    setup_arch_rules(tmp.path(), "layer");
    write_src(tmp.path(), "layer/src/lib.rs", "#[doc(hidden)]\npub(crate) fn restricted() {}\n");

    let outcome = verify(tmp.path());

    assert!(outcome.is_ok(), "expected ok but got: {:?}", outcome.findings());
    assert!(outcome.findings().is_empty());
}

#[test]
fn test_verify_public_trait_method_doc_hidden_returns_error() {
    let tmp = TempDir::new().unwrap();
    setup_arch_rules(tmp.path(), "layer");
    write_src(
        tmp.path(),
        "layer/src/lib.rs",
        "pub trait Visible {\n    #[doc(hidden)]\n    fn hidden(&self);\n}\n",
    );

    let outcome = verify(tmp.path());

    assert!(outcome.has_errors(), "expected trait method violation");
    let msg = outcome.findings()[0].to_string();
    assert!(msg.contains("Visible::hidden"), "trait method name missing: {msg}");
    assert!(msg.contains("pub + #[doc(hidden)] is forbidden"), "reason missing: {msg}");
}

#[test]
fn test_verify_public_impl_method_doc_hidden_returns_error() {
    let tmp = TempDir::new().unwrap();
    setup_arch_rules(tmp.path(), "layer");
    write_src(
        tmp.path(),
        "layer/src/lib.rs",
        "pub struct Visible;\nimpl Visible {\n    #[doc(hidden)]\n    pub fn hidden(&self) {}\n}\n",
    );

    let outcome = verify(tmp.path());

    assert!(outcome.has_errors(), "expected impl method violation");
    let msg = outcome.findings()[0].to_string();
    assert!(msg.contains("hidden"), "impl method name missing: {msg}");
    assert!(msg.contains("pub + #[doc(hidden)] is forbidden"), "reason missing: {msg}");
}

#[test]
fn test_verify_public_struct_field_doc_hidden_returns_error() {
    let tmp = TempDir::new().unwrap();
    setup_arch_rules(tmp.path(), "layer");
    write_src(
        tmp.path(),
        "layer/src/lib.rs",
        "pub struct Visible {\n    #[doc(hidden)]\n    pub hidden: String,\n}\n",
    );

    let outcome = verify(tmp.path());

    assert!(outcome.has_errors(), "expected struct field violation");
    let msg = outcome.findings()[0].to_string();
    assert!(msg.contains("Visible::hidden"), "field name missing: {msg}");
    assert!(msg.contains("pub + #[doc(hidden)] is forbidden"), "reason missing: {msg}");
}

#[test]
fn test_verify_public_enum_variant_doc_hidden_returns_error() {
    let tmp = TempDir::new().unwrap();
    setup_arch_rules(tmp.path(), "layer");
    write_src(
        tmp.path(),
        "layer/src/lib.rs",
        "pub enum Visible {\n    #[doc(hidden)]\n    Hidden,\n    Shown,\n}\n",
    );

    let outcome = verify(tmp.path());

    assert!(outcome.has_errors(), "expected enum variant violation");
    let msg = outcome.findings()[0].to_string();
    assert!(msg.contains("Visible::Hidden"), "variant name missing: {msg}");
    assert!(msg.contains("pub + #[doc(hidden)] is forbidden"), "reason missing: {msg}");
}

#[test]
fn test_verify_layer_path_escape_returns_error() {
    let tmp = TempDir::new().unwrap();
    setup_arch_rules(tmp.path(), "../outside");

    let outcome = verify(tmp.path());

    assert!(outcome.has_errors(), "expected path containment error");
    let msg = outcome.findings()[0].to_string();
    assert!(msg.contains("../outside"), "layer path missing: {msg}");
    assert!(msg.contains("resolves outside trusted root"), "containment reason missing: {msg}");
}

// --- cfg_attr forms: AC-01 coverage for `#[cfg_attr(<pred>, doc(hidden))]` ---

// (f) cfg_attr(not(test), doc(hidden)) on a pub fn — flagged
#[test]
fn cfg_attr_not_test_doc_hidden_pub_fn_is_flagged() {
    let tmp = TempDir::new().unwrap();
    setup_arch_rules(tmp.path(), "layer");
    write_src(
        tmp.path(),
        "layer/src/lib.rs",
        "#[cfg_attr(not(test), doc(hidden))]\npub fn foo() {}\n",
    );

    let outcome = verify(tmp.path());

    assert!(outcome.has_errors(), "expected error for cfg_attr(not(test), doc(hidden))");
    let msg = outcome.findings()[0].to_string();
    assert!(msg.contains("foo"), "item name missing: {msg}");
    assert!(msg.contains("lib.rs"), "file path missing: {msg}");
    assert!(msg.contains("pub + #[doc(hidden)] is forbidden"), "base reason missing: {msg}");
    assert!(
        msg.contains("cfg_attr(<pred>, doc(hidden)) forms are also forbidden"),
        "cfg_attr suffix missing: {msg}"
    );
}

// (g) cfg_attr(feature = "x", doc(hidden)) on a pub fn — flagged
#[test]
fn cfg_attr_feature_doc_hidden_pub_fn_is_flagged() {
    let tmp = TempDir::new().unwrap();
    setup_arch_rules(tmp.path(), "layer");
    write_src(
        tmp.path(),
        "layer/src/lib.rs",
        "#[cfg_attr(feature = \"x\", doc(hidden))]\npub fn bar() {}\n",
    );

    let outcome = verify(tmp.path());

    assert!(outcome.has_errors(), "expected error for cfg_attr(feature = \"x\", doc(hidden))");
    let msg = outcome.findings()[0].to_string();
    assert!(msg.contains("bar"), "item name missing: {msg}");
    assert!(msg.contains("pub + #[doc(hidden)] is forbidden"), "base reason missing: {msg}");
    assert!(
        msg.contains("cfg_attr(<pred>, doc(hidden)) forms are also forbidden"),
        "cfg_attr suffix missing: {msg}"
    );
}

// (h) Nested cfg_attr: cfg_attr(not(test), cfg_attr(any(), doc(hidden))) — flagged
#[test]
fn nested_cfg_attr_doc_hidden_pub_fn_is_flagged() {
    let tmp = TempDir::new().unwrap();
    setup_arch_rules(tmp.path(), "layer");
    write_src(
        tmp.path(),
        "layer/src/lib.rs",
        "#[cfg_attr(not(test), cfg_attr(any(), doc(hidden)))]\npub fn baz() {}\n",
    );

    let outcome = verify(tmp.path());

    assert!(outcome.has_errors(), "expected error for nested cfg_attr doc(hidden)");
    let msg = outcome.findings()[0].to_string();
    assert!(msg.contains("baz"), "item name missing: {msg}");
    assert!(msg.contains("pub + #[doc(hidden)] is forbidden"), "base reason missing: {msg}");
    assert!(
        msg.contains("cfg_attr(<pred>, doc(hidden)) forms are also forbidden"),
        "cfg_attr suffix missing: {msg}"
    );
}

// (i) cfg_attr(test, doc(hidden)) — flagged regardless of predicate; the rule
// forbids writing doc(hidden) on any pub item irrespective of the cfg condition.
#[test]
fn cfg_attr_test_doc_hidden_pub_fn_is_still_flagged() {
    let tmp = TempDir::new().unwrap();
    setup_arch_rules(tmp.path(), "layer");
    write_src(tmp.path(), "layer/src/lib.rs", "#[cfg_attr(test, doc(hidden))]\npub fn quux() {}\n");

    let outcome = verify(tmp.path());

    assert!(
        outcome.has_errors(),
        "expected error: cfg_attr(test, doc(hidden)) on pub item is forbidden regardless of predicate"
    );
    let msg = outcome.findings()[0].to_string();
    assert!(msg.contains("quux"), "item name missing: {msg}");
    assert!(msg.contains("pub + #[doc(hidden)] is forbidden"), "base reason missing: {msg}");
    assert!(
        msg.contains("cfg_attr(<pred>, doc(hidden)) forms are also forbidden"),
        "cfg_attr suffix missing: {msg}"
    );
}

// --- cfg-test file-backed module exclusion (2-pass approach) ---

// (j) lib.rs declares `#[cfg(test)] mod tests;` (file-backed); tests.rs contains
// a pub + #[doc(hidden)] helper. The helper must NOT be flagged because tests.rs
// is reachable only through the cfg-test module declaration.
#[test]
fn cfg_test_file_backed_mod_sibling_rs_not_flagged() {
    let tmp = TempDir::new().unwrap();
    setup_arch_rules(tmp.path(), "layer");
    write_src(tmp.path(), "layer/src/lib.rs", "#[cfg(test)] mod tests;\n");
    write_src(tmp.path(), "layer/src/tests.rs", "#[doc(hidden)]\npub fn helper() {}\n");

    let outcome = verify(tmp.path());

    assert!(
        outcome.is_ok(),
        "expected ok — tests.rs is cfg-test, helper should not be flagged: {:?}",
        outcome.findings()
    );
    assert!(outcome.findings().is_empty());
}

// (k) lib.rs declares `#[cfg(test)] mod tests;` (file-backed); tests/mod.rs
// contains a pub + #[doc(hidden)] helper. The helper must NOT be flagged.
#[test]
fn cfg_test_file_backed_mod_dir_mod_rs_not_flagged() {
    let tmp = TempDir::new().unwrap();
    setup_arch_rules(tmp.path(), "layer");
    write_src(tmp.path(), "layer/src/lib.rs", "#[cfg(test)] mod tests;\n");
    write_src(tmp.path(), "layer/src/tests/mod.rs", "#[doc(hidden)]\npub fn helper() {}\n");

    let outcome = verify(tmp.path());

    assert!(
        outcome.is_ok(),
        "expected ok — tests/mod.rs is cfg-test, helper should not be flagged: {:?}",
        outcome.findings()
    );
    assert!(outcome.findings().is_empty());
}

// (l) Nested case: lib.rs declares `mod foo;` (production); foo.rs declares
// `#[cfg(test)] mod inner;`; foo/inner.rs contains a pub + #[doc(hidden)] helper.
// The helper must NOT be flagged because inner.rs is transitively cfg-test.
#[test]
fn cfg_test_nested_file_backed_mod_not_flagged() {
    let tmp = TempDir::new().unwrap();
    setup_arch_rules(tmp.path(), "layer");
    write_src(tmp.path(), "layer/src/lib.rs", "mod foo;\n");
    write_src(tmp.path(), "layer/src/foo.rs", "#[cfg(test)] mod inner;\n");
    write_src(tmp.path(), "layer/src/foo/inner.rs", "#[doc(hidden)]\npub fn helper() {}\n");

    let outcome = verify(tmp.path());

    assert!(
        outcome.is_ok(),
        "expected ok — foo/inner.rs is transitively cfg-test, helper should not be flagged: {:?}",
        outcome.findings()
    );
    assert!(outcome.findings().is_empty());
}

// --- #[path = "..."] attribute resolution (file-backed cfg-test modules) ---

// (m) lib.rs declares `#[cfg(test)] #[path = "shared_helpers.rs"] mod tests;`
// (split-test pattern). shared_helpers.rs contains a pub + #[doc(hidden)] helper.
// The helper must NOT be flagged because shared_helpers.rs is cfg-test via #[path].
#[test]
fn cfg_test_file_backed_mod_with_path_attr_sibling_not_flagged() {
    let tmp = TempDir::new().unwrap();
    setup_arch_rules(tmp.path(), "layer");
    write_src(
        tmp.path(),
        "layer/src/lib.rs",
        "#[cfg(test)]\n#[path = \"shared_helpers.rs\"]\nmod tests;\n",
    );
    write_src(tmp.path(), "layer/src/shared_helpers.rs", "#[doc(hidden)]\npub fn helper() {}\n");

    let outcome = verify(tmp.path());

    assert!(
        outcome.is_ok(),
        "expected ok — shared_helpers.rs is cfg-test via #[path], helper should not be flagged: {:?}",
        outcome.findings()
    );
    assert!(outcome.findings().is_empty());
}

// (n) lib.rs declares `#[cfg(test)] #[path = "subdir/helpers.rs"] mod tests;`
// subdir/helpers.rs contains a pub + #[doc(hidden)] helper.
// The helper must NOT be flagged — #[path] is resolved relative to lib.rs's directory.
#[test]
fn cfg_test_file_backed_mod_with_path_attr_subdir_not_flagged() {
    let tmp = TempDir::new().unwrap();
    setup_arch_rules(tmp.path(), "layer");
    write_src(
        tmp.path(),
        "layer/src/lib.rs",
        "#[cfg(test)]\n#[path = \"subdir/helpers.rs\"]\nmod tests;\n",
    );
    write_src(tmp.path(), "layer/src/subdir/helpers.rs", "#[doc(hidden)]\npub fn helper() {}\n");

    let outcome = verify(tmp.path());

    assert!(
        outcome.is_ok(),
        "expected ok — subdir/helpers.rs is cfg-test via #[path], helper should not be flagged: {:?}",
        outcome.findings()
    );
    assert!(outcome.findings().is_empty());
}

// (o) Regression: #[cfg(test)] mod tests; without #[path] continues to work
// as before — tests.rs is correctly identified as cfg-test.
#[test]
fn cfg_test_file_backed_mod_without_path_attr_regression() {
    let tmp = TempDir::new().unwrap();
    setup_arch_rules(tmp.path(), "layer");
    write_src(tmp.path(), "layer/src/lib.rs", "#[cfg(test)] mod tests;\n");
    write_src(tmp.path(), "layer/src/tests.rs", "#[doc(hidden)]\npub fn helper() {}\n");

    let outcome = verify(tmp.path());

    assert!(
        outcome.is_ok(),
        "expected ok — tests.rs (no #[path]) is cfg-test, helper should not be flagged: {:?}",
        outcome.findings()
    );
    assert!(outcome.findings().is_empty());
}

// --- inline-mod #[path] and implicit-path resolution (2-pass, inline module stacking) ---

// (p) lib.rs has `#[cfg(test)] mod tests { #[path = "shared_helpers.rs"] mod helpers; }`
// (inline tests block). tests/shared_helpers.rs contains a pub + #[doc(hidden)] helper.
// The helper must NOT be flagged — inline mod + #[path] resolved via stacked basedir.
#[test]
fn cfg_test_inline_mod_with_path_attr_not_flagged() {
    let tmp = TempDir::new().unwrap();
    setup_arch_rules(tmp.path(), "layer");
    write_src(
        tmp.path(),
        "layer/src/lib.rs",
        "#[cfg(test)]\nmod tests {\n    #[path = \"shared_helpers.rs\"]\n    mod helpers;\n}\n",
    );
    write_src(
        tmp.path(),
        "layer/src/tests/shared_helpers.rs",
        "#[doc(hidden)]\npub fn helper() {}\n",
    );

    let outcome = verify(tmp.path());

    assert!(
        outcome.is_ok(),
        "expected ok — tests/shared_helpers.rs is cfg-test via inline mod + #[path], \
         helper should not be flagged: {:?}",
        outcome.findings()
    );
    assert!(outcome.findings().is_empty());
}

// (q) lib.rs has `#[cfg(test)] mod tests { mod helpers; }` (inline mod, no #[path]).
// tests/helpers.rs contains a pub + #[doc(hidden)] helper.
// The helper must NOT be flagged — inline mod child file-backed resolution (regression).
#[test]
fn cfg_test_inline_mod_implicit_child_not_flagged() {
    let tmp = TempDir::new().unwrap();
    setup_arch_rules(tmp.path(), "layer");
    write_src(tmp.path(), "layer/src/lib.rs", "#[cfg(test)]\nmod tests {\n    mod helpers;\n}\n");
    write_src(tmp.path(), "layer/src/tests/helpers.rs", "#[doc(hidden)]\npub fn helper() {}\n");

    let outcome = verify(tmp.path());

    assert!(
        outcome.is_ok(),
        "expected ok — tests/helpers.rs is cfg-test via inline mod (no #[path]), \
         helper should not be flagged: {:?}",
        outcome.findings()
    );
    assert!(outcome.findings().is_empty());
}

// (r) lib.rs has `#[cfg(test)] mod tests { mod nested { #[path = "deep.rs"] mod deep; } }`
// (2-level inline nesting). tests/nested/deep.rs contains a pub + #[doc(hidden)] helper.
// The helper must NOT be flagged — two stacked inline basedirs + #[path].
#[test]
fn cfg_test_two_level_inline_mod_with_path_attr_not_flagged() {
    let tmp = TempDir::new().unwrap();
    setup_arch_rules(tmp.path(), "layer");
    write_src(
        tmp.path(),
        "layer/src/lib.rs",
        "#[cfg(test)]\nmod tests {\n    mod nested {\n        #[path = \"deep.rs\"]\n        mod deep;\n    }\n}\n",
    );
    write_src(tmp.path(), "layer/src/tests/nested/deep.rs", "#[doc(hidden)]\npub fn helper() {}\n");

    let outcome = verify(tmp.path());

    assert!(
        outcome.is_ok(),
        "expected ok — tests/nested/deep.rs is cfg-test via 2-level inline mod + #[path], \
         helper should not be flagged: {:?}",
        outcome.findings()
    );
    assert!(outcome.findings().is_empty());
}

// --- Combined doc meta forms: #[doc(hidden, alias = "x")] etc. ---

// (s) #[doc(hidden, alias = "x")] on a pub fn — flagged (combined meta, hidden first)
#[test]
fn doc_hidden_with_alias_combined_is_flagged() {
    let tmp = TempDir::new().unwrap();
    setup_arch_rules(tmp.path(), "layer");
    write_src(tmp.path(), "layer/src/lib.rs", "#[doc(hidden, alias = \"x\")]\npub fn foo() {}\n");

    let outcome = verify(tmp.path());

    assert!(outcome.has_errors(), "expected error for #[doc(hidden, alias = \"x\")]");
    let msg = outcome.findings()[0].to_string();
    assert!(msg.contains("foo"), "item name missing: {msg}");
    assert!(msg.contains("lib.rs"), "file path missing: {msg}");
    assert!(msg.contains("pub + #[doc(hidden)] is forbidden"), "reason missing: {msg}");
}

// (t) #[doc(alias = "x", hidden)] on a pub fn — flagged (order reversed)
#[test]
fn doc_alias_then_hidden_combined_is_flagged() {
    let tmp = TempDir::new().unwrap();
    setup_arch_rules(tmp.path(), "layer");
    write_src(tmp.path(), "layer/src/lib.rs", "#[doc(alias = \"x\", hidden)]\npub fn bar() {}\n");

    let outcome = verify(tmp.path());

    assert!(outcome.has_errors(), "expected error for #[doc(alias = \"x\", hidden)]");
    let msg = outcome.findings()[0].to_string();
    assert!(msg.contains("bar"), "item name missing: {msg}");
    assert!(msg.contains("pub + #[doc(hidden)] is forbidden"), "reason missing: {msg}");
}

// (u) #[doc(alias = "x")] on a pub fn — NOT flagged (no hidden, control case)
#[test]
fn doc_alias_only_not_flagged() {
    let tmp = TempDir::new().unwrap();
    setup_arch_rules(tmp.path(), "layer");
    write_src(tmp.path(), "layer/src/lib.rs", "#[doc(alias = \"x\")]\npub fn baz() {}\n");

    let outcome = verify(tmp.path());

    assert!(outcome.is_ok(), "expected ok for #[doc(alias = \"x\")]: {:?}", outcome.findings());
    assert!(outcome.findings().is_empty());
}

// (v) #[doc = "/// doc comment"] on a pub fn — NOT flagged (NameValue form, control case)
#[test]
fn doc_name_value_form_not_flagged() {
    let tmp = TempDir::new().unwrap();
    setup_arch_rules(tmp.path(), "layer");
    write_src(tmp.path(), "layer/src/lib.rs", "#[doc = \"/// doc comment\"]\npub fn qux() {}\n");

    let outcome = verify(tmp.path());

    assert!(
        outcome.is_ok(),
        "expected ok for #[doc = \"...\"] NameValue form: {:?}",
        outcome.findings()
    );
    assert!(outcome.findings().is_empty());
}

// (w) cfg_attr with combined doc(hidden, alias = "x") — flagged
#[test]
fn cfg_attr_doc_hidden_alias_combined_is_flagged() {
    let tmp = TempDir::new().unwrap();
    setup_arch_rules(tmp.path(), "layer");
    write_src(
        tmp.path(),
        "layer/src/lib.rs",
        "#[cfg_attr(not(test), doc(hidden, alias = \"x\"))]\npub fn quux() {}\n",
    );

    let outcome = verify(tmp.path());

    assert!(
        outcome.has_errors(),
        "expected error for cfg_attr(not(test), doc(hidden, alias = \"x\"))"
    );
    let msg = outcome.findings()[0].to_string();
    assert!(msg.contains("quux"), "item name missing: {msg}");
    assert!(msg.contains("pub + #[doc(hidden)] is forbidden"), "base reason missing: {msg}");
    assert!(
        msg.contains("cfg_attr(<pred>, doc(hidden)) forms are also forbidden"),
        "cfg_attr suffix missing: {msg}"
    );
}

// --- Inner #![doc(hidden)] on file-backed public modules ---

// (x) pub mod foo; + foo.rs with #![doc(hidden)] + pub fn bar() → flagged
#[test]
fn inner_doc_hidden_pub_file_backed_sibling_rs_is_flagged() {
    let tmp = TempDir::new().unwrap();
    setup_arch_rules(tmp.path(), "layer");
    write_src(tmp.path(), "layer/src/lib.rs", "pub mod foo;\n");
    write_src(tmp.path(), "layer/src/foo.rs", "#![doc(hidden)]\npub fn bar() {}\n");

    let outcome = verify(tmp.path());

    assert!(
        outcome.has_errors(),
        "expected error — foo.rs is pub-reachable with inner #![doc(hidden)]: {:?}",
        outcome.findings()
    );
    let msg = outcome.findings()[0].to_string();
    assert!(msg.contains("foo.rs"), "file path missing: {msg}");
    assert!(msg.contains("pub + #[doc(hidden)] is forbidden"), "reason missing: {msg}");
    assert!(
        msg.contains("inner #![doc(hidden)] on a public module file"),
        "inner-attr suffix missing: {msg}"
    );
}

// (y) pub mod foo; + foo/mod.rs with #![doc(hidden)] + pub fn bar() → flagged
#[test]
fn inner_doc_hidden_pub_file_backed_dir_mod_rs_is_flagged() {
    let tmp = TempDir::new().unwrap();
    setup_arch_rules(tmp.path(), "layer");
    write_src(tmp.path(), "layer/src/lib.rs", "pub mod foo;\n");
    write_src(tmp.path(), "layer/src/foo/mod.rs", "#![doc(hidden)]\npub fn bar() {}\n");

    let outcome = verify(tmp.path());

    assert!(
        outcome.has_errors(),
        "expected error — foo/mod.rs is pub-reachable with inner #![doc(hidden)]: {:?}",
        outcome.findings()
    );
    let msg = outcome.findings()[0].to_string();
    assert!(msg.contains("mod.rs"), "file path missing: {msg}");
    assert!(msg.contains("pub + #[doc(hidden)] is forbidden"), "reason missing: {msg}");
    assert!(
        msg.contains("inner #![doc(hidden)] on a public module file"),
        "inner-attr suffix missing: {msg}"
    );
}

// (z) pub mod foo; + foo.rs with #![doc(hidden, alias = "x")] → flagged (combined form)
#[test]
fn inner_doc_hidden_alias_combined_pub_file_backed_is_flagged() {
    let tmp = TempDir::new().unwrap();
    setup_arch_rules(tmp.path(), "layer");
    write_src(tmp.path(), "layer/src/lib.rs", "pub mod foo;\n");
    write_src(tmp.path(), "layer/src/foo.rs", "#![doc(hidden, alias = \"x\")]\npub fn bar() {}\n");

    let outcome = verify(tmp.path());

    assert!(
        outcome.has_errors(),
        "expected error — foo.rs with combined #![doc(hidden, alias = \"x\")]: {:?}",
        outcome.findings()
    );
    let msg = outcome.findings()[0].to_string();
    assert!(msg.contains("foo.rs"), "file path missing: {msg}");
    assert!(msg.contains("pub + #[doc(hidden)] is forbidden"), "reason missing: {msg}");
    assert!(
        msg.contains("inner #![doc(hidden)] on a public module file"),
        "inner-attr suffix missing: {msg}"
    );
}

// (aa) pub(crate) mod foo; + foo.rs with #![doc(hidden)] → not flagged (not pub-reachable)
#[test]
fn inner_doc_hidden_pub_crate_mod_not_flagged() {
    let tmp = TempDir::new().unwrap();
    setup_arch_rules(tmp.path(), "layer");
    write_src(tmp.path(), "layer/src/lib.rs", "pub(crate) mod foo;\n");
    write_src(tmp.path(), "layer/src/foo.rs", "#![doc(hidden)]\npub fn bar() {}\n");

    let outcome = verify(tmp.path());

    assert!(
        outcome.is_ok(),
        "expected ok — pub(crate) mod is not pub-reachable: {:?}",
        outcome.findings()
    );
    assert!(outcome.findings().is_empty());
}

// (ab) mod foo; (non-pub) + foo.rs with #![doc(hidden)] → not flagged
#[test]
fn inner_doc_hidden_non_pub_mod_not_flagged() {
    let tmp = TempDir::new().unwrap();
    setup_arch_rules(tmp.path(), "layer");
    write_src(tmp.path(), "layer/src/lib.rs", "mod foo;\n");
    write_src(tmp.path(), "layer/src/foo.rs", "#![doc(hidden)]\npub fn bar() {}\n");

    let outcome = verify(tmp.path());

    assert!(
        outcome.is_ok(),
        "expected ok — non-pub mod is not pub-reachable: {:?}",
        outcome.findings()
    );
    assert!(outcome.findings().is_empty());
}

// (ac) crate root (lib.rs) with #![doc(hidden)] → not flagged (no parent declaration)
#[test]
fn inner_doc_hidden_on_crate_root_not_flagged() {
    let tmp = TempDir::new().unwrap();
    setup_arch_rules(tmp.path(), "layer");
    write_src(tmp.path(), "layer/src/lib.rs", "#![doc(hidden)]\npub fn bar() {}\n");

    let outcome = verify(tmp.path());

    assert!(
        outcome.is_ok(),
        "expected ok — inner #![doc(hidden)] on crate root is out of scope: {:?}",
        outcome.findings()
    );
    assert!(outcome.findings().is_empty());
}

// (ad) #[cfg(test)] pub mod tests; + tests.rs with #![doc(hidden)] → not flagged (cfg-test path)
#[test]
fn inner_doc_hidden_cfg_test_pub_mod_not_flagged() {
    let tmp = TempDir::new().unwrap();
    setup_arch_rules(tmp.path(), "layer");
    write_src(tmp.path(), "layer/src/lib.rs", "#[cfg(test)] pub mod tests;\n");
    write_src(tmp.path(), "layer/src/tests.rs", "#![doc(hidden)]\npub fn helper() {}\n");

    let outcome = verify(tmp.path());

    assert!(
        outcome.is_ok(),
        "expected ok — tests.rs is cfg-test, inner #![doc(hidden)] not flagged: {:?}",
        outcome.findings()
    );
    assert!(outcome.findings().is_empty());
}
