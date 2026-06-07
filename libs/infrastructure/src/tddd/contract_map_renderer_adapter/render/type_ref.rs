//! syn-based type-expression extraction and TypeRef/trait_ref resolution.
//!
//! All items are `pub(super)` — implementation details of the render module.

use std::collections::BTreeMap;

use super::node_index::{NodeIndex, strip_generics};

// ---------------------------------------------------------------------------
// syn-based type-expression extraction
// ---------------------------------------------------------------------------

/// Collect all leaf type-path names from a `syn::Type` AST.
///
/// Recurses into `Type::Reference` (`&T`/`&mut T`), `Type::Slice` (`[T]`),
/// `Type::Array` (`[T; N]`), `Type::Tuple` (`(A, B, …)`), `Type::Group`/`Type::Paren`,
/// and every generic argument of `Type::Path` (covers `Result<T, E>`, `Vec<T>`,
/// `Option<T>`, `Box<T>`, `Arc<T>`, nested generics).  For each `Type::Path` the last
/// segment name (the type's short name) is pushed as a lookup candidate alongside the
/// full dot-joined path — both forms are tried so that `NodeIndex::resolve` can match
/// either a qualified (`"domain::MyType"`) or bare (`"MyType"`) catalogue key.
///
/// `ImplTrait`, `TraitObject`, and `Infer`/`Never`/`Verbatim` produce no output
/// (they cannot be catalogue types).
fn collect_type_names_from_syn(ty: &syn::Type, out: &mut Vec<String>) {
    match ty {
        syn::Type::Path(tp) => {
            // Skip UFCS projections such as `<T as Trait>::Assoc` or `Self::Output`.
            // When `qself` is present the leading path segment is not a type name at
            // the catalogue level; reducing it to just the last segment (e.g. `Assoc`)
            // would create bogus edges to any unrelated declared type of the same name.
            // Catalogue TypeRefs should never use UFCS form, so safe to skip entirely.
            if tp.qself.is_some() {
                return;
            }

            // Push the full path as a `"::"` joined string so that qualified lookups
            // (`"domain::MyType"`) have a chance to match.
            let full_path: String = tp
                .path
                .segments
                .iter()
                .map(|seg| seg.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");
            out.push(full_path);

            // Recurse into every generic argument (covers `Result<T, E>`, `Vec<T>`, …).
            for seg in &tp.path.segments {
                if let syn::PathArguments::AngleBracketed(ref args) = seg.arguments {
                    for arg in &args.args {
                        match arg {
                            syn::GenericArgument::Type(inner_ty) => {
                                collect_type_names_from_syn(inner_ty, out);
                            }
                            // Associated-type bindings: `Iterator<Item = Foo>`.
                            // The bound type `Foo` must be extracted so edges to
                            // declared catalogue types inside these bindings are emitted.
                            syn::GenericArgument::AssocType(assoc) => {
                                collect_type_names_from_syn(&assoc.ty, out);
                            }
                            // Lifetimes, const generics, assoc const — not type paths.
                            _ => {}
                        }
                    }
                } else if let syn::PathArguments::Parenthesized(ref args) = seg.arguments {
                    // Fn trait `Fn(A, B) -> C`
                    for input in &args.inputs {
                        collect_type_names_from_syn(input, out);
                    }
                    if let syn::ReturnType::Type(_, ref ret) = args.output {
                        collect_type_names_from_syn(ret, out);
                    }
                }
            }
        }
        syn::Type::Reference(tr) => {
            collect_type_names_from_syn(&tr.elem, out);
        }
        syn::Type::Slice(ts) => {
            collect_type_names_from_syn(&ts.elem, out);
        }
        syn::Type::Array(ta) => {
            collect_type_names_from_syn(&ta.elem, out);
        }
        syn::Type::Tuple(tt) => {
            for elem in &tt.elems {
                collect_type_names_from_syn(elem, out);
            }
        }
        syn::Type::Paren(tp) => {
            collect_type_names_from_syn(&tp.elem, out);
        }
        syn::Type::Group(tg) => {
            collect_type_names_from_syn(&tg.elem, out);
        }
        syn::Type::Ptr(ptr) => {
            collect_type_names_from_syn(&ptr.elem, out);
        }
        // ImplTrait, TraitObject, BareFn, Infer, Never, Verbatim, Macro — not catalogue types.
        _ => {}
    }
}

/// Resolve a `TypeRef` string to **all** rendered mermaid node IDs that it references.
///
/// Uses `syn::parse_str::<syn::Type>` to parse the full type expression (handling
/// `&T`, `&mut T`, `Result<T, E>`, `Vec<T>`, `Option<T>`, `Box<T>`, `Arc<T>`,
/// `[T]`, `(A, B)`, nested generics, etc.), then walks the resulting AST to collect
/// every referenced type-path name.  Each candidate is resolved against `node_index`;
/// only names that map to a **declared** catalogue node produce an entry in the
/// returned `Vec`.  Undeclared/primitive/generic/external types are silently skipped.
///
/// `self_node_id` — when `Some`, the literal name `"Self"` extracted by the syn walk
/// is substituted with the provided node_id directly, without going through
/// `NodeIndex::resolve`.  This handles nested `Self` occurrences such as
/// `Option<Self>` or `Result<Self, E>` in method signatures; `NodeIndex` never holds a
/// `"Self"` key (it indexes declared types by their bare names), so without
/// substitution the edge would be silently dropped.  Pass `None` for field / alias /
/// function-level TypeRefs where `Self` has no meaningful resolution.
///
/// Returns an empty `Vec` (never panics) when:
/// - `syn::parse_str` fails on a malformed TypeRef string (graceful fallback).
/// - No inner type resolves to a declared catalogue node.
///
/// This upholds ADR 2026-04-17-1528 §D1: edges only between **declared** types.
/// `current_crate` is forwarded to `NodeIndex::resolve` as a tie-breaker for bare
/// TypeRef names that appear in multiple crates.
pub(crate) fn resolve_type_ref_node_ids(
    type_ref_str: &str,
    node_index: &NodeIndex,
    current_crate: &str,
    self_node_id: Option<&str>,
) -> Vec<String> {
    // Parse with syn; fall back silently on malformed input.
    let syn_type = match syn::parse_str::<syn::Type>(type_ref_str) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };

    let mut candidates: Vec<String> = Vec::new();
    collect_type_names_from_syn(&syn_type, &mut candidates);

    // Deduplicate: the same path may appear multiple times (e.g. nested).
    candidates.sort_unstable();
    candidates.dedup();

    // Resolve each candidate against the node index; keep only declared types.
    // The literal name "Self" is substituted directly with `self_node_id` when
    // provided — `NodeIndex` does not hold a "Self" key (OS-04 / correctness).
    let mut resolved: Vec<String> = Vec::new();
    for candidate in &candidates {
        if candidate == "Self" {
            if let Some(id) = self_node_id {
                let id_str = id.to_string();
                if !resolved.contains(&id_str) {
                    resolved.push(id_str);
                }
            }
            // If self_node_id is None, "Self" has no resolution — silent skip.
            continue;
        }
        if let Some(node_id) = node_index.resolve(candidate, current_crate) {
            let node_id_str = node_id.to_string();
            if !resolved.contains(&node_id_str) {
                resolved.push(node_id_str);
            }
        }
    }
    resolved
}

