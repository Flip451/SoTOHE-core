//! Mermaid type graph renderer — generates flowchart visualizations from `TypeGraph`.
//!
//! Phase 1 (ADR `knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md` §D7):
//! flat rendering with method edges only. Produces a markdown file containing a
//! fenced `mermaid` block with `flowchart LR`.
//!
//! Phase 2 (ADR §D4, §D9, Scope K):
//! cluster directory layout — `write_type_graph_dir` produces an `index.md`
//! (overview) plus one `<cluster>.md` per cluster. Cross-cluster references are
//! rendered as `[→ other::Type]` ghost labels so each sub-diagram stays
//! self-contained. Stale flat files are removed when writing cluster dirs and
//! vice versa. Symlink rejection follows `knowledge/conventions/security.md
//! §Symlink Rejection in Infrastructure Adapters`.
//!
//! Types as nodes:
//! - struct → `[Name]` (rectangle) with `structNode` class
//! - enum → `{{Name}}` (hexagon) with `enumNode` class
//! - trait → `([Name])` (stadium) with `traitNode` class (only when impl edges present)
//!
//! Edge types (all controlled by `EdgeSet`):
//! - Methods: `A -->|method_name| B`  solid arrow; inherent self-receiver methods
//! - Fields:  `A ---|field_name| B`   solid no-arrow; struct fields whose type is in-graph
//! - Impls:   `A -.->|impl| Trait`    dashed arrow; trait implementations
//!
//! **Known limitations**:
//! - Associated type binding labels (e.g. `Item` in `Iterator<Item = Foo>`) are
//!   extracted as PascalCase tokens. If a workspace type coincidentally shares the
//!   label name, a false edge may appear. Label-aware filtering is deferred.
//! - Stdlib wrapper names (`Result`, `Option`, `Vec` …) are NOT explicitly
//!   filtered — they are naturally excluded because `TypeGraph` only contains
//!   types from the workspace crate's rustdoc export, not stdlib re-exports.
//! - Enum variant payloads are not extracted at L1 (variant members carry name only).

use std::collections::HashSet;
use std::path::Path;

use domain::schema::{TypeGraph, TypeKind};

use crate::tddd::type_graph_cluster::{ClusterPlan, classify_types};
use crate::track::atomic_write::atomic_write_file;
use crate::track::symlink_guard::reject_symlinks_below;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Selects which edge types to include in the mermaid type graph render.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeSet {
    /// Only inherent method edges (self → return type).
    ///
    /// Mermaid symbol: `A -->|method_name| B` (solid arrow).
    Methods,
    /// Only struct field edges.
    ///
    /// Each `MemberDeclaration::Field` whose type string contains a PascalCase
    /// token that exists in the graph produces an edge. Enum variant members
    /// carry no payload type at L1 and are skipped.
    ///
    /// Mermaid symbol: `A ---|field_name| B` (solid, no arrowhead).
    Fields,
    /// Only trait impl edges.
    ///
    /// Each `TraitImplEntry` on a type produces an edge to the trait node when
    /// the trait name appears in `TypeGraph::trait_names()`. Trait nodes are
    /// rendered with stadium shape `([TraitName])` and `traitNode` class.
    ///
    /// Mermaid symbol: `A -.->|impl| TraitName` (dashed arrow).
    Impls,
    /// All edge types: methods + fields + impls (union, no deduplication).
    All,
}

/// Configuration options for the type graph mermaid renderer.
#[derive(Debug, Clone)]
pub struct TypeGraphRenderOptions {
    /// Edge types to include in the diagram.
    pub edge_set: EdgeSet,
    /// Maximum nodes per diagram. Types beyond this limit are omitted with a
    /// truncation note.
    pub max_nodes_per_diagram: usize,
    /// Cluster depth for directory layout.  `0` → single flat file via
    /// `write_type_graph_file`.  `≥ 1` → cluster directory layout via
    /// `write_type_graph_dir`.  Default is `2` (group by first two
    /// `module_path` segments, e.g. `domain::review_v2`).
    pub cluster_depth: usize,
}

impl Default for TypeGraphRenderOptions {
    fn default() -> Self {
        Self { edge_set: EdgeSet::Methods, max_nodes_per_diagram: 50, cluster_depth: 2 }
    }
}

// ---------------------------------------------------------------------------
// Render function
// ---------------------------------------------------------------------------

