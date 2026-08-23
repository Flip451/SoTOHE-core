//! syn-based type-expression extraction and TypeRef/trait_ref resolution.
//!
//! All items are `pub(super)` — implementation details of the render module.

use super::node_index::NodeIndex;

// ---------------------------------------------------------------------------
// syn-based type-expression extraction
// ---------------------------------------------------------------------------

/// Collected candidates from a `syn::Type` AST.
///
/// Type candidates continue through the type-only [`NodeIndex`]. Trait candidates
/// originate solely in `dyn Trait` bounds and resolve through the separate trait index.
#[derive(Default)]
struct TypeRefCandidates {
    type_names: Vec<TypeCandidate>,
    trait_names: Vec<String>,
}

/// A type-path candidate together with the syntactic information needed to
/// distinguish a relative generic projection from an absolute crate path.
#[derive(Ord, PartialOrd, Eq, PartialEq)]
struct TypeCandidate {
    name: String,
    is_absolute: bool,
}

/// Collect all leaf type-path names and `dyn Trait` bounds from a `syn::Type` AST.
///
/// Recurses into `Type::Reference` (`&T`/`&mut T`), `Type::Slice` (`[T]`),
/// `Type::Array` (`[T; N]`), `Type::Tuple` (`(A, B, …)`), `Type::Group`/`Type::Paren`,
/// and every generic argument of `Type::Path` (covers `Result<T, E>`, `Vec<T>`,
/// `Option<T>`, `Box<T>`, `Arc<T>`, nested generics).  For each `Type::Path` the last
/// segment name (the type's short name) is pushed as a lookup candidate alongside the
/// full dot-joined path — both forms are tried so that `NodeIndex::resolve` can match
/// either a qualified (`"domain::MyType"`) or bare (`"MyType"`) catalogue key.
///
/// `ImplTrait` and `Infer`/`Never`/`Verbatim` produce no output (they cannot be
/// catalogue types). `TraitObject` contributes its bound paths only to the trait
/// candidate stream; generic arguments and associated-type values still recurse as
/// ordinary type candidates.
fn collect_type_names_from_syn(ty: &syn::Type, candidates: &mut TypeRefCandidates) {
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

            collect_path_as_type_candidate(&tp.path, candidates);
        }
        syn::Type::Reference(tr) => {
            collect_type_names_from_syn(&tr.elem, candidates);
        }
        syn::Type::Slice(ts) => {
            collect_type_names_from_syn(&ts.elem, candidates);
        }
        syn::Type::Array(ta) => {
            collect_type_names_from_syn(&ta.elem, candidates);
        }
        syn::Type::Tuple(tt) => {
            for elem in &tt.elems {
                collect_type_names_from_syn(elem, candidates);
            }
        }
        syn::Type::Paren(tp) => {
            collect_type_names_from_syn(&tp.elem, candidates);
        }
        syn::Type::Group(tg) => {
            collect_type_names_from_syn(&tg.elem, candidates);
        }
        syn::Type::Ptr(ptr) => {
            collect_type_names_from_syn(&ptr.elem, candidates);
        }
        syn::Type::TraitObject(trait_object) => {
            for bound in &trait_object.bounds {
                if let syn::TypeParamBound::Trait(trait_bound) = bound {
                    candidates.trait_names.push(path_as_string(&trait_bound.path));
                    collect_path_generic_type_candidates(&trait_bound.path, candidates);
                }
            }
        }
        // ImplTrait, BareFn, Infer, Never, Verbatim, Macro — not catalogue types.
        _ => {}
    }
}

/// Add one path to the ordinary type candidate stream and recurse into its arguments.
fn collect_path_as_type_candidate(path: &syn::Path, candidates: &mut TypeRefCandidates) {
    candidates.type_names.push(TypeCandidate {
        name: path_as_string(path),
        is_absolute: path.leading_colon.is_some(),
    });
    collect_path_generic_type_candidates(path, candidates);
}

/// Recurse into generic arguments and associated-type values of a path.
///
/// This intentionally does not add `path` itself to either stream. It lets a `dyn`
/// bound contribute the bound only as a trait candidate while retaining type edges for
/// its generic arguments and associated-type binding values.
fn collect_path_generic_type_candidates(path: &syn::Path, candidates: &mut TypeRefCandidates) {
    for seg in &path.segments {
        if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
            for arg in &args.args {
                match arg {
                    syn::GenericArgument::Type(inner_ty) => {
                        collect_type_names_from_syn(inner_ty, candidates);
                    }
                    syn::GenericArgument::AssocType(assoc) => {
                        collect_type_names_from_syn(&assoc.ty, candidates);
                    }
                    // Lifetimes, const generics, assoc const — not type paths.
                    _ => {}
                }
            }
        } else if let syn::PathArguments::Parenthesized(args) = &seg.arguments {
            // Fn trait `Fn(A, B) -> C`
            for input in &args.inputs {
                collect_type_names_from_syn(input, candidates);
            }
            if let syn::ReturnType::Type(_, ret) = &args.output {
                collect_type_names_from_syn(ret, candidates);
            }
        }
    }
}

/// Return a path in the same `::`-joined form used by catalogue references.
fn path_as_string(path: &syn::Path) -> String {
    let segments =
        path.segments.iter().map(|seg| seg.ident.to_string()).collect::<Vec<_>>().join("::");
    if path.leading_colon.is_some() { format!("::{segments}") } else { segments }
}

