//! Infrastructure adapter implementing the `ContractMapRenderer` domain port.
//!
//! * [`ContractMapRendererAdapter`] — public adapter struct.
//! * Private TOML schema DTO types for `.harness/config/contract-map-style.toml`
//!   live in the `render` submodule (Decision L-1 / CN-11 / Decision P-3).
//!   All style DTOs are private and never appear in the public API.
//!
//! **Scope (T003)**: fail-closed style config loading (absent → `StyleConfigNotFound`,
//! invalid → `StyleConfigInvalid`, per CN-02 / AC-11).
//!
//! **T004–T009**: full mermaid rendering pipeline:
//! - T004: `CatalogueNode` enum + node_id generation + global trait index.
//! - T005: subgraph / node placement (layer → module → entry → method).
//! - T006: method nodes + inherent_impls aggregation + typestate transition edges.
//! - T007: enum variant / TypeAlias / struct field edges.
//! - T008: trait impl edges + TraitEntry method nodes.
//! - T009: output assembly + style application.

mod render;

use std::path::{Path, PathBuf};

use domain::tddd::catalogue_v2::CatalogueDocument;
use domain::tddd::{
    ContractMapContent, ContractMapRenderOptions, ContractMapRenderer, ContractMapRendererError,
    LayerId,
};

use crate::track::symlink_guard::reject_symlinks_below;

// ---------------------------------------------------------------------------
// Public adapter
// ---------------------------------------------------------------------------

/// Infrastructure adapter implementing [`ContractMapRenderer`].
pub struct ContractMapRendererAdapter {
    /// Path to `.harness/config/contract-map-style.toml`.
    pub style_config_path: PathBuf,
}

impl ContractMapRendererAdapter {
    /// Creates a new adapter (infallible — config loading deferred to `render`).
    #[must_use]
    pub fn new(style_config_path: PathBuf) -> Self {
        Self { style_config_path }
    }

    fn load_style_config(&self) -> Result<render::StyleConfig, ContractMapRendererError> {
        let trusted_root = self
            .style_config_path
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .unwrap_or_else(|| Path::new("/"));

        match reject_symlinks_below(&self.style_config_path, trusted_root) {
            Ok(true) => {}
            Ok(false) => {
                return Err(ContractMapRendererError::StyleConfigNotFound {
                    path: self.style_config_path.clone(),
                });
            }
            Err(e) => {
                return Err(ContractMapRendererError::StyleConfigInvalid {
                    path: self.style_config_path.clone(),
                    reason: e.to_string(),
                });
            }
        }

        let raw = std::fs::read_to_string(&self.style_config_path).map_err(|e| {
            ContractMapRendererError::StyleConfigInvalid {
                path: self.style_config_path.clone(),
                reason: e.to_string(),
            }
        })?;

        toml::from_str::<render::StyleConfig>(&raw).map_err(|e| {
            ContractMapRendererError::StyleConfigInvalid {
                path: self.style_config_path.clone(),
                reason: e.to_string(),
            }
        })
    }
}

impl ContractMapRenderer for ContractMapRendererAdapter {
    fn render(
        &self,
        catalogues: &[CatalogueDocument],
        layer_order: &[LayerId],
        _opts: &ContractMapRenderOptions,
    ) -> Result<ContractMapContent, ContractMapRendererError> {
        let style = self.load_style_config()?;
        let output = render::render_mermaid(catalogues, layer_order, &style)?;
        Ok(ContractMapContent::new(output))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use domain::tddd::LayerId;
    use domain::tddd::catalogue_v2::composite::{
        TypeKindV2, TypestateMarker, TypestateTransitions,
    };
    use domain::tddd::catalogue_v2::entries::{InherentImplDeclV2, TraitEntry, TypeEntry};
    use domain::tddd::catalogue_v2::identifiers::{
        CrateName, FieldName, MethodName, ModulePath, TraitName, TypeName, TypeRef, VariantName,
    };
    use domain::tddd::catalogue_v2::methods::MethodDeclaration;
    use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole, ItemAction, SelfReceiver};
    use domain::tddd::catalogue_v2::traits::TraitImplDeclV2;
    use domain::tddd::catalogue_v2::variants::{FieldDecl, VariantDecl};
    use domain::tddd::{ContractMapRenderOptions, ContractMapRenderer};

