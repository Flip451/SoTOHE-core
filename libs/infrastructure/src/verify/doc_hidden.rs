//! Verify that no `pub` item is annotated with `#[doc(hidden)]` in production
//! code across all architecture layers listed in `architecture-rules.json`.
//!
//! `pub + #[doc(hidden)]` causes rustdoc to exclude the item from its paths
//! output. TDDD chain③ (`calc-impl-catalog`) uses that paths output to build
//! the implementation catalogue; an excluded item triggers a `DanglingId`
//! finding that blocks the `track-active-gate` commit gate.
//!
//! This gate detects the root cause directly so that the error message names
//! the attribute combination and explains the chain, rather than surfacing as
//! an opaque `DanglingId`.

use std::path::{Path, PathBuf};

use domain::verify::{VerifyFinding, VerifyOutcome};

use super::syn_helpers::{has_cfg_test_attr, scan_rs_files};

/// Human-readable explanation appended to every violation finding.
const PROHIBITION_REASON: &str = "pub + #[doc(hidden)] is forbidden: rustdoc excludes this item from its paths output, \
     causing TDDD chain\u{2462} to fire DanglingId and block the track-active-gate commit gate";

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Scan all layers listed in `architecture-rules.json` for `pub + #[doc(hidden)]`
/// violations in production code. Returns findings for each violation.
///
/// Test-scoped items (inside `#[cfg(test)]` blocks or carrying `#[test]`) are
/// excluded at the syn AST level (CN-04, IN-06, AC-04).
///
/// # Errors
///
/// Returns an error finding when `architecture-rules.json` cannot be read or
/// parsed.
pub fn verify(root: &Path) -> VerifyOutcome {
    let rules = match crate::arch::load_rules(root) {
        Ok(r) => r,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "verify doc-hidden: failed to load architecture-rules.json: {e}"
            ))]);
        }
    };

    let trusted_root =
        super::path_safety::lexical_normalize(&super::trusted_root::absolutize(root));
    let mut all_findings: Vec<VerifyFinding> = Vec::new();

    for layer in rules.layers() {
        let src_dir = match resolve_layer_src_dir(&trusted_root, &layer.path) {
            Ok(path) => path,
            Err(finding) => {
                all_findings.push(finding);
                continue;
            }
        };
        match crate::track::symlink_guard::reject_symlinks_below(&src_dir, &trusted_root) {
            Ok(true) => {}
            Ok(false) => continue, // Layer src not present yet — skip silently.
            Err(e) => {
                all_findings.push(VerifyFinding::error(format!(
                    "verify doc-hidden: refusing to scan layer path '{}': {e}",
                    layer.path
                )));
                continue;
            }
        }

        let outcome =
            scan_rs_files(&src_dir, |file_path, ast| check_file_items(file_path, &ast.items));

        for finding in outcome.findings() {
            all_findings.push(finding.clone());
        }
    }

    VerifyOutcome::from_findings(all_findings)
}

fn resolve_layer_src_dir(trusted_root: &Path, layer_path: &str) -> Result<PathBuf, VerifyFinding> {
    let layer_root = Path::new(layer_path);
    let absolute_layer_root = if layer_root.is_absolute() {
        layer_root.to_path_buf()
    } else {
        trusted_root.join(layer_root)
    };
    let src_dir = super::path_safety::lexical_normalize(&absolute_layer_root.join("src"));

    if src_dir.starts_with(trusted_root) {
        return Ok(src_dir);
    }

    Err(VerifyFinding::error(format!(
        "verify doc-hidden: architecture-rules.json layer path '{}' resolves outside trusted root '{}'",
        layer_path,
        trusted_root.display()
    )))
}

// ---------------------------------------------------------------------------
// Item-level detection helpers
// ---------------------------------------------------------------------------

/// Check all top-level items in a parsed file for `pub + #[doc(hidden)]`
/// violations. Recurses into non-test `mod` blocks.
fn check_file_items(file_path: &Path, items: &[syn::Item]) -> Vec<VerifyFinding> {
    let mut findings = Vec::new();
    check_items(items, file_path, &mut findings);
    findings
}

/// Recursive item walker — called for top-level items and for each non-test
/// inline `mod` block encountered.
fn check_items(items: &[syn::Item], file_path: &Path, findings: &mut Vec<VerifyFinding>) {
    for item in items {
        check_item(item, file_path, findings);
    }
}

