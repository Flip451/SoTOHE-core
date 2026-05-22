//! Entry subgraph emission for the baseline-graph renderer (T007).
//!
//! Implements the following ADR decisions:
//!
//! - **F-r1**: Struct / Enum / Trait / TypeAlias entries are rendered as mermaid
//!   subgraphs. FunctionEntry is rendered as a standalone callable node (out of scope here).
//! - **H**: Enum variants are node-ified inside the entry subgraph. Payload edges:
//!   `VariantKind::Tuple` → `--o` per element; `VariantKind::Struct` → `--o|field_name|`;
//!   `VariantKind::Plain` → no edge.
//! - **H'**: Trait.items are scanned for `ItemEnum::Function` entries; each becomes a
//!   method node inside the Trait subgraph.
//! - **K**: PlainStruct fields → `--o|field_name|` edge per field; TupleStruct fields →
//!   `--o|.N|` (positional index); `has_stripped_fields: true` or `None` slot → skip;
//!   Unit → no edge.
//! - **N**: TypeAlias → undirected `---|alias_of|` edge to the target type subgraph.
//!   The target is identified by its type_node_id; if the target is not a locally known
//!   type it is emitted as an anonymous node (best-effort without cross-baseline lookup).
//!
//! All functions are panic-free (no `unwrap` / `expect` / slice indexing on `[i]`
//! in production code — only `.get()` / iterators).
//!
//! (IN-06 / IN-07 / IN-08 / IN-10 / IN-11 / AC-04 / AC-06 / AC-07 / AC-09 / AC-10)

use std::collections::BTreeMap;

use rustdoc_types::{Id, ItemEnum, StructKind, Type, VariantKind, Visibility};

use domain::tddd::baseline_document::BaselineDocument;
use domain::tddd::baseline_graph_ports::BaselineGraphRendererError;

use super::node_id_generator::{
    function_node_id, module_path_from_summary, trait_node_id, trait_rep_node_id, type_node_id,
    type_rep_node_id,
};
use super::{StyleConfig, apply_shape, edge_arrow_label, sanitize};

// ---------------------------------------------------------------------------
// Public entry points
// ---------------------------------------------------------------------------

