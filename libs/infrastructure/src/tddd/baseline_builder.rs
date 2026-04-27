//! Builds a `TypeBaseline` from a `TypeGraph`.
//!
//! Baseline schema v2 — members are captured as `Vec<MemberDeclaration>`
//! (struct fields / enum variants) and methods as `Vec<MethodDeclaration>`
//! (L1 signatures). Previously the baseline stored only a flat `Vec<String>`
//! for members and `Vec<String>` return type names. The `outgoing` field is
//! excluded (derivable from `methods_with_self_receiver ∩ typestate_names`),
//! and `module_path` is excluded (module moves are not structural changes).
//!
//! T007 (S4) extensions:
//! - `TypeBaselineEntry::trait_impls` is populated from `TypeNode::trait_impls()`.
//! - `TypeBaseline::functions` is populated from `TypeGraph::functions()`.
//!   Tuple key `(short_name, module_path)` is converted to a fully-qualified
//!   string key: `module_path::short_name`, or just `short_name` when
//!   `module_path` is `None`.

use std::collections::HashMap;

use domain::schema::TypeGraph;
use domain::{
    FunctionBaselineEntry, Timestamp, TraitBaselineEntry, TraitImplBaselineEntry, TypeBaseline,
    TypeBaselineEntry,
};

/// Builds a `TypeBaseline` from the given `TypeGraph` and capture timestamp.
///
/// Each `TypeNode` contributes a `TypeBaselineEntry` with:
/// - `kind`: the node's `TypeKind`
/// - `members`: structured enum variants / struct fields
/// - `methods`: structured L1 inherent method signatures
/// - `trait_impls`: trait implementations with `origin_crate` (T007)
///
/// Each `TraitNode` contributes a `TraitBaselineEntry` with
/// `Vec<MethodDeclaration>`.
///
/// `TypeBaseline::functions` is populated from `TypeGraph::functions()` (T007).
#[must_use]
pub fn build_baseline(graph: &TypeGraph, captured_at: Timestamp) -> TypeBaseline {
    let mut types = HashMap::new();
    for name in graph.type_names() {
        if let Some(node) = graph.get_type(name) {
            // T007 (S4): convert TraitImplEntry → TraitImplBaselineEntry.
            let trait_impls: Vec<TraitImplBaselineEntry> = node
                .trait_impls()
                .iter()
                .map(|ti| TraitImplBaselineEntry::new(ti.trait_name(), ti.origin_crate()))
                .collect();

            let entry = TypeBaselineEntry::with_trait_impls(
                node.kind().clone(),
                node.members().to_vec(),
                node.methods().to_vec(),
                trait_impls,
            );
            types.insert(name.clone(), entry);
        }
    }

    let mut traits = HashMap::new();
    for name in graph.trait_names() {
        if let Some(node) = graph.get_trait(name) {
            let entry = TraitBaselineEntry::new(node.methods().to_vec());
            traits.insert(name.clone(), entry);
        }
    }

    // T007 (S4): convert TypeGraph::functions() tuple keys to fully-qualified string keys.
    let functions: HashMap<String, FunctionBaselineEntry> = graph
        .functions()
        .iter()
        .map(|((short_name, module_path), fn_node)| {
            let fq_key = match module_path {
                Some(mp) => format!("{mp}::{short_name}"),
                None => short_name.clone(),
            };
            let entry = FunctionBaselineEntry::new(
                fn_node.params().to_vec(),
                fn_node.returns().to_vec(),
                fn_node.is_async(),
                fn_node.module_path().map(str::to_string),
            );
            (fq_key, entry)
        })
        .collect();

    TypeBaseline::with_functions(2, captured_at, types, traits, functions)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use std::collections::HashSet;

    use domain::schema::{FunctionNode, TraitImplEntry, TraitNode, TypeKind, TypeNode};
    use domain::tddd::catalogue::{MemberDeclaration, MethodDeclaration, ParamDeclaration};

    use super::*;

    fn make_timestamp() -> Timestamp {
        Timestamp::new("2026-04-13T00:00:00Z").unwrap()
    }

    fn unit_method(name: &str) -> MethodDeclaration {
        MethodDeclaration::new(name, Some("&self".into()), vec![], "()", false)
    }

    #[test]
    fn test_build_baseline_extracts_struct_type() {
        let mut types = HashMap::new();
        types.insert(
            "TrackId".to_string(),
            TypeNode::new(
                TypeKind::Struct,
                vec![MemberDeclaration::field("0", "u64")],
                vec![],
                HashSet::new(),
            ),
        );
        let graph = TypeGraph::new(types, HashMap::new());

        let bl = build_baseline(&graph, make_timestamp());

        let entry = bl.get_type("TrackId").unwrap();
        assert_eq!(entry.kind(), &TypeKind::Struct);
        assert_eq!(entry.members().len(), 1);
        assert_eq!(entry.members()[0].name(), "0");
        assert!(entry.methods().is_empty());
    }

    #[test]
    fn test_build_baseline_extracts_enum_type_with_variants() {
        let mut types = HashMap::new();
        types.insert(
            "TaskStatus".to_string(),
            TypeNode::new(
                TypeKind::Enum,
                vec![
                    MemberDeclaration::variant("Todo"),
                    MemberDeclaration::variant("InProgress"),
                    MemberDeclaration::variant("Done"),
                ],
                vec![unit_method("kind")],
                HashSet::new(),
            ),
        );
        let graph = TypeGraph::new(types, HashMap::new());

        let bl = build_baseline(&graph, make_timestamp());

        let entry = bl.get_type("TaskStatus").unwrap();
        assert_eq!(entry.kind(), &TypeKind::Enum);
        let names: Vec<&str> = entry.members().iter().map(|m| m.name()).collect();
        assert_eq!(names, vec!["Done", "InProgress", "Todo"]);
        assert_eq!(entry.methods().len(), 1);
        assert_eq!(entry.methods()[0].name(), "kind");
    }

    #[test]
    fn test_build_baseline_excludes_outgoing() {
        let mut types = HashMap::new();
        let outgoing = HashSet::from(["Published".to_string()]);
        types.insert(
            "Draft".to_string(),
            TypeNode::new(TypeKind::Struct, vec![], vec![unit_method("publish")], outgoing),
        );
        let graph = TypeGraph::new(types, HashMap::new());

        let bl = build_baseline(&graph, make_timestamp());

        let entry = bl.get_type("Draft").unwrap();
        assert_eq!(entry.methods().len(), 1);
        assert_eq!(entry.methods()[0].name(), "publish");
    }

    #[test]
    fn test_build_baseline_extracts_trait() {
        let mut traits = HashMap::new();
        traits.insert(
            "TrackReader".to_string(),
            TraitNode::new(vec![unit_method("find"), unit_method("list")]),
        );
        let graph = TypeGraph::new(HashMap::new(), traits);

        let bl = build_baseline(&graph, make_timestamp());

        let entry = bl.get_trait("TrackReader").unwrap();
        let names: Vec<&str> = entry.methods().iter().map(|m| m.name()).collect();
        assert_eq!(names, vec!["find", "list"]);
    }

    #[test]
    fn test_build_baseline_sets_schema_version_and_timestamp() {
        let graph = TypeGraph::new(HashMap::new(), HashMap::new());
        let bl = build_baseline(&graph, make_timestamp());

        assert_eq!(bl.schema_version(), 2);
        assert_eq!(bl.captured_at().as_str(), "2026-04-13T00:00:00Z");
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
            TypeNode::new(TypeKind::Struct, vec![], vec![], HashSet::new()),
        );
        types.insert(
            "B".to_string(),
            TypeNode::new(
                TypeKind::Enum,
                vec![MemberDeclaration::variant("X")],
                vec![],
                HashSet::new(),
            ),
        );

        let mut traits = HashMap::new();
        traits.insert("T1".to_string(), TraitNode::new(vec![unit_method("m1")]));
        traits.insert("T2".to_string(), TraitNode::new(vec![unit_method("m2")]));

        let graph = TypeGraph::new(types, traits);
        let bl = build_baseline(&graph, make_timestamp());

        assert_eq!(bl.types().len(), 2);
        assert_eq!(bl.traits().len(), 2);
    }

    // --- AC-07: baseline trait_impls capture ---

    /// AC-07: trait_impls on a TypeNode are captured into TypeBaselineEntry::trait_impls
    /// with trait_name and origin_crate preserved.
    #[test]
    fn test_build_baseline_captures_trait_impls_with_origin_crate() {
        let mut node = TypeNode::new(TypeKind::Struct, vec![], vec![], HashSet::new());
        node.set_trait_impls(vec![
            TraitImplEntry::with_origin_crate("TrackReader", vec![], "domain"),
            TraitImplEntry::with_origin_crate("Display", vec![], "std"),
        ]);

        let mut types = HashMap::new();
        types.insert("FsStore".to_string(), node);
        let graph = TypeGraph::new(types, HashMap::new());

        let bl = build_baseline(&graph, make_timestamp());

        let entry = bl.get_type("FsStore").unwrap();
        let impls = entry.trait_impls();
        assert_eq!(impls.len(), 2);

        let track_reader = impls.iter().find(|i| i.trait_name() == "TrackReader").unwrap();
        assert_eq!(track_reader.origin_crate(), "domain");

        let display = impls.iter().find(|i| i.trait_name() == "Display").unwrap();
        assert_eq!(display.origin_crate(), "std");
    }

    /// AC-07: TypeNode with no trait_impls produces empty trait_impls in baseline.
    #[test]
    fn test_build_baseline_empty_trait_impls_produces_empty_baseline_trait_impls() {
        let mut types = HashMap::new();
        types.insert(
            "Plain".to_string(),
            TypeNode::new(TypeKind::Struct, vec![], vec![], HashSet::new()),
        );
        let graph = TypeGraph::new(types, HashMap::new());

        let bl = build_baseline(&graph, make_timestamp());

        let entry = bl.get_type("Plain").unwrap();
        assert!(entry.trait_impls().is_empty());
    }

    /// AC-07: origin_crate empty string is preserved faithfully.
    #[test]
    fn test_build_baseline_captures_trait_impl_with_empty_origin_crate() {
        let mut node = TypeNode::new(TypeKind::Struct, vec![], vec![], HashSet::new());
        node.set_trait_impls(vec![TraitImplEntry::new("UnknownTrait", vec![])]);
        let mut types = HashMap::new();
        types.insert("Foo".to_string(), node);
        let graph = TypeGraph::new(types, HashMap::new());

        let bl = build_baseline(&graph, make_timestamp());

        let entry = bl.get_type("Foo").unwrap();
        assert_eq!(entry.trait_impls().len(), 1);
        assert_eq!(entry.trait_impls()[0].trait_name(), "UnknownTrait");
        assert_eq!(entry.trait_impls()[0].origin_crate(), "");
    }

    // --- AC-08: baseline functions capture ---

    /// AC-08: free functions in TypeGraph are captured in TypeBaseline::functions
    /// with tuple key converted to fully-qualified string key.
    #[test]
    fn test_build_baseline_captures_functions_with_module_path() {
        let mut functions = HashMap::new();
        let params = vec![ParamDeclaration::new("graph", "TypeGraph")];
        let returns = vec!["TypeBaseline".to_string()];
        let node = FunctionNode::new(params, returns, false, Some("infra::tddd".to_string()));
        functions.insert(("build_baseline".to_string(), Some("infra::tddd".to_string())), node);

        let graph = TypeGraph::with_functions(HashMap::new(), HashMap::new(), functions);
        let bl = build_baseline(&graph, make_timestamp());

        assert_eq!(bl.functions().len(), 1);
        let entry = bl.get_function("infra::tddd::build_baseline").unwrap();
        assert_eq!(entry.params().len(), 1);
        assert_eq!(entry.params()[0].name(), "graph");
        assert_eq!(entry.returns(), &["TypeBaseline"]);
        assert!(!entry.is_async());
        assert_eq!(entry.module_path(), Some("infra::tddd"));
    }

    /// AC-08: free function with no module_path uses short_name as fully-qualified key.
    #[test]
    fn test_build_baseline_captures_top_level_function_without_module_path() {
        let mut functions = HashMap::new();
        let node = FunctionNode::new(vec![], vec!["String".to_string()], true, None);
        functions.insert(("top_fn".to_string(), None), node);

        let graph = TypeGraph::with_functions(HashMap::new(), HashMap::new(), functions);
        let bl = build_baseline(&graph, make_timestamp());

        assert_eq!(bl.functions().len(), 1);
        assert!(bl.has_function("top_fn"));
        let entry = bl.get_function("top_fn").unwrap();
        assert!(entry.is_async());
        assert!(entry.module_path().is_none());
    }

    /// AC-08: empty functions map in TypeGraph produces empty functions in baseline.
    #[test]
    fn test_build_baseline_empty_functions_produces_empty_baseline_functions() {
        let graph = TypeGraph::new(HashMap::new(), HashMap::new());
        let bl = build_baseline(&graph, make_timestamp());
        assert!(bl.functions().is_empty());
    }

    /// AC-08: multiple functions with and without module_path are all captured.
    #[test]
    fn test_build_baseline_captures_multiple_functions() {
        let mut functions = HashMap::new();
        functions.insert(
            ("fn_a".to_string(), Some("crate::mod_a".to_string())),
            FunctionNode::new(vec![], vec![], false, Some("crate::mod_a".to_string())),
        );
        functions
            .insert(("fn_b".to_string(), None), FunctionNode::new(vec![], vec![], false, None));

        let graph = TypeGraph::with_functions(HashMap::new(), HashMap::new(), functions);
        let bl = build_baseline(&graph, make_timestamp());

        assert_eq!(bl.functions().len(), 2);
        assert!(bl.has_function("crate::mod_a::fn_a"));
        assert!(bl.has_function("fn_b"));
    }
}