/// Renders a flat (non-clustered) mermaid type graph from a `TypeGraph`.
///
/// Returns a markdown string with a `Generated from` header and a fenced
/// mermaid `flowchart LR` block. Only types with at least one edge are
/// included as nodes to keep the diagram readable.
///
/// # Arguments
///
/// * `graph` — pre-indexed `TypeGraph` from `build_type_graph`
/// * `layer_name` — layer identifier for the header (e.g. `"domain"`)
/// * `opts` — render configuration
#[must_use]
pub fn render_type_graph_flat(
    graph: &TypeGraph,
    layer_name: &str,
    opts: &TypeGraphRenderOptions,
) -> String {
    // Collect edges with their kind tags so we can choose the correct mermaid
    // edge symbol in the render step.
    let raw_edges = collect_edges(graph, opts.edge_set);
    let mut edges: Vec<(String, String, String, &'static str)> = raw_edges;

    // Apply max_nodes guard by limiting edges first, then deriving nodes.
    // This ensures every rendered node participates in at least one rendered
    // edge, avoiding isolated nodes that appear when nodes are truncated
    // without regard for edge connectivity.
    //
    // Mermaid node IDs: type nodes use the bare short name; trait nodes use
    // `_trait_{name}` to avoid collision with a type that shares the same
    // short name (Rust type and trait namespaces are separate, but Mermaid
    // IDs must be globally unique within a diagram).  The budget guard must
    // count these as distinct nodes even when the short name is the same.
    //
    // Build the initial set of Mermaid node IDs present before truncation.
    // Type nodes keep their short name; impl-edge targets use `_trait_{name}`.
    let total_nodes_connected = {
        let mut set: HashSet<String> = HashSet::new();
        for (src, _, tgt, kind) in &edges {
            set.insert(src.clone()); // sources are always type nodes
            if *kind == "impl" {
                set.insert(trait_node_id(tgt));
            } else {
                set.insert(tgt.clone());
            }
        }
        set.len()
    };
    let truncated = total_nodes_connected > opts.max_nodes_per_diagram;
    if truncated {
        // Keep only edges whose both endpoints fit within the node budget.
        // Greedily accept edges (already sorted) until the node set is full.
        // Node IDs use the same disambiguation as in the renderer.
        let mut kept_ids: HashSet<String> = HashSet::new();
        edges.retain(|(src, _, tgt, kind)| {
            let src_id = src.clone();
            let tgt_id = if *kind == "impl" { trait_node_id(tgt) } else { tgt.clone() };
            let src_new = !kept_ids.contains(&src_id);
            let tgt_new = !kept_ids.contains(&tgt_id);
            let would_add = src_new as usize + tgt_new as usize;
            if kept_ids.len() + would_add <= opts.max_nodes_per_diagram {
                kept_ids.insert(src_id);
                kept_ids.insert(tgt_id);
                true
            } else {
                false
            }
        });
    }

    // Collect connected node names from the (possibly truncated) edge set.
    // Separate struct/enum type nodes from trait nodes (impl edge targets).
    let (type_node_names, trait_node_names): (Vec<&str>, Vec<&str>) = {
        let mut type_set: HashSet<&str> = HashSet::new();
        let mut trait_set: HashSet<&str> = HashSet::new();
        for (src, _, tgt, kind) in &edges {
            type_set.insert(src.as_str());
            if *kind == "impl" {
                // Trait nodes are rendered with stadium shape — keep them separate.
                trait_set.insert(tgt.as_str());
            } else {
                type_set.insert(tgt.as_str());
            }
        }
        let mut type_names: Vec<&str> = type_set.into_iter().collect();
        type_names.sort();
        let mut trait_names: Vec<&str> = trait_set.into_iter().collect();
        trait_names.sort();
        (type_names, trait_names)
    };
    // node_names is for header reporting. Count by unique Mermaid node IDs
    // (type nodes use short name, trait nodes use `_trait_{name}`) so that
    // a type and trait sharing a short name are counted as two separate nodes.
    let node_count = {
        let mut id_set: HashSet<String> = HashSet::new();
        for name in &type_node_names {
            id_set.insert((*name).to_owned());
        }
        for name in &trait_node_names {
            id_set.insert(trait_node_id(name));
        }
        id_set.len()
    };
    // node_set: membership test for edge filtering (uses short names for both
    // type and trait short names, since the edge list uses short names for tgt).
    let node_set: HashSet<&str> = {
        let mut s: HashSet<&str> = HashSet::new();
        for name in &type_node_names {
            s.insert(*name);
        }
        for name in &trait_node_names {
            s.insert(*name);
        }
        s
    };
    let has_trait_nodes = !trait_node_names.is_empty();

    // Build output
    let mut out = String::new();
    out.push_str(&format!(
        "<!-- Generated from {layer_name} TypeGraph — DO NOT EDIT DIRECTLY -->\n"
    ));
    out.push_str(&format!("# {layer_name} Type Graph\n\n"));

    let total_types = graph.type_names().count();
    // Report connected type nodes only (trait nodes from impl edges are a separate
    // namespace and would make the `connected` count exceed `total` if included).
    let connected_types = type_node_names.len();
    out.push_str(&format!(
        "Types: {total_types} total, {connected_types} connected, {} edges",
        edges.len()
    ));
    if truncated {
        out.push_str(&format!(" (truncated to {} nodes)", opts.max_nodes_per_diagram));
    }
    out.push_str("\n\n");

    out.push_str("```mermaid\nflowchart LR\n");
    out.push_str("    classDef structNode fill:#f3e5f5,stroke:#7b1fa2\n");
    out.push_str("    classDef enumNode fill:#e1f5fe,stroke:#0288d1\n");
    if has_trait_nodes {
        out.push_str("    classDef traitNode fill:#e8f5e9,stroke:#388e3c\n");
    }
    out.push('\n');

    // Emit type (struct/enum) nodes
    for name in &type_node_names {
        if let Some(node) = graph.get_type(name) {
            let shape = match node.kind() {
                TypeKind::Enum => format!("    {name}{{{{{name}}}}}:::{}", "enumNode"),
                _ => format!("    {name}[{name}]:::{}", "structNode"),
            };
            out.push_str(&shape);
            out.push('\n');
        }
    }
    // Emit trait nodes with disambiguated ID `_trait_{name}` and stadium shape.
    // The `_trait_` prefix avoids Mermaid node ID collisions with type nodes
    // that share the same short name (trait and type namespaces are separate in
    // Rust but Mermaid IDs must be globally unique within a diagram).
    for name in &trait_node_names {
        let node_id = trait_node_id(name);
        out.push_str(&format!("    {node_id}([{name}]):::traitNode\n"));
    }

    if node_count > 0 && !edges.is_empty() {
        out.push('\n');
    }

    // Emit edges (only between nodes in the node_set), choosing the mermaid
    // edge symbol based on the edge_kind.
    for (src, label, tgt, kind) in &edges {
        if node_set.contains(src.as_str()) && node_set.contains(tgt.as_str()) {
            let edge_str = render_edge_symbol(src, label, tgt, kind);
            out.push_str(&format!("    {edge_str}\n"));
        }
    }

    out.push_str("```\n");
    out
}

// ---------------------------------------------------------------------------
// Write helper (symlink-checked) — flat mode
// ---------------------------------------------------------------------------

/// Renders a mermaid type graph and writes it to `<layer_id>-graph.md` inside
/// `track_dir`, with symlink protection relative to `trusted_root`.
///
/// Combines `render_type_graph_flat` + `reject_symlinks_below` + `atomic_write_file`
/// so that the symlink guard stays in the infrastructure layer (not CLI).
///
/// When a `<layer_id>-graph/` directory exists at the target location this
/// function removes it first (stale cluster-dir cleanup) after checking for
/// symlinks per `knowledge/conventions/security.md §Symlink Rejection in
/// Infrastructure Adapters`.
///
/// # Errors
///
/// Returns `std::io::Error` if `layer_id` contains unsafe path characters (path
/// separators `/` or `\`, `:`, or `..`), if the symlink guard rejects the output
/// path, or if the atomic write fails.
pub fn write_type_graph_file(
    graph: &TypeGraph,
    layer_id: &str,
    track_dir: &Path,
    trusted_root: &Path,
    opts: &TypeGraphRenderOptions,
) -> Result<String, std::io::Error> {
    // Validate layer_id to prevent path traversal.  Uses the same rules as
    // `is_safe_path_component` in `verify::tddd_layers`, plus a bare `:` check
    // to prevent Windows drive-relative paths (e.g. `C:escape` → `C:escape-graph.md`
    // which Path::join resolves relative to the drive root, not track_dir).
    validate_layer_id(layer_id)?;

    let rendered = render_type_graph_flat(graph, layer_id, opts);

    let graph_filename = format!("{layer_id}-graph.md");
    let graph_path = track_dir.join(&graph_filename);

    // Stale cluster-dir cleanup: if a directory exists at `<layer_id>-graph/`
    // from a previous cluster-mode run, remove it before writing the flat file.
    // Double guard: (1) call reject_symlinks_below on the directory path itself
    // so broken symlinks (which `exists()` does not detect) are caught, and
    // (2) recursively scan directory contents for symlinks before calling
    // remove_dir_all, so no symlinked child can escape the removal boundary.
    let stale_dir = track_dir.join(format!("{layer_id}-graph"));
    if reject_symlinks_below(&stale_dir, trusted_root)? && stale_dir.is_dir() {
        reject_symlinks_recursively(&stale_dir)?;
        std::fs::remove_dir_all(&stale_dir)?;
    }

    reject_symlinks_below(&graph_path, trusted_root)?;
    atomic_write_file(&graph_path, rendered.as_bytes())?;

    Ok(graph_filename)
}

// ---------------------------------------------------------------------------
// Cluster render functions
// ---------------------------------------------------------------------------

/// Builds a flat edge list from a `TypeGraph` for the given `EdgeSet`.
///
/// Returns `(source, label, target, edge_kind)` tuples. Deduplicates and sorts.
/// Edge kinds:
/// - `"method"`: inherent method with self-receiver (solid arrow `-->|label|`)
/// - `"field"`: struct field whose type is a workspace type (solid no-arrow `---|label|`)
/// - `"impl"`: trait implementation (dashed arrow `-.->|impl|`)
fn collect_edges(
    graph: &TypeGraph,
    edge_set: EdgeSet,
) -> Vec<(String, String, String, &'static str)> {
    let graph_type_names: HashSet<&str> = graph.type_names().map(|s| s.as_str()).collect();
    let graph_trait_names: HashSet<&str> = graph.trait_names().map(|s| s.as_str()).collect();
    let mut edges: Vec<(String, String, String, &'static str)> = Vec::new();

    if matches!(edge_set, EdgeSet::Methods | EdgeSet::All) {
        for source_name in graph.type_names() {
            if let Some(node) = graph.get_type(source_name) {
                for method in node.methods() {
                    if method.receiver().is_none() {
                        continue;
                    }
                    let targets = extract_type_names(method.returns());
                    for target in targets {
                        if graph_type_names.contains(target) && target != source_name.as_str() {
                            edges.push((
                                source_name.clone(),
                                method.name().to_string(),
                                target.to_string(),
                                "method",
                            ));
                        }
                    }
                }
            }
        }
    }

    if matches!(edge_set, EdgeSet::Fields | EdgeSet::All) {
        use domain::tddd::catalogue::MemberDeclaration;
        for source_name in graph.type_names() {
            if let Some(node) = graph.get_type(source_name) {
                for member in node.members() {
                    match member {
                        MemberDeclaration::Field { name: field_name, ty } => {
                            // Extract PascalCase type tokens from the field type string.
                            // Only include edges to types that exist in the graph
                            // (same workspace-only filter as Methods).
                            // Note: self-referential field edges (e.g. `Node { next: Node }`)
                            // are intentionally kept — unlike method self-loops, a recursive
                            // struct field is meaningful structural information to show.
                            let targets = extract_type_names(ty);
                            for target in targets {
                                if graph_type_names.contains(target) {
                                    edges.push((
                                        source_name.clone(),
                                        field_name.clone(),
                                        target.to_string(),
                                        "field",
                                    ));
                                }
                            }
                        }
                        // Variants carry no payload type at L1 — skip them.
                        MemberDeclaration::Variant(_) => {}
                    }
                }
            }
        }
    }

    if matches!(edge_set, EdgeSet::Impls | EdgeSet::All) {
        for source_name in graph.type_names() {
            if let Some(node) = graph.get_type(source_name) {
                for trait_impl in node.trait_impls() {
                    let trait_name = trait_impl.trait_name();
                    // Only emit edges to traits that exist in the graph's trait map.
                    if graph_trait_names.contains(trait_name) {
                        edges.push((
                            source_name.clone(),
                            "impl".to_string(),
                            trait_name.to_string(),
                            "impl",
                        ));
                    }
                }
            }
        }
    }

    edges.sort();
    edges.dedup();
    edges
}

/// Renders a per-cluster mermaid block for a single cluster.
///
/// Intra-cluster edges are rendered with the appropriate symbol for their
/// edge kind: solid arrow for methods, solid no-arrow for fields, dashed
/// arrow for impl edges. Trait nodes from impl edges are rendered with
/// stadium shape `([TraitName])` within the cluster diagram.
/// Cross-cluster references appear as ghost nodes with label `[→ other::Type]`
/// so the diagram remains self-contained.
///
/// # Arguments
///
/// * `graph` — pre-indexed `TypeGraph`
/// * `cluster_key` — the cluster key to render (must exist in `plan.assignments`)
/// * `plan` — `ClusterPlan` from `classify_types`
/// * `opts` — render configuration
#[must_use]
pub fn render_type_graph_clustered(
    graph: &TypeGraph,
    cluster_key: &str,
    plan: &ClusterPlan,
    opts: &TypeGraphRenderOptions,
) -> String {
    let cluster_types: HashSet<&str> = plan
        .assignments
        .get(cluster_key)
        .map(|v| v.iter().map(|s| s.as_str()).collect())
        .unwrap_or_default();

    // Build full edge list from the graph.
    let all_edges = collect_edges(graph, opts.edge_set);

    // Separate intra-cluster edges (both endpoints in this cluster) from
    // cross-cluster references (source in this cluster, target elsewhere).
    // For impl edges, the target is a trait node (not in cluster_types), so
    // impl-edge targets are collected separately as intra-cluster trait nodes.
    // Tuple: (source, label, target, edge_kind)
    let mut intra_edges: Vec<(&str, &str, &str, &str)> = Vec::new();
    let mut intra_trait_targets: Vec<&str> = Vec::new(); // trait node targets within this cluster
    let mut cross_targets: Vec<(&str, &str, &str, &str)> = Vec::new(); // (source, label, target_type, edge_kind)

    for (src, label, tgt, kind) in &all_edges {
        if !cluster_types.contains(src.as_str()) {
            continue;
        }
        if *kind == "impl" {
            // Impl edges always point to traits — render within the cluster.
            // Trait targets are not in cluster_types so we track them separately.
            intra_edges.push((src.as_str(), label.as_str(), tgt.as_str(), kind));
            intra_trait_targets.push(tgt.as_str());
        } else if cluster_types.contains(tgt.as_str()) {
            intra_edges.push((src.as_str(), label.as_str(), tgt.as_str(), kind));
        } else {
            cross_targets.push((src.as_str(), label.as_str(), tgt.as_str(), kind));
        }
    }
    intra_trait_targets.sort();
    intra_trait_targets.dedup();

    // Collect cluster member nodes and apply max_nodes_per_diagram guard.
    // Member nodes, trait nodes, and cross-cluster ghost nodes all share a
    // single budget: member nodes take priority, then trait nodes fill the
    // next slots (in deduped order), and ghost nodes use whatever remains.
    // This keeps the total mermaid node count at or below max_nodes_per_diagram.
    let mut sorted_nodes: Vec<&str> = cluster_types.iter().copied().collect();
    sorted_nodes.sort();
    let mut truncated = sorted_nodes.len() > opts.max_nodes_per_diagram;
    if truncated {
        sorted_nodes.truncate(opts.max_nodes_per_diagram);
    }
    let kept_nodes: HashSet<&str> = sorted_nodes.iter().copied().collect();

    // After truncation, filter intra_edges and cross_targets to kept_nodes only.
    intra_edges.retain(|(src, _, tgt, kind)| {
        if *kind == "impl" {
            // Impl edge source must be in kept_nodes; trait targets are always kept.
            kept_nodes.contains(*src)
        } else {
            kept_nodes.contains(*src) && kept_nodes.contains(*tgt)
        }
    });
    cross_targets.retain(|(src, _, _, _)| kept_nodes.contains(*src));

    // Rebuild intra_trait_targets from retained impl edges so the list only
    // contains traits referenced by the kept (non-truncated) member nodes.
    // Then cap the list to fit within the remaining budget after member nodes
    // so that the total rendered nodes never exceeds max_nodes_per_diagram.
    let trait_budget = opts.max_nodes_per_diagram.saturating_sub(sorted_nodes.len());
    let mut intra_trait_targets: Vec<&str> = {
        let mut v: Vec<&str> = intra_edges
            .iter()
            .filter(|(_, _, _, kind)| *kind == "impl")
            .map(|(_, _, tgt, _)| *tgt)
            .collect();
        v.sort();
        v.dedup();
        v
    };
    let trait_truncated = intra_trait_targets.len() > trait_budget;
    if trait_truncated {
        // Remove impl edges whose trait target will be dropped so the diagram
        // does not emit edges pointing at non-existent nodes.
        intra_trait_targets.truncate(trait_budget);
        let kept_traits: HashSet<&str> = intra_trait_targets.iter().copied().collect();
        intra_edges.retain(
            |(_, _, tgt, kind)| {
                if *kind == "impl" { kept_traits.contains(*tgt) } else { true }
            },
        );
    }
    let has_trait_nodes = !intra_trait_targets.is_empty();
    truncated |= trait_truncated;

    // Build a type → cluster reverse lookup so ghost labels can carry the
    // target's cluster prefix (e.g. `→ usecase::publish::Draft`).
    let type_to_cluster: std::collections::HashMap<&str, &str> = plan
        .assignments
        .iter()
        .flat_map(|(cluster, types)| types.iter().map(move |t| (t.as_str(), cluster.as_str())))
        .collect();

    // Include cross-cluster targets as ghost nodes.
    // Ghost node IDs use the `_xref_` prefix, which cannot collide with real
    // type node IDs because real types are PascalCase while `_xref_` starts
    // with an underscore.
    // The display label is `→ {cluster}::{type}` matching the spec's
    // `[→ other::Type]` convention (cluster context visible at a glance).
    // The ghost_id incorporates the sanitized cluster key to remain unique
    // when the same short type name appears in multiple clusters.
    // Tuple: (ghost_id, source, label, target, display, edge_kind)
    let cross_ghost_ids: Vec<(String, &str, &str, &str, String, &str)> = cross_targets
        .iter()
        .map(|(src, label, tgt, kind)| {
            let tgt_cluster = type_to_cluster.get(*tgt).copied().unwrap_or("unresolved");
            let sanitized_cluster = sanitize_cluster_id(tgt_cluster);
            let ghost_id = format!("_xref_{sanitized_cluster}_{tgt}");
            let display = format!("{tgt_cluster}::{tgt}");
            (ghost_id, *src, *label, *tgt, display, *kind)
        })
        .collect();

    // Count unique ghost node IDs (multiple cross-cluster edges may reference
    // the same target type and therefore share one ghost node).  The budget
    // is applied to unique nodes so that duplicate references do not consume
    // extra slots.
    let unique_ghost_count = {
        let mut seen: HashSet<&str> = HashSet::new();
        for (ghost_id, ..) in &cross_ghost_ids {
            seen.insert(ghost_id.as_str());
        }
        seen.len()
    };
    // Ghost nodes share the same budget as member and trait nodes.  After
    // member nodes and trait nodes are placed, ghost nodes may use the
    // remaining budget only (counted as unique ghost nodes, not raw edge
    // references).
    let ghost_budget = opts
        .max_nodes_per_diagram
        .saturating_sub(sorted_nodes.len())
        .saturating_sub(intra_trait_targets.len());
    let ghost_truncated = unique_ghost_count > ghost_budget;

    // When truncation is needed, keep only edges whose ghost node falls within
    // the budget.  Accept ghost nodes in first-seen order; edges to a ghost
    // node that is already accepted are always retained (they contribute an
    // edge in the diagram, not an extra node).
    let cross_ghost_ids: Vec<(String, &str, &str, &str, String, &str)> = if ghost_truncated {
        let mut accepted_ghosts: HashSet<String> = HashSet::new();
        cross_ghost_ids
            .into_iter()
            .filter(|(ghost_id, ..)| {
                if accepted_ghosts.contains(ghost_id.as_str()) {
                    // Already accepted ghost — keep the edge.
                    true
                } else if accepted_ghosts.len() < ghost_budget {
                    // New ghost that still fits in the budget.
                    accepted_ghosts.insert(ghost_id.clone());
                    true
                } else {
                    // Budget exhausted for new ghost nodes.
                    false
                }
            })
            .collect()
    } else {
        cross_ghost_ids
    };
    truncated |= ghost_truncated;

    let display_label = if cluster_key.is_empty() { "flat" } else { cluster_key };

    let mut out = String::new();
    out.push_str(&format!(
        "<!-- Generated from {display_label} cluster TypeGraph — DO NOT EDIT DIRECTLY -->\n"
    ));
    out.push_str(&format!("# {display_label} Type Graph\n\n"));

    let total_types = cluster_types.len();
    out.push_str(&format!(
        "Types: {total_types} in cluster, {} intra-cluster edges",
        intra_edges.len()
    ));
    if !cross_ghost_ids.is_empty() {
        out.push_str(&format!(", {} cross-cluster references", cross_ghost_ids.len()));
    }
    if truncated {
        out.push_str(&format!(" (truncated to {} nodes)", opts.max_nodes_per_diagram));
    }
    out.push_str("\n\n");

    out.push_str("```mermaid\nflowchart LR\n");
    out.push_str("    classDef structNode fill:#f3e5f5,stroke:#7b1fa2\n");
    out.push_str("    classDef enumNode fill:#e1f5fe,stroke:#0288d1\n");
    out.push_str("    classDef ghostNode fill:#f5f5f5,stroke:#9e9e9e,color:#757575\n");
    if has_trait_nodes {
        out.push_str("    classDef traitNode fill:#e8f5e9,stroke:#388e3c\n");
    }
    out.push('\n');

    // Emit cluster-member nodes.
    for name in &sorted_nodes {
        if let Some(node) = graph.get_type(name) {
            let shape = match node.kind() {
                TypeKind::Enum => format!("    {name}{{{{{name}}}}}:::{}", "enumNode"),
                _ => format!("    {name}[{name}]:::{}", "structNode"),
            };
            out.push_str(&shape);
            out.push('\n');
        }
    }

    // Emit trait nodes with disambiguated ID `_trait_{name}` and stadium shape.
    // See `trait_node_id` for the rationale behind the prefix.
    for name in &intra_trait_targets {
        let node_id = trait_node_id(name);
        out.push_str(&format!("    {node_id}([{name}]):::traitNode\n"));
    }

    // Emit ghost nodes for cross-cluster targets.
    // The display string carries the cluster prefix: `→ cluster::TypeName`.
    let mut ghost_ids_emitted: HashSet<String> = HashSet::new();
    for (ghost_id, _src, _label, _tgt, display, _kind) in &cross_ghost_ids {
        if ghost_ids_emitted.insert(ghost_id.clone()) {
            out.push_str(&format!("    {ghost_id}[\"→ {display}\"]:::ghostNode\n"));
        }
    }

    if !intra_edges.is_empty() || !cross_ghost_ids.is_empty() {
        out.push('\n');
    }

    // Emit intra-cluster edges using the correct mermaid symbol per edge_kind.
    for (src, label, tgt, kind) in &intra_edges {
        let edge_str = render_edge_symbol(src, label, tgt, kind);
        out.push_str(&format!("    {edge_str}\n"));
    }

    // Emit cross-cluster ghost edges.
    for (ghost_id, src, label, _tgt, _display, kind) in &cross_ghost_ids {
        let edge_str = render_edge_symbol(src, label, ghost_id, kind);
        out.push_str(&format!("    {edge_str}\n"));
    }

    out.push_str("```\n");
    out
}

/// Renders a cluster-to-cluster overview diagram.
///
/// Each cluster appears as a single node. Edges are drawn between cluster
/// nodes when at least one cross-cluster edge exists between the two clusters.
///
/// # Arguments
///
/// * `graph` — pre-indexed `TypeGraph` (used for edge collection)
/// * `plan` — `ClusterPlan` from `classify_types`
/// * `opts` — render configuration
#[must_use]
pub fn render_type_graph_overview(
    graph: &TypeGraph,
    plan: &ClusterPlan,
    opts: &TypeGraphRenderOptions,
) -> String {
    // Collect cluster names (sorted for determinism).
    let mut cluster_names: Vec<&str> = plan.assignments.keys().map(|s| s.as_str()).collect();
    cluster_names.sort();

    // Build cross-cluster edge set: (source_cluster, label_count, target_cluster).
    // We collapse multiple edges into one per (src_cluster, tgt_cluster) pair.
    // Impl-kind edges are excluded here because trait targets are not cluster
    // members — they are rendered as intra-cluster nodes within each cluster
    // diagram.  Including impl cross-edges would add a spurious `unresolved`
    // cluster node to the overview for every trait implementation.
    let mut overview_edges: std::collections::BTreeMap<(String, String), usize> =
        std::collections::BTreeMap::new();
    for edge in &plan.cross_edges {
        if edge.edge_kind == "impl" {
            continue; // trait impl targets rendered intra-cluster, not as cross-cluster
        }
        let key = (edge.source_cluster.clone(), edge.target_cluster.clone());
        *overview_edges.entry(key).or_default() += 1;
    }

    // If the plan has no cross_edges recorded, compute them from the graph.
    // (This handles the case where classify_types was called without edges.)
    // Impl-kind edges are excluded for the same reason as above.
    let computed_edges: Vec<(String, String, String, &'static str)>;
    let has_computed = plan.cross_edges.is_empty();
    if has_computed {
        computed_edges = collect_edges(graph, opts.edge_set);
        // Build a type → cluster lookup from plan.
        let type_to_cluster: std::collections::HashMap<&str, &str> = plan
            .assignments
            .iter()
            .flat_map(|(cluster, types)| types.iter().map(move |t| (t.as_str(), cluster.as_str())))
            .collect();
        for (src, _label, tgt, kind) in &computed_edges {
            if *kind == "impl" {
                continue; // trait impl targets rendered intra-cluster
            }
            let src_cluster = type_to_cluster.get(src.as_str()).copied().unwrap_or("unresolved");
            let tgt_cluster = type_to_cluster.get(tgt.as_str()).copied().unwrap_or("unresolved");
            if src_cluster != tgt_cluster {
                let key = (src_cluster.to_owned(), tgt_cluster.to_owned());
                *overview_edges.entry(key).or_default() += 1;
            }
        }
    }

    // Also include any clusters that appear only as edge endpoints (e.g. the
    // `UNRESOLVED_CLUSTER` sentinel for types absent from the graph).
    // These clusters are not in `assignments` but must be emitted as mermaid
    // nodes so that edges referencing them are valid.
    let known_clusters: std::collections::HashSet<&str> = cluster_names.iter().copied().collect();
    let mut extra_clusters: Vec<String> = Vec::new();
    for (src_cluster, tgt_cluster) in overview_edges.keys() {
        if !known_clusters.contains(src_cluster.as_str()) {
            extra_clusters.push(src_cluster.clone());
        }
        if !known_clusters.contains(tgt_cluster.as_str()) {
            extra_clusters.push(tgt_cluster.clone());
        }
    }
    extra_clusters.sort();
    extra_clusters.dedup();

    // Apply max_nodes_per_diagram guard to the overview cluster node set.
    // Truncate cluster_names first (sorted deterministically), then extra_clusters.
    let total_before = cluster_names.len() + extra_clusters.len();
    let overview_truncated = total_before > opts.max_nodes_per_diagram;
    if overview_truncated {
        if cluster_names.len() >= opts.max_nodes_per_diagram {
            cluster_names.truncate(opts.max_nodes_per_diagram);
            extra_clusters.clear();
        } else {
            let remaining = opts.max_nodes_per_diagram - cluster_names.len();
            extra_clusters.truncate(remaining);
        }
        // Retain only edges whose both endpoints are in the kept node set.
        let kept: std::collections::HashSet<&str> = cluster_names
            .iter()
            .copied()
            .chain(extra_clusters.iter().map(|s| s.as_str()))
            .collect();
        overview_edges
            .retain(|(src, tgt), _| kept.contains(src.as_str()) && kept.contains(tgt.as_str()));
    }
    let all_cluster_count = cluster_names.len() + extra_clusters.len();

    let mut out = String::new();
    out.push_str("<!-- Generated from cluster overview TypeGraph — DO NOT EDIT DIRECTLY -->\n");
    out.push_str("# Type Graph Overview\n\n");
    out.push_str(&format!(
        "Clusters: {all_cluster_count}, cross-cluster edge groups: {}",
        overview_edges.len()
    ));
    if overview_truncated {
        out.push_str(&format!(" (truncated to {} nodes)", opts.max_nodes_per_diagram));
    }
    out.push_str("\n\n");

    out.push_str("```mermaid\nflowchart LR\n");
    out.push_str("    classDef clusterNode fill:#e8f5e9,stroke:#388e3c\n\n");

    // Emit cluster nodes. Sanitize cluster keys for mermaid IDs.
    for name in &cluster_names {
        let node_id = sanitize_cluster_id(name);
        let display = if name.is_empty() { "flat" } else { name };
        out.push_str(&format!("    {node_id}[\"{display}\"]:::clusterNode\n"));
    }
    // Emit phantom cluster nodes (only appear as cross-edge endpoints).
    for name in &extra_clusters {
        let node_id = sanitize_cluster_id(name);
        out.push_str(&format!("    {node_id}[\"{name}\"]:::clusterNode\n"));
    }

    if !overview_edges.is_empty() {
        out.push('\n');
    }

    // Emit cluster-to-cluster edges.
    for ((src_cluster, tgt_cluster), count) in &overview_edges {
        let src_id = sanitize_cluster_id(src_cluster);
        let tgt_id = sanitize_cluster_id(tgt_cluster);
        out.push_str(&format!("    {src_id} -->|\"{count} edges\"| {tgt_id}\n"));
    }

    out.push_str("```\n");
    out
}

// ---------------------------------------------------------------------------
// Write helper (symlink-checked) — cluster directory mode
// ---------------------------------------------------------------------------

/// Renders a cluster directory layout and writes files under
/// `<track_dir>/<layer_id>-graph/`.
///
/// Produces:
/// - `<layer_id>-graph/index.md` — cluster overview (`render_type_graph_overview`)
/// - `<layer_id>-graph/<cluster>.md` — per-cluster diagrams
///   (`render_type_graph_clustered`), one per cluster key in `plan`
///
/// Cluster filename: `module_path` with `::` replaced by `_`
/// (e.g. `domain::review_v2` → `domain_review_v2.md`).
///
/// Stale flat-file cleanup: if `<layer_id>-graph.md` exists as a regular file
/// it is removed before writing (previous flat-mode output).  Double guard:
/// `reject_symlinks_below` is called on that path first.
///
/// The `cluster_depth` in `opts` must be `>= 1`; call `write_type_graph_file`
/// for depth 0.
///
/// Returns the list of written paths relative to `track_dir`.
///
/// # Errors
///
/// Returns `std::io::Error` if `opts.cluster_depth == 0` (use
/// `write_type_graph_file` for depth 0), if `layer_id` is unsafe, a symlink
/// guard fires, directory creation fails, or any write fails.
pub fn write_type_graph_dir(
    graph: &TypeGraph,
    layer_id: &str,
    track_dir: &Path,
    trusted_root: &Path,
    opts: &TypeGraphRenderOptions,
) -> Result<Vec<String>, std::io::Error> {
    // Enforce the documented precondition: cluster_depth must be >= 1.
    // Use write_type_graph_file for depth 0 (flat mode).
    if opts.cluster_depth == 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "write_type_graph_dir requires cluster_depth >= 1; use write_type_graph_file for depth 0",
        ));
    }

    validate_layer_id(layer_id)?;

    // Stale flat-file cleanup: remove `<layer_id>-graph.md` if it is a plain file.
    // Double guard: call reject_symlinks_below unconditionally so broken symlinks
    // (which `exists()` does not detect) are still caught and fail-closed.
    let stale_flat = track_dir.join(format!("{layer_id}-graph.md"));
    if reject_symlinks_below(&stale_flat, trusted_root)? && stale_flat.is_file() {
        std::fs::remove_file(&stale_flat)?;
    }

    // Collect edges for clustering.
    let edges = collect_edges(graph, opts.edge_set);
    let plan = classify_types(graph, opts.cluster_depth, &edges);

    // Target directory: `<layer_id>-graph/`.
    let graph_dir = track_dir.join(format!("{layer_id}-graph"));
    // Guard the directory path itself before creating it.
    reject_symlinks_below(&graph_dir, trusted_root)?;
    std::fs::create_dir_all(&graph_dir)?;

    let mut written: Vec<String> = Vec::new();

    // Build the set of expected cluster filenames (excluding index.md) so stale
    // cluster files from a previous run with different cluster membership can be
    // removed.  Double guard: reject symlinks before removing each stale file.
    let expected_cluster_files: HashSet<String> =
        plan.assignments.keys().map(|k| cluster_key_to_filename(k.as_str())).collect();

    // Stale cluster-file cleanup: scan the directory for `.md` files that are
    // neither `index.md` nor in the current cluster set and remove them.
    //
    // Note: `<layer_id>-graph/` is a fully tool-managed output directory.
    // All `.md` files in it (except `index.md`) are generated by
    // `cluster_key_to_filename` and are subject to stale removal when the
    // cluster membership changes.  Do not place manually-maintained files
    // inside a `<layer_id>-graph/` directory.
    if graph_dir.is_dir() {
        let read_dir = std::fs::read_dir(&graph_dir)?;
        for entry in read_dir {
            let entry = entry?;
            let fname = entry.file_name();
            let fname_str = fname.to_string_lossy();
            if fname_str == "index.md" {
                continue;
            }
            if fname_str.ends_with(".md") && !expected_cluster_files.contains(fname_str.as_ref()) {
                let stale_cluster = entry.path();
                reject_symlinks_below(&stale_cluster, trusted_root)?;
                std::fs::remove_file(&stale_cluster)?;
            }
        }
    }

    // Write overview index.md.
    let index_content = render_type_graph_overview(graph, &plan, opts);
    let index_path = graph_dir.join("index.md");
    reject_symlinks_below(&index_path, trusted_root)?;
    atomic_write_file(&index_path, index_content.as_bytes())?;
    written.push(format!("{layer_id}-graph/index.md"));

    // Write per-cluster files.
    let mut cluster_keys: Vec<&str> = plan.assignments.keys().map(|s| s.as_str()).collect();
    cluster_keys.sort();

    for cluster_key in cluster_keys {
        let filename = cluster_key_to_filename(cluster_key);
        let cluster_path = graph_dir.join(&filename);
        reject_symlinks_below(&cluster_path, trusted_root)?;

        let content = render_type_graph_clustered(graph, cluster_key, &plan, opts);
        atomic_write_file(&cluster_path, content.as_bytes())?;
        written.push(format!("{layer_id}-graph/{filename}"));
    }

    Ok(written)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Recursively scans a directory for symlinks and fails closed if any are found.
///
/// Used before `remove_dir_all` to enforce the invariant that stale cluster
/// directory removal never follows symlinks into directories outside the
/// `<layer_id>-graph/` subtree.
fn reject_symlinks_recursively(dir: &Path) -> Result<(), std::io::Error> {
    let read_dir = std::fs::read_dir(dir)?;
    for entry in read_dir {
        let entry = entry?;
        let entry_path = entry.path();
        let meta = entry_path.symlink_metadata()?;
        if meta.file_type().is_symlink() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "refusing to remove directory containing symlink: {}",
                    entry_path.display()
                ),
            ));
        }
        if meta.file_type().is_dir() {
            reject_symlinks_recursively(&entry_path)?;
        }
    }
    Ok(())
}

