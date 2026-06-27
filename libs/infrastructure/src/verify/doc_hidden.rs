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
        let attr_line = attr.pound_token.spans.first().map(|s| s.start().line).unwrap_or(0);
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
// Unit tests (split out to keep this module under the line guideline)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "doc_hidden_tests.rs"]
mod tests;
