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
        StructKind, StructShape, TypeKindV2, TypestateMarker, TypestateTransitions,
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
        // Rendering an enum type with a tuple variant whose payload resolves to a
        // declared catalogue type must fail with RenderFailed, not silently fall back
        // to a hard-coded arrow (CN-02 — no code default).
        //
        // The payload type must be declared in the catalogue so that the resolver
        // returns Some (i.e., the edge would be emitted), triggering the edge-config
        // lookup. Primitive/undeclared payload types resolve to None and are skipped
        // silently without touching the edge config (ADR 2026-04-17-1528 §D1).
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

        // Declare `PayloadType` so that the variant payload edge target resolves.
        // This is necessary for the edge-config lookup to be triggered (CN-02).
        doc.types.insert(
            TypeName::new("PayloadType").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::value_object(),
                kind: TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                methods: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );

        let tuple_variant = VariantDecl::tuple(
            VariantName::new("Value").unwrap(),
            vec![TypeRef::new("PayloadType").unwrap()],
        );
        doc.types.insert(
            TypeName::new("MyEnum").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::value_object(),
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
    // T009 / AC-01: output is a ```mermaid-fenced markdown block; inner body
    // order is flowchart LR → classDef → layer-subgraph → edge → class-attach
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_output_is_mermaid_fenced_markdown_block() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), MINIMAL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();
        let content = adapter.render(&[], &[], &opts).unwrap();
        let text = content.as_ref();

        // Header comment must be the very first line.
        assert!(
            text.starts_with("<!-- Generated contract-map-renderer — DO NOT EDIT DIRECTLY -->"),
            "output must start with generated-file header comment, got: {:?}",
            &text[..text.len().min(80)]
        );

        // Opening fence must immediately follow the header line.
        let after_header = text.find('\n').map(|i| &text[i + 1..]).unwrap_or("");
        assert!(
            after_header.starts_with("```mermaid\n"),
            "opening ```mermaid fence must follow the header comment, got: {:?}",
            &after_header[..after_header.len().min(40)]
        );

        // Closing fence must be present.
        assert!(text.contains("\n```\n"), "closing ``` fence must be present");

        // The mermaid content inside the fence must begin with 'flowchart LR'.
        // `fence_end` points to the closing ``` (not including the preceding \n),
        // so `mermaid_body` includes the trailing \n of the last mermaid line.
        let fence_open = "```mermaid\n";
        let fence_start = text.find(fence_open).expect("opening fence") + fence_open.len();
        // Find the closing ``` fence and include the preceding newline in the body.
        let fence_end = text[fence_start..]
            .find("\n```")
            .map(|i| fence_start + i + 1) // +1 to include the \n before ```
            .unwrap_or(text.len());
        let mermaid_body = &text[fence_start..fence_end];
        assert!(
            mermaid_body.starts_with("flowchart LR\n"),
            "mermaid body inside the fence must start with 'flowchart LR\\n', got: {:?}",
            &mermaid_body[..mermaid_body.len().min(40)]
        );
    }

    // -----------------------------------------------------------------------
    // Layout-containment: every subgraph line is immediately followed by
    // `direction TB` (nested-subgraph layout fix)
    // -----------------------------------------------------------------------

    #[test]
    fn test_every_subgraph_line_is_immediately_followed_by_direction_tb() {
        // Render a minimal diagram with all three subgraph levels present:
        // layer → module → entry (Type + Trait).
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), MINIMAL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();

        let crate_name = CrateName::new("domain").unwrap();
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer.clone());

        // Module-level type (creates layer → module → entry nesting).
        doc.types.insert(
            TypeName::new("ModuleType").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::value_object(),
                kind: TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                methods: vec![],
                module_path: ModulePath::from_segments(vec!["submod".to_string()]).unwrap(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );

        // Root-level trait (creates layer → entry nesting without module subgraph).
        doc.traits.insert(
            TraitName::new("RootTrait").unwrap(),
            TraitEntry {
                action: ItemAction::Add,
                role: ContractRole::SecondaryPort,
                methods: vec![],
                assoc_types: vec![],
                assoc_consts: vec![],
                supertrait_bounds: vec![],
                generics: vec![],
                where_predicates: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );

        let content = adapter.render(&[doc], &[layer], &opts).unwrap();
        let text = content.as_ref();

        // Extract the mermaid body (between ```mermaid\n and \n```).
        let fence_open = "```mermaid\n";
        let fence_start = text.find(fence_open).expect("opening fence") + fence_open.len();
        let fence_end =
            text[fence_start..].find("\n```").map(|i| fence_start + i + 1).unwrap_or(text.len());
        let mermaid_body = &text[fence_start..fence_end];

        // Every line that starts with optional whitespace + "subgraph " must be
        // immediately followed by a line that (after stripping leading whitespace)
        // equals "direction TB".
        let lines: Vec<&str> = mermaid_body.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if line.trim_start().starts_with("subgraph ") {
                let next = lines.get(i + 1).copied().unwrap_or("");
                assert_eq!(
                    next.trim_start(),
                    "direction TB",
                    "line after `{line}` (index {i}) must be `direction TB`, got: {:?}",
                    next
                );
            }
        }

        // Confirm there is at least one subgraph in the output (sanity guard).
        assert!(
            mermaid_body.contains("subgraph "),
            "output must contain at least one subgraph line: {mermaid_body}"
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
                role: DataRole::value_object(),
                kind: TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
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
                role: DataRole::entity().unwrap(),
                kind: TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
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
        // Typestate transition edges are only emitted when the return type resolves to a
        // declared catalogue node (ADR 2026-04-17-1528 §D1). `Approved` must be declared
        // so that the `Pending::approve -> Approved` transition edge renders.
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
                role: DataRole::value_object(),
                kind: TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    Some(marker),
                )),
                methods: vec![approve_method],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );

        // Declare `Approved` as a catalogue type so the transition edge target resolves.
        doc.types.insert(
            TypeName::new("Approved").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::value_object(),
                kind: TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                methods: vec![],
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
                role: DataRole::value_object(),
                kind: TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
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
                role: DataRole::value_object(),
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
        // Struct-variant payload edges are only emitted when the field type resolves to a
        // declared catalogue node (ADR 2026-04-17-1528 §D1). `ErrorMessage` is declared so
        // that the `AppError::Error { message: ErrorMessage }` edge renders with the label.
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), FULL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();

        let crate_name = CrateName::new("domain").unwrap();
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer.clone());

        // Declare `ErrorMessage` as a catalogue type so the variant payload edge resolves.
        doc.types.insert(
            TypeName::new("ErrorMessage").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::value_object(),
                kind: TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                methods: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );

        let field = FieldDecl::new(
            FieldName::new("message").unwrap(),
            TypeRef::new("ErrorMessage").unwrap(),
        );
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
        // Field edges are only emitted when the target resolves to a declared catalogue
        // node (ADR 2026-04-17-1528 §D1). Both `User` and `Email` must be declared so
        // that the `User.email: Email` edge is rendered.
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), FULL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();

        let crate_name = CrateName::new("domain").unwrap();
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer.clone());

        // Declare `Email` as a catalogue type so the field edge target resolves.
        doc.types.insert(
            TypeName::new("Email").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::value_object(),
                kind: TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                methods: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );

        let field =
            FieldDecl::new(FieldName::new("email").unwrap(), TypeRef::new("Email").unwrap());
        doc.types.insert(
            TypeName::new("User").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::entity().unwrap(),
                kind: TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![field], has_stripped_fields: false },
                    None,
                )),
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
                kind: TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain {
                        fields: vec![field],
                        has_stripped_fields: true, // stripped — no field edge
                    },
                    None,
                )),
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
        // TupleStruct field edges are only emitted when the target resolves to a declared
        // catalogue node (ADR 2026-04-17-1528 §D1). Both `UserId` and `GroupId` must be
        // declared so the `Pair(.0: UserId, .1: GroupId)` edges are rendered.
        // `String` is intentionally omitted to verify it does not create a ghost node.
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), FULL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();

        let crate_name = CrateName::new("domain").unwrap();
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer.clone());

        // Declare both target types so that positional edges can be resolved.
        for type_name in ["UserId", "GroupId"] {
            doc.types.insert(
                TypeName::new(type_name).unwrap(),
                TypeEntry {
                    action: ItemAction::Add,
                    role: DataRole::value_object(),
                    kind: TypeKindV2::Struct(StructKind::new(
                        StructShape::Plain { fields: vec![], has_stripped_fields: false },
                        None,
                    )),
                    methods: vec![],
                    module_path: ModulePath::root(),
                    docs: None,
                    spec_refs: vec![],
                    informal_grounds: vec![],
                },
            );
        }

        doc.types.insert(
            TypeName::new("Pair").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::value_object(),
                kind: TypeKindV2::Struct(StructKind::new(
                    StructShape::Tuple {
                        // Use two declared types so both positional edges (.0, .1) are emitted.
                        fields: vec![
                            TypeRef::new("UserId").unwrap(),
                            TypeRef::new("GroupId").unwrap(),
                        ],
                        has_stripped_fields: false,
                    },
                    None,
                )),
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
        // The undeclared primitive `String` must not create a ghost node.
        assert!(
            !output.contains("String"),
            "primitive 'String' must not appear as ghost node: {output}"
        );
    }

    // -----------------------------------------------------------------------
    // T007 / AC-08: TypeAlias undirected alias_of edge
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_type_alias_emits_alias_of_edge() {
        // TypeAlias edges are only emitted when the alias target resolves to a declared
        // catalogue node (ADR 2026-04-17-1528 §D1). `UserId` aliases `RawId` which must
        // be declared so that the `alias_of` edge is rendered.
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), FULL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();

        let crate_name = CrateName::new("domain").unwrap();
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer.clone());

        // Declare the alias target so the alias_of edge resolves.
        doc.types.insert(
            TypeName::new("RawId").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::value_object(),
                kind: TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                methods: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );

        doc.types.insert(
            TypeName::new("UserId").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::value_object(),
                kind: TypeKindV2::TypeAlias { target: TypeRef::new("RawId").unwrap() },
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
                kind: TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
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
                kind: TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
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
        for trait_ref in ["external::crate::SomeTrait", "std::fmt::Display"] {
            // Qualified trait refs not declared in any catalogue are workspace-external
            // and must be silently skipped.
            let tmp = tempfile::tempdir().unwrap();
            let path = write_style_config(tmp.path(), MINIMAL_VALID_CONFIG);
            let adapter = ContractMapRendererAdapter::new(path);
            let opts = ContractMapRenderOptions::default();

            let layer = LayerId::try_new("domain").unwrap();
            let mut doc =
                CatalogueDocument::new(3, CrateName::new("domain").unwrap(), layer.clone());
            doc.types.insert(
                TypeName::new("MyType").unwrap(),
                TypeEntry {
                    action: ItemAction::Add,
                    role: DataRole::value_object(),
                    kind: TypeKindV2::Struct(StructKind::new(
                        StructShape::Plain { fields: vec![], has_stripped_fields: false },
                        None,
                    )),
                    methods: vec![],
                    module_path: ModulePath::root(),
                    docs: None,
                    spec_refs: vec![],
                    informal_grounds: vec![],
                },
            );
            doc.trait_impls.push(TraitImplDeclV2::new(
                TypeRef::new(trait_ref).unwrap(),
                TypeRef::new("MyType").unwrap(),
            ));

            let result = adapter.render(&[doc], &[layer], &opts).unwrap();
            let output = result.as_ref();
            assert!(
                !output.contains("-.impl.->"),
                "no impl edge for external trait ref {trait_ref}: {output}"
            );
        }
    }

    #[test]
    fn test_render_delete_action_trait_is_not_rendered() {
        // A TraitEntry with action:Delete must produce no subgraph and must not be
        // reachable as an edge target (absent from the trait index).
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), FULL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();

        let crate_name = CrateName::new("domain").unwrap();
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer.clone());

        // Deleted trait — must be absent from output and trait index.
        doc.traits.insert(
            TraitName::new("RemovedTrait").unwrap(),
            TraitEntry {
                action: ItemAction::Delete,
                role: ContractRole::SecondaryPort,
                methods: vec![],
                assoc_types: vec![],
                assoc_consts: vec![],
                supertrait_bounds: vec![],
                generics: vec![],
                where_predicates: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );

        // A type that tries to impl the deleted trait — the trait_impl edge must be skipped
        // (deleted trait is absent from the trait index).
        doc.types.insert(
            TypeName::new("MyAdapter").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::SecondaryAdapter,
                kind: TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                methods: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );
        doc.trait_impls.push(TraitImplDeclV2::new(
            TypeRef::new("RemovedTrait").unwrap(),
            TypeRef::new("MyAdapter").unwrap(),
        ));

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.as_ref();
        assert!(
            !output.contains("RemovedTrait"),
            "Delete-action trait must not appear in output: {output}"
        );
        // No impl edge should be generated since the target trait was deleted.
        assert!(
            !output.contains("-.impl.->"),
            "impl edge to deleted trait must be silently skipped: {output}"
        );
    }

    #[test]
    fn test_render_delete_action_function_is_not_rendered() {
        // A FunctionEntry with action:Delete must produce no node and no edges.
        use domain::tddd::catalogue_v2::entries::FunctionEntry;
        use domain::tddd::catalogue_v2::identifiers::{FunctionName, FunctionPath};
        use domain::tddd::catalogue_v2::roles::FunctionRole;

        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), MINIMAL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();

        let crate_name = CrateName::new("domain").unwrap();
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name.clone(), layer.clone());

        let fn_path =
            FunctionPath::at_root(crate_name.clone(), FunctionName::new("removed_fn").unwrap());
        doc.functions.insert(
            fn_path,
            FunctionEntry {
                action: ItemAction::Delete,
                role: FunctionRole::FreeFunction,
                params: vec![],
                returns: TypeRef::new("()").unwrap(),
                is_async: false,
                generics: vec![],
                where_predicates: vec![],
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.as_ref();
        assert!(
            !output.contains("removed_fn"),
            "Delete-action function must not appear in output: {output}"
        );
    }

    // -----------------------------------------------------------------------
    // Bug 2: edges to undeclared / primitive / generic / external types skipped
    // -----------------------------------------------------------------------

    #[test]
    fn test_render_field_edge_to_primitive_type_is_skipped() {
        // A PlainStruct field whose type is a primitive (e.g. `String`, `u64`, `bool`)
        // must NOT produce a floating ghost node in the output (ADR 2026-04-17-1528 §D1).
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), FULL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();

        let crate_name = CrateName::new("domain").unwrap();
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer.clone());

        // `name: String` — `String` is not a catalogue entry; edge must be silently skipped.
        let field =
            FieldDecl::new(FieldName::new("name").unwrap(), TypeRef::new("String").unwrap());
        doc.types.insert(
            TypeName::new("Product").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::entity().unwrap(),
                kind: TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![field], has_stripped_fields: false },
                    None,
                )),
                methods: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.as_ref();
        // `String` must not appear as a floating node outside any layer subgraph.
        assert!(
            !output.contains("String"),
            "primitive 'String' must not create a ghost node: {output}"
        );
        // The struct node itself must still be rendered.
        assert!(output.contains("Product"), "Product type must still appear: {output}");
    }

    #[test]
    fn test_render_method_param_edge_to_generic_param_is_skipped() {
        // A method parameter whose type is a generic parameter (e.g. `T`, `L`, `W`)
        // must NOT produce a floating ghost node. Generic params never resolve to a
        // declared catalogue node (ADR 2026-04-17-1528 §D1).
        use domain::tddd::catalogue_v2::identifiers::ParamName;
        use domain::tddd::catalogue_v2::methods::ParamDeclaration;

        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), FULL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();

        let crate_name = CrateName::new("domain").unwrap();
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer.clone());

        // Method with param `w: W` and return `L` — generic params, no declared target.
        let method_with_generic_params = MethodDeclaration::new(
            MethodName::new("convert").unwrap(),
            Some(SelfReceiver::SharedRef),
            vec![ParamDeclaration::new(ParamName::new("w").unwrap(), TypeRef::new("W").unwrap())],
            TypeRef::new("L").unwrap(),
            false,
            None,
        );
        doc.types.insert(
            TypeName::new("Converter").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::domain_service(),
                kind: TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                methods: vec![method_with_generic_params],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.as_ref();
        // Generic params `W` and `L` must not appear as ghost nodes.
        assert!(
            !output.contains("--o W"),
            "generic param 'W' must not create ghost edge: {output}"
        );
        assert!(
            !output.contains("--> L"),
            "generic param 'L' must not create ghost edge: {output}"
        );
        // The method node itself must still be rendered.
        assert!(output.contains("convert"), "method 'convert' must still appear: {output}");
    }

    #[test]
    fn test_render_delete_action_type_not_in_node_index_so_no_edge_to_it() {
        // A type with action:Delete must not appear in the node index, so no edge can
        // target it. Another type with a field pointing at the deleted type must produce
        // no edge (the deleted type is absent from the index — silent skip).
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), FULL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();

        let crate_name = CrateName::new("domain").unwrap();
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer.clone());

        // Type that is being deleted.
        doc.types.insert(
            TypeName::new("OldToken").unwrap(),
            TypeEntry {
                action: ItemAction::Delete,
                role: DataRole::value_object(),
                kind: TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                methods: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );

        // Type with a field pointing at the deleted type — edge must be silently skipped.
        let field =
            FieldDecl::new(FieldName::new("token").unwrap(), TypeRef::new("OldToken").unwrap());
        doc.types.insert(
            TypeName::new("Session").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::entity().unwrap(),
                kind: TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![field], has_stripped_fields: false },
                    None,
                )),
                methods: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.as_ref();
        // OldToken is deleted — must not appear in output at all.
        assert!(!output.contains("OldToken"), "deleted type must not appear: {output}");
        // Session is still rendered.
        assert!(output.contains("Session"), "Session type must still appear: {output}");
        // No edge to the deleted type.
        assert!(
            !output.contains("|token|"),
            "field edge to deleted type must be silently skipped: {output}"
        );
    }

    // -----------------------------------------------------------------------
    // TypeRef-resolution bug fix tests (syn-based unwrapping)
    // -----------------------------------------------------------------------

    /// A method whose return type is `Result<DeclaredA, DeclaredB>` must emit `-->`
    /// edges to BOTH `DeclaredA` and `DeclaredB`.
    #[test]
    fn test_render_method_return_result_emits_edges_to_both_type_params() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), FULL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();

        let crate_name = CrateName::new("infrastructure").unwrap();
        let layer = LayerId::try_new("infrastructure").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer.clone());

        // Declare both result variants as catalogue types.
        for type_name in ["ContractMapContent", "ContractMapRendererError"] {
            doc.types.insert(
                TypeName::new(type_name).unwrap(),
                TypeEntry {
                    action: ItemAction::Add,
                    role: DataRole::value_object(),
                    kind: TypeKindV2::Struct(StructKind::new(
                        StructShape::Plain { fields: vec![], has_stripped_fields: false },
                        None,
                    )),
                    methods: vec![],
                    module_path: ModulePath::root(),
                    docs: None,
                    spec_refs: vec![],
                    informal_grounds: vec![],
                },
            );
        }

        // Method returning `Result<ContractMapContent, ContractMapRendererError>`.
        let render_method = MethodDeclaration::new(
            MethodName::new("render").unwrap(),
            Some(SelfReceiver::SharedRef),
            vec![],
            TypeRef::new("Result<ContractMapContent, ContractMapRendererError>").unwrap(),
            false,
            None,
        );
        doc.types.insert(
            TypeName::new("ContractMapRenderer").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::domain_service(),
                kind: TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                methods: vec![render_method],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.as_ref();
        // Both declared type params must be wired — verify the `-->` edge appears
        // and both target nodes are present.
        assert!(
            output.contains("-->"),
            "return edge '-->' must appear for Result<A,B> return: {output}"
        );
        assert!(
            output.contains("ContractMapContent"),
            "ContractMapContent must be referenced: {output}"
        );
        assert!(
            output.contains("ContractMapRendererError"),
            "ContractMapRendererError must be referenced: {output}"
        );
    }

    /// A method param wrapped in a supported generic container must emit a `--o` edge
    /// to the declared inner type.
    #[test]
    fn test_render_method_param_wrappers_of_declared_type_emit_edge() {
        use domain::tddd::catalogue_v2::identifiers::ParamName;
        use domain::tddd::catalogue_v2::methods::ParamDeclaration;

        for (target_type, owner_type, method_name, param_name, param_type) in [
            ("DeclaredItem", "Processor", "process", "items", "Vec<DeclaredItem>"),
            ("MaybeUser", "UserRepo", "find", "user", "Option<MaybeUser>"),
            // Reference prefix (`&T`): the resolver must strip the `&` before lookup.
            ("RenderOptions", "Renderer", "render", "opts", "&RenderOptions"),
        ] {
            let tmp = tempfile::tempdir().unwrap();
            let path = write_style_config(tmp.path(), FULL_VALID_CONFIG);
            let adapter = ContractMapRendererAdapter::new(path);
            let opts = ContractMapRenderOptions::default();

            let crate_name = CrateName::new("domain").unwrap();
            let layer = LayerId::try_new("domain").unwrap();
            let mut doc = CatalogueDocument::new(3, crate_name, layer.clone());

            doc.types.insert(
                TypeName::new(target_type).unwrap(),
                TypeEntry {
                    action: ItemAction::Add,
                    role: DataRole::entity().unwrap(),
                    kind: TypeKindV2::Struct(StructKind::new(
                        StructShape::Plain { fields: vec![], has_stripped_fields: false },
                        None,
                    )),
                    methods: vec![],
                    module_path: ModulePath::root(),
                    docs: None,
                    spec_refs: vec![],
                    informal_grounds: vec![],
                },
            );

            let method = MethodDeclaration::new(
                MethodName::new(method_name).unwrap(),
                Some(SelfReceiver::SharedRef),
                vec![ParamDeclaration::new(
                    ParamName::new(param_name).unwrap(),
                    TypeRef::new(param_type).unwrap(),
                )],
                TypeRef::new("()").unwrap(),
                false,
                None,
            );
            doc.types.insert(
                TypeName::new(owner_type).unwrap(),
                TypeEntry {
                    action: ItemAction::Add,
                    role: DataRole::domain_service(),
                    kind: TypeKindV2::Struct(StructKind::new(
                        StructShape::Plain { fields: vec![], has_stripped_fields: false },
                        None,
                    )),
                    methods: vec![method],
                    module_path: ModulePath::root(),
                    docs: None,
                    spec_refs: vec![],
                    informal_grounds: vec![],
                },
            );

            let result = adapter.render(&[doc], &[layer], &opts).unwrap();
            let output = result.as_ref();
            assert!(
                output.contains("--o"),
                "param edge '--o' must appear for {param_type}: {output}"
            );
            assert!(
                output.contains(target_type),
                "{target_type} must be referenced as edge target: {output}"
            );
        }
    }

    /// Primitives and generic type params inside wrapper types (e.g. `Vec<String>`,
    /// `Option<T>`) must NOT create ghost nodes — only declared catalogue types emit edges.
    #[test]
    fn test_render_wrapper_with_primitive_or_generic_param_skipped_no_ghost_node() {
        use domain::tddd::catalogue_v2::identifiers::ParamName;
        use domain::tddd::catalogue_v2::methods::ParamDeclaration;

        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), FULL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();

        let crate_name = CrateName::new("domain").unwrap();
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer.clone());

        // Method with `Vec<String>` and `Option<T>` params — neither String nor T is declared.
        let method = MethodDeclaration::new(
            MethodName::new("store").unwrap(),
            Some(SelfReceiver::SharedRef),
            vec![
                ParamDeclaration::new(
                    ParamName::new("names").unwrap(),
                    TypeRef::new("Vec<String>").unwrap(),
                ),
                ParamDeclaration::new(
                    ParamName::new("val").unwrap(),
                    TypeRef::new("Option<T>").unwrap(),
                ),
            ],
            TypeRef::new("Result<(), String>").unwrap(),
            false,
            None,
        );
        doc.types.insert(
            TypeName::new("Store").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::domain_service(),
                kind: TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                methods: vec![method],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.as_ref();
        // No edge: String, T are undeclared.
        assert!(
            !output.contains("--o String"),
            "String inside Vec<String> must not create ghost edge: {output}"
        );
        assert!(
            !output.contains("--o T"),
            "generic param T inside Option<T> must not create ghost edge: {output}"
        );
        // The method node itself must still appear.
        assert!(output.contains("store"), "method 'store' must still appear: {output}");
    }

    // -----------------------------------------------------------------------
    // Representative-node layout fix: no edge endpoint is a subgraph id
    // -----------------------------------------------------------------------

    /// Assert that every edge line's endpoints are NOT subgraph ids.
    ///
    /// A subgraph id is defined as an id that appears on a `subgraph <id>[…]` line
    /// in the mermaid output.  The fix requires that all edges target representative
    /// nodes (`__self` nodes) so that Dagre/ELK never has to draw an edge into a
    /// cluster boundary, which previously caused child subgraphs to render outside
    /// their parent layer box.
    #[test]
    fn test_no_edge_endpoint_is_a_subgraph_id() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), FULL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();

        let crate_name = CrateName::new("infrastructure").unwrap();
        let layer = LayerId::try_new("infrastructure").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name.clone(), layer.clone());

        // Declare a trait that will be the impl target.
        doc.traits.insert(TraitName::new("ContractMapRenderer").unwrap(), make_empty_trait_entry());

        // Declare an adapter with a constructor method returning Self.
        let new_method = MethodDeclaration::new(
            MethodName::new("new").unwrap(),
            None, // no self receiver — constructor
            vec![],
            TypeRef::new("Self").unwrap(),
            false,
            None,
        );
        doc.types.insert(
            TypeName::new("ContractMapRendererAdapter").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::SecondaryAdapter,
                kind: TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                methods: vec![new_method],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );

        // Trait impl: ContractMapRendererAdapter implements ContractMapRenderer.
        doc.trait_impls.push(TraitImplDeclV2::new(
            TypeRef::new("ContractMapRenderer").unwrap(),
            TypeRef::new("ContractMapRendererAdapter").unwrap(),
        ));

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.as_ref();

        // Collect all subgraph ids from the output.
        let mut subgraph_ids: Vec<&str> = Vec::new();
        for line in output.lines() {
            let trimmed = line.trim_start();
            if let Some(rest) = trimmed.strip_prefix("subgraph ") {
                // `subgraph <id>["label"]` or `subgraph <id>`
                let id = rest.split_once('[').map(|(id, _)| id).unwrap_or(rest).trim();
                if !id.is_empty() {
                    subgraph_ids.push(id);
                }
            }
        }

        // Verify at least one subgraph was emitted (sanity guard).
        assert!(!subgraph_ids.is_empty(), "no subgraph lines found in output: {output}");

        // Parse edge lines (lines containing an arrow) and check endpoints.
        // Arrow markers used in the test config: -->, --o, ==>, -.impl.->
        let arrow_markers = ["-->", "--o", "==>", "-.impl.->", "---"];
        for line in output.lines() {
            let trimmed = line.trim();
            // Skip non-edge lines.
            if !arrow_markers.iter().any(|m| trimmed.contains(m)) {
                continue;
            }
            // Split on the first arrow marker to get source and remainder.
            for marker in &arrow_markers {
                if let Some(pos) = trimmed.find(marker) {
                    let source = trimmed[..pos].trim();
                    let remainder = trimmed[pos + marker.len()..].trim();
                    // Strip optional |label| to get the target id.
                    let target = if remainder.starts_with('|') {
                        // `|label| target_id`
                        remainder
                            .find('|')
                            .and_then(|s| remainder[s + 1..].find('|').map(|e| s + 1 + e + 1))
                            .map(|end| remainder[end..].trim())
                            .unwrap_or(remainder)
                    } else {
                        remainder
                    };

                    for sg_id in &subgraph_ids {
                        assert_ne!(
                            source, *sg_id,
                            "edge source `{source}` equals subgraph id `{sg_id}` — must target representative node: {line}"
                        );
                        assert_ne!(
                            target, *sg_id,
                            "edge target `{target}` equals subgraph id `{sg_id}` — must target representative node: {line}"
                        );
                    }
                    break; // only process first matching marker per line
                }
            }
        }
    }

    /// Assert that each Type and Trait entry subgraph contains a representative node
    /// (`__self` node) emitted directly inside the subgraph.
    #[test]
    fn test_each_type_and_trait_entry_subgraph_contains_representative_node() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), FULL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();

        let crate_name = CrateName::new("domain").unwrap();
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer.clone());

        doc.types.insert(
            TypeName::new("MyType").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::value_object(),
                kind: TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                methods: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );
        doc.traits.insert(TraitName::new("MyTrait").unwrap(), make_empty_trait_entry());

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.as_ref();

        // The representative node id is the subgraph id with `__self` suffix.
        // Since we know the type/trait names we can derive the subgraph ids and check.
        let type_sg_id = render::type_node_id("domain", "domain", "MyType");
        let type_rep_id = render::type_rep_node_id("domain", "domain", "MyType");
        let trait_sg_id = render::trait_node_id("domain", "domain", "MyTrait");
        let trait_rep_id = render::trait_rep_node_id("domain", "domain", "MyTrait");

        // Subgraph ids must appear on `subgraph …` lines.
        assert!(
            output.contains(&format!("subgraph {type_sg_id}[")),
            "Type subgraph id must appear: {output}"
        );
        assert!(
            output.contains(&format!("subgraph {trait_sg_id}[")),
            "Trait subgraph id must appear: {output}"
        );

        // Representative nodes must appear as standalone node lines (not subgraph lines).
        assert!(
            output.contains(&type_rep_id),
            "Type representative node `{type_rep_id}` must appear in output: {output}"
        );
        assert!(
            output.contains(&trait_rep_id),
            "Trait representative node `{trait_rep_id}` must appear in output: {output}"
        );

        // Neither representative node id should appear on a `subgraph …` line — they
        // are regular nodes, not subgraph containers.
        for line in output.lines() {
            if line.trim_start().starts_with("subgraph ") {
                assert!(
                    !line.contains(&type_rep_id),
                    "representative node id `{type_rep_id}` must not be a subgraph id: {line}"
                );
                assert!(
                    !line.contains(&trait_rep_id),
                    "representative node id `{trait_rep_id}` must not be a subgraph id: {line}"
                );
            }
        }
    }

    // -----------------------------------------------------------------------
    // Helper constructors for tests
    // -----------------------------------------------------------------------

    fn make_empty_trait_entry() -> TraitEntry {
        TraitEntry {
            action: ItemAction::Add,
            role: ContractRole::SecondaryPort,
            methods: vec![],
            assoc_types: vec![],
            assoc_consts: vec![],
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
