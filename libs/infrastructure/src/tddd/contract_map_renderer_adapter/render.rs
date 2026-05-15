//! Mermaid rendering logic for [`super::super::ContractMapRendererAdapter`].
//!
//! Implements T006 scope:
//! - 4-level subgraph nesting: layer → top-module → entry → method (Decision U-6d-iii)
//! - TypeEntry / TraitEntry rendered as subgraphs (Decision F-2+b2-ii)
//! - FunctionEntry as standalone callable node (Decision F-2+d1)
//! - method → param type edges (`--o`) and method → return type edges (`-->`)
//! - PlainStruct.fields → entry → field type edges (`--o|field_name|`) (Decision K-2+(d))
//! - TupleStruct.fields → entry → field type edges (`--o|.N|`) (Decision K-2)
//! - module_path = [] entries placed directly in layer subgraph (AC-11)
//! - Same-catalogue TypeRef resolution; unresolved / cross-crate refs silently skipped
//!
//! Out of scope for T006 (deferred to T007 / T008):
//! - Enum variant nodes and payload edges (T007)
//! - TypeAlias undirected edge (T007)
//! - Typestate transition edges (T007)
//! - Cross-catalogue trait_impl edges (T008)
//! - classDef alphabetical ordering + `class <id> <className>` attach lines (T008)

mod builder;
mod type_index;

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use domain::tddd::catalogue_v2::composite::TypeKindV2;
use domain::tddd::catalogue_v2::entries::{FunctionEntry, TraitEntry, TypeEntry};
use domain::tddd::catalogue_v2::identifiers::{CrateName, ModulePath, TypeRef};
use domain::tddd::catalogue_v2::{CatalogueDocument, FunctionPath, TraitName, TypeName};
use domain::tddd::{ContractMapRenderOptions, LayerId};

use super::{EdgeStyle, NodeStyle, StyleConfig, function_node_id, trait_node_id, type_node_id};
use builder::MermaidBuilder;
use type_index::TypeIndex;

// ---------------------------------------------------------------------------
// Public render entry point
// ---------------------------------------------------------------------------

/// Renders a mermaid `flowchart TD` string from the given catalogues.
///
/// Implements Decision U-6d-iii (4-level nesting: layer → top-module → entry →
/// method), Decision F-2+b2-ii (TypeEntry / TraitEntry as subgraphs), and
/// Decision F-2+d1 (FunctionEntry as standalone node). Field edges and method
/// edges are emitted after subgraph declarations.
///
/// Layer iteration follows `layer_order`. When `opts.layers` is non-empty it
/// acts as an allowlist: only layers present in both `layer_order` and
/// `opts.layers` are emitted (preserving `layer_order` ordering). When
/// `opts.layers` is empty all layers from `layer_order` are emitted. Catalogue
/// layers absent from `layer_order` are always silently skipped regardless of
/// the filter state.
///
/// Within each layer, documents are sorted by `crate_name` (alphabetical).
/// Within each document, entries are iterated in BTreeMap alphabetical order
/// (TypeName / TraitName / FunctionPath).
pub(super) fn render_mermaid(
    catalogues: &[CatalogueDocument],
    layer_order: &[LayerId],
    opts: &ContractMapRenderOptions,
    style: &StyleConfig,
) -> String {
    // Build the type index for same-catalogue TypeRef resolution.
    let type_index = TypeIndex::build(catalogues);

    // Group documents by layer.
    let mut by_layer: BTreeMap<&str, Vec<&CatalogueDocument>> = BTreeMap::new();
    for doc in catalogues {
        by_layer.entry(doc.layer.as_ref()).or_default().push(doc);
    }
    // Sort each layer's documents by crate_name (alphabetical).
    for docs in by_layer.values_mut() {
        docs.sort_by_key(|d| d.crate_name.as_str());
    }

    // Compute the layer allowlist when opts.layers is non-empty.
    // An empty allowlist means "render all layers".
    let allowlist: BTreeSet<&str> = opts.layers.iter().map(|l| l.as_ref()).collect();
    let has_filter = !allowlist.is_empty();

    // Collect minimal classDef lines (T006: emit class definitions but no
    // ordering enforcement — T008 finalizes alphabetical ordering).
    let classdefs = collect_classdefs(style);

    let mut builder = MermaidBuilder::new();

    // Emit layers in the order specified by layer_order, applying the filter.
    // Catalogue layers absent from layer_order are always silently skipped.
    for layer_id in layer_order {
        let layer_str = layer_id.as_ref();
        // Skip layers excluded by the opts.layers allowlist.
        if has_filter && !allowlist.contains(layer_str) {
            continue;
        }
        if let Some(docs) = by_layer.get(layer_str) {
            emit_layer(&mut builder, layer_id, docs, &type_index, style);
        }
    }

    builder.build(&classdefs)
}

