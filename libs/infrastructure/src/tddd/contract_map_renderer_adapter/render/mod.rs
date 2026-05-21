//! Mermaid rendering internals for the contract-map renderer (T004–T009).
//!
//! All items in this module are `pub(super)` — they are implementation details
//! of `ContractMapRendererAdapter` and must not appear in the infrastructure
//! crate's public API (Decision P-3 / CN-11).

mod emit;

use std::collections::BTreeMap;

use serde::Deserialize;

use domain::tddd::catalogue_v2::CatalogueDocument;
use domain::tddd::{ContractMapRendererError, LayerId};

use emit::{EntryKind, emit_entry};

// ---------------------------------------------------------------------------
// Global node index for TypeRef resolution
// ---------------------------------------------------------------------------

/// Global node index for resolving `TypeRef` strings to rendered mermaid node IDs.
///
/// Built once per render call from all catalogue documents (Decision O-2/O-3
/// pattern, CN-05). Used to resolve field/param/return/variant TypeRef targets so
/// edges connect to the actual rendered subgraph nodes rather than auto-created
/// ghost nodes.
///
/// The index stores a single qualified map: `"crate_name::TypeName"` → `node_id`.
/// This supports two resolution modes:
/// - **Qualified lookup** (`"crate::Name"` in the TypeRef): exact map lookup.
/// - **Bare-name lookup** (no `::` in the TypeRef): self-crate scoped — resolves
///   `current_crate::name`. Bare names in the catalogue schema represent self-crate
///   types; no cross-crate fallback is performed (avoids silently wiring generic
///   params like `T` or `Self` to a coincidentally-named type in another crate).
pub(super) struct NodeIndex {
    /// `"crate_name::TypeName"` → `node_id`.
    qualified: BTreeMap<String, String>,
}

impl NodeIndex {
    fn new() -> Self {
        Self { qualified: BTreeMap::new() }
    }

    /// Insert a type entry into the index.
    fn insert(&mut self, crate_name: &str, bare_name: &str, node_id: String) {
        let qualified_key = format!("{crate_name}::{bare_name}");
        self.qualified.insert(qualified_key, node_id);
    }

    /// Look up a `TypeRef` string and return the matching node_id, if resolvable.
    ///
    /// `current_crate` is the crate name of the catalogue document that owns the
    /// entry being emitted. It is used to scope bare-name lookups: bare `TypeRef`
    /// strings denote self-crate types, so resolution is restricted to the
    /// current-crate's index entries.
    ///
    /// Resolution:
    /// 1. Strip generic suffix (`"Foo<T>"` → `"Foo"`). If stripping yields an empty
    ///    string (e.g. `"<T as Trait>::Assoc"`), skip index lookup — these complex
    ///    forms are never catalogue entries and would produce malformed ids.
    /// 2. Normalize Rust-keyword path prefixes (`crate::`, `self::`, `super::`) by
    ///    taking the last `::` segment. This handles catalogue TypeRefs written as
    ///    `"crate::Foo"` or `"crate::module::Foo"`, treating them as self-crate bare
    ///    names (`"Foo"`).
    /// 3. If the normalised ref has `::`, try qualified lookup (`"crate_name::Foo"`)
    ///    in `qualified`. Returns `None` if not found (workspace-external path).
    /// 4. For bare names, look up `current_crate::stripped` in `qualified`. Returns
    ///    `None` if not found — bare names in the catalogue schema represent self-crate
    ///    types; no cross-crate fallback is performed (avoids silently wiring generic
    ///    params like `T` or `Self` to a coincidentally-named type in another crate).
    fn resolve(&self, type_ref_str: &str, current_crate: &str) -> Option<&str> {
        let stripped = strip_generics(type_ref_str);
        // Guard: complex refs that strip to empty are not catalogue entries.
        if stripped.is_empty() {
            return None;
        }
        // Normalize Rust-keyword path prefixes (crate::, self::, super::) to bare name.
        // e.g. "crate::module::Foo" → "Foo", "self::Bar" → "Bar".
        let normalised = if stripped.starts_with("crate::")
            || stripped.starts_with("self::")
            || stripped.starts_with("super::")
        {
            stripped.rsplit("::").next().unwrap_or(stripped)
        } else {
            stripped
        };
        if normalised.is_empty() {
            return None;
        }
        if normalised.contains("::") {
            // Qualified path: try exact lookup first (e.g. "domain_core::UserId" — 2 segments).
            if let Some(node_id) = self.qualified.get(normalised) {
                return Some(node_id.as_str());
            }
            // Fallback for module-qualified paths (3+ segments, e.g. "domain::module::TypeName"):
            // extract crate (first segment) + type name (last segment) and try "crate::TypeName".
            // This covers TypeRefs written as fully module-qualified paths where the index key
            // stores only "crate::TypeName" (the catalogue key is bare name, not module-path).
            let mut segments = normalised.splitn(2, "::");
            if let (Some(crate_seg), Some(rest)) = (segments.next(), segments.next()) {
                let type_name = rest.rsplit("::").next().unwrap_or(rest);
                let fallback_key = format!("{crate_seg}::{type_name}");
                return self.qualified.get(fallback_key.as_str()).map(|s| s.as_str());
            }
            return None;
        }
        // Bare name: self-crate only (no cross-crate fallback).
        let current_crate_key = format!("{current_crate}::{normalised}");
        self.qualified.get(&current_crate_key).map(|s| s.as_str())
    }
}

