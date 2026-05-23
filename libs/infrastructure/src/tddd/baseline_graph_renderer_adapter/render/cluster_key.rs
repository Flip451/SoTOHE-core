//! Cluster key types and utilities for the depth-1 overview renderer (T009).
//!
//! Implements ADR U-r3 cluster enumeration: `(crate_name, top-level module)` collapsed
//! to 1 node. Alphabetical ordering is enforced via `BTreeMap` (CN-08).
//!
//! (IN-14 / IN-16 / AC-13 / CN-07 / CN-08)

use std::collections::BTreeMap;

use domain::tddd::baseline_document::BaselineDocument;

use super::node_extractor::{ExtractedNode, extract_nodes};
use super::node_id_generator::{
    function_node_id, module_path_from_summary, trait_node_id, type_node_id,
};
use super::style_config::sanitize;

// ---------------------------------------------------------------------------
// ClusterKey
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
pub(super) fn cluster_key_from_path(path: &[String]) -> Option<ClusterKey> {
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
pub(super) fn build_node_cluster_map(
    baselines: &[BaselineDocument],
    layer: &str,
) -> BTreeMap<String, ClusterKey> {
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
pub(super) fn lookup_cluster<'a>(
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
