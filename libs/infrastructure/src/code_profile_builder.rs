//! Builds a [`TypeGraph`] from a [`SchemaExport`].
//!
//! This module is intentionally in the infrastructure layer because it depends on
//! both [`SchemaExport`] (domain) and [`TypeGraph`] (domain), and bridges the
//! flat serializable export into the pre-indexed query structure used by domain
//! evaluation logic.
//!
//! T005 (TDDD-01 Phase 1 Task 5): `TypeNode::new` no longer takes a
//! `method_return_types: HashSet<String>` argument — the legacy bridge is
//! gone. `outgoing` is still computed here from `FunctionInfo::return_type_names`
//! ∩ typestate_names.

use std::collections::{HashMap, HashSet};

use domain::schema::{FunctionInfo, SchemaExport, TraitImplEntry, TraitNode, TypeGraph, TypeNode};
use domain::tddd::catalogue::MethodDeclaration;

/// Builds a [`TypeGraph`] from a [`SchemaExport`].
///
/// For each type in the schema, collects return type names from all inherent
/// (non-trait) impl methods targeting that type.  Trait impls are excluded so
/// that transition detection focuses on the type's own behaviour.
///
/// `typestate_names` is the set of type names declared as typestate in the
/// domain-types catalogue.  After building the graph, each node's `outgoing`
/// field is populated with the subset of `method_return_types` that are in
/// `typestate_names`.
#[must_use]
pub fn build_type_graph(schema: &SchemaExport, typestate_names: &HashSet<String>) -> TypeGraph {
    let mut types: HashMap<String, TypeNode> = HashMap::new();

    for type_info in schema.types() {
        let inherent_methods: Vec<&FunctionInfo> = schema
            .impls()
            .iter()
            .filter(|i| base_name(i.target_type()) == type_info.name() && i.trait_name().is_none())
            .flat_map(|i| i.methods())
            .collect();

        let method_decls: Vec<MethodDeclaration> =
            inherent_methods.iter().map(|f| function_info_to_method_decl(f)).collect();

        let outgoing: HashSet<String> = inherent_methods
            .iter()
            .filter(|m| m.has_self_receiver())
            .flat_map(|m| m.return_type_names().iter().cloned())
            .filter(|rtn| typestate_names.contains(rtn.as_str()))
            .collect();

        let name_key = type_info.name().to_string();

        if let Some(existing) = types.get(&name_key) {
            eprintln!(
                "warning: same-name type collision for `{}`: existing={:?}, new={:?} — later entry overwrites earlier",
                name_key,
                existing.module_path(),
                type_info.module_path(),
            );
        }

        // Collect trait impls (separate path — outgoing stays inherent-only)
        let trait_impl_entries: Vec<TraitImplEntry> = schema
            .impls()
            .iter()
            .filter(|i| base_name(i.target_type()) == type_info.name())
            .filter_map(|i| {
                let trait_name = i.trait_name()?;
                let methods = i.methods().iter().map(function_info_to_method_decl).collect();
                Some(TraitImplEntry::new(trait_name, methods))
            })
            .collect();

        let mut node = TypeNode::new(
            type_info.kind().clone(),
            type_info.members().to_vec(),
            method_decls,
            outgoing,
        );
        if !trait_impl_entries.is_empty() {
            node.set_trait_impls(trait_impl_entries);
        }
        if let Some(mp) = type_info.module_path() {
            node.set_module_path(mp.to_string());
        }

        types.insert(name_key, node);
    }

    let mut traits = HashMap::new();
    for trait_info in schema.traits() {
        let method_decls: Vec<MethodDeclaration> =
            trait_info.methods().iter().map(function_info_to_method_decl).collect();
        traits.insert(trait_info.name().to_string(), TraitNode::new(method_decls));
    }

    TypeGraph::new(types, traits)
}

/// Converts a `FunctionInfo` (flat rustdoc-derived) into a `MethodDeclaration`
/// (the structured L1 signature used by `TypeNode::methods` / `TraitNode::methods`).
fn function_info_to_method_decl(f: &FunctionInfo) -> MethodDeclaration {
    MethodDeclaration::new(
        f.name().to_string(),
        f.receiver().map(str::to_string),
        f.params().to_vec(),
        f.returns().to_string(),
        f.is_async(),
    )
}