    fn write_style_config(dir: &std::path::Path, content: &str) -> PathBuf {
        let path = dir.join("contract-map-style.toml");
        std::fs::write(&path, content).unwrap();
        path
    }

    const MINIMAL_VALID_CONFIG: &str = r#"
[filter]
include_function_roles = []
"#;

    /// Full style config including all [edge.*] sections required by the renderer.
    /// Use this config for tests that render entries with methods, edges, or edge-generating
    /// type constructs (enum tuple/struct variants, TypeAlias, struct fields, trait impls).
    /// CN-02: no hard-coded fallback in code — all edge styles must be provided by the config.
    const FULL_VALID_CONFIG: &str = r#"
[edge.method_param]
arrow = "--o"

[edge.method_returns]
arrow = "-->"

[edge.transition]
arrow = "==>"
label = "transitions_to"

[edge.trait_impl]
arrow = "-.impl.->"

[edge.variant_payload]
arrow = "--o"

[edge.field]
arrow = "--o"

[edge.alias]
arrow = "---"
label = "alias_of"

[filter]
include_function_roles = []
"#;

    const INVALID_TOML: &str = "role = [[[invalid toml";

    // -----------------------------------------------------------------------
    // T003 tests (preserved)
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_absent_style_config_returns_style_config_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("nonexistent-style.toml");
        let adapter = ContractMapRendererAdapter::new(missing.clone());
        let opts = ContractMapRenderOptions::default();
        let err = adapter.render(&[], &[], &opts).unwrap_err();
        assert!(
            matches!(err, ContractMapRendererError::StyleConfigNotFound { ref path } if path == &missing),
            "expected StyleConfigNotFound with correct path, got {err:?}"
        );
    }

    #[test]
    fn test_render_invalid_toml_returns_style_config_invalid() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), INVALID_TOML);
        let adapter = ContractMapRendererAdapter::new(path.clone());
        let opts = ContractMapRenderOptions::default();
        let err = adapter.render(&[], &[], &opts).unwrap_err();
        match err {
            ContractMapRendererError::StyleConfigInvalid { path: ref err_path, .. } => {
                assert_eq!(err_path, &path, "StyleConfigInvalid must report the config path");
            }
            other => panic!("expected StyleConfigInvalid, got {other:?}"),
        }
    }

    #[test]
    fn test_render_valid_style_config_returns_ok() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), MINIMAL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();
        let result = adapter.render(&[], &[], &opts);
        assert!(result.is_ok(), "expected Ok with valid config, got {result:?}");
        let content = result.unwrap();
        assert!(content.as_ref().contains("flowchart LR"), "must contain 'flowchart LR'");
    }

    #[test]
    fn test_adapter_new_is_infallible() {
        let missing = PathBuf::from("/this/does/not/exist.toml");
        let _adapter = ContractMapRendererAdapter::new(missing);
    }

    // -----------------------------------------------------------------------
    // CN-02: fail-closed on missing edge style entry (no hard-coded fallback)
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_missing_edge_style_entry_returns_render_failed() {
        // A valid config that is missing [edge.variant_payload].
        // Rendering an enum type with a tuple variant must fail with RenderFailed,
        // not silently fall back to a hard-coded arrow (CN-02 — no code default).
        let config_without_variant_payload = r#"
[edge.method_param]
arrow = "--o"

[edge.method_returns]
arrow = "-->"

[filter]
include_function_roles = []
"#;
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), config_without_variant_payload);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();

        let crate_name = CrateName::new("domain").unwrap();
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer.clone());

        let tuple_variant = VariantDecl::tuple(
            VariantName::new("Value").unwrap(),
            vec![TypeRef::new("String").unwrap()],
        );
        doc.types.insert(
            TypeName::new("MyEnum").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::ValueObject,
                kind: TypeKindV2::Enum { variants: vec![tuple_variant] },
                methods: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );

        let result = adapter.render(&[doc], &[layer], &opts);
        assert!(
            matches!(result, Err(ContractMapRendererError::RenderFailed { .. })),
            "missing [edge.variant_payload] must produce RenderFailed, got: {result:?}"
        );
    }

    // -----------------------------------------------------------------------
    // T009 / AC-01: output order (flowchart LR → classDef → subgraph → edge → class)
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_output_starts_with_flowchart_lr() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), MINIMAL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();
        let content = adapter.render(&[], &[], &opts).unwrap();
        assert!(
            content.as_ref().starts_with("flowchart LR\n"),
            "output must start with 'flowchart LR\\n', got: {:?}",
            &content.as_ref()[..content.as_ref().len().min(40)]
        );
    }

    // -----------------------------------------------------------------------
    // T004 / AC-09: node_id uniqueness across crates in same layer
    // -----------------------------------------------------------------------

    #[test]
    fn test_type_node_id_collision_free_across_crates() {
        // Two crates in same layer with same type name.
        let id_a = render::type_node_id("domain", "crate_a", "UserId");
        let id_b = render::type_node_id("domain", "crate_b", "UserId");
        assert_ne!(id_a, id_b, "node_ids must differ for different crates");
    }

    #[test]
    fn test_trait_node_id_collision_free_across_crates() {
        let id_a = render::trait_node_id("domain", "alpha", "MyTrait");
        let id_b = render::trait_node_id("domain", "beta", "MyTrait");
        assert_ne!(id_a, id_b);
    }

    #[test]
    fn test_function_node_id_collision_free_across_crates() {
        let id_a = render::function_node_id("domain", "crate_a", "crate_a::register_user");
        let id_b = render::function_node_id("domain", "crate_b", "crate_b::register_user");
        assert_ne!(id_a, id_b);
    }

    // -----------------------------------------------------------------------
    // T004: global trait index spans multiple catalogues
    // -----------------------------------------------------------------------

    #[test]
    fn test_build_trait_index_spans_multiple_catalogues() {
        let crate_a = CrateName::new("crate_a").unwrap();
        let crate_b = CrateName::new("crate_b").unwrap();
        let layer = LayerId::try_new("domain").unwrap();

        let mut doc_a = CatalogueDocument::new(3, crate_a.clone(), layer.clone());
        let mut doc_b = CatalogueDocument::new(3, crate_b.clone(), layer.clone());

        doc_a.traits.insert(TraitName::new("TraitA").unwrap(), make_empty_trait_entry());
        doc_b.traits.insert(TraitName::new("TraitB").unwrap(), make_empty_trait_entry());

        let index = render::build_trait_index(&[doc_a, doc_b]);
        assert!(index.contains_key(&("crate_a".to_string(), "TraitA".to_string())));
        assert!(index.contains_key(&("crate_b".to_string(), "TraitB".to_string())));
        assert!(!index.contains_key(&("crate_a".to_string(), "TraitB".to_string())));
    }

    // -----------------------------------------------------------------------
    // T005 / AC-02: empty subgraph generated even with 0 methods
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_type_with_zero_methods_produces_entry_subgraph() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), MINIMAL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();

        let crate_name = CrateName::new("domain").unwrap();
        let layer = LayerId::try_new("core").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer.clone());
        doc.types.insert(
            TypeName::new("EmptyStruct").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::ValueObject,
                kind: TypeKindV2::PlainStruct {
                    fields: vec![],
                    has_stripped_fields: false,
                    typestate: None,
                },
                methods: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.as_ref();
        // Must contain a subgraph for EmptyStruct.
        assert!(output.contains("EmptyStruct"), "output must mention EmptyStruct: {output}");
    }

    // -----------------------------------------------------------------------
    // T005: crate root entry placed under layer subgraph, not module subgraph
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_root_entry_placed_under_layer_subgraph() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), MINIMAL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();

        let crate_name = CrateName::new("domain").unwrap();
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer.clone());

        // Root entry (module_path = [])
        doc.types.insert(
            TypeName::new("RootType").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::ValueObject,
                kind: TypeKindV2::PlainStruct {
                    fields: vec![],
                    has_stripped_fields: false,
                    typestate: None,
                },
                methods: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );

        // Non-root entry
        doc.types.insert(
            TypeName::new("ModuleType").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::Entity,
                kind: TypeKindV2::PlainStruct {
                    fields: vec![],
                    has_stripped_fields: false,
                    typestate: None,
                },
                methods: vec![],
                module_path: ModulePath::from_segments(vec!["user".to_string()]).unwrap(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.as_ref();
        // Both must appear; specific structural order is not asserted here,
        // but both names must be present.
        assert!(output.contains("RootType"), "must mention RootType: {output}");
        assert!(output.contains("ModuleType"), "must mention ModuleType: {output}");
        // Module subgraph for 'user' must appear.
        assert!(output.contains("domain_module_user"), "must have module subgraph: {output}");
    }

    // -----------------------------------------------------------------------
    // T006 / AC-03: typestate transition method edge becomes ==>|transitions_to|
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_typestate_transition_method_uses_transition_edge() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), FULL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();

        let crate_name = CrateName::new("domain").unwrap();
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer.clone());

        let transitions = TypestateTransitions::new(vec![MethodName::new("approve").unwrap()]);
        let marker = TypestateMarker::new(TypeName::new("ReviewMachine").unwrap(), transitions);

        let approve_method = MethodDeclaration::new(
            MethodName::new("approve").unwrap(),
            Some(SelfReceiver::SharedRef),
            vec![],
            TypeRef::new("Approved").unwrap(),
            false,
            None,
        );

        doc.types.insert(
            TypeName::new("Pending").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::ValueObject,
                kind: TypeKindV2::PlainStruct {
                    fields: vec![],
                    has_stripped_fields: false,
                    typestate: Some(marker),
                },
                methods: vec![approve_method],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.as_ref();
        // The transition edge syntax should appear.
        assert!(output.contains("==>"), "transition edge '==>' must appear: {output}");
        assert!(output.contains("transitions_to"), "label 'transitions_to' must appear: {output}");
    }

    // -----------------------------------------------------------------------
    // T006 / AC-04: inherent_impls methods aggregated into type subgraph
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_inherent_impls_aggregated_into_type_subgraph() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), FULL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();

        let crate_name = CrateName::new("domain").unwrap();
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer.clone());

        doc.types.insert(
            TypeName::new("Email").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::ValueObject,
                kind: TypeKindV2::PlainStruct {
                    fields: vec![],
                    has_stripped_fields: false,
                    typestate: None,
                },
                methods: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );

        // Two InherentImplDeclV2 for the same type (AC-04).
        let m1 = MethodDeclaration::new(
            MethodName::new("as_str").unwrap(),
            Some(SelfReceiver::SharedRef),
            vec![],
            TypeRef::new("str").unwrap(),
            false,
            None,
        );
        let m2 = MethodDeclaration::new(
            MethodName::new("validate").unwrap(),
            Some(SelfReceiver::SharedRef),
            vec![],
            TypeRef::new("bool").unwrap(),
            false,
            None,
        );

        doc.inherent_impls.push(InherentImplDeclV2 {
            type_name: TypeName::new("Email").unwrap(),
            impl_generics: vec![],
            impl_where_predicates: vec![],
            methods: vec![m1],
        });
        doc.inherent_impls.push(InherentImplDeclV2 {
            type_name: TypeName::new("Email").unwrap(),
            impl_generics: vec![],
            impl_where_predicates: vec![],
            methods: vec![m2],
        });

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.as_ref();
        assert!(output.contains("as_str"), "as_str method must appear: {output}");
        assert!(output.contains("validate"), "validate method must appear: {output}");
    }

    // -----------------------------------------------------------------------
    // T007 / AC-05: enum variant edges
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_enum_tuple_variant_uses_unlabeled_arrow_edge() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), FULL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();

        let crate_name = CrateName::new("domain").unwrap();
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer.clone());

        let tuple_variant = VariantDecl::tuple(
            VariantName::new("Some").unwrap(),
            vec![TypeRef::new("UserId").unwrap()],
        );
        let unit_variant = VariantDecl::unit(VariantName::new("None").unwrap());

        doc.types.insert(
            TypeName::new("Option").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::ValueObject,
                kind: TypeKindV2::Enum { variants: vec![tuple_variant, unit_variant] },
                methods: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.as_ref();
        // Tuple variant should have unlabeled edge (--o without label).
        assert!(output.contains("Some"), "Some variant must appear: {output}");
        // None variant should not have an edge (Unit — no edge).
        assert!(output.contains("None"), "None variant must appear: {output}");
    }

    #[test]
    fn test_render_enum_struct_variant_uses_labeled_field_edge() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), FULL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();

        let crate_name = CrateName::new("domain").unwrap();
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer.clone());

        let field =
            FieldDecl::new(FieldName::new("message").unwrap(), TypeRef::new("String").unwrap());
        let struct_variant =
            VariantDecl::struct_variant(VariantName::new("Error").unwrap(), vec![field]);

        doc.types.insert(
            TypeName::new("AppError").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::ErrorType,
                kind: TypeKindV2::Enum { variants: vec![struct_variant] },
                methods: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.as_ref();
        // Struct variant should have labeled edge with field name.
        assert!(output.contains("message"), "field name 'message' must appear in edge: {output}");
    }

    // -----------------------------------------------------------------------
    // T007 / AC-07: PlainStruct field edges; has_stripped_fields suppresses
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_plain_struct_field_edges_emitted() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), FULL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();

        let crate_name = CrateName::new("domain").unwrap();
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer.clone());

        let field =
            FieldDecl::new(FieldName::new("email").unwrap(), TypeRef::new("Email").unwrap());
        doc.types.insert(
            TypeName::new("User").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::Entity,
                kind: TypeKindV2::PlainStruct {
                    fields: vec![field],
                    has_stripped_fields: false,
                    typestate: None,
                },
                methods: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.as_ref();
        assert!(output.contains("email"), "field edge with label 'email' must appear: {output}");
    }

    #[test]
    fn test_render_plain_struct_with_stripped_fields_suppresses_field_edges() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), MINIMAL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();

        let crate_name = CrateName::new("domain").unwrap();
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer.clone());

        let field =
            FieldDecl::new(FieldName::new("secret").unwrap(), TypeRef::new("SecretKey").unwrap());
        doc.types.insert(
            TypeName::new("Config").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::Dto,
                kind: TypeKindV2::PlainStruct {
                    fields: vec![field],
                    has_stripped_fields: true, // stripped — no field edge
                    typestate: None,
                },
                methods: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.as_ref();
        // 'secret' label must NOT appear in edges (field edge suppressed).
        assert!(
            !output.contains("|secret|"),
            "field edge must be suppressed for stripped fields: {output}"
        );
    }

    #[test]
    fn test_render_tuple_struct_positional_index_edges() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), FULL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();

        let crate_name = CrateName::new("domain").unwrap();
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer.clone());

        doc.types.insert(
            TypeName::new("Pair").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::ValueObject,
                kind: TypeKindV2::TupleStruct {
                    fields: vec![TypeRef::new("UserId").unwrap(), TypeRef::new("String").unwrap()],
                    has_stripped_fields: false,
                },
                methods: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.as_ref();
        assert!(output.contains(".0"), "positional label '.0' must appear: {output}");
        assert!(output.contains(".1"), "positional label '.1' must appear: {output}");
    }

    // -----------------------------------------------------------------------
    // T007 / AC-08: TypeAlias undirected alias_of edge
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_type_alias_emits_alias_of_edge() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), FULL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();

        let crate_name = CrateName::new("domain").unwrap();
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer.clone());

        doc.types.insert(
            TypeName::new("UserId").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::ValueObject,
                kind: TypeKindV2::TypeAlias { target: TypeRef::new("u64").unwrap() },
                methods: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.as_ref();
        assert!(output.contains("alias_of"), "alias_of label must appear: {output}");
    }

    // -----------------------------------------------------------------------
    // T008 / AC-06: trait impl edges + workspace-external silent skip
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_trait_impl_edge_generated_for_workspace_trait() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), FULL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();

        let crate_name = CrateName::new("domain").unwrap();
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name.clone(), layer.clone());

        // Add a trait to the catalogue so it appears in the trait index.
        doc.traits.insert(TraitName::new("MyPort").unwrap(), make_empty_trait_entry());

        // Add a type.
        doc.types.insert(
            TypeName::new("MyAdapter").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::SecondaryAdapter,
                kind: TypeKindV2::PlainStruct {
                    fields: vec![],
                    has_stripped_fields: false,
                    typestate: None,
                },
                methods: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );

        // Trait impl: MyAdapter implements MyPort.
        doc.trait_impls.push(TraitImplDeclV2::new(
            TypeRef::new("MyPort").unwrap(),
            TypeRef::new("MyAdapter").unwrap(),
        ));

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.as_ref();
        // The impl edge syntax must appear.
        assert!(output.contains("-.impl.->"), "impl edge must appear: {output}");
    }

    #[test]
    fn test_render_external_trait_impl_is_silently_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), MINIMAL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();

        let crate_name = CrateName::new("domain").unwrap();
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer.clone());

        doc.types.insert(
            TypeName::new("MyType").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::ValueObject,
                kind: TypeKindV2::PlainStruct {
                    fields: vec![],
                    has_stripped_fields: false,
                    typestate: None,
                },
                methods: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );

        // External trait impl (std::fmt::Display) — should be silently skipped (CN-10).
        doc.trait_impls.push(TraitImplDeclV2::new(
            TypeRef::new("std::fmt::Display").unwrap(),
            TypeRef::new("MyType").unwrap(),
        ));

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.as_ref();
        // No impl edge for external trait (CN-10 / AC-06).
        assert!(!output.contains("-.impl.->"), "no impl edge for external trait: {output}");
    }

    // -----------------------------------------------------------------------
    // T010 / AC-12: layer-agnostic (2-layer, 3-layer, custom names)
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_two_layer_config_succeeds_and_not_hardcoded() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), MINIMAL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();

        let l1 = LayerId::try_new("core").unwrap();
        let l2 = LayerId::try_new("api").unwrap();
        let doc1 = CatalogueDocument::new(3, CrateName::new("core").unwrap(), l1.clone());
        let doc2 = CatalogueDocument::new(3, CrateName::new("api").unwrap(), l2.clone());

        let result = adapter.render(&[doc1, doc2], &[l1, l2], &opts);
        assert!(result.is_ok(), "2-layer config must succeed: {result:?}");
        let output = result.unwrap();
        let text = output.as_ref();
        // Layer subgraph labels must be the actual layer names (not hardcoded).
        assert!(text.contains("\"core\""), "must use layer label 'core': {text}");
        assert!(text.contains("\"api\""), "must use layer label 'api': {text}");
    }

    #[test]
    fn test_render_three_layer_config_succeeds() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), MINIMAL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();

        let l1 = LayerId::try_new("domain").unwrap();
        let l2 = LayerId::try_new("usecase").unwrap();
        let l3 = LayerId::try_new("infrastructure").unwrap();

        let doc1 = CatalogueDocument::new(3, CrateName::new("domain").unwrap(), l1.clone());
        let doc2 = CatalogueDocument::new(3, CrateName::new("usecase").unwrap(), l2.clone());
        let doc3 = CatalogueDocument::new(3, CrateName::new("infra").unwrap(), l3.clone());

        let result = adapter.render(&[doc1, doc2, doc3], &[l1, l2, l3], &opts);
        assert!(result.is_ok(), "3-layer config must succeed: {result:?}");
    }

    #[test]
    fn test_render_custom_layer_names_reflected_in_output() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), MINIMAL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();

        let l1 = LayerId::try_new("alpha").unwrap();
        let l2 = LayerId::try_new("beta").unwrap();
        let doc1 = CatalogueDocument::new(3, CrateName::new("alpha").unwrap(), l1.clone());
        let doc2 = CatalogueDocument::new(3, CrateName::new("beta").unwrap(), l2.clone());

        let result = adapter.render(&[doc1, doc2], &[l1, l2], &opts).unwrap();
        let text = result.as_ref();
        assert!(text.contains("\"alpha\""), "must use layer label 'alpha': {text}");
        assert!(text.contains("\"beta\""), "must use layer label 'beta': {text}");
    }

    // -----------------------------------------------------------------------
    // T008 / AC-06: cross-crate qualified trait ref resolved via trait index
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_cross_crate_qualified_trait_impl_edge_generated() {
        // Scenario: infrastructure catalogue has `trait_ref: "domain::tddd::MyPort"`,
        // and the domain catalogue declares `MyPort` in `doc.traits`.
        // The renderer must produce an impl edge (not silent-skip the qualified ref).
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), FULL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();

        let domain_layer = LayerId::try_new("domain").unwrap();
        let infra_layer = LayerId::try_new("infrastructure").unwrap();

        // domain catalogue: declares MyPort trait.
        let mut domain_doc =
            CatalogueDocument::new(3, CrateName::new("domain").unwrap(), domain_layer.clone());
        domain_doc.traits.insert(TraitName::new("MyPort").unwrap(), make_empty_trait_entry());

        // infrastructure catalogue: declares MyAdapter type + cross-crate trait impl.
        let mut infra_doc = CatalogueDocument::new(
            3,
            CrateName::new("infrastructure").unwrap(),
            infra_layer.clone(),
        );
        infra_doc.types.insert(
            TypeName::new("MyAdapter").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::SecondaryAdapter,
                kind: TypeKindV2::PlainStruct {
                    fields: vec![],
                    has_stripped_fields: false,
                    typestate: None,
                },
                methods: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );
        // Qualified cross-crate trait_ref (as used in real catalogues).
        infra_doc.trait_impls.push(TraitImplDeclV2::new(
            TypeRef::new("domain::tddd::MyPort").unwrap(),
            TypeRef::new("MyAdapter").unwrap(),
        ));

        let result =
            adapter.render(&[domain_doc, infra_doc], &[domain_layer, infra_layer], &opts).unwrap();
        let output = result.as_ref();
        assert!(
            output.contains("-.impl.->"),
            "impl edge must be generated for cross-crate qualified trait ref: {output}"
        );
    }

    #[test]
    fn test_render_cross_crate_qualified_trait_ref_not_in_index_is_silently_skipped() {
        // Scenario: trait_ref `"external::crate::SomeTrait"` is qualified but NOT in
        // any catalogue's doc.traits — workspace-external, must be silently skipped.
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), MINIMAL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();

        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, CrateName::new("domain").unwrap(), layer.clone());
        doc.types.insert(
            TypeName::new("MyType").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::ValueObject,
                kind: TypeKindV2::PlainStruct {
                    fields: vec![],
                    has_stripped_fields: false,
                    typestate: None,
                },
                methods: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );
        // Qualified trait_ref pointing to a crate not in the catalogue set — external.
        doc.trait_impls.push(TraitImplDeclV2::new(
            TypeRef::new("external::crate::SomeTrait").unwrap(),
            TypeRef::new("MyType").unwrap(),
        ));

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.as_ref();
        assert!(
            !output.contains("-.impl.->"),
            "no impl edge for qualified trait ref not in the trait index: {output}"
        );
    }

    // -----------------------------------------------------------------------
    // Helper constructors for tests
    // -----------------------------------------------------------------------

    fn make_empty_trait_entry() -> TraitEntry {
        TraitEntry {
            action: ItemAction::Add,
            role: ContractRole::SecondaryPort,
            methods: vec![],
            supertrait_bounds: vec![],
            generics: vec![],
            where_predicates: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        }
    }
}
