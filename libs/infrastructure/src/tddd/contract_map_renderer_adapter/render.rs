//! Mermaid rendering logic for `super::super::ContractMapRendererAdapter`.
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
//! Additional T007 scope (this module):
//! - Enum variant nodes and payload edges (Decision H-3, AC-04)
//! - TypeAlias undirected `---|alias_of|` edge (Decision N-1', AC-09)
//! - Typestate transition `==>|transitions_to|` returns edge (Decision G-2'b, AC-03)
//!
//! T008 additions (this module):
//! - Cross-catalogue trait_impl edges (Decision O-2 + O-3 + O-a)
//! - FunctionEntry role filter: empty `include_function_roles` = all; non-empty = subset
//!   (Decision I-1, IN-10)

mod builder;
mod enum_variants;
mod field_edges;
mod style_helpers;
mod trait_impl;
mod type_index;
mod typestate;

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use domain::tddd::catalogue_v2::composite::TypeKindV2;
use domain::tddd::catalogue_v2::entries::{FunctionEntry, TraitEntry, TypeEntry};
use domain::tddd::catalogue_v2::identifiers::{CrateName, ModulePath};
use domain::tddd::catalogue_v2::roles::ItemAction;
use domain::tddd::catalogue_v2::{CatalogueDocument, FunctionPath, TraitName, TypeName};
use domain::tddd::{ContractMapRenderOptions, ContractMapRendererError, LayerId};

use super::{StyleConfig, function_node_id, trait_node_id, type_node_id};
use builder::MermaidBuilder;
use enum_variants::emit_enum_variant_nodes;
use field_edges::{collect_param_edge, collect_returns_edge, emit_field_edges, entry_label};
use style_helpers::{collect_classdefs, node_class_name, node_shape, role_class_name};
use trait_impl::{TraitIndex, emit_trait_impl_edges};
use type_index::TypeIndex;
use typestate::{emit_methods_with_typestate, maybe_emit_typestate_overlay};

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
/// T008 additions:
/// - Cross-catalogue trait_impl edges (Decision O-2 + O-3 + O-a): a `TraitIndex`
///   is built once at render start from the **rendered** catalogues only (layers that
///   pass both `layer_order` and `opts.layers`), then used during TypeEntry rendering
///   to emit `-.->|impl|` edges to workspace-internal trait targets. Building from the
///   rendered subset prevents dangling Mermaid edges to traits whose layer was excluded
///   by the filter (Decision O-a, CN-08). Workspace-external traits (std, core, serde,
///   etc.) silently produce no edge.
/// - FunctionEntry role filter (Decision I-1, IN-10): when `style.filter.
///   include_function_roles` is non-empty, only functions whose `FunctionRole`
///   Display string appears in the list are emitted. An empty list means render all.
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
) -> Result<String, ContractMapRendererError> {
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

    // Collect only the catalogues that will actually be rendered (i.e., from
    // layers that pass both the layer_order filter and the opts.layers allowlist).
    // The TraitIndex and TypeIndex must be built from this filtered set so that
    // trait_impl edges never point to trait nodes that were not rendered — which
    // would leave dangling Mermaid edges (Decision O-a, CN-08).
    let rendered_layer_strs: BTreeSet<&str> = layer_order
        .iter()
        .map(|l| l.as_ref())
        .filter(|l| !has_filter || allowlist.contains(l))
        .collect();
    let rendered_catalogues: Vec<&CatalogueDocument> =
        catalogues.iter().filter(|doc| rendered_layer_strs.contains(doc.layer.as_ref())).collect();

    // Build the same-catalogue type index for TypeRef resolution.
    // Only includes catalogues from rendered layers (no dangling edges).
    let type_index = TypeIndex::build(&rendered_catalogues);

    // Build the cross-catalogue trait index for trait_impl edge resolution (T008).
    // Only includes catalogues from rendered layers so that trait_impl edges never
    // point to trait nodes absent from the rendered output (Decision O-a, CN-08).
    let trait_index = TraitIndex::build(&rendered_catalogues);

    // Collect classDef lines (alphabetical by class name — Decision U, CN-05).
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
            emit_layer(&mut builder, layer_id, docs, &type_index, &trait_index, style)?;
        }
    }

    Ok(builder.build(&classdefs))
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
///
/// Exposed as `pub(super)` so that node-id helpers in the parent module can use
/// the same injective encoding for layer components.
pub(super) fn escape_id_component(s: &str) -> String {
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
///
/// # Errors
///
/// Returns `ContractMapRendererError::RenderFailed` when any entry contains a `TypeRef`
/// with mismatched angle brackets (fail-closed, CN-03).
fn emit_layer(
    builder: &mut MermaidBuilder,
    layer_id: &LayerId,
    docs: &[&CatalogueDocument],
    type_index: &TypeIndex,
    trait_index: &TraitIndex,
    style: &StyleConfig,
) -> Result<(), ContractMapRendererError> {
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
        emit_layer_entry(builder, entry, layer_id, type_index, trait_index, style)?;
    }

    // Emit top-module subgraphs in alphabetical order (BTreeMap iter is sorted).
    for (top_seg, entries) in &top_module_map {
        let top_module_id = top_module_sg_id(layer_str, top_seg);
        let top_module_label = format!("{layer_str}::{top_seg}");

        builder.open_subgraph(&top_module_id, &top_module_label);

        for entry in entries {
            emit_layer_entry(builder, entry, layer_id, type_index, trait_index, style)?;
        }

        builder.close_subgraph();
    }

    builder.close_subgraph();
    Ok(())
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

impl LayerEntry<'_> {
    /// Returns `(primary, secondary)` sort keys for deterministic cross-kind ordering.
    ///
    /// Primary key: short name (type/trait name, or function `name` field).
    /// Secondary key: full disambiguator — for functions, the complete `FunctionPath`
    /// display string (`crate::module::name`) breaks ties when two functions share the
    /// same short name in different modules. For types/traits the name is unique within
    /// a document, so secondary equals primary.
    fn sort_keys(&self) -> (String, String) {
        match self {
            LayerEntry::Type { name, .. } => {
                let s = name.as_str().to_owned();
                (s.clone(), s)
            }
            LayerEntry::Trait { name, .. } => {
                let s = name.as_str().to_owned();
                (s.clone(), s)
            }
            LayerEntry::Function { path, .. } => (path.name.as_str().to_owned(), path.to_string()),
        }
    }
}

