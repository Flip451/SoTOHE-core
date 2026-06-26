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

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use domain::verify::{VerifyFinding, VerifyOutcome};

use super::syn_helpers::{
    collect_pub_reachable_files, has_cfg_test_attr, is_scan_crate_root, scan_rs_files,
};

/// Human-readable explanation appended to every violation finding.
const PROHIBITION_REASON: &str = "pub + #[doc(hidden)] is forbidden: rustdoc excludes this item from its paths output, \
     causing TDDD chain\u{2462} to fire DanglingId and block the track-active-gate commit gate";

/// Additional suffix appended when the violation is via `#[cfg_attr(<pred>, doc(hidden))]`.
const CFG_ATTR_PROHIBITION_SUFFIX: &str = "; cfg_attr(<pred>, doc(hidden)) forms are also forbidden because \
     they expand to #[doc(hidden)] in production builds";
/// Additional suffix appended when the violation is via an inner `#![doc(hidden)]`
/// on a file-backed public module, which is equivalent to placing `#[doc(hidden)]`
/// on the `mod` declaration.
const INNER_ATTR_PROHIBITION_SUFFIX: &str = "; inner #![doc(hidden)] on a public module file is equivalent to \
     #[doc(hidden)] on the `mod` declaration and is also excluded from rustdoc paths";

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

        // Pass 1b: collect files reachable via `pub mod` chains so that
        // inner `#![doc(hidden)]` on those files can be detected in the callback.
        let pub_reachable = collect_pub_reachable_files(&src_dir);
        let outcome = scan_rs_files(&src_dir, |file_path, ast| {
            let mut findings = check_file_items(file_path, &ast.items);
            check_inner_doc_hidden_file(file_path, ast, &pub_reachable, &src_dir, &mut findings);
            findings
        });

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
            if !is_test_scoped(&i.attrs) && is_pub_vis(&i.vis) {
                if has_doc_hidden(&i.attrs) {
                    findings.push(VerifyFinding::error(format!(
                        "{}: `use` item — {PROHIBITION_REASON}",
                        file_path.display(),
                    )));
                } else if has_cfg_attr_doc_hidden(&i.attrs) {
                    findings.push(VerifyFinding::error(format!(
                        "{}: `use` item — {PROHIBITION_REASON}{CFG_ATTR_PROHIBITION_SUFFIX}",
                        file_path.display(),
                    )));
                }
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

/// Check whether `file_path` has an inner `#![doc(hidden)]` attribute and is
/// reachable from the crate root through a chain of bare `pub mod` declarations.
///
/// Crate roots (`lib.rs` / `main.rs` directly in `scan_root`) are skipped: an
/// inner `#![doc(hidden)]` there is a deliberate "hide the whole crate" design
/// choice that is out of scope for this gate.
fn check_inner_doc_hidden_file(
    file_path: &Path,
    ast: &syn::File,
    pub_reachable_files: &HashSet<PathBuf>,
    scan_root: &Path,
    findings: &mut Vec<VerifyFinding>,
) {
    // Crate roots have no parent `mod` declaration; skip.
    if is_scan_crate_root(scan_root, file_path) {
        return;
    }
    // Only flag files reachable via a bare `pub mod` chain.
    if !pub_reachable_files.contains(file_path) {
        return;
    }
    if has_inner_doc_hidden(&ast.attrs) {
        findings.push(VerifyFinding::error(format!(
            "{}: inner `#![doc(hidden)]` on public module file — \
             {PROHIBITION_REASON}{INNER_ATTR_PROHIBITION_SUFFIX}",
            file_path.display(),
        )));
    } else if has_inner_cfg_attr_doc_hidden(&ast.attrs) {
        findings.push(VerifyFinding::error(format!(
            "{}: inner `#![doc(hidden)]` on public module file — \
             {PROHIBITION_REASON}{CFG_ATTR_PROHIBITION_SUFFIX}{INNER_ATTR_PROHIBITION_SUFFIX}",
            file_path.display(),
        )));
    }
}

/// Returns `true` if `attrs` contains an inner `#![doc(hidden)]` attribute,
/// including combined forms like `#![doc(hidden, alias = "x")]`.
fn has_inner_doc_hidden(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !matches!(attr.style, syn::AttrStyle::Inner(_)) {
            return false;
        }
        if !attr.path().is_ident("doc") {
            return false;
        }
        let parser = syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated;
        attr.parse_args_with(parser).is_ok_and(|metas| {
            metas.iter().any(|m| matches!(m, syn::Meta::Path(p) if p.is_ident("hidden")))
        })
    })
}