/// Validates `layer_id` to prevent path traversal.
///
/// Rejects empty strings, path separators `/` or `\`, `:`, and `..`.
fn validate_layer_id(layer_id: &str) -> Result<(), std::io::Error> {
    if layer_id.is_empty()
        || layer_id.contains('/')
        || layer_id.contains('\\')
        || layer_id.contains(':')
        || layer_id == ".."
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("layer_id contains unsafe path characters: {layer_id:?}"),
        ));
    }
    Ok(())
}

/// Converts a cluster key to a safe filename.
///
/// Replaces `::` with `_` (e.g. `domain::review_v2` → `domain_review_v2.md`).
/// No other characters need escaping because `module_path` only contains
/// `[A-Za-z0-9_:]`. Empty cluster key (depth-0 flat cluster) becomes `flat.md`.
///
/// **Known limitation**: the mapping is not injective when a module path
/// component ends with `_` adjacent to a `::` boundary (e.g. `foo_bar::baz`
/// and `foo::bar_baz` both produce `foo_bar_baz.md`). This degenerate case
/// does not arise in practice because Rust module identifiers conventionally
/// use lowercase snake_case without trailing underscores.
fn cluster_key_to_filename(cluster_key: &str) -> String {
    if cluster_key.is_empty() {
        "flat.md".to_owned()
    } else {
        format!("{}.md", cluster_key.replace("::", "_"))
    }
}