/// Partitions a document's entries into root entries and top-module buckets.
///
/// For `TypeEntry` and `TraitEntry`, `module_path` is stored on the entry itself.
/// For `FunctionEntry`, `module_path` is stored in the `FunctionPath` key (the
/// map key in `CatalogueDocument::functions`). The document's `crate_name` is
/// threaded into every `LayerEntry` for same-catalogue TypeRef resolution.
///
/// Entries from all three kinds (types, traits, functions) are collected together
/// and sorted cross-kind alphabetically by a (primary, secondary) key pair. This
/// avoids kind-batching (all types before all traits before all functions) and
/// produces a fully deterministic ordering that is independent of entry kind.
/// See `LayerEntry::sort_keys` for the key definition.
fn collect_layer_entries<'a>(
    doc: &'a CatalogueDocument,
    _layer_id: &LayerId,
    top_module_map: &mut BTreeMap<String, Vec<LayerEntry<'a>>>,
    root_entries: &mut Vec<LayerEntry<'a>>,
) {
    let crate_name = &doc.crate_name;

    // Collect all entries from the three BTreeMaps into a single Vec, then sort
    // cross-kind alphabetically to avoid kind-batching artefacts.
    let mut all_entries: Vec<LayerEntry<'a>> =
        Vec::with_capacity(doc.types.len() + doc.traits.len() + doc.functions.len());

    for (name, entry) in &doc.types {
        // Skip deletion records — contract map shows current surface, not historical records
        // (ADR `knowledge/adr/2026-04-11-0003-type-action-declarations.md`).
        if entry.action == ItemAction::Delete {
            continue;
        }
        let module_path = &entry.module_path;
        all_entries.push(LayerEntry::Type { name, entry, module_path, crate_name });
    }
    for (name, entry) in &doc.traits {
        if entry.action == ItemAction::Delete {
            continue;
        }
        let module_path = &entry.module_path;
        all_entries.push(LayerEntry::Trait { name, entry, module_path, crate_name });
    }
    for (path, entry) in &doc.functions {
        if entry.action == ItemAction::Delete {
            continue;
        }
        // FunctionEntry has no module_path field; use the FunctionPath key's module_path.
        let module_path = &path.module_path;
        all_entries.push(LayerEntry::Function { path, entry, module_path, crate_name });
    }

    // Sort cross-kind alphabetically. Primary key = short name, secondary key = full
    // disambiguator (full FunctionPath string for functions, same as primary for types/traits).
    // The secondary key ensures a deterministic total order when two functions share the
    // same short name but differ in module or crate.
    all_entries.sort_by_key(|e| e.sort_keys());

    for le in all_entries {
        let module_path = match &le {
            LayerEntry::Type { module_path, .. } => *module_path,
            LayerEntry::Trait { module_path, .. } => *module_path,
            LayerEntry::Function { module_path, .. } => *module_path,
        };
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
///
/// # Errors
///
/// Returns `ContractMapRendererError::RenderFailed` when a `TypeRef` in the entry
/// has mismatched angle brackets (fail-closed, CN-03).
fn emit_layer_entry(
    builder: &mut MermaidBuilder,
    entry: &LayerEntry<'_>,
    layer_id: &LayerId,
    type_index: &TypeIndex,
    trait_index: &TraitIndex,
    style: &StyleConfig,
) -> Result<(), ContractMapRendererError> {
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
                trait_index,
                style,
            )?;
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
            )?;
        }
        LayerEntry::Function { path, entry, module_path, crate_name } => {
            emit_function_node_filtered(
                builder,
                layer_id,
                crate_name,
                path,
                entry,
                module_path,
                type_index,
                style,
            )?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// TypeEntry subgraph rendering
// ---------------------------------------------------------------------------

/// Emits a TypeEntry as a mermaid subgraph with method nodes (and variant nodes
/// for Enum entries) inside.
///
/// Subgraph id: `T<len>_<sanitized_layer>_<sanitized_crate>_<sanitized_name>` (Decision D-2).
/// Label: sub-module path + name (e.g. `team::manager::TeamManager` when
/// `module_path = ["team", "manager"]` and `name = "TeamManager"`).
/// When module_path is root, label = `name`.
///
/// T007 additions:
/// - For `Enum` entries, variant nodes are placed inside the entry subgraph
///   (Decision H-3, AC-04).
/// - For `PlainStruct` entries with typestate, transition method returns edges
///   use `==>|transitions_to|` (Decision G-2'b, AC-03).
/// - Typestate overlay class is attached additively after the role class (T007).
///
/// T008 additions:
/// - After field edges, emit cross-catalogue trait_impl edges (Decision O-2 + O-3 + O-a).
///   Workspace-external traits silently produce no edge (Decision J-2 + CN-08).
///
/// # Errors
///
/// Returns `ContractMapRendererError::RenderFailed` when a `TypeRef` in the entry
/// has mismatched angle brackets (fail-closed, CN-03).
#[allow(clippy::too_many_arguments)]
fn emit_type_subgraph(
    builder: &mut MermaidBuilder,
    layer_id: &LayerId,
    crate_name: &CrateName,
    name: &TypeName,
    entry: &TypeEntry,
    module_path: &ModulePath,
    type_index: &TypeIndex,
    trait_index: &TraitIndex,
    style: &StyleConfig,
) -> Result<(), ContractMapRendererError> {
    let entry_id = type_node_id(layer_id, crate_name, name);
    let label = entry_label(module_path, name.as_str());

    builder.open_subgraph(&entry_id, &label);

    // TypeAlias entries are empty subgraphs (no method nodes, no variant nodes).
    // Only non-alias entries emit method and variant content (Decision N-1', AC-09).
    let is_type_alias = matches!(&entry.kind, TypeKindV2::TypeAlias { .. });

    // For PlainStruct with typestate, extract the marker for transition-aware edge style.
    // For TypeAlias, typestate_marker is always None (TypeAlias cannot carry typestate).
    let typestate_marker = if is_type_alias {
        None
    } else {
        match &entry.kind {
            TypeKindV2::PlainStruct { typestate, .. } => typestate.as_ref(),
            _ => None,
        }
    };

    if !is_type_alias {
        let method_shape = node_shape("Method", style);
        emit_methods_with_typestate(
            builder,
            &entry_id,
            &entry.methods,
            typestate_marker,
            crate_name,
            type_index,
            style,
            &method_shape,
        )?;

        // For Enum entries, emit variant nodes inside the entry subgraph (Decision H-3, AC-04).
        if let TypeKindV2::Enum { variants } = &entry.kind {
            emit_enum_variant_nodes(builder, &entry_id, variants, crate_name, type_index, style)?;
        }
    }

    builder.close_subgraph();

    // Emit field edges for PlainStruct / TupleStruct (Decision K-2+(d), K-2).
    // For TypeAlias, emit the undirected alias edge (Decision N-1', AC-09).
    emit_field_edges(builder, &entry_id, &entry.kind, crate_name, type_index, style)?;

    // T008: emit cross-catalogue trait_impl edges (Decision O-2 + O-3 + O-a).
    emit_trait_impl_edges(builder, &entry_id, entry, trait_index, style);

    // Emit class attach for entry subgraph.
    let class_name = role_class_name(&entry.role.to_string(), style);
    builder.push_class(&entry_id, &class_name);

    // Emit typestate overlay class (additive, T007 AC-03).
    // Skipped for TypeAlias since typestate_marker is always None for aliases.
    maybe_emit_typestate_overlay(builder, &entry_id, typestate_marker, style);

    // Emit class attach for each method node.
    // Skipped for TypeAlias (no method nodes emitted).
    if !is_type_alias {
        let method_class = node_class_name("Method", style);
        for (i, _method) in entry.methods.iter().enumerate() {
            let method_id = format!("{entry_id}_m_{i}");
            builder.push_class(&method_id, &method_class);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// TraitEntry subgraph rendering
// ---------------------------------------------------------------------------

/// Emits a TraitEntry as a mermaid subgraph with method nodes inside.
///
/// Subgraph id: `R<len>_<sanitized_layer>_<sanitized_crate>_<sanitized_name>` (Decision D-2).
/// Label: sub-module path + name.
///
/// # Errors
///
/// Returns `ContractMapRendererError::RenderFailed` when a method param or returns `TypeRef`
/// has mismatched angle brackets (fail-closed, CN-03).
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
) -> Result<(), ContractMapRendererError> {
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
            collect_param_edge(builder, &method_id, &param.ty, crate_name, type_index, style)?;
        }
        // Collect method returns edge.
        collect_returns_edge(builder, &method_id, &method.returns, crate_name, type_index, style)?;
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
    Ok(())
}

// ---------------------------------------------------------------------------
// FunctionEntry node rendering (with T008 role filter)
// ---------------------------------------------------------------------------

/// Emits a FunctionEntry as a standalone callable node (Decision F-2+d1),
/// applying the function role filter from the style config (T008, Decision I-1).
///
/// When `style.filter.include_function_roles` is non-empty, the function is
/// only emitted if its `FunctionRole` Display string appears in the list.
/// An empty list means "render all functions" (Decision I-1).
///
/// Node shape is driven by `[node.Function].shape` in the style config (e.g. `"subroutine"`
/// → `[[name]]`). Node id: `F<len>_<sanitized_layer>_<sanitized_full_path>` (Decision D-2).
/// `crate_name` scopes TypeRef resolution to the same catalogue document.
///
/// # Errors
///
/// Returns `ContractMapRendererError::RenderFailed` when a param or returns `TypeRef`
/// has mismatched angle brackets (fail-closed, CN-03).
#[allow(clippy::too_many_arguments)]
fn emit_function_node_filtered(
    builder: &mut MermaidBuilder,
    layer_id: &LayerId,
    crate_name: &CrateName,
    path: &FunctionPath,
    entry: &FunctionEntry,
    _module_path: &ModulePath,
    type_index: &TypeIndex,
    style: &StyleConfig,
) -> Result<(), ContractMapRendererError> {
    // T008 function role filter (Decision I-1, IN-10).
    // Empty include_function_roles → render all (no filter).
    if !style.filter.include_function_roles.is_empty() {
        let role_str = entry.role.to_string();
        if !style.filter.include_function_roles.iter().any(|r| r == &role_str) {
            return Ok(()); // Silently skip this function.
        }
    }

    let fn_id = function_node_id(layer_id, path);
    let fn_shape = node_shape("Function", style);
    builder.push_function_node(&fn_id, path.name.as_str(), &fn_shape);

    // Emit param and returns edges for the function node.
    for param in &entry.params {
        collect_param_edge(builder, &fn_id, &param.ty, crate_name, type_index, style)?;
    }
    collect_returns_edge(builder, &fn_id, &entry.returns, crate_name, type_index, style)?;

    // Emit class attach for function node.
    let class_name = role_class_name(&entry.role.to_string(), style);
    let fn_node_class = node_class_name("Function", style);
    // Both role class and node class are attached (role for color, Function for shape).
    builder.push_class(&fn_id, &class_name);
    builder.push_class(&fn_id, &fn_node_class);
    Ok(())
}