/// Resolve a `TypeRef` string to **all** rendered mermaid node IDs that it references.
///
/// Uses `syn::parse_str::<syn::Type>` to parse the full type expression (handling
/// `&T`, `&mut T`, `Result<T, E>`, `Vec<T>`, `Option<T>`, `Box<T>`, `Arc<T>`,
/// `[T]`, `(A, B)`, nested generics, etc.), then walks the resulting AST to collect
/// every referenced type-path name and `dyn Trait` bound. Type candidates are resolved
/// against `node_index`; trait candidates are resolved against `trait_index`. Only names
/// that map to a **declared** catalogue representative node produce an entry in the
/// returned `Vec`. Undeclared/primitive/generic/external types and traits are silently
/// skipped.
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
    trait_index: &NodeIndex,
    current_crate: &str,
    self_node_id: Option<&str>,
) -> Vec<String> {
    resolve_type_ref_node_ids_with_generics(
        type_ref_str,
        node_index,
        trait_index,
        current_crate,
        self_node_id,
        &[],
    )
}

/// Same as [`resolve_type_ref_node_ids`], but treats `generic_params` as the
/// generic parameter names declared by the surrounding declaration: a bare
/// candidate matching one of them is a generic use that shadows any identically
/// named catalogue type, so it never resolves to a node (no bogus edge to an
/// unrelated declared type of the same name).
pub(crate) fn resolve_type_ref_node_ids_with_generics(
    type_ref_str: &str,
    node_index: &NodeIndex,
    trait_index: &NodeIndex,
    current_crate: &str,
    self_node_id: Option<&str>,
    generic_params: &[&str],
) -> Vec<String> {
    // Parse with syn; fall back silently on malformed input.
    let syn_type = match syn::parse_str::<syn::Type>(type_ref_str) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };

    let mut candidates = TypeRefCandidates::default();
    collect_type_names_from_syn(&syn_type, &mut candidates);

    // Deduplicate: the same path may appear multiple times (e.g. nested).
    candidates.type_names.sort_unstable();
    candidates.type_names.dedup();
    candidates.trait_names.sort_unstable();
    candidates.trait_names.dedup();

    // Resolve each candidate against the node index; keep only declared types.
    // The literal name "Self" is substituted directly with `self_node_id` when
    // provided — `NodeIndex` does not hold a "Self" key (OS-04 / correctness).
    let mut resolved: Vec<String> = Vec::new();
    for candidate in &candidates.type_names {
        if candidate.name == "Self" {
            if let Some(id) = self_node_id {
                push_unique_node_id(&mut resolved, id);
            }
            // If self_node_id is None, "Self" has no resolution — silent skip.
            continue;
        }
        // A declared parameter shadows catalogue types both as a bare name and
        // as the root of a generic projection (`T::Item` is a projection on
        // the parameter, never a qualified catalogue type).
        let candidate_root = candidate.name.split("::").next().unwrap_or(candidate.name.as_str());
        if !candidate.is_absolute && generic_params.contains(&candidate_root) {
            continue;
        }
        let lookup_name = if candidate.is_absolute && !candidate.name.starts_with("::") {
            format!("::{}", candidate.name)
        } else {
            candidate.name.clone()
        };
        if let Some(node_id) = node_index.resolve(&lookup_name, current_crate) {
            push_unique_node_id(&mut resolved, node_id);
        }
    }

    for candidate in &candidates.trait_names {
        if let Some(node_id) = resolve_trait_ref_node_id(candidate, current_crate, trait_index) {
            push_unique_node_id(&mut resolved, node_id);
        }
    }
    resolved
}

/// Add a node id to the resolved union only once.
fn push_unique_node_id(resolved: &mut Vec<String>, node_id: &str) {
    if !resolved.iter().any(|resolved_id| resolved_id == node_id) {
        resolved.push(node_id.to_string());
    }
}

/// Resolve a `trait_ref` string to the rendered representative node ID for that trait.
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
    trait_index: &'a NodeIndex,
) -> Option<&'a str> {
    resolve_trait_ref_node_id(trait_ref_str, current_crate, trait_index)
}

/// Resolve a trait reference through the shared four-step trait lookup policy.
///
/// Bare names resolve in the current crate. `crate::`, `self::`, and `super::` paths
/// are normalised to the current crate and their last segment. Other qualified paths
/// use their first segment as the crate and their last segment as the trait. Missing
/// entries are workspace-external or undeclared and therefore silently skipped.
fn resolve_trait_ref_node_id<'a>(
    trait_ref_str: &str,
    current_crate: &str,
    trait_index: &'a NodeIndex,
) -> Option<&'a str> {
    trait_index.resolve(trait_ref_str, current_crate)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::{NodeIndex, resolve_type_ref_node_ids_with_generics};

    #[test]
    fn test_resolve_absolute_path_with_generic_named_as_crate() {
        let mut node_index = NodeIndex::new();
        node_index.insert("T", "Item", "T_Item".to_owned());
        let trait_index = NodeIndex::new();

        let resolved = resolve_type_ref_node_ids_with_generics(
            "::T::Item",
            &node_index,
            &trait_index,
            "domain",
            None,
            &["T"],
        );

        assert_eq!(resolved, ["T_Item"]);
    }

    #[test]
    fn test_resolve_absolute_path_does_not_fall_back_to_local_module() {
        use domain::tddd::catalogue_v2::{CatalogueEntryKey, CrateName, ModulePath};

        let mut node_index = NodeIndex::new();
        let crate_name = CrateName::new("domain".to_owned()).unwrap();
        let key = CatalogueEntryKey::try_new("Item".to_owned()).unwrap();
        let module = ModulePath::from_segments(vec!["T".to_owned()]).unwrap();
        node_index.insert_catalogue_entry(&crate_name, &key, &module, "local_item".to_owned());
        let trait_index = NodeIndex::new();

        let resolved = resolve_type_ref_node_ids_with_generics(
            "::T::Item",
            &node_index,
            &trait_index,
            "domain",
            None,
            &[],
        );

        assert!(resolved.is_empty(), "absolute external path must not use a local module");
    }
}
