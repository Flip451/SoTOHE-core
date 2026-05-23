//! Own-crate type node-id resolution utilities for the baseline-graph renderer (T015 / T016).
//!
//! Provides recursive `ResolvedPath.args` traversal to collect own-crate representative
//! node ids from a `rustdoc_types::Type`. Used by both `impl_processor` (method edges)
//! and `entry_emitter` (struct field, enum variant payload, TypeAlias target edges).
//!
//! Extracted from `impl_processor` to keep that module within the 700-line production limit.
//!
//! (AC-19 / AC-20 / CN-10)

use rustdoc_types::{ItemEnum, Visibility};

use super::node_id_generator::{module_path_from_summary, type_rep_node_id};

// ---------------------------------------------------------------------------
// Own-crate type resolution
// ---------------------------------------------------------------------------

/// Collect own-crate representative node ids from a single `rustdoc_types::Type`.
///
/// Recursively traverses `ResolvedPath.args` to find all own-crate types referenced
/// by the given type (direct, wrapped, or nested in generic args). For each such type,
/// produces the mermaid representative node id via [`type_rep_node_id`].
///
/// **Rules (T016 / AC-20)**:
/// - `Type::Primitive` / `Type::Generic` → no output (no edge generated).
/// - External crate types (`crate_id != 0`) → no output (CN-10).
/// - Non-rendered kinds (Union, Macro, etc.) → no output.
/// - Private types (`!Visibility::Public`) → no output (CC-1).
/// - Transparent wrappers (`BorrowedRef`, `RawPointer`, `Slice`, `Array`, `Pat`, `Tuple`)
///   → recurse into inner type.
/// - `ResolvedPath` with generic args → recurse into args after processing the path.
///
/// Returns an empty `Vec` when no own-crate types are found.
///
/// No panics — all indexing via `.get()` / iterators.
pub(super) fn collect_own_crate_node_ids_from_type(
    ty: &rustdoc_types::Type,
    krate: &rustdoc_types::Crate,
    layer: &str,
    crate_name: &str,
) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    collect_own_crate_node_ids_recursive(ty, krate, layer, crate_name, &mut out);
    out
}

/// Internal recursive helper for [`collect_own_crate_node_ids_from_type`].
fn collect_own_crate_node_ids_recursive(
    ty: &rustdoc_types::Type,
    krate: &rustdoc_types::Crate,
    layer: &str,
    crate_name: &str,
    out: &mut Vec<String>,
) {
    match ty {
        rustdoc_types::Type::ResolvedPath(path) => {
            // Look up the type in krate.paths.
            if let Some(summary) = krate.paths.get(&path.id) {
                if summary.crate_id == 0 {
                    // Own-crate type: apply the same visibility + kind filter as the
                    // renderer (parity with resolve_type_rep_node_id / extract_nodes).
                    // Private types and non-rendered kinds (Union, etc.) have no entry
                    // subgraph, so emitting an edge to their rep node would create a
                    // phantom Mermaid node (CN-10 / CC-1 / B-r1).
                    if let Some(item) = krate.index.get(&path.id) {
                        if matches!(item.visibility, Visibility::Public)
                            && matches!(
                                item.inner,
                                ItemEnum::Struct(_)
                                    | ItemEnum::Enum(_)
                                    | ItemEnum::TypeAlias(_)
                                    | ItemEnum::Trait(_)
                            )
                        {
                            let module_path = module_path_from_summary(&summary.path);
                            if let Some(type_name) = summary.path.last() {
                                out.push(type_rep_node_id(
                                    layer,
                                    crate_name,
                                    &module_path,
                                    type_name,
                                ));
                            }
                        }
                    }
                    // Private or non-rendered kind: skip edge (but still recurse into
                    // generic args — they may contain other renderable own-crate types).
                }
                // Whether own-crate or external, recurse into generic args to
                // find nested own-crate types (e.g. Arc<MyType> → MyType).
            }
            // Recurse into generic arguments (e.g. Vec<TrackId> → TrackId).
            if let Some(args) = path.args.as_deref() {
                collect_own_crate_node_ids_from_generic_args(args, krate, layer, crate_name, out);
            }
        }
        rustdoc_types::Type::Primitive(_) | rustdoc_types::Type::Generic(_) => {
            // Primitive (u32, bool, etc.) and generic type params (T, U) → skip.
        }
        rustdoc_types::Type::BorrowedRef { type_: inner, .. }
        | rustdoc_types::Type::RawPointer { type_: inner, .. }
        | rustdoc_types::Type::Slice(inner)
        | rustdoc_types::Type::Array { type_: inner, .. }
        | rustdoc_types::Type::Pat { type_: inner, .. } => {
            // Transparent wrappers — recurse into the inner type.
            collect_own_crate_node_ids_recursive(inner, krate, layer, crate_name, out);
        }
        rustdoc_types::Type::Tuple(tys) => {
            for t in tys {
                collect_own_crate_node_ids_recursive(t, krate, layer, crate_name, out);
            }
        }
        _ => {
            // DynTrait, ImplTrait, QualifiedPath, FunctionPointer, etc.:
            // too complex for our edge-drawing purpose; no own-crate node ids emitted.
        }
    }
}

/// Internal recursive helper: descend into [`rustdoc_types::GenericArgs`].
fn collect_own_crate_node_ids_from_generic_args(
    args: &rustdoc_types::GenericArgs,
    krate: &rustdoc_types::Crate,
    layer: &str,
    crate_name: &str,
    out: &mut Vec<String>,
) {
    use rustdoc_types::{AssocItemConstraintKind, GenericArg, GenericArgs, Term};
    match args {
        GenericArgs::AngleBracketed { args: ga, constraints } => {
            // Process positional/type generic arguments (e.g. `Vec<MyType>`).
            for arg in ga {
                if let GenericArg::Type(t) = arg {
                    collect_own_crate_node_ids_recursive(t, krate, layer, crate_name, out);
                }
            }
            // Process associated-type constraints (e.g. `Iterator<Item = MyType>`).
            // Rustdoc stores these in `constraints` as `AssocItemConstraint` entries
            // (AC-20: must capture nested own-crate types in assoc-type bindings).
            for constraint in constraints {
                // Recurse into args of the constraint itself (e.g.
                // `Item<Foo = Bar>` — the args on the constraint).
                if let Some(c_args) = constraint.args.as_deref() {
                    collect_own_crate_node_ids_from_generic_args(
                        c_args, krate, layer, crate_name, out,
                    );
                }
                // Recurse into the constraint binding (Equality or Bound).
                match &constraint.binding {
                    AssocItemConstraintKind::Equality(term) => {
                        if let Term::Type(t) = term {
                            collect_own_crate_node_ids_recursive(t, krate, layer, crate_name, out);
                        }
                        // Term::Constant: no type reference to follow.
                    }
                    AssocItemConstraintKind::Constraint(_) => {
                        // Generic bounds (e.g. `Item: Trait`) do not contribute
                        // a direct type reference for an edge; skip.
                    }
                }
            }
        }
        GenericArgs::Parenthesized { inputs, output } => {
            // Fn traits: Fn(A, B) -> C — recurse into inputs and optional output.
            for t in inputs {
                collect_own_crate_node_ids_recursive(t, krate, layer, crate_name, out);
            }
            if let Some(ret) = output {
                collect_own_crate_node_ids_recursive(ret, krate, layer, crate_name, out);
            }
        }
        _ => {}
    }
}