/// Emit a Struct entry subgraph (F-r1 / K decision).
///
/// Struct / Enum / Trait / TypeAlias become subgraphs; FunctionEntry is handled
/// separately as a standalone node.
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

    // K decision: struct fields.
    // The [edge.field] style is looked up lazily — only immediately before the first
    // edge is emitted — so that structs with no renderable fields (empty plain struct,
    // all-None tuple struct, has_stripped_fields=true, or Unit) do not fail when
    // [edge.field] is absent from the style config.
    if let ItemEnum::Struct(s) = &item.inner {
        match &s.kind {
            StructKind::Plain { fields, has_stripped_fields } => {
                if !has_stripped_fields {
                    for &field_id in fields {
                        if let Some(field_item) = krate.index.get(&field_id) {
                            let field_name = field_item.name.as_deref().unwrap_or("?");
                            if let ItemEnum::StructField(field_ty) = &field_item.inner {
                                // Lazy lookup: only called when an edge is about to be emitted.
                                let (field_arrow, _) = edge_arrow_label(&style.edge, "field")?;
                                let target_id =
                                    field_type_node_id(field_ty, krate, layer, crate_name);
                                edge_lines.push(format!(
                                    "{rep_node_id} {field_arrow}|\"{field_name}\"| {target_id}"
                                ));
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
                                // Lazy lookup: only called when an edge is about to be emitted.
                                let (field_arrow, _) = edge_arrow_label(&style.edge, "field")?;
                                let target_id =
                                    field_type_node_id(field_ty, krate, layer, crate_name);
                                let label = format!(".{idx}");
                                edge_lines.push(format!(
                                    "{rep_node_id} {field_arrow}|\"{label}\"| {target_id}"
                                ));
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

            // Payload edges (H decision).
            if let ItemEnum::Variant(variant_data) = &variant_item.inner {
                match &variant_data.kind {
                    VariantKind::Plain => {
                        // No edge.
                    }
                    VariantKind::Tuple(field_ids) => {
                        // Each Some(Id) → lookup StructField → `--o` edge (no label).
                        for maybe_id in field_ids {
                            if let Some(&fid) = maybe_id.as_ref() {
                                if let Some(field_item) = krate.index.get(&fid) {
                                    if let ItemEnum::StructField(field_ty) = &field_item.inner {
                                        // Lazy lookup: only called when an edge is about to be emitted.
                                        let (payload_arrow, _) =
                                            edge_arrow_label(&style.edge, "variant_payload")?;
                                        let target_id =
                                            field_type_node_id(field_ty, krate, layer, crate_name);
                                        edge_lines.push(format!(
                                            "{variant_node_id} {payload_arrow} {target_id}"
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    VariantKind::Struct { fields, has_stripped_fields } => {
                        // Skip edge emission when the variant has stripped (hidden) fields
                        // (consistent with K decision: has_stripped_fields → render nothing).
                        if !has_stripped_fields {
                            // Each field → `--o|field_name|` edge (H decision).
                            for &fid in fields {
                                if let Some(field_item) = krate.index.get(&fid) {
                                    let field_name = field_item.name.as_deref().unwrap_or("?");
                                    if let ItemEnum::StructField(field_ty) = &field_item.inner {
                                        // Lazy lookup: only called when an edge is about to be emitted.
                                        let (payload_arrow, _) =
                                            edge_arrow_label(&style.edge, "variant_payload")?;
                                        let target_id =
                                            field_type_node_id(field_ty, krate, layer, crate_name);
                                        edge_lines.push(format!(
                                            "{variant_node_id} {payload_arrow}|\"{field_name}\"| {target_id}"
                                        ));
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

    // Close subgraph.
    subgraph_lines.push(format!("{indent}end"));

    if let Some(ns) = style.node.get("Type") {
        if let Some(class_name) = ns.class.as_deref() {
            class_attach.push(format!("class {entry_sg_id} {class_name}"));
        }
    }

    Ok(())
}

/// Emit a Trait entry subgraph with method nodes (F-r1 / H' decision).
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
    _edge_lines: &mut Vec<String>,
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
            if !matches!(method_item.inner, ItemEnum::Function(_)) {
                continue;
            }
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

    // Close subgraph.
    subgraph_lines.push(format!("{indent}end"));

    if let Some(ns) = style.node.get("Type") {
        if let Some(class_name) = ns.class.as_deref() {
            class_attach.push(format!("class {entry_sg_id} {class_name}"));
        }
    }

    // N decision: undirected alias_of edge to the target type.
    if let ItemEnum::TypeAlias(alias_data) = &item.inner {
        let (alias_arrow, _) = edge_arrow_label(&style.edge, "alias")?;
        let target_id = field_type_node_id(&alias_data.type_, krate, layer, crate_name);
        edge_lines.push(format!("{rep_node_id} {alias_arrow}|\"alias_of\"| {target_id}"));
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
/// Entries are sorted alphabetically by their fully-qualified key (module_path + name)
/// within each crate (CN-08). Functions are emitted after the subgraph entries
/// (alphabetical by full path, CN-08).
///
/// # Errors
///
/// Returns `BaselineGraphRendererError::RenderFailed` when a required `[edge.*]`
/// key is absent from the style config (CN-02 — fail-closed).
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
    use super::node_extractor::{ExtractedNode, extract_nodes};

    // Extract all B-r1 nodes for this layer.
    let layer_id = domain::tddd::layer_id::LayerId::try_new(layer).map_err(|_| {
        BaselineGraphRendererError::RenderFailed { reason: format!("invalid layer name: {layer}") }
    })?;

    let nodes = extract_nodes(baselines, &layer_id);

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
        // Apply crate filter: skip nodes from crtes that don't match the requested crate.
        if let Some(filter) = crate_filter {
            if doc.crate_name.as_str() != filter {
                continue;
            }
        }
        let item = node.item();
        let id = node.id();
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
        emit_struct_subgraph(
            doc,
            *id,
            layer,
            subgraph_lines,
            edge_lines,
            class_attach,
            style,
            indent,
        )?;
    }
    for (id, doc) in enums.values() {
        emit_enum_subgraph(
            doc,
            *id,
            layer,
            subgraph_lines,
            edge_lines,
            class_attach,
            style,
            indent,
        )?;
    }
    for (id, doc) in traits.values() {
        emit_trait_subgraph(
            doc,
            *id,
            layer,
            subgraph_lines,
            edge_lines,
            class_attach,
            style,
            indent,
        )?;
    }
    for (id, doc) in aliases.values() {
        emit_type_alias_subgraph(
            doc,
            *id,
            layer,
            subgraph_lines,
            edge_lines,
            class_attach,
            style,
            indent,
        )?;
    }

    // Emit FunctionEntry callable nodes (alphabetical by full path, CN-08).
    for (id, doc) in functions.values() {
        emit_function_node(doc, *id, layer, subgraph_lines, class_attach, style, indent)?;
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

/// Resolve a `rustdoc_types::Type` to a mermaid node_id for use as an edge endpoint.
///
/// For same-crate `ResolvedPath` types, returns the **representative node id** (`__self`
/// suffix) so that edges terminate on the concrete node inside the target subgraph rather
/// than on the subgraph container itself (Finding 3 / spec N decision: edges point to
/// the rep-node of the target type, not the subgraph container).
///
/// Handles `Type::ResolvedPath` (lookup via krate.paths), `Type::Primitive`, and
/// generic/other types.  For non-resolved types an anonymous descriptive node_id
/// is generated (best-effort — cross-crate lookups not yet implemented per O-r1
/// scope of T007).
fn field_type_node_id(
    ty: &Type,
    krate: &rustdoc_types::Crate,
    layer: &str,
    crate_name: &str,
) -> String {
    match ty {
        Type::ResolvedPath(path) => {
            // Try to look up the target in krate.paths.
            if let Some(summary) = krate.paths.get(&path.id) {
                // Only handle same-crate items (crate_id == 0).
                if summary.crate_id == 0 {
                    let module_path = module_path_from_summary(&summary.path);
                    let type_name = summary.path.last().map(|s| s.as_str()).unwrap_or("?");
                    // Return the rep-node id (__self) so edges target the concrete node
                    // inside the target subgraph, not the subgraph container.
                    return type_rep_node_id(layer, crate_name, &module_path, type_name);
                }
            }
            // External type or not found — emit an anonymous node using the path string.
            let anon_name = sanitize(&path.path);
            format!("anon_{anon_name}")
        }
        Type::Primitive(prim) => {
            // Primitive types (u32, bool, etc.) — emit a shared anonymous node.
            let sanitized = sanitize(prim);
            format!("prim_{sanitized}")
        }
        Type::Generic(g) => {
            let sanitized = sanitize(g);
            format!("generic_{sanitized}")
        }
        _ => {
            // Tuple, Slice, Array, DynTrait, etc. — best-effort fallback.
            format!("anon_type_{}", sanitize(&format!("{ty:?}")))
        }
    }
}

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
    fn test_emit_struct_subgraph_plain_struct_with_fields_emits_edges() {
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
        )
        .unwrap();

        assert_eq!(edge_lines.len(), 1, "one field → one edge");
        assert!(edge_lines[0].contains("value"), "edge must reference field name 'value'");
        assert!(edge_lines[0].contains("--o"), "field edge must use --o arrow");
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
        )
        .unwrap();

        assert!(edge_lines.is_empty(), "has_stripped_fields=true → no field edges (K decision)");
    }

    #[test]
    fn test_emit_struct_subgraph_tuple_struct_emits_positional_edges() {
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
        )
        .unwrap();

        assert_eq!(edge_lines.len(), 2, "two fields → two positional edges");
        assert!(edge_lines[0].contains(".0"), "first edge must use .0 label");
        assert!(edge_lines[1].contains(".1"), "second edge must use .1 label");
    }

    #[test]
    fn test_emit_struct_subgraph_tuple_struct_none_slot_skipped() {
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
        )
        .unwrap();

        assert_eq!(edge_lines.len(), 1, "None slot must be skipped; only 1 edge expected");
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
        )
        .unwrap();

        let joined = subgraph_lines.join("\n");
        assert!(joined.contains("PlainVariant"), "plain variant node must be emitted");
        assert!(edge_lines.is_empty(), "PlainVariant → no payload edges (H decision)");
    }

    #[test]
    fn test_emit_enum_subgraph_tuple_variant_emits_payload_edges() {
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
        )
        .unwrap();

        assert_eq!(edge_lines.len(), 1, "tuple variant with 1 field → 1 payload edge");
        assert!(edge_lines[0].contains("--o"), "tuple variant payload edge must use --o");
        // No label for tuple variant (unlabeled edge).
        assert!(
            !edge_lines[0].contains("field_name"),
            "tuple variant payload edge must not have a field name label"
        );
    }

    #[test]
    fn test_emit_enum_subgraph_struct_variant_emits_named_edges() {
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
        )
        .unwrap();

        assert_eq!(edge_lines.len(), 1, "struct variant with 1 named field → 1 edge");
        assert!(
            edge_lines[0].contains("username"),
            "struct variant edge must include field name 'username'"
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
    // T007: N decision — TypeAlias alias edge
    // -----------------------------------------------------------------------

    #[test]
    fn test_emit_type_alias_subgraph_emits_alias_of_edge() {
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
        )
        .unwrap();

        let joined = subgraph_lines.join("\n");
        assert!(joined.contains("MyAlias"), "subgraph must contain alias name");
        assert_eq!(edge_lines.len(), 1, "TypeAlias must produce exactly 1 alias edge (N decision)");
        assert!(edge_lines[0].contains("alias_of"), "alias edge must contain 'alias_of' label");
        assert!(
            edge_lines[0].contains("---"),
            "alias edge must be undirected (---) per N decision"
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

        // Style config with NO [edge.field] → must fail-closed.
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
        );

        assert!(
            matches!(result, Err(BaselineGraphRendererError::RenderFailed { .. })),
            "missing [edge.field] must return RenderFailed (CN-02), got {result:?}"
        );
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
        )
        .unwrap();

        let rep_id = type_rep_node_id("my_custom_layer", "my_crate", "", "Config");
        let joined = subgraph_lines.join("\n");
        assert!(joined.contains(&rep_id), "node_id must embed the custom layer name");
    }
}