/// Returns `true` if `attrs` contains an inner `#![cfg_attr(<pred>, doc(hidden))]`
/// attribute (or a nested `cfg_attr` that transitively applies `doc(hidden)`).
fn has_inner_cfg_attr_doc_hidden(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        matches!(attr.style, syn::AttrStyle::Inner(_))
            && attr.path().is_ident("cfg_attr")
            && cfg_attr_applies_doc_hidden(attr)
    })
}

/// Returns `true` if the item carries `#[cfg(test)]` or `#[test]` and should
/// therefore be excluded from production checks.
fn is_test_scoped(attrs: &[syn::Attribute]) -> bool {
    has_cfg_test_attr(attrs) || has_test_attr(attrs)
}

/// Returns `true` if `attrs` contains a bare `#[test]` attribute.
fn has_test_attr(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident("test"))
}

/// Returns `true` if `attrs` contains a direct `#[doc(hidden)]` attribute,
/// including combined forms like `#[doc(hidden, alias = "x")]`.
fn has_doc_hidden(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if !attr.path().is_ident("doc") {
            return false;
        }
        // Parse the inner tokens of `doc(...)` as comma-separated `Meta` items.
        // This handles single-item `#[doc(hidden)]` as well as combined forms
        // like `#[doc(hidden, alias = "x")]` or `#[doc(alias = "x", hidden)]`.
        // `#[doc = "..."]` (NameValue form) will fail to parse here and
        // return false — which is correct since it is not `doc(hidden)`.
        let parser = syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated;
        attr.parse_args_with(parser).is_ok_and(|metas| {
            metas.iter().any(|m| matches!(m, syn::Meta::Path(p) if p.is_ident("hidden")))
        })
    })
}

/// Returns `true` if `attrs` contains a `#[cfg_attr(<pred>, ..., doc(hidden), ...)]`
/// form — including nested `cfg_attr` — regardless of the cfg predicate.
///
/// The ADR forbids `#[doc(hidden)]` outright; `cfg_attr` is not an escape hatch
/// because the attribute expands to `#[doc(hidden)]` in production builds.
fn has_cfg_attr_doc_hidden(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident("cfg_attr") && cfg_attr_applies_doc_hidden(attr))
}

/// Returns `true` if the `#[cfg_attr(...)]` attribute's conditional argument
/// list contains `doc(hidden)` (or a nested `cfg_attr` that transitively does).
///
/// `cfg_attr(pred, attr1, attr2, ...)` — the first token is the cfg predicate
/// (skipped) and the rest are the conditional attributes we inspect.
fn cfg_attr_applies_doc_hidden(attr: &syn::Attribute) -> bool {
    let parser = syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated;
    let Ok(metas) = attr.parse_args_with(parser) else {
        return false;
    };
    // Skip element [0] (the cfg predicate); check the conditional attrs.
    metas.iter().skip(1).any(conditional_meta_is_doc_hidden)
}

/// Returns `true` if a single `Meta` (from the conditional portion of a
/// `cfg_attr` argument list) is `doc(hidden)` or a nested `cfg_attr` that
/// transitively applies `doc(hidden)`.
fn conditional_meta_is_doc_hidden(meta: &syn::Meta) -> bool {
    let syn::Meta::List(list) = meta else {
        return false;
    };
    if list.path.is_ident("doc") {
        // Parse inner tokens as comma-separated Metas and check for `hidden`.
        // Handles `doc(hidden)` as well as combined forms like `doc(hidden, alias = "x")`.
        use syn::parse::Parser;
        let parser = syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated;
        return parser.parse2(list.tokens.clone()).is_ok_and(|metas| {
            metas.iter().any(|m| matches!(m, syn::Meta::Path(p) if p.is_ident("hidden")))
        });
    }
    if list.path.is_ident("cfg_attr") {
        // Nested cfg_attr — recurse into its conditional argument list.
        use syn::parse::Parser;
        let parser = syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated;
        return match parser.parse2(list.tokens.clone()) {
            Ok(inner) => inner.iter().skip(1).any(conditional_meta_is_doc_hidden),
            Err(_) => false,
        };
    }
    false
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
    if !is_public {
        return;
    }
    if has_doc_hidden(attrs) {
        findings.push(VerifyFinding::error(format!(
            "{}: `{name}` — {PROHIBITION_REASON}",
            file_path.display(),
        )));
    } else if has_cfg_attr_doc_hidden(attrs) {
        findings.push(VerifyFinding::error(format!(
            "{}: `{name}` — {PROHIBITION_REASON}{CFG_ATTR_PROHIBITION_SUFFIX}",
            file_path.display(),
        )));
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
#[path = "doc_hidden_tests.rs"]
mod tests;