// ---------------------------------------------------------------------------
// Layer rendering
// ---------------------------------------------------------------------------

/// Injectively encodes a raw component string into a Mermaid-safe identifier segment.
///
/// Encoding rules (applied char-by-char):
/// - ASCII alphanumeric → pass through unchanged.
/// - `_` → `__` (double underscore).
/// - `-` → `_d_` (mnemonic: d for dash).
/// - Any other character → `_x<hex>_` (not expected for `LayerId`/`ModulePath` inputs).
///
/// This makes the mapping injective: distinct inputs always produce distinct outputs.
/// For example, `"my-layer"` → `"my_d_layer"` and `"my_layer"` → `"my__layer"`,
/// which are distinct despite having the same length and the same sanitized form.
///
/// Top-module segments come from `ModulePath`, which only allows valid Rust identifiers
/// (`[a-zA-Z0-9_]`), so `_` is the only special character they can contain.
fn escape_id_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else if ch == '_' {
            out.push_str("__");
        } else if ch == '-' {
            out.push_str("_d_");
        } else {
            // Fallback for unexpected characters (e.g. Unicode); encode as hex.
            let code = ch as u32;
            out.push_str(&format!("_x{code:x}_"));
        }
    }
    out
}

/// Generates an injective mermaid subgraph id for an architecture layer.
///
/// Format: `L_<escaped_layer>` where `escaped_layer` is produced by
/// [`escape_id_component`]. The encoding is injective: distinct `LayerId` values
/// always produce distinct ids (e.g. `my-layer` → `L_my_d_layer` and
/// `my_layer` → `L_my__layer`).
fn layer_sg_id(layer_str: &str) -> String {
    format!("L_{}", escape_id_component(layer_str))
}

/// Generates an injective mermaid subgraph id for a top-module within a layer.
///
/// Format: `<layer_sg_id>_M_<escaped_top_seg>` where both components use the
/// injective [`escape_id_component`] encoding. Distinct `(layer, top_seg)` pairs
/// always produce distinct ids.
fn top_module_sg_id(layer_str: &str, top_seg: &str) -> String {
    format!("{}_M_{}", layer_sg_id(layer_str), escape_id_component(top_seg))
}

/// Emits one layer subgraph with all its top-module subgraphs and entries.
fn emit_layer(
    builder: &mut MermaidBuilder,
    layer_id: &LayerId,
    docs: &[&CatalogueDocument],
    type_index: &TypeIndex,
    style: &StyleConfig,
) {
    let layer_str = layer_id.as_ref();
    let sg_id = layer_sg_id(layer_str);

    builder.open_subgraph(&sg_id, layer_str);

    // Collect all entries across this layer's documents grouped by top-module.
    // `module_path = []` entries go directly in the layer subgraph (AC-11).
    let mut top_module_map: BTreeMap<String, Vec<LayerEntry<'_>>> = BTreeMap::new();
    let mut root_entries: Vec<LayerEntry<'_>> = Vec::new();

    for doc in docs {
        collect_layer_entries(doc, layer_id, &mut top_module_map, &mut root_entries);
    }

    // Emit crate-root entries directly in the layer subgraph (AC-11).
    for entry in &root_entries {
        emit_layer_entry(builder, entry, layer_id, type_index, style);
    }

    // Emit top-module subgraphs in alphabetical order (BTreeMap iter is sorted).
    for (top_seg, entries) in &top_module_map {
        let top_module_id = top_module_sg_id(layer_str, top_seg);
        let top_module_label = format!("{layer_str}::{top_seg}");

        builder.open_subgraph(&top_module_id, &top_module_label);

        for entry in entries {
            emit_layer_entry(builder, entry, layer_id, type_index, style);
        }

        builder.close_subgraph();
    }

    builder.close_subgraph();
}

