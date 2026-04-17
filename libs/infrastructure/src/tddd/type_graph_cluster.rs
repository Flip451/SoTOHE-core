//! Cluster classification for TypeGraph mermaid rendering (T003).
//!
//! Phase 2 (ADR `knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md` §D4):
//! groups `TypeGraph` types into named clusters by `TypeNode::module_path`
//! prefix up to a configurable depth, and collects the edges that cross
//! cluster boundaries.
//!
//! The output [`ClusterPlan`] is consumed by the mermaid renderer
//! (`type_graph_render::render_type_graph_clustered` /
//! `render_type_graph_overview`) to emit per-cluster `subgraph` blocks and
//! a cluster-level overview respectively. The module is structured to also
//! support a future drift checker (ADR §D8) that re-uses the same
//! `cross_edges` data without round-tripping through rustdoc.
//!
//! ### Semantics of `depth`
//!
//! - `0` → no clustering (all types land in one cluster with an empty key).
//!   Matches ADR §D4 "クラスタなし、全ノードが 1 つのフラット図に入る".
//!   `cross_edges` is always empty at depth 0 (nothing can cross one cluster).
//! - `1` → group by the first `::` segment of `module_path`
//!   (e.g. `domain`, `usecase`, `infrastructure`).
//! - `N ≥ 2` → group by the first `N` segments
//!   (e.g. depth 2 yields `domain::review_v2`, `domain::spec`, …).
//!
//! Types whose `module_path` is `None` land in [`UNRESOLVED_CLUSTER`]
//! regardless of depth (provided `depth ≥ 1`).

use std::collections::HashMap;

use domain::schema::TypeGraph;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A cluster identifier derived from `TypeNode::module_path` (joined with `::`).
///
/// Crate-private type alias: `ClusterPlan` and `CrossEdge` expose the
/// underlying `String` type publicly to avoid leaking the alias through
/// rustdoc (which would require a dedicated catalogue entry for a purely
/// cosmetic shortcut). The alias remains useful for readability inside
/// this module and its siblings.
pub(crate) type ClusterKey = String;

/// Fallback cluster key for types whose `module_path` is `None` or empty.
/// A dedicated sentinel makes the "unresolved" bucket visually distinguishable
/// from empty-string clusters (e.g. depth 0's single flat cluster).
pub const UNRESOLVED_CLUSTER: &str = "unresolved";

/// Assignment of all types in a `TypeGraph` to named clusters, plus the
/// edges that cross cluster boundaries.
///
/// The renderer and drift checker both consume this structure without
/// re-parsing the source `TypeGraph`.
#[derive(Debug, Clone)]
pub struct ClusterPlan {
    /// The prefix depth used to compute cluster keys.
    pub depth: usize,
    /// Maps each cluster key to the sorted list of short type names it contains.
    pub assignments: HashMap<ClusterKey, Vec<String>>,
    /// Directed edges whose source and target live in different clusters.
    pub cross_edges: Vec<CrossEdge>,
}

/// A directed edge spanning cluster boundaries.
///
/// `edge_kind` is a `&'static str` tag (`"method"` / `"field"` / `"impl"`)
/// matching the edge-set taxonomy in `type_graph_render::EdgeSet`. Keeping
/// the tag static-string rather than an enum lets the renderer pass edge
/// data through without borrowing a full mermaid-specific enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossEdge {
    pub source_type: String,
    pub source_cluster: ClusterKey,
    pub target_type: String,
    pub target_cluster: ClusterKey,
    /// Edge label (method name, field name, or trait name).
    pub label: String,
    /// Edge kind tag: `"method"` | `"field"` | `"impl"`.
    pub edge_kind: &'static str,
}

// ---------------------------------------------------------------------------
// Public API: classify_types
// ---------------------------------------------------------------------------