/// Strips the generic parameter list from a `format_type`-rendered string.
///
/// `format_type` returns short names without `::`, but generic types include
/// angle brackets (e.g., `"Foo<T>"`). Strip the `<...>` suffix so that
/// `"Foo<T>"` matches the `TypeNode` keyed as `"Foo"`.
fn base_name(formatted: &str) -> &str {
    formatted.split('<').next().unwrap_or(formatted)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use domain::schema::{FunctionInfo, ImplInfo, SchemaExport, TraitInfo, TypeInfo, TypeKind};
    use domain::tddd::catalogue::MemberDeclaration;

    use super::*;

    /// Helper: build a `FunctionInfo` for a method returning a single named type.
    fn method_returning(
        name: &str,
        return_name: &str,
        has_self_receiver: bool,
        receiver: Option<&str>,
    ) -> FunctionInfo {
        FunctionInfo::new(
            name.to_string(),
            None,
            vec![return_name.to_string()],
            has_self_receiver,
            vec![],
            return_name.to_string(),
            receiver.map(str::to_string),
            false,
        )
    }

    fn make_schema_with_impl(
        type_name: &str,
        target_type: &str,
        method_return: &str,
    ) -> SchemaExport {
        let types = vec![
            TypeInfo::new(type_name.to_string(), TypeKind::Struct, None, vec![]),
            TypeInfo::new(target_type.to_string(), TypeKind::Struct, None, vec![]),
        ];
        let impls = vec![ImplInfo::new(
            type_name.to_string(),
            None,
            vec![method_returning("transition", method_return, true, Some("self"))],
        )];
        SchemaExport::new("test".to_string(), types, vec![], vec![], impls)
    }

    #[test]
    fn test_build_type_graph_with_struct_type_creates_type_entry() {
        let schema = SchemaExport::new(
            "test".to_string(),
            vec![TypeInfo::new("MyType".to_string(), TypeKind::Struct, None, vec![])],
            vec![],
            vec![],
            vec![],
        );
        let profile = build_type_graph(&schema, &HashSet::new());
        assert!(profile.has_type("MyType"));
        assert!(!profile.has_type("Missing"));
    }

    #[test]
    fn test_build_type_graph_with_enum_type_preserves_members() {
        let schema = SchemaExport::new(
            "test".to_string(),
            vec![TypeInfo::new(
                "Status".to_string(),
                TypeKind::Enum,
                None,
                vec![MemberDeclaration::variant("Active"), MemberDeclaration::variant("Done")],
            )],
            vec![],
            vec![],
            vec![],
        );
        let profile = build_type_graph(&schema, &HashSet::new());
        let code_type = profile.get_type("Status").unwrap();
        assert_eq!(code_type.kind(), &TypeKind::Enum);
        let names: Vec<&str> = code_type.members().iter().map(|m| m.name()).collect();
        assert_eq!(names, vec!["Active", "Done"]);
    }

    #[test]
    fn test_build_type_graph_with_inherent_impl_collects_method_returns() {
        let schema = make_schema_with_impl("Draft", "Published", "Published");
        let typestates = HashSet::from(["Draft".to_string(), "Published".to_string()]);
        let profile = build_type_graph(&schema, &typestates);
        let draft = profile.get_type("Draft").unwrap();
        assert!(draft.outgoing().contains("Published"));
    }

    #[test]
    fn test_build_type_graph_with_inherent_impl_populates_method_decls() {
        let schema = make_schema_with_impl("Draft", "Published", "Published");
        let profile = build_type_graph(&schema, &HashSet::new());
        let draft = profile.get_type("Draft").unwrap();
        assert_eq!(draft.methods().len(), 1);
        let transition = &draft.methods()[0];
        assert_eq!(transition.name(), "transition");
        assert_eq!(transition.returns(), "Published");
        assert_eq!(transition.receiver(), Some("self"));
    }

    #[test]
    fn test_build_type_graph_associated_fn_without_self_excluded_from_transitions() {
        let types = vec![
            TypeInfo::new("Draft".to_string(), TypeKind::Struct, None, vec![]),
            TypeInfo::new("Published".to_string(), TypeKind::Struct, None, vec![]),
        ];
        let impls = vec![ImplInfo::new(
            "Draft".to_string(),
            None,
            vec![method_returning("from_db", "Published", false, None)],
        )];
        let schema = SchemaExport::new("test".to_string(), types, vec![], vec![], impls);
        let typestates = HashSet::from(["Published".to_string()]);
        let profile = build_type_graph(&schema, &typestates);
        let draft = profile.get_type("Draft").unwrap();
        assert!(
            !draft.outgoing().contains("Published"),
            "associated fn without self receiver must be excluded from transitions"
        );
    }

    #[test]
    fn test_build_type_graph_with_trait_impl_excludes_outgoing() {
        let types = vec![TypeInfo::new("Foo".to_string(), TypeKind::Struct, None, vec![])];
        let impls = vec![ImplInfo::new(
            "Foo".to_string(),
            Some("Display".to_string()),
            vec![method_returning("fmt", "Result", true, Some("&self"))],
        )];
        let schema = SchemaExport::new("test".to_string(), types, vec![], vec![], impls);
        let typestates = HashSet::from(["Result".to_string()]);
        let profile = build_type_graph(&schema, &typestates);
        let foo = profile.get_type("Foo").unwrap();
        assert!(foo.outgoing().is_empty());
    }

    #[test]
    fn test_build_type_graph_with_trait_creates_trait_entry() {
        let trait_info = TraitInfo::new(
            "Repo".to_string(),
            None,
            vec![
                method_returning("save", "()", true, Some("&self")),
                method_returning("find", "()", true, Some("&self")),
            ],
        );
        let schema =
            SchemaExport::new("test".to_string(), vec![], vec![], vec![trait_info], vec![]);
        let profile = build_type_graph(&schema, &HashSet::new());
        let code_trait = profile.get_trait("Repo").unwrap();
        let names: Vec<&str> = code_trait.methods().iter().map(|m| m.name()).collect();
        assert_eq!(names, vec!["save", "find"]);
        assert_eq!(code_trait.methods().len(), 2);
    }

    #[test]
    fn test_build_type_graph_missing_trait_returns_none() {
        let schema = SchemaExport::new("test".to_string(), vec![], vec![], vec![], vec![]);
        let profile = build_type_graph(&schema, &HashSet::new());
        assert!(profile.get_trait("NonExistent").is_none());
    }

    // --- T004 TDDD-05: trait impl collection ---

    #[test]
    fn test_build_type_graph_trait_impl_populated() {
        let types = vec![TypeInfo::new("FsStore".to_string(), TypeKind::Struct, None, vec![])];
        let impls = vec![ImplInfo::new(
            "FsStore".to_string(),
            Some("TrackReader".to_string()),
            vec![method_returning("read", "()", true, Some("&self"))],
        )];
        let schema = SchemaExport::new("test".to_string(), types, vec![], vec![], impls);
        let profile = build_type_graph(&schema, &HashSet::new());
        let node = profile.get_type("FsStore").unwrap();
        assert_eq!(node.trait_impls().len(), 1);
        assert_eq!(node.trait_impls()[0].trait_name(), "TrackReader");
        assert_eq!(node.trait_impls()[0].methods().len(), 1);
        assert_eq!(node.trait_impls()[0].methods()[0].name(), "read");
    }

    #[test]
    fn test_build_type_graph_outgoing_unaffected_by_trait_impl() {
        // Trait impls must NOT pollute outgoing (inherent-only invariant)
        let types = vec![
            TypeInfo::new("FsStore".to_string(), TypeKind::Struct, None, vec![]),
            TypeInfo::new("Published".to_string(), TypeKind::Struct, None, vec![]),
        ];
        let impls = vec![ImplInfo::new(
            "FsStore".to_string(),
            Some("TrackReader".to_string()),
            vec![method_returning("read", "Published", true, Some("&self"))],
        )];
        let schema = SchemaExport::new("test".to_string(), types, vec![], vec![], impls);
        let typestates = HashSet::from(["Published".to_string()]);
        let profile = build_type_graph(&schema, &typestates);
        let node = profile.get_type("FsStore").unwrap();
        assert!(node.outgoing().is_empty(), "trait impl return types must not appear in outgoing");
        assert_eq!(node.trait_impls().len(), 1, "trait impl must still be collected");
    }

    #[test]
    fn test_build_type_graph_multiple_trait_impls_on_same_type() {
        let types = vec![TypeInfo::new("FsStore".to_string(), TypeKind::Struct, None, vec![])];
        let impls = vec![
            ImplInfo::new(
                "FsStore".to_string(),
                Some("TrackReader".to_string()),
                vec![method_returning("read", "()", true, Some("&self"))],
            ),
            ImplInfo::new(
                "FsStore".to_string(),
                Some("TrackWriter".to_string()),
                vec![method_returning("write", "()", true, Some("&self"))],
            ),
        ];
        let schema = SchemaExport::new("test".to_string(), types, vec![], vec![], impls);
        let profile = build_type_graph(&schema, &HashSet::new());
        let node = profile.get_type("FsStore").unwrap();
        assert_eq!(node.trait_impls().len(), 2);
        let trait_names: Vec<&str> = node.trait_impls().iter().map(|t| t.trait_name()).collect();
        assert!(trait_names.contains(&"TrackReader"));
        assert!(trait_names.contains(&"TrackWriter"));
    }

    #[test]
    fn test_build_type_graph_outgoing_contains_only_typestate_targets() {
        let types = vec![
            TypeInfo::new("Draft".to_string(), TypeKind::Struct, None, vec![]),
            TypeInfo::new("Published".to_string(), TypeKind::Struct, None, vec![]),
            TypeInfo::new("Archived".to_string(), TypeKind::Struct, None, vec![]),
        ];
        let impls = vec![ImplInfo::new(
            "Draft".to_string(),
            None,
            vec![
                method_returning("publish", "Published", true, Some("self")),
                method_returning("archive", "Archived", true, Some("self")),
            ],
        )];
        let schema = SchemaExport::new("test".to_string(), types, vec![], vec![], impls);

        let mut typestate_names = HashSet::new();
        typestate_names.insert("Published".to_string());

        let profile = build_type_graph(&schema, &typestate_names);
        let draft = profile.get_type("Draft").unwrap();

        assert!(draft.outgoing().contains("Published"), "Published must be in outgoing");
        assert!(
            !draft.outgoing().contains("Archived"),
            "Archived must not be in outgoing (not a typestate)"
        );
    }
}