fn check_item(item: &syn::Item, file_path: &Path, findings: &mut Vec<VerifyFinding>) {
    match item {
        syn::Item::Fn(i) => {
            if !is_test_scoped(&i.attrs) {
                report_if_pub_doc_hidden(
                    &i.vis,
                    &i.attrs,
                    &i.sig.ident.to_string(),
                    file_path,
                    findings,
                );
            }
        }
        syn::Item::Struct(i) => {
            if !is_test_scoped(&i.attrs) {
                let is_public = is_pub_vis(&i.vis);
                report_if_pub_doc_hidden(
                    &i.vis,
                    &i.attrs,
                    &i.ident.to_string(),
                    file_path,
                    findings,
                );
                if is_public {
                    check_fields_with_explicit_visibility(
                        &i.fields,
                        &i.ident.to_string(),
                        file_path,
                        findings,
                    );
                }
            }
        }
        syn::Item::Enum(i) => {
            if !is_test_scoped(&i.attrs) {
                let is_public = is_pub_vis(&i.vis);
                report_if_pub_doc_hidden(
                    &i.vis,
                    &i.attrs,
                    &i.ident.to_string(),
                    file_path,
                    findings,
                );
                if is_public {
                    check_public_enum_variants(
                        &i.ident.to_string(),
                        &i.variants,
                        file_path,
                        findings,
                    );
                }
            }
        }
        syn::Item::Trait(i) => {
            if !is_test_scoped(&i.attrs) {
                let is_public = is_pub_vis(&i.vis);
                report_if_pub_doc_hidden(
                    &i.vis,
                    &i.attrs,
                    &i.ident.to_string(),
                    file_path,
                    findings,
                );
                if is_public {
                    check_public_trait_items(&i.ident.to_string(), &i.items, file_path, findings);
                }
            }
        }
        syn::Item::Type(i) => {
            if !is_test_scoped(&i.attrs) {
                report_if_pub_doc_hidden(
                    &i.vis,
                    &i.attrs,
                    &i.ident.to_string(),
                    file_path,
                    findings,
                );
            }
        }
        syn::Item::Const(i) => {
            if !is_test_scoped(&i.attrs) {
                report_if_pub_doc_hidden(
                    &i.vis,
                    &i.attrs,
                    &i.ident.to_string(),
                    file_path,
                    findings,
                );
            }
        }
        syn::Item::Static(i) => {
            if !is_test_scoped(&i.attrs) {
                report_if_pub_doc_hidden(
                    &i.vis,
                    &i.attrs,
                    &i.ident.to_string(),
                    file_path,
                    findings,
                );
            }
        }
        syn::Item::TraitAlias(i) => {
            if !is_test_scoped(&i.attrs) {
                report_if_pub_doc_hidden(
                    &i.vis,
                    &i.attrs,
                    &i.ident.to_string(),
                    file_path,
                    findings,
                );
            }
        }
        syn::Item::ExternCrate(i) => {
            if !is_test_scoped(&i.attrs) {
                report_if_pub_doc_hidden(
                    &i.vis,
                    &i.attrs,
                    &i.ident.to_string(),
                    file_path,
                    findings,
                );
            }
        }
        syn::Item::ForeignMod(i) => {
            if !is_test_scoped(&i.attrs) {
                check_foreign_items(&i.items, file_path, findings);
            }
        }
        syn::Item::Impl(i) => {
            if !is_test_scoped(&i.attrs) {
                check_impl_items(&i.items, file_path, findings);
            }
        }
        syn::Item::Mod(i) => {
            // If the `mod` itself is test-scoped, skip it and all its children.
            if !is_test_scoped(&i.attrs) {
                // Check the mod declaration itself.
                report_if_pub_doc_hidden(
                    &i.vis,
                    &i.attrs,
                    &i.ident.to_string(),
                    file_path,
                    findings,
                );
                // Recurse into inline module contents (file-backed modules are
                // discovered separately during directory traversal).
                if let Some((_, items)) = &i.content {
                    check_items(items, file_path, findings);
                }
            }
        }
        // `use` items rarely carry `#[doc(hidden)]`, but include them for completeness.
        syn::Item::Use(i) => {
            if !is_test_scoped(&i.attrs) && is_pub_vis(&i.vis) && has_doc_hidden(&i.attrs) {
                findings.push(VerifyFinding::error(format!(
                    "{}: `use` item — {PROHIBITION_REASON}",
                    file_path.display(),
                )));
            }
        }
        syn::Item::Union(i) => {
            if !is_test_scoped(&i.attrs) {
                let is_public = is_pub_vis(&i.vis);
                report_if_pub_doc_hidden(
                    &i.vis,
                    &i.attrs,
                    &i.ident.to_string(),
                    file_path,
                    findings,
                );
                if is_public {
                    check_named_fields_with_explicit_visibility(
                        &i.fields.named,
                        &i.ident.to_string(),
                        file_path,
                        findings,
                    );
                }
            }
        }
        // Macros and unknown items have no relevant `pub` visibility to check
        // at the block level.
        _ => {}
    }
}