/// Assigns every type in `graph` to a cluster based on `module_path`
/// prefix up to `depth` segments, and collects the edges that cross
/// cluster boundaries.
///
/// Pure function: no I/O, deterministic output (assignments sorted within
/// each cluster, cross_edges in input order with cross-cluster filter).
///
/// # Arguments
///
/// * `graph` — pre-indexed `TypeGraph` whose `type_names()` enumerate the
///   node set to classify.
/// * `depth` — number of `::` segments to use as cluster key. See
///   [module-level documentation](self) for depth semantics.
/// * `edges` — directed edges as `(source_type, label, target_type, edge_kind)`.
///   Edges referencing types absent from `graph` are treated as pointing
///   into [`UNRESOLVED_CLUSTER`] (not silently dropped).
#[must_use]
pub fn classify_types(
    graph: &TypeGraph,
    depth: usize,
    edges: &[(String, String, String, &'static str)],
) -> ClusterPlan {
    // Assign every known type to a cluster key.
    let mut assignments: HashMap<ClusterKey, Vec<String>> = HashMap::new();
    for type_name in graph.type_names() {
        let cluster = cluster_key_for_type(graph, type_name, depth);
        assignments.entry(cluster).or_default().push(type_name.clone());
    }

    // Deterministic order within each cluster.
    for types in assignments.values_mut() {
        types.sort();
    }

    // Reverse lookup: short type name → cluster key (borrowed).
    let type_to_cluster: HashMap<&str, &str> = assignments
        .iter()
        .flat_map(|(cluster, types)| types.iter().map(move |t| (t.as_str(), cluster.as_str())))
        .collect();

    // At depth 0 there is exactly one cluster (empty-string key), so no edge can
    // cross cluster boundaries — including edges whose endpoints are absent from
    // the graph (which would otherwise map to UNRESOLVED_CLUSTER and produce a
    // spurious cross-edge against the single flat cluster).
    //
    // Ensure the single flat cluster key ("") is always present, even when the
    // graph has zero types. Callers that rely on `depth == 0` producing exactly
    // one cluster would otherwise see an empty map for an empty graph.
    if depth == 0 {
        assignments.entry(String::new()).or_default();
        return ClusterPlan { depth, assignments, cross_edges: Vec::new() };
    }

    // Collect cross-cluster edges in input order.
    let mut cross_edges = Vec::new();
    for (source_type, label, target_type, edge_kind) in edges {
        let source_cluster =
            type_to_cluster.get(source_type.as_str()).copied().unwrap_or(UNRESOLVED_CLUSTER);
        let target_cluster =
            type_to_cluster.get(target_type.as_str()).copied().unwrap_or(UNRESOLVED_CLUSTER);
        if source_cluster != target_cluster {
            cross_edges.push(CrossEdge {
                source_type: source_type.clone(),
                source_cluster: source_cluster.to_owned(),
                target_type: target_type.clone(),
                target_cluster: target_cluster.to_owned(),
                label: label.clone(),
                edge_kind,
            });
        }
    }

    ClusterPlan { depth, assignments, cross_edges }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns the cluster key for a single type under the given depth setting.
///
/// - `depth == 0` → empty-string key (single flat cluster per ADR §D4).
/// - `depth ≥ 1` → first `depth` `::`-joined segments of `module_path`, or
///   [`UNRESOLVED_CLUSTER`] when `module_path` is `None` or empty.
fn cluster_key_for_type(graph: &TypeGraph, type_name: &str, depth: usize) -> ClusterKey {
    if depth == 0 {
        return String::new();
    }
    let Some(node) = graph.get_type(type_name) else {
        return UNRESOLVED_CLUSTER.to_owned();
    };
    let Some(path) = node.module_path() else {
        return UNRESOLVED_CLUSTER.to_owned();
    };
    if path.is_empty() {
        return UNRESOLVED_CLUSTER.to_owned();
    }
    path.split("::").take(depth).collect::<Vec<_>>().join("::")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use std::collections::HashSet;

    use domain::schema::{TypeKind, TypeNode};

    use super::*;

    fn node_with_module(path: Option<&str>) -> TypeNode {
        let mut node = TypeNode::new(TypeKind::Struct, vec![], vec![], HashSet::new());
        if let Some(p) = path {
            node.set_module_path(p.to_owned());
        }
        node
    }

    fn graph_with(types: &[(&str, Option<&str>)]) -> TypeGraph {
        let mut map = HashMap::new();
        for (name, path) in types {
            map.insert((*name).to_owned(), node_with_module(*path));
        }
        TypeGraph::new(map, HashMap::new())
    }

    // --- depth classification ---

    #[test]
    fn test_classify_types_depth_zero_returns_single_cluster() {
        let graph = graph_with(&[
            ("Alpha", Some("domain::review")),
            ("Beta", Some("usecase::merge")),
            ("Gamma", None),
        ]);
        let plan = classify_types(&graph, 0, &[]);

        assert_eq!(plan.depth, 0);
        assert_eq!(plan.assignments.len(), 1, "depth=0 must produce a single cluster");
        let single = plan.assignments.get("").expect("depth=0 key is empty string");
        assert_eq!(single.len(), 3);
        // Sorted within the cluster.
        assert_eq!(single, &vec!["Alpha".to_owned(), "Beta".to_owned(), "Gamma".to_owned()]);
    }

    #[test]
    fn test_classify_types_depth_zero_cross_edges_always_empty() {
        // Even when edges reference types absent from the graph (which would
        // map to UNRESOLVED_CLUSTER at depth >= 1), depth=0 must produce no
        // cross_edges because there is only one cluster.
        let graph =
            graph_with(&[("Alpha", Some("domain::review")), ("Beta", Some("usecase::merge"))]);
        let edges = vec![
            // intra-graph edge
            ("Alpha".to_owned(), "call".to_owned(), "Beta".to_owned(), "method"),
            // edge referencing a type absent from the graph
            ("Alpha".to_owned(), "call".to_owned(), "Missing".to_owned(), "method"),
        ];
        let plan = classify_types(&graph, 0, &edges);

        assert!(
            plan.cross_edges.is_empty(),
            "depth=0 must produce no cross_edges even with out-of-graph edge targets"
        );
    }

    #[test]
    fn test_classify_types_depth_zero_empty_graph_still_yields_single_cluster() {
        // An empty TypeGraph at depth=0 must still yield exactly one cluster
        // with an empty-string key (containing no types), not an empty map.
        let graph = graph_with(&[]);
        let plan = classify_types(&graph, 0, &[]);

        assert_eq!(plan.depth, 0);
        assert_eq!(
            plan.assignments.len(),
            1,
            "depth=0 must produce exactly 1 cluster even for empty graph"
        );
        let single = plan.assignments.get("").expect("depth=0 key is empty string");
        assert!(single.is_empty(), "empty graph has no types in the cluster");
        assert!(plan.cross_edges.is_empty());
    }

    #[test]
    fn test_classify_types_depth_one_groups_by_top_segment() {
        let graph = graph_with(&[
            ("A", Some("domain::review")),
            ("B", Some("domain::spec")),
            ("C", Some("usecase::merge")),
        ]);
        let plan = classify_types(&graph, 1, &[]);

        assert_eq!(plan.depth, 1);
        assert_eq!(plan.assignments.len(), 2, "expected domain + usecase clusters");
        assert_eq!(plan.assignments.get("domain").unwrap(), &vec!["A".to_owned(), "B".to_owned()]);
        assert_eq!(plan.assignments.get("usecase").unwrap(), &vec!["C".to_owned()]);
    }

    #[test]
    fn test_classify_types_depth_two_groups_by_two_segments() {
        let graph = graph_with(&[
            ("A", Some("domain::review_v2::types")),
            ("B", Some("domain::review_v2::error")),
            ("C", Some("domain::spec")),
        ]);
        let plan = classify_types(&graph, 2, &[]);

        assert_eq!(plan.assignments.len(), 2);
        assert_eq!(
            plan.assignments.get("domain::review_v2").unwrap(),
            &vec!["A".to_owned(), "B".to_owned()]
        );
        assert_eq!(plan.assignments.get("domain::spec").unwrap(), &vec!["C".to_owned()]);
    }

    #[test]
    fn test_classify_types_none_module_path_goes_to_unresolved() {
        let graph = graph_with(&[
            ("Resolved", Some("domain::review")),
            ("Orphan", None),
            ("Empty", Some("")),
        ]);
        let plan = classify_types(&graph, 1, &[]);

        let unresolved = plan.assignments.get(UNRESOLVED_CLUSTER).unwrap();
        assert_eq!(unresolved, &vec!["Empty".to_owned(), "Orphan".to_owned()]);
        assert_eq!(plan.assignments.get("domain").unwrap(), &vec!["Resolved".to_owned()]);
    }

    // --- cross_edges detection ---

    #[test]
    fn test_cross_edges_detected_for_inter_cluster_method_edges() {
        let graph = graph_with(&[
            ("Reader", Some("domain::review")),
            ("Store", Some("infrastructure::fs")),
        ]);
        let edges = vec![("Store".to_owned(), "read".to_owned(), "Reader".to_owned(), "method")];
        let plan = classify_types(&graph, 1, &edges);

        assert_eq!(plan.cross_edges.len(), 1);
        let edge = &plan.cross_edges[0];
        assert_eq!(edge.source_type, "Store");
        assert_eq!(edge.source_cluster, "infrastructure");
        assert_eq!(edge.target_type, "Reader");
        assert_eq!(edge.target_cluster, "domain");
        assert_eq!(edge.label, "read");
        assert_eq!(edge.edge_kind, "method");
    }

    #[test]
    fn test_cross_edges_empty_when_all_types_in_same_cluster() {
        let graph = graph_with(&[("A", Some("domain::review")), ("B", Some("domain::review"))]);
        let edges = vec![("A".to_owned(), "publish".to_owned(), "B".to_owned(), "method")];
        let plan = classify_types(&graph, 1, &edges);

        assert!(plan.cross_edges.is_empty(), "intra-cluster edges must be excluded");
    }

    #[test]
    fn test_cross_edges_absent_type_maps_to_unresolved_at_depth_one() {
        // Verifies the documented contract: "Edges referencing types absent from
        // `graph` are treated as pointing into UNRESOLVED_CLUSTER (not silently
        // dropped)" — under depth >= 1 (the early depth-0 return does not apply here).
        let graph = graph_with(&[("Known", Some("domain::review"))]);
        let edges = vec![
            // target type "Missing" is not in the graph
            ("Known".to_owned(), "call".to_owned(), "Missing".to_owned(), "method"),
        ];
        let plan = classify_types(&graph, 1, &edges);

        assert_eq!(plan.cross_edges.len(), 1, "absent target must produce a cross-edge");
        let edge = &plan.cross_edges[0];
        assert_eq!(edge.source_type, "Known");
        assert_eq!(edge.source_cluster, "domain");
        assert_eq!(edge.target_type, "Missing");
        assert_eq!(edge.target_cluster, UNRESOLVED_CLUSTER);
    }
}
