//! Individual entry subgraph emission for the baseline-graph renderer (T007 / T008 / T015 / T016).
//!
//! Implements the following ADR decisions:
//!
//! - **F-r1**: Struct / Enum / Trait / TypeAlias entries are rendered as mermaid
//!   subgraphs. FunctionEntry is rendered as a standalone callable node.
//! - **H**: Enum variants are node-ified inside the entry subgraph with payload edges.
//! - **H'**: Trait.items are scanned for `ItemEnum::Function` entries as method nodes.
//! - **K**: PlainStruct fields → field edges; TupleStruct fields → indexed edges.
//! - **N**: TypeAlias → undirected `---|alias_of|` edge to own-crate target types.
//! - **BB-4-fix1 (T008)**: Inherent impl methods merged into type entry subgraphs.
//! - **AC-19 / AC-20 (T015 — method path)**: method nodes emit `method_param` /
//!   `method_returns` edges to own-crate types.
//!
//! All functions are panic-free (no `unwrap` / `expect` / slice indexing on `[i]`
//! in production code — only `.get()` / iterators).
//!
//! (IN-06 / IN-07 / IN-08 / IN-09 / IN-10 / IN-11 / IN-13 / AC-04 / AC-05 / AC-06 /
//! AC-07 / AC-08 / AC-09 / AC-10 / AC-19 / AC-20)

use rustdoc_types::{Id, ItemEnum, StructKind, VariantKind, Visibility};

use domain::tddd::baseline_document::BaselineDocument;
use domain::tddd::baseline_graph_ports::BaselineGraphRendererError;

use super::impl_processor::{BlanketBodyEntry, emit_inherent_methods, emit_method_signature_edges};
use super::node_id_generator::{
    function_node_id, module_path_from_summary, trait_node_id, trait_rep_node_id, type_node_id,
    type_rep_node_id,
};
use super::style_config::{StyleConfig, apply_shape, edge_arrow_label, sanitize};
use super::type_resolver::collect_own_crate_node_ids_from_type;

// ---------------------------------------------------------------------------
// Individual entry subgraph emitters
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
        emit_inherent_methods(
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
        emit_inherent_methods(
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
            emit_method_signature_edges(
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
        emit_inherent_methods(
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
pub(super) fn build_entry_label(summary_path: &[String], name: &str) -> String {
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