// ---------------------------------------------------------------------------
// Entry collection helpers
// ---------------------------------------------------------------------------

/// Discriminated entry reference used during layer rendering.
///
/// `crate_name` is carried alongside the entry so that TypeRef resolution
/// stays scoped to the originating catalogue document (same-catalogue semantics).
enum LayerEntry<'a> {
    Type {
        name: &'a TypeName,
        entry: &'a TypeEntry,
        module_path: &'a ModulePath,
        crate_name: &'a CrateName,
    },
    Trait {
        name: &'a TraitName,
        entry: &'a TraitEntry,
        module_path: &'a ModulePath,
        crate_name: &'a CrateName,
    },
    Function {
        path: &'a FunctionPath,
        entry: &'a FunctionEntry,
        module_path: &'a ModulePath,
        crate_name: &'a CrateName,
    },
}

/// Partitions a document's entries into root entries and top-module buckets.
///
/// For `TypeEntry` and `TraitEntry`, `module_path` is stored on the entry itself.
/// For `FunctionEntry`, `module_path` is stored in the `FunctionPath` key (the
/// map key in `CatalogueDocument::functions`). The document's `crate_name` is
/// threaded into every `LayerEntry` for same-catalogue TypeRef resolution.
fn collect_layer_entries<'a>(
    doc: &'a CatalogueDocument,
    _layer_id: &LayerId,
    top_module_map: &mut BTreeMap<String, Vec<LayerEntry<'a>>>,
    root_entries: &mut Vec<LayerEntry<'a>>,
) {
    let crate_name = &doc.crate_name;
    for (name, entry) in &doc.types {
        let module_path = &entry.module_path;
        let le = LayerEntry::Type { name, entry, module_path, crate_name };
        push_entry(le, module_path, top_module_map, root_entries);
    }
    for (name, entry) in &doc.traits {
        let module_path = &entry.module_path;
        let le = LayerEntry::Trait { name, entry, module_path, crate_name };
        push_entry(le, module_path, top_module_map, root_entries);
    }
    for (path, entry) in &doc.functions {
        // FunctionEntry has no module_path field; use the FunctionPath key's module_path.
        let module_path = &path.module_path;
        let le = LayerEntry::Function { path, entry, module_path, crate_name };
        push_entry(le, module_path, top_module_map, root_entries);
    }
}

/// Routes an entry into the appropriate bucket (root vs. top-module).
fn push_entry<'a>(
    le: LayerEntry<'a>,
    module_path: &ModulePath,
    top_module_map: &mut BTreeMap<String, Vec<LayerEntry<'a>>>,
    root_entries: &mut Vec<LayerEntry<'a>>,
) {
    match module_path.segments().first() {
        None => {
            // module_path = [] → directly in layer subgraph (AC-11).
            root_entries.push(le);
        }
        Some(top_seg) => {
            top_module_map.entry(top_seg.as_str().to_owned()).or_default().push(le);
        }
    }
}

// ---------------------------------------------------------------------------
// Entry rendering dispatch
// ---------------------------------------------------------------------------

