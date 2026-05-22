//! Entry emission logic for the mermaid contract-map renderer (T005–T008).
//!
//! All items in this module are `pub(super)` — they are implementation details
//! of `ContractMapRendererAdapter` and must not appear in the infrastructure
//! crate's public API.

use std::collections::BTreeMap;

use domain::tddd::ContractMapRendererError;

use super::{
    NodeIndex, StyleConfig, apply_shape, edge_arrow_label, edge_line, function_node_id,
    resolve_type_ref_node_ids, sanitize, trait_node_id, trait_rep_node_id, type_node_id,
    type_rep_node_id,
};

// ---------------------------------------------------------------------------
// Entry kind enum (T004 / Decision B-1)
// ---------------------------------------------------------------------------

pub(super) enum EntryKind<'a> {
    Type(&'a str, &'a domain::tddd::catalogue_v2::entries::TypeEntry),
    Trait(&'a str, &'a domain::tddd::catalogue_v2::entries::TraitEntry),
    Function(
        &'a domain::tddd::catalogue_v2::identifiers::FunctionPath,
        &'a domain::tddd::catalogue_v2::entries::FunctionEntry,
    ),
}

// ---------------------------------------------------------------------------
// Emit entry into subgraph lines + edge lines + class attach lines
// ---------------------------------------------------------------------------

