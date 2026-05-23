//! Entry subgraph emission for the baseline-graph renderer (T007 / T008 / T015 / T016).
//!
//! Implements the following ADR decisions:
//!
//! - **F-r1**: Struct / Enum / Trait / TypeAlias entries are rendered as mermaid
//!   subgraphs. FunctionEntry is rendered as a standalone callable node (out of scope here).
//! - **H**: Enum variants are node-ified inside the entry subgraph. Payload edges:
//!   `VariantKind::Tuple` → one `--o` edge per own-crate type found by recursive
//!   `ResolvedPath.args` traversal; `VariantKind::Struct` → one `--o|field_name|` edge
//!   per own-crate type; `VariantKind::Plain` → no edge.
//!   `Type::Primitive` / `Type::Generic` / external types produce no edge (T016 / AC-20).
//! - **H'**: Trait.items are scanned for `ItemEnum::Function` entries; each becomes a
//!   method node inside the Trait subgraph.
//! - **K**: PlainStruct fields → `--o|field_name|` edge per own-crate type found by
//!   recursive `ResolvedPath.args` traversal; TupleStruct fields → `--o|.N|` per own-crate
//!   type; `has_stripped_fields: true` or `None` slot → skip; Unit → no edge.
//!   `Type::Primitive` / `Type::Generic` / external types produce no edge (T016 / AC-20).
//!   Anonymous nodes (`prim_*` / `generic_*` / `anon_*`) are no longer generated (T016).
//! - **N**: TypeAlias → undirected `---|alias_of|` edge to each own-crate type found by
//!   recursive `ResolvedPath.args` traversal of the alias target type.
//!   `Type::Primitive` / `Type::Generic` / external types produce no edge (T016 / AC-20).
//!   Anonymous nodes (`prim_*` / `generic_*` / `anon_*`) are no longer generated (T016).
//! - **BB-4-fix1 (T008)**: Inherent impl methods are merged into the target type's entry
//!   subgraph. The `emit_*_subgraph` functions accept an optional `inherent_method_ids`
//!   slice; when provided, those method nodes are emitted inside the subgraph before `end`.
//! - **AC-19 / AC-20 (T015 — method path)**: inherent method nodes (BB, via
//!   `impl_processor::emit_inherent_methods`) and Trait method nodes (H', via
//!   `emit_trait_subgraph`) emit `method_param` / `method_returns` edges to own-crate
//!   types extracted from `FunctionSignature.inputs` / `.output` by recursive
//!   `ResolvedPath.args` traversal (`collect_own_crate_node_ids_from_type`).
//!   `Type::Primitive` / `Type::Generic` / external types produce no edge.
//! - **AC-20 (T016 — existing edge paths)**: struct field (K), enum variant payload (H),
//!   and TypeAlias target (N) edges are now resolved via the same recursive
//!   `collect_own_crate_node_ids_from_type` utility as method edges (T015).
//!   Anonymous nodes (`prim_*` / `generic_*` / `anon_*`) are eliminated from all edge
//!   paths. Only own-crate types (entry subgraph present) receive edges.
//!
//! All functions are panic-free (no `unwrap` / `expect` / slice indexing on `[i]`
//! in production code — only `.get()` / iterators).
//!
//! (IN-06 / IN-07 / IN-08 / IN-09 / IN-10 / IN-11 / IN-13 / AC-04 / AC-05 / AC-06 /
//! AC-07 / AC-08 / AC-09 / AC-10 / AC-19 / AC-20)

use std::collections::BTreeMap;

use rustdoc_types::{Id, ItemEnum, StructKind, VariantKind, Visibility};

use domain::tddd::baseline_document::BaselineDocument;
use domain::tddd::baseline_graph_ports::BaselineGraphRendererError;

use super::impl_processor::{
    BlanketBodyEntry, build_blanket_body_map, build_inherent_method_map, build_trait_index,
    collect_own_crate_node_ids_from_type, emit_all_impl_edges_for_layer,
};
use super::node_id_generator::{
    function_node_id, module_path_from_summary, trait_node_id, trait_rep_node_id, type_node_id,
    type_rep_node_id,
};
use super::{StyleConfig, apply_shape, edge_arrow_label, sanitize};

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Emit a Struct entry subgraph (F-r1 / K / BB-4-fix1 decision).
///
/// Struct / Enum / Trait / TypeAlias become subgraphs; FunctionEntry is handled
/// separately as a standalone node.
///
/// `inherent_method_ids` — when `Some`, the method Ids (collected by
/// [`super::impl_processor::collect_inherent_methods`] for this type's rep node id)
/// are emitted as method nodes inside the subgraph (BB-4-fix1, T008).
///
/// # Errors
///
/// Returns `BaselineGraphRendererError::RenderFailed` when a required `[edge.*]`
/// key is absent from the style config (CN-02 — fail-closed).
#[allow(clippy::too_many_arguments)]
pub(super) fn emit_struct_subgraph(
    doc: &BaselineDocument,
    id: Id,
    layer: &str,
    subgraph_lines: &mut Vec<String>,
    edge_lines: &mut Vec<String>,
    class_attach: &mut Vec<String>,
    style: &StyleConfig,
    indent: &str,
    inherent_method_ids: Option<&[Id]>,
) -> Result<(), BaselineGraphRendererError> {
    let krate = &doc.krate;

    // Retrieve the Item from the index — skip silently if absent.
    let item = match krate.index.get(&id) {
        Some(i) => i,
        None => return Ok(()),
    };
    let name = match item.name.as_deref() {
        Some(n) => n,
        None => return Ok(()),
    };

    // Derive module_path for the node_id (D decision) and summary_path for display label.
    let summary_path_opt = krate.paths.get(&id).map(|s| s.path.as_slice());
    let module_path = summary_path_opt.map(module_path_from_summary).unwrap_or_default();

    let crate_name = doc.crate_name.as_str();
    let entry_sg_id = type_node_id(layer, crate_name, &module_path, name);
    let rep_node_id = type_rep_node_id(layer, crate_name, &module_path, name);

    // Subgraph label (full module-qualified name, U-6d-iii).
    // Uses the raw summary path to avoid conflating segment-internal underscores with "::" separators.
    let label =
        summary_path_opt.map(|p| build_entry_label(p, name)).unwrap_or_else(|| name.to_string());

    // Open subgraph (F-r1).
    subgraph_lines.push(format!("{indent}subgraph {entry_sg_id}[\"{label}\"]"));
    subgraph_lines.push(format!("{indent}  direction TB"));

    // Representative node so edges can target a concrete node inside the subgraph.
    let rep_shape = style.node.get("Type").and_then(|ns| ns.shape.as_deref());
    let rep_node_def = apply_shape(name, rep_shape);
    subgraph_lines.push(format!("{indent}  {rep_node_id}{rep_node_def}"));

    // Attach representative node class.
    if let Some(ns) = style.node.get("Type") {
        if let Some(class_name) = ns.class.as_deref() {
            class_attach.push(format!("class {rep_node_id} {class_name}"));
        }
    }

    // K decision: struct fields (T016 / AC-20).
    // Type resolution uses collect_own_crate_node_ids_from_type (recursive ResolvedPath.args
    // traversal, same utility as method edges in T015). Only own-crate types (crate_id == 0,
    // rendered entry subgraph) receive edges. Primitive / generic / external types are silently
    // skipped — no anonymous node (`prim_*` / `generic_*` / `anon_*`) is generated.
    // The [edge.field] style is looked up lazily — only immediately before the first edge is
    // emitted — so that structs with no renderable fields do not fail when [edge.field] is
    // absent from the style config (CN-02 fail-closed).
    if let ItemEnum::Struct(s) = &item.inner {
        match &s.kind {
            StructKind::Plain { fields, has_stripped_fields } => {
                if !has_stripped_fields {
                    for &field_id in fields {
                        if let Some(field_item) = krate.index.get(&field_id) {
                            let field_name = field_item.name.as_deref().unwrap_or("?");
                            if let ItemEnum::StructField(field_ty) = &field_item.inner {
                                let targets = collect_own_crate_node_ids_from_type(
                                    field_ty, krate, layer, crate_name,
                                );
                                for target_id in &targets {
                                    // Lazy lookup: only when an edge is about to be emitted.
                                    let (field_arrow, _) = edge_arrow_label(&style.edge, "field")?;
                                    edge_lines.push(format!(
                                        "{rep_node_id} {field_arrow}|\"{field_name}\"| {target_id}"
                                    ));
                                }
                            }
                        }
                    }
                }
                // has_stripped_fields == true → skip (K decision)
            }
            StructKind::Tuple(fields) => {
                for (idx, maybe_id) in fields.iter().enumerate() {
                    // None slot = stripped field → skip (K decision).
                    if let Some(&field_id) = maybe_id.as_ref() {
                        if let Some(field_item) = krate.index.get(&field_id) {
                            if let ItemEnum::StructField(field_ty) = &field_item.inner {
                                let targets = collect_own_crate_node_ids_from_type(
                                    field_ty, krate, layer, crate_name,
                                );
                                let label = format!(".{idx}");
                                for target_id in &targets {
                                    // Lazy lookup: only when an edge is about to be emitted.
                                    let (field_arrow, _) = edge_arrow_label(&style.edge, "field")?;
                                    edge_lines.push(format!(
                                        "{rep_node_id} {field_arrow}|\"{label}\"| {target_id}"
                                    ));
                                }
                            }
                        }
                    }
                }
            }
            StructKind::Unit => {
                // Unit → no edge (K decision).
            }
        }
    }

    // BB-4-fix1 (T008) + T015: emit inherent method nodes inside the subgraph,
    // and method_param / method_returns edges from each method node.
    if let Some(method_ids) = inherent_method_ids {
        let child_indent = format!("{indent}  ");
        super::impl_processor::emit_inherent_methods(
            method_ids,
            krate,
            &entry_sg_id,
            layer,
            crate_name,
            subgraph_lines,
            edge_lines,
            class_attach,
            style,
            &child_indent,
        )?;
    }

    // Close subgraph.
    subgraph_lines.push(format!("{indent}end"));

    // Attach subgraph class (separate `class` line — inline :::className causes parse error).
    if let Some(ns) = style.node.get("Type") {
        if let Some(class_name) = ns.class.as_deref() {
            class_attach.push(format!("class {entry_sg_id} {class_name}"));
        }
    }

    Ok(())
}

