//! Builds a [`CodeProfile`] from a [`SchemaExport`].
//!
//! This module is intentionally in the infrastructure layer because it depends on
//! both [`SchemaExport`] (domain) and [`CodeProfile`] (domain), and bridges the
//! flat serializable export into the pre-indexed query structure used by domain
//! evaluation logic.

use std::collections::{HashMap, HashSet};

use domain::schema::{CodeProfile, CodeTrait, CodeType, SchemaExport};

/// Builds a [`CodeProfile`] from a [`SchemaExport`].
///
/// For each type in the schema, collects return type names from all inherent
/// (non-trait) impl methods targeting that type.  Trait impls are excluded so
/// that transition detection focuses on the type's own behaviour.
#[must_use]
pub fn build_code_profile(schema: &SchemaExport) -> CodeProfile {
    let mut types = HashMap::new();

    for type_info in schema.types() {
        let method_return_types: HashSet<String> = schema
            .impls()
            .iter()
            .filter(|i| {
                last_segment(i.target_type()) == type_info.name() && i.trait_name().is_none()
            })
            .flat_map(|i| i.methods())
            .filter(|m| m.has_self_receiver())
            .flat_map(|m| m.return_type_names().iter().cloned())
            .collect();

        types.insert(
            type_info.name().to_string(),
            CodeType::new(
                type_info.kind().clone(),
                type_info.members().to_vec(),
                method_return_types,
            ),
        );
    }

    let mut traits = HashMap::new();
    for trait_info in schema.traits() {
        traits.insert(
            trait_info.name().to_string(),
            CodeTrait::new(trait_info.methods().iter().map(|m| m.name().to_string()).collect()),
        );
    }

    CodeProfile::new(types, traits)
}

/// Extracts the last `::` segment from a path (e.g., `crate::Foo` → `Foo`).
fn last_segment(path: &str) -> &str {
    path.rsplit("::").next().unwrap_or(path)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use domain::schema::{FunctionInfo, ImplInfo, SchemaExport, TraitInfo, TypeInfo, TypeKind};

    use super::*;

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
            vec![FunctionInfo::new(
                "transition".to_string(),
                "fn transition(self) -> Target".to_string(),
                None,
                vec![method_return.to_string()],
                true,
            )],
        )];
        SchemaExport::new("test".to_string(), types, vec![], vec![], impls)
    }

    #[test]
    fn test_build_code_profile_with_struct_type_creates_type_entry() {
        let schema = SchemaExport::new(
            "test".to_string(),
            vec![TypeInfo::new("MyType".to_string(), TypeKind::Struct, None, vec![])],
            vec![],
            vec![],
            vec![],
        );
        let profile = build_code_profile(&schema);
        assert!(profile.has_type("MyType"));
        assert!(!profile.has_type("Missing"));
    }

    #[test]
    fn test_build_code_profile_with_enum_type_preserves_members() {
        let schema = SchemaExport::new(
            "test".to_string(),
            vec![TypeInfo::new(
                "Status".to_string(),
                TypeKind::Enum,
                None,
                vec!["Active".to_string(), "Done".to_string()],
            )],
            vec![],
            vec![],
            vec![],
        );
        let profile = build_code_profile(&schema);
        let code_type = profile.get_type("Status").unwrap();
        assert_eq!(code_type.kind(), &TypeKind::Enum);
        assert_eq!(code_type.members(), &["Active", "Done"]);
    }

    #[test]
    fn test_build_code_profile_with_inherent_impl_collects_return_types() {
        let schema = make_schema_with_impl("Draft", "Published", "Published");
        let profile = build_code_profile(&schema);
        let draft = profile.get_type("Draft").unwrap();
        assert!(draft.method_return_types().contains("Published"));
    }

    #[test]
    fn test_build_code_profile_associated_fn_without_self_excluded_from_transitions() {
        // An associated function (no self receiver) like `fn from_db() -> Published`
        // must NOT appear in method_return_types — it is not a state transition.
        let types = vec![
            TypeInfo::new("Draft".to_string(), TypeKind::Struct, None, vec![]),
            TypeInfo::new("Published".to_string(), TypeKind::Struct, None, vec![]),
        ];
        let impls = vec![ImplInfo::new(
            "Draft".to_string(),
            None,
            vec![FunctionInfo::new(
                "from_db".to_string(),
                "fn from_db() -> Published".to_string(),
                None,
                vec!["Published".to_string()],
                false, // associated function — no self receiver
            )],
        )];
        let schema = SchemaExport::new("test".to_string(), types, vec![], vec![], impls);
        let profile = build_code_profile(&schema);
        let draft = profile.get_type("Draft").unwrap();
        assert!(
            !draft.method_return_types().contains("Published"),
            "associated fn without self receiver must be excluded from transitions"
        );
    }

    #[test]
    fn test_build_code_profile_with_trait_impl_excludes_return_types() {
        let types = vec![TypeInfo::new("Foo".to_string(), TypeKind::Struct, None, vec![])];
        let impls = vec![ImplInfo::new(
            "Foo".to_string(),
            Some("Display".to_string()),
            vec![FunctionInfo::new(
                "fmt".to_string(),
                "fn fmt(&self, f: &mut Formatter) -> fmt::Result".to_string(),
                None,
                vec!["fmt::Result".to_string()],
                true,
            )],
        )];
        let schema = SchemaExport::new("test".to_string(), types, vec![], vec![], impls);
        let profile = build_code_profile(&schema);
        let foo = profile.get_type("Foo").unwrap();
        // trait impls must be excluded — no return types collected
        assert!(foo.method_return_types().is_empty());
    }

    #[test]
    fn test_build_code_profile_with_trait_creates_trait_entry() {
        let trait_info = TraitInfo::new(
            "Repo".to_string(),
            None,
            vec![
                FunctionInfo::new(
                    "save".to_string(),
                    "fn save(&self)".to_string(),
                    None,
                    vec![],
                    true,
                ),
                FunctionInfo::new(
                    "find".to_string(),
                    "fn find(&self)".to_string(),
                    None,
                    vec![],
                    true,
                ),
            ],
        );
        let schema =
            SchemaExport::new("test".to_string(), vec![], vec![], vec![trait_info], vec![]);
        let profile = build_code_profile(&schema);
        let code_trait = profile.get_trait("Repo").unwrap();
        assert_eq!(code_trait.method_names(), &["save", "find"]);
    }

    #[test]
    fn test_build_code_profile_missing_trait_returns_none() {
        let schema = SchemaExport::new("test".to_string(), vec![], vec![], vec![], vec![]);
        let profile = build_code_profile(&schema);
        assert!(profile.get_trait("NonExistent").is_none());
    }
}