/// # Errors
///
/// Returns `ContractMapRendererError::RenderFailed` when a required `[edge.*]`
/// key is absent from the style configuration (CN-02 — fail-closed, no hard-coded
/// fallback).
#[allow(clippy::too_many_arguments)]
pub(super) fn emit_entry<'a>(
    entry: &EntryKind<'a>,
    subgraph_lines: &mut Vec<String>,
    edge_lines: &mut Vec<String>,
    class_attach: &mut Vec<String>,
    style: &StyleConfig,
    inherent_methods: &BTreeMap<
        String,
        Vec<&'a domain::tddd::catalogue_v2::methods::MethodDeclaration>,
    >,
    node_index: &NodeIndex,
    layer: &str,
    crate_name: &str,
) -> Result<(), ContractMapRendererError> {
    use domain::tddd::catalogue_v2::composite::TypeKindV2;
    use domain::tddd::catalogue_v2::variants::VariantPayload;

    match entry {
        EntryKind::Type(type_name, type_entry) => {
            let entry_sg_id = type_node_id(layer, crate_name, type_name);
            // The representative node id is the sole valid edge target for this type.
            // It is emitted as a real node inside the subgraph so that no edge ever
            // points at the subgraph container id (which would break Dagre/ELK layout).
            let rep_node_id = type_rep_node_id(layer, crate_name, type_name);

            // Build entry subgraph label: full module path + name (U-6d-iii).
            let label = build_entry_label(&type_entry.module_path, type_name);
            // Short name used as the representative node label (matches subgraph title).
            let short_name = type_name;

            // T005: entry subgraph (empty subgraph even with 0 methods, AC-02).
            subgraph_lines.push(format!("  subgraph {entry_sg_id}[\"{label}\"]"));
            subgraph_lines.push("    direction TB".to_string());

            // Emit the representative node — stands for the type itself inside the subgraph.
            // All edges that target this type must use `rep_node_id`, never `entry_sg_id`.
            let rep_shape = style.node.get("Type").and_then(|ns| ns.shape.as_deref());
            let rep_node_def = apply_shape(short_name, rep_shape);
            subgraph_lines.push(format!("    {rep_node_id}{rep_node_def}"));

            // T006: methods from TypeEntry.methods.
            // Collect transition method names for typestate (Decision G-2'b).
            let transition_method_names: Vec<&str> =
                if let TypeKindV2::PlainStruct { typestate: Some(ref ts), .. } = type_entry.kind {
                    ts.transitions().transition_methods().iter().map(|m| m.as_str()).collect()
                } else {
                    vec![]
                };

            // T007: enum variant nodes (H-3 / IN-09).
            if let TypeKindV2::Enum { ref variants } = type_entry.kind {
                for variant in variants {
                    let variant_id = format!("{entry_sg_id}_{}", sanitize(variant.name.as_str()));
                    let variant_label = variant.name.as_str();
                    let variant_node_shape =
                        style.node.get("Variant").and_then(|ns| ns.shape.as_deref());
                    let variant_node_def = apply_shape(variant_label, variant_node_shape);
                    subgraph_lines.push(format!("    {variant_id}{variant_node_def}"));

                    // Attach variant class.
                    if let Some(ns) = style.node.get("Variant") {
                        if let Some(ref class) = ns.class {
                            class_attach.push(format!("class {variant_id} {class}"));
                        }
                    }

                    // Variant payload edges (AC-05, declaration order).
                    // Edges are only emitted when the payload type resolves to a declared
                    // catalogue node (ADR 2026-04-17-1528 §D1 / CN-10 — silent skip for
                    // primitives, std, generics, impl-trait, and external types).
                    // A single type expression may yield edges to MULTIPLE declared types
                    // (e.g. `Result<A, B>` → edges to both A and B).
                    // `edge_arrow_label` is deferred until we know an edge will be emitted
                    // (fail-closed per CN-02 — missing [edge.variant_payload] only errors
                    // when the key would actually be used).
                    match &variant.payload {
                        VariantPayload::Unit => {
                            // No edge.
                        }
                        VariantPayload::Tuple(type_refs) => {
                            for tr in type_refs {
                                let target_ids = resolve_type_ref_node_ids(
                                    tr.as_str(),
                                    node_index,
                                    crate_name,
                                    None, // variant payloads have no Self context
                                );
                                for target_id in &target_ids {
                                    let (arrow, _) =
                                        edge_arrow_label(&style.edge, "variant_payload")?;
                                    edge_lines.push(edge_line(&variant_id, arrow, None, target_id));
                                }
                            }
                        }
                        VariantPayload::Struct(fields) => {
                            for field in fields {
                                let field_name = field.name.as_str();
                                let target_ids = resolve_type_ref_node_ids(
                                    field.ty.as_str(),
                                    node_index,
                                    crate_name,
                                    None, // variant fields have no Self context
                                );
                                for target_id in &target_ids {
                                    let (arrow, _) =
                                        edge_arrow_label(&style.edge, "variant_payload")?;
                                    edge_lines.push(edge_line(
                                        &variant_id,
                                        arrow,
                                        Some(field_name),
                                        target_id,
                                    ));
                                }
                            }
                        }
                    }
                }
            }

            // T006/T007: method nodes from TypeEntry.methods.
            // Pass rep_node_id as self_node_id so that `Self` TypeRef in method
            // signatures resolves to the representative node, not the subgraph.
            emit_method_nodes(
                &type_entry.methods,
                &entry_sg_id,
                &transition_method_names,
                subgraph_lines,
                edge_lines,
                class_attach,
                style,
                node_index,
                crate_name,
                Some(rep_node_id.as_str()),
            )?;

            // T006: inherent_impls methods aggregated into this type subgraph (AC-04).
            if let Some(extra_methods) = inherent_methods.get(*type_name) {
                let method_refs: Vec<&domain::tddd::catalogue_v2::methods::MethodDeclaration> =
                    extra_methods.to_vec();
                emit_method_nodes(
                    method_refs,
                    &entry_sg_id,
                    &transition_method_names,
                    subgraph_lines,
                    edge_lines,
                    class_attach,
                    style,
                    node_index,
                    crate_name,
                    Some(rep_node_id.as_str()),
                )?;
            }

            subgraph_lines.push("  end".to_string());

            // T007: TypeAlias undirected edge (N-1').
            // Edge source is the representative node (not the subgraph container id).
            // Edge emitted only when the alias target resolves to a declared catalogue node
            // (ADR 2026-04-17-1528 §D1 — silent skip for primitives / external types).
            // Uses resolve_type_ref_node_ids so that reference-wrapped alias targets
            // (e.g. `&DeclaredType`) are also resolved correctly.
            if let TypeKindV2::TypeAlias { ref target } = type_entry.kind {
                let target_ids = resolve_type_ref_node_ids(
                    target.as_str(),
                    node_index,
                    crate_name,
                    None, // type alias targets have no Self context
                );
                for target_id in &target_ids {
                    let (arrow, label) = edge_arrow_label(&style.edge, "alias")?;
                    edge_lines.push(edge_line(&rep_node_id, arrow, label, target_id));
                }
            }

            // T007: PlainStruct field edges (K-2+(d)).
            // Edge source is the representative node (not the subgraph container id).
            // Each field edge is emitted only when the target resolves to a declared catalogue
            // node (ADR 2026-04-17-1528 §D1 — silent skip for primitives / external types).
            // A single field type may yield edges to MULTIPLE declared types
            // (e.g. a field `result: Result<OkType, ErrType>` → edges to both).
            // `edge_arrow_label` is deferred until we know an edge will be emitted
            // (fail-closed per CN-02 — missing [edge.field] only errors when it would be used).
            if let TypeKindV2::PlainStruct { ref fields, has_stripped_fields, .. } = type_entry.kind
            {
                if !has_stripped_fields && !fields.is_empty() {
                    for field in fields {
                        let target_ids = resolve_type_ref_node_ids(
                            field.ty.as_str(),
                            node_index,
                            crate_name,
                            None, // struct fields have no Self context
                        );
                        for target_id in &target_ids {
                            let (arrow, _) = edge_arrow_label(&style.edge, "field")?;
                            edge_lines.push(edge_line(
                                &rep_node_id,
                                arrow,
                                Some(field.name.as_str()),
                                target_id,
                            ));
                        }
                    }
                }
            }

            // T007: TupleStruct field edges (K-2+(d)).
            // Edge source is the representative node (not the subgraph container id).
            // Each field edge is emitted only when the target resolves to a declared catalogue
            // node (ADR 2026-04-17-1528 §D1 — silent skip for primitives / external types).
            // A single positional field type may yield edges to MULTIPLE declared types.
            // `edge_arrow_label` is deferred until we know an edge will be emitted
            // (fail-closed per CN-02 — missing [edge.field] only errors when it would be used).
            if let TypeKindV2::TupleStruct { ref fields, has_stripped_fields } = type_entry.kind {
                if !has_stripped_fields && !fields.is_empty() {
                    for (idx, field_ty) in fields.iter().enumerate() {
                        let positional_label = format!(".{idx}");
                        let target_ids = resolve_type_ref_node_ids(
                            field_ty.as_str(),
                            node_index,
                            crate_name,
                            None, // tuple struct fields have no Self context
                        );
                        for target_id in &target_ids {
                            let (arrow, _) = edge_arrow_label(&style.edge, "field")?;
                            edge_lines.push(edge_line(
                                &rep_node_id,
                                arrow,
                                Some(&positional_label),
                                target_id,
                            ));
                        }
                    }
                }
            }

            // Class attach: the role class is attached to the representative node (which
            // carries the type identity) rather than the subgraph container.
            let role_key = type_entry.role.to_string();
            if let Some(rs) = style.role.get(&role_key) {
                class_attach.push(format!("class {rep_node_id} {}", rs.class));
            }

            // T009: [pattern.Typestate] overlay_class for typestate PlainStruct.
            // Also attached to the representative node.
            if let TypeKindV2::PlainStruct { typestate: Some(_), .. } = type_entry.kind {
                if let Some(ps) = style.pattern.get("Typestate") {
                    class_attach.push(format!("class {rep_node_id} {}", ps.overlay_class));
                }
            }
        }

        EntryKind::Trait(trait_name, trait_entry) => {
            let entry_sg_id = trait_node_id(layer, crate_name, trait_name);
            // The representative node id is the sole valid edge target for this trait.
            let rep_node_id = trait_rep_node_id(layer, crate_name, trait_name);
            let label = build_entry_label(&trait_entry.module_path, trait_name);
            let short_name = trait_name;

            // T005: trait entry subgraph (empty even with 0 methods, AC-02).
            subgraph_lines.push(format!("  subgraph {entry_sg_id}[\"{label}\"]"));
            subgraph_lines.push("    direction TB".to_string());

            // Emit the representative node — stands for the trait itself inside the subgraph.
            // All trait_impl edges that target this trait must use `rep_node_id`.
            let rep_shape = style.node.get("Trait").and_then(|ns| ns.shape.as_deref());
            let rep_node_def = apply_shape(short_name, rep_shape);
            subgraph_lines.push(format!("    {rep_node_id}{rep_node_def}"));

            // T008 / H'-1: TraitEntry method nodes.
            // Pass None for self_node_id: trait methods' `Self` has no fixed type resolution
            // at render time (the concrete impl type is unknown).
            emit_method_nodes(
                &trait_entry.methods,
                &entry_sg_id,
                &[], // no typestate transitions for traits
                subgraph_lines,
                edge_lines,
                class_attach,
                style,
                node_index,
                crate_name,
                None, // Self unresolvable in trait context
            )?;

            subgraph_lines.push("  end".to_string());

            // Role class attach: attached to the representative node (carries trait identity).
            let role_key = trait_entry.role.to_string();
            if let Some(rs) = style.role.get(&role_key) {
                class_attach.push(format!("class {rep_node_id} {}", rs.class));
            }
        }

        EntryKind::Function(fn_path, fn_entry) => {
            // T005: FunctionEntry standalone callable node (F-2+d1 / I-1).
            let full_path_str = fn_path.to_string();
            let fn_node_id = function_node_id(layer, crate_name, &full_path_str);
            let fn_label = fn_path.name.as_str();

            let fn_shape = style.node.get("Function").and_then(|ns| ns.shape.as_deref());
            let fn_node_def = apply_shape(fn_label, fn_shape);
            subgraph_lines.push(format!("  {fn_node_id}{fn_node_def}"));

            // Function param/return edges.
            // Edges are only emitted when the target type resolves to a declared catalogue
            // node (ADR 2026-04-17-1528 §D1 — silent skip for primitives / external types).
            // A single param/return type expression may yield edges to MULTIPLE declared types
            // (e.g. `Result<OkType, ErrType>` → edges to both).
            // `edge_arrow_label` is deferred until we know an edge will be emitted
            // (fail-closed per CN-02 — missing [edge.method_param] / [edge.method_returns]
            // only errors when the key would actually be used).
            for param in &fn_entry.params {
                let target_ids = resolve_type_ref_node_ids(
                    param.ty.as_str(),
                    node_index,
                    crate_name,
                    None, // free functions have no Self context
                );
                for target_id in &target_ids {
                    let (param_arrow, param_label) = edge_arrow_label(&style.edge, "method_param")?;
                    edge_lines.push(edge_line(&fn_node_id, param_arrow, param_label, target_id));
                }
            }
            let ret_targets = resolve_type_ref_node_ids(
                fn_entry.returns.as_str(),
                node_index,
                crate_name,
                None, // free functions have no Self context
            );
            for ret_target in &ret_targets {
                let (ret_arrow, ret_label) = edge_arrow_label(&style.edge, "method_returns")?;
                edge_lines.push(edge_line(&fn_node_id, ret_arrow, ret_label, ret_target));
            }

            // Class attach.
            let role_key = fn_entry.role.to_string();
            if let Some(rs) = style.role.get(&role_key) {
                class_attach.push(format!("class {fn_node_id} {}", rs.class));
            }
            if let Some(ns) = style.node.get("Function") {
                if let Some(ref class) = ns.class {
                    class_attach.push(format!("class {fn_node_id} {class}"));
                }
            }
        }
    }

    Ok(())
}