// ---------------------------------------------------------------------------
// Private TOML schema DTOs (Decision P-3 / CN-11 / Decision L-1)
// ---------------------------------------------------------------------------

/// Top-level structure for `.harness/config/contract-map-style.toml`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StyleConfig {
    #[serde(default)]
    pub(super) role: BTreeMap<String, RoleStyle>,
    #[serde(default)]
    pub(super) node: BTreeMap<String, NodeStyle>,
    #[serde(default)]
    pub(super) pattern: BTreeMap<String, PatternStyle>,
    #[serde(default)]
    pub(super) class: BTreeMap<String, ClassStyle>,
    #[serde(default)]
    pub(super) edge: BTreeMap<String, EdgeStyle>,
    // [filter] is structurally read on deserialization but its fields are not yet
    // used for filtering logic (I-1 reserve: all FunctionEntries are rendered).
    #[allow(dead_code)]
    #[serde(default)]
    pub(super) filter: FilterConfig,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RoleStyle {
    pub(super) class: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NodeStyle {
    #[serde(default)]
    pub(super) shape: Option<String>,
    #[serde(default)]
    pub(super) class: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PatternStyle {
    pub(super) overlay_class: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ClassStyle {
    #[serde(default)]
    fill: Option<String>,
    #[serde(default)]
    stroke: Option<String>,
    #[serde(default)]
    stroke_width: Option<String>,
    #[serde(default)]
    stroke_dasharray: Option<String>,
}

/// `[edge.<EdgeKind>]` — arrow syntax and optional label for an edge kind.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EdgeStyle {
    pub(super) arrow: String,
    #[serde(default)]
    pub(super) label: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FilterConfig {
    // Future extension point for role-based function filtering (I-1 reserve).
    // Not used in current implementation — all FunctionEntries are rendered.
    #[allow(dead_code)]
    #[serde(default)]
    include_function_roles: Vec<String>,
}

// ---------------------------------------------------------------------------
// Rendering helpers
// ---------------------------------------------------------------------------

/// Sanitize a string for use as a mermaid node_id segment.
/// Replaces every character that is not ASCII alphanumeric or underscore with `_`.
pub(super) fn sanitize(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect()
}

/// Generate a node_id for a Type entry (Decision D-2).
///
/// Format: `T<len>_<sanitized_layer>_<sanitized_crate>_<sanitized_name>`
/// where `<len>` is the length of `<sanitized_layer>_<sanitized_crate>_<sanitized_name>`.
pub(super) fn type_node_id(layer: &str, crate_name: &str, type_name: &str) -> String {
    let sl = sanitize(layer);
    let sc = sanitize(crate_name);
    let sn = sanitize(type_name);
    let body = format!("{sl}_{sc}_{sn}");
    format!("T{}_{}", body.len(), body)
}

/// Generate a node_id for a Trait entry (Decision D-2).
///
/// Format: `R<len>_<sanitized_layer>_<sanitized_crate>_<sanitized_name>`
pub(super) fn trait_node_id(layer: &str, crate_name: &str, trait_name: &str) -> String {
    let sl = sanitize(layer);
    let sc = sanitize(crate_name);
    let sn = sanitize(trait_name);
    let body = format!("{sl}_{sc}_{sn}");
    format!("R{}_{}", body.len(), body)
}

/// Generate a node_id for a Function entry (Decision D-2).
///
/// Format: `F<len>_<sanitized_layer>_<sanitized_crate>_<sanitized_full_path>`
pub(super) fn function_node_id(layer: &str, crate_name: &str, full_path: &str) -> String {
    let sl = sanitize(layer);
    let sc = sanitize(crate_name);
    let sp = sanitize(full_path);
    let body = format!("{sl}_{sc}_{sp}");
    format!("F{}_{}", body.len(), body)
}

/// Generate a subgraph id for a module (top-level module aggregation, U-6d-iii).
fn module_subgraph_id(layer: &str, crate_name: &str, module_first_segment: &str) -> String {
    let sl = sanitize(layer);
    let sc = sanitize(crate_name);
    let sm = sanitize(module_first_segment);
    format!("{sl}_{sc}_module_{sm}")
}

/// Generate a subgraph id for a layer.
fn layer_subgraph_id(layer: &str) -> String {
    sanitize(layer)
}

/// Format a mermaid `classDef` line from a `ClassStyle`.
fn class_def_line(name: &str, style: &ClassStyle) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(ref fill) = style.fill {
        parts.push(format!("fill:{fill}"));
    }
    if let Some(ref stroke) = style.stroke {
        parts.push(format!("stroke:{stroke}"));
    }
    if let Some(ref sw) = style.stroke_width {
        parts.push(format!("stroke-width:{sw}"));
    }
    if let Some(ref sd) = style.stroke_dasharray {
        parts.push(format!("stroke-dasharray:{sd}"));
    }
    if parts.is_empty() {
        format!("classDef {name}")
    } else {
        format!("classDef {name} {}", parts.join(","))
    }
}

/// Apply a node shape from a `NodeStyle` to a node label.
pub(super) fn apply_shape(label: &str, shape: Option<&str>) -> String {
    match shape {
        Some(s) => s.replace("{label}", label),
        None => format!("[{label}]"),
    }
}

/// Resolve an `EdgeStyle` to `(arrow, label_option)`.
///
/// Returns `Ok((arrow, label))` when the edge key is present in the style map.
/// Returns `Err(ContractMapRendererError::RenderFailed)` when the key is absent —
/// fail-closed per CN-02 (no code-internal hard-coded fallback or code default).
///
/// # Errors
///
/// Returns `ContractMapRendererError::RenderFailed` when `key` is not found in
/// `style_map`. The style config is required to define all edge kinds that the
/// renderer uses (CN-02 / AC-11 — no hard-coded styling in code).
pub(super) fn edge_arrow_label<'a>(
    style_map: &'a BTreeMap<String, EdgeStyle>,
    key: &str,
) -> Result<(&'a str, Option<&'a str>), ContractMapRendererError> {
    match style_map.get(key) {
        Some(es) => Ok((es.arrow.as_str(), es.label.as_deref())),
        None => Err(ContractMapRendererError::RenderFailed {
            reason: format!(
                "missing edge style configuration: [edge.{key}] not found in style config (CN-02)"
            ),
        }),
    }
}

/// Format an edge line: `source arrow[|label|] target`.
pub(super) fn edge_line(source: &str, arrow: &str, label: Option<&str>, target: &str) -> String {
    match label {
        Some(l) => format!("{source} {arrow}|{l}| {target}"),
        None => format!("{source} {arrow} {target}"),
    }
}

// ---------------------------------------------------------------------------
// T004: global trait index
// ---------------------------------------------------------------------------

/// Build a global trait index from all catalogues (Decision O-2/O-3).
///
/// Returns `BTreeMap<(crate_name_str, trait_name_str), subgraph_id_str>`.
pub(super) fn build_trait_index(
    catalogues: &[CatalogueDocument],
) -> BTreeMap<(String, String), String> {
    let mut index: BTreeMap<(String, String), String> = BTreeMap::new();
    for doc in catalogues {
        let layer = doc.layer.as_ref();
        let crate_name = doc.crate_name.as_str();
        for trait_name in doc.traits.keys() {
            let subgraph_id = trait_node_id(layer, crate_name, trait_name.as_str());
            index.insert((crate_name.to_string(), trait_name.as_str().to_string()), subgraph_id);
        }
    }
    index
}

/// Build a global node index from all catalogues for TypeRef resolution.
///
/// Populates `NodeIndex` covering **`TypeEntry` only** (not `TraitEntry`), keyed
/// both by qualified `"crate_name::Name"` and by bare `"Name"`. This index is
/// used to resolve field/param/return/variant TypeRef targets to their actual
/// rendered type subgraph node IDs (Decision D-2).
///
/// `TraitEntry` names are deliberately excluded: trait_impl target resolution uses
/// a separate `build_trait_index` + `resolve_trait_subgraph` path. Mixing type and
/// trait names in the same index would cause a TypeRef that matches only a trait to
/// incorrectly link to a trait subgraph, and a name shared by a type and a trait to
/// become ambiguous and fall back to a ghost node.
pub(super) fn build_node_index(catalogues: &[CatalogueDocument]) -> NodeIndex {
    let mut index = NodeIndex::new();
    for doc in catalogues {
        let layer = doc.layer.as_ref();
        let crate_name = doc.crate_name.as_str();
        for type_name in doc.types.keys() {
            let node_id = type_node_id(layer, crate_name, type_name.as_str());
            index.insert(crate_name, type_name.as_str(), node_id);
        }
    }
    index
}

/// Strip generic arguments from a type/trait name string.
///
/// `"SomeTrait<Foo, Bar>"` → `"SomeTrait"`.
/// `"MyType"` → `"MyType"` (unchanged).
fn strip_generics(name: &str) -> &str {
    name.split_once('<').map_or(name, |(head, _)| head)
}

/// Resolve a `TypeRef` string to a rendered mermaid node ID.
///
/// Delegates to `NodeIndex::resolve`; falls back to `sanitize(type_ref_str)` when
/// no unambiguous match exists (external type, unknown type, or bare-name collision
/// across multiple catalogues).
///
/// `current_crate` is forwarded to `NodeIndex::resolve` as a tie-breaker for bare
/// TypeRef names that appear in multiple crates.
pub(super) fn resolve_type_ref_node_id(
    type_ref_str: &str,
    node_index: &NodeIndex,
    current_crate: &str,
) -> String {
    match node_index.resolve(type_ref_str, current_crate) {
        Some(node_id) => node_id.to_string(),
        None => sanitize(type_ref_str),
    }
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
fn resolve_trait_subgraph<'a>(
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

// ---------------------------------------------------------------------------
// T009: main assembly
// ---------------------------------------------------------------------------

/// Render a mermaid flowchart from a set of catalogue documents.
///
/// # Errors
///
/// Propagates any `ContractMapRendererError` that arises during rendering
/// (none in the current implementation, but the signature is kept for future extension).
pub(super) fn render_mermaid(
    catalogues: &[CatalogueDocument],
    layer_order: &[LayerId],
    style: &StyleConfig,
) -> Result<String, ContractMapRendererError> {
    // T004: build global trait index (per-render-call, CN-05).
    let trait_index = build_trait_index(catalogues);
    // T004: build global node index for TypeRef resolution (field/param/return edges).
    let node_index = build_node_index(catalogues);

    // Collect: for each layer subgraph, collect all catalogue documents belonging to it.
    // Index documents by layer id string for quick lookup.
    let mut docs_by_layer: BTreeMap<String, Vec<&CatalogueDocument>> = BTreeMap::new();
    for doc in catalogues {
        docs_by_layer.entry(doc.layer.as_ref().to_string()).or_default().push(doc);
    }

    // Output sections.
    let mut class_defs: Vec<String> = Vec::new();
    let mut subgraph_lines: Vec<String> = Vec::new();
    let mut edge_lines: Vec<String> = Vec::new();
    let mut class_attach: Vec<String> = Vec::new();

    // T009(b): classDef definitions — alphabetical from [class.*] (CN-08).
    for (class_name, class_style) in &style.class {
        class_defs.push(class_def_line(class_name, class_style));
    }

    // T009(c): layer subgraphs in layer_order (CN-01/GO-03).
    for layer_id in layer_order {
        let layer_str = layer_id.as_ref();
        let layer_sg_id = layer_subgraph_id(layer_str);

        subgraph_lines.push(format!("subgraph {layer_sg_id}[\"{layer_str}\"]"));

        // Sort docs within layer alphabetically by crate_name (CN-08).
        let docs_in_layer = docs_by_layer.get(layer_str).cloned().unwrap_or_default();
        let mut sorted_docs: Vec<&CatalogueDocument> = docs_in_layer;
        sorted_docs.sort_by_key(|d| d.crate_name.as_str());

        for doc in &sorted_docs {
            let crate_str = doc.crate_name.as_str();
            let layer_str_doc = doc.layer.as_ref();

            // Build inherent_impls index for this doc: type_name -> Vec<methods>
            let mut inherent_methods: BTreeMap<
                String,
                Vec<&domain::tddd::catalogue_v2::methods::MethodDeclaration>,
            > = BTreeMap::new();
            for impl_decl in &doc.inherent_impls {
                let tn = impl_decl.type_name.as_str().to_string();
                for m in &impl_decl.methods {
                    inherent_methods.entry(tn.clone()).or_default().push(m);
                }
            }

            // Separate entries into root (module_path=[]) and module-grouped.
            let mut module_first_segs: BTreeMap<String, Vec<EntryKind<'_>>> = BTreeMap::new();
            let mut root_entries: Vec<EntryKind<'_>> = Vec::new();

            // Types
            for (type_name, type_entry) in &doc.types {
                if type_entry.module_path.is_root() {
                    root_entries.push(EntryKind::Type(type_name.as_str(), type_entry));
                } else {
                    let first_seg = type_entry
                        .module_path
                        .segments()
                        .first()
                        .map(|s| s.as_str())
                        .unwrap_or("")
                        .to_string();
                    module_first_segs
                        .entry(first_seg)
                        .or_default()
                        .push(EntryKind::Type(type_name.as_str(), type_entry));
                }
            }

            // Traits
            for (trait_name, trait_entry) in &doc.traits {
                if trait_entry.module_path.is_root() {
                    root_entries.push(EntryKind::Trait(trait_name.as_str(), trait_entry));
                } else {
                    let first_seg = trait_entry
                        .module_path
                        .segments()
                        .first()
                        .map(|s| s.as_str())
                        .unwrap_or("")
                        .to_string();
                    module_first_segs
                        .entry(first_seg)
                        .or_default()
                        .push(EntryKind::Trait(trait_name.as_str(), trait_entry));
                }
            }

            // Functions
            for (fn_path, fn_entry) in &doc.functions {
                if fn_path.module_path.is_root() {
                    root_entries.push(EntryKind::Function(fn_path, fn_entry));
                } else {
                    let first_seg = fn_path
                        .module_path
                        .segments()
                        .first()
                        .map(|s| s.as_str())
                        .unwrap_or("")
                        .to_string();
                    module_first_segs
                        .entry(first_seg)
                        .or_default()
                        .push(EntryKind::Function(fn_path, fn_entry));
                }
            }

            // Emit root entries directly under the layer subgraph.
            for entry in &root_entries {
                emit_entry(
                    entry,
                    &mut subgraph_lines,
                    &mut edge_lines,
                    &mut class_attach,
                    style,
                    &inherent_methods,
                    &node_index,
                    layer_str_doc,
                    crate_str,
                )?;
            }

            // Emit module subgraphs.
            for (first_seg, entries) in &module_first_segs {
                let mod_sg_id = module_subgraph_id(layer_str_doc, crate_str, first_seg);
                let mod_label = format!("{crate_str}::{first_seg}");
                subgraph_lines.push(format!("  subgraph {mod_sg_id}[\"{mod_label}\"]"));

                for entry in entries {
                    emit_entry(
                        entry,
                        &mut subgraph_lines,
                        &mut edge_lines,
                        &mut class_attach,
                        style,
                        &inherent_methods,
                        &node_index,
                        layer_str_doc,
                        crate_str,
                    )?;
                }

                subgraph_lines.push("  end".to_string());
            }
        }

        subgraph_lines.push("end".to_string());

        // T008: trait impl edges for this layer's docs.
        for doc in &sorted_docs {
            let crate_str = doc.crate_name.as_str();
            for trait_impl in &doc.trait_impls {
                let for_type_str = trait_impl.for_type.as_str();
                let trait_ref_str = trait_impl.trait_ref.as_str();

                // Resolve for_type to a node_id via the global node index.
                // Workspace-internal cross-crate for_type (e.g. "domain::MyType") is
                // resolved through the index. Workspace-external types (std, external
                // crates) are not in the index and are silently skipped (O-2 / ADR line 286).
                let source_id = match node_index.resolve(for_type_str, crate_str) {
                    Some(id) => id.to_string(),
                    None => continue, // silent skip (workspace-external, OS-04)
                };

                // Resolve trait_ref to target subgraph_id (CN-10: silent skip if external).
                let target_id = match resolve_trait_subgraph(trait_ref_str, crate_str, &trait_index)
                {
                    Some(id) => id.to_string(),
                    None => continue, // silent skip (CN-10 / AC-06)
                };

                let (arrow, label) = edge_arrow_label(&style.edge, "trait_impl")?;
                edge_lines.push(edge_line(&source_id, arrow, label, &target_id));
            }
        }
    }

    // Assemble output per IN-18 / ADR Render Output structure.
    let mut out = String::new();
    out.push_str("flowchart LR\n");

    for line in &class_defs {
        out.push_str(line);
        out.push('\n');
    }

    for line in &subgraph_lines {
        out.push_str(line);
        out.push('\n');
    }

    for line in &edge_lines {
        out.push_str(line);
        out.push('\n');
    }

    for line in &class_attach {
        out.push_str(line);
        out.push('\n');
    }

    Ok(out)
}