/// Sanitizes a cluster key into a valid mermaid node ID (alphanumeric + `_`).
///
/// Replaces `::` with `__` and uses `_flat_` for the empty key.
/// Mermaid node IDs cannot contain `:`, so `::` must be replaced with
/// a safe character sequence. Using `__` keeps the ID human-readable.
///
/// **Known limitation**: `a::b` and `a__b` both sanitize to `a__b`. This
/// degenerate case does not arise in practice because Rust module identifiers
/// do not contain `__` by convention.
fn sanitize_cluster_id(cluster_key: &str) -> String {
    if cluster_key.is_empty() { "_flat_".to_owned() } else { cluster_key.replace("::", "__") }
}

/// Returns the disambiguated Mermaid node ID for a trait node.
///
/// Prefixes the trait name with `_trait_` to avoid collision with type nodes
/// that share the same short name.  `TypeGraph` stores types and traits
/// separately, so a workspace can legitimately have both a struct `Foo` and a
/// trait `Foo`; using bare names for both would collapse them into one Mermaid
/// node.
///
/// The `_trait_` prefix starts with an underscore, which is never a valid
/// leading character for a Rust type name (PascalCase), guaranteeing no
/// collision with real type node IDs.
fn trait_node_id(name: &str) -> String {
    format!("_trait_{name}")
}

