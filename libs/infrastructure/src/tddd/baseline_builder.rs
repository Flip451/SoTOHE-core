//! Builds a `TypeBaseline` from a `TypeGraph`.
//!
//! Extracts kind, members, and method_return_types from each `TypeNode`,
//! and method_names from each `TraitNode`. Excludes `outgoing` (derivable
//! from method_return_types) and `module_path` (module moves are not
//! structural changes).

use std::collections::HashMap;

use domain::schema::TypeGraph;
use domain::{Timestamp, TraitBaselineEntry, TypeBaseline, TypeBaselineEntry};

/// Builds a `TypeBaseline` from the given `TypeGraph` and capture timestamp.
///
/// Each `TypeNode` contributes a `TypeBaselineEntry` with:
/// - `kind`: the node's `TypeKind`
/// - `members`: variant names (enums) or field names (structs)
/// - `method_return_types`: return types of inherent impl methods
///
/// Each `TraitNode` contributes a `TraitBaselineEntry` with method names.
///
/// Excluded fields (per ADR TDDD-02 §1):
/// - `outgoing`: derivable from `method_return_types ∩ typestate_names`
/// - `module_path`: module moves are refactoring, not structural changes
#[must_use]
pub fn build_baseline(graph: &TypeGraph, captured_at: Timestamp) -> TypeBaseline {
    let mut types = HashMap::new();
    for name in graph.type_names() {
        if let Some(node) = graph.get_type(name) {
            let entry = TypeBaselineEntry::new(
                node.kind().clone(),
                node.members().to_vec(),
                node.method_return_types().iter().cloned().collect(),
            );
            types.insert(name.clone(), entry);
        }
    }

    let mut traits = HashMap::new();
    for name in graph.trait_names() {
        if let Some(node) = graph.get_trait(name) {
            let entry = TraitBaselineEntry::new(node.method_names().to_vec());
            traits.insert(name.clone(), entry);
        }
    }

    TypeBaseline::new(1, captured_at, types, traits)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::HashSet;

    use domain::schema::{TraitNode, TypeKind, TypeNode};

    use super::*;

    fn make_timestamp() -> Timestamp {
        Timestamp::new("2026-04-11T00:00:00Z").unwrap()
    }

    #[test]
    fn test_build_baseline_extracts_struct_type() {
        let mut types = HashMap::new();
        types.insert(
            "TrackId".to_string(),
            TypeNode::new(TypeKind::Struct, vec!["0".into()], HashSet::new(), HashSet::new()),
        );
        let graph = TypeGraph::new(types, HashMap::new());

        let bl = build_baseline(&graph, make_timestamp());

        let entry = bl.get_type("TrackId").unwrap();
        assert_eq!(entry.kind(), &TypeKind::Struct);
        assert_eq!(entry.members(), &["0"]);
        assert!(entry.method_return_types().is_empty());
    }

    #[test]
    fn test_build_baseline_extracts_enum_type_with_variants() {
        let mut types = HashMap::new();
        types.insert(
            "TaskStatus".to_string(),
            TypeNode::new(
                TypeKind::Enum,
                vec!["Todo".into(), "InProgress".into(), "Done".into()],
                HashSet::from(["TaskStatusKind".to_string()]),
                HashSet::new(),
            ),
        );
        let graph = TypeGraph::new(types, HashMap::new());

        let bl = build_baseline(&graph, make_timestamp());

        let entry = bl.get_type("TaskStatus").unwrap();
        assert_eq!(entry.kind(), &TypeKind::Enum);
        // Members are sorted by TypeBaselineEntry::new
        assert_eq!(entry.members(), &["Done", "InProgress", "Todo"]);
        assert_eq!(entry.method_return_types(), &["TaskStatusKind"]);
    }

    #[test]
    fn test_build_baseline_excludes_outgoing() {
        let mut types = HashMap::new();
        let method_return_types = HashSet::from(["Published".to_string()]);
        let outgoing = HashSet::from(["Published".to_string()]);
        types.insert(
            "Draft".to_string(),
            TypeNode::new(TypeKind::Struct, vec![], method_return_types, outgoing),
        );
        let graph = TypeGraph::new(types, HashMap::new());

        let bl = build_baseline(&graph, make_timestamp());

        let entry = bl.get_type("Draft").unwrap();
        // method_return_types is included, but outgoing is not a field on TypeBaselineEntry
        assert_eq!(entry.method_return_types(), &["Published"]);
    }

    #[test]
    fn test_build_baseline_extracts_trait() {
        let mut traits = HashMap::new();
        traits
            .insert("TrackReader".to_string(), TraitNode::new(vec!["find".into(), "list".into()]));
        let graph = TypeGraph::new(HashMap::new(), traits);

        let bl = build_baseline(&graph, make_timestamp());

        let entry = bl.get_trait("TrackReader").unwrap();
        // Methods are sorted by TraitBaselineEntry::new
        assert_eq!(entry.methods(), &["find", "list"]);
    }

    #[test]
    fn test_build_baseline_sets_schema_version_and_timestamp() {
        let graph = TypeGraph::new(HashMap::new(), HashMap::new());
        let bl = build_baseline(&graph, make_timestamp());

        assert_eq!(bl.schema_version(), 1);
        assert_eq!(bl.captured_at().as_str(), "2026-04-11T00:00:00Z");
    }

    #[test]
    fn test_build_baseline_empty_graph_produces_empty_baseline() {
        let graph = TypeGraph::new(HashMap::new(), HashMap::new());
        let bl = build_baseline(&graph, make_timestamp());

        assert!(bl.types().is_empty());
        assert!(bl.traits().is_empty());
    }

    #[test]
    fn test_build_baseline_multiple_types_and_traits() {
        let mut types = HashMap::new();
        types.insert(
            "A".to_string(),
            TypeNode::new(TypeKind::Struct, vec![], HashSet::new(), HashSet::new()),
        );
        types.insert(
            "B".to_string(),
            TypeNode::new(TypeKind::Enum, vec!["X".into()], HashSet::new(), HashSet::new()),
        );

        let mut traits = HashMap::new();
        traits.insert("T1".to_string(), TraitNode::new(vec!["m1".into()]));
        traits.insert("T2".to_string(), TraitNode::new(vec!["m2".into()]));

        let graph = TypeGraph::new(types, traits);
        let bl = build_baseline(&graph, make_timestamp());

        assert_eq!(bl.types().len(), 2);
        assert_eq!(bl.traits().len(), 2);
    }
}
