//! Mermaid rendering internals for the baseline-graph renderer (T004–T010).
//!
//! All items in this module are `pub(super)` — they are implementation details
//! of `BaselineGraphRendererAdapter` and must not appear in the infrastructure
//! crate's public API (Decision P-3 / CN-11, symmetric to ContractMapRendererAdapter).
//!
//! **Scope (T004)**: private TOML schema DTOs + style config loading + skeleton
//! mermaid output (classDef block + layer subgraph frame). Full node/edge rendering
//! will be added in T005–T010.
//!
//! **Scope (T005)**: node extraction logic — [`node_extractor`] submodule provides
//! [`node_extractor::extract_nodes`] which extracts the 5 B-r1 node kinds (Decision B-r1)
//! from `rustdoc_types::Crate` index entries, applying the visibility filter (Decision CC-1)
//! and the standalone-Function listing range (Decision I).
//!
//! **Scope (T006)**: node_id generation — [`node_id_generator`] submodule implements
//! Decision D (T/R/F prefix + length-prefix + sanitized_module_path) to avoid collisions
//! between same-named types in different modules (IN-05 / AC-11).
//!
//! **Scope (T007)**: entry subgraph + variant/method/field/alias edge emission —
//! [`entry_subgraph`] submodule implements F-r1 / H / H' / K / N decisions:
//! Struct/Enum/Trait/TypeAlias as subgraphs, enum variant nodes + payload edges,
//! Trait method inclusion, struct field edges, and TypeAlias alias edges.
//!
//! **Scope (T008)**: cross-baseline trait index + Impl Item processing —
//! [`impl_processor`] submodule implements O-r1 (global trait index), BB-4-fix1
//! (inherent merge / trait impl edge / blanket body / skip rules), and J decision
//! (`-.impl.->` edge style). (IN-09 / IN-12 / IN-13 / AC-05 / AC-08 / AC-17 /
//! CN-04 / CN-05 / CN-10 / CN-11)
//!
//! **Scope (T009)**: depth-1 overview renderer — cluster (crate_name × top-level module)
//! is collapsed to 1 node, cross-cluster edges are collected and emitted, alphabetical
//! ordering is enforced (CN-08). Implements IN-14 / IN-16 / AC-13 / CN-07 / CN-08.

pub(super) mod entry_subgraph;
pub(super) mod impl_processor;
pub(super) mod node_extractor;
pub(super) mod node_id_generator;

use std::collections::BTreeMap;

use serde::Deserialize;

use domain::tddd::baseline_document::BaselineDocument;
use domain::tddd::baseline_graph_ports::BaselineGraphRendererError;
use domain::tddd::layer_id::LayerId;

// ---------------------------------------------------------------------------
// Private TOML schema DTOs (symmetric to ContractMapRendererAdapter render::StyleConfig)
//
// Section structure: [node.*] + [pattern.*] + [class.*] + [edge.*] + [filter].
// [role.*] is intentionally ABSENT — Reality View input is rustdoc_types::Crate
// which carries no catalogue role data (ADR 2026-05-22-1507 Decision C / IN-04).
// ---------------------------------------------------------------------------

/// Top-level structure for `.harness/config/baseline-graph-style.toml`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StyleConfig {
    /// `[node.<NodeCategory>]` — shape + class for a node category (Method, Variant, Function).
    /// Used in T005-T010 for node shape/class rendering.
    #[allow(dead_code)]
    #[serde(default)]
    pub(super) node: BTreeMap<String, NodeStyle>,
    /// `[pattern.<PatternName>]` — overlay_class for structural patterns (future extension).
    #[allow(dead_code)]
    #[serde(default)]
    pub(super) pattern: BTreeMap<String, PatternStyle>,
    /// `[class.<ClassName>]` — mermaid classDef parameters.
    #[serde(default)]
    pub(super) class: BTreeMap<String, ClassStyle>,
    /// `[edge.<EdgeKind>]` — arrow syntax + optional label. Used in T005-T010 for edge rendering.
    #[allow(dead_code)]
    #[serde(default)]
    pub(super) edge: BTreeMap<String, EdgeStyle>,
    /// `[filter]` — future extension point (I decision reserve).
    #[allow(dead_code)]
    #[serde(default)]
    pub(super) filter: FilterConfig,
}

/// `[node.<NodeCategory>]` — shape template + class name for a node category.
///
/// Used in T005-T010 for applying node shapes and class names to rendered nodes.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NodeStyle {
    /// Optional mermaid shape template (e.g. `"([{label}])"` for stadium shape).
    /// When absent the default mermaid rectangular shape is used.
    #[serde(default)]
    pub(super) shape: Option<String>,
    /// Optional classDef name to apply to nodes of this category.
    #[serde(default)]
    pub(super) class: Option<String>,
}

/// `[pattern.<PatternName>]` — overlay class for a structural pattern (future).
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PatternStyle {
    /// classDef name used as an overlay for this pattern.
    pub(super) overlay_class: String,
}

/// `[class.<ClassName>]` — mermaid `classDef` CSS-like parameters.
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
///
/// Used in T005-T010 for edge rendering (trait impl, variant payload, field, alias, etc.).
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EdgeStyle {
    pub(super) arrow: String,
    #[serde(default)]
    pub(super) label: Option<String>,
}

/// `[filter]` — future rendering filter configuration (I decision reserve).
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FilterConfig {
    /// Whether to include all public functions (default: true, all rendered).
    #[allow(dead_code)]
    #[serde(default = "default_include_functions")]
    include_functions: bool,
}

fn default_include_functions() -> bool {
    true
}

// ---------------------------------------------------------------------------
// Rendering helpers (symmetric to ContractMapRendererAdapter render helpers)
// ---------------------------------------------------------------------------

/// Sanitize a string for use as a mermaid node_id segment.
///
/// Replaces every character that is not ASCII alphanumeric or underscore with `_`.
pub(super) fn sanitize(s: &str) -> String {
    s.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect()
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

/// Apply a node shape template from a `NodeStyle` to a node label.
///
/// Used in T005-T010 for rendering nodes with configurable shapes.
#[allow(dead_code)]
pub(super) fn apply_shape(label: &str, shape: Option<&str>) -> String {
    match shape {
        Some(s) => s.replace("{label}", label),
        None => format!("[{label}]"),
    }
}

/// Resolve an `EdgeStyle` to `(arrow, label_option)`.
///
/// Returns `Ok((arrow, label))` when the edge key is present in the style map.
/// Returns `Err(BaselineGraphRendererError::RenderFailed)` when the key is absent —
/// fail-closed per CN-02 (no code-internal hard-coded fallback).
///
/// Used in T005-T010 for fail-closed edge style resolution.
///
/// # Errors
///
/// Returns `BaselineGraphRendererError::RenderFailed` when `key` is not found in
/// `style_map`. The style config is required to define all edge kinds that the
/// renderer uses (CN-02 — no hard-coded styling in code).
#[allow(dead_code)]
pub(super) fn edge_arrow_label<'a>(
    style_map: &'a BTreeMap<String, EdgeStyle>,
    key: &str,
) -> Result<(&'a str, Option<&'a str>), BaselineGraphRendererError> {
    match style_map.get(key) {
        Some(es) => Ok((es.arrow.as_str(), es.label.as_deref())),
        None => Err(BaselineGraphRendererError::RenderFailed {
            reason: format!(
                "missing edge style configuration: [edge.{key}] not found in baseline-graph style config (CN-02)"
            ),
        }),
    }
}

/// Generate a subgraph id for a layer.
fn layer_subgraph_id(layer: &str) -> String {
    sanitize(layer)
}

// ---------------------------------------------------------------------------
// T009: depth-1 overview renderer
//
// Cluster = (crate_name, top-level module) collapsed to 1 node.
// Cross-cluster edges collected from the full entry render and emitted in section 3.
// Alphabetical ordering enforced via BTreeMap (CN-08).
//
// Mermaid output structure (IN-16, ADR U-r3):
//   1. classDef definitions (alphabetical)
//   2. layer subgraph > cluster node group (alphabetical by cluster key)
//   3. cross-cluster edge group
//   4. class attach group (`class <id> <className>` — never inline :::className)
// ---------------------------------------------------------------------------

/// Cluster key for grouping entries: `(crate_name, module_seg1_or_root)`.
///
/// - `crate_name`: raw crate name from `BaselineDocument`.
/// - `module_seg1`: first module segment from `ItemSummary.path` (index 1),
///   or the literal string `"root"` for crate-root entries (path length ≤ 2).
///
/// (ADR U-r3, IN-14 / CN-07)
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct ClusterKey {
    pub(super) crate_name: String,
    pub(super) module_seg1: String,
}

impl ClusterKey {
    /// Derive the mermaid node id for this cluster node.
    ///
    /// Format: `<sanitized_crate_name>_<sanitized_module_seg1>`
    /// e.g. `domain_review`, `domain_root`
    ///
    /// ADR U-r3 (ライン337) defines the cluster key format as `<crate_name>_<module_seg1>`.
    /// This matches the depth-1 overview examples in the ADR (ライン352: `domain_review[domain::review]`).
    ///
    /// **Known limitation (ADR ライン199)**: sanitize replaces non-alphanumeric chars with `_`,
    /// so a crate name ending with `_` and a module name starting with `_` would produce the
    /// same node id as a different pair. The ADR explicitly accepts this for the real-world
    /// architecture (crate names like `domain` / `usecase` / `infrastructure` do not end with `_`,
    /// and module names do not start with `_` in practice).
    pub(super) fn node_id(&self) -> String {
        format!("{}_{}", sanitize(&self.crate_name), sanitize(&self.module_seg1))
    }

    /// Derive the human-readable label for this cluster node.
    ///
    /// - For non-root clusters: `"<crate_name>::<module_seg1>"`
    /// - For root clusters:     `"<crate_name> root"`
    pub(super) fn label(&self) -> String {
        if self.module_seg1 == "root" {
            format!("{} root", self.crate_name)
        } else {
            format!("{}::{}", self.crate_name, self.module_seg1)
        }
    }
}

/// Derive the [`ClusterKey`] for a node given its `ItemSummary.path`.
///
/// - path = `[crate_name, module_seg1, ..., item_name]` (len ≥ 3): cluster is `(path[0], path[1])`.
/// - path = `[crate_name, item_name]` (len == 2) or shorter: cluster is `(crate_name, "root")`.
///
/// Returns `None` only when `path` is empty (degenerate case).
///
/// ADR U-r3 (ライン338): crate root entry は cluster key `(crate_name, "root")` として
/// 集約することが明示されている。`"root"` は ADR の設計上のセンチネル値であり、実際の
/// アーキテクチャで top-level module が `root` と命名されるケースは想定外 (既知の制限)。
///
/// No panics: uses `.get()` for all slice indexing.
fn cluster_key_from_path(path: &[String]) -> Option<ClusterKey> {
    let crate_name = path.first()?.clone();
    let module_seg1 = if path.len() >= 3 {
        // path[1] is the first module segment (between crate_name and item_name).
        path.get(1)?.clone()
    } else {
        // len == 2: [crate_name, item_name] — crate root (ADR U-r3 ライン338).
        // `"root"` is the ADR-mandated sentinel; a Rust top-level module named `root`
        // would collide, but ADR U-r3 accepts this as a known architectural constraint
        // (real-world architectures do not name top-level modules `root`).
        "root".to_string()
    };
    Some(ClusterKey { crate_name, module_seg1 })
}

/// Build the node-id → cluster-key map for all public entries in `baselines` for `layer`.
///
/// For each B-r1 node extracted from `baselines`, determines the entry's cluster key
/// using its `ItemSummary.path` (first module segment or "root"), then records the
/// mapping from the entry's **representative node id** to the cluster key.
///
/// The representative node id is used because edges in mermaid edge lines reference
/// rep-node ids (e.g. `T25_domain_my_crate__MyStruct__self`).
///
/// Also records all method node ids (format: `{entry_sg_id}_{method_name}`) and
/// subgraph ids so that any node ID that is a child of an entry can be traced back
/// to the entry's cluster.
///
/// Returns a map: `node_id_prefix → ClusterKey` where `node_id_prefix` is the entry
/// subgraph id (without `__self` suffix).  Callers use prefix matching because:
/// - rep-node ids = `{subgraph_id}__self`
/// - method node ids = `{subgraph_id}_{method_name}`
/// - subgraph id itself (edge target for N alias, best-effort) = `{subgraph_id}`
///
/// No panics: all indexing via iterators and `.get()`.
fn build_node_cluster_map(
    baselines: &[BaselineDocument],
    layer: &str,
) -> BTreeMap<String, ClusterKey> {
    use node_extractor::{ExtractedNode, extract_nodes};
    use node_id_generator::{
        function_node_id, module_path_from_summary, trait_node_id, type_node_id,
    };

    let layer_id = match domain::tddd::layer_id::LayerId::try_new(layer) {
        Ok(l) => l,
        Err(_) => return BTreeMap::new(),
    };

    let nodes = extract_nodes(baselines, &layer_id);
    let mut map: BTreeMap<String, ClusterKey> = BTreeMap::new();

    for node in &nodes {
        let doc = node.doc();
        let item = node.item();
        let id = node.id();
        let krate = &doc.krate;
        let crate_name = doc.crate_name.as_str();
        let layer_str = doc.layer.as_ref();

        let path_opt = krate.paths.get(&id).map(|s| s.path.as_slice());
        let cluster_key = match path_opt.and_then(cluster_key_from_path) {
            Some(k) => k,
            None => continue,
        };

        // Compute the entry subgraph id (prefix used for all child node ids).
        let entry_sg_id: Option<String> = match node {
            ExtractedNode::Struct { .. }
            | ExtractedNode::Enum { .. }
            | ExtractedNode::TypeAlias { .. } => {
                let name = item.name.as_deref().unwrap_or("");
                let mp = path_opt.map(module_path_from_summary).unwrap_or_default();
                Some(type_node_id(layer_str, crate_name, &mp, name))
            }
            ExtractedNode::Trait { .. } => {
                let name = item.name.as_deref().unwrap_or("");
                let mp = path_opt.map(module_path_from_summary).unwrap_or_default();
                Some(trait_node_id(layer_str, crate_name, &mp, name))
            }
            ExtractedNode::Function { .. } => {
                // Function standalone node — full path for id.
                let full_path = path_opt
                    .map(|p| p.join("::"))
                    .unwrap_or_else(|| item.name.as_deref().unwrap_or("").to_string());
                Some(function_node_id(layer_str, crate_name, &full_path))
            }
        };

        if let Some(sg_id) = entry_sg_id {
            map.insert(sg_id, cluster_key);
        }
    }

    map
}