fn check_fields_with_explicit_visibility(
    fields: &syn::Fields,
    parent_name: &str,
    file_path: &Path,
    findings: &mut Vec<VerifyFinding>,
) {
    for (index, field) in fields.iter().enumerate() {
        if is_test_scoped(&field.attrs) {
            continue;
        }
        let name = field_name(parent_name, index, field);
        report_if_pub_doc_hidden(&field.vis, &field.attrs, &name, file_path, findings);
    }
}

fn check_named_fields_with_explicit_visibility(
    fields: &syn::punctuated::Punctuated<syn::Field, syn::Token![,]>,
    parent_name: &str,
    file_path: &Path,
    findings: &mut Vec<VerifyFinding>,
) {
    for (index, field) in fields.iter().enumerate() {
        if is_test_scoped(&field.attrs) {
            continue;
        }
        let name = field_name(parent_name, index, field);
        report_if_pub_doc_hidden(&field.vis, &field.attrs, &name, file_path, findings);
    }
}

fn check_inherited_public_fields(
    fields: &syn::Fields,
    parent_name: &str,
    file_path: &Path,
    findings: &mut Vec<VerifyFinding>,
) {
    for (index, field) in fields.iter().enumerate() {
        if is_test_scoped(&field.attrs) {
            continue;
        }
        let name = field_name(parent_name, index, field);
        report_if_public_doc_hidden(true, &field.attrs, &name, file_path, findings);
    }
}

fn field_name(parent_name: &str, index: usize, field: &syn::Field) -> String {
    field
        .ident
        .as_ref()
        .map(|ident| format!("{parent_name}::{ident}"))
        .unwrap_or_else(|| format!("{parent_name}::{index}"))
}

fn check_public_enum_variants(
    enum_name: &str,
    variants: &syn::punctuated::Punctuated<syn::Variant, syn::Token![,]>,
    file_path: &Path,
    findings: &mut Vec<VerifyFinding>,
) {
    for variant in variants {
        if is_test_scoped(&variant.attrs) {
            continue;
        }
        let variant_name = format!("{enum_name}::{}", variant.ident);
        report_if_public_doc_hidden(true, &variant.attrs, &variant_name, file_path, findings);
        check_inherited_public_fields(&variant.fields, &variant_name, file_path, findings);
    }
}

fn check_public_trait_items(
    trait_name: &str,
    items: &[syn::TraitItem],
    file_path: &Path,
    findings: &mut Vec<VerifyFinding>,
) {
    for item in items {
        match item {
            syn::TraitItem::Const(i) => {
                if !is_test_scoped(&i.attrs) {
                    let name = format!("{trait_name}::{}", i.ident);
                    report_if_public_doc_hidden(true, &i.attrs, &name, file_path, findings);
                }
            }
            syn::TraitItem::Fn(i) => {
                if !is_test_scoped(&i.attrs) {
                    let name = format!("{trait_name}::{}", i.sig.ident);
                    report_if_public_doc_hidden(true, &i.attrs, &name, file_path, findings);
                }
            }
            syn::TraitItem::Type(i) => {
                if !is_test_scoped(&i.attrs) {
                    let name = format!("{trait_name}::{}", i.ident);
                    report_if_public_doc_hidden(true, &i.attrs, &name, file_path, findings);
                }
            }
            syn::TraitItem::Macro(i) => {
                if !is_test_scoped(&i.attrs) {
                    let name = format!("{trait_name}::macro");
                    report_if_public_doc_hidden(true, &i.attrs, &name, file_path, findings);
                }
            }
            _ => {}
        }
    }
}