/// Resolve a `trait_ref` string to the rendered mermaid subgraph ID for that trait.
///
/// Two forms of workspace-internal trait refs are supported (per `TraitImplDeclV2`
/// schema — ADR `2026-05-20-0048` D2):
///
/// - **Bare name** (e.g., `"MyTrait"` or `"MyTrait<Foo>"`): self-crate trait.
///   The `TraitImplDeclV2` schema specifies that bare names denote traits in the same
///   crate as the `for_type`. Lookup is scoped to `(current_crate, bare_name)`; if not
///   found (the trait is not in the current crate's catalogue), returns `None` (silent
///   skip — avoids wiring to a same-named trait in a different catalogue crate).
///
/// - **Qualified cross-crate path** (e.g., `"domain::tddd::ContractMapRenderer"`): a
///   workspace-internal trait in another catalogue crate. Resolved by extracting the
///   first segment as the crate name and the last segment as the trait name, then
///   looking up `(crate, trait_name)` in the trait index. If not found, silent skip
///   (workspace-external; std / third-party; CN-10 / AC-06).
///
/// Returns `None` (silent skip) for workspace-external trait refs not present in any
/// provided catalogue.
pub(crate) fn resolve_trait_subgraph<'a>(
    trait_ref_str: &str,
    current_crate: &str,
    trait_index: &'a BTreeMap<(String, String), String>,
) -> Option<&'a str> {
    // Strip generic suffix first so that `"MyTrait<crate::Foo>"` is treated as
    // `"MyTrait"` (bare) rather than being classified as qualified because the
    // generic argument contains `::`.
    let bare_name = strip_generics(trait_ref_str);
    if bare_name.is_empty() {
        return None;
    }
    if bare_name.contains("::") {
        // Qualified path (e.g. "domain::tddd::ContractMapRenderer"):
        // Extract crate (first segment) and trait name (last segment).
        // Look up (crate, trait_name) in the index. Returns None (silent skip)
        // if the pair is not in the index — workspace-external trait (CN-10 / AC-06).
        let mut iter = bare_name.splitn(2, "::");
        if let (Some(crate_seg), Some(rest)) = (iter.next(), iter.next()) {
            let trait_name = rest.rsplit("::").next().unwrap_or(rest);
            let key = (crate_seg.to_string(), trait_name.to_string());
            return trait_index.get(&key).map(|s| s.as_str());
        }
        return None;
    }
    // Bare name: self-crate only (TraitImplDeclV2 schema: bare trait_ref = self-crate trait).
    // Scoped to (current_crate, bare_name) — prevents incorrect wiring when two catalogues
    // contain a trait with the same short name.
    let key = (current_crate.to_string(), bare_name.to_string());
    trait_index.get(&key).map(|s| s.as_str())
}
