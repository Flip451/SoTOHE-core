//! Mermaid type graph renderer — generates flowchart visualizations from `TypeGraph`.
//!
//! Phase 1 (ADR `knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md` §D7):
//! flat rendering with method edges only. Produces a markdown file containing a
//! fenced `mermaid` block with `flowchart LR`.
//!
//! Types as nodes:
//! - struct → `[Name]` (rectangle) with `structNode` class
//! - enum → `{{Name}}` (hexagon) with `enumNode` class
//!
//! Edges (Phase 1, methods only):
//! - For each inherent method with a self-receiver, extract PascalCase type names
//!   from the `returns()` string and create `A -->|method_name| B` edges for each
//!   return type that exists in the `TypeGraph`.
//!
//! **Known Phase 1 limitations** (acceptable for the readability spike):
//! - Associated type binding labels (e.g. `Item` in `Iterator<Item = Foo>`) are
//!   extracted as PascalCase tokens. If a workspace type coincidentally shares the
//!   label name, a false edge may appear. Phase 2 can add label-aware filtering.
//! - Stdlib wrapper names (`Result`, `Option`, `Vec` …) are NOT explicitly
//!   filtered — they are naturally excluded because `TypeGraph` only contains
//!   types from the workspace crate's rustdoc export, not stdlib re-exports.

use std::collections::HashSet;
use std::path::Path;

use domain::schema::{TypeGraph, TypeKind};

use crate::track::atomic_write::atomic_write_file;
use crate::track::symlink_guard::reject_symlinks_below;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Selects which edge types to include in the mermaid type graph render.
///
/// Phase 1 implements only `Methods`. `Fields` and `Impls` are Phase 2 stubs
/// that currently produce no edges (callers receive a method-only diagram or an
/// empty diagram, respectively). `All` includes every implemented edge type,
/// which in Phase 1 is the same as `Methods`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeSet {
    /// Only inherent method edges (self → return type). Fully implemented in Phase 1.
    Methods,
    /// Only struct field / enum variant edges.
    /// **Phase 2 stub** — currently produces no edges.
    Fields,
    /// Only trait impl edges.
    /// **Phase 2 stub** — currently produces no edges.
    Impls,
    /// All edge types. In Phase 1 this is equivalent to `Methods`
    /// (field and impl edges are Phase 2 stubs).
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
}