fn check_impl_items(items: &[syn::ImplItem], file_path: &Path, findings: &mut Vec<VerifyFinding>) {
    for item in items {
        match item {
            syn::ImplItem::Const(i) => {
                if !is_test_scoped(&i.attrs) {
                    report_if_pub_doc_hidden(
                        &i.vis,
                        &i.attrs,
                        &i.ident.to_string(),
                        file_path,
                        findings,
                    );
                }
            }
            syn::ImplItem::Fn(i) => {
                if !is_test_scoped(&i.attrs) {
                    report_if_pub_doc_hidden(
                        &i.vis,
                        &i.attrs,
                        &i.sig.ident.to_string(),
                        file_path,
                        findings,
                    );
                }
            }
            syn::ImplItem::Type(i) => {
                if !is_test_scoped(&i.attrs) {
                    report_if_pub_doc_hidden(
                        &i.vis,
                        &i.attrs,
                        &i.ident.to_string(),
                        file_path,
                        findings,
                    );
                }
            }
            _ => {}
        }
    }
}

fn check_foreign_items(
    items: &[syn::ForeignItem],
    file_path: &Path,
    findings: &mut Vec<VerifyFinding>,
) {
    for item in items {
        match item {
            syn::ForeignItem::Fn(i) => {
                if !is_test_scoped(&i.attrs) {
                    report_if_pub_doc_hidden(
                        &i.vis,
                        &i.attrs,
                        &i.sig.ident.to_string(),
                        file_path,
                        findings,
                    );
                }
            }
            syn::ForeignItem::Static(i) => {
                if !is_test_scoped(&i.attrs) {
                    report_if_pub_doc_hidden(
                        &i.vis,
                        &i.attrs,
                        &i.ident.to_string(),
                        file_path,
                        findings,
                    );
                }
            }
            syn::ForeignItem::Type(i) => {
                if !is_test_scoped(&i.attrs) {
                    report_if_pub_doc_hidden(
                        &i.vis,
                        &i.attrs,
                        &i.ident.to_string(),
                        file_path,
                        findings,
                    );
                }
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Attribute predicate helpers
// ---------------------------------------------------------------------------

/// Returns `true` if the item carries `#[cfg(test)]` or `#[test]` and should
/// therefore be excluded from production checks.
fn is_test_scoped(attrs: &[syn::Attribute]) -> bool {
    has_cfg_test_attr(attrs) || has_test_attr(attrs)
}

/// Returns `true` if `attrs` contains a bare `#[test]` attribute.
fn has_test_attr(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident("test"))
}

/// Returns `true` if `attrs` contains `#[doc(hidden)]`.
fn has_doc_hidden(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("doc") {
            return false;
        }
        // `#[doc(hidden)]` has the form doc(hidden) where the inner token is
        // the path `hidden`. Mirrors the `has_cfg_test_attr` pattern.
        attr.parse_args::<syn::Path>().is_ok_and(|path| path.is_ident("hidden"))
    })
}

/// Returns `true` if `vis` is bare `pub` (not `pub(crate)`, `pub(super)`,
/// or private). Only bare `pub` is tracked by rustdoc's paths output.
fn is_pub_vis(vis: &syn::Visibility) -> bool {
    matches!(vis, syn::Visibility::Public(_))
}

/// Emit a finding if the item has bare `pub` visibility and `#[doc(hidden)]`.
fn report_if_pub_doc_hidden(
    vis: &syn::Visibility,
    attrs: &[syn::Attribute],
    name: &str,
    file_path: &Path,
    findings: &mut Vec<VerifyFinding>,
) {
    report_if_public_doc_hidden(is_pub_vis(vis), attrs, name, file_path, findings);
}

fn report_if_public_doc_hidden(
    is_public: bool,
    attrs: &[syn::Attribute],
    name: &str,
    file_path: &Path,
    findings: &mut Vec<VerifyFinding>,
) {
    if is_public && has_doc_hidden(attrs) {
        findings.push(VerifyFinding::error(format!(
            "{}: `{name}` — {PROHIBITION_REASON}",
            file_path.display(),
        )));
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
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
        write_src(
            tmp.path(),
            "layer/src/lib.rs",
            "#[doc(hidden)]\npub(crate) fn restricted() {}\n",
        );

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
}
