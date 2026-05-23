//! Cross-cluster edge collection for the depth-1 overview renderer (T009 / T016 / T017).
//!
//! Provides style-free edge extraction from rustdoc data structures, used by the
//! depth-1 overview to determine cross-cluster connectivity without applying depth-2
//! edge style config keys (which may be absent from the overview style config).
//!
//! (IN-14 / AC-13 / CN-07 / T016 / AC-20)

use domain::tddd::baseline_document::BaselineDocument;

use super::impl_processor;
use super::node_id_generator::{module_path_from_summary, trait_rep_node_id, type_rep_node_id};
use super::trait_index::{TraitKey, build_trait_index};

// ---------------------------------------------------------------------------
// Cross-cluster edge collection (style-free, depth-1 path)
// ---------------------------------------------------------------------------

/// Collect cross-cluster edge pairs for the depth-1 overview without requiring edge style keys.
///
/// The depth-1 overview needs to know which node pairs are connected by any edge so it
/// can determine cross-cluster connectivity.  Calling the depth-2 entry renderer
/// (`emit_all_entries_for_layer`) for this purpose is incorrect because the depth-2
/// renderer fails when edge style keys (e.g. `[edge.field]`) are absent from the style
/// config — even though the overview does not render those edge styles.
///
/// This function extracts (src_rep_node_id, dst_node_id) pairs directly from the rustdoc
/// data structures, bypassing all edge style lookups.  It covers all edge kinds emitted
/// by the depth-2 renderer:
///
/// - **Struct fields** (K decision): plain and tuple struct field type references.
/// - **TypeAlias targets** (N decision): alias target type references.
/// - **Enum variant payload edges** (H decision): tuple-variant elements and struct-variant fields.
/// - **Trait impl edges** (O-r1 / BB-4-fix1 / J decision): type `-.impl.->` trait edges,
///   excluding negative, synthetic, and blanket impls.
///
/// **T016 / AC-20**: type resolution for field / alias / payload edges now uses recursive
/// `ResolvedPath.args` traversal (see [`collect_resolved_node_ids_from_type`]).
/// `Type::Primitive` / `Type::Generic` / types absent from `krate.paths` produce no pairs.
/// Anonymous nodes (`prim_*` / `generic_*` / `anon_*`) are never generated.
///
/// Cross-crate edges within the same layer are resolved correctly: `collect_resolved_node_ids_from_type`
/// uses `summary.path[0]` as the target crate name (not crate_id == 0 only), so a type
/// in cluster A with a field in cluster B (different crate, same layer) still produces a
/// cross-cluster edge.
///
/// Returns a vec of (src_node_id, dst_node_id) pairs.  Callers filter for cross-cluster.
///
/// No panics: all indexing via `.get()` / iterators.
pub(super) fn collect_entry_edge_pairs(
    baselines: &[BaselineDocument],
    layer: &str,
) -> Vec<(String, String)> {
    use super::node_extractor::{ExtractedNode, extract_nodes};
    use rustdoc_types::{Id, ItemEnum, StructKind, Type, VariantKind};

    let layer_id = match domain::tddd::layer_id::LayerId::try_new(layer) {
        Ok(l) => l,
        Err(_) => return Vec::new(),
    };

    let nodes = extract_nodes(baselines, &layer_id);
    let mut pairs: Vec<(String, String)> = Vec::new();

    // -----------------------------------------------------------------------
    // Pass 1: field, alias, and enum variant payload edges.
    // -----------------------------------------------------------------------
    for node in &nodes {
        let doc = node.doc();
        let item = node.item();
        let id = node.id();
        let krate = &doc.krate;
        let crate_name = doc.crate_name.as_str();
        let layer_str = doc.layer.as_ref();

        let summary_path_opt = krate.paths.get(&id).map(|s| s.path.as_slice());
        let module_path = summary_path_opt.map(module_path_from_summary).unwrap_or_default();

        // Compute the rep-node id for the src entry.
        let src_rep_id: Option<String> = match node {
            ExtractedNode::Struct { .. } | ExtractedNode::Enum { .. } => {
                let name = item.name.as_deref().unwrap_or("");
                Some(type_rep_node_id(layer_str, crate_name, &module_path, name))
            }
            ExtractedNode::Trait { .. } => {
                let name = item.name.as_deref().unwrap_or("");
                Some(trait_rep_node_id(layer_str, crate_name, &module_path, name))
            }
            ExtractedNode::TypeAlias { .. } => {
                let name = item.name.as_deref().unwrap_or("");
                Some(type_rep_node_id(layer_str, crate_name, &module_path, name))
            }
            ExtractedNode::Function { .. } => None,
        };

        let src = match src_rep_id {
            Some(s) => s,
            None => continue,
        };

        // Collect outgoing edge targets based on item kind (T016 / AC-20: recursive resolution).
        match &item.inner {
            ItemEnum::Struct(s) => {
                // K decision: struct fields.
                let field_ids: Vec<Id> =
                    match &s.kind {
                        StructKind::Plain { fields, has_stripped_fields } => {
                            if *has_stripped_fields { Vec::new() } else { fields.to_vec() }
                        }
                        StructKind::Tuple(maybe_ids) => {
                            maybe_ids.iter().filter_map(|opt| *opt).collect()
                        }
                        StructKind::Unit => Vec::new(),
                    };
                for field_id in field_ids {
                    if let Some(field_item) = krate.index.get(&field_id) {
                        if let ItemEnum::StructField(field_ty) = &field_item.inner {
                            for dst in
                                collect_resolved_node_ids_from_type(field_ty, krate, layer_str)
                            {
                                pairs.push((src.clone(), dst));
                            }
                        }
                    }
                }
            }
            ItemEnum::Enum(e) => {
                // H decision: enum variant payload edges.
                for &variant_id in &e.variants {
                    if let Some(variant_item) = krate.index.get(&variant_id) {
                        if let ItemEnum::Variant(v) = &variant_item.inner {
                            match &v.kind {
                                VariantKind::Tuple(maybe_ids) => {
                                    for &maybe_id in maybe_ids {
                                        if let Some(field_id) = maybe_id {
                                            if let Some(field_item) = krate.index.get(&field_id) {
                                                if let ItemEnum::StructField(field_ty) =
                                                    &field_item.inner
                                                {
                                                    for dst in collect_resolved_node_ids_from_type(
                                                        field_ty, krate, layer_str,
                                                    ) {
                                                        pairs.push((src.clone(), dst));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                VariantKind::Struct { fields, has_stripped_fields } => {
                                    if !has_stripped_fields {
                                        for &field_id in fields {
                                            if let Some(field_item) = krate.index.get(&field_id) {
                                                if let ItemEnum::StructField(field_ty) =
                                                    &field_item.inner
                                                {
                                                    for dst in collect_resolved_node_ids_from_type(
                                                        field_ty, krate, layer_str,
                                                    ) {
                                                        pairs.push((src.clone(), dst));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                VariantKind::Plain => {}
                            }
                        }
                    }
                }
            }
            ItemEnum::TypeAlias(ta) => {
                // N decision: alias target(s).
                for dst in collect_resolved_node_ids_from_type(&ta.type_, krate, layer_str) {
                    pairs.push((src.clone(), dst));
                }
            }
            _ => {}
        }
    }

    // -----------------------------------------------------------------------
    // Pass 2: trait impl edges (O-r1 / BB-4-fix1 / J decision).
    //
    // For each baseline in the layer, scan all ItemEnum::Impl items.
    // Only concrete-type (ResolvedPath for_) non-negative non-synthetic non-blanket
    // non-inherent (has trait_) impls are included.
    //
    // We need the trait's entry subgraph id as the edge dst. The trait index maps
    // TraitKey → trait_sg_id. We build this index to look up the dst, mirroring
    // the logic in impl_processor::emit_impl_edges but without style dependency.
    // -----------------------------------------------------------------------
    let trait_index = build_trait_index(baselines, layer);

    for doc in baselines {
        if doc.layer.as_ref() != layer {
            continue;
        }
        let krate = &doc.krate;
        let own_crate_name = doc.crate_name.as_str();
        let layer_str = doc.layer.as_ref();

        for item in krate.index.values() {
            let impl_data = match &item.inner {
                ItemEnum::Impl(i) => i,
                _ => continue,
            };
            // Skip negative, synthetic, blanket copies (BB-4-fix1).
            if impl_data.is_negative || impl_data.is_synthetic || impl_data.blanket_impl.is_some() {
                continue;
            }
            // Inherent impl (no trait_): skip.
            let trait_path = match &impl_data.trait_ {
                Some(p) => p,
                None => continue,
            };
            // Blanket body (for_: Generic): skip.
            if matches!(impl_data.for_, Type::Generic(_)) {
                continue;
            }
            // Only concrete type trait impls (J decision).
            if !matches!(impl_data.for_, Type::ResolvedPath(_)) {
                continue;
            }
            // Resolve the type's rep node id.
            //
            // We do NOT check `krate.index.get(&p.id)` for visibility or item kind here,
            // because cross-crate `for_` types (implementing type from another workspace
            // crate) may not appear in the current crate's `index` — they appear only in
            // `krate.paths` / `external_crates`. Instead we compute the node id from
            // `krate.paths` alone and let `lookup_cluster` / `node_cluster_map` decide
            // whether the type is an in-scope public entry (if not in the map, the edge
            // is skipped in the cross-cluster filter step).
            let type_rep_id: Option<String> = if let Type::ResolvedPath(p) = &impl_data.for_ {
                krate.paths.get(&p.id).and_then(|summary| {
                    // Use path[0] as crate name to handle cross-crate within same layer.
                    let src_crate =
                        summary.path.first().map(|s| s.as_str()).unwrap_or(own_crate_name);
                    let module_path = module_path_from_summary(&summary.path);
                    let type_name = summary.path.last()?;
                    Some(type_rep_node_id(layer_str, src_crate, &module_path, type_name))
                })
            } else {
                None
            };
            let type_rep_id = match type_rep_id {
                Some(id) => id,
                None => continue,
            };

            // Resolve trait key for index lookup (O-r1, CN-05: no Id cross-comparison).
            // `build_trait_index` keys traits with the `"::"` joined middle segments
            // (= `module_path_str_from_summary` in impl_processor, a private function).
            // We replicate that join here: skip crate_name (index 0) and trait_name (last),
            // then join the middle with `"::"` — matching what the index key holds.
            let trait_sg_id: Option<&String> =
                krate.paths.get(&trait_path.id).and_then(|trait_summary| {
                    // Build a TraitKey matching what build_trait_index produces.
                    let trait_crate = trait_summary.path.first().map(|s| s.as_str())?;
                    // Module path for the key uses "::" separator (matching build_trait_index).
                    let total = trait_summary.path.len();
                    let trait_mp: String = if total <= 2 {
                        String::new()
                    } else {
                        trait_summary
                            .path
                            .iter()
                            .skip(1)
                            .take(total - 2)
                            .cloned()
                            .collect::<Vec<_>>()
                            .join("::")
                    };
                    let trait_name = trait_summary.path.last().map(|s| s.as_str())?;
                    let key = TraitKey {
                        crate_name: trait_crate.to_string(),
                        module_path: trait_mp,
                        trait_name: trait_name.to_string(),
                    };
                    trait_index.get(&key)
                });
            let trait_sg_id = match trait_sg_id {
                Some(id) => id,
                None => continue, // trait not in index — skip (CN-10)
            };
            // Trait rep node id: `{trait_sg_id}__self`.
            let trait_rep_id = format!("{trait_sg_id}__self");
            pairs.push((type_rep_id, trait_rep_id));
        }
    }

    // -----------------------------------------------------------------------
    // Pass 3: method-signature edges (T017 / AC-19 depth-1 path).
    //
    // Collect (src_rep_id, dst_type_rep_id) pairs from:
    //   3a) Inherent method params/returns: from Impl items where
    //       `trait_: None / blanket_impl: None / is_negative: false /
    //       is_synthetic: false / for_: Type::ResolvedPath`.
    //       src = the `for_` type's rep node id.
    //   3b) Trait method params/returns: from Trait items' Function variants.
    //       src = the Trait entry's rep node id.
    //
    // Type resolution uses `collect_resolved_node_ids_from_type` (same as Pass 1),
    // which includes cross-crate within the same layer (unlike the depth-2 renderer
    // which uses `collect_own_crate_node_ids_from_type` with crate_id == 0 only).
    // This is correct for the depth-1 overview where any cross-cluster edge must be
    // reported regardless of whether src and dst are in the same or different crates.
    //
    // No panics: all indexing via `.get()` / iterators.
    // -----------------------------------------------------------------------
    for doc in baselines {
        if doc.layer.as_ref() != layer {
            continue;
        }
        let krate = &doc.krate;
        let own_crate_name = doc.crate_name.as_str();
        let layer_str = doc.layer.as_ref();

        // Pass 3a: inherent method param/return edges.
        //
        // Scan all Impl items. For each inherent impl (trait_: None, blanket_impl: None,
        // is_negative: false, is_synthetic: false, for_: ResolvedPath), compute the
        // src rep node id from `for_`, then for each Public, non-provided method item
        // that is a Function, resolve param/return types to cross-cluster pairs.
        //
        // Guards applied (symmetric to `emit_inherent_methods` in impl_processor):
        // - Visibility::Public only (CC-1): private helpers do not contribute to
        //   the rendered API surface and must not inflate the depth-1 overview.
        // - Provided-method skip (CN-11): method ids that appear as items in any
        //   trait impl block of the same krate are provided methods; they must not
        //   be double-counted as inherent method edges.
        //
        // Note on phantom source nodes: pairs whose src is absent from
        // `node_cluster_map` (private types, non-rendered entries, etc.) are
        // silently dropped by `render_overview_mermaid`'s `lookup_cluster` call
        // (`None => continue`).  No phantom source node can enter the mermaid output.
        let provided_method_ids = impl_processor::collect_provided_method_ids(krate);

        for item in krate.index.values() {
            let impl_data = match &item.inner {
                ItemEnum::Impl(i) => i,
                _ => continue,
            };
            // Only inherent impls (BB-4-fix1 / CN-11).
            if impl_data.trait_.is_some()
                || impl_data.blanket_impl.is_some()
                || impl_data.is_negative
                || impl_data.is_synthetic
            {
                continue;
            }
            // Only concrete for_ types (ResolvedPath).
            let for_path = match &impl_data.for_ {
                Type::ResolvedPath(p) => p,
                _ => continue,
            };
            // Resolve the src rep node id from `for_` via krate.paths.
            let src_rep_id: Option<String> = krate.paths.get(&for_path.id).and_then(|summary| {
                // Use path[0] as crate name to handle cross-crate within same layer.
                let src_crate = summary.path.first().map(|s| s.as_str()).unwrap_or(own_crate_name);
                let module_path = module_path_from_summary(&summary.path);
                let type_name = summary.path.last()?;
                Some(type_rep_node_id(layer_str, src_crate, &module_path, type_name))
            });
            let src_rep_id = match src_rep_id {
                Some(id) => id,
                None => continue,
            };

            // Walk each method item in this inherent impl block.
            for &method_id in &impl_data.items {
                // Skip provided methods (CN-11 safety guard, symmetric to emit_inherent_methods).
                if provided_method_ids.contains(&method_id) {
                    continue;
                }
                let method_item = match krate.index.get(&method_id) {
                    Some(m) => m,
                    None => continue,
                };
                // Visibility filter (CC-1): Public only for inherent methods.
                // Private helpers do not contribute to the rendered API surface.
                if !matches!(method_item.visibility, rustdoc_types::Visibility::Public) {
                    continue;
                }
                let fn_data = match &method_item.inner {
                    ItemEnum::Function(f) => f,
                    _ => continue,
                };

                // Collect param types.
                for (_param_name, param_ty) in &fn_data.sig.inputs {
                    for dst in collect_resolved_node_ids_from_type(param_ty, krate, layer_str) {
                        pairs.push((src_rep_id.clone(), dst));
                    }
                }
                // Collect return type.
                if let Some(output_ty) = &fn_data.sig.output {
                    for dst in collect_resolved_node_ids_from_type(output_ty, krate, layer_str) {
                        pairs.push((src_rep_id.clone(), dst));
                    }
                }
            }
        }

        // Pass 3b: trait method param/return edges.
        //
        // Scan all Trait items (own-crate, crate_id == 0, Public) in krate.index.
        // For each Trait's Function items (H' decision), resolve param/return types.
        // src = trait's rep node id.
        for (id, item) in &krate.index {
            // Own-crate items only (crate_id == 0, mirroring build_trait_index / CC-1).
            if item.crate_id != 0 {
                continue;
            }
            if !matches!(item.visibility, rustdoc_types::Visibility::Public) {
                continue;
            }
            let trait_data = match &item.inner {
                ItemEnum::Trait(t) => t,
                _ => continue,
            };
            // Must be in krate.paths to compute the rep node id.
            let summary = match krate.paths.get(id) {
                Some(s) => s,
                None => continue,
            };
            let module_path = module_path_from_summary(&summary.path);
            let trait_name = match summary.path.last() {
                Some(n) => n,
                None => continue,
            };
            let src_rep_id = super::node_id_generator::trait_rep_node_id(
                layer_str,
                own_crate_name,
                &module_path,
                trait_name,
            );

            // Walk each method item in the trait definition (H' decision).
            for &method_item_id in &trait_data.items {
                let method_item = match krate.index.get(&method_item_id) {
                    Some(m) => m,
                    None => continue,
                };
                // CC-1 exception: trait methods use Visibility::Default — accepted.
                if !matches!(
                    method_item.visibility,
                    rustdoc_types::Visibility::Public | rustdoc_types::Visibility::Default
                ) {
                    continue;
                }
                let fn_data = match &method_item.inner {
                    ItemEnum::Function(f) => f,
                    _ => continue,
                };

                // Collect param types.
                for (_param_name, param_ty) in &fn_data.sig.inputs {
                    for dst in collect_resolved_node_ids_from_type(param_ty, krate, layer_str) {
                        pairs.push((src_rep_id.clone(), dst));
                    }
                }
                // Collect return type.
                if let Some(output_ty) = &fn_data.sig.output {
                    for dst in collect_resolved_node_ids_from_type(output_ty, krate, layer_str) {
                        pairs.push((src_rep_id.clone(), dst));
                    }
                }
            }
        }
    }

    pairs
}

// ---------------------------------------------------------------------------
// Type reference resolution helpers (cross-crate, depth-1 path)
// ---------------------------------------------------------------------------

/// Collect representative node ids for all **resolved** (own-crate or cross-crate within
/// the same layer) types referenced directly or nested inside generic arguments in `ty`.
///
/// **T016 / AC-20** — replaces the old `resolve_type_node_id` single-target helper.  The
/// key differences from the depth-2 `collect_own_crate_node_ids_from_type` (in
/// `impl_processor`) are:
///
/// - **Cross-crate within same layer is included**: the target crate name is derived from
///   `summary.path[0]` (not via `crate_id == 0`), so a type from a sibling crate in the
///   same layer produces a cross-cluster edge in the depth-1 overview.
/// - **`Type::Primitive` / `Type::Generic`**: produces no node ids (silent skip).
/// - **`ResolvedPath` not in `krate.paths`**: produces no node ids (silent skip).
/// - **Recursive traversal**: `ResolvedPath.args` (generic arguments) are traversed, so
///   nested own-crate types inside `Vec<MyType>`, `Arc<MyType>`, etc. are captured.
///
/// Returns a `Vec<String>` of representative node ids.  The vec may be empty (e.g. for
/// primitives, generics, or types absent from `krate.paths`).
///
/// No panics: all indexing via `.get()` / iterators.
pub(super) fn collect_resolved_node_ids_from_type(
    ty: &rustdoc_types::Type,
    krate: &rustdoc_types::Crate,
    layer: &str,
) -> Vec<String> {
    let mut out = Vec::new();
    collect_resolved_node_ids_recursive(ty, krate, layer, &mut out);
    out
}

/// Internal recursive helper for [`collect_resolved_node_ids_from_type`].
fn collect_resolved_node_ids_recursive(
    ty: &rustdoc_types::Type,
    krate: &rustdoc_types::Crate,
    layer: &str,
    out: &mut Vec<String>,
) {
    use rustdoc_types::Type;
    match ty {
        Type::ResolvedPath(path) => {
            if let Some(summary) = krate.paths.get(&path.id) {
                // Use path[0] as the target crate name (works for both same-crate and
                // cross-crate within same layer, unlike the crate_id == 0 check in
                // collect_own_crate_node_ids_from_type which is depth-2 only).
                if let Some(target_crate) = summary.path.first().map(|s| s.as_str()) {
                    let module_path = module_path_from_summary(&summary.path);
                    if let Some(type_name) = summary.path.last() {
                        out.push(type_rep_node_id(layer, target_crate, &module_path, type_name));
                    }
                }
            }
            // Whether found or not, recurse into generic args (e.g. Vec<MyType> → MyType).
            if let Some(args) = path.args.as_deref() {
                collect_resolved_generic_args(args, krate, layer, out);
            }
        }
        Type::Primitive(_) | Type::Generic(_) => {
            // Primitive (u32, bool, etc.) and generic type params → skip (T016 / AC-20).
        }
        Type::BorrowedRef { type_: inner, .. }
        | Type::RawPointer { type_: inner, .. }
        | Type::Slice(inner)
        | Type::Array { type_: inner, .. }
        | Type::Pat { type_: inner, .. } => {
            collect_resolved_node_ids_recursive(inner, krate, layer, out);
        }
        Type::Tuple(tys) => {
            for t in tys {
                collect_resolved_node_ids_recursive(t, krate, layer, out);
            }
        }
        _ => {
            // DynTrait, ImplTrait, QualifiedPath, FunctionPointer, etc.: skip.
        }
    }
}

/// Internal helper: descend into [`rustdoc_types::GenericArgs`] for depth-1 edge collection.
fn collect_resolved_generic_args(
    args: &rustdoc_types::GenericArgs,
    krate: &rustdoc_types::Crate,
    layer: &str,
    out: &mut Vec<String>,
) {
    use rustdoc_types::{AssocItemConstraintKind, GenericArg, GenericArgs, Term};
    match args {
        GenericArgs::AngleBracketed { args: ga, constraints } => {
            for arg in ga {
                if let GenericArg::Type(t) = arg {
                    collect_resolved_node_ids_recursive(t, krate, layer, out);
                }
            }
            for constraint in constraints {
                if let Some(c_args) = constraint.args.as_deref() {
                    collect_resolved_generic_args(c_args, krate, layer, out);
                }
                match &constraint.binding {
                    AssocItemConstraintKind::Equality(term) => {
                        if let Term::Type(t) = term {
                            collect_resolved_node_ids_recursive(t, krate, layer, out);
                        }
                    }
                    AssocItemConstraintKind::Constraint(_) => {}
                }
            }
        }
        GenericArgs::Parenthesized { inputs, output } => {
            for t in inputs {
                collect_resolved_node_ids_recursive(t, krate, layer, out);
            }
            if let Some(ret) = output {
                collect_resolved_node_ids_recursive(ret, krate, layer, out);
            }
        }
        _ => {}
    }
}