impl Default for TypeGraphRenderOptions {
    fn default() -> Self {
        Self { edge_set: EdgeSet::Methods, max_nodes_per_diagram: 50 }
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
    let graph_type_names: HashSet<&str> = graph.type_names().map(|s| s.as_str()).collect();

    // Collect edges: (source, method_name, target)
    let mut edges: Vec<(String, String, String)> = Vec::new();

    if matches!(opts.edge_set, EdgeSet::Methods | EdgeSet::All) {
        for source_name in graph.type_names() {
            if let Some(node) = graph.get_type(source_name) {
                for method in node.methods() {
                    if method.receiver().is_none() {
                        continue; // skip associated functions
                    }
                    let targets = extract_type_names(method.returns());
                    for target in targets {
                        // Only create edges to types that exist in the workspace TypeGraph.
                        // Stdlib types (Result, Option, Vec, etc.) are naturally excluded
                        // because rustdoc only exports the workspace crate's own pub API.
                        if graph_type_names.contains(target) && target != source_name.as_str() {
                            edges.push((
                                source_name.clone(),
                                method.name().to_string(),
                                target.to_string(),
                            ));
                        }
                    }
                }
            }
        }
    }

    // Deduplicate edges (same source→target with same label)
    edges.sort();
    edges.dedup();

    // Apply max_nodes guard by limiting edges first, then deriving nodes.
    // This ensures every rendered node participates in at least one rendered
    // edge, avoiding isolated nodes that appear when nodes are truncated
    // without regard for edge connectivity.
    let total_nodes_connected = {
        let mut set: HashSet<&str> = HashSet::new();
        for (src, _, tgt) in &edges {
            set.insert(src.as_str());
            set.insert(tgt.as_str());
        }
        set.len()
    };
    let truncated = total_nodes_connected > opts.max_nodes_per_diagram;
    if truncated {
        // Keep only edges whose both endpoints fit within the node budget.
        // Greedily accept edges (already sorted) until the node set is full.
        let mut kept_nodes: HashSet<String> = HashSet::new();
        edges.retain(|(src, _, tgt)| {
            let src_new = !kept_nodes.contains(src);
            let tgt_new = !kept_nodes.contains(tgt);
            let would_add = src_new as usize + tgt_new as usize;
            if kept_nodes.len() + would_add <= opts.max_nodes_per_diagram {
                kept_nodes.insert(src.clone());
                kept_nodes.insert(tgt.clone());
                true
            } else {
                false
            }
        });
    }

    // Collect connected node names from the (possibly truncated) edge set
    let node_names: Vec<&str> = {
        let mut set: HashSet<&str> = HashSet::new();
        for (src, _, tgt) in &edges {
            set.insert(src.as_str());
            set.insert(tgt.as_str());
        }
        let mut names: Vec<&str> = set.into_iter().collect();
        names.sort();
        names
    };
    let node_set: HashSet<&str> = node_names.iter().copied().collect();

    // Build output
    let mut out = String::new();
    out.push_str(&format!(
        "<!-- Generated from {layer_name} TypeGraph — DO NOT EDIT DIRECTLY -->\n"
    ));
    out.push_str(&format!("# {layer_name} Type Graph\n\n"));

    let total_types = graph.type_names().count();
    out.push_str(&format!(
        "Types: {total_types} total, {} connected, {} edges",
        node_names.len(),
        edges.len()
    ));
    if truncated {
        out.push_str(&format!(" (truncated to {} nodes)", opts.max_nodes_per_diagram));
    }
    out.push_str("\n\n");

    out.push_str("```mermaid\nflowchart LR\n");
    out.push_str("    classDef structNode fill:#f3e5f5,stroke:#7b1fa2\n");
    out.push_str("    classDef enumNode fill:#e1f5fe,stroke:#0288d1\n\n");

    // Emit nodes
    for name in &node_names {
        if let Some(node) = graph.get_type(name) {
            let shape = match node.kind() {
                TypeKind::Enum => format!("    {name}{{{{{name}}}}}:::{}", "enumNode"),
                _ => format!("    {name}[{name}]:::{}", "structNode"),
            };
            out.push_str(&shape);
            out.push('\n');
        }
    }

    if !node_names.is_empty() && !edges.is_empty() {
        out.push('\n');
    }

    // Emit edges (only between nodes in the node_set)
    for (src, label, tgt) in &edges {
        if node_set.contains(src.as_str()) && node_set.contains(tgt.as_str()) {
            out.push_str(&format!("    {src} -->|{label}| {tgt}\n"));
        }
    }

    out.push_str("```\n");
    out
}

// ---------------------------------------------------------------------------
// Write helper (symlink-checked)
// ---------------------------------------------------------------------------

/// Renders a mermaid type graph and writes it to `<layer_id>-graph.md` inside
/// `track_dir`, with symlink protection relative to `trusted_root`.
///
/// Combines `render_type_graph_flat` + `reject_symlinks_below` + `atomic_write_file`
/// so that the symlink guard stays in the infrastructure layer (not CLI).
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

    let rendered = render_type_graph_flat(graph, layer_id, opts);

    let graph_filename = format!("{layer_id}-graph.md");
    let graph_path = track_dir.join(&graph_filename);

    reject_symlinks_below(&graph_path, trusted_root)?;
    atomic_write_file(&graph_path, rendered.as_bytes())?;

    Ok(graph_filename)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extracts PascalCase type names from a type string.
///
/// Splits on non-alphanumeric/underscore characters and keeps tokens that
/// start with an uppercase letter. Used to find potential type references
/// in return type strings like `"Result<Option<User>, DomainError>"`.
fn extract_type_names(ty: &str) -> Vec<&str> {
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
}