/// Selects the mermaid edge symbol for the given `edge_kind`.
///
/// - `"method"` → `A -->|label| B` (solid arrow — method call relationship)
/// - `"field"` → `A ---|label| B` (solid, no arrowhead — containment/composition)
/// - `"impl"` → `A -.->|label| _trait_B` (dashed arrow — interface implementation;
///   trait node ID uses `_trait_` prefix to avoid collision with type nodes)
/// - any other kind falls back to the method (solid arrow) symbol.
fn render_edge_symbol(src: &str, label: &str, tgt: &str, kind: &str) -> String {
    match kind {
        "field" => format!("{src} ---|{label}| {tgt}"),
        "impl" => {
            let tgt_id = trait_node_id(tgt);
            format!("{src} -.->|{label}| {tgt_id}")
        }
        _ => format!("{src} -->|{label}| {tgt}"),
    }
}

/// Extracts PascalCase type names from a type string.
///
/// Splits on non-alphanumeric/underscore characters and keeps tokens that
/// start with an uppercase letter. Used to find potential type references
/// in return type strings like `"Result<Option<User>, DomainError>"`.
///
/// Visibility: `pub(crate)` so other modules inside the `infrastructure`
/// crate (for example `catalogue_bulk_loader` / future contract-map edge
/// extraction) can reuse this helper without re-implementing the
/// tokenisation. The function is kept inside `type_graph_render` because
/// that is where the tokenisation rules originally evolved.
pub(crate) fn extract_type_names(ty: &str) -> Vec<&str> {
    ty.split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|s| !s.is_empty())
        .filter(|s| s.starts_with(char::is_uppercase))
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use std::collections::{HashMap, HashSet};

    use domain::schema::{TypeGraph, TypeKind, TypeNode};
    use domain::tddd::catalogue::{MemberDeclaration, MethodDeclaration};
    use tempfile::TempDir;

    use super::*;

    fn method_returning(name: &str, returns: &str) -> MethodDeclaration {
        MethodDeclaration::new(name, Some("&self".into()), vec![], returns, false)
    }

    fn struct_node(methods: Vec<MethodDeclaration>) -> TypeNode {
        TypeNode::new(TypeKind::Struct, vec![], methods, HashSet::new())
    }

    fn enum_node() -> TypeNode {
        TypeNode::new(
            TypeKind::Enum,
            vec![MemberDeclaration::variant("A"), MemberDeclaration::variant("B")],
            vec![],
            HashSet::new(),
        )
    }

    // --- extract_type_names ---

    #[test]
    fn test_extract_type_names_from_simple_type() {
        assert_eq!(extract_type_names("User"), vec!["User"]);
    }

    #[test]
    fn test_extract_type_names_from_result_option() {
        let names = extract_type_names("Result<Option<User>, DomainError>");
        assert_eq!(names, vec!["Result", "Option", "User", "DomainError"]);
    }

    #[test]
    fn test_extract_type_names_from_unit_returns_empty() {
        let names = extract_type_names("()");
        assert!(names.is_empty());
    }

    #[test]
    fn test_extract_type_names_skips_lowercase_generics() {
        let names = extract_type_names("Vec<str>");
        assert_eq!(names, vec!["Vec"]);
    }

    // --- render_type_graph_flat ---

    #[test]
    fn test_render_empty_graph_contains_mermaid_block() {
        let graph = TypeGraph::new(HashMap::new(), HashMap::new());
        let opts = TypeGraphRenderOptions::default();
        let output = render_type_graph_flat(&graph, "domain", &opts);

        assert!(output.contains("```mermaid"));
        assert!(output.contains("flowchart LR"));
        assert!(output.contains("```\n"));
        assert!(output.contains("Generated from domain TypeGraph"));
        assert!(output.contains("Types: 0 total"));
    }

    #[test]
    fn test_render_single_method_edge() {
        let mut types = HashMap::new();
        types.insert(
            "Draft".to_string(),
            struct_node(vec![method_returning("publish", "Published")]),
        );
        types.insert("Published".to_string(), struct_node(vec![]));
        let graph = TypeGraph::new(types, HashMap::new());

        let output = render_type_graph_flat(&graph, "domain", &TypeGraphRenderOptions::default());

        assert!(output.contains("Draft[Draft]:::structNode"));
        assert!(output.contains("Published[Published]:::structNode"));
        assert!(output.contains("Draft -->|publish| Published"));
    }

    #[test]
    fn test_render_multiple_edges_from_same_type() {
        let mut types = HashMap::new();
        types.insert(
            "Draft".to_string(),
            struct_node(vec![
                method_returning("publish", "Published"),
                method_returning("archive", "Archived"),
            ]),
        );
        types.insert("Published".to_string(), struct_node(vec![]));
        types.insert("Archived".to_string(), struct_node(vec![]));
        let graph = TypeGraph::new(types, HashMap::new());

        let output = render_type_graph_flat(&graph, "domain", &TypeGraphRenderOptions::default());

        assert!(output.contains("Draft -->|publish| Published"));
        assert!(output.contains("Draft -->|archive| Archived"));
        assert!(output.contains("3 connected"));
    }

    #[test]
    fn test_render_enum_uses_hexagon_shape() {
        let mut types = HashMap::new();
        types.insert(
            "Converter".to_string(),
            struct_node(vec![method_returning("convert", "Status")]),
        );
        types.insert("Status".to_string(), enum_node());
        let graph = TypeGraph::new(types, HashMap::new());

        let output = render_type_graph_flat(&graph, "domain", &TypeGraphRenderOptions::default());

        assert!(
            output.contains("Status{{Status}}:::enumNode"),
            "enum must use hexagon shape, got: {output}"
        );
        assert!(output.contains("Converter[Converter]:::structNode"));
    }

    #[test]
    fn test_render_filters_return_types_to_graph_types_only() {
        // Method returns Result<Published, DomainError> but only Published is in graph.
        // Result, Option, etc. are naturally excluded because they are not workspace types.
        let mut types = HashMap::new();
        types.insert(
            "Draft".to_string(),
            struct_node(vec![method_returning("publish", "Result<Published, DomainError>")]),
        );
        types.insert("Published".to_string(), struct_node(vec![]));
        // DomainError is NOT in the graph — no edge should be created for it
        let graph = TypeGraph::new(types, HashMap::new());

        let output = render_type_graph_flat(&graph, "domain", &TypeGraphRenderOptions::default());

        assert!(output.contains("Draft -->|publish| Published"));
        assert!(!output.contains("DomainError"), "DomainError is not in graph, must not appear");
        // Result and Option are also not in the graph, so no false edges
        assert!(!output.contains("-->|publish| Result"));
    }

    #[test]
    fn test_render_skips_self_return_edges() {
        let mut types = HashMap::new();
        types.insert(
            "Builder".to_string(),
            struct_node(vec![method_returning("with_name", "Builder")]),
        );
        let graph = TypeGraph::new(types, HashMap::new());

        let output = render_type_graph_flat(&graph, "domain", &TypeGraphRenderOptions::default());

        assert!(!output.contains("Builder -->|with_name| Builder"), "self-loops must be excluded");
    }

    #[test]
    fn test_render_max_nodes_truncation() {
        let mut types = HashMap::new();
        for i in 0..6 {
            let methods =
                if i < 5 { vec![method_returning("next", &format!("T{}", i + 1))] } else { vec![] };
            types.insert(format!("T{i}"), struct_node(methods));
        }
        let graph = TypeGraph::new(types, HashMap::new());

        let opts = TypeGraphRenderOptions { max_nodes_per_diagram: 3, ..Default::default() };
        let output = render_type_graph_flat(&graph, "domain", &opts);

        assert!(output.contains("truncated to 3 nodes"));
    }

    #[test]
    fn test_render_skips_associated_functions_without_self() {
        let mut types = HashMap::new();
        types.insert(
            "Factory".to_string(),
            struct_node(vec![MethodDeclaration::new("create", None, vec![], "Product", false)]),
        );
        types.insert("Product".to_string(), struct_node(vec![]));
        let graph = TypeGraph::new(types, HashMap::new());

        let output = render_type_graph_flat(&graph, "domain", &TypeGraphRenderOptions::default());

        assert!(
            !output.contains("Factory -->|create| Product"),
            "associated functions without self must not create edges"
        );
    }

    // --- write_type_graph_file ---

    fn minimal_graph() -> TypeGraph {
        let mut types = HashMap::new();
        types.insert(
            "Draft".to_string(),
            struct_node(vec![method_returning("publish", "Published")]),
        );
        types.insert("Published".to_string(), struct_node(vec![]));
        TypeGraph::new(types, HashMap::new())
    }

    #[test]
    fn test_write_type_graph_file_success_path() {
        let tmp = TempDir::new().unwrap();
        let track_dir = tmp.path().join("track_dir");
        std::fs::create_dir_all(&track_dir).unwrap();

        let graph = minimal_graph();
        let opts = TypeGraphRenderOptions::default();

        let result = write_type_graph_file(&graph, "domain", &track_dir, tmp.path(), &opts);

        assert!(result.is_ok(), "write should succeed: {:?}", result);
        let filename = result.unwrap();
        assert_eq!(filename, "domain-graph.md");

        let written = std::fs::read_to_string(track_dir.join(&filename)).unwrap();
        assert!(written.contains("```mermaid"));
        assert!(written.contains("Draft -->|publish| Published"));
    }

    #[test]
    fn test_write_type_graph_file_rejects_path_traversal_layer_id() {
        let tmp = TempDir::new().unwrap();
        let track_dir = tmp.path().join("track_dir");
        std::fs::create_dir_all(&track_dir).unwrap();

        let graph = TypeGraph::new(HashMap::new(), HashMap::new());
        let opts = TypeGraphRenderOptions::default();

        let result = write_type_graph_file(&graph, "../../escape", &track_dir, tmp.path(), &opts);

        assert!(result.is_err(), "path traversal in layer_id must be rejected");
        let err = result.unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn test_write_type_graph_file_rejects_empty_layer_id() {
        let tmp = TempDir::new().unwrap();
        let track_dir = tmp.path().join("track_dir");
        std::fs::create_dir_all(&track_dir).unwrap();

        let graph = TypeGraph::new(HashMap::new(), HashMap::new());
        let opts = TypeGraphRenderOptions::default();

        let result = write_type_graph_file(&graph, "", &track_dir, tmp.path(), &opts);

        assert!(result.is_err(), "empty layer_id must be rejected");
        let err = result.unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn test_write_type_graph_file_rejects_colon_in_layer_id() {
        // Colon in layer_id could form a Windows drive-relative path (e.g. `C:escape`)
        // where Path::join resolves to the drive root rather than track_dir.
        let tmp = TempDir::new().unwrap();
        let track_dir = tmp.path().join("track_dir");
        std::fs::create_dir_all(&track_dir).unwrap();

        let graph = TypeGraph::new(HashMap::new(), HashMap::new());
        let opts = TypeGraphRenderOptions::default();

        let result = write_type_graph_file(&graph, "C:escape", &track_dir, tmp.path(), &opts);

        assert!(result.is_err(), "colon in layer_id must be rejected");
        let err = result.unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[cfg(unix)]
    #[test]
    fn test_write_type_graph_file_rejects_symlink_in_track_dir() {
        let tmp = TempDir::new().unwrap();
        let real_dir = tmp.path().join("real");
        std::fs::create_dir_all(&real_dir).unwrap();

        let symlink_track = tmp.path().join("symlink_track");
        std::os::unix::fs::symlink(&real_dir, &symlink_track).unwrap();

        let graph = TypeGraph::new(HashMap::new(), HashMap::new());
        let opts = TypeGraphRenderOptions::default();

        // symlink_track itself is a symlink under trusted_root (tmp.path()),
        // so reject_symlinks_below should reject the output path.
        let result = write_type_graph_file(&graph, "domain", &symlink_track, tmp.path(), &opts);

        assert!(result.is_err(), "symlinked track_dir must be rejected by guard");
    }

    // -----------------------------------------------------------------------
    // T004: write_type_graph_dir / render_type_graph_clustered /
    //       render_type_graph_overview
    // -----------------------------------------------------------------------

    // Helper: builds a graph where types carry module_path information.
    fn graph_with_modules() -> TypeGraph {
        let mut types = HashMap::new();

        // domain::review cluster: Draft → Published (intra-cluster edge)
        let mut draft = struct_node(vec![method_returning("publish", "Published")]);
        draft.set_module_path("domain::review".to_owned());
        types.insert("Draft".to_string(), draft);

        let mut published = struct_node(vec![]);
        published.set_module_path("domain::review".to_owned());
        types.insert("Published".to_string(), published);

        // usecase::publish cluster: Publisher → Draft (cross-cluster edge)
        let mut publisher = struct_node(vec![method_returning("create_draft", "Draft")]);
        publisher.set_module_path("usecase::publish".to_owned());
        types.insert("Publisher".to_string(), publisher);

        TypeGraph::new(types, HashMap::new())
    }

    /// T004 test 1 — dir structure: `write_type_graph_dir` writes index.md
    /// and one file per cluster under `<layer_id>-graph/`.
    #[test]
    fn test_write_type_graph_dir_creates_expected_structure() {
        let tmp = TempDir::new().unwrap();
        let track_dir = tmp.path().join("track_dir");
        std::fs::create_dir_all(&track_dir).unwrap();

        let graph = graph_with_modules();
        let opts = TypeGraphRenderOptions { cluster_depth: 2, ..Default::default() };

        let result = write_type_graph_dir(&graph, "domain", &track_dir, tmp.path(), &opts);
        assert!(result.is_ok(), "write_type_graph_dir must succeed: {result:?}");

        let written = result.unwrap();
        assert!(
            written.contains(&"domain-graph/index.md".to_owned()),
            "must write index.md, got: {written:?}"
        );
        // Cluster files should include both clusters.
        // Filenames use `_` as the `::` separator per the cluster-layout spec.
        assert!(
            written.iter().any(|p| p.ends_with("domain_review.md")),
            "must write domain_review.md, got: {written:?}"
        );
        assert!(
            written.iter().any(|p| p.ends_with("usecase_publish.md")),
            "must write usecase_publish.md, got: {written:?}"
        );

        // Verify files exist on disk.
        let graph_dir = track_dir.join("domain-graph");
        assert!(graph_dir.join("index.md").exists(), "index.md must exist");
        assert!(graph_dir.join("domain_review.md").exists(), "domain_review.md must exist");
        assert!(graph_dir.join("usecase_publish.md").exists(), "usecase_publish.md must exist");
    }

    /// T004 test 2 — path traversal guard: `write_type_graph_dir` rejects
    /// unsafe `layer_id` values with `InvalidInput`.
    #[test]
    fn test_write_type_graph_dir_rejects_path_traversal_layer_id() {
        let tmp = TempDir::new().unwrap();
        let track_dir = tmp.path().join("track_dir");
        std::fs::create_dir_all(&track_dir).unwrap();

        let graph = TypeGraph::new(HashMap::new(), HashMap::new());
        let opts = TypeGraphRenderOptions { cluster_depth: 2, ..Default::default() };

        let result = write_type_graph_dir(&graph, "../../escape", &track_dir, tmp.path(), &opts);
        assert!(result.is_err(), "path traversal in layer_id must be rejected");
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidInput);
    }

    /// T004 extra — precondition guard: `write_type_graph_dir` rejects
    /// `cluster_depth == 0` with `InvalidInput`.
    #[test]
    fn test_write_type_graph_dir_rejects_cluster_depth_zero() {
        let tmp = TempDir::new().unwrap();
        let track_dir = tmp.path().join("track_dir");
        std::fs::create_dir_all(&track_dir).unwrap();

        let graph = TypeGraph::new(HashMap::new(), HashMap::new());
        let opts = TypeGraphRenderOptions { cluster_depth: 0, ..Default::default() };

        let result = write_type_graph_dir(&graph, "domain", &track_dir, tmp.path(), &opts);
        assert!(result.is_err(), "cluster_depth=0 must be rejected by write_type_graph_dir");
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidInput);
    }

    /// T004 test 3 — stale flat→cluster cleanup: when a flat `<layer_id>-graph.md`
    /// exists and `write_type_graph_dir` is called, the flat file is removed.
    #[test]
    fn test_write_type_graph_dir_removes_stale_flat_file() {
        let tmp = TempDir::new().unwrap();
        let track_dir = tmp.path().join("track_dir");
        std::fs::create_dir_all(&track_dir).unwrap();

        // Create a stale flat file from a previous flat-mode run.
        let stale = track_dir.join("domain-graph.md");
        std::fs::write(&stale, "stale flat content").unwrap();
        assert!(stale.exists(), "precondition: stale flat file must exist");

        let graph = TypeGraph::new(HashMap::new(), HashMap::new());
        let opts = TypeGraphRenderOptions { cluster_depth: 2, ..Default::default() };

        let result = write_type_graph_dir(&graph, "domain", &track_dir, tmp.path(), &opts);
        assert!(result.is_ok(), "write_type_graph_dir must succeed: {result:?}");
        assert!(!stale.exists(), "stale flat file must have been removed");
    }

    /// T004 test 4 — stale cluster→flat cleanup: when a cluster directory
    /// `<layer_id>-graph/` exists and `write_type_graph_file` is called with
    /// `cluster_depth=0`, the cluster directory is removed.
    #[test]
    fn test_write_type_graph_file_removes_stale_cluster_dir() {
        let tmp = TempDir::new().unwrap();
        let track_dir = tmp.path().join("track_dir");
        std::fs::create_dir_all(&track_dir).unwrap();

        // Create a stale cluster directory from a previous cluster-mode run.
        let stale_dir = track_dir.join("domain-graph");
        std::fs::create_dir_all(&stale_dir).unwrap();
        std::fs::write(stale_dir.join("index.md"), "stale index").unwrap();
        assert!(stale_dir.exists(), "precondition: stale cluster dir must exist");

        let graph = TypeGraph::new(HashMap::new(), HashMap::new());
        let opts = TypeGraphRenderOptions { cluster_depth: 0, ..Default::default() };

        let result = write_type_graph_file(&graph, "domain", &track_dir, tmp.path(), &opts);
        assert!(result.is_ok(), "write_type_graph_file must succeed: {result:?}");
        assert!(!stale_dir.exists(), "stale cluster directory must have been removed");
        assert!(track_dir.join("domain-graph.md").exists(), "flat file must have been written");
    }

    /// T004 test 5 — overview node set: `render_type_graph_overview` emits one
    /// mermaid node per cluster.
    #[test]
    fn test_render_type_graph_overview_emits_one_node_per_cluster() {
        let graph = graph_with_modules();
        let edges = collect_edges(&graph, EdgeSet::Methods);
        let plan = crate::tddd::type_graph_cluster::classify_types(&graph, 2, &edges);
        let opts = TypeGraphRenderOptions { cluster_depth: 2, ..Default::default() };

        let output = render_type_graph_overview(&graph, &plan, &opts);

        // Should have nodes for both clusters.
        assert!(
            output.contains("domain__review") || output.contains("\"domain::review\""),
            "overview must contain domain::review cluster node, got: {output}"
        );
        assert!(
            output.contains("usecase__publish") || output.contains("\"usecase::publish\""),
            "overview must contain usecase::publish cluster node, got: {output}"
        );
        assert!(output.contains("```mermaid"), "must contain mermaid block");
        assert!(output.contains("clusterNode"), "must use clusterNode class");
    }

    /// T004 test 6 — clustered intra-cluster-only edges: `render_type_graph_clustered`
    /// for the `domain::review` cluster must include the intra-cluster
    /// `Draft →|publish| Published` edge but NOT the cross-cluster edge
    /// from `Publisher` (which lives in `usecase::publish`).
    #[test]
    fn test_render_type_graph_clustered_intra_cluster_edges_only() {
        let graph = graph_with_modules();
        let edges = collect_edges(&graph, EdgeSet::Methods);
        let plan = crate::tddd::type_graph_cluster::classify_types(&graph, 2, &edges);
        let opts = TypeGraphRenderOptions { cluster_depth: 2, ..Default::default() };

        let output = render_type_graph_clustered(&graph, "domain::review", &plan, &opts);

        // Intra-cluster edge must appear.
        assert!(
            output.contains("Draft -->|publish| Published"),
            "intra-cluster edge must appear, got: {output}"
        );
        // Cross-cluster edge from Publisher must NOT appear as a direct edge,
        // but may appear as a ghost reference (the publisher is in a different cluster).
        // The cross-cluster outgoing edge FROM Publisher lives in usecase::publish cluster,
        // so the domain::review cluster diagram must not contain Publisher as a source.
        assert!(
            !output.contains("Publisher -->"),
            "Publisher is not in domain::review cluster; must not appear as edge source: {output}"
        );
    }

    // -----------------------------------------------------------------------
    // EdgeSet::Fields + EdgeSet::Impls + EdgeSet::All + traitNode
    // -----------------------------------------------------------------------

    /// Helper: builds a struct node with a list of struct fields.
    fn struct_node_with_fields(
        fields: Vec<MemberDeclaration>,
        methods: Vec<MethodDeclaration>,
    ) -> TypeNode {
        TypeNode::new(TypeKind::Struct, fields, methods, HashSet::new())
    }

    /// Helper: builds a struct node with trait impls attached.
    fn struct_node_with_impls(impls: Vec<domain::schema::TraitImplEntry>) -> TypeNode {
        let mut node = TypeNode::new(TypeKind::Struct, vec![], vec![], HashSet::new());
        node.set_trait_impls(impls);
        node
    }

    /// Field edges test: `EdgeSet::Fields` produces solid no-arrow edges
    /// `A ---|field_name| B` when a struct field type exists in the graph.
    #[test]
    fn test_render_field_edges_uses_solid_no_arrow_syntax() {
        let mut types = HashMap::new();
        // Order has a field `item: Item` — both types are in the graph.
        types.insert(
            "Order".to_string(),
            struct_node_with_fields(vec![MemberDeclaration::field("item", "Item")], vec![]),
        );
        types.insert("Item".to_string(), struct_node(vec![]));
        let graph = TypeGraph::new(types, HashMap::new());

        let opts = TypeGraphRenderOptions {
            edge_set: EdgeSet::Fields,
            cluster_depth: 0,
            ..Default::default()
        };
        let output = render_type_graph_flat(&graph, "domain", &opts);

        // Field edge must use `---|label|` (solid, no arrowhead).
        assert!(
            output.contains("Order ---|item| Item"),
            "field edge must use solid no-arrow syntax, got: {output}"
        );
        // Must NOT produce a method arrow.
        assert!(
            !output.contains("Order -->|item| Item"),
            "field edge must NOT use arrow syntax: {output}"
        );
    }

    /// Trait impl edges test: `EdgeSet::Impls` produces dashed arrow edges
    /// `A -.->|impl| TraitName` and renders trait nodes as stadium shape `([TraitName])`.
    #[test]
    fn test_render_impl_edges_uses_dashed_arrow_and_trait_node_stadium_shape() {
        use domain::schema::TraitImplEntry;

        let mut types = HashMap::new();
        types.insert(
            "FsStore".to_string(),
            struct_node_with_impls(vec![TraitImplEntry::new("Repository", vec![])]),
        );
        let mut traits = HashMap::new();
        traits.insert("Repository".to_string(), domain::schema::TraitNode::new(vec![]));
        let graph = TypeGraph::new(types, traits);

        let opts = TypeGraphRenderOptions {
            edge_set: EdgeSet::Impls,
            cluster_depth: 0,
            ..Default::default()
        };
        let output = render_type_graph_flat(&graph, "domain", &opts);

        // Impl edge must use `-.->|impl|` (dashed arrow) with disambiguated trait node ID.
        assert!(
            output.contains("FsStore -.->|impl| _trait_Repository"),
            "impl edge must use dashed arrow syntax with _trait_ prefix, got: {output}"
        );
        // Trait node must use disambiguated ID `_trait_{name}` and stadium shape.
        assert!(
            output.contains("_trait_Repository([Repository]):::traitNode"),
            "trait node must use disambiguated ID and stadium shape, got: {output}"
        );
        // classDef for traitNode must be emitted.
        assert!(
            output.contains("classDef traitNode"),
            "traitNode classDef must be emitted, got: {output}"
        );
    }

    /// `EdgeSet::All` is the union of methods + fields + impls.
    #[test]
    fn test_edge_set_all_is_union_of_methods_fields_and_impls() {
        use domain::schema::TraitImplEntry;

        let mut types = HashMap::new();
        // Draft has a method edge → Published
        // Draft has a field edge → Item
        // Draft has an impl edge → DraftTrait
        let mut draft = TypeNode::new(
            TypeKind::Struct,
            vec![MemberDeclaration::field("item", "Item")],
            vec![method_returning("publish", "Published")],
            HashSet::new(),
        );
        draft.set_trait_impls(vec![TraitImplEntry::new("DraftTrait", vec![])]);
        types.insert("Draft".to_string(), draft);
        types.insert("Published".to_string(), struct_node(vec![]));
        types.insert("Item".to_string(), struct_node(vec![]));

        let mut traits = HashMap::new();
        traits.insert("DraftTrait".to_string(), domain::schema::TraitNode::new(vec![]));
        let graph = TypeGraph::new(types, traits);

        let opts = TypeGraphRenderOptions {
            edge_set: EdgeSet::All,
            cluster_depth: 0,
            ..Default::default()
        };
        let output = render_type_graph_flat(&graph, "domain", &opts);

        // Method edge must appear.
        assert!(
            output.contains("Draft -->|publish| Published"),
            "method edge must appear in All, got: {output}"
        );
        // Field edge must appear.
        assert!(
            output.contains("Draft ---|item| Item"),
            "field edge must appear in All, got: {output}"
        );
        // Impl edge must appear with disambiguated trait node ID.
        assert!(
            output.contains("Draft -.->|impl| _trait_DraftTrait"),
            "impl edge must appear in All with _trait_ prefix, got: {output}"
        );
    }

    /// Trait node uses stadium shape `([TraitName])` in cluster mode.
    ///
    /// Verifies that trait nodes rendered inside a cluster diagram also use
    /// the stadium shape with `traitNode` class (not the default rect/hexagon shapes).
    #[test]
    fn test_trait_node_stadium_shape_in_clustered_rendering() {
        use domain::schema::TraitImplEntry;

        let mut types = HashMap::new();
        let mut adapter = TypeNode::new(TypeKind::Struct, vec![], vec![], HashSet::new());
        adapter.set_module_path("infra::store".to_owned());
        adapter.set_trait_impls(vec![TraitImplEntry::new("StorePort", vec![])]);
        types.insert("FsAdapter".to_string(), adapter);

        let mut traits = HashMap::new();
        traits.insert("StorePort".to_string(), domain::schema::TraitNode::new(vec![]));
        let graph = TypeGraph::new(types, traits);

        let opts = TypeGraphRenderOptions {
            edge_set: EdgeSet::Impls,
            cluster_depth: 2,
            ..Default::default()
        };

        let edges = collect_edges(&graph, opts.edge_set);
        let plan = crate::tddd::type_graph_cluster::classify_types(&graph, 2, &edges);

        let output = render_type_graph_clustered(&graph, "infra::store", &plan, &opts);

        // Trait node in cluster diagram must use disambiguated ID and stadium shape.
        assert!(
            output.contains("_trait_StorePort([StorePort]):::traitNode"),
            "trait node in cluster must use disambiguated ID and stadium shape, got: {output}"
        );
        // Impl edge must appear with dashed arrow and disambiguated trait node ID.
        assert!(
            output.contains("FsAdapter -.->|impl| _trait_StorePort"),
            "impl edge must appear in cluster rendering with _trait_ prefix, got: {output}"
        );
    }
}