/// Emit an Enum entry subgraph with variant nodes and payload edges (F-r1 / H decision).
///
/// # Errors
///
/// Returns `BaselineGraphRendererError::RenderFailed` when a required `[edge.*]`
/// key is absent from the style config (CN-02 — fail-closed).
#[allow(clippy::too_many_arguments)]
pub(super) fn emit_enum_subgraph(
    doc: &BaselineDocument,
    id: Id,
    layer: &str,
    subgraph_lines: &mut Vec<String>,
    edge_lines: &mut Vec<String>,
    class_attach: &mut Vec<String>,
    style: &StyleConfig,
    indent: &str,
    inherent_method_ids: Option<&[Id]>,
) -> Result<(), BaselineGraphRendererError> {
    let krate = &doc.krate;

    let item = match krate.index.get(&id) {
        Some(i) => i,
        None => return Ok(()),
    };
    let name = match item.name.as_deref() {
        Some(n) => n,
        None => return Ok(()),
    };

    let summary_path_opt = krate.paths.get(&id).map(|s| s.path.as_slice());
    let module_path = summary_path_opt.map(module_path_from_summary).unwrap_or_default();

    let crate_name = doc.crate_name.as_str();
    let entry_sg_id = type_node_id(layer, crate_name, &module_path, name);
    let rep_node_id = type_rep_node_id(layer, crate_name, &module_path, name);

    let label =
        summary_path_opt.map(|p| build_entry_label(p, name)).unwrap_or_else(|| name.to_string());

    // Open subgraph.
    subgraph_lines.push(format!("{indent}subgraph {entry_sg_id}[\"{label}\"]"));
    subgraph_lines.push(format!("{indent}  direction TB"));

    // Representative node.
    let rep_shape = style.node.get("Type").and_then(|ns| ns.shape.as_deref());
    let rep_node_def = apply_shape(name, rep_shape);
    subgraph_lines.push(format!("{indent}  {rep_node_id}{rep_node_def}"));

    if let Some(ns) = style.node.get("Type") {
        if let Some(class_name) = ns.class.as_deref() {
            class_attach.push(format!("class {rep_node_id} {class_name}"));
        }
    }

    // H decision: enum variant nodes.
    // The [edge.variant_payload] style is looked up lazily — only immediately before
    // the first payload edge is emitted — so that enums with only Plain variants (or no
    // variants) do not fail when [edge.variant_payload] is absent from the style config.
    if let ItemEnum::Enum(enum_data) = &item.inner {
        let variant_shape = style.node.get("Variant").and_then(|ns| ns.shape.as_deref());

        for &variant_id in &enum_data.variants {
            let variant_item = match krate.index.get(&variant_id) {
                Some(v) => v,
                None => continue,
            };
            // CC-1 exception: enum variants use Visibility::Default — accepted when parent is Public.
            if !matches!(variant_item.visibility, Visibility::Public | Visibility::Default) {
                continue;
            }
            let variant_name = match variant_item.name.as_deref() {
                Some(n) => n,
                None => continue,
            };

            let variant_node_id = format!("{entry_sg_id}_{}", sanitize(variant_name));
            let variant_node_def = apply_shape(variant_name, variant_shape);
            subgraph_lines.push(format!("{indent}  {variant_node_id}{variant_node_def}"));

            if let Some(ns) = style.node.get("Variant") {
                if let Some(class_name) = ns.class.as_deref() {
                    class_attach.push(format!("class {variant_node_id} {class_name}"));
                }
            }

            // Payload edges (H decision, T016 / AC-20).
            // Type resolution uses collect_own_crate_node_ids_from_type (recursive
            // ResolvedPath.args traversal). Only own-crate types receive edges.
            // Primitive / generic / external types → silent skip (no anon node).
            if let ItemEnum::Variant(variant_data) = &variant_item.inner {
                match &variant_data.kind {
                    VariantKind::Plain => {
                        // No edge.
                    }
                    VariantKind::Tuple(field_ids) => {
                        // Each Some(Id) → lookup StructField → one `--o` edge per own-crate type.
                        for maybe_id in field_ids {
                            if let Some(&fid) = maybe_id.as_ref() {
                                if let Some(field_item) = krate.index.get(&fid) {
                                    if let ItemEnum::StructField(field_ty) = &field_item.inner {
                                        let targets = collect_own_crate_node_ids_from_type(
                                            field_ty, krate, layer, crate_name,
                                        );
                                        for target_id in &targets {
                                            // Lazy lookup: only when an edge is about to be emitted.
                                            let (payload_arrow, _) =
                                                edge_arrow_label(&style.edge, "variant_payload")?;
                                            edge_lines.push(format!(
                                                "{variant_node_id} {payload_arrow} {target_id}"
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    VariantKind::Struct { fields, has_stripped_fields } => {
                        // Skip edge emission when the variant has stripped (hidden) fields
                        // (consistent with K decision: has_stripped_fields → render nothing).
                        if !has_stripped_fields {
                            // Each field → one `--o|field_name|` edge per own-crate type (H decision).
                            for &fid in fields {
                                if let Some(field_item) = krate.index.get(&fid) {
                                    let field_name = field_item.name.as_deref().unwrap_or("?");
                                    if let ItemEnum::StructField(field_ty) = &field_item.inner {
                                        let targets = collect_own_crate_node_ids_from_type(
                                            field_ty, krate, layer, crate_name,
                                        );
                                        for target_id in &targets {
                                            // Lazy lookup: only when an edge is about to be emitted.
                                            let (payload_arrow, _) =
                                                edge_arrow_label(&style.edge, "variant_payload")?;
                                            edge_lines.push(format!(
                                                "{variant_node_id} {payload_arrow}|\"{field_name}\"| {target_id}"
                                            ));
                                        }
                                    }
                                }
                            }
                        }
                        // has_stripped_fields == true → skip (consistent with K decision).
                    }
                }
            }
        }
    }

    // BB-4-fix1 (T008) + T015: emit inherent method nodes inside the subgraph,
    // and method_param / method_returns edges from each method node.
    if let Some(method_ids) = inherent_method_ids {
        let child_indent = format!("{indent}  ");
        super::impl_processor::emit_inherent_methods(
            method_ids,
            krate,
            &entry_sg_id,
            layer,
            crate_name,
            subgraph_lines,
            edge_lines,
            class_attach,
            style,
            &child_indent,
        )?;
    }

    // Close subgraph.
    subgraph_lines.push(format!("{indent}end"));

    if let Some(ns) = style.node.get("Type") {
        if let Some(class_name) = ns.class.as_deref() {
            class_attach.push(format!("class {entry_sg_id} {class_name}"));
        }
    }

    Ok(())
}

/// Emit a Trait entry subgraph with method nodes (F-r1 / H' decision / T015).
///
/// `blanket_entries` — when `Some`, blanket body indicator nodes collected by
/// [`super::impl_processor::build_blanket_body_map`] are emitted inside the subgraph
/// before its `end` line (a-plan, AC-17 / BB-4-fix1). Each entry contains a pre-built
/// `node_id`, `node_def`, and an optional class attach statement.
///
/// **T015 (AC-19 / AC-20 — method path)**: for each Trait method node emitted (H'),
/// the function also emits `method_param` edges from the method node to own-crate types
/// in `FunctionSignature.inputs`, and `method_returns` edges to own-crate types in
/// `FunctionSignature.output`.  Edge styles are resolved from `[edge.method_param]` and
/// `[edge.method_returns]` (fail-closed, CN-02).
///
/// # Errors
///
/// Returns `BaselineGraphRendererError::RenderFailed` when a required `[edge.*]`
/// key is absent from the style config (CN-02 — fail-closed).
#[allow(clippy::too_many_arguments)]
pub(super) fn emit_trait_subgraph(
    doc: &BaselineDocument,
    id: Id,
    layer: &str,
    subgraph_lines: &mut Vec<String>,
    edge_lines: &mut Vec<String>,
    class_attach: &mut Vec<String>,
    style: &StyleConfig,
    indent: &str,
    blanket_entries: Option<&[BlanketBodyEntry]>,
) -> Result<(), BaselineGraphRendererError> {
    let krate = &doc.krate;

    let item = match krate.index.get(&id) {
        Some(i) => i,
        None => return Ok(()),
    };
    let name = match item.name.as_deref() {
        Some(n) => n,
        None => return Ok(()),
    };

    let summary_path_opt = krate.paths.get(&id).map(|s| s.path.as_slice());
    let module_path = summary_path_opt.map(module_path_from_summary).unwrap_or_default();

    let crate_name = doc.crate_name.as_str();
    let entry_sg_id = trait_node_id(layer, crate_name, &module_path, name);
    let rep_node_id = trait_rep_node_id(layer, crate_name, &module_path, name);

    let label =
        summary_path_opt.map(|p| build_entry_label(p, name)).unwrap_or_else(|| name.to_string());

    // Open subgraph.
    subgraph_lines.push(format!("{indent}subgraph {entry_sg_id}[\"{label}\"]"));
    subgraph_lines.push(format!("{indent}  direction TB"));

    // Representative node.
    let rep_shape = style.node.get("Trait").and_then(|ns| ns.shape.as_deref());
    let rep_node_def = apply_shape(name, rep_shape);
    subgraph_lines.push(format!("{indent}  {rep_node_id}{rep_node_def}"));

    if let Some(ns) = style.node.get("Trait") {
        if let Some(class_name) = ns.class.as_deref() {
            class_attach.push(format!("class {rep_node_id} {class_name}"));
        }
    }

    // H' decision: Trait.items — extract Function variants as method nodes.
    // T015 (AC-19 / AC-20): also emit method_param / method_returns edges.
    let crate_name = doc.crate_name.as_str();
    if let ItemEnum::Trait(trait_data) = &item.inner {
        let method_shape = style.node.get("Method").and_then(|ns| ns.shape.as_deref());

        for &method_item_id in &trait_data.items {
            let method_item = match krate.index.get(&method_item_id) {
                Some(m) => m,
                None => continue,
            };
            // CC-1 exception: trait methods use Visibility::Default — accepted.
            if !matches!(method_item.visibility, Visibility::Public | Visibility::Default) {
                continue;
            }
            let fn_data = match &method_item.inner {
                ItemEnum::Function(f) => f,
                _ => continue,
            };
            let method_name = match method_item.name.as_deref() {
                Some(n) => n,
                None => continue,
            };

            let method_node_id = format!("{entry_sg_id}_{}", sanitize(method_name));
            let method_node_def = apply_shape(method_name, method_shape);
            subgraph_lines.push(format!("{indent}  {method_node_id}{method_node_def}"));

            if let Some(ns) = style.node.get("Method") {
                if let Some(class_name) = ns.class.as_deref() {
                    class_attach.push(format!("class {method_node_id} {class_name}"));
                }
            }

            // T015 (AC-19 / AC-20): emit method_param / method_returns edges.
            super::impl_processor::emit_method_signature_edges(
                &fn_data.sig,
                &method_node_id,
                krate,
                layer,
                crate_name,
                edge_lines,
                style,
            )?;
        }
    }

    // BB-4-fix1 / a-plan (AC-17): inject blanket body indicator nodes inside the trait
    // subgraph before `end`. Nodes were pre-collected by `build_blanket_body_map` so
    // they appear inside this subgraph block, not as disconnected top-level nodes.
    if let Some(entries) = blanket_entries {
        let child_indent = format!("{indent}  ");
        for entry in entries {
            subgraph_lines.push(format!("{child_indent}{}{}", entry.node_id, entry.node_def));
            if let Some(ref ca) = entry.class_attach_line {
                class_attach.push(ca.clone());
            }
        }
    }

    // Close subgraph.
    subgraph_lines.push(format!("{indent}end"));

    if let Some(ns) = style.node.get("Trait") {
        if let Some(class_name) = ns.class.as_deref() {
            class_attach.push(format!("class {entry_sg_id} {class_name}"));
        }
    }

    Ok(())
}

/// Emit a TypeAlias entry subgraph with alias edge (F-r1 / N decision).
///
/// # Errors
///
/// Returns `BaselineGraphRendererError::RenderFailed` when a required `[edge.*]`
/// key is absent from the style config (CN-02 — fail-closed).
#[allow(clippy::too_many_arguments)]
pub(super) fn emit_type_alias_subgraph(
    doc: &BaselineDocument,
    id: Id,
    layer: &str,
    subgraph_lines: &mut Vec<String>,
    edge_lines: &mut Vec<String>,
    class_attach: &mut Vec<String>,
    style: &StyleConfig,
    indent: &str,
    inherent_method_ids: Option<&[Id]>,
) -> Result<(), BaselineGraphRendererError> {
    let krate = &doc.krate;

    let item = match krate.index.get(&id) {
        Some(i) => i,
        None => return Ok(()),
    };
    let name = match item.name.as_deref() {
        Some(n) => n,
        None => return Ok(()),
    };

    let summary_path_opt = krate.paths.get(&id).map(|s| s.path.as_slice());
    let module_path = summary_path_opt.map(module_path_from_summary).unwrap_or_default();

    let crate_name = doc.crate_name.as_str();
    let entry_sg_id = type_node_id(layer, crate_name, &module_path, name);
    let rep_node_id = type_rep_node_id(layer, crate_name, &module_path, name);

    let label =
        summary_path_opt.map(|p| build_entry_label(p, name)).unwrap_or_else(|| name.to_string());

    // Open subgraph.
    subgraph_lines.push(format!("{indent}subgraph {entry_sg_id}[\"{label}\"]"));
    subgraph_lines.push(format!("{indent}  direction TB"));

    // Representative node.
    let rep_shape = style.node.get("Type").and_then(|ns| ns.shape.as_deref());
    let rep_node_def = apply_shape(name, rep_shape);
    subgraph_lines.push(format!("{indent}  {rep_node_id}{rep_node_def}"));

    if let Some(ns) = style.node.get("Type") {
        if let Some(class_name) = ns.class.as_deref() {
            class_attach.push(format!("class {rep_node_id} {class_name}"));
        }
    }

    // BB-4-fix1 (T008) + T015: emit inherent method nodes inside the subgraph,
    // and method_param / method_returns edges from each method node.
    if let Some(method_ids) = inherent_method_ids {
        let child_indent = format!("{indent}  ");
        super::impl_processor::emit_inherent_methods(
            method_ids,
            krate,
            &entry_sg_id,
            layer,
            crate_name,
            subgraph_lines,
            edge_lines,
            class_attach,
            style,
            &child_indent,
        )?;
    }

    // Close subgraph.
    subgraph_lines.push(format!("{indent}end"));

    if let Some(ns) = style.node.get("Type") {
        if let Some(class_name) = ns.class.as_deref() {
            class_attach.push(format!("class {entry_sg_id} {class_name}"));
        }
    }

    // N decision: undirected alias_of edge(s) to own-crate target types (T016 / AC-20).
    // Type resolution uses collect_own_crate_node_ids_from_type (recursive ResolvedPath.args
    // traversal). Only own-crate types receive edges. Primitive / generic / external types →
    // silent skip (no anonymous node). One edge per own-crate type found in the alias target.
    if let ItemEnum::TypeAlias(alias_data) = &item.inner {
        let targets =
            collect_own_crate_node_ids_from_type(&alias_data.type_, krate, layer, crate_name);
        for target_id in &targets {
            let (alias_arrow, _) = edge_arrow_label(&style.edge, "alias")?;
            edge_lines.push(format!("{rep_node_id} {alias_arrow}|\"alias_of\"| {target_id}"));
        }
    }

    Ok(())
}

/// Emit a FunctionEntry as a standalone callable node (F-r1: Function is NOT subgraphed).
///
/// # Errors
///
/// This function is currently infallible (no edge style lookup required).
pub(super) fn emit_function_node(
    doc: &BaselineDocument,
    id: Id,
    layer: &str,
    subgraph_lines: &mut Vec<String>,
    class_attach: &mut Vec<String>,
    style: &StyleConfig,
    indent: &str,
) -> Result<(), BaselineGraphRendererError> {
    let krate = &doc.krate;

    let item = match krate.index.get(&id) {
        Some(i) => i,
        None => return Ok(()),
    };
    let name = match item.name.as_deref() {
        Some(n) => n,
        None => return Ok(()),
    };

    // Derive the full path for the node_id (D decision).
    let full_path = match krate.paths.get(&id) {
        Some(summary) => summary.path.join("::"),
        None => format!("{}::{}", doc.crate_name.as_str(), name),
    };

    let crate_name = doc.crate_name.as_str();
    let node_id = function_node_id(layer, crate_name, &full_path);

    let fn_shape = style.node.get("Function").and_then(|ns| ns.shape.as_deref());
    let node_def = apply_shape(name, fn_shape);
    subgraph_lines.push(format!("{indent}{node_id}{node_def}"));

    if let Some(ns) = style.node.get("Function") {
        if let Some(class_name) = ns.class.as_deref() {
            class_attach.push(format!("class {node_id} {class_name}"));
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Batch emission: render all entries for a given layer+crate into output buffers
// ---------------------------------------------------------------------------

/// Render all B-r1 entry nodes for the given baselines+layer into three output buffers.
///
/// `subgraph_lines` receives subgraph/node definitions (to be placed inside a
/// top-module subgraph by the caller).
/// `edge_lines` receives edge definitions to be emitted after the subgraph block.
/// `class_attach` receives `class <id> <className>` attach statements.
///
/// When `crate_filter` is `Some(crate_name)`, only baselines whose crate_name matches
/// are processed. This ensures that each cluster file contains only the entries for its
/// own crate and not all crates in the layer.
///
/// When `cluster_key_filter` is `Some(ck)`, only entries whose `ItemSummary.path`
/// maps to the given cluster key are rendered. This is used for T010 depth-2 cluster
/// rendering where each cluster file contains only the entries for its own cluster
/// (crate_name × top-level module). Pass `None` to include all entries for the crate.
///
/// Entries are sorted alphabetically by their fully-qualified key (module_path + name)
/// within each crate (CN-08). Functions are emitted after the subgraph entries
/// (alphabetical by full path, CN-08).
///
/// # Errors
///
/// Returns `BaselineGraphRendererError::RenderFailed` when a required `[edge.*]`
/// key is absent from the style config (CN-02 — fail-closed).
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(super) fn emit_all_entries_for_layer(
    baselines: &[BaselineDocument],
    layer: &str,
    crate_filter: Option<&str>,
    subgraph_lines: &mut Vec<String>,
    edge_lines: &mut Vec<String>,
    class_attach: &mut Vec<String>,
    style: &StyleConfig,
    indent: &str,
) -> Result<(), BaselineGraphRendererError> {
    emit_entries_for_layer_and_cluster(
        baselines,
        layer,
        crate_filter,
        None,
        subgraph_lines,
        edge_lines,
        class_attach,
        style,
        indent,
    )
}

/// Render B-r1 entry nodes for a specific cluster (crate_name × top-level module).
///
/// Equivalent to [`emit_all_entries_for_layer`] but additionally filters entries to
/// only those whose `ItemSummary.path` maps to `cluster_key` (via the cluster
/// enumeration logic: `path[1]` = top-level module, or `"root"` for crate-root entries).
///
/// Used by T010 (depth-2 cluster renderer): each cluster detail file renders only
/// the entries belonging to that specific cluster. Cross-cluster edges emitted by
/// [`emit_all_entries_for_layer`] are suppressed via the caller's edge post-filter.
///
/// # Errors
///
/// Returns `BaselineGraphRendererError::RenderFailed` when a required `[edge.*]`
/// key is absent from the style config (CN-02 — fail-closed).
#[allow(clippy::too_many_arguments)]
pub(super) fn emit_entries_for_cluster(
    baselines: &[BaselineDocument],
    layer: &str,
    crate_name: &str,
    cluster_module_seg1: &str,
    subgraph_lines: &mut Vec<String>,
    edge_lines: &mut Vec<String>,
    class_attach: &mut Vec<String>,
    style: &StyleConfig,
    indent: &str,
) -> Result<(), BaselineGraphRendererError> {
    emit_entries_for_layer_and_cluster(
        baselines,
        layer,
        Some(crate_name),
        Some(cluster_module_seg1),
        subgraph_lines,
        edge_lines,
        class_attach,
        style,
        indent,
    )
}

/// Internal implementation shared by [`emit_all_entries_for_layer`] and
/// [`emit_entries_for_cluster`].
///
/// When `cluster_module_seg1_filter` is `Some(seg1)`, only entries whose
/// `ItemSummary.path[1]` equals `seg1` (or `"root"` for crate-root entries when
/// `seg1 == "root"`) are rendered.
#[allow(clippy::too_many_arguments)]
fn emit_entries_for_layer_and_cluster(
    baselines: &[BaselineDocument],
    layer: &str,
    crate_filter: Option<&str>,
    cluster_module_seg1_filter: Option<&str>,
    subgraph_lines: &mut Vec<String>,
    edge_lines: &mut Vec<String>,
    class_attach: &mut Vec<String>,
    style: &StyleConfig,
    indent: &str,
) -> Result<(), BaselineGraphRendererError> {
    use super::node_extractor::{ExtractedNode, extract_nodes};

    // Extract all B-r1 nodes for this layer.
    let layer_id = domain::tddd::layer_id::LayerId::try_new(layer).map_err(|_| {
        BaselineGraphRendererError::RenderFailed { reason: format!("invalid layer name: {layer}") }
    })?;

    let nodes = extract_nodes(baselines, &layer_id);

    // T008 (O-r1, CN-04): build per-render-call indices once before the entry loop.
    // These are local to this invocation — not cached/stored long-term.
    let trait_index = build_trait_index(baselines, layer);
    let inherent_map = build_inherent_method_map(baselines, layer);
    // BB-4-fix1 / a-plan (AC-17): collect blanket body entries for each trait subgraph.
    // Must be built before the traits emit loop so entries are available for injection
    // inside each trait subgraph before its `end` line.
    let blanket_body_map =
        build_blanket_body_map(baselines, layer, crate_filter, &trait_index, style);

    // Sort entries alphabetically by module-qualified key (CN-08) — Functions separately.
    // Key = "<module_path>::<name>" (or just "<name>" for crate-root items) for stable
    // alphabetical ordering that avoids silent overwrites when the same name appears in
    // different modules (Finding 2 fix: use module-qualified key, not bare name).
    let mut structs: BTreeMap<String, (Id, &BaselineDocument)> = BTreeMap::new();
    let mut enums: BTreeMap<String, (Id, &BaselineDocument)> = BTreeMap::new();
    let mut traits: BTreeMap<String, (Id, &BaselineDocument)> = BTreeMap::new();
    let mut aliases: BTreeMap<String, (Id, &BaselineDocument)> = BTreeMap::new();
    let mut functions: BTreeMap<String, (Id, &BaselineDocument)> = BTreeMap::new();

    for node in &nodes {
        let doc = node.doc();
        // Apply crate filter: skip nodes from crates that don't match the requested crate.
        if let Some(filter) = crate_filter {
            if doc.crate_name.as_str() != filter {
                continue;
            }
        }
        let item = node.item();
        let id = node.id();
        // Apply cluster filter (T010): restrict to entries whose top-level module segment
        // matches the requested cluster. Use krate.paths to derive the cluster membership.
        if let Some(seg1_filter) = cluster_module_seg1_filter {
            let path_opt = doc.krate.paths.get(&id).map(|s| s.path.as_slice());
            let entry_seg1 = match path_opt {
                Some(p) if p.len() >= 3 => p.get(1).map(|s| s.as_str()).unwrap_or("root"),
                _ => "root", // crate-root entry (path length <= 2)
            };
            if entry_seg1 != seg1_filter {
                continue;
            }
        }
        let name = item.name.as_deref().unwrap_or("").to_string();
        // Build a module-qualified sort key to avoid overwriting same-named items from
        // different modules. Uses krate.paths for the module path when available.
        let module_path = doc
            .krate
            .paths
            .get(&id)
            .map(|s| super::node_id_generator::module_path_from_summary(&s.path))
            .unwrap_or_default();
        let qualified_key =
            if module_path.is_empty() { name.clone() } else { format!("{module_path}::{name}") };
        match node {
            ExtractedNode::Struct { .. } => {
                structs.insert(qualified_key, (id, doc));
            }
            ExtractedNode::Enum { .. } => {
                enums.insert(qualified_key, (id, doc));
            }
            ExtractedNode::TypeAlias { .. } => {
                aliases.insert(qualified_key, (id, doc));
            }
            ExtractedNode::Trait { .. } => {
                traits.insert(qualified_key, (id, doc));
            }
            ExtractedNode::Function { .. } => {
                // For function sort key, use the full path for uniqueness.
                let full_path = doc
                    .krate
                    .paths
                    .get(&id)
                    .map(|s| s.path.join("::"))
                    .unwrap_or_else(|| name.clone());
                functions.insert(full_path, (id, doc));
            }
        }
    }

    // Emit entry subgraphs: Structs → Enums → Traits → TypeAliases (all alphabetical).
    for (id, doc) in structs.values() {
        // T008 BB-4-fix1: look up inherent methods for this type's rep_node_id.
        let crate_name_str = doc.crate_name.as_str();
        let rep_id = {
            let summary_path_opt = doc.krate.paths.get(id).map(|s| s.path.as_slice());
            let mp = summary_path_opt
                .map(super::node_id_generator::module_path_from_summary)
                .unwrap_or_default();
            let item_name = doc.krate.index.get(id).and_then(|i| i.name.as_deref()).unwrap_or("");
            type_rep_node_id(layer, crate_name_str, &mp, item_name)
        };
        let method_ids_for_type =
            inherent_map.get(crate_name_str).and_then(|m| m.get(&rep_id)).map(|v| v.as_slice());
        emit_struct_subgraph(
            doc,
            *id,
            layer,
            subgraph_lines,
            edge_lines,
            class_attach,
            style,
            indent,
            method_ids_for_type,
        )?;
    }
    for (id, doc) in enums.values() {
        // T008 BB-4-fix1: look up inherent methods for this type's rep_node_id.
        let crate_name_str = doc.crate_name.as_str();
        let rep_id = {
            let summary_path_opt = doc.krate.paths.get(id).map(|s| s.path.as_slice());
            let mp = summary_path_opt
                .map(super::node_id_generator::module_path_from_summary)
                .unwrap_or_default();
            let item_name = doc.krate.index.get(id).and_then(|i| i.name.as_deref()).unwrap_or("");
            type_rep_node_id(layer, crate_name_str, &mp, item_name)
        };
        let method_ids_for_type =
            inherent_map.get(crate_name_str).and_then(|m| m.get(&rep_id)).map(|v| v.as_slice());
        emit_enum_subgraph(
            doc,
            *id,
            layer,
            subgraph_lines,
            edge_lines,
            class_attach,
            style,
            indent,
            method_ids_for_type,
        )?;
    }
    for (id, doc) in traits.values() {
        // BB-4-fix1 / a-plan: look up blanket body entries for this trait subgraph.
        // The trait_sg_id is needed to match against the blanket_body_map key.
        let trait_sg_id = {
            let summary_path_opt = doc.krate.paths.get(id).map(|s| s.path.as_slice());
            let mp = summary_path_opt
                .map(super::node_id_generator::module_path_from_summary)
                .unwrap_or_default();
            let item_name = doc.krate.index.get(id).and_then(|i| i.name.as_deref()).unwrap_or("");
            super::node_id_generator::trait_node_id(layer, doc.crate_name.as_str(), &mp, item_name)
        };
        let blanket_entries_for_trait = blanket_body_map.get(&trait_sg_id).map(|v| v.as_slice());
        emit_trait_subgraph(
            doc,
            *id,
            layer,
            subgraph_lines,
            edge_lines,
            class_attach,
            style,
            indent,
            blanket_entries_for_trait,
        )?;
    }
    for (id, doc) in aliases.values() {
        // T008 BB-4-fix1: look up inherent methods for this type alias's rep_node_id.
        let crate_name_str = doc.crate_name.as_str();
        let rep_id = {
            let summary_path_opt = doc.krate.paths.get(id).map(|s| s.path.as_slice());
            let mp = summary_path_opt
                .map(super::node_id_generator::module_path_from_summary)
                .unwrap_or_default();
            let item_name = doc.krate.index.get(id).and_then(|i| i.name.as_deref()).unwrap_or("");
            type_rep_node_id(layer, crate_name_str, &mp, item_name)
        };
        let method_ids_for_type =
            inherent_map.get(crate_name_str).and_then(|m| m.get(&rep_id)).map(|v| v.as_slice());
        emit_type_alias_subgraph(
            doc,
            *id,
            layer,
            subgraph_lines,
            edge_lines,
            class_attach,
            style,
            indent,
            method_ids_for_type,
        )?;
    }

    // Emit FunctionEntry callable nodes (alphabetical by full path, CN-08).
    for (id, doc) in functions.values() {
        emit_function_node(doc, *id, layer, subgraph_lines, class_attach, style, indent)?;
    }

    // T008 (BB-4-fix1 / J decision): emit trait impl edges for all baselines in this layer.
    // Called after all entry subgraphs are emitted so all type/trait subgraph IDs are stable.
    // Blanket body indicator nodes are already injected inside trait subgraphs above via
    // build_blanket_body_map + emit_trait_subgraph (a-plan, AC-17).
    emit_all_impl_edges_for_layer(baselines, layer, crate_filter, &trait_index, edge_lines, style)?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Build an entry label from a rustdoc `ItemSummary.path` slice and the item name.
///
/// `summary_path` is the full `[crate_name, module_seg1, ..., item_name]` slice from
/// `rustdoc_types::ItemSummary`. The middle segments (excluding the leading crate name
/// and the trailing item name) are joined with `::` for the display label.
///
/// - If there are middle segments: `"<seg1>::<seg2>::...:::<name>"`.
/// - If there are no middle segments (crate-root item): `"<name>"`.
///
/// This function uses the raw path segments directly (not the `_`-joined
/// `module_path_from_summary` output) to avoid misinterpreting underscores in
/// legitimate segment names as `::` separators.
fn build_entry_label(summary_path: &[String], name: &str) -> String {
    let total = summary_path.len();
    // Middle segments: skip 1 (crate_name), drop 1 (item_name) = total - 2 segments.
    if total <= 2 {
        // Crate-root item: no middle segments.
        return name.to_string();
    }
    let middle_count = total - 2;
    let middle: Vec<&str> =
        summary_path.iter().skip(1).take(middle_count).map(|s| s.as_str()).collect();
    format!("{}::{}", middle.join("::"), name)
}

// NOTE (T016): `field_type_node_id` has been removed.
// The single-target anonymous-node approach (`prim_*` / `generic_*` / `anon_*`) has been
// replaced by `collect_own_crate_node_ids_from_type` (recursive ResolvedPath.args traversal),
// which produces zero or more own-crate target node ids without ever creating anonymous nodes.
// All callers in this module now use that utility directly (AC-20).

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use std::collections::HashMap;

    use domain::tddd::baseline_document::BaselineDocument;
    use domain::tddd::catalogue_v2::identifiers::CrateName;
    use domain::tddd::layer_id::LayerId;
    use rustdoc_types::{
        Crate, Enum, FORMAT_VERSION, FunctionHeader, FunctionSignature, Generics, Id, Item,
        ItemEnum, ItemKind, ItemSummary, Module, Struct, StructKind, Target, Trait, Type,
        TypeAlias, Variant, VariantKind, Visibility,
    };

    use super::*;

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn empty_generics() -> Generics {
        Generics { params: vec![], where_predicates: vec![] }
    }

    fn make_item(id: Id, name: Option<&str>, inner: ItemEnum, vis: Visibility) -> Item {
        Item {
            id,
            crate_id: 0,
            name: name.map(|s| s.to_string()),
            span: None,
            visibility: vis,
            docs: None,
            links: HashMap::new(),
            attrs: vec![],
            deprecation: None,
            inner,
        }
    }

    fn pub_item(id: Id, name: &str, inner: ItemEnum) -> Item {
        make_item(id, Some(name), inner, Visibility::Public)
    }

    fn default_vis_item(id: Id, name: &str, inner: ItemEnum) -> Item {
        make_item(id, Some(name), inner, Visibility::Default)
    }

    fn empty_function_inner() -> ItemEnum {
        ItemEnum::Function(rustdoc_types::Function {
            sig: FunctionSignature { inputs: vec![], output: None, is_c_variadic: false },
            generics: empty_generics(),
            header: FunctionHeader {
                is_unsafe: false,
                is_const: false,
                is_async: false,
                abi: rustdoc_types::Abi::Rust,
            },
            has_body: true,
        })
    }

    fn make_crate(root_id: Id, index: HashMap<Id, Item>, paths: HashMap<Id, ItemSummary>) -> Crate {
        Crate {
            root: root_id,
            crate_version: None,
            includes_private: false,
            index,
            paths,
            external_crates: HashMap::new(),
            format_version: FORMAT_VERSION,
            target: Target { triple: String::new(), target_features: vec![] },
        }
    }

    fn item_summary(path: Vec<&str>, kind: ItemKind) -> ItemSummary {
        ItemSummary { crate_id: 0, path: path.into_iter().map(|s| s.to_string()).collect(), kind }
    }

    fn make_baseline(layer_str: &str, crate_name_str: &str, krate: Crate) -> BaselineDocument {
        BaselineDocument::new(
            LayerId::try_new(layer_str).unwrap(),
            CrateName::new(crate_name_str).unwrap(),
            krate,
        )
    }

    fn minimal_style() -> StyleConfig {
        toml::from_str::<StyleConfig>(
            r#"
[edge.field]
arrow = "--o"

[edge.variant_payload]
arrow = "--o"

[edge.alias]
arrow = "---"

[filter]
include_functions = true
"#,
        )
        .unwrap()
    }

    // -----------------------------------------------------------------------
    // T007: F-r1 — Struct entry subgraph
    // -----------------------------------------------------------------------

    #[test]
    fn test_emit_struct_subgraph_plain_struct_produces_subgraph_and_rep_node() {
        let root_id = Id(0);
        let struct_id = Id(1);

        let mut index = HashMap::new();
        index.insert(
            root_id,
            pub_item(
                root_id,
                "my_crate",
                ItemEnum::Module(Module {
                    is_crate: true,
                    items: vec![struct_id],
                    is_stripped: false,
                }),
            ),
        );
        index.insert(
            struct_id,
            pub_item(
                struct_id,
                "MyStruct",
                ItemEnum::Struct(Struct {
                    kind: StructKind::Plain { fields: vec![], has_stripped_fields: false },
                    generics: empty_generics(),
                    impls: vec![],
                }),
            ),
        );

        let mut paths = HashMap::new();
        paths.insert(struct_id, item_summary(vec!["my_crate", "MyStruct"], ItemKind::Struct));

        let krate = make_crate(root_id, index, paths);
        let doc = make_baseline("domain", "my_crate", krate);

        let mut subgraph_lines = vec![];
        let mut edge_lines = vec![];
        let mut class_attach = vec![];
        let style = minimal_style();

        emit_struct_subgraph(
            &doc,
            struct_id,
            "domain",
            &mut subgraph_lines,
            &mut edge_lines,
            &mut class_attach,
            &style,
            "  ",
            None,
        )
        .unwrap();

        let joined = subgraph_lines.join("\n");
        assert!(joined.contains("subgraph"), "must contain subgraph keyword");
        assert!(joined.contains("MyStruct"), "must contain type name");
        // Rep node for plain struct with no fields.
        let rep_id = type_rep_node_id("domain", "my_crate", "", "MyStruct");
        assert!(joined.contains(&rep_id), "must contain representative node id");
        assert!(edge_lines.is_empty(), "plain struct with no fields → no edges");
    }

    #[test]
    fn test_emit_struct_subgraph_plain_struct_primitive_field_emits_no_edge() {
        // T016 / AC-20: primitive field type → no edge (silent skip).
        // Anonymous nodes (prim_*) must NOT be generated.
        let root_id = Id(0);
        let struct_id = Id(1);
        let field_id = Id(2);

        let field_type = Type::Primitive("u32".to_string());

        let mut index = HashMap::new();
        index.insert(
            root_id,
            pub_item(
                root_id,
                "my_crate",
                ItemEnum::Module(Module {
                    is_crate: true,
                    items: vec![struct_id],
                    is_stripped: false,
                }),
            ),
        );
        index.insert(
            struct_id,
            pub_item(
                struct_id,
                "MyStruct",
                ItemEnum::Struct(Struct {
                    kind: StructKind::Plain { fields: vec![field_id], has_stripped_fields: false },
                    generics: empty_generics(),
                    impls: vec![],
                }),
            ),
        );
        index.insert(
            field_id,
            pub_item(field_id, "value", ItemEnum::StructField(field_type.clone())),
        );

        let mut paths = HashMap::new();
        paths.insert(struct_id, item_summary(vec!["my_crate", "MyStruct"], ItemKind::Struct));

        let krate = make_crate(root_id, index, paths);
        let doc = make_baseline("domain", "my_crate", krate);

        let mut subgraph_lines = vec![];
        let mut edge_lines = vec![];
        let mut class_attach = vec![];
        let style = minimal_style();

        emit_struct_subgraph(
            &doc,
            struct_id,
            "domain",
            &mut subgraph_lines,
            &mut edge_lines,
            &mut class_attach,
            &style,
            "  ",
            None,
        )
        .unwrap();

        // Primitive field → no edge (T016 / AC-20).
        assert!(
            edge_lines.is_empty(),
            "primitive field must produce no edge (T016 / AC-20); got: {edge_lines:?}"
        );
        // Also verify no prim_* node appears anywhere in the output.
        let joined = [subgraph_lines.join("\n"), edge_lines.join("\n")].join("\n");
        assert!(
            !joined.contains("prim_"),
            "no prim_* anonymous node must be generated; got: {joined}"
        );
    }

    #[test]
    fn test_emit_struct_subgraph_plain_struct_own_crate_field_emits_edge() {
        // T016 / AC-20: own-crate field type → edge to the target's rep-node id.
        let root_id = Id(0);
        let struct_id = Id(1);
        let field_id = Id(2);
        let field_type_id = Id(3);

        let field_type = Type::ResolvedPath(rustdoc_types::Path {
            path: "FieldType".to_string(),
            id: field_type_id,
            args: None,
        });

        let mut index = HashMap::new();
        index.insert(
            root_id,
            pub_item(
                root_id,
                "my_crate",
                ItemEnum::Module(Module {
                    is_crate: true,
                    items: vec![struct_id, field_type_id],
                    is_stripped: false,
                }),
            ),
        );
        index.insert(
            struct_id,
            pub_item(
                struct_id,
                "MyStruct",
                ItemEnum::Struct(Struct {
                    kind: StructKind::Plain { fields: vec![field_id], has_stripped_fields: false },
                    generics: empty_generics(),
                    impls: vec![],
                }),
            ),
        );
        index.insert(field_id, pub_item(field_id, "inner", ItemEnum::StructField(field_type)));
        index.insert(
            field_type_id,
            pub_item(
                field_type_id,
                "FieldType",
                ItemEnum::Struct(Struct {
                    kind: StructKind::Unit,
                    generics: empty_generics(),
                    impls: vec![],
                }),
            ),
        );

        let mut paths = HashMap::new();
        paths.insert(struct_id, item_summary(vec!["my_crate", "MyStruct"], ItemKind::Struct));
        paths.insert(
            field_type_id,
            ItemSummary {
                crate_id: 0,
                path: vec!["my_crate".to_string(), "FieldType".to_string()],
                kind: ItemKind::Struct,
            },
        );

        let krate = make_crate(root_id, index, paths);
        let doc = make_baseline("domain", "my_crate", krate);

        let mut subgraph_lines = vec![];
        let mut edge_lines = vec![];
        let mut class_attach = vec![];
        let style = minimal_style();

        emit_struct_subgraph(
            &doc,
            struct_id,
            "domain",
            &mut subgraph_lines,
            &mut edge_lines,
            &mut class_attach,
            &style,
            "  ",
            None,
        )
        .unwrap();

        assert_eq!(edge_lines.len(), 1, "own-crate field → one edge; got: {edge_lines:?}");
        assert!(edge_lines[0].contains("inner"), "edge must reference field name 'inner'");
        assert!(edge_lines[0].contains("--o"), "field edge must use --o arrow");
        let expected_target = type_rep_node_id("domain", "my_crate", "", "FieldType");
        assert!(
            edge_lines[0].contains(&expected_target),
            "edge must target FieldType rep-node; got: {}; expected target: {}",
            edge_lines[0],
            expected_target
        );
    }

    #[test]
    fn test_emit_struct_subgraph_stripped_fields_emits_no_edges() {
        let root_id = Id(0);
        let struct_id = Id(1);
        let field_id = Id(2);

        let mut index = HashMap::new();
        index.insert(
            root_id,
            pub_item(
                root_id,
                "my_crate",
                ItemEnum::Module(Module {
                    is_crate: true,
                    items: vec![struct_id],
                    is_stripped: false,
                }),
            ),
        );
        index.insert(
            struct_id,
            pub_item(
                struct_id,
                "Opaque",
                ItemEnum::Struct(Struct {
                    kind: StructKind::Plain {
                        fields: vec![field_id],
                        has_stripped_fields: true, // stripped → skip
                    },
                    generics: empty_generics(),
                    impls: vec![],
                }),
            ),
        );

        let mut paths = HashMap::new();
        paths.insert(struct_id, item_summary(vec!["my_crate", "Opaque"], ItemKind::Struct));

        let krate = make_crate(root_id, index, paths);
        let doc = make_baseline("domain", "my_crate", krate);

        let mut subgraph_lines = vec![];
        let mut edge_lines = vec![];
        let mut class_attach = vec![];
        let style = minimal_style();

        emit_struct_subgraph(
            &doc,
            struct_id,
            "domain",
            &mut subgraph_lines,
            &mut edge_lines,
            &mut class_attach,
            &style,
            "  ",
            None,
        )
        .unwrap();

        assert!(edge_lines.is_empty(), "has_stripped_fields=true → no field edges (K decision)");
    }

    #[test]
    fn test_emit_struct_subgraph_tuple_struct_primitive_fields_emit_no_edges() {
        // T016 / AC-20: tuple struct with primitive fields → no edge (silent skip).
        let root_id = Id(0);
        let struct_id = Id(1);
        let field0_id = Id(2);
        let field1_id = Id(3);

        let field_type = Type::Primitive("u32".to_string());

        let mut index = HashMap::new();
        index.insert(
            root_id,
            pub_item(
                root_id,
                "my_crate",
                ItemEnum::Module(Module {
                    is_crate: true,
                    items: vec![struct_id],
                    is_stripped: false,
                }),
            ),
        );
        index.insert(
            struct_id,
            pub_item(
                struct_id,
                "MyTuple",
                ItemEnum::Struct(Struct {
                    kind: StructKind::Tuple(vec![Some(field0_id), Some(field1_id)]),
                    generics: empty_generics(),
                    impls: vec![],
                }),
            ),
        );
        index
            .insert(field0_id, pub_item(field0_id, "0", ItemEnum::StructField(field_type.clone())));
        index
            .insert(field1_id, pub_item(field1_id, "1", ItemEnum::StructField(field_type.clone())));

        let mut paths = HashMap::new();
        paths.insert(struct_id, item_summary(vec!["my_crate", "MyTuple"], ItemKind::Struct));

        let krate = make_crate(root_id, index, paths);
        let doc = make_baseline("domain", "my_crate", krate);

        let mut subgraph_lines = vec![];
        let mut edge_lines = vec![];
        let mut class_attach = vec![];
        let style = minimal_style();

        emit_struct_subgraph(
            &doc,
            struct_id,
            "domain",
            &mut subgraph_lines,
            &mut edge_lines,
            &mut class_attach,
            &style,
            "  ",
            None,
        )
        .unwrap();

        assert!(
            edge_lines.is_empty(),
            "primitive tuple fields → no positional edges (T016 / AC-20); got: {edge_lines:?}"
        );
    }

    #[test]
    fn test_emit_struct_subgraph_tuple_struct_own_crate_field_emits_positional_edge() {
        // T016 / AC-20: tuple struct with own-crate field type → positional edge.
        let root_id = Id(0);
        let struct_id = Id(1);
        let field0_id = Id(2);
        let field_type_id = Id(3);

        let field_type = Type::ResolvedPath(rustdoc_types::Path {
            path: "Inner".to_string(),
            id: field_type_id,
            args: None,
        });

        let mut index = HashMap::new();
        index.insert(
            root_id,
            pub_item(
                root_id,
                "my_crate",
                ItemEnum::Module(Module {
                    is_crate: true,
                    items: vec![struct_id, field_type_id],
                    is_stripped: false,
                }),
            ),
        );
        index.insert(
            struct_id,
            pub_item(
                struct_id,
                "MyNewtype",
                ItemEnum::Struct(Struct {
                    kind: StructKind::Tuple(vec![Some(field0_id)]),
                    generics: empty_generics(),
                    impls: vec![],
                }),
            ),
        );
        index.insert(field0_id, pub_item(field0_id, "0", ItemEnum::StructField(field_type)));
        index.insert(
            field_type_id,
            pub_item(
                field_type_id,
                "Inner",
                ItemEnum::Struct(Struct {
                    kind: StructKind::Unit,
                    generics: empty_generics(),
                    impls: vec![],
                }),
            ),
        );

        let mut paths = HashMap::new();
        paths.insert(struct_id, item_summary(vec!["my_crate", "MyNewtype"], ItemKind::Struct));
        paths.insert(
            field_type_id,
            ItemSummary {
                crate_id: 0,
                path: vec!["my_crate".to_string(), "Inner".to_string()],
                kind: ItemKind::Struct,
            },
        );

        let krate = make_crate(root_id, index, paths);
        let doc = make_baseline("domain", "my_crate", krate);

        let mut subgraph_lines = vec![];
        let mut edge_lines = vec![];
        let mut class_attach = vec![];
        let style = minimal_style();

        emit_struct_subgraph(
            &doc,
            struct_id,
            "domain",
            &mut subgraph_lines,
            &mut edge_lines,
            &mut class_attach,
            &style,
            "  ",
            None,
        )
        .unwrap();

        assert_eq!(
            edge_lines.len(),
            1,
            "own-crate tuple field → one positional edge; got: {edge_lines:?}"
        );
        assert!(edge_lines[0].contains(".0"), "positional edge must use .0 label");
        let expected_target = type_rep_node_id("domain", "my_crate", "", "Inner");
        assert!(
            edge_lines[0].contains(&expected_target),
            "edge must target Inner rep-node; edge: {}; expected target: {}",
            edge_lines[0],
            expected_target
        );
    }

    #[test]
    fn test_emit_struct_subgraph_tuple_struct_none_slot_skipped_no_edge() {
        // T016 / AC-20: None slot (stripped field) must be skipped even with own-crate type.
        // This tests the None-slot skip behavior; the field0 slot has a primitive so no edge.
        let root_id = Id(0);
        let struct_id = Id(1);
        let field0_id = Id(2);

        let field_type = Type::Primitive("u32".to_string());

        let mut index = HashMap::new();
        index.insert(
            root_id,
            pub_item(
                root_id,
                "my_crate",
                ItemEnum::Module(Module {
                    is_crate: true,
                    items: vec![struct_id],
                    is_stripped: false,
                }),
            ),
        );
        index.insert(
            struct_id,
            pub_item(
                struct_id,
                "TupleWithStripped",
                ItemEnum::Struct(Struct {
                    // Second slot is None (stripped field).
                    kind: StructKind::Tuple(vec![Some(field0_id), None]),
                    generics: empty_generics(),
                    impls: vec![],
                }),
            ),
        );
        index
            .insert(field0_id, pub_item(field0_id, "0", ItemEnum::StructField(field_type.clone())));

        let mut paths = HashMap::new();
        paths.insert(
            struct_id,
            item_summary(vec!["my_crate", "TupleWithStripped"], ItemKind::Struct),
        );

        let krate = make_crate(root_id, index, paths);
        let doc = make_baseline("domain", "my_crate", krate);

        let mut subgraph_lines = vec![];
        let mut edge_lines = vec![];
        let mut class_attach = vec![];
        let style = minimal_style();

        emit_struct_subgraph(
            &doc,
            struct_id,
            "domain",
            &mut subgraph_lines,
            &mut edge_lines,
            &mut class_attach,
            &style,
            "  ",
            None,
        )
        .unwrap();

        // field0 is primitive → no edge; None slot skipped → no edge. Total: 0.
        assert!(
            edge_lines.is_empty(),
            "None slot + primitive field → no edges (T016 / AC-20); got: {edge_lines:?}"
        );
    }

    #[test]
    fn test_emit_struct_subgraph_unit_struct_no_edges() {
        let root_id = Id(0);
        let struct_id = Id(1);

        let mut index = HashMap::new();
        index.insert(
            root_id,
            pub_item(
                root_id,
                "my_crate",
                ItemEnum::Module(Module {
                    is_crate: true,
                    items: vec![struct_id],
                    is_stripped: false,
                }),
            ),
        );
        index.insert(
            struct_id,
            pub_item(
                struct_id,
                "MyUnit",
                ItemEnum::Struct(Struct {
                    kind: StructKind::Unit,
                    generics: empty_generics(),
                    impls: vec![],
                }),
            ),
        );

        let mut paths = HashMap::new();
        paths.insert(struct_id, item_summary(vec!["my_crate", "MyUnit"], ItemKind::Struct));

        let krate = make_crate(root_id, index, paths);
        let doc = make_baseline("domain", "my_crate", krate);

        let mut subgraph_lines = vec![];
        let mut edge_lines = vec![];
        let mut class_attach = vec![];
        let style = minimal_style();

        emit_struct_subgraph(
            &doc,
            struct_id,
            "domain",
            &mut subgraph_lines,
            &mut edge_lines,
            &mut class_attach,
            &style,
            "  ",
            None,
        )
        .unwrap();

        assert!(edge_lines.is_empty(), "Unit struct → no field edges (K decision)");
        let joined = subgraph_lines.join("\n");
        assert!(joined.contains("MyUnit"), "subgraph must be emitted for Unit");
    }

    // -----------------------------------------------------------------------
    // T007: H decision — Enum variant nodes + payload edges
    // -----------------------------------------------------------------------

    #[test]
    fn test_emit_enum_subgraph_plain_variant_no_edge() {
        let root_id = Id(0);
        let enum_id = Id(1);
        let variant_id = Id(2);

        let mut index = HashMap::new();
        index.insert(
            root_id,
            pub_item(
                root_id,
                "my_crate",
                ItemEnum::Module(Module {
                    is_crate: true,
                    items: vec![enum_id],
                    is_stripped: false,
                }),
            ),
        );
        index.insert(
            enum_id,
            pub_item(
                enum_id,
                "MyEnum",
                ItemEnum::Enum(Enum {
                    generics: empty_generics(),
                    variants: vec![variant_id],
                    impls: vec![],
                    has_stripped_variants: false,
                }),
            ),
        );
        index.insert(
            variant_id,
            default_vis_item(
                variant_id,
                "PlainVariant",
                ItemEnum::Variant(Variant { kind: VariantKind::Plain, discriminant: None }),
            ),
        );

        let mut paths = HashMap::new();
        paths.insert(enum_id, item_summary(vec!["my_crate", "MyEnum"], ItemKind::Enum));

        let krate = make_crate(root_id, index, paths);
        let doc = make_baseline("domain", "my_crate", krate);

        let mut subgraph_lines = vec![];
        let mut edge_lines = vec![];
        let mut class_attach = vec![];
        let style = minimal_style();

        emit_enum_subgraph(
            &doc,
            enum_id,
            "domain",
            &mut subgraph_lines,
            &mut edge_lines,
            &mut class_attach,
            &style,
            "  ",
            None,
        )
        .unwrap();

        let joined = subgraph_lines.join("\n");
        assert!(joined.contains("PlainVariant"), "plain variant node must be emitted");
        assert!(edge_lines.is_empty(), "PlainVariant → no payload edges (H decision)");
    }

    #[test]
    fn test_emit_enum_subgraph_tuple_variant_primitive_payload_no_edge() {
        // T016 / AC-20: tuple variant with primitive payload → no edge.
        let root_id = Id(0);
        let enum_id = Id(1);
        let variant_id = Id(2);
        let field_id = Id(3);

        let field_type = Type::Primitive("u64".to_string());

        let mut index = HashMap::new();
        index.insert(
            root_id,
            pub_item(
                root_id,
                "my_crate",
                ItemEnum::Module(Module {
                    is_crate: true,
                    items: vec![enum_id],
                    is_stripped: false,
                }),
            ),
        );
        index.insert(
            enum_id,
            pub_item(
                enum_id,
                "MyEnum",
                ItemEnum::Enum(Enum {
                    generics: empty_generics(),
                    variants: vec![variant_id],
                    impls: vec![],
                    has_stripped_variants: false,
                }),
            ),
        );
        index.insert(
            variant_id,
            default_vis_item(
                variant_id,
                "TupleVariant",
                ItemEnum::Variant(Variant {
                    kind: VariantKind::Tuple(vec![Some(field_id)]),
                    discriminant: None,
                }),
            ),
        );
        index.insert(field_id, pub_item(field_id, "0", ItemEnum::StructField(field_type.clone())));

        let mut paths = HashMap::new();
        paths.insert(enum_id, item_summary(vec!["my_crate", "MyEnum"], ItemKind::Enum));

        let krate = make_crate(root_id, index, paths);
        let doc = make_baseline("domain", "my_crate", krate);

        let mut subgraph_lines = vec![];
        let mut edge_lines = vec![];
        let mut class_attach = vec![];
        let style = minimal_style();

        emit_enum_subgraph(
            &doc,
            enum_id,
            "domain",
            &mut subgraph_lines,
            &mut edge_lines,
            &mut class_attach,
            &style,
            "  ",
            None,
        )
        .unwrap();

        assert!(
            edge_lines.is_empty(),
            "primitive tuple variant payload → no edge (T016 / AC-20); got: {edge_lines:?}"
        );
    }

    #[test]
    fn test_emit_enum_subgraph_tuple_variant_own_crate_payload_emits_edge() {
        // T016 / AC-20: tuple variant with own-crate payload → one unlabeled edge.
        let root_id = Id(0);
        let enum_id = Id(1);
        let variant_id = Id(2);
        let field_id = Id(3);
        let payload_type_id = Id(4);

        let field_type = Type::ResolvedPath(rustdoc_types::Path {
            path: "PayloadType".to_string(),
            id: payload_type_id,
            args: None,
        });

        let mut index = HashMap::new();
        index.insert(
            root_id,
            pub_item(
                root_id,
                "my_crate",
                ItemEnum::Module(Module {
                    is_crate: true,
                    items: vec![enum_id, payload_type_id],
                    is_stripped: false,
                }),
            ),
        );
        index.insert(
            enum_id,
            pub_item(
                enum_id,
                "MyEnum",
                ItemEnum::Enum(Enum {
                    generics: empty_generics(),
                    variants: vec![variant_id],
                    impls: vec![],
                    has_stripped_variants: false,
                }),
            ),
        );
        index.insert(
            variant_id,
            default_vis_item(
                variant_id,
                "Wrapped",
                ItemEnum::Variant(Variant {
                    kind: VariantKind::Tuple(vec![Some(field_id)]),
                    discriminant: None,
                }),
            ),
        );
        index.insert(field_id, pub_item(field_id, "0", ItemEnum::StructField(field_type)));
        index.insert(
            payload_type_id,
            pub_item(
                payload_type_id,
                "PayloadType",
                ItemEnum::Struct(Struct {
                    kind: StructKind::Unit,
                    generics: empty_generics(),
                    impls: vec![],
                }),
            ),
        );

        let mut paths = HashMap::new();
        paths.insert(enum_id, item_summary(vec!["my_crate", "MyEnum"], ItemKind::Enum));
        paths.insert(
            payload_type_id,
            ItemSummary {
                crate_id: 0,
                path: vec!["my_crate".to_string(), "PayloadType".to_string()],
                kind: ItemKind::Struct,
            },
        );

        let krate = make_crate(root_id, index, paths);
        let doc = make_baseline("domain", "my_crate", krate);

        let mut subgraph_lines = vec![];
        let mut edge_lines = vec![];
        let mut class_attach = vec![];
        let style = minimal_style();

        emit_enum_subgraph(
            &doc,
            enum_id,
            "domain",
            &mut subgraph_lines,
            &mut edge_lines,
            &mut class_attach,
            &style,
            "  ",
            None,
        )
        .unwrap();

        assert_eq!(
            edge_lines.len(),
            1,
            "own-crate tuple variant → 1 payload edge; got: {edge_lines:?}"
        );
        assert!(edge_lines[0].contains("--o"), "tuple variant payload edge must use --o");
        let expected_target = type_rep_node_id("domain", "my_crate", "", "PayloadType");
        assert!(
            edge_lines[0].contains(&expected_target),
            "edge must target PayloadType rep-node; edge: {}; expected: {}",
            edge_lines[0],
            expected_target
        );
    }

    #[test]
    fn test_emit_enum_subgraph_struct_variant_primitive_field_no_edge() {
        // T016 / AC-20: struct variant with primitive field → no named edge.
        let root_id = Id(0);
        let enum_id = Id(1);
        let variant_id = Id(2);
        let field_id = Id(3);

        let field_type = Type::Primitive("String".to_string());

        let mut index = HashMap::new();
        index.insert(
            root_id,
            pub_item(
                root_id,
                "my_crate",
                ItemEnum::Module(Module {
                    is_crate: true,
                    items: vec![enum_id],
                    is_stripped: false,
                }),
            ),
        );
        index.insert(
            enum_id,
            pub_item(
                enum_id,
                "MyEnum",
                ItemEnum::Enum(Enum {
                    generics: empty_generics(),
                    variants: vec![variant_id],
                    impls: vec![],
                    has_stripped_variants: false,
                }),
            ),
        );
        index.insert(
            variant_id,
            default_vis_item(
                variant_id,
                "StructVariant",
                ItemEnum::Variant(Variant {
                    kind: VariantKind::Struct {
                        fields: vec![field_id],
                        has_stripped_fields: false,
                    },
                    discriminant: None,
                }),
            ),
        );
        index.insert(
            field_id,
            pub_item(field_id, "username", ItemEnum::StructField(field_type.clone())),
        );

        let mut paths = HashMap::new();
        paths.insert(enum_id, item_summary(vec!["my_crate", "MyEnum"], ItemKind::Enum));

        let krate = make_crate(root_id, index, paths);
        let doc = make_baseline("domain", "my_crate", krate);

        let mut subgraph_lines = vec![];
        let mut edge_lines = vec![];
        let mut class_attach = vec![];
        let style = minimal_style();

        emit_enum_subgraph(
            &doc,
            enum_id,
            "domain",
            &mut subgraph_lines,
            &mut edge_lines,
            &mut class_attach,
            &style,
            "  ",
            None,
        )
        .unwrap();

        assert!(
            edge_lines.is_empty(),
            "primitive struct variant field → no edge (T016 / AC-20); got: {edge_lines:?}"
        );
    }

    #[test]
    fn test_emit_enum_subgraph_struct_variant_own_crate_field_emits_named_edge() {
        // T016 / AC-20: struct variant with own-crate field → named edge.
        let root_id = Id(0);
        let enum_id = Id(1);
        let variant_id = Id(2);
        let field_id = Id(3);
        let field_type_id = Id(4);

        let field_type = Type::ResolvedPath(rustdoc_types::Path {
            path: "UserId".to_string(),
            id: field_type_id,
            args: None,
        });

        let mut index = HashMap::new();
        index.insert(
            root_id,
            pub_item(
                root_id,
                "my_crate",
                ItemEnum::Module(Module {
                    is_crate: true,
                    items: vec![enum_id, field_type_id],
                    is_stripped: false,
                }),
            ),
        );
        index.insert(
            enum_id,
            pub_item(
                enum_id,
                "MyEnum",
                ItemEnum::Enum(Enum {
                    generics: empty_generics(),
                    variants: vec![variant_id],
                    impls: vec![],
                    has_stripped_variants: false,
                }),
            ),
        );
        index.insert(
            variant_id,
            default_vis_item(
                variant_id,
                "StructVariant",
                ItemEnum::Variant(Variant {
                    kind: VariantKind::Struct {
                        fields: vec![field_id],
                        has_stripped_fields: false,
                    },
                    discriminant: None,
                }),
            ),
        );
        index.insert(field_id, pub_item(field_id, "owner_id", ItemEnum::StructField(field_type)));
        index.insert(
            field_type_id,
            pub_item(
                field_type_id,
                "UserId",
                ItemEnum::Struct(Struct {
                    kind: StructKind::Unit,
                    generics: empty_generics(),
                    impls: vec![],
                }),
            ),
        );

        let mut paths = HashMap::new();
        paths.insert(enum_id, item_summary(vec!["my_crate", "MyEnum"], ItemKind::Enum));
        paths.insert(
            field_type_id,
            ItemSummary {
                crate_id: 0,
                path: vec!["my_crate".to_string(), "UserId".to_string()],
                kind: ItemKind::Struct,
            },
        );

        let krate = make_crate(root_id, index, paths);
        let doc = make_baseline("domain", "my_crate", krate);

        let mut subgraph_lines = vec![];
        let mut edge_lines = vec![];
        let mut class_attach = vec![];
        let style = minimal_style();

        emit_enum_subgraph(
            &doc,
            enum_id,
            "domain",
            &mut subgraph_lines,
            &mut edge_lines,
            &mut class_attach,
            &style,
            "  ",
            None,
        )
        .unwrap();

        assert_eq!(
            edge_lines.len(),
            1,
            "own-crate struct variant field → 1 named edge; got: {edge_lines:?}"
        );
        assert!(
            edge_lines[0].contains("owner_id"),
            "struct variant edge must include field name 'owner_id'; got: {}",
            edge_lines[0]
        );
        let expected_target = type_rep_node_id("domain", "my_crate", "", "UserId");
        assert!(
            edge_lines[0].contains(&expected_target),
            "edge must target UserId rep-node; edge: {}; expected: {}",
            edge_lines[0],
            expected_target
        );
    }

    // -----------------------------------------------------------------------
    // T007: H' decision — Trait method nodes inside Trait subgraph
    // -----------------------------------------------------------------------

    #[test]
    fn test_emit_trait_subgraph_with_method_emits_method_node_inside() {
        let root_id = Id(0);
        let trait_id = Id(1);
        let method_id = Id(2);

        let mut index = HashMap::new();
        index.insert(
            root_id,
            pub_item(
                root_id,
                "my_crate",
                ItemEnum::Module(Module {
                    is_crate: true,
                    items: vec![trait_id],
                    is_stripped: false,
                }),
            ),
        );
        index.insert(
            trait_id,
            pub_item(
                trait_id,
                "MyTrait",
                ItemEnum::Trait(Trait {
                    is_auto: false,
                    is_unsafe: false,
                    is_dyn_compatible: true,
                    items: vec![method_id],
                    generics: empty_generics(),
                    bounds: vec![],
                    implementations: vec![],
                }),
            ),
        );
        index
            .insert(method_id, default_vis_item(method_id, "do_something", empty_function_inner()));

        let mut paths = HashMap::new();
        paths.insert(trait_id, item_summary(vec!["my_crate", "MyTrait"], ItemKind::Trait));

        let krate = make_crate(root_id, index, paths);
        let doc = make_baseline("domain", "my_crate", krate);

        let mut subgraph_lines = vec![];
        let mut edge_lines = vec![];
        let mut class_attach = vec![];
        let style = minimal_style();

        emit_trait_subgraph(
            &doc,
            trait_id,
            "domain",
            &mut subgraph_lines,
            &mut edge_lines,
            &mut class_attach,
            &style,
            "  ",
            None,
        )
        .unwrap();

        let joined = subgraph_lines.join("\n");
        assert!(joined.contains("MyTrait"), "subgraph must contain trait name");
        assert!(
            joined.contains("do_something"),
            "method node 'do_something' must appear inside trait subgraph (H' decision)"
        );
    }

    #[test]
    fn test_emit_trait_subgraph_no_methods_still_emits_subgraph() {
        let root_id = Id(0);
        let trait_id = Id(1);

        let mut index = HashMap::new();
        index.insert(
            root_id,
            pub_item(
                root_id,
                "my_crate",
                ItemEnum::Module(Module {
                    is_crate: true,
                    items: vec![trait_id],
                    is_stripped: false,
                }),
            ),
        );
        index.insert(
            trait_id,
            pub_item(
                trait_id,
                "Marker",
                ItemEnum::Trait(Trait {
                    is_auto: false,
                    is_unsafe: false,
                    is_dyn_compatible: true,
                    items: vec![],
                    generics: empty_generics(),
                    bounds: vec![],
                    implementations: vec![],
                }),
            ),
        );

        let mut paths = HashMap::new();
        paths.insert(trait_id, item_summary(vec!["my_crate", "Marker"], ItemKind::Trait));

        let krate = make_crate(root_id, index, paths);
        let doc = make_baseline("domain", "my_crate", krate);

        let mut subgraph_lines = vec![];
        let mut edge_lines = vec![];
        let mut class_attach = vec![];
        let style = minimal_style();

        emit_trait_subgraph(
            &doc,
            trait_id,
            "domain",
            &mut subgraph_lines,
            &mut edge_lines,
            &mut class_attach,
            &style,
            "  ",
            None,
        )
        .unwrap();

        let joined = subgraph_lines.join("\n");
        assert!(
            joined.contains("subgraph"),
            "even a trait with no methods must produce a subgraph"
        );
        assert!(joined.contains("Marker"), "subgraph must contain trait name");
    }

    // -----------------------------------------------------------------------
    // T015: H' — Trait method method_param / method_returns edges (AC-19)
    // -----------------------------------------------------------------------

    fn style_with_method_edges() -> StyleConfig {
        toml::from_str::<StyleConfig>(
            r#"
[edge.field]
arrow = "--o"

[edge.variant_payload]
arrow = "--o"

[edge.alias]
arrow = "---"

[edge.method_param]
arrow = "-->"

[edge.method_returns]
arrow = "==>"

[filter]
include_functions = true
"#,
        )
        .unwrap()
    }

    fn item_summary_ext(crate_id: u32, path: Vec<&str>, kind: ItemKind) -> ItemSummary {
        ItemSummary { crate_id, path: path.into_iter().map(|s| s.to_string()).collect(), kind }
    }

    #[test]
    fn test_emit_trait_subgraph_method_with_own_crate_param_emits_method_param_edge() {
        // H' decision: a Trait method with an own-crate param → method_param edge is emitted.
        let root_id = Id(0);
        let trait_id = Id(1);
        let method_id = Id(2);
        let param_type_id = Id(3);

        let mut index = HashMap::new();
        index.insert(
            root_id,
            pub_item(
                root_id,
                "my_crate",
                ItemEnum::Module(Module {
                    is_crate: true,
                    items: vec![trait_id],
                    is_stripped: false,
                }),
            ),
        );
        index.insert(
            trait_id,
            pub_item(
                trait_id,
                "MyTrait",
                ItemEnum::Trait(Trait {
                    is_auto: false,
                    is_unsafe: false,
                    is_dyn_compatible: true,
                    items: vec![method_id],
                    generics: empty_generics(),
                    bounds: vec![],
                    implementations: vec![],
                }),
            ),
        );

        let param_ty = rustdoc_types::Type::ResolvedPath(rustdoc_types::Path {
            path: "ParamType".to_string(),
            id: param_type_id,
            args: None,
        });
        index.insert(
            method_id,
            default_vis_item(
                method_id,
                "process",
                ItemEnum::Function(rustdoc_types::Function {
                    sig: FunctionSignature {
                        inputs: vec![("arg".to_string(), param_ty)],
                        output: None,
                        is_c_variadic: false,
                    },
                    generics: empty_generics(),
                    header: FunctionHeader {
                        is_unsafe: false,
                        is_const: false,
                        is_async: false,
                        abi: rustdoc_types::Abi::Rust,
                    },
                    has_body: false,
                }),
            ),
        );
        // ParamType must be in krate.index as a Public Struct so the visibility/kind
        // filter in collect_own_crate_node_ids_recursive accepts it as a renderable type.
        index.insert(
            param_type_id,
            pub_item(
                param_type_id,
                "ParamType",
                ItemEnum::Struct(Struct {
                    kind: StructKind::Unit,
                    generics: empty_generics(),
                    impls: vec![],
                }),
            ),
        );

        let mut paths = HashMap::new();
        paths.insert(trait_id, item_summary(vec!["my_crate", "MyTrait"], ItemKind::Trait));
        paths.insert(
            param_type_id,
            item_summary_ext(0, vec!["my_crate", "ParamType"], ItemKind::Struct),
        );

        let krate = make_crate(root_id, index, paths);
        let doc = make_baseline("domain", "my_crate", krate);

        let mut subgraph_lines = vec![];
        let mut edge_lines = vec![];
        let mut class_attach = vec![];
        let style = style_with_method_edges();

        emit_trait_subgraph(
            &doc,
            trait_id,
            "domain",
            &mut subgraph_lines,
            &mut edge_lines,
            &mut class_attach,
            &style,
            "  ",
            None,
        )
        .unwrap();

        let param_edges: Vec<_> = edge_lines.iter().filter(|l| l.contains("-->")).collect();
        assert_eq!(
            param_edges.len(),
            1,
            "trait method with one own-crate param → one method_param edge; got: {edge_lines:?}"
        );
        let expected_target = type_rep_node_id("domain", "my_crate", "", "ParamType");
        assert!(
            param_edges[0].contains(&expected_target),
            "method_param edge must target ParamType rep-node; edge: {}; expected: {}",
            param_edges[0],
            expected_target
        );
    }

    #[test]
    fn test_emit_trait_subgraph_method_with_own_crate_return_type_emits_method_returns_edge() {
        // H' decision: a Trait method with an own-crate return type → method_returns edge.
        let root_id = Id(0);
        let trait_id = Id(1);
        let method_id = Id(2);
        let ret_type_id = Id(3);

        let mut index = HashMap::new();
        index.insert(
            root_id,
            pub_item(
                root_id,
                "my_crate",
                ItemEnum::Module(Module {
                    is_crate: true,
                    items: vec![trait_id],
                    is_stripped: false,
                }),
            ),
        );
        index.insert(
            trait_id,
            pub_item(
                trait_id,
                "MyTrait",
                ItemEnum::Trait(Trait {
                    is_auto: false,
                    is_unsafe: false,
                    is_dyn_compatible: true,
                    items: vec![method_id],
                    generics: empty_generics(),
                    bounds: vec![],
                    implementations: vec![],
                }),
            ),
        );

        let ret_ty = rustdoc_types::Type::ResolvedPath(rustdoc_types::Path {
            path: "ReturnType".to_string(),
            id: ret_type_id,
            args: None,
        });
        index.insert(
            method_id,
            default_vis_item(
                method_id,
                "build",
                ItemEnum::Function(rustdoc_types::Function {
                    sig: FunctionSignature {
                        inputs: vec![],
                        output: Some(ret_ty),
                        is_c_variadic: false,
                    },
                    generics: empty_generics(),
                    header: FunctionHeader {
                        is_unsafe: false,
                        is_const: false,
                        is_async: false,
                        abi: rustdoc_types::Abi::Rust,
                    },
                    has_body: false,
                }),
            ),
        );
        // ReturnType must be in krate.index as a Public Struct so the visibility/kind
        // filter in collect_own_crate_node_ids_recursive accepts it as a renderable type.
        index.insert(
            ret_type_id,
            pub_item(
                ret_type_id,
                "ReturnType",
                ItemEnum::Struct(Struct {
                    kind: StructKind::Unit,
                    generics: empty_generics(),
                    impls: vec![],
                }),
            ),
        );

        let mut paths = HashMap::new();
        paths.insert(trait_id, item_summary(vec!["my_crate", "MyTrait"], ItemKind::Trait));
        paths.insert(
            ret_type_id,
            item_summary_ext(0, vec!["my_crate", "ReturnType"], ItemKind::Struct),
        );

        let krate = make_crate(root_id, index, paths);
        let doc = make_baseline("domain", "my_crate", krate);

        let mut subgraph_lines = vec![];
        let mut edge_lines = vec![];
        let mut class_attach = vec![];
        let style = style_with_method_edges();

        emit_trait_subgraph(
            &doc,
            trait_id,
            "domain",
            &mut subgraph_lines,
            &mut edge_lines,
            &mut class_attach,
            &style,
            "  ",
            None,
        )
        .unwrap();

        let ret_edges: Vec<_> = edge_lines.iter().filter(|l| l.contains("==>")).collect();
        assert_eq!(
            ret_edges.len(),
            1,
            "trait method with own-crate return type → one method_returns edge; got: {edge_lines:?}"
        );
        let expected_target = type_rep_node_id("domain", "my_crate", "", "ReturnType");
        assert!(
            ret_edges[0].contains(&expected_target),
            "method_returns edge must target ReturnType rep-node; edge: {}; expected: {}",
            ret_edges[0],
            expected_target
        );
    }

    #[test]
    fn test_emit_trait_subgraph_method_with_primitive_param_no_method_param_edge() {
        // H' decision: primitive param → no method_param edge (AC-20 exclusion).
        let root_id = Id(0);
        let trait_id = Id(1);
        let method_id = Id(2);

        let mut index = HashMap::new();
        index.insert(
            root_id,
            pub_item(
                root_id,
                "my_crate",
                ItemEnum::Module(Module {
                    is_crate: true,
                    items: vec![trait_id],
                    is_stripped: false,
                }),
            ),
        );
        index.insert(
            trait_id,
            pub_item(
                trait_id,
                "MyTrait",
                ItemEnum::Trait(Trait {
                    is_auto: false,
                    is_unsafe: false,
                    is_dyn_compatible: true,
                    items: vec![method_id],
                    generics: empty_generics(),
                    bounds: vec![],
                    implementations: vec![],
                }),
            ),
        );
        index.insert(
            method_id,
            default_vis_item(
                method_id,
                "with_val",
                ItemEnum::Function(rustdoc_types::Function {
                    sig: FunctionSignature {
                        inputs: vec![("n".to_string(), Type::Primitive("usize".to_string()))],
                        output: None,
                        is_c_variadic: false,
                    },
                    generics: empty_generics(),
                    header: FunctionHeader {
                        is_unsafe: false,
                        is_const: false,
                        is_async: false,
                        abi: rustdoc_types::Abi::Rust,
                    },
                    has_body: false,
                }),
            ),
        );

        let mut paths = HashMap::new();
        paths.insert(trait_id, item_summary(vec!["my_crate", "MyTrait"], ItemKind::Trait));

        let krate = make_crate(root_id, index, paths);
        let doc = make_baseline("domain", "my_crate", krate);

        let mut subgraph_lines = vec![];
        let mut edge_lines = vec![];
        let mut class_attach = vec![];
        let style = style_with_method_edges();

        emit_trait_subgraph(
            &doc,
            trait_id,
            "domain",
            &mut subgraph_lines,
            &mut edge_lines,
            &mut class_attach,
            &style,
            "  ",
            None,
        )
        .unwrap();

        assert!(
            edge_lines.is_empty(),
            "primitive param type → no method_param edge; got: {edge_lines:?}"
        );
        // Method node still appears in subgraph.
        let joined = subgraph_lines.join("\n");
        assert!(joined.contains("with_val"), "method node must still appear in subgraph");
    }

    #[test]
    fn test_emit_trait_subgraph_method_with_external_type_param_no_edge() {
        // H' decision: external-crate type param (crate_id != 0) → no method_param edge.
        let root_id = Id(0);
        let trait_id = Id(1);
        let method_id = Id(2);
        let ext_type_id = Id(3);

        let mut index = HashMap::new();
        index.insert(
            root_id,
            pub_item(
                root_id,
                "my_crate",
                ItemEnum::Module(Module {
                    is_crate: true,
                    items: vec![trait_id],
                    is_stripped: false,
                }),
            ),
        );
        index.insert(
            trait_id,
            pub_item(
                trait_id,
                "MyTrait",
                ItemEnum::Trait(Trait {
                    is_auto: false,
                    is_unsafe: false,
                    is_dyn_compatible: true,
                    items: vec![method_id],
                    generics: empty_generics(),
                    bounds: vec![],
                    implementations: vec![],
                }),
            ),
        );

        let ext_ty = rustdoc_types::Type::ResolvedPath(rustdoc_types::Path {
            path: "ext_crate::ExtType".to_string(),
            id: ext_type_id,
            args: None,
        });
        index.insert(
            method_id,
            default_vis_item(
                method_id,
                "process_ext",
                ItemEnum::Function(rustdoc_types::Function {
                    sig: FunctionSignature {
                        inputs: vec![("arg".to_string(), ext_ty)],
                        output: None,
                        is_c_variadic: false,
                    },
                    generics: empty_generics(),
                    header: FunctionHeader {
                        is_unsafe: false,
                        is_const: false,
                        is_async: false,
                        abi: rustdoc_types::Abi::Rust,
                    },
                    has_body: false,
                }),
            ),
        );

        let mut paths = HashMap::new();
        paths.insert(trait_id, item_summary(vec!["my_crate", "MyTrait"], ItemKind::Trait));
        // ext_type_id has crate_id = 99 (external)
        paths.insert(
            ext_type_id,
            item_summary_ext(99, vec!["ext_crate", "ExtType"], ItemKind::Struct),
        );

        let krate = make_crate(root_id, index, paths);
        let doc = make_baseline("domain", "my_crate", krate);

        let mut subgraph_lines = vec![];
        let mut edge_lines = vec![];
        let mut class_attach = vec![];
        let style = style_with_method_edges();

        emit_trait_subgraph(
            &doc,
            trait_id,
            "domain",
            &mut subgraph_lines,
            &mut edge_lines,
            &mut class_attach,
            &style,
            "  ",
            None,
        )
        .unwrap();

        assert!(
            edge_lines.is_empty(),
            "external-crate type param → no method_param edge; got: {edge_lines:?}"
        );
    }

    // -----------------------------------------------------------------------
    // T007: N decision — TypeAlias alias edge
    // -----------------------------------------------------------------------

    #[test]
    fn test_emit_type_alias_subgraph_primitive_target_no_edge() {
        // T016 / AC-20 / N decision: TypeAlias with primitive target → no alias edge.
        let root_id = Id(0);
        let alias_id = Id(1);

        let target_type = Type::Primitive("u32".to_string());

        let mut index = HashMap::new();
        index.insert(
            root_id,
            pub_item(
                root_id,
                "my_crate",
                ItemEnum::Module(Module {
                    is_crate: true,
                    items: vec![alias_id],
                    is_stripped: false,
                }),
            ),
        );
        index.insert(
            alias_id,
            pub_item(
                alias_id,
                "MyAlias",
                ItemEnum::TypeAlias(TypeAlias { type_: target_type, generics: empty_generics() }),
            ),
        );

        let mut paths = HashMap::new();
        paths.insert(alias_id, item_summary(vec!["my_crate", "MyAlias"], ItemKind::TypeAlias));

        let krate = make_crate(root_id, index, paths);
        let doc = make_baseline("domain", "my_crate", krate);

        let mut subgraph_lines = vec![];
        let mut edge_lines = vec![];
        let mut class_attach = vec![];
        let style = minimal_style();

        emit_type_alias_subgraph(
            &doc,
            alias_id,
            "domain",
            &mut subgraph_lines,
            &mut edge_lines,
            &mut class_attach,
            &style,
            "  ",
            None,
        )
        .unwrap();

        let joined = subgraph_lines.join("\n");
        assert!(joined.contains("MyAlias"), "subgraph must contain alias name");
        assert!(
            edge_lines.is_empty(),
            "TypeAlias with primitive target → no alias edge (T016 / AC-20); got: {edge_lines:?}"
        );
    }

    #[test]
    fn test_emit_type_alias_subgraph_own_crate_target_emits_alias_edge() {
        // T016 / AC-20 / N decision: TypeAlias pointing to own-crate type → alias_of edge.
        let root_id = Id(0);
        let alias_id = Id(1);
        let target_id = Id(2);

        let target_type = Type::ResolvedPath(rustdoc_types::Path {
            path: "ConcreteType".to_string(),
            id: target_id,
            args: None,
        });

        let mut index = HashMap::new();
        index.insert(
            root_id,
            pub_item(
                root_id,
                "my_crate",
                ItemEnum::Module(Module {
                    is_crate: true,
                    items: vec![alias_id, target_id],
                    is_stripped: false,
                }),
            ),
        );
        index.insert(
            alias_id,
            pub_item(
                alias_id,
                "MyAlias",
                ItemEnum::TypeAlias(TypeAlias { type_: target_type, generics: empty_generics() }),
            ),
        );
        index.insert(
            target_id,
            pub_item(
                target_id,
                "ConcreteType",
                ItemEnum::Struct(Struct {
                    kind: StructKind::Unit,
                    generics: empty_generics(),
                    impls: vec![],
                }),
            ),
        );

        let mut paths = HashMap::new();
        paths.insert(alias_id, item_summary(vec!["my_crate", "MyAlias"], ItemKind::TypeAlias));
        paths.insert(
            target_id,
            ItemSummary {
                crate_id: 0,
                path: vec!["my_crate".to_string(), "ConcreteType".to_string()],
                kind: ItemKind::Struct,
            },
        );

        let krate = make_crate(root_id, index, paths);
        let doc = make_baseline("domain", "my_crate", krate);

        let mut subgraph_lines = vec![];
        let mut edge_lines = vec![];
        let mut class_attach = vec![];
        let style = minimal_style();

        emit_type_alias_subgraph(
            &doc,
            alias_id,
            "domain",
            &mut subgraph_lines,
            &mut edge_lines,
            &mut class_attach,
            &style,
            "  ",
            None,
        )
        .unwrap();

        let joined = subgraph_lines.join("\n");
        assert!(joined.contains("MyAlias"), "subgraph must contain alias name");
        assert_eq!(
            edge_lines.len(),
            1,
            "TypeAlias with own-crate target → 1 alias_of edge; got: {edge_lines:?}"
        );
        assert!(edge_lines[0].contains("alias_of"), "alias edge must contain 'alias_of' label");
        assert!(
            edge_lines[0].contains("---"),
            "alias edge must be undirected (---) per N decision"
        );
        let expected_target = type_rep_node_id("domain", "my_crate", "", "ConcreteType");
        assert!(
            edge_lines[0].contains(&expected_target),
            "alias edge must target ConcreteType rep-node; edge: {}; expected: {}",
            edge_lines[0],
            expected_target
        );
    }

    // -----------------------------------------------------------------------
    // T007: F-r1 — FunctionEntry as standalone callable node (no subgraph)
    // -----------------------------------------------------------------------

    #[test]
    fn test_emit_function_node_produces_node_not_subgraph() {
        let root_id = Id(0);
        let fn_id = Id(1);

        let mut index = HashMap::new();
        index.insert(
            root_id,
            pub_item(
                root_id,
                "my_crate",
                ItemEnum::Module(Module { is_crate: true, items: vec![fn_id], is_stripped: false }),
            ),
        );
        index.insert(fn_id, pub_item(fn_id, "my_fn", empty_function_inner()));

        let mut paths = HashMap::new();
        paths.insert(fn_id, item_summary(vec!["my_crate", "my_fn"], ItemKind::Function));

        let krate = make_crate(root_id, index, paths);
        let doc = make_baseline("domain", "my_crate", krate);

        let mut subgraph_lines = vec![];
        let mut class_attach = vec![];
        let style = minimal_style();

        emit_function_node(
            &doc,
            fn_id,
            "domain",
            &mut subgraph_lines,
            &mut class_attach,
            &style,
            "  ",
        )
        .unwrap();

        let joined = subgraph_lines.join("\n");
        // Must NOT contain 'subgraph' — function is a plain node.
        assert!(
            !joined.contains("subgraph"),
            "FunctionEntry must NOT be wrapped in a subgraph (F-r1 decision)"
        );
        assert!(joined.contains("my_fn"), "function node must mention function name");
    }

    // -----------------------------------------------------------------------
    // T007: missing edge style config → RenderFailed (CN-02 fail-closed)
    // -----------------------------------------------------------------------

    #[test]
    fn test_emit_struct_subgraph_missing_field_edge_style_returns_render_failed() {
        // T016 / CN-02: struct with own-crate field + missing [edge.field] style → RenderFailed.
        // Primitive fields are silently skipped (no edge), so this test uses a ResolvedPath
        // pointing to an own-crate type to trigger the style lookup.
        let root_id = Id(0);
        let struct_id = Id(1);
        let field_id = Id(2);
        let field_type_id = Id(3);

        let field_type = Type::ResolvedPath(rustdoc_types::Path {
            path: "OtherType".to_string(),
            id: field_type_id,
            args: None,
        });

        let mut index = HashMap::new();
        index.insert(
            root_id,
            pub_item(
                root_id,
                "my_crate",
                ItemEnum::Module(Module {
                    is_crate: true,
                    items: vec![struct_id, field_type_id],
                    is_stripped: false,
                }),
            ),
        );
        index.insert(
            struct_id,
            pub_item(
                struct_id,
                "MyStruct",
                ItemEnum::Struct(Struct {
                    kind: StructKind::Plain { fields: vec![field_id], has_stripped_fields: false },
                    generics: empty_generics(),
                    impls: vec![],
                }),
            ),
        );
        index.insert(field_id, pub_item(field_id, "value", ItemEnum::StructField(field_type)));
        index.insert(
            field_type_id,
            pub_item(
                field_type_id,
                "OtherType",
                ItemEnum::Struct(Struct {
                    kind: StructKind::Unit,
                    generics: empty_generics(),
                    impls: vec![],
                }),
            ),
        );

        let mut paths = HashMap::new();
        paths.insert(struct_id, item_summary(vec!["my_crate", "MyStruct"], ItemKind::Struct));
        paths.insert(
            field_type_id,
            ItemSummary {
                crate_id: 0,
                path: vec!["my_crate".to_string(), "OtherType".to_string()],
                kind: ItemKind::Struct,
            },
        );

        let krate = make_crate(root_id, index, paths);
        let doc = make_baseline("domain", "my_crate", krate);

        // Style config with NO [edge.field] → must fail-closed (CN-02).
        let style: StyleConfig = toml::from_str("[filter]\ninclude_functions = true\n").unwrap();

        let mut subgraph_lines = vec![];
        let mut edge_lines = vec![];
        let mut class_attach = vec![];

        let result = emit_struct_subgraph(
            &doc,
            struct_id,
            "domain",
            &mut subgraph_lines,
            &mut edge_lines,
            &mut class_attach,
            &style,
            "  ",
            None,
        );

        assert!(
            matches!(result, Err(BaselineGraphRendererError::RenderFailed { .. })),
            "own-crate field + missing [edge.field] must return RenderFailed (CN-02), got {result:?}"
        );
    }

    // -----------------------------------------------------------------------
    // T016 / AC-20: generic arg recursion — Vec<OwnType> → edge to OwnType
    // -----------------------------------------------------------------------

    #[test]
    fn test_emit_struct_subgraph_generic_arg_own_crate_type_emits_edge() {
        // A struct field of type Vec<OwnCrateType> — the outer Vec is external,
        // but the generic argument OwnCrateType is own-crate.
        // T016 expects: exactly 1 field edge pointing to OwnCrateType (not Vec).
        use rustdoc_types::{GenericArg, GenericArgs};
        let root_id = Id(0);
        let struct_id = Id(1);
        let field_id = Id(2);
        let vec_id = Id(3); // std::vec::Vec — external crate_id != 0
        let inner_id = Id(4); // OwnCrateType — own crate_id == 0

        // Build Vec<OwnCrateType> as a ResolvedPath with one generic type arg.
        let inner_path = Type::ResolvedPath(rustdoc_types::Path {
            path: "OwnCrateType".to_string(),
            id: inner_id,
            args: None,
        });
        let field_type = Type::ResolvedPath(rustdoc_types::Path {
            path: "Vec".to_string(),
            id: vec_id,
            args: Some(Box::new(GenericArgs::AngleBracketed {
                args: vec![GenericArg::Type(inner_path)],
                constraints: vec![],
            })),
        });

        let mut index = HashMap::new();
        index.insert(
            root_id,
            pub_item(
                root_id,
                "my_crate",
                ItemEnum::Module(Module {
                    is_crate: true,
                    items: vec![struct_id, inner_id],
                    is_stripped: false,
                }),
            ),
        );
        index.insert(
            struct_id,
            pub_item(
                struct_id,
                "MyStruct",
                ItemEnum::Struct(Struct {
                    kind: StructKind::Plain { fields: vec![field_id], has_stripped_fields: false },
                    generics: empty_generics(),
                    impls: vec![],
                }),
            ),
        );
        index.insert(field_id, pub_item(field_id, "items", ItemEnum::StructField(field_type)));
        index.insert(
            inner_id,
            pub_item(
                inner_id,
                "OwnCrateType",
                ItemEnum::Struct(Struct {
                    kind: StructKind::Unit,
                    generics: empty_generics(),
                    impls: vec![],
                }),
            ),
        );

        let mut paths = HashMap::new();
        paths.insert(struct_id, item_summary(vec!["my_crate", "MyStruct"], ItemKind::Struct));
        // Vec is external: crate_id != 0
        paths.insert(
            vec_id,
            ItemSummary {
                crate_id: 1, // not own crate
                path: vec!["std".to_string(), "vec".to_string(), "Vec".to_string()],
                kind: ItemKind::Struct,
            },
        );
        paths.insert(
            inner_id,
            ItemSummary {
                crate_id: 0, // own crate
                path: vec!["my_crate".to_string(), "OwnCrateType".to_string()],
                kind: ItemKind::Struct,
            },
        );

        let krate = make_crate(root_id, index, paths);
        let doc = make_baseline("domain", "my_crate", krate);

        let mut subgraph_lines = vec![];
        let mut edge_lines = vec![];
        let mut class_attach = vec![];
        let style = minimal_style();

        emit_struct_subgraph(
            &doc,
            struct_id,
            "domain",
            &mut subgraph_lines,
            &mut edge_lines,
            &mut class_attach,
            &style,
            "  ",
            None,
        )
        .unwrap();

        assert_eq!(
            edge_lines.len(),
            1,
            "Vec<OwnCrateType> field → 1 edge to inner own-crate type; got: {edge_lines:?}"
        );
        let expected_target = type_rep_node_id("domain", "my_crate", "", "OwnCrateType");
        assert!(
            edge_lines[0].contains(&expected_target),
            "edge must target OwnCrateType (generic arg recursion); edge: {}; expected: {}",
            edge_lines[0],
            expected_target
        );
        // No anonymous prim_* / anon_* / generic_* nodes
        let joined = format!("{}\n{}", subgraph_lines.join("\n"), edge_lines.join("\n"));
        assert!(!joined.contains("prim_"), "no prim_* anonymous nodes (T016)");
        assert!(!joined.contains("anon_"), "no anon_* anonymous nodes (T016)");
        assert!(!joined.contains("generic_"), "no generic_* anonymous nodes (T016)");
    }

    // -----------------------------------------------------------------------
    // T007: emit_all_entries_for_layer — integration smoke test
    // -----------------------------------------------------------------------

    #[test]
    fn test_emit_all_entries_for_layer_smoke_test() {
        // Arrange: one crate with a Struct, an Enum (plain variant), a Trait (no methods),
        // a TypeAlias, and a Function — all Public.
        let root_id = Id(0);
        let struct_id = Id(1);
        let enum_id = Id(2);
        let trait_id = Id(3);
        let alias_id = Id(4);
        let fn_id = Id(5);

        let mut index = HashMap::new();
        index.insert(
            root_id,
            pub_item(
                root_id,
                "my_crate",
                ItemEnum::Module(Module {
                    is_crate: true,
                    items: vec![struct_id, enum_id, trait_id, alias_id, fn_id],
                    is_stripped: false,
                }),
            ),
        );
        index.insert(
            struct_id,
            pub_item(
                struct_id,
                "MyStruct",
                ItemEnum::Struct(Struct {
                    kind: StructKind::Unit,
                    generics: empty_generics(),
                    impls: vec![],
                }),
            ),
        );
        index.insert(
            enum_id,
            pub_item(
                enum_id,
                "MyEnum",
                ItemEnum::Enum(Enum {
                    generics: empty_generics(),
                    variants: vec![],
                    impls: vec![],
                    has_stripped_variants: false,
                }),
            ),
        );
        index.insert(
            trait_id,
            pub_item(
                trait_id,
                "MyTrait",
                ItemEnum::Trait(Trait {
                    is_auto: false,
                    is_unsafe: false,
                    is_dyn_compatible: true,
                    items: vec![],
                    generics: empty_generics(),
                    bounds: vec![],
                    implementations: vec![],
                }),
            ),
        );
        index.insert(
            alias_id,
            pub_item(
                alias_id,
                "MyAlias",
                ItemEnum::TypeAlias(TypeAlias {
                    type_: Type::Primitive("u32".to_string()),
                    generics: empty_generics(),
                }),
            ),
        );
        index.insert(fn_id, pub_item(fn_id, "my_fn", empty_function_inner()));

        let mut paths = HashMap::new();
        paths.insert(struct_id, item_summary(vec!["my_crate", "MyStruct"], ItemKind::Struct));
        paths.insert(enum_id, item_summary(vec!["my_crate", "MyEnum"], ItemKind::Enum));
        paths.insert(trait_id, item_summary(vec!["my_crate", "MyTrait"], ItemKind::Trait));
        paths.insert(alias_id, item_summary(vec!["my_crate", "MyAlias"], ItemKind::TypeAlias));
        paths.insert(fn_id, item_summary(vec!["my_crate", "my_fn"], ItemKind::Function));

        let krate = make_crate(root_id, index, paths);
        let baseline = make_baseline("domain", "my_crate", krate);

        let mut subgraph_lines = vec![];
        let mut edge_lines = vec![];
        let mut class_attach = vec![];
        let style = minimal_style();

        emit_all_entries_for_layer(
            &[baseline],
            "domain",
            None,
            &mut subgraph_lines,
            &mut edge_lines,
            &mut class_attach,
            &style,
            "  ",
        )
        .unwrap();

        let joined = subgraph_lines.join("\n");

        // All 5 entry kinds must appear.
        assert!(joined.contains("MyStruct"), "Struct must appear");
        assert!(joined.contains("MyEnum"), "Enum must appear");
        assert!(joined.contains("MyTrait"), "Trait must appear");
        assert!(joined.contains("MyAlias"), "TypeAlias must appear");
        assert!(joined.contains("my_fn"), "Function must appear");

        // Struct/Enum/Trait/TypeAlias are subgraphs; Function is a plain node.
        let subgraph_count = subgraph_lines.iter().filter(|l| l.contains("subgraph")).count();
        assert_eq!(subgraph_count, 4, "4 entry kinds → 4 subgraphs (Struct/Enum/Trait/TypeAlias)");
    }

    // -----------------------------------------------------------------------
    // T007: layer-agnostic — custom layer name propagates correctly
    // -----------------------------------------------------------------------

    #[test]
    fn test_emit_struct_subgraph_layer_agnostic() {
        let root_id = Id(0);
        let struct_id = Id(1);

        let mut index = HashMap::new();
        index.insert(
            root_id,
            pub_item(
                root_id,
                "my_crate",
                ItemEnum::Module(Module {
                    is_crate: true,
                    items: vec![struct_id],
                    is_stripped: false,
                }),
            ),
        );
        index.insert(
            struct_id,
            pub_item(
                struct_id,
                "Config",
                ItemEnum::Struct(Struct {
                    kind: StructKind::Unit,
                    generics: empty_generics(),
                    impls: vec![],
                }),
            ),
        );

        let mut paths = HashMap::new();
        paths.insert(struct_id, item_summary(vec!["my_crate", "Config"], ItemKind::Struct));

        let krate = make_crate(root_id, index, paths);

        // Use a custom layer name — not hardcoded to "domain"/"infrastructure".
        let doc = make_baseline("my_custom_layer", "my_crate", krate);

        let mut subgraph_lines = vec![];
        let mut edge_lines = vec![];
        let mut class_attach = vec![];
        let style = minimal_style();

        emit_struct_subgraph(
            &doc,
            struct_id,
            "my_custom_layer",
            &mut subgraph_lines,
            &mut edge_lines,
            &mut class_attach,
            &style,
            "  ",
            None,
        )
        .unwrap();

        let rep_id = type_rep_node_id("my_custom_layer", "my_crate", "", "Config");
        let joined = subgraph_lines.join("\n");
        assert!(joined.contains(&rep_id), "node_id must embed the custom layer name");
    }
}