/// Emits a single catalogue entry (Type subgraph / Trait subgraph / Function node).
fn emit_layer_entry(
    builder: &mut MermaidBuilder,
    entry: &LayerEntry<'_>,
    layer_id: &LayerId,
    type_index: &TypeIndex,
    style: &StyleConfig,
) {
    match entry {
        LayerEntry::Type { name, entry, module_path, crate_name } => {
            emit_type_subgraph(
                builder,
                layer_id,
                crate_name,
                name,
                entry,
                module_path,
                type_index,
                style,
            );
        }
        LayerEntry::Trait { name, entry, module_path, crate_name } => {
            emit_trait_subgraph(
                builder,
                layer_id,
                crate_name,
                name,
                entry,
                module_path,
                type_index,
                style,
            );
        }
        LayerEntry::Function { path, entry, module_path, crate_name } => {
            emit_function_node(
                builder,
                layer_id,
                crate_name,
                path,
                entry,
                module_path,
                type_index,
                style,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// TypeEntry subgraph rendering
// ---------------------------------------------------------------------------

/// Emits a TypeEntry as a mermaid subgraph with method nodes inside.
///
/// Subgraph id: `T<len>_<sanitized_layer>_<sanitized_crate>_<sanitized_name>` (Decision D-2).
/// Label: sub-module path + name (e.g. `team::manager::TeamManager` when
/// `module_path = ["team", "manager"]` and `name = "TeamManager"`).
/// When module_path is root, label = `name`.
#[allow(clippy::too_many_arguments)]
fn emit_type_subgraph(
    builder: &mut MermaidBuilder,
    layer_id: &LayerId,
    crate_name: &CrateName,
    name: &TypeName,
    entry: &TypeEntry,
    module_path: &ModulePath,
    type_index: &TypeIndex,
    style: &StyleConfig,
) {
    let entry_id = type_node_id(layer_id, crate_name, name);
    let label = entry_label(module_path, name.as_str());

    builder.open_subgraph(&entry_id, &label);

    // Emit method nodes inside the entry subgraph.
    let method_shape = node_shape("Method", style);
    for (i, method) in entry.methods.iter().enumerate() {
        let method_id = format!("{entry_id}_m_{i}");
        builder.push_method_node(&method_id, method.name.as_str(), &method_shape);

        // Collect method param edges.
        for param in &method.params {
            collect_param_edge(builder, &method_id, &param.ty, crate_name, type_index, style);
        }
        // Collect method returns edge.
        collect_returns_edge(builder, &method_id, &method.returns, crate_name, type_index, style);
    }

    builder.close_subgraph();

    // Emit field edges for PlainStruct / TupleStruct (Decision K-2+(d), K-2).
    emit_field_edges(builder, &entry_id, &entry.kind, crate_name, type_index, style);

    // Emit class attach for entry subgraph.
    let class_name = role_class_name(&entry.role.to_string(), style);
    builder.push_class(&entry_id, &class_name);

    // Emit class attach for each method node.
    for (i, _method) in entry.methods.iter().enumerate() {
        let method_id = format!("{entry_id}_m_{i}");
        let method_class = node_class_name("Method", style);
        builder.push_class(&method_id, &method_class);
    }
}

// ---------------------------------------------------------------------------
// TraitEntry subgraph rendering
// ---------------------------------------------------------------------------

/// Emits a TraitEntry as a mermaid subgraph with method nodes inside.
///
/// Subgraph id: `R<len>_<sanitized_layer>_<sanitized_crate>_<sanitized_name>` (Decision D-2).
/// Label: sub-module path + name.
#[allow(clippy::too_many_arguments)]
fn emit_trait_subgraph(
    builder: &mut MermaidBuilder,
    layer_id: &LayerId,
    crate_name: &CrateName,
    name: &TraitName,
    entry: &TraitEntry,
    module_path: &ModulePath,
    type_index: &TypeIndex,
    style: &StyleConfig,
) {
    let entry_id = trait_node_id(layer_id, crate_name, name);
    let label = entry_label(module_path, name.as_str());

    builder.open_subgraph(&entry_id, &label);

    // Emit method nodes inside the entry subgraph.
    let method_shape = node_shape("Method", style);
    for (i, method) in entry.methods.iter().enumerate() {
        let method_id = format!("{entry_id}_m_{i}");
        builder.push_method_node(&method_id, method.name.as_str(), &method_shape);

        // Collect method param edges.
        for param in &method.params {
            collect_param_edge(builder, &method_id, &param.ty, crate_name, type_index, style);
        }
        // Collect method returns edge.
        collect_returns_edge(builder, &method_id, &method.returns, crate_name, type_index, style);
    }

    builder.close_subgraph();

    // Emit class attach for entry subgraph.
    let class_name = role_class_name(&entry.role.to_string(), style);
    builder.push_class(&entry_id, &class_name);

    // Emit class attach for each method node.
    for (i, _method) in entry.methods.iter().enumerate() {
        let method_id = format!("{entry_id}_m_{i}");
        let method_class = node_class_name("Method", style);
        builder.push_class(&method_id, &method_class);
    }
}

// ---------------------------------------------------------------------------
// FunctionEntry node rendering
// ---------------------------------------------------------------------------

/// Emits a FunctionEntry as a standalone callable node (Decision F-2+d1).
///
/// Node shape is driven by `[node.Function].shape` in the style config (e.g. `"subroutine"`
/// → `[[name]]`). Node id: `F<len>_<sanitized_layer>_<sanitized_full_path>` (Decision D-2).
/// NOT a subgraph — entry subgraphs and function nodes are parallel siblings.
/// `crate_name` scopes TypeRef resolution to the same catalogue document.
#[allow(clippy::too_many_arguments)]
fn emit_function_node(
    builder: &mut MermaidBuilder,
    layer_id: &LayerId,
    crate_name: &CrateName,
    path: &FunctionPath,
    entry: &FunctionEntry,
    _module_path: &ModulePath,
    type_index: &TypeIndex,
    style: &StyleConfig,
) {
    let fn_id = function_node_id(layer_id, path);
    let fn_shape = node_shape("Function", style);
    builder.push_function_node(&fn_id, path.name.as_str(), &fn_shape);

    // Emit param and returns edges for the function node.
    for param in &entry.params {
        collect_param_edge(builder, &fn_id, &param.ty, crate_name, type_index, style);
    }
    collect_returns_edge(builder, &fn_id, &entry.returns, crate_name, type_index, style);

    // Emit class attach for function node.
    let class_name = role_class_name(&entry.role.to_string(), style);
    let fn_node_class = node_class_name("Function", style);
    // Both role class and node class are attached (role for color, Function for shape).
    builder.push_class(&fn_id, &class_name);
    builder.push_class(&fn_id, &fn_node_class);
}

// ---------------------------------------------------------------------------
// Field edges (Decision K-2+(d) + Decision K-2 for TupleStruct)
// ---------------------------------------------------------------------------

/// Emits field edges from a TypeEntry subgraph to field type subgraphs.
///
/// - `PlainStruct { has_stripped_fields: false, fields }`: one `--o|field_name|` edge per field.
/// - `PlainStruct { has_stripped_fields: true }`: no edges emitted (AC-08).
/// - `TupleStruct { has_stripped_fields: false, fields }`: `--o|.0|`, `--o|.1|` etc.
/// - `TupleStruct { has_stripped_fields: true }`: no edges emitted (AC-08).
/// - `UnitStruct`, `Enum`, `TypeAlias`: no field edges (deferred to T007 for Enum/TypeAlias).
///
/// `caller_crate` scopes TypeRef resolution to the same catalogue document (same-catalogue).
fn emit_field_edges(
    builder: &mut MermaidBuilder,
    entry_id: &str,
    kind: &TypeKindV2,
    caller_crate: &CrateName,
    type_index: &TypeIndex,
    style: &StyleConfig,
) {
    let arrow = style.edge.get("field").map(|e| e.arrow.as_str()).unwrap_or("--o");

    match kind {
        TypeKindV2::PlainStruct { fields, has_stripped_fields: false, .. } => {
            for field in fields {
                if let Some(target_id) = type_index.resolve(&field.ty, caller_crate) {
                    builder.push_edge(format!(
                        "{entry_id} {arrow}|{}| {target_id}",
                        field.name.as_str()
                    ));
                }
            }
        }
        TypeKindV2::TupleStruct { fields, has_stripped_fields: false } => {
            for (i, field_ty) in fields.iter().enumerate() {
                if let Some(target_id) = type_index.resolve(field_ty, caller_crate) {
                    builder.push_edge(format!("{entry_id} {arrow}|.{i}| {target_id}"));
                }
            }
        }
        // has_stripped_fields: true → skip all field edges (AC-08).
        TypeKindV2::PlainStruct { has_stripped_fields: true, .. }
        | TypeKindV2::TupleStruct { has_stripped_fields: true, .. }
        // UnitStruct, Enum, TypeAlias: no field edges for T006.
        | TypeKindV2::UnitStruct
        | TypeKindV2::Enum { .. }
        | TypeKindV2::TypeAlias { .. } => {}
    }
}

// ---------------------------------------------------------------------------
// Method edge emission helpers
// ---------------------------------------------------------------------------

/// Emits a `--o` edge from `src_id` to the resolved param type subgraph.
///
/// The edge style is read from `style.edge["method_param"].arrow`. If the
/// TypeRef is unresolvable (not in the same catalogue), the edge is silently skipped.
/// `caller_crate` scopes TypeRef resolution to the same catalogue document.
fn collect_param_edge(
    builder: &mut MermaidBuilder,
    src_id: &str,
    param_ty: &TypeRef,
    caller_crate: &CrateName,
    type_index: &TypeIndex,
    style: &StyleConfig,
) {
    if let Some(target_id) = type_index.resolve(param_ty, caller_crate) {
        let arrow =
            style.edge.get("method_param").map(|e: &EdgeStyle| e.arrow.as_str()).unwrap_or("--o");
        builder.push_edge(format!("{src_id} {arrow} {target_id}"));
    }
}

/// Emits a `-->` edge from `src_id` to the resolved return type subgraph.
///
/// The edge style is read from `style.edge["method_returns"].arrow`. If the
/// TypeRef is unresolvable (not in the same catalogue), the edge is silently skipped.
/// `caller_crate` scopes TypeRef resolution to the same catalogue document.
fn collect_returns_edge(
    builder: &mut MermaidBuilder,
    src_id: &str,
    returns_ty: &TypeRef,
    caller_crate: &CrateName,
    type_index: &TypeIndex,
    style: &StyleConfig,
) {
    if let Some(target_id) = type_index.resolve(returns_ty, caller_crate) {
        let arrow =
            style.edge.get("method_returns").map(|e: &EdgeStyle| e.arrow.as_str()).unwrap_or("-->");
        builder.push_edge(format!("{src_id} {arrow} {target_id}"));
    }
}

// ---------------------------------------------------------------------------
// Label helpers
// ---------------------------------------------------------------------------

/// Builds the entry subgraph label from sub-module path and short name.
///
/// When `module_path` is root (empty), returns just `name`.
/// When `module_path` has segments, returns `seg1::seg2::...::name`
/// (sub-module path + name, per Decision U-6d-iii label spec).
///
/// Example: `module_path = ["team", "manager"]`, `name = "TeamManager"` →
/// `"team::manager::TeamManager"`.
fn entry_label(module_path: &ModulePath, name: &str) -> String {
    if module_path.is_root() { name.to_owned() } else { format!("{}::{name}", module_path) }
}

// ---------------------------------------------------------------------------
// Style helpers
// ---------------------------------------------------------------------------

/// Returns the classDef class name for a role string (e.g. `"ValueObject"`).
///
/// Looks up `style.role[role_str].class`. Falls back to the lowercase role
/// string if the role is not configured (should not happen with a well-formed
/// config file).
fn role_class_name(role_str: &str, style: &StyleConfig) -> String {
    style.role.get(role_str).map(|r| r.class.clone()).unwrap_or_else(|| role_str.to_lowercase())
}

/// Returns the classDef class name for a node category (e.g. `"Method"`, `"Function"`).
///
/// Looks up `style.node[category].class`. Falls back to the lowercase category
/// if not configured.
fn node_class_name(category: &str, style: &StyleConfig) -> String {
    style
        .node
        .get(category)
        .map(|n: &NodeStyle| n.class.clone())
        .unwrap_or_else(|| category.to_lowercase())
}

/// Returns the mermaid shape string for a node category (e.g. `"Method"`, `"Function"`).
///
/// Looks up `style.node[category].shape`. Falls back to `"round"` if the category
/// is not configured, so that unconfigured nodes still produce valid Mermaid output.
fn node_shape(category: &str, style: &StyleConfig) -> String {
    style
        .node
        .get(category)
        .map(|n: &NodeStyle| n.shape.clone())
        .unwrap_or_else(|| "round".to_owned())
}

/// Collects `classDef` lines from the style config in alphabetical order by class name.
///
/// Alphabetical ordering is used so that the renderer output is deterministic
/// across runs regardless of `HashMap` iteration order. T008 may enforce a
/// different canonical ordering; for T006 alphabetical is a stable baseline.
fn collect_classdefs(style: &StyleConfig) -> Vec<String> {
    let mut entries: Vec<(&String, _)> = style.class.iter().collect();
    entries.sort_by_key(|(name, _)| name.as_str());
    entries
        .into_iter()
        .map(|(name, cs)| {
            format!(
                "classDef {name} fill:{},stroke:{},stroke-width:{},stroke-dasharray:{}",
                cs.fill, cs.stroke, cs.stroke_width, cs.stroke_dasharray
            )
        })
        .collect()
}