/// Build an entry subgraph label that includes sub-module path (U-6d-iii).
///
/// For module_path = ["team", "manager"] and name "TeamManager", produces
/// "team::manager::TeamManager".
fn build_entry_label(
    module_path: &domain::tddd::catalogue_v2::identifiers::ModulePath,
    name: &str,
) -> String {
    if module_path.is_root() { name.to_string() } else { format!("{module_path}::{name}") }
}

/// Resolve a TypeRef target node, substituting `self_node_id` for the `"Self"` keyword
/// in both top-level and nested positions.
///
/// Rust method signatures commonly use `Self` as the return type (e.g. builder
/// patterns, `Clone::clone`) as well as in nested positions (`Option<Self>`,
/// `Result<Self, E>`).  `NodeIndex` never holds a `"Self"` key, so without special
/// handling any occurrence of `Self` — top-level or nested — would silently drop the
/// edge.  When `self_node_id` is `Some`, `"Self"` resolves to the enclosing entry's
/// subgraph node, wiring the edge to the actual type node.
///
/// Returns an empty `Vec` when the target contains no declared catalogue nodes —
/// callers must treat an empty result as "skip this edge" (consistent with
/// ADR 2026-04-17-1528 §D1 and CN-10 / AC-06).
fn resolve_method_type_refs(
    type_ref_str: &str,
    node_index: &NodeIndex,
    current_crate: &str,
    self_node_id: Option<&str>,
) -> Vec<String> {
    // Delegate to resolve_type_ref_node_ids, forwarding self_node_id so that
    // "Self" in both top-level (`"Self"`) and nested (`"Option<Self>"`,
    // `"Result<Self, E>"`) positions is substituted correctly.
    resolve_type_ref_node_ids(type_ref_str, node_index, current_crate, self_node_id)
}