/// Look up the cluster key for a mermaid node id given the `node_cluster_map`.
///
/// The map keys are entry subgraph ids (e.g. `T25_domain_my_crate__MyStruct`).
/// Node ids in edges may be:
/// - rep-node ids: `{subgraph_id}__self`
/// - method node ids: `{subgraph_id}_{method_name}`
/// - the subgraph id itself
/// - anonymous/primitive ids (e.g. `anon_*`, `prim_*`, `generic_*`)
///
/// Strategy: look for the longest key in the map that is a proper prefix of `node_id`
/// (i.e. map_key == node_id OR node_id starts with `{map_key}_`).
/// This correctly handles the case where one subgraph id is a prefix of another (e.g.
/// `T10_domain_foo` vs `T10_domain_foobar`).
///
/// Note: map keys all have `T<len>_` / `R<len>_` / `F<len>_` prefixes (node_id_generator
/// Decision D). Anonymous / primitive / generic node ids (`anon_*`, `prim_*`, `generic_*`)
/// cannot prefix-match any map key, so they correctly return `None` — callers skip them.
fn lookup_cluster<'a>(
    node_id: &str,
    node_cluster_map: &'a BTreeMap<String, ClusterKey>,
) -> Option<&'a ClusterKey> {
    // The entry subgraph id is the longest key in the map that is a prefix of node_id.
    // We iterate in reverse alphabetical order (BTreeMap is ascending; we want longest match).
    let mut best: Option<(&str, &ClusterKey)> = None;
    for (key, cluster) in node_cluster_map {
        if node_id == key || node_id.starts_with(&format!("{key}_")) {
            // Valid prefix: check if longer than current best.
            match best {
                None => {
                    best = Some((key.as_str(), cluster));
                }
                Some((prev_key, _)) if key.len() > prev_key.len() => {
                    best = Some((key.as_str(), cluster));
                }
                _ => {}
            }
        }
    }
    best.map(|(_, c)| c)
}

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
fn collect_entry_edge_pairs(baselines: &[BaselineDocument], layer: &str) -> Vec<(String, String)> {
    use node_extractor::{ExtractedNode, extract_nodes};
    use node_id_generator::{module_path_from_summary, trait_rep_node_id, type_rep_node_id};
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
    let trait_index = impl_processor::build_trait_index(baselines, layer);

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
                    let key = impl_processor::TraitKey {
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
            let src_rep_id = node_id_generator::trait_rep_node_id(
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
fn collect_resolved_node_ids_from_type(
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
    use node_id_generator::{module_path_from_summary, type_rep_node_id};
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

/// Render a depth-1 overview mermaid diagram (T009).
///
/// Implements IN-14 / IN-16 / AC-13 / CN-07 / CN-08 / U-r3:
/// - clusters (= `(crate_name, top-level module)`) are collapsed to 1 node each.
/// - Only cross-cluster edges are emitted (intra-cluster edges belong to depth-2).
/// - Alphabetical ordering via `BTreeMap` (CN-08).
///
/// Output structure (IN-16):
/// 1. `classDef` definitions (alphabetical by class name).
/// 2. layer `subgraph` containing cluster nodes (alphabetical by cluster key).
/// 3. Cross-cluster edge group.
/// 4. `class <id> <className>` attach group.
///
/// # Errors
///
/// Returns [`BaselineGraphRendererError::RenderFailed`] if rendering fails (e.g.
/// style config file absent — CN-02 fail-closed).
pub(super) fn render_overview_mermaid(
    baselines: &[BaselineDocument],
    layer: &LayerId,
    style: &StyleConfig,
) -> Result<String, BaselineGraphRendererError> {
    let layer_str = layer.as_ref();
    let layer_sg_id = layer_subgraph_id(layer_str);

    // ---------------------------------------------------------------------------
    // Step 1: Enumerate all clusters for this layer (alphabetical, BTreeMap, CN-08).
    //
    // Scan all baselines for this layer. For each B-r1 node found in krate.paths,
    // derive the cluster key and collect unique clusters.
    // ---------------------------------------------------------------------------
    use node_extractor::extract_nodes;

    let nodes = extract_nodes(baselines, layer);

    // Collect unique clusters (BTreeMap ensures alphabetical ordering by ClusterKey).
    // Map: ClusterKey → cluster_node_id (mermaid id for the cluster node).
    let mut clusters: BTreeMap<ClusterKey, ()> = BTreeMap::new();
    for node in &nodes {
        let doc = node.doc();
        let id = node.id();
        let krate = &doc.krate;
        if let Some(path) = krate.paths.get(&id).map(|s| s.path.as_slice()) {
            if let Some(ck) = cluster_key_from_path(path) {
                clusters.insert(ck, ());
            }
        }
    }

    // ---------------------------------------------------------------------------
    // Step 2: Build node-id → cluster key map (for cross-cluster edge detection).
    // ---------------------------------------------------------------------------
    let node_cluster_map = build_node_cluster_map(baselines, layer_str);

    // ---------------------------------------------------------------------------
    // Step 3: Collect cross-cluster edge pairs using the style-free edge extractor.
    //
    // `collect_entry_edge_pairs` traverses rustdoc data directly to find (src, dst)
    // node id pairs without applying any edge style config lookups.  This is correct
    // for the overview because the overview emits plain `-->` arrows (not styled
    // depth-2 arrows) and must not fail when depth-2-only edge style keys are absent.
    //
    // Duplicate cluster-pair edges are deduplicated (BTreeMap for ordering).
    // cluster_pair: (src_cluster_key, dst_cluster_key)
    // ---------------------------------------------------------------------------
    let entry_edge_pairs = collect_entry_edge_pairs(baselines, layer_str);
    let mut cross_cluster_edges: BTreeMap<(ClusterKey, ClusterKey), ()> = BTreeMap::new();

    for (src_id, dst_id) in &entry_edge_pairs {
        let src_id: &str = src_id.as_str();
        let dst_id: &str = dst_id.as_str();

        let src_cluster = match lookup_cluster(src_id, &node_cluster_map) {
            Some(c) => c,
            None => continue, // anonymous/primitive node — skip (not an entry)
        };
        let dst_cluster = match lookup_cluster(dst_id, &node_cluster_map) {
            Some(c) => c,
            None => continue, // anonymous/primitive node — skip
        };

        // Cross-cluster: source and destination belong to different clusters.
        if src_cluster != dst_cluster {
            cross_cluster_edges.insert((src_cluster.clone(), dst_cluster.clone()), ());
        }
    }

    // ---------------------------------------------------------------------------
    // Step 4: Assemble mermaid output (IN-16 section order).
    //
    // 1. classDef definitions (alphabetical, BTreeMap iteration)
    // 2. layer subgraph > cluster node group (alphabetical by ClusterKey)
    // 3. cross-cluster edge group (sorted by (src_cluster, dst_cluster))
    // 4. class attach group
    // ---------------------------------------------------------------------------

    // Section 1: classDef definitions (alphabetical, CN-08).
    let mut class_defs: Vec<String> = Vec::new();
    for (class_name, class_style) in &style.class {
        class_defs.push(class_def_line(class_name, class_style));
    }

    // Section 2: layer subgraph > cluster node group.
    // Resolve cluster shape from [node.Cluster].shape in the style config (CN-02 config-driven).
    // Falls back to the default rectangular shape (`[{label}]`) when absent.
    let cluster_shape = style.node.get("Cluster").and_then(|ns| ns.shape.as_deref());
    let mut subgraph_lines: Vec<String> = Vec::new();
    subgraph_lines.push(format!("subgraph {layer_sg_id}[\"{layer_str}\"]"));
    subgraph_lines.push("  direction TB".to_string());
    for (ck, ()) in &clusters {
        let cluster_node_id = ck.node_id();
        let label = ck.label();
        // Cluster node definition (no inline :::className — class applied via section 4
        // `class <id> <className>` lines, consistent with IN-16 output structure).
        let shaped = apply_shape(&label, cluster_shape);
        subgraph_lines.push(format!("  {cluster_node_id}{shaped}"));
    }
    subgraph_lines.push("end".to_string());

    // Section 3: cross-cluster edge group (alphabetical by cluster pair via BTreeMap).
    // Cross-cluster edges are emitted as plain `-->` arrows (ADR U-r3 — "集約して表示"):
    // the depth-1 overview is a deliberately collapsed view that shows cluster-to-cluster
    // connectivity, not the original entry-level edge kinds. Edge type detail is preserved
    // in the depth-2 cluster diagrams. Multiple distinct entry-level edges between the same
    // cluster pair collapse into one cluster→cluster `-->` edge (dedup by BTreeMap).
    let mut edge_lines_out: Vec<String> = Vec::new();
    for ((src_ck, dst_ck), ()) in &cross_cluster_edges {
        edge_lines_out.push(format!("{} --> {}", src_ck.node_id(), dst_ck.node_id()));
    }

    // Section 4: class attach group.
    // Class is applied via separate `class <id> <className>` lines (IN-16 section 4).
    // This is consistent with the prohibition on inline :::className in subgraph contexts
    // and keeps class assignment fully config-driven.
    let mut class_attach_lines: Vec<String> = Vec::new();
    if let Some(ns) = style.node.get("Cluster") {
        if let Some(class_name) = ns.class.as_deref() {
            for (ck, ()) in &clusters {
                class_attach_lines.push(format!("class {} {class_name}", ck.node_id()));
            }
        }
    }

    // Assemble final output.
    let mut lines: Vec<String> = Vec::new();
    lines.push(
        "<!-- Generated baseline-graph-renderer (overview) — DO NOT EDIT DIRECTLY -->".to_string(),
    );
    lines.push("```mermaid".to_string());
    lines.push("flowchart LR".to_string());
    for cd in &class_defs {
        lines.push(cd.clone());
    }
    for sl in &subgraph_lines {
        lines.push(sl.clone());
    }
    for el in &edge_lines_out {
        lines.push(el.clone());
    }
    for ca in &class_attach_lines {
        lines.push(ca.clone());
    }
    lines.push("```".to_string());
    lines.push(String::new()); // trailing newline

    Ok(lines.join("\n"))
}

/// Enumerate all clusters in `baselines` for `layer` and render each as a depth-2
/// cluster mermaid diagram (T010).
///
/// Cluster enumeration (IN-14/IN-15 / U-r3):
/// - Scans `krate.paths` for all public B-r1 entries in the layer.
/// - Derives a `ClusterKey` per entry: `(crate_name, path[1])` for module entries,
///   or `(crate_name, "root")` for crate-root entries (path length <= 2).
/// - One mermaid file per unique cluster key.
///
/// Each cluster detail file (depth-2 / IN-15 / AC-14):
/// 1. `classDef` definitions (alphabetical, CN-08).
/// 2. layer subgraph → top-module subgraph → entry subgraph group (alphabetical by name) →
///    method/variant nodes (rustdoc Vec order) + FunctionEntry callable node group (alphabetical).
/// 3. edge group (cluster-internal edges only; cross-cluster edges are suppressed).
/// 4. class attach group.
///
/// Sub-module path is included in the entry subgraph label (e.g. `team::manager::TeamManager`)
/// via the `build_entry_label` function in `entry_subgraph`.
///
/// Cluster key → file name stem: `<crate_name>_<module_seg1>` or `<crate_name>_root`
/// (the cluster_key field of the returned `ClusterRender` is the stem; IN-15 / AC-14).
///
/// # Errors
///
/// Returns `BaselineGraphRendererError::RenderFailed` if rendering fails (e.g.
/// required `[edge.*]` key absent from style config — CN-02 fail-closed).
pub(super) fn render_clusters_mermaid(
    baselines: &[BaselineDocument],
    layer: &LayerId,
    style: &StyleConfig,
) -> Result<Vec<(String, String)>, BaselineGraphRendererError> {
    use node_extractor::extract_nodes;

    let layer_str = layer.as_ref();

    // ---------------------------------------------------------------------------
    // Step 1: Enumerate ALL unique clusters for this layer (alphabetical, CN-08).
    //
    // Scan all B-r1 entries in krate.paths for the layer; derive a ClusterKey per
    // entry and collect into a BTreeMap (for alphabetical ordering).
    // ---------------------------------------------------------------------------
    let nodes = extract_nodes(baselines, layer);

    // BTreeMap: ClusterKey → (crate_name string) — use BTreeMap for alphabetical order.
    let mut clusters: BTreeMap<ClusterKey, String> = BTreeMap::new();
    for node in &nodes {
        let doc = node.doc();
        let id = node.id();
        let krate = &doc.krate;
        if let Some(summary) = krate.paths.get(&id) {
            if let Some(ck) = cluster_key_from_path(&summary.path) {
                clusters.insert(ck, doc.crate_name.as_str().to_string());
            }
        }
    }

    // ---------------------------------------------------------------------------
    // Step 2: Build the node-cluster map for edge cross-cluster filtering.
    // ---------------------------------------------------------------------------
    let node_cluster_map = build_node_cluster_map(baselines, layer_str);

    // ---------------------------------------------------------------------------
    // Step 3: Render each cluster.
    // ---------------------------------------------------------------------------
    let mut results: Vec<(String, String)> = Vec::new();

    for (cluster_key, crate_str) in &clusters {
        let cluster_key_str = cluster_key.node_id(); // e.g. "domain_review" or "domain_root"
        let layer_sg_id = layer_subgraph_id(layer_str);

        // Top-module subgraph id and label (U-r3 depth-2 structure).
        // For root clusters: label = "<crate_name> root"; id = sanitize("<crate_name>") + "_root_sg"
        // For module clusters: label = "<crate_name>::<module_seg1>"; id = cluster_key_str + "_sg"
        let (top_mod_sg_id, top_mod_label) = if cluster_key.module_seg1 == "root" {
            (format!("{}_root_sg", sanitize(crate_str)), format!("{} root", crate_str))
        } else {
            (
                format!("{}_sg", sanitize(&cluster_key_str)),
                format!("{}::{}", crate_str, cluster_key.module_seg1),
            )
        };

        // Section 1: classDef definitions (alphabetical, CN-08).
        let mut class_defs: Vec<String> = Vec::new();
        for (class_name, class_style) in &style.class {
            class_defs.push(class_def_line(class_name, class_style));
        }

        // Section 2: layer subgraph > top-module subgraph > entry subgraphs (T010).
        let mut subgraph_lines: Vec<String> = Vec::new();
        // Section 3: edge definitions (cluster-internal only — cross-cluster suppressed).
        let mut raw_edge_lines: Vec<String> = Vec::new();
        // Section 4: class attach statements.
        let mut class_attach: Vec<String> = Vec::new();

        subgraph_lines.push(format!("subgraph {layer_sg_id}[\"{layer_str}\"]"));
        subgraph_lines.push("  direction TB".to_string());
        subgraph_lines.push(format!("  subgraph {top_mod_sg_id}[\"{top_mod_label}\"]"));
        subgraph_lines.push("    direction TB".to_string());

        // T010: emit entry subgraphs + callable nodes for this cluster only.
        // cluster_key.module_seg1 is "root" for crate-root entries, or the top-level module name.
        entry_subgraph::emit_entries_for_cluster(
            baselines,
            layer_str,
            crate_str,
            &cluster_key.module_seg1,
            &mut subgraph_lines,
            &mut raw_edge_lines,
            &mut class_attach,
            style,
            "    ",
        )?;

        subgraph_lines.push("  end".to_string());
        subgraph_lines.push("end".to_string());

        // ---------------------------------------------------------------------------
        // Cross-cluster edge suppression (IN-15 / AC-14 / CN-07):
        // Only intra-cluster edges are included in the depth-2 cluster detail.
        // An edge is intra-cluster if both its source and destination node ids
        // belong to `cluster_key` in the node_cluster_map (or if either endpoint
        // is an anonymous/primitive node — those are always kept since they have
        // no cluster membership).
        // ---------------------------------------------------------------------------
        let mut edge_lines: Vec<String> = Vec::new();
        for edge_line in &raw_edge_lines {
            // Parse the edge source and destination node ids from the mermaid edge line.
            // Mermaid edge lines have the form: `<src_id> <arrow> <dst_id>` or
            // `<src_id> <arrow>|"label"| <dst_id>`.
            // We extract the first and last whitespace-separated tokens.
            let parts: Vec<&str> = edge_line.split_whitespace().collect();
            let (src_id, dst_id) = match (parts.first(), parts.last()) {
                (Some(&s), Some(&d)) if s != d => (s, d),
                _ => {
                    // Cannot parse — keep the edge (safe default).
                    edge_lines.push(edge_line.clone());
                    continue;
                }
            };

            // Determine cluster membership of both endpoints.
            let src_cluster = lookup_cluster(src_id, &node_cluster_map);
            let dst_cluster = lookup_cluster(dst_id, &node_cluster_map);

            match (src_cluster, dst_cluster) {
                (Some(sc), Some(dc)) => {
                    // Both endpoints have known cluster membership.
                    // Keep only intra-cluster edges (same cluster on both sides).
                    if sc == cluster_key || dc == cluster_key {
                        // At least one endpoint belongs to the current cluster.
                        // Suppress if the other endpoint belongs to a DIFFERENT cluster.
                        let is_intra = sc == dc;
                        if is_intra {
                            edge_lines.push(edge_line.clone());
                        }
                        // Cross-cluster edge: suppress (collected in depth-1 overview).
                    }
                    // Both endpoints belong to another cluster entirely: skip.
                }
                (Some(sc), None) => {
                    // Source belongs to a known cluster; destination has no cluster entry.
                    //
                    // T016 note: anonymous nodes (`anon_*` / `prim_*` / `generic_*`) are no
                    // longer generated by the depth-2 renderer or `collect_entry_edge_pairs`.
                    // This branch now handles only structured node-ids (D-decision prefix
                    // `T` / `R` / `F` followed by digits) that map to types not in the layer's
                    // B-r1 node set — e.g. external-library types present in `krate.paths` but
                    // not rendered as entry subgraphs.  Such edges point to undefined mermaid
                    // nodes and must be suppressed.
                    //
                    // Suppress in all cases: either the source is from a different cluster, or
                    // the destination is a non-rendered type (structured id not in the map).
                    let _ = sc; // suppress unused-variable warning
                }
                _ => {
                    // Source is anonymous/primitive (no cluster entry in the map).
                    // Keep the edge — a source with no cluster membership cannot be
                    // attributed to a foreign cluster, so it is treated as local.
                    edge_lines.push(edge_line.clone());
                }
            }
        }

        // Assemble mermaid content in section order (IN-16):
        // 1) classDef  2) layer subgraph  3) edge definitions  4) class attach
        let mut lines: Vec<String> = Vec::new();
        lines.push(format!(
            "<!-- Generated baseline-graph-renderer (cluster: {cluster_key_str}) — DO NOT EDIT DIRECTLY -->"
        ));
        lines.push("```mermaid".to_string());
        lines.push("flowchart LR".to_string());
        for cd in &class_defs {
            lines.push(cd.clone());
        }
        for sl in &subgraph_lines {
            lines.push(sl.clone());
        }
        for el in &edge_lines {
            lines.push(el.clone());
        }
        for ca in &class_attach {
            lines.push(ca.clone());
        }
        lines.push("```".to_string());
        lines.push(String::new()); // trailing newline

        results.push((cluster_key_str, lines.join("\n")));
    }

    Ok(results)
}

// ---------------------------------------------------------------------------
// Tests for render internals
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // sanitize
    // -----------------------------------------------------------------------

    #[test]
    fn test_sanitize_replaces_hyphens_and_colons() {
        let result = sanitize("my-crate::module");
        assert_eq!(result, "my_crate__module");
    }

    #[test]
    fn test_sanitize_preserves_alphanumeric_and_underscore() {
        let result = sanitize("domain_crate_123");
        assert_eq!(result, "domain_crate_123");
    }

    // -----------------------------------------------------------------------
    // apply_shape
    // -----------------------------------------------------------------------

    #[test]
    fn test_apply_shape_with_template() {
        let result = apply_shape("MyNode", Some("([{label}])"));
        assert_eq!(result, "([MyNode])");
    }

    #[test]
    fn test_apply_shape_without_template_uses_default_brackets() {
        let result = apply_shape("MyNode", None);
        assert_eq!(result, "[MyNode]");
    }

    // -----------------------------------------------------------------------
    // edge_arrow_label
    // -----------------------------------------------------------------------

    #[test]
    fn test_edge_arrow_label_returns_ok_for_present_key() {
        let mut map: BTreeMap<String, EdgeStyle> = BTreeMap::new();
        map.insert(
            "trait_impl".to_string(),
            EdgeStyle { arrow: "-.impl.->".to_string(), label: None },
        );
        let (arrow, label) = edge_arrow_label(&map, "trait_impl").unwrap();
        assert_eq!(arrow, "-.impl.->");
        assert!(label.is_none());
    }

    #[test]
    fn test_edge_arrow_label_returns_err_for_absent_key() {
        let map: BTreeMap<String, EdgeStyle> = BTreeMap::new();
        let err = edge_arrow_label(&map, "alias").unwrap_err();
        assert!(
            matches!(err, BaselineGraphRendererError::RenderFailed { .. }),
            "absent edge key must return RenderFailed"
        );
    }

    #[test]
    fn test_edge_arrow_label_returns_label_when_present() {
        let mut map: BTreeMap<String, EdgeStyle> = BTreeMap::new();
        map.insert(
            "alias".to_string(),
            EdgeStyle { arrow: "---".to_string(), label: Some("alias_of".to_string()) },
        );
        let (arrow, label) = edge_arrow_label(&map, "alias").unwrap();
        assert_eq!(arrow, "---");
        assert_eq!(label, Some("alias_of"));
    }

    // -----------------------------------------------------------------------
    // render_overview_mermaid: minimal valid output checks
    // -----------------------------------------------------------------------

    fn minimal_style() -> StyleConfig {
        toml::from_str::<StyleConfig>("[filter]\ninclude_functions = true\n").unwrap()
    }

    fn minimal_crate() -> rustdoc_types::Crate {
        let json = format!(
            r#"{{
                "root": 0,
                "crate_version": null,
                "includes_private": false,
                "index": {{}},
                "paths": {{}},
                "external_crates": {{}},
                "format_version": {},
                "target": {{"triple": "", "target_features": []}}
            }}"#,
            rustdoc_types::FORMAT_VERSION
        );
        serde_json::from_str(&json).expect("minimal_crate JSON must be valid")
    }

    fn make_baseline(layer_str: &str, crate_str: &str) -> BaselineDocument {
        use domain::tddd::baseline_document::BaselineDocument;
        use domain::tddd::catalogue_v2::identifiers::CrateName;
        use domain::tddd::layer_id::LayerId;
        BaselineDocument::new(
            LayerId::try_new(layer_str).unwrap(),
            CrateName::new(crate_str).unwrap(),
            minimal_crate(),
        )
    }

    #[test]
    fn test_render_overview_mermaid_starts_with_header_comment() {
        let layer = LayerId::try_new("domain").unwrap();
        let baseline = make_baseline("domain", "domain");
        let style = minimal_style();
        let output = render_overview_mermaid(&[baseline], &layer, &style).unwrap();
        assert!(
            output.starts_with("<!-- Generated baseline-graph-renderer"),
            "must start with header comment; got: {:?}",
            &output[..output.len().min(80)]
        );
    }

    #[test]
    fn test_render_overview_mermaid_contains_mermaid_fence() {
        let layer = LayerId::try_new("domain").unwrap();
        let style = minimal_style();
        let output = render_overview_mermaid(&[], &layer, &style).unwrap();
        assert!(output.contains("```mermaid\n"), "must contain opening mermaid fence");
        assert!(output.contains("\n```\n"), "must contain closing mermaid fence");
    }

    #[test]
    fn test_render_overview_mermaid_body_starts_with_flowchart_lr() {
        let layer = LayerId::try_new("domain").unwrap();
        let style = minimal_style();
        let output = render_overview_mermaid(&[], &layer, &style).unwrap();
        let fence_open = "```mermaid\n";
        let fence_start = output.find(fence_open).unwrap() + fence_open.len();
        let fence_end = output[fence_start..]
            .find("\n```")
            .map(|i| fence_start + i + 1)
            .unwrap_or(output.len());
        let mermaid_body = &output[fence_start..fence_end];
        assert!(
            mermaid_body.starts_with("flowchart LR\n"),
            "mermaid body must start with 'flowchart LR\\n'; got: {:?}",
            &mermaid_body[..mermaid_body.len().min(40)]
        );
    }

    #[test]
    fn test_render_overview_mermaid_contains_layer_subgraph() {
        let layer = LayerId::try_new("domain").unwrap();
        let baseline = make_baseline("domain", "domain");
        let style = minimal_style();
        let output = render_overview_mermaid(&[baseline], &layer, &style).unwrap();
        assert!(output.contains("subgraph domain"), "must contain layer subgraph");
    }

    #[test]
    fn test_render_overview_mermaid_filters_to_given_layer() {
        let layer = LayerId::try_new("domain").unwrap();
        let baseline_domain = make_baseline("domain", "domain");
        let baseline_usecase = make_baseline("usecase", "usecase");
        let style = minimal_style();
        let output =
            render_overview_mermaid(&[baseline_domain, baseline_usecase], &layer, &style).unwrap();
        // Must contain a reference to domain layer.
        assert!(output.contains("domain"), "must mention domain");
        // Must NOT contain the usecase layer (different layer) in the layer subgraph area.
        // The layer subgraph for "domain" must not be labelled "usecase".
        assert!(
            !output.contains("subgraph usecase"),
            "must not include usecase layer subgraph in domain layer render"
        );
    }

    // -----------------------------------------------------------------------
    // render_clusters_mermaid: minimal valid output checks
    // -----------------------------------------------------------------------

    // T010: test helpers that produce a crate with real B-r1 entries in krate.paths
    // (required for render_clusters_mermaid — the T010 implementation reads paths, not just baselines)

    fn make_baseline_with_module_struct(
        layer_str: &str,
        crate_str: &str,
        module_seg1: Option<&str>,
        struct_name: &str,
    ) -> BaselineDocument {
        use domain::tddd::baseline_document::BaselineDocument;
        use domain::tddd::catalogue_v2::identifiers::CrateName;
        use domain::tddd::layer_id::LayerId;
        let krate = make_crate_with_struct(crate_str, module_seg1, struct_name);
        BaselineDocument::new(
            LayerId::try_new(layer_str).unwrap(),
            CrateName::new(crate_str).unwrap(),
            krate,
        )
    }

    // -----------------------------------------------------------------------
    // T010: render_clusters_mermaid — full cluster enumeration
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_clusters_mermaid_crate_root_entry_produces_root_cluster() {
        // T010: a baseline with a struct at crate root → one cluster with key "domain_root"
        let layer = LayerId::try_new("domain").unwrap();
        let baseline = make_baseline_with_module_struct("domain", "domain", None, "MyStruct");
        let style = minimal_style();
        let clusters = render_clusters_mermaid(&[baseline], &layer, &style).unwrap();
        assert_eq!(clusters.len(), 1, "one entry at crate root → one cluster");
        assert_eq!(clusters[0].0, "domain_root", "cluster key must be 'domain_root'");
    }

    #[test]
    fn test_render_clusters_mermaid_module_entry_produces_module_cluster() {
        // T010: a struct in "review" module → cluster key "domain_review"
        let layer = LayerId::try_new("domain").unwrap();
        let baseline =
            make_baseline_with_module_struct("domain", "domain", Some("review"), "Review");
        let style = minimal_style();
        let clusters = render_clusters_mermaid(&[baseline], &layer, &style).unwrap();
        assert_eq!(clusters.len(), 1, "one entry in 'review' module → one cluster");
        assert_eq!(clusters[0].0, "domain_review", "cluster key must be 'domain_review'");
    }

    #[test]
    fn test_render_clusters_mermaid_two_modules_produce_two_clusters() {
        // T010: entries in two different modules → two separate clusters
        let layer = LayerId::try_new("domain").unwrap();
        let baseline_review =
            make_baseline_with_module_struct("domain", "domain", Some("review"), "Review");
        let baseline_user =
            make_baseline_with_module_struct("domain", "domain", Some("user"), "User");
        let style = minimal_style();
        let clusters =
            render_clusters_mermaid(&[baseline_review, baseline_user], &layer, &style).unwrap();
        assert_eq!(clusters.len(), 2, "two modules → two clusters");
        // Alphabetical order: "review" < "user"
        assert_eq!(clusters[0].0, "domain_review", "first cluster alphabetically");
        assert_eq!(clusters[1].0, "domain_user", "second cluster alphabetically");
    }

    #[test]
    fn test_render_clusters_mermaid_cluster_content_contains_mermaid_fence() {
        // T010: cluster content must be a valid mermaid fenced block
        let layer = LayerId::try_new("domain").unwrap();
        let baseline = make_baseline_with_module_struct("domain", "domain", None, "MyStruct");
        let style = minimal_style();
        let clusters = render_clusters_mermaid(&[baseline], &layer, &style).unwrap();
        assert!(!clusters.is_empty(), "one crate root entry → one cluster");
        let content = &clusters[0].1;
        assert!(content.contains("```mermaid\n"), "cluster content must have mermaid fence");
        assert!(content.contains("\n```\n"), "cluster content must have closing fence");
    }

    #[test]
    fn test_render_clusters_mermaid_empty_baselines_returns_empty() {
        let layer = LayerId::try_new("domain").unwrap();
        let style = minimal_style();
        let clusters = render_clusters_mermaid(&[], &layer, &style).unwrap();
        assert!(clusters.is_empty(), "no baselines → no clusters");
    }

    #[test]
    fn test_render_clusters_mermaid_empty_crate_returns_empty() {
        // T010: a baseline with no entries in krate.paths → no clusters emitted
        let layer = LayerId::try_new("domain").unwrap();
        let baseline = make_baseline("domain", "domain"); // empty crate — no index entries
        let style = minimal_style();
        let clusters = render_clusters_mermaid(&[baseline], &layer, &style).unwrap();
        assert!(clusters.is_empty(), "empty crate (no entries in paths) → no clusters");
    }

    #[test]
    fn test_render_clusters_mermaid_filters_to_given_layer() {
        // T010: baselines from a different layer must not produce clusters for the target layer
        let layer = LayerId::try_new("domain").unwrap();
        let baseline_domain =
            make_baseline_with_module_struct("domain", "domain", Some("review"), "Review");
        let baseline_usecase =
            make_baseline_with_module_struct("usecase", "usecase", Some("commands"), "CreateUser");
        let style = minimal_style();
        let clusters =
            render_clusters_mermaid(&[baseline_domain, baseline_usecase], &layer, &style).unwrap();
        // Only domain layer clusters should be returned (layer filter).
        assert_eq!(clusters.len(), 1, "only domain layer baseline should produce clusters");
        assert!(
            clusters[0].0.starts_with("domain"),
            "cluster key must be prefixed with crate name; got: {}",
            clusters[0].0
        );
    }

    #[test]
    fn test_render_clusters_mermaid_cluster_contains_top_module_subgraph() {
        // T010: depth-2 cluster detail must contain a top-module subgraph with the module name
        let layer = LayerId::try_new("domain").unwrap();
        let baseline =
            make_baseline_with_module_struct("domain", "domain", Some("review"), "Review");
        let style = minimal_style();
        let clusters = render_clusters_mermaid(&[baseline], &layer, &style).unwrap();
        assert_eq!(clusters.len(), 1);
        let content = &clusters[0].1;
        // The top-module subgraph label includes the crate::module path
        assert!(
            content.contains("domain::review"),
            "cluster detail must show top-module subgraph label 'domain::review'; content:\n{content}"
        );
    }

    #[test]
    fn test_render_clusters_mermaid_root_cluster_has_root_subgraph_label() {
        // T010: crate-root cluster must have a top-module subgraph labelled "<crate_name> root"
        let layer = LayerId::try_new("domain").unwrap();
        let baseline = make_baseline_with_module_struct("domain", "domain", None, "MyStruct");
        let style = minimal_style();
        let clusters = render_clusters_mermaid(&[baseline], &layer, &style).unwrap();
        assert_eq!(clusters.len(), 1);
        let content = &clusters[0].1;
        assert!(
            content.contains("domain root"),
            "root cluster label must be 'domain root'; content:\n{content}"
        );
    }

    #[test]
    fn test_render_clusters_mermaid_cluster_key_is_file_stem() {
        // T010 / AC-14: cluster_key field is the file name stem (no extension)
        // e.g. "domain_review" → file stem, caller adds ".md"
        let layer = LayerId::try_new("domain").unwrap();
        let baseline =
            make_baseline_with_module_struct("domain", "domain", Some("review"), "Review");
        let style = minimal_style();
        let clusters = render_clusters_mermaid(&[baseline], &layer, &style).unwrap();
        assert_eq!(clusters.len(), 1);
        // The cluster key returned is the stem (no ".md" suffix)
        let stem = &clusters[0].0;
        assert!(!stem.contains('.'), "cluster key must not include extension; got: {stem}");
        assert_eq!(stem, "domain_review", "stem must equal cluster_key_str");
    }

    #[test]
    fn test_render_clusters_mermaid_clusters_alphabetical() {
        // T010 / CN-08: clusters must be returned in alphabetical order by ClusterKey
        let layer = LayerId::try_new("domain").unwrap();
        // Add in reverse alphabetical order; output must still be alphabetical.
        let baseline_user =
            make_baseline_with_module_struct("domain", "domain", Some("user"), "User");
        let baseline_review =
            make_baseline_with_module_struct("domain", "domain", Some("review"), "Review");
        let baseline_root = make_baseline_with_module_struct("domain", "domain", None, "Root");
        let style = minimal_style();
        let clusters = render_clusters_mermaid(
            &[baseline_user, baseline_review, baseline_root],
            &layer,
            &style,
        )
        .unwrap();
        assert_eq!(clusters.len(), 3);
        // ClusterKey orders: (domain, review) < (domain, root) < (domain, user)
        // because 'r' < 'r' at second char 'e' vs 'o' → "review" < "root",
        // and "root" < "user" (alphabetical).
        assert_eq!(clusters[0].0, "domain_review");
        assert_eq!(clusters[1].0, "domain_root");
        assert_eq!(clusters[2].0, "domain_user");
    }

    // -----------------------------------------------------------------------
    // StyleConfig TOML deserialization
    // -----------------------------------------------------------------------

    #[test]
    fn test_style_config_minimal_deserializes_ok() {
        let toml_str = "[filter]\ninclude_functions = true\n";
        let config = toml::from_str::<StyleConfig>(toml_str);
        assert!(config.is_ok(), "minimal valid TOML must deserialize; err={config:?}");
    }

    #[test]
    fn test_style_config_with_class_section_deserializes_ok() {
        // Use single-quote TOML literals to avoid Rust raw-string termination issues.
        let toml_str = r#"
[class.struct_entry]
fill = '#dbeafe'
stroke = '#1e40af'
stroke_width = '2px'

[filter]
include_functions = true
"#;
        let config = toml::from_str::<StyleConfig>(toml_str);
        assert!(config.is_ok(), "TOML with [class.*] must deserialize; err={config:?}");
        let config = config.unwrap();
        assert!(config.class.contains_key("struct_entry"));
    }

    #[test]
    fn test_style_config_with_edge_section_deserializes_ok() {
        let toml_str = r#"
[edge.trait_impl]
arrow = "-.impl.->"

[filter]
include_functions = true
"#;
        let config = toml::from_str::<StyleConfig>(toml_str);
        assert!(config.is_ok(), "TOML with [edge.*] must deserialize; err={config:?}");
        let config = config.unwrap();
        let edge = &config.edge["trait_impl"];
        assert_eq!(edge.arrow, "-.impl.->");
        assert!(edge.label.is_none());
    }

    #[test]
    fn test_style_config_with_role_section_fails_deny_unknown_fields() {
        // [role.*] is NOT part of baseline-graph-style.toml schema (IN-04).
        // deny_unknown_fields must reject it.
        let toml_str = r#"
[role.Entity]
class = "entity"

[filter]
include_functions = true
"#;
        let result = toml::from_str::<StyleConfig>(toml_str);
        assert!(
            result.is_err(),
            "[role.*] section must be rejected by deny_unknown_fields in baseline StyleConfig"
        );
    }

    // -----------------------------------------------------------------------
    // T009: ClusterKey + cluster_key_from_path
    // -----------------------------------------------------------------------

    #[test]
    fn test_cluster_key_from_path_crate_root_item() {
        // path = [crate_name, item_name] → cluster key = (crate, "root")
        let path = vec!["my_crate".to_string(), "MyStruct".to_string()];
        let key = cluster_key_from_path(&path).unwrap();
        assert_eq!(key.crate_name, "my_crate");
        assert_eq!(key.module_seg1, "root");
    }

    #[test]
    fn test_cluster_key_from_path_module_item() {
        // path = [crate_name, module_seg1, ..., item_name] → cluster key = (crate, module_seg1)
        let path = vec!["my_crate".to_string(), "review".to_string(), "MyReview".to_string()];
        let key = cluster_key_from_path(&path).unwrap();
        assert_eq!(key.crate_name, "my_crate");
        assert_eq!(key.module_seg1, "review");
    }

    #[test]
    fn test_cluster_key_from_path_deep_nested_item_uses_only_seg1() {
        // path = [crate, mod1, mod2, item] → cluster key uses only mod1 (not mod2)
        let path = vec![
            "my_crate".to_string(),
            "team".to_string(),
            "manager".to_string(),
            "TeamManager".to_string(),
        ];
        let key = cluster_key_from_path(&path).unwrap();
        assert_eq!(key.crate_name, "my_crate");
        assert_eq!(key.module_seg1, "team"); // only the top-level module segment
    }

    #[test]
    fn test_cluster_key_from_path_empty_path_returns_none() {
        let path: Vec<String> = vec![];
        assert!(cluster_key_from_path(&path).is_none());
    }

    #[test]
    fn test_cluster_key_node_id_root_cluster() {
        let ck = ClusterKey { crate_name: "domain".to_string(), module_seg1: "root".to_string() };
        assert_eq!(ck.node_id(), "domain_root");
    }

    #[test]
    fn test_cluster_key_node_id_module_cluster() {
        let ck = ClusterKey { crate_name: "domain".to_string(), module_seg1: "review".to_string() };
        assert_eq!(ck.node_id(), "domain_review");
    }

    #[test]
    fn test_cluster_key_label_root_cluster() {
        let ck = ClusterKey { crate_name: "domain".to_string(), module_seg1: "root".to_string() };
        assert_eq!(ck.label(), "domain root");
    }

    #[test]
    fn test_cluster_key_label_module_cluster() {
        let ck = ClusterKey { crate_name: "domain".to_string(), module_seg1: "review".to_string() };
        assert_eq!(ck.label(), "domain::review");
    }

    #[test]
    fn test_cluster_key_node_id_sanitizes_hyphens() {
        let ck =
            ClusterKey { crate_name: "my-crate".to_string(), module_seg1: "my-module".to_string() };
        // sanitize("my-crate") = "my_crate", sanitize("my-module") = "my_module" → "my_crate_my_module"
        assert_eq!(ck.node_id(), "my_crate_my_module");
    }

    #[test]
    fn test_cluster_key_node_id_format_matches_adr_u_r3() {
        // ADR U-r3 (ライン337/352) specifies cluster node id as `<crate_name>_<module_seg1>`.
        // Known limitation (ADR ライン199): collision possible for pathological names (e.g.
        // crate ending with `_` and module starting with `_`), accepted for real architectures.
        let ck1 = ClusterKey { crate_name: "foo".to_string(), module_seg1: "bar_baz".to_string() };
        let ck2 = ClusterKey { crate_name: "foo_bar".to_string(), module_seg1: "baz".to_string() };
        // Same sanitized suffix "foo_bar_baz" — this is the accepted non-injective case per ADR line 199.
        // Both produce the SAME id "foo_bar_baz"; real architectures do not encounter this.
        assert_eq!(ck1.node_id(), "foo_bar_baz");
        assert_eq!(ck2.node_id(), "foo_bar_baz");
    }

    // -----------------------------------------------------------------------
    // T009: render_overview_mermaid — cluster enumeration
    // -----------------------------------------------------------------------

    use domain::tddd::catalogue_v2::identifiers::CrateName;
    use std::collections::HashMap;

    fn make_crate_with_struct(
        crate_name: &str,
        module_seg1: Option<&str>,
        struct_name: &str,
    ) -> rustdoc_types::Crate {
        use rustdoc_types::{
            Crate, Generics, Id, Item, ItemEnum, ItemKind, ItemSummary, Struct, StructKind, Target,
            Visibility,
        };

        let root_id = Id(0);
        let struct_id = Id(1);

        let mut index = HashMap::new();
        index.insert(
            root_id,
            Item {
                id: root_id,
                crate_id: 0,
                name: Some(crate_name.to_string()),
                span: None,
                visibility: Visibility::Public,
                docs: None,
                links: HashMap::new(),
                attrs: vec![],
                deprecation: None,
                inner: ItemEnum::Module(rustdoc_types::Module {
                    is_crate: true,
                    items: vec![struct_id],
                    is_stripped: false,
                }),
            },
        );
        index.insert(
            struct_id,
            Item {
                id: struct_id,
                crate_id: 0,
                name: Some(struct_name.to_string()),
                span: None,
                visibility: Visibility::Public,
                docs: None,
                links: HashMap::new(),
                attrs: vec![],
                deprecation: None,
                inner: ItemEnum::Struct(Struct {
                    kind: StructKind::Plain { fields: vec![], has_stripped_fields: false },
                    generics: Generics { params: vec![], where_predicates: vec![] },
                    impls: vec![],
                }),
            },
        );

        let mut paths = HashMap::new();
        let path = if let Some(mod1) = module_seg1 {
            vec![crate_name.to_string(), mod1.to_string(), struct_name.to_string()]
        } else {
            vec![crate_name.to_string(), struct_name.to_string()]
        };
        paths.insert(struct_id, ItemSummary { crate_id: 0, path, kind: ItemKind::Struct });

        Crate {
            root: root_id,
            crate_version: None,
            includes_private: false,
            index,
            paths,
            external_crates: HashMap::new(),
            format_version: rustdoc_types::FORMAT_VERSION,
            target: Target { triple: String::new(), target_features: vec![] },
        }
    }

    #[test]
    fn test_render_overview_empty_baselines_no_cluster_nodes() {
        let layer = LayerId::try_new("domain").unwrap();
        let style = minimal_style();
        let output = render_overview_mermaid(&[], &layer, &style).unwrap();
        // No clusters → no cluster-specific node ids in the output.
        // The only node id present in the empty case must not be a cluster node id.
        // Check that the layer subgraph is present.
        assert!(output.contains("subgraph domain"), "must have layer subgraph frame");
        // No cluster node definitions: cluster nodes have ids like "domain_root" or "domain_review".
        // With no baselines, no such ids should appear.
        assert!(
            !output.contains("domain_root"),
            "no baselines → no cluster node ids; output: {output}"
        );
        assert!(
            !output.contains("domain_review"),
            "no baselines → no cluster node ids; output: {output}"
        );
    }

    #[test]
    fn test_render_overview_crate_root_entry_produces_root_cluster_node() {
        // A struct with path = [crate_name, StructName] → cluster node "domain_root".
        let layer = LayerId::try_new("domain").unwrap();
        let krate = make_crate_with_struct("domain", None, "MyStruct");
        let baseline = BaselineDocument::new(
            LayerId::try_new("domain").unwrap(),
            CrateName::new("domain").unwrap(),
            krate,
        );
        let style = minimal_style();
        let output = render_overview_mermaid(&[baseline], &layer, &style).unwrap();
        assert!(
            output.contains("domain_root"),
            "crate root struct must produce domain_root cluster node; output:\n{output}"
        );
        // Class applied via section 4 `class <id> <name>` only when [node.Cluster].class is set.
        // minimal_style() has no [node.Cluster], so no class annotation; node id is sufficient.
    }

    #[test]
    fn test_render_overview_module_entry_produces_module_cluster_node() {
        // A struct with path = [crate_name, "review", StructName] → cluster node "domain_review".
        let layer = LayerId::try_new("domain").unwrap();
        let krate = make_crate_with_struct("domain", Some("review"), "Review");
        let baseline = BaselineDocument::new(
            LayerId::try_new("domain").unwrap(),
            CrateName::new("domain").unwrap(),
            krate,
        );
        let style = minimal_style();
        let output = render_overview_mermaid(&[baseline], &layer, &style).unwrap();
        assert!(
            output.contains("domain_review"),
            "module struct must produce domain_review cluster node; output:\n{output}"
        );
        // Class applied via section 4 only when [node.Cluster].class is set.
        // minimal_style() has no [node.Cluster]; node id presence is the key assertion.
    }

    #[test]
    fn test_render_overview_two_clusters_alphabetical_order() {
        // Two module entries: "user" and "review" — should appear in alphabetical order.
        let layer = LayerId::try_new("domain").unwrap();
        let krate_review = make_crate_with_struct("domain", Some("review"), "Review");
        let krate_user = make_crate_with_struct("domain", Some("user"), "User");
        let baseline_review = BaselineDocument::new(
            LayerId::try_new("domain").unwrap(),
            CrateName::new("domain").unwrap(),
            krate_review,
        );
        let baseline_user = BaselineDocument::new(
            LayerId::try_new("domain").unwrap(),
            CrateName::new("domain").unwrap(),
            krate_user,
        );
        let style = minimal_style();
        // Use baselines in reverse order — output must still be alphabetical.
        let output =
            render_overview_mermaid(&[baseline_user, baseline_review], &layer, &style).unwrap();

        // Both clusters must appear.
        assert!(output.contains("domain_review"), "domain_review cluster must be present");
        assert!(output.contains("domain_user"), "domain_user cluster must be present");

        // "review" cluster must appear before "user" cluster (alphabetical).
        let pos_review = output.find("domain_review").unwrap();
        let pos_user = output.find("domain_user").unwrap();
        assert!(
            pos_review < pos_user,
            "domain_review must appear before domain_user (alphabetical); got positions review={pos_review} user={pos_user}"
        );
    }

    #[test]
    fn test_render_overview_layer_agnostic_custom_layer_in_subgraph() {
        // Renderer must not hardcode layer names.
        let layer = LayerId::try_new("my_custom_layer").unwrap();
        let krate = make_crate_with_struct("my_crate", None, "MyStruct");
        let baseline = BaselineDocument::new(
            LayerId::try_new("my_custom_layer").unwrap(),
            CrateName::new("my_crate").unwrap(),
            krate,
        );
        let style = minimal_style();
        let output = render_overview_mermaid(&[baseline], &layer, &style).unwrap();
        assert!(
            output.contains("my_custom_layer"),
            "custom layer name must appear in layer subgraph; output:\n{output}"
        );
        assert!(
            !output.contains("domain"),
            "hardcoded 'domain' must not appear in output; output:\n{output}"
        );
    }

    #[test]
    fn test_render_overview_different_layer_baselines_not_included() {
        // Baselines from a different layer must NOT produce cluster nodes.
        let layer = LayerId::try_new("domain").unwrap();
        let krate_usecase = make_crate_with_struct("usecase", Some("commands"), "CreateUser");
        let baseline_usecase = BaselineDocument::new(
            LayerId::try_new("usecase").unwrap(),
            CrateName::new("usecase").unwrap(),
            krate_usecase,
        );
        let style = minimal_style();
        let output = render_overview_mermaid(&[baseline_usecase], &layer, &style).unwrap();
        // Usecase layer nodes must NOT appear in domain layer overview.
        assert!(
            !output.contains("usecase_commands"),
            "usecase cluster must not appear in domain layer; output:\n{output}"
        );
    }

    #[test]
    fn test_render_overview_output_structure_section_order() {
        // Section order: 1. classDef 2. layer subgraph 3. edges 4. class attach
        // With empty baselines + 1 class, verify that subgraph comes after classDef.
        let layer = LayerId::try_new("domain").unwrap();
        let toml_with_class = r#"
[class.clusterStyle]
fill = '#f0f9ff'

[filter]
include_functions = true
"#;
        let style = toml::from_str::<StyleConfig>(toml_with_class).unwrap();
        let output = render_overview_mermaid(&[], &layer, &style).unwrap();

        let pos_classdef = output.find("classDef clusterStyle").unwrap_or(usize::MAX);
        let pos_subgraph = output.find("subgraph domain").unwrap_or(usize::MAX);
        assert!(
            pos_classdef < pos_subgraph,
            "classDef must appear before layer subgraph; classDef at {pos_classdef}, subgraph at {pos_subgraph}"
        );
    }

    #[test]
    fn test_render_overview_no_intra_cluster_edges_emitted() {
        // Two structs in the SAME module cluster → no cross-cluster edge must appear.
        // Use two separate baselines to test the intra-cluster suppression.
        let layer = LayerId::try_new("domain").unwrap();
        // Both in "review" module — same cluster.
        let krate = make_crate_with_struct("domain", Some("review"), "Review");
        let baseline = BaselineDocument::new(
            LayerId::try_new("domain").unwrap(),
            CrateName::new("domain").unwrap(),
            krate,
        );
        let style = minimal_style();
        let output = render_overview_mermaid(&[baseline], &layer, &style).unwrap();
        // Single cluster → no cross-cluster edges can exist.
        // Extract mermaid body (between ```mermaid and closing ```) to avoid matching
        // the HTML comment's `-->` terminator.
        let fence_open = "```mermaid\n";
        let fence_start = output.find(fence_open).unwrap() + fence_open.len();
        let fence_end = output[fence_start..]
            .find("\n```")
            .map(|i| fence_start + i + 1)
            .unwrap_or(output.len());
        let mermaid_body = &output[fence_start..fence_end];
        assert!(
            !mermaid_body.contains("-->"),
            "no cross-cluster edges expected with single cluster; mermaid body:\n{mermaid_body}"
        );
    }

    // -----------------------------------------------------------------------
    // T012: layer-agnostic unit tests (AC-16 / CN-01 / CN-03 / CN-09)
    //
    // These tests verify that the renderer (BaselineGraphRendererAdapter) works
    // correctly with 2-layer, 3-layer, and custom-layer-name configurations,
    // and that subgraph labels / node ids do NOT hard-code layer names.
    // -----------------------------------------------------------------------

    // T012 helper: create a baseline with one public struct in a module.
    // `layer_str` and `crate_str` are kept independent to support multi-layer setups
    // where the layer name differs from the crate name.
    fn make_baseline_with_struct(
        layer_str: &str,
        crate_str: &str,
        module_seg1: Option<&str>,
        struct_name: &str,
    ) -> BaselineDocument {
        let krate = make_crate_with_struct(crate_str, module_seg1, struct_name);
        BaselineDocument::new(
            LayerId::try_new(layer_str).unwrap(),
            CrateName::new(crate_str).unwrap(),
            krate,
        )
    }

    // T012: 2-layer configuration — renderer produces separate, correct output for each layer.
    //
    // AC-16: renderer operates correctly with 2-layer fixture (domain + usecase).
    // CN-01: layer name must not be hardcoded — both layers must produce valid output
    //        with their own layer name in the subgraph, not each other's name.
    #[test]
    fn test_render_overview_two_layer_configuration_each_layer_produces_correct_output() {
        let style = minimal_style();
        let baselines = [
            make_baseline_with_struct("domain", "domain", Some("review"), "Review"),
            make_baseline_with_struct("usecase", "usecase", Some("commands"), "CreateReview"),
        ];

        for layer_str in ["domain", "usecase"] {
            let layer = LayerId::try_new(layer_str).unwrap();
            let output = render_overview_mermaid(&baselines, &layer, &style).unwrap();
            assert!(
                output.contains(layer_str),
                "2-layer: output for layer '{layer_str}' must contain its own name; got:\n{output}"
            );
            // The other layer's name must not appear as a subgraph label.
            let other = if layer_str == "domain" { "usecase" } else { "domain" };
            assert!(
                !output.contains(&format!("subgraph {other}")),
                "2-layer: output for '{layer_str}' must not contain other layer's subgraph '{other}'; got:\n{output}"
            );
        }
    }

    // T012: 3-layer configuration — renderer operates correctly for all three layers.
    //
    // AC-16: renderer operates correctly with 3-layer fixture.
    // CN-01: each layer produces its own output without hardcoded foreign layer names.
    #[test]
    fn test_render_overview_three_layer_configuration_each_layer_independent() {
        let style = minimal_style();
        let layers = ["domain", "usecase", "infrastructure"];
        let baselines: Vec<BaselineDocument> = layers
            .iter()
            .map(|&l| make_baseline_with_struct(l, l, Some("types"), "MyType"))
            .collect();

        for &layer_str in &layers {
            let layer = LayerId::try_new(layer_str).unwrap();
            let output = render_overview_mermaid(&baselines, &layer, &style).unwrap();
            // Own layer name must appear.
            assert!(
                output.contains(layer_str),
                "3-layer: output for '{layer_str}' must contain its own name"
            );
            // Other layer names must NOT appear as subgraph labels.
            for &other in &layers {
                if other != layer_str {
                    assert!(
                        !output.contains(&format!("subgraph {other}")),
                        "3-layer: layer '{layer_str}' output must not contain subgraph '{other}'"
                    );
                }
            }
        }
    }

    // T012: custom (non-standard) layer names — renderer is layer-agnostic (CN-01).
    //
    // AC-16: confirms that non-domain/usecase/infrastructure layer names work correctly.
    // CN-01: renderer must not hardcode standard layer names; it must handle arbitrary LayerId values.
    #[test]
    fn test_render_overview_custom_layer_names_not_hardcoded() {
        let style = minimal_style();
        let custom_layers = ["alpha", "beta", "gamma"];
        let baselines: Vec<BaselineDocument> = custom_layers
            .iter()
            .map(|&l| make_baseline_with_struct(l, l, Some("module_a"), "TypeA"))
            .collect();

        for &layer_str in &custom_layers {
            let layer = LayerId::try_new(layer_str).unwrap();
            let output = render_overview_mermaid(&baselines, &layer, &style).unwrap();
            // Custom layer name must appear in subgraph label.
            assert!(
                output.contains(&format!("subgraph {layer_str}")),
                "custom layer '{layer_str}' must appear as subgraph; output:\n{output}"
            );
            // Standard layer names must NOT appear (layer-agnostic guarantee).
            for standard_name in ["domain", "usecase", "infrastructure"] {
                assert!(
                    !output.contains(&format!("subgraph {standard_name}")),
                    "custom layer '{layer_str}' output must not hardcode standard name '{standard_name}'"
                );
            }
        }
    }

    // T012: subgraph label does not hardcode layer names — render_clusters_mermaid (CN-01).
    //
    // The depth-2 cluster detail must use the actual layer name passed in, not a hardcoded string.
    // This is the counterpart of the depth-1 overview assertion above, for render_clusters_mermaid.
    #[test]
    fn test_render_clusters_custom_layer_name_appears_in_subgraph_not_hardcoded() {
        let style = minimal_style();
        let layer = LayerId::try_new("my_layer").unwrap();
        let baseline = make_baseline_with_struct("my_layer", "my_crate", None, "MyStruct");
        let clusters = render_clusters_mermaid(&[baseline], &layer, &style).unwrap();
        assert!(!clusters.is_empty(), "baseline with struct must produce at least one cluster");
        let content = &clusters[0].1;
        // The custom layer name must appear as the actual subgraph header label.
        // Use the exact header form `subgraph my_layer["my_layer"]` to distinguish the
        // subgraph label from entry node ids that also contain the layer name (CN-01).
        assert!(
            content.contains(r#"subgraph my_layer["my_layer"]"#),
            "depth-2 cluster must contain layer subgraph header 'subgraph my_layer[\"my_layer\"]'; content:\n{content}"
        );
        // Standard layer names must not appear as subgraph labels (layer-agnostic guarantee).
        for standard_name in ["domain", "usecase", "infrastructure"] {
            assert!(
                !content.contains(&format!("subgraph {standard_name}")),
                "depth-2 content must not hardcode standard name '{standard_name}'; content:\n{content}"
            );
        }
    }

    // T012: 2-layer render_clusters — each layer produces its own cluster files (CN-01 / AC-16).
    //
    // When baselines from two layers are provided, render_clusters for layer A must only produce
    // clusters from layer A's baselines (layer filter guarantee, symmetric to depth-1 overview).
    #[test]
    fn test_render_clusters_two_layer_configuration_layer_filter_correct() {
        let style = minimal_style();
        let baselines = [
            make_baseline_with_struct("domain", "domain", Some("review"), "Review"),
            make_baseline_with_struct("usecase", "usecase", Some("commands"), "CreateReview"),
        ];

        // Domain layer clusters must only contain domain-layer entries.
        let domain_layer = LayerId::try_new("domain").unwrap();
        let domain_clusters = render_clusters_mermaid(&baselines, &domain_layer, &style).unwrap();
        assert_eq!(
            domain_clusters.len(),
            1,
            "domain layer must produce exactly 1 cluster for the 2-layer fixture; got {}",
            domain_clusters.len()
        );
        for (key, content) in &domain_clusters {
            assert!(
                key.starts_with("domain"),
                "domain layer cluster key must start with 'domain'; got: {key}"
            );
            assert!(
                !content.contains("subgraph usecase"),
                "domain cluster content must not include usecase subgraph; key={key}"
            );
        }

        // Usecase layer clusters must only contain usecase-layer entries.
        let usecase_layer = LayerId::try_new("usecase").unwrap();
        let usecase_clusters = render_clusters_mermaid(&baselines, &usecase_layer, &style).unwrap();
        assert_eq!(
            usecase_clusters.len(),
            1,
            "usecase layer must produce exactly 1 cluster for the 2-layer fixture; got {}",
            usecase_clusters.len()
        );
        for (key, content) in &usecase_clusters {
            assert!(
                key.starts_with("usecase"),
                "usecase layer cluster key must start with 'usecase'; got: {key}"
            );
            assert!(
                !content.contains("subgraph domain"),
                "usecase cluster content must not include domain subgraph; key={key}"
            );
        }
    }

    // T012: 3-layer render_clusters — each layer has independent, non-overlapping cluster output.
    //
    // Verifies the isolation guarantee across 3 layers (AC-16 / CN-01).
    #[test]
    fn test_render_clusters_three_layer_configuration_all_layers_independent() {
        let style = minimal_style();
        let layer_strs = ["domain", "usecase", "infrastructure"];
        let baselines: Vec<BaselineDocument> = layer_strs
            .iter()
            .map(|&l| make_baseline_with_struct(l, l, Some("module_x"), "TypeX"))
            .collect();

        for &layer_str in &layer_strs {
            let layer = LayerId::try_new(layer_str).unwrap();
            let clusters = render_clusters_mermaid(&baselines, &layer, &style).unwrap();
            // Each layer must produce exactly 1 cluster (one baseline, one module).
            assert_eq!(
                clusters.len(),
                1,
                "3-layer: layer '{layer_str}' must produce exactly 1 cluster; got {}",
                clusters.len()
            );
            let (key, content) = &clusters[0];
            // Cluster key must belong to the current layer's crate.
            assert!(
                key.starts_with(layer_str),
                "3-layer: cluster key for '{layer_str}' must start with layer name; got: {key}"
            );
            // Content must contain the current layer name in the subgraph.
            assert!(
                content.contains(layer_str),
                "3-layer: cluster content for '{layer_str}' must contain its own name"
            );
            // Content must NOT contain other layer names as subgraph labels.
            for &other in &layer_strs {
                if other != layer_str {
                    assert!(
                        !content.contains(&format!("subgraph {other}")),
                        "3-layer: cluster for '{layer_str}' must not contain subgraph '{other}'"
                    );
                }
            }
        }
    }

    // T012: subgraph label non-hardcode property — layer name is dynamic, not compiled-in.
    //
    // Runs the same renderer with many different layer names (all arbitrary) and asserts
    // that (a) each layer name appears in its own output, and (b) none of the other layer
    // names appear in the output. This is the exhaustive form of the CN-01 guarantee.
    #[test]
    fn test_render_overview_layer_name_is_fully_dynamic_not_hardcoded() {
        let style = minimal_style();
        // Arbitrary layer names that are not the conventional architecture names.
        let layer_names = ["apple", "banana", "cherry", "date"];
        let baselines: Vec<BaselineDocument> =
            layer_names.iter().map(|&l| make_baseline_with_struct(l, l, None, "Fruit")).collect();

        for &layer_str in &layer_names {
            let layer = LayerId::try_new(layer_str).unwrap();
            let output = render_overview_mermaid(&baselines, &layer, &style).unwrap();

            // Must contain its own layer name.
            assert!(output.contains(layer_str), "layer name '{layer_str}' must appear in output");
            // Must NOT contain any other layer name's subgraph label.
            for &other in &layer_names {
                if other != layer_str {
                    assert!(
                        !output.contains(&format!("subgraph {other}")),
                        "output for '{layer_str}' must not contain subgraph '{other}'"
                    );
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // T017: Pass 3 — method-signature edge walk (AC-19 depth-1 path)
    //
    // Tests for collect_entry_edge_pairs Pass 3a (inherent methods) and
    // Pass 3b (trait methods). Verifies that method param/return types in
    // own-crate (or cross-crate within same layer) produce cross-cluster pairs,
    // and primitive/external types do not.
    // -----------------------------------------------------------------------

    // Helper: build a minimal rustdoc::Crate with a struct (in module_a) that has
    // an inherent method whose parameter is a type from another module (module_b).
    //
    // Layout:
    //   crate_name::module_a::Sender          (Struct)
    //   crate_name::module_a::Sender::send()  (inherent method, param: &module_b::Receiver)
    //   crate_name::module_b::Receiver        (Struct)
    //
    // This creates a cross-cluster dependency: module_a → module_b via method param.
    fn make_crate_with_inherent_method_param(crate_name: &str) -> rustdoc_types::Crate {
        use rustdoc_types::{
            Crate, FunctionHeader, FunctionSignature, Generics, Id, Impl, Item, ItemEnum, ItemKind,
            ItemSummary, Module, Path, Struct, StructKind, Target, Visibility,
        };

        let root_id = Id(0);
        let sender_id = Id(1);
        let receiver_id = Id(2);
        let impl_id = Id(3);
        let method_id = Id(4);

        // Receiver type as a ResolvedPath (the method param type).
        let receiver_path = rustdoc_types::Type::ResolvedPath(Path {
            path: format!("{crate_name}::module_b::Receiver"),
            id: receiver_id,
            args: None,
        });

        let mut index = HashMap::new();
        index.insert(
            root_id,
            Item {
                id: root_id,
                crate_id: 0,
                name: Some(crate_name.to_string()),
                span: None,
                visibility: Visibility::Public,
                docs: None,
                links: HashMap::new(),
                attrs: vec![],
                deprecation: None,
                inner: ItemEnum::Module(Module {
                    is_crate: true,
                    items: vec![sender_id, receiver_id],
                    is_stripped: false,
                }),
            },
        );
        // Sender struct in module_a.
        index.insert(
            sender_id,
            Item {
                id: sender_id,
                crate_id: 0,
                name: Some("Sender".to_string()),
                span: None,
                visibility: Visibility::Public,
                docs: None,
                links: HashMap::new(),
                attrs: vec![],
                deprecation: None,
                inner: ItemEnum::Struct(Struct {
                    kind: StructKind::Unit,
                    generics: Generics { params: vec![], where_predicates: vec![] },
                    impls: vec![impl_id],
                }),
            },
        );
        // Receiver struct in module_b.
        index.insert(
            receiver_id,
            Item {
                id: receiver_id,
                crate_id: 0,
                name: Some("Receiver".to_string()),
                span: None,
                visibility: Visibility::Public,
                docs: None,
                links: HashMap::new(),
                attrs: vec![],
                deprecation: None,
                inner: ItemEnum::Struct(Struct {
                    kind: StructKind::Unit,
                    generics: Generics { params: vec![], where_predicates: vec![] },
                    impls: vec![],
                }),
            },
        );
        // Inherent impl for Sender.
        let sender_type = rustdoc_types::Type::ResolvedPath(Path {
            path: format!("{crate_name}::module_a::Sender"),
            id: sender_id,
            args: None,
        });
        index.insert(
            impl_id,
            Item {
                id: impl_id,
                crate_id: 0,
                name: None,
                span: None,
                visibility: Visibility::Default,
                docs: None,
                links: HashMap::new(),
                attrs: vec![],
                deprecation: None,
                inner: ItemEnum::Impl(Impl {
                    is_unsafe: false,
                    generics: Generics { params: vec![], where_predicates: vec![] },
                    provided_trait_methods: vec![],
                    trait_: None,
                    for_: sender_type,
                    items: vec![method_id],
                    is_negative: false,
                    is_synthetic: false,
                    blanket_impl: None,
                }),
            },
        );
        // Method `send` with param: Receiver.
        index.insert(
            method_id,
            Item {
                id: method_id,
                crate_id: 0,
                name: Some("send".to_string()),
                span: None,
                visibility: Visibility::Public,
                docs: None,
                links: HashMap::new(),
                attrs: vec![],
                deprecation: None,
                inner: ItemEnum::Function(rustdoc_types::Function {
                    sig: FunctionSignature {
                        inputs: vec![("receiver".to_string(), receiver_path)],
                        output: None,
                        is_c_variadic: false,
                    },
                    generics: Generics { params: vec![], where_predicates: vec![] },
                    header: FunctionHeader {
                        is_unsafe: false,
                        is_const: false,
                        is_async: false,
                        abi: rustdoc_types::Abi::Rust,
                    },
                    has_body: true,
                }),
            },
        );

        let mut paths = HashMap::new();
        paths.insert(
            sender_id,
            ItemSummary {
                crate_id: 0,
                path: vec![crate_name.to_string(), "module_a".to_string(), "Sender".to_string()],
                kind: ItemKind::Struct,
            },
        );
        paths.insert(
            receiver_id,
            ItemSummary {
                crate_id: 0,
                path: vec![crate_name.to_string(), "module_b".to_string(), "Receiver".to_string()],
                kind: ItemKind::Struct,
            },
        );

        Crate {
            root: root_id,
            crate_version: None,
            includes_private: false,
            index,
            paths,
            external_crates: HashMap::new(),
            format_version: rustdoc_types::FORMAT_VERSION,
            target: Target { triple: String::new(), target_features: vec![] },
        }
    }

    // Helper: build a crate where Sender has an inherent method whose RETURN type is Receiver.
    fn make_crate_with_inherent_method_return(crate_name: &str) -> rustdoc_types::Crate {
        use rustdoc_types::{
            Crate, FunctionHeader, FunctionSignature, Generics, Id, Item, ItemEnum, ItemKind,
            ItemSummary, Module, Path, Struct, StructKind, Target, Visibility,
        };

        let root_id = Id(0);
        let sender_id = Id(1);
        let receiver_id = Id(2);
        let impl_id = Id(3);
        let method_id = Id(4);

        let receiver_path = rustdoc_types::Type::ResolvedPath(Path {
            path: format!("{crate_name}::module_b::Receiver"),
            id: receiver_id,
            args: None,
        });

        let mut index = HashMap::new();
        index.insert(
            root_id,
            Item {
                id: root_id,
                crate_id: 0,
                name: Some(crate_name.to_string()),
                span: None,
                visibility: Visibility::Public,
                docs: None,
                links: HashMap::new(),
                attrs: vec![],
                deprecation: None,
                inner: ItemEnum::Module(Module {
                    is_crate: true,
                    items: vec![sender_id, receiver_id],
                    is_stripped: false,
                }),
            },
        );
        index.insert(
            sender_id,
            Item {
                id: sender_id,
                crate_id: 0,
                name: Some("Sender".to_string()),
                span: None,
                visibility: Visibility::Public,
                docs: None,
                links: HashMap::new(),
                attrs: vec![],
                deprecation: None,
                inner: ItemEnum::Struct(Struct {
                    kind: StructKind::Unit,
                    generics: Generics { params: vec![], where_predicates: vec![] },
                    impls: vec![impl_id],
                }),
            },
        );
        index.insert(
            receiver_id,
            Item {
                id: receiver_id,
                crate_id: 0,
                name: Some("Receiver".to_string()),
                span: None,
                visibility: Visibility::Public,
                docs: None,
                links: HashMap::new(),
                attrs: vec![],
                deprecation: None,
                inner: ItemEnum::Struct(Struct {
                    kind: StructKind::Unit,
                    generics: Generics { params: vec![], where_predicates: vec![] },
                    impls: vec![],
                }),
            },
        );
        let sender_type = rustdoc_types::Type::ResolvedPath(Path {
            path: format!("{crate_name}::module_a::Sender"),
            id: sender_id,
            args: None,
        });
        // Inherent impl for Sender with method returning Receiver.
        index.insert(
            impl_id,
            Item {
                id: impl_id,
                crate_id: 0,
                name: None,
                span: None,
                visibility: Visibility::Default,
                docs: None,
                links: HashMap::new(),
                attrs: vec![],
                deprecation: None,
                inner: ItemEnum::Impl(rustdoc_types::Impl {
                    is_unsafe: false,
                    generics: Generics { params: vec![], where_predicates: vec![] },
                    provided_trait_methods: vec![],
                    trait_: None,
                    for_: sender_type,
                    items: vec![method_id],
                    is_negative: false,
                    is_synthetic: false,
                    blanket_impl: None,
                }),
            },
        );
        // Method `build` returns Receiver.
        index.insert(
            method_id,
            Item {
                id: method_id,
                crate_id: 0,
                name: Some("build".to_string()),
                span: None,
                visibility: Visibility::Public,
                docs: None,
                links: HashMap::new(),
                attrs: vec![],
                deprecation: None,
                inner: ItemEnum::Function(rustdoc_types::Function {
                    sig: FunctionSignature {
                        inputs: vec![],
                        output: Some(receiver_path),
                        is_c_variadic: false,
                    },
                    generics: Generics { params: vec![], where_predicates: vec![] },
                    header: FunctionHeader {
                        is_unsafe: false,
                        is_const: false,
                        is_async: false,
                        abi: rustdoc_types::Abi::Rust,
                    },
                    has_body: true,
                }),
            },
        );

        let mut paths = HashMap::new();
        paths.insert(
            sender_id,
            ItemSummary {
                crate_id: 0,
                path: vec![crate_name.to_string(), "module_a".to_string(), "Sender".to_string()],
                kind: ItemKind::Struct,
            },
        );
        paths.insert(
            receiver_id,
            ItemSummary {
                crate_id: 0,
                path: vec![crate_name.to_string(), "module_b".to_string(), "Receiver".to_string()],
                kind: ItemKind::Struct,
            },
        );

        Crate {
            root: root_id,
            crate_version: None,
            includes_private: false,
            index,
            paths,
            external_crates: HashMap::new(),
            format_version: rustdoc_types::FORMAT_VERSION,
            target: Target { triple: String::new(), target_features: vec![] },
        }
    }

    // Helper: build a crate with a Trait (in module_a) that has a method
    // whose param is a type from another module (module_b).
    //
    // Layout:
    //   crate_name::module_a::MyTrait          (Trait)
    //   crate_name::module_a::MyTrait::handle() (method, param: module_b::Event)
    //   crate_name::module_b::Event             (Struct)
    fn make_crate_with_trait_method_param(crate_name: &str) -> rustdoc_types::Crate {
        use rustdoc_types::{
            Crate, FunctionHeader, FunctionSignature, Generics, Id, Item, ItemEnum, ItemKind,
            ItemSummary, Module, Path, Struct, StructKind, Target, Trait, Visibility,
        };

        let root_id = Id(0);
        let trait_id = Id(1);
        let event_id = Id(2);
        let method_id = Id(3);

        let event_path = rustdoc_types::Type::ResolvedPath(Path {
            path: format!("{crate_name}::module_b::Event"),
            id: event_id,
            args: None,
        });

        let mut index = HashMap::new();
        index.insert(
            root_id,
            Item {
                id: root_id,
                crate_id: 0,
                name: Some(crate_name.to_string()),
                span: None,
                visibility: Visibility::Public,
                docs: None,
                links: HashMap::new(),
                attrs: vec![],
                deprecation: None,
                inner: ItemEnum::Module(Module {
                    is_crate: true,
                    items: vec![trait_id, event_id],
                    is_stripped: false,
                }),
            },
        );
        // Trait in module_a.
        index.insert(
            trait_id,
            Item {
                id: trait_id,
                crate_id: 0,
                name: Some("MyTrait".to_string()),
                span: None,
                visibility: Visibility::Public,
                docs: None,
                links: HashMap::new(),
                attrs: vec![],
                deprecation: None,
                inner: ItemEnum::Trait(Trait {
                    is_auto: false,
                    is_unsafe: false,
                    is_dyn_compatible: true,
                    items: vec![method_id],
                    generics: Generics { params: vec![], where_predicates: vec![] },
                    bounds: vec![],
                    implementations: vec![],
                }),
            },
        );
        // Event struct in module_b.
        index.insert(
            event_id,
            Item {
                id: event_id,
                crate_id: 0,
                name: Some("Event".to_string()),
                span: None,
                visibility: Visibility::Public,
                docs: None,
                links: HashMap::new(),
                attrs: vec![],
                deprecation: None,
                inner: ItemEnum::Struct(Struct {
                    kind: StructKind::Unit,
                    generics: Generics { params: vec![], where_predicates: vec![] },
                    impls: vec![],
                }),
            },
        );
        // Method `handle` with param: Event.
        index.insert(
            method_id,
            Item {
                id: method_id,
                crate_id: 0,
                name: Some("handle".to_string()),
                span: None,
                visibility: Visibility::Default, // trait method: Default visibility
                docs: None,
                links: HashMap::new(),
                attrs: vec![],
                deprecation: None,
                inner: ItemEnum::Function(rustdoc_types::Function {
                    sig: FunctionSignature {
                        inputs: vec![("event".to_string(), event_path)],
                        output: None,
                        is_c_variadic: false,
                    },
                    generics: Generics { params: vec![], where_predicates: vec![] },
                    header: FunctionHeader {
                        is_unsafe: false,
                        is_const: false,
                        is_async: false,
                        abi: rustdoc_types::Abi::Rust,
                    },
                    has_body: false,
                }),
            },
        );

        let mut paths = HashMap::new();
        paths.insert(
            trait_id,
            ItemSummary {
                crate_id: 0,
                path: vec![crate_name.to_string(), "module_a".to_string(), "MyTrait".to_string()],
                kind: ItemKind::Trait,
            },
        );
        paths.insert(
            event_id,
            ItemSummary {
                crate_id: 0,
                path: vec![crate_name.to_string(), "module_b".to_string(), "Event".to_string()],
                kind: ItemKind::Struct,
            },
        );

        Crate {
            root: root_id,
            crate_version: None,
            includes_private: false,
            index,
            paths,
            external_crates: HashMap::new(),
            format_version: rustdoc_types::FORMAT_VERSION,
            target: Target { triple: String::new(), target_features: vec![] },
        }
    }

    // T017 / AC-19 depth-1: inherent method with own-crate param produces cross-cluster pair.
    //
    // Sender (module_a) has an inherent method `send(receiver: Receiver)` where Receiver is
    // in module_b. Both are in the same layer. The pair (Sender rep, Receiver rep) must appear
    // in the pairs returned by collect_entry_edge_pairs.
    #[test]
    fn test_collect_entry_edge_pairs_inherent_method_param_produces_pair() {
        let krate = make_crate_with_inherent_method_param("domain");
        let baseline = BaselineDocument::new(
            domain::tddd::layer_id::LayerId::try_new("domain").unwrap(),
            CrateName::new("domain").unwrap(),
            krate,
        );

        let pairs = collect_entry_edge_pairs(&[baseline], "domain");

        // Sender rep node id (module_a).
        let sender_rep =
            node_id_generator::type_rep_node_id("domain", "domain", "module_a", "Sender");
        // Receiver rep node id (module_b).
        let receiver_rep =
            node_id_generator::type_rep_node_id("domain", "domain", "module_b", "Receiver");

        assert!(
            pairs.iter().any(|(src, dst)| src == &sender_rep && dst == &receiver_rep),
            "inherent method param must produce (Sender, Receiver) pair; pairs: {pairs:?}\nsender_rep={sender_rep}\nreceiver_rep={receiver_rep}"
        );
    }

    // T017 / AC-19 depth-1: inherent method with own-crate return produces cross-cluster pair.
    #[test]
    fn test_collect_entry_edge_pairs_inherent_method_return_produces_pair() {
        let krate = make_crate_with_inherent_method_return("domain");
        let baseline = BaselineDocument::new(
            domain::tddd::layer_id::LayerId::try_new("domain").unwrap(),
            CrateName::new("domain").unwrap(),
            krate,
        );

        let pairs = collect_entry_edge_pairs(&[baseline], "domain");

        let sender_rep =
            node_id_generator::type_rep_node_id("domain", "domain", "module_a", "Sender");
        let receiver_rep =
            node_id_generator::type_rep_node_id("domain", "domain", "module_b", "Receiver");

        assert!(
            pairs.iter().any(|(src, dst)| src == &sender_rep && dst == &receiver_rep),
            "inherent method return must produce (Sender, Receiver) pair; pairs: {pairs:?}"
        );
    }

    // T017 / AC-19 depth-1: trait method with own-crate param produces cross-cluster pair.
    //
    // MyTrait (module_a) has a method `handle(event: Event)` where Event is in module_b.
    // The pair (MyTrait rep, Event rep) must appear in the pairs.
    #[test]
    fn test_collect_entry_edge_pairs_trait_method_param_produces_pair() {
        let krate = make_crate_with_trait_method_param("domain");
        let baseline = BaselineDocument::new(
            domain::tddd::layer_id::LayerId::try_new("domain").unwrap(),
            CrateName::new("domain").unwrap(),
            krate,
        );

        let pairs = collect_entry_edge_pairs(&[baseline], "domain");

        let trait_rep =
            node_id_generator::trait_rep_node_id("domain", "domain", "module_a", "MyTrait");
        let event_rep =
            node_id_generator::type_rep_node_id("domain", "domain", "module_b", "Event");

        assert!(
            pairs.iter().any(|(src, dst)| src == &trait_rep && dst == &event_rep),
            "trait method param must produce (MyTrait, Event) pair; pairs: {pairs:?}\ntrait_rep={trait_rep}\nevent_rep={event_rep}"
        );
    }

    // T017 / AC-19 depth-1: inherent method with primitive param produces no pair.
    //
    // If the method param is a primitive type (u32, bool, etc.), no pair should be produced
    // by Pass 3 (AC-20: primitive / generic / external types produce no edge).
    #[test]
    fn test_collect_entry_edge_pairs_inherent_method_primitive_param_no_pair() {
        use rustdoc_types::{
            Crate, FunctionHeader, FunctionSignature, Generics, Id, Impl, Item, ItemEnum, ItemKind,
            ItemSummary, Module, Struct, StructKind, Target, Visibility,
        };

        let root_id = Id(0);
        let struct_id = Id(1);
        let impl_id = Id(2);
        let method_id = Id(3);

        // Method param is a primitive type (u32) — no pair should be produced.
        let prim_ty = rustdoc_types::Type::Primitive("u32".to_string());

        let mut index = HashMap::new();
        index.insert(
            root_id,
            Item {
                id: root_id,
                crate_id: 0,
                name: Some("domain".to_string()),
                span: None,
                visibility: Visibility::Public,
                docs: None,
                links: HashMap::new(),
                attrs: vec![],
                deprecation: None,
                inner: ItemEnum::Module(Module {
                    is_crate: true,
                    items: vec![struct_id],
                    is_stripped: false,
                }),
            },
        );
        index.insert(
            struct_id,
            Item {
                id: struct_id,
                crate_id: 0,
                name: Some("Counter".to_string()),
                span: None,
                visibility: Visibility::Public,
                docs: None,
                links: HashMap::new(),
                attrs: vec![],
                deprecation: None,
                inner: ItemEnum::Struct(Struct {
                    kind: StructKind::Unit,
                    generics: Generics { params: vec![], where_predicates: vec![] },
                    impls: vec![impl_id],
                }),
            },
        );
        let struct_ty = rustdoc_types::Type::ResolvedPath(rustdoc_types::Path {
            path: "domain::stats::Counter".to_string(),
            id: struct_id,
            args: None,
        });
        index.insert(
            impl_id,
            Item {
                id: impl_id,
                crate_id: 0,
                name: None,
                span: None,
                visibility: Visibility::Default,
                docs: None,
                links: HashMap::new(),
                attrs: vec![],
                deprecation: None,
                inner: ItemEnum::Impl(Impl {
                    is_unsafe: false,
                    generics: Generics { params: vec![], where_predicates: vec![] },
                    provided_trait_methods: vec![],
                    trait_: None,
                    for_: struct_ty,
                    items: vec![method_id],
                    is_negative: false,
                    is_synthetic: false,
                    blanket_impl: None,
                }),
            },
        );
        index.insert(
            method_id,
            Item {
                id: method_id,
                crate_id: 0,
                name: Some("increment".to_string()),
                span: None,
                visibility: Visibility::Public,
                docs: None,
                links: HashMap::new(),
                attrs: vec![],
                deprecation: None,
                inner: ItemEnum::Function(rustdoc_types::Function {
                    sig: FunctionSignature {
                        inputs: vec![("amount".to_string(), prim_ty)],
                        output: None,
                        is_c_variadic: false,
                    },
                    generics: Generics { params: vec![], where_predicates: vec![] },
                    header: FunctionHeader {
                        is_unsafe: false,
                        is_const: false,
                        is_async: false,
                        abi: rustdoc_types::Abi::Rust,
                    },
                    has_body: true,
                }),
            },
        );

        let mut paths = HashMap::new();
        paths.insert(
            struct_id,
            ItemSummary {
                crate_id: 0,
                path: vec!["domain".to_string(), "stats".to_string(), "Counter".to_string()],
                kind: ItemKind::Struct,
            },
        );

        let krate = Crate {
            root: root_id,
            crate_version: None,
            includes_private: false,
            index,
            paths,
            external_crates: HashMap::new(),
            format_version: rustdoc_types::FORMAT_VERSION,
            target: Target { triple: String::new(), target_features: vec![] },
        };

        let baseline = BaselineDocument::new(
            domain::tddd::layer_id::LayerId::try_new("domain").unwrap(),
            CrateName::new("domain").unwrap(),
            krate,
        );

        let pairs = collect_entry_edge_pairs(&[baseline], "domain");

        // Primitive param must not produce any pair from Pass 3.
        // (Pass 1 and Pass 2 also produce nothing for this fixture.)
        let counter_rep =
            node_id_generator::type_rep_node_id("domain", "domain", "stats", "Counter");
        // No pair where src = counter_rep AND dst is any prim_* node.
        let has_prim_pair = pairs.iter().any(|(src, dst)| {
            src == &counter_rep && (dst.starts_with("prim_") || dst.contains("u32"))
        });
        assert!(
            !has_prim_pair,
            "primitive param must not produce a pair; pairs from counter: {:?}",
            pairs.iter().filter(|(src, _)| src == &counter_rep).collect::<Vec<_>>()
        );
    }

    // T017 / AC-19 depth-1: inherent method with negative impl is skipped.
    //
    // is_negative: true → the Impl must be completely skipped in Pass 3a.
    #[test]
    fn test_collect_entry_edge_pairs_inherent_method_negative_impl_skipped() {
        use rustdoc_types::{
            Crate, FunctionHeader, FunctionSignature, Generics, Id, Impl, Item, ItemEnum, ItemKind,
            ItemSummary, Module, Path, Struct, StructKind, Target, Visibility,
        };

        let root_id = Id(0);
        let sender_id = Id(1);
        let receiver_id = Id(2);
        let impl_id = Id(3);
        let method_id = Id(4);

        let receiver_path = rustdoc_types::Type::ResolvedPath(Path {
            path: "domain::module_b::Receiver".to_string(),
            id: receiver_id,
            args: None,
        });
        let mut index = HashMap::new();
        index.insert(
            root_id,
            Item {
                id: root_id,
                crate_id: 0,
                name: Some("domain".to_string()),
                span: None,
                visibility: Visibility::Public,
                docs: None,
                links: HashMap::new(),
                attrs: vec![],
                deprecation: None,
                inner: ItemEnum::Module(Module {
                    is_crate: true,
                    items: vec![sender_id, receiver_id],
                    is_stripped: false,
                }),
            },
        );
        index.insert(
            sender_id,
            Item {
                id: sender_id,
                crate_id: 0,
                name: Some("Sender".to_string()),
                span: None,
                visibility: Visibility::Public,
                docs: None,
                links: HashMap::new(),
                attrs: vec![],
                deprecation: None,
                inner: ItemEnum::Struct(Struct {
                    kind: StructKind::Unit,
                    generics: Generics { params: vec![], where_predicates: vec![] },
                    impls: vec![impl_id],
                }),
            },
        );
        index.insert(
            receiver_id,
            Item {
                id: receiver_id,
                crate_id: 0,
                name: Some("Receiver".to_string()),
                span: None,
                visibility: Visibility::Public,
                docs: None,
                links: HashMap::new(),
                attrs: vec![],
                deprecation: None,
                inner: ItemEnum::Struct(Struct {
                    kind: StructKind::Unit,
                    generics: Generics { params: vec![], where_predicates: vec![] },
                    impls: vec![],
                }),
            },
        );
        let sender_type = rustdoc_types::Type::ResolvedPath(Path {
            path: "domain::module_a::Sender".to_string(),
            id: sender_id,
            args: None,
        });
        // is_negative: true → must be skipped.
        index.insert(
            impl_id,
            Item {
                id: impl_id,
                crate_id: 0,
                name: None,
                span: None,
                visibility: Visibility::Default,
                docs: None,
                links: HashMap::new(),
                attrs: vec![],
                deprecation: None,
                inner: ItemEnum::Impl(Impl {
                    is_unsafe: false,
                    generics: Generics { params: vec![], where_predicates: vec![] },
                    provided_trait_methods: vec![],
                    trait_: None,
                    for_: sender_type,
                    items: vec![method_id],
                    is_negative: true, // NEGATIVE — must skip
                    is_synthetic: false,
                    blanket_impl: None,
                }),
            },
        );
        index.insert(
            method_id,
            Item {
                id: method_id,
                crate_id: 0,
                name: Some("send".to_string()),
                span: None,
                visibility: Visibility::Public,
                docs: None,
                links: HashMap::new(),
                attrs: vec![],
                deprecation: None,
                inner: ItemEnum::Function(rustdoc_types::Function {
                    sig: FunctionSignature {
                        inputs: vec![("receiver".to_string(), receiver_path)],
                        output: None,
                        is_c_variadic: false,
                    },
                    generics: Generics { params: vec![], where_predicates: vec![] },
                    header: FunctionHeader {
                        is_unsafe: false,
                        is_const: false,
                        is_async: false,
                        abi: rustdoc_types::Abi::Rust,
                    },
                    has_body: true,
                }),
            },
        );

        let mut paths = HashMap::new();
        paths.insert(
            sender_id,
            ItemSummary {
                crate_id: 0,
                path: vec!["domain".to_string(), "module_a".to_string(), "Sender".to_string()],
                kind: ItemKind::Struct,
            },
        );
        paths.insert(
            receiver_id,
            ItemSummary {
                crate_id: 0,
                path: vec!["domain".to_string(), "module_b".to_string(), "Receiver".to_string()],
                kind: ItemKind::Struct,
            },
        );

        let krate = Crate {
            root: root_id,
            crate_version: None,
            includes_private: false,
            index,
            paths,
            external_crates: HashMap::new(),
            format_version: rustdoc_types::FORMAT_VERSION,
            target: Target { triple: String::new(), target_features: vec![] },
        };

        let baseline = BaselineDocument::new(
            domain::tddd::layer_id::LayerId::try_new("domain").unwrap(),
            CrateName::new("domain").unwrap(),
            krate,
        );

        let pairs = collect_entry_edge_pairs(&[baseline], "domain");

        // Negative impl must be completely skipped — no pair from Pass 3a.
        let sender_rep =
            node_id_generator::type_rep_node_id("domain", "domain", "module_a", "Sender");
        let receiver_rep =
            node_id_generator::type_rep_node_id("domain", "domain", "module_b", "Receiver");
        assert!(
            !pairs.iter().any(|(src, dst)| src == &sender_rep && dst == &receiver_rep),
            "negative impl must be skipped in Pass 3a; pairs: {pairs:?}"
        );
    }

    // T017 / AC-19 depth-1: method-signature cross-cluster edges appear in depth-1 overview.
    //
    // End-to-end test: Sender (module_a) has inherent method with param Receiver (module_b).
    // render_overview_mermaid must contain a cross-cluster edge module_a → module_b.
    #[test]
    fn test_render_overview_inherent_method_param_cross_cluster_edge_emitted() {
        let krate = make_crate_with_inherent_method_param("domain");
        let baseline = BaselineDocument::new(
            domain::tddd::layer_id::LayerId::try_new("domain").unwrap(),
            CrateName::new("domain").unwrap(),
            krate,
        );
        let layer = domain::tddd::layer_id::LayerId::try_new("domain").unwrap();
        let style = minimal_style();
        let output = render_overview_mermaid(&[baseline], &layer, &style).unwrap();

        // Extract mermaid body to avoid matching HTML comment arrows.
        let fence_open = "```mermaid\n";
        let fence_start = output.find(fence_open).unwrap() + fence_open.len();
        let fence_end = output[fence_start..]
            .find("\n```")
            .map(|i| fence_start + i + 1)
            .unwrap_or(output.len());
        let mermaid_body = &output[fence_start..fence_end];

        // Both cluster nodes must be present.
        assert!(
            mermaid_body.contains("domain_module_a"),
            "domain_module_a cluster node must appear; body:\n{mermaid_body}"
        );
        assert!(
            mermaid_body.contains("domain_module_b"),
            "domain_module_b cluster node must appear; body:\n{mermaid_body}"
        );
        // A cross-cluster edge must connect module_a → module_b.
        assert!(
            mermaid_body.contains("domain_module_a --> domain_module_b"),
            "cross-cluster edge domain_module_a --> domain_module_b must be present (method param); body:\n{mermaid_body}"
        );
    }

    // T017 / AC-19 depth-1: trait method param cross-cluster edge appears in depth-1 overview.
    #[test]
    fn test_render_overview_trait_method_param_cross_cluster_edge_emitted() {
        let krate = make_crate_with_trait_method_param("domain");
        let baseline = BaselineDocument::new(
            domain::tddd::layer_id::LayerId::try_new("domain").unwrap(),
            CrateName::new("domain").unwrap(),
            krate,
        );
        let layer = domain::tddd::layer_id::LayerId::try_new("domain").unwrap();
        let style = minimal_style();
        let output = render_overview_mermaid(&[baseline], &layer, &style).unwrap();

        let fence_open = "```mermaid\n";
        let fence_start = output.find(fence_open).unwrap() + fence_open.len();
        let fence_end = output[fence_start..]
            .find("\n```")
            .map(|i| fence_start + i + 1)
            .unwrap_or(output.len());
        let mermaid_body = &output[fence_start..fence_end];

        // Both clusters must appear.
        assert!(
            mermaid_body.contains("domain_module_a"),
            "domain_module_a cluster node must appear; body:\n{mermaid_body}"
        );
        assert!(
            mermaid_body.contains("domain_module_b"),
            "domain_module_b cluster node must appear; body:\n{mermaid_body}"
        );
        // Cross-cluster edge from module_a (where Trait lives) → module_b (where Event lives).
        assert!(
            mermaid_body.contains("domain_module_a --> domain_module_b"),
            "cross-cluster edge domain_module_a --> domain_module_b must be present (trait method param); body:\n{mermaid_body}"
        );
    }
}