/// Emit method nodes inside an entry subgraph.
///
/// # Errors
///
/// Returns `ContractMapRendererError::RenderFailed` when a required `[edge.*]`
/// key is absent from the style configuration (CN-02 — fail-closed, no hard-coded
/// fallback).
#[allow(clippy::too_many_arguments)]
pub(super) fn emit_method_nodes<'a>(
    methods: impl IntoIterator<Item = &'a domain::tddd::catalogue_v2::methods::MethodDeclaration>,
    entry_sg_id: &str,
    transition_method_names: &[&str],
    subgraph_lines: &mut Vec<String>,
    edge_lines: &mut Vec<String>,
    class_attach: &mut Vec<String>,
    style: &StyleConfig,
    node_index: &NodeIndex,
    current_crate: &str,
    // The node_id of the enclosing entry subgraph, used to resolve `"Self"` TypeRef
    // to the current type's node instead of a ghost node (OS-04 / fallback policy).
    // Pass `None` for trait method nodes where `Self` has no fixed resolution.
    self_node_id: Option<&str>,
) -> Result<(), ContractMapRendererError> {
    for method in methods {
        let method_name_str = method.name.as_str();
        let method_node_id = format!("{entry_sg_id}_{}", sanitize(method_name_str));

        // Method node shape from [node.Method].
        let method_shape = style.node.get("Method").and_then(|ns| ns.shape.as_deref());
        let method_node_def = apply_shape(method_name_str, method_shape);
        subgraph_lines.push(format!("    {method_node_id}{method_node_def}"));

        // Attach method class.
        if let Some(ns) = style.node.get("Method") {
            if let Some(ref class) = ns.class {
                class_attach.push(format!("class {method_node_id} {class}"));
            }
        }

        // Param edges: method_node --o param_type (Decision F-2+b2-ii).
        // Edges emitted only when the target resolves to a declared catalogue node
        // (ADR 2026-04-17-1528 §D1 — silent skip for primitives / external types).
        // A single param type may yield edges to MULTIPLE declared types
        // (e.g. `&DeclaredType`, `Result<A, B>` → edges to each resolved type).
        // `edge_arrow_label` is deferred until we know an edge will be emitted
        // (fail-closed per CN-02 — missing [edge.method_param] only errors when
        // the key would actually be used).
        for param in &method.params {
            let target_ids = resolve_method_type_refs(
                param.ty.as_str(),
                node_index,
                current_crate,
                self_node_id,
            );
            for target_id in &target_ids {
                let (param_arrow, param_label) = edge_arrow_label(&style.edge, "method_param")?;
                edge_lines.push(edge_line(&method_node_id, param_arrow, param_label, target_id));
            }
        }

        // Returns edge: normal --> or typestate transition ==> (Decision G-2'b, AC-03).
        // A return type may yield edges to MULTIPLE declared types (e.g. `Result<A, B>`).
        // Edge emitted only when the target resolves to a declared catalogue node
        // (ADR 2026-04-17-1528 §D1 — silent skip for primitives / external types).
        // `edge_arrow_label` is deferred until we know an edge will be emitted
        // (fail-closed per CN-02 — missing [edge.method_returns] / [edge.transition]
        // only errors when the key would actually be used).
        let returns_targets = resolve_method_type_refs(
            method.returns.as_str(),
            node_index,
            current_crate,
            self_node_id,
        );
        let is_transition = transition_method_names.contains(&method_name_str);
        if is_transition {
            for target_id in &returns_targets {
                let (arrow, label) = edge_arrow_label(&style.edge, "transition")?;
                edge_lines.push(edge_line(&method_node_id, arrow, label, target_id));
            }
        } else {
            for target_id in &returns_targets {
                let (arrow, label) = edge_arrow_label(&style.edge, "method_returns")?;
                edge_lines.push(edge_line(&method_node_id, arrow, label, target_id));
            }
        }
    }

    Ok(())
}
