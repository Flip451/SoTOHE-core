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

use domain::tddd::catalogue_linter::RoleKind;
use domain::tddd::catalogue_v2::CatalogueDocument;
use domain::tddd::catalogue_v2::roles::ItemAction;
use domain::tddd::{
    ContractMapContent, ContractMapRenderOptions, ContractMapRenderResult,
    ContractMapRenderWarning, ContractMapRenderer, ContractMapRendererError, LayerId,
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
    ) -> Result<ContractMapRenderResult, ContractMapRendererError> {
        let style = self.load_style_config()?;
        let output = render::render_mermaid(catalogues, layer_order, &style)?;
        Ok(ContractMapRenderResult::new(
            ContractMapContent::new(output),
            undefined_role_style_warnings(catalogues, layer_order, &style),
        ))
    }
}

fn undefined_role_style_warnings(
    catalogues: &[CatalogueDocument],
    layer_order: &[LayerId],
    style: &render::StyleConfig,
) -> Vec<ContractMapRenderWarning> {
    let mut roles = Vec::new();
    for catalogue in catalogues.iter().filter(|catalogue| layer_order.contains(catalogue.layer())) {
        for entry in catalogue.types().values() {
            if entry.action() != ItemAction::Delete {
                roles.push(RoleKind::from_data_role(entry.role()));
            }
        }
        for entry in catalogue.traits().values() {
            if entry.action() != ItemAction::Delete {
                roles.push(RoleKind::from_contract_role(entry.role()));
            }
        }
        for entry in catalogue.functions().values() {
            if entry.action() != ItemAction::Delete {
                roles.push(RoleKind::from_function_role(&entry.role()));
            }
        }
    }

    missing_role_style_warnings(&roles, style)
}

fn missing_role_style_warnings(
    rendered_roles: &[RoleKind],
    style: &render::StyleConfig,
) -> Vec<ContractMapRenderWarning> {
    let mut warnings = Vec::new();
    for role in RoleKind::all() {
        if rendered_roles.contains(role)
            && style
                .role
                .get(role.variant_name())
                .is_none_or(|role_style| !style.class.contains_key(&role_style.class))
        {
            warnings.push(ContractMapRenderWarning::UndefinedRoleStyle { role: *role });
        }
    }
    warnings
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use domain::tddd::LayerId;
    use domain::tddd::catalogue_v2::CatalogueEntryKey;
    use domain::tddd::catalogue_v2::composite::{
        StructKind, StructShape, TypeKindV2, TypestateMarker, TypestateTransitions,
    };
    use domain::tddd::catalogue_v2::entries::{
        FunctionEntry, InherentImplDeclV2, TraitEntry, TypeEntry,
    };
    use domain::tddd::catalogue_v2::identifiers::{
        CrateName, FieldName, FunctionName, FunctionPath, MethodName, ModulePath, TypeName,
        TypeRef, VariantName,
    };
    use domain::tddd::catalogue_v2::methods::{MethodDeclaration, ParamDeclaration};
    use domain::tddd::catalogue_v2::roles::{
        ContractRole, DataRole, FunctionRole, ItemAction, SelfReceiver,
    };
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
        assert!(content.content().as_ref().contains("flowchart LR"), "must contain 'flowchart LR'");
    }

    #[test]
    fn test_render_undefined_role_style_returns_typed_warning() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), MINIMAL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let layer = LayerId::try_new("domain").unwrap();
        let crate_name = CrateName::new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer.clone());
        doc.insert_type(
            CatalogueEntryKey::try_new("UnstyledValue".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );

        let result =
            adapter.render(&[doc], &[layer], &ContractMapRenderOptions::default()).unwrap();
        assert_eq!(
            result.warnings().to_vec(),
            vec![ContractMapRenderWarning::UndefinedRoleStyle { role: RoleKind::ValueObject }]
        );
    }

    #[test]
    fn test_missing_role_style_warnings_cover_all_role_kinds() {
        let style = toml::from_str::<render::StyleConfig>(MINIMAL_VALID_CONFIG).unwrap();

        let warnings = missing_role_style_warnings(RoleKind::all(), &style);
        let expected = RoleKind::all()
            .iter()
            .map(|role| ContractMapRenderWarning::UndefinedRoleStyle { role: *role })
            .collect::<Vec<_>>();

        assert_eq!(warnings, expected, "every declared role must be checked for a classDef");
    }

    #[test]
    fn test_render_undefined_role_style_ignores_excluded_layer() {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), MINIMAL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let rendered_layer = LayerId::try_new("domain").unwrap();
        let excluded_layer = LayerId::try_new("usecase").unwrap();
        let crate_name = CrateName::new("usecase").unwrap();
        let mut excluded_catalogue = CatalogueDocument::new(3, crate_name, excluded_layer);
        excluded_catalogue.insert_type(
            CatalogueEntryKey::try_new("UnstyledBoundaryValue".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );

        let result = adapter
            .render(&[excluded_catalogue], &[rendered_layer], &ContractMapRenderOptions::default())
            .unwrap();

        assert!(
            result.warnings().is_empty(),
            "roles in excluded layers must not produce style warnings: {:?}",
            result.warnings()
        );
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
        doc.insert_type(
            CatalogueEntryKey::try_new("PayloadType".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );

        let tuple_variant = VariantDecl::tuple(
            VariantName::new("Value").unwrap(),
            vec![TypeRef::new("PayloadType").unwrap()],
        );
        doc.insert_type(
            CatalogueEntryKey::try_new("MyEnum".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Enum { variants: vec![tuple_variant] },
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
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
        let text = content.content().as_ref();

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
        // ELK layout frontmatter (`---\nconfig:\n  layout: elk\n---\n`) sits
        // between the fence and `flowchart LR` to sidestep the dagre
        // cluster-ordering crash on large 3-level subgraphs. Both must be
        // present at the top of the fence.
        assert!(
            mermaid_body.starts_with("---\nconfig:\n  layout: elk\n---\nflowchart LR\n"),
            "mermaid body must begin with ELK frontmatter followed by 'flowchart LR\\n', got: {:?}",
            &mermaid_body[..mermaid_body.len().min(80)]
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
        doc.insert_type(
            CatalogueEntryKey::try_new("ModuleType".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                vec![],
                vec![],
                vec![],
                ModulePath::from_segments(vec!["submod".to_string()]).unwrap(),
                None,
                vec![],
                vec![],
            ),
        );

        // Root-level trait (creates layer → entry nesting without module subgraph).
        doc.insert_trait(
            CatalogueEntryKey::try_new("RootTrait".to_owned()).unwrap(),
            TraitEntry::new(
                ItemAction::Add,
                ContractRole::SecondaryPort,
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );

        let content = adapter.render(&[doc], &[layer], &opts).unwrap();
        let text = content.content().as_ref();

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

        doc_a.insert_trait(
            CatalogueEntryKey::try_new("TraitA".to_owned()).unwrap(),
            make_empty_trait_entry(),
        );
        doc_b.insert_trait(
            CatalogueEntryKey::try_new("TraitB".to_owned()).unwrap(),
            make_empty_trait_entry(),
        );

        let index = render::build_trait_index(&[doc_a, doc_b]);
        assert!(index.resolve("TraitA", "crate_a").unwrap().is_some());
        assert!(index.resolve("TraitB", "crate_b").unwrap().is_some());
        assert!(index.resolve("TraitB", "crate_a").unwrap().is_none());
    }

    #[test]
    fn test_relative_node_reference_fails_closed_with_spelling_and_accepted_forms() {
        let index = render::NodeIndex::new();
        let error = index.resolve("super::port::Port", "domain").unwrap_err();

        let reason = match error {
            ContractMapRendererError::RenderFailed { reason } => reason,
            other => panic!("relative reference must return RenderFailed, got: {other:?}"),
        };
        assert!(
            reason.contains("super::port::Port"),
            "diagnostic must preserve spelling: {reason}"
        );
        assert!(reason.contains("crate 'domain'"), "diagnostic must identify location: {reason}");
        assert!(
            reason.contains("accepted forms"),
            "diagnostic must state accepted forms: {reason}"
        );
        assert!(
            reason.contains("module_path + name"),
            "diagnostic must name module form: {reason}"
        );
    }

    #[test]
    fn test_render_nested_relative_reference_fails_closed_instead_of_emitting_wrong_edge() {
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, CrateName::new("domain").unwrap(), layer.clone());
        let unit_entry = |module_path: ModulePath| {
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
                vec![],
                vec![],
                vec![],
                module_path,
                None,
                vec![],
                vec![],
            )
        };
        doc.insert_type(
            CatalogueEntryKey::try_new("domain::alpha::port::Port".to_owned()).unwrap(),
            unit_entry(ModulePath::from_segments(vec!["alpha", "port"]).unwrap()),
        );
        // This root-level homonym demonstrates why stripping `super::` is unsafe:
        // the old rewrite could select this node instead of alpha::port::Port.
        doc.insert_type(
            CatalogueEntryKey::try_new("domain::port::Port".to_owned()).unwrap(),
            unit_entry(ModulePath::from_segments(vec!["port"]).unwrap()),
        );
        doc.insert_type(
            CatalogueEntryKey::try_new("domain::alpha::detail::Owner".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain {
                        fields: vec![FieldDecl::new(
                            FieldName::new("port").unwrap(),
                            TypeRef::new("super::port::Port").unwrap(),
                        )],
                        has_stripped_fields: false,
                    },
                    None,
                )),
                vec![],
                vec![],
                vec![],
                ModulePath::from_segments(vec!["alpha", "detail"]).unwrap(),
                None,
                vec![],
                vec![],
            ),
        );

        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), FULL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let error =
            adapter.render(&[doc], &[layer], &ContractMapRenderOptions::default()).unwrap_err();
        let ContractMapRendererError::RenderFailed { reason } = error else {
            panic!("relative reference must produce RenderFailed: {error:?}");
        };
        assert!(
            reason.contains("super::port::Port"),
            "diagnostic must preserve spelling: {reason}"
        );
        assert!(reason.contains("crate 'domain'"), "diagnostic must identify location: {reason}");
        assert!(
            reason.contains("accepted forms"),
            "diagnostic must state accepted forms: {reason}"
        );
    }

    #[test]
    fn test_render_nested_entry_resolves_module_and_fully_qualified_forms() {
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, CrateName::new("domain").unwrap(), layer.clone());
        doc.insert_type(
            CatalogueEntryKey::try_new("domain::alpha::port::Port".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
                vec![],
                vec![],
                vec![],
                ModulePath::from_segments(vec!["alpha", "port"]).unwrap(),
                None,
                vec![],
                vec![],
            ),
        );
        doc.insert_type(
            CatalogueEntryKey::try_new("domain::alpha::detail::Owner".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain {
                        fields: vec![
                            FieldDecl::new(
                                FieldName::new("joined").unwrap(),
                                TypeRef::new("alpha::port::Port").unwrap(),
                            ),
                            FieldDecl::new(
                                FieldName::new("qualified").unwrap(),
                                TypeRef::new("domain::alpha::port::Port").unwrap(),
                            ),
                        ],
                        has_stripped_fields: false,
                    },
                    None,
                )),
                vec![],
                vec![],
                vec![],
                ModulePath::from_segments(vec!["alpha", "detail"]).unwrap(),
                None,
                vec![],
                vec![],
            ),
        );

        let rendered = render_and_scan(&[doc], &[layer]);
        let owner = render::type_rep_node_id("domain", "domain", "domain::alpha::detail::Owner");
        let target = render::type_rep_node_id("domain", "domain", "domain::alpha::port::Port");
        assert_edge_count(&rendered, &owner, "--o", Some("joined"), &target, 1);
        assert_edge_count(&rendered, &owner, "--o", Some("qualified"), &target, 1);
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
        doc.insert_type(
            CatalogueEntryKey::try_new("RootType".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );

        // Non-root entry
        doc.insert_type(
            CatalogueEntryKey::try_new("ModuleType".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::entity().unwrap(),
                TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                vec![],
                vec![],
                vec![],
                ModulePath::from_segments(vec!["user".to_string()]).unwrap(),
                None,
                vec![],
                vec![],
            ),
        );

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.content().as_ref();
        // Both must appear; specific structural order is not asserted here,
        // but both names must be present.
        assert!(output.contains("RootType"), "must mention RootType: {output}");
        assert!(output.contains("ModuleType"), "must mention ModuleType: {output}");
        // Module subgraph for 'user' must appear.
        assert!(
            output.contains("_4_75736572[\"domain::user\"]"),
            "must have encoded module subgraph: {output}"
        );
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
            false,
            vec![],
            vec![],
            vec![],
            ItemAction::Add,
            None,
        );

        doc.insert_type(
            CatalogueEntryKey::try_new("Pending".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    Some(marker),
                )),
                vec![approve_method],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );

        // Declare `Approved` as a catalogue type so the transition edge target resolves.
        doc.insert_type(
            CatalogueEntryKey::try_new("Approved".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.content().as_ref();
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

        doc.insert_type(
            CatalogueEntryKey::try_new("Email".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );

        // Two InherentImplDeclV2 for the same type (AC-04).
        let m1 = MethodDeclaration::new(
            MethodName::new("as_str").unwrap(),
            Some(SelfReceiver::SharedRef),
            vec![],
            TypeRef::new("str").unwrap(),
            false,
            false,
            vec![],
            vec![],
            vec![],
            ItemAction::Add,
            None,
        );
        let m2 = MethodDeclaration::new(
            MethodName::new("validate").unwrap(),
            Some(SelfReceiver::SharedRef),
            vec![],
            TypeRef::new("bool").unwrap(),
            false,
            false,
            vec![],
            vec![],
            vec![],
            ItemAction::Add,
            None,
        );

        doc.push_inherent_impl(InherentImplDeclV2::new(
            CatalogueEntryKey::try_new("Email".to_owned()).unwrap(),
            vec![],
            vec![],
            vec![m1],
        ));
        doc.push_inherent_impl(InherentImplDeclV2::new(
            CatalogueEntryKey::try_new("Email".to_owned()).unwrap(),
            vec![],
            vec![],
            vec![m2],
        ));

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.content().as_ref();
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

        doc.insert_type(
            CatalogueEntryKey::try_new("Option".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Enum { variants: vec![tuple_variant, unit_variant] },
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.content().as_ref();
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
        doc.insert_type(
            CatalogueEntryKey::try_new("ErrorMessage".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );

        let field = FieldDecl::new(
            FieldName::new("message").unwrap(),
            TypeRef::new("ErrorMessage").unwrap(),
        );
        let struct_variant =
            VariantDecl::struct_variant(VariantName::new("Error").unwrap(), vec![field]);

        doc.insert_type(
            CatalogueEntryKey::try_new("AppError".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::ErrorType,
                TypeKindV2::Enum { variants: vec![struct_variant] },
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.content().as_ref();
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
        doc.insert_type(
            CatalogueEntryKey::try_new("Email".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );

        let field =
            FieldDecl::new(FieldName::new("email").unwrap(), TypeRef::new("Email").unwrap());
        doc.insert_type(
            CatalogueEntryKey::try_new("User".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::entity().unwrap(),
                TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![field], has_stripped_fields: false },
                    None,
                )),
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.content().as_ref();
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
        doc.insert_type(
            CatalogueEntryKey::try_new("Config".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::Dto,
                TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain {
                        fields: vec![field],
                        has_stripped_fields: true, // stripped — no field edge
                    },
                    None,
                )),
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.content().as_ref();
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
            doc.insert_type(
                CatalogueEntryKey::try_new(type_name.to_owned()).unwrap(),
                TypeEntry::new(
                    ItemAction::Add,
                    DataRole::value_object(),
                    TypeKindV2::Struct(StructKind::new(
                        StructShape::Plain { fields: vec![], has_stripped_fields: false },
                        None,
                    )),
                    vec![],
                    vec![],
                    vec![],
                    ModulePath::root(),
                    None,
                    vec![],
                    vec![],
                ),
            );
        }

        doc.insert_type(
            CatalogueEntryKey::try_new("Pair".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(
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
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.content().as_ref();
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
        doc.insert_type(
            CatalogueEntryKey::try_new("RawId".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );

        doc.insert_type(
            CatalogueEntryKey::try_new("UserId".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::TypeAlias { target: TypeRef::new("RawId").unwrap(), generics: vec![] },
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.content().as_ref();
        assert!(output.contains("alias_of"), "alias_of label must appear: {output}");
    }

    #[test]
    fn test_render_generic_alias_parameter_shadows_declared_type() {
        // A kind-level alias parameter (`Alias<T> = Result<T, RawId>`) shadows an
        // identically named declared catalogue type `T`: the generic use must not
        // resolve to that unrelated type, while the declared `RawId` still gets
        // its alias_of edge.
        use domain::tddd::catalogue_v2::identifiers::ParamName;
        use domain::tddd::catalogue_v2::methods::MethodGenericParam;

        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), FULL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();

        let crate_name = CrateName::new("domain").unwrap();
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer.clone());

        for name in ["T", "RawId"] {
            doc.insert_type(
                CatalogueEntryKey::try_new(name.to_owned()).unwrap(),
                TypeEntry::new(
                    ItemAction::Add,
                    DataRole::value_object(),
                    TypeKindV2::Struct(StructKind::new(
                        StructShape::Plain { fields: vec![], has_stripped_fields: false },
                        None,
                    )),
                    vec![],
                    vec![],
                    vec![],
                    ModulePath::root(),
                    None,
                    vec![],
                    vec![],
                ),
            );
        }

        doc.insert_type(
            CatalogueEntryKey::try_new("UserId".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::TypeAlias {
                    target: TypeRef::new("Result<T, RawId>").unwrap(),
                    generics: vec![MethodGenericParam {
                        name: ParamName::new("T").unwrap(),
                        bounds: vec![],
                    }],
                },
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.content().as_ref();
        let source = render::type_rep_node_id("domain", "domain", "UserId");
        let raw_id = render::type_rep_node_id("domain", "domain", "RawId");
        let shadowed_id = render::type_rep_node_id("domain", "domain", "T");
        let raw_edge = format!("{source} ---|alias_of| {raw_id}");
        let shadowed_edge = format!("{source} ---|alias_of| {shadowed_id}");
        assert_eq!(
            output.matches(&raw_edge).count(),
            1,
            "the declared RawId target must produce exactly one alias_of edge: {output}"
        );
        assert!(
            !output.contains(&shadowed_edge),
            "the alias parameter T must not resolve to the declared type T: {output}"
        );
    }

    #[test]
    fn test_render_generic_rooted_alias_target_is_not_resolved_as_qualified_type() {
        // `Alias<T> = T::Item` is a projection on the alias parameter. Even
        // when another rendered catalogue is a crate literally named `T` with
        // a type `Item`, the generic-rooted target must not resolve to that
        // unrelated node.
        use domain::tddd::catalogue_v2::identifiers::ParamName;
        use domain::tddd::catalogue_v2::methods::MethodGenericParam;

        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), FULL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();

        let t_crate = CrateName::new("T").unwrap();
        let t_layer = LayerId::try_new("usecase").unwrap();
        let mut t_doc = CatalogueDocument::new(3, t_crate, t_layer.clone());
        t_doc.insert_type(
            CatalogueEntryKey::try_new("Item".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );

        let crate_name = CrateName::new("domain").unwrap();
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer.clone());
        doc.insert_type(
            CatalogueEntryKey::try_new("UserId".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::TypeAlias {
                    target: TypeRef::new("T::Item").unwrap(),
                    generics: vec![MethodGenericParam {
                        name: ParamName::new("T").unwrap(),
                        bounds: vec![],
                    }],
                },
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );

        let result = adapter.render(&[doc, t_doc], &[layer, t_layer], &opts).unwrap();
        let output = result.content().as_ref();
        assert!(
            !output.contains("alias_of"),
            "a generic-rooted alias target must not resolve to a same-named crate's type: \
             {output}"
        );
    }

    #[test]
    fn test_render_legacy_alias_parameter_shadows_declared_type() {
        // The same shadowing must apply when the alias parameter is declared in
        // the legacy entry-level generics field instead of the TypeAlias kind.
        use domain::tddd::catalogue_v2::identifiers::ParamName;
        use domain::tddd::catalogue_v2::methods::MethodGenericParam;

        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), FULL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let opts = ContractMapRenderOptions::default();

        let crate_name = CrateName::new("domain").unwrap();
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer.clone());

        for name in ["T", "RawId"] {
            doc.insert_type(
                CatalogueEntryKey::try_new(name.to_owned()).unwrap(),
                TypeEntry::new(
                    ItemAction::Add,
                    DataRole::value_object(),
                    TypeKindV2::Struct(StructKind::new(
                        StructShape::Plain { fields: vec![], has_stripped_fields: false },
                        None,
                    )),
                    vec![],
                    vec![],
                    vec![],
                    ModulePath::root(),
                    None,
                    vec![],
                    vec![],
                ),
            );
        }

        doc.insert_type(
            CatalogueEntryKey::try_new("UserId".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::TypeAlias {
                    target: TypeRef::new("Result<T, RawId>").unwrap(),
                    generics: vec![],
                },
                vec![],
                vec![MethodGenericParam { name: ParamName::new("T").unwrap(), bounds: vec![] }],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.content().as_ref();
        let source = render::type_rep_node_id("domain", "domain", "UserId");
        let raw_id = render::type_rep_node_id("domain", "domain", "RawId");
        let shadowed_id = render::type_rep_node_id("domain", "domain", "T");
        let raw_edge = format!("{source} ---|alias_of| {raw_id}");
        let shadowed_edge = format!("{source} ---|alias_of| {shadowed_id}");
        assert_eq!(
            output.matches(&raw_edge).count(),
            1,
            "the declared RawId target must produce exactly one alias_of edge: {output}"
        );
        assert!(
            !output.contains(&shadowed_edge),
            "the legacy alias parameter T must not resolve to the declared type T: {output}"
        );
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
        doc.insert_trait(
            CatalogueEntryKey::try_new("MyPort".to_owned()).unwrap(),
            make_empty_trait_entry(),
        );

        // Add a type.
        doc.insert_type(
            CatalogueEntryKey::try_new("MyAdapter".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::SecondaryAdapter,
                TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );

        // Trait impl: MyAdapter implements MyPort.
        doc.push_trait_impl(TraitImplDeclV2::new(
            TypeRef::new("MyPort").unwrap(),
            TypeRef::new("MyAdapter").unwrap(),
        ));

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.content().as_ref();
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
        let text = output.content().as_ref();
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
        let text = result.content().as_ref();
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
        domain_doc.insert_trait(
            CatalogueEntryKey::try_new("domain::tddd::MyPort".to_owned()).unwrap(),
            make_empty_trait_entry(),
        );

        // infrastructure catalogue: declares MyAdapter type + cross-crate trait impl.
        let mut infra_doc = CatalogueDocument::new(
            3,
            CrateName::new("infrastructure").unwrap(),
            infra_layer.clone(),
        );
        infra_doc.insert_type(
            CatalogueEntryKey::try_new("MyAdapter".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::SecondaryAdapter,
                TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );
        // Qualified cross-crate trait_ref (as used in real catalogues).
        infra_doc.push_trait_impl(TraitImplDeclV2::new(
            TypeRef::new("domain::tddd::MyPort").unwrap(),
            TypeRef::new("MyAdapter").unwrap(),
        ));

        let result =
            adapter.render(&[domain_doc, infra_doc], &[domain_layer, infra_layer], &opts).unwrap();
        let output = result.content().as_ref();
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
            doc.insert_type(
                CatalogueEntryKey::try_new("MyType".to_owned()).unwrap(),
                TypeEntry::new(
                    ItemAction::Add,
                    DataRole::value_object(),
                    TypeKindV2::Struct(StructKind::new(
                        StructShape::Plain { fields: vec![], has_stripped_fields: false },
                        None,
                    )),
                    vec![],
                    vec![],
                    vec![],
                    ModulePath::root(),
                    None,
                    vec![],
                    vec![],
                ),
            );
            doc.push_trait_impl(TraitImplDeclV2::new(
                TypeRef::new(trait_ref).unwrap(),
                TypeRef::new("MyType").unwrap(),
            ));

            let result = adapter.render(&[doc], &[layer], &opts).unwrap();
            let output = result.content().as_ref();
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
        doc.insert_trait(
            CatalogueEntryKey::try_new("RemovedTrait".to_owned()).unwrap(),
            TraitEntry::new(
                ItemAction::Delete,
                ContractRole::SecondaryPort,
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );

        // A type that tries to impl the deleted trait — the trait_impl edge must be skipped
        // (deleted trait is absent from the trait index).
        doc.insert_type(
            CatalogueEntryKey::try_new("MyAdapter".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::SecondaryAdapter,
                TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );
        doc.push_trait_impl(TraitImplDeclV2::new(
            TypeRef::new("RemovedTrait").unwrap(),
            TypeRef::new("MyAdapter").unwrap(),
        ));

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.content().as_ref();
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
        doc.insert_function(
            fn_path,
            FunctionEntry::new(
                ItemAction::Delete,
                FunctionRole::FreeFunction,
                vec![],
                TypeRef::new("()").unwrap(),
                false,
                vec![],
                vec![],
                None,
                vec![],
                vec![],
            ),
        );

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.content().as_ref();
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
        doc.insert_type(
            CatalogueEntryKey::try_new("Product".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::entity().unwrap(),
                TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![field], has_stripped_fields: false },
                    None,
                )),
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.content().as_ref();
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
            false,
            vec![],
            vec![],
            vec![],
            ItemAction::Add,
            None,
        );
        doc.insert_type(
            CatalogueEntryKey::try_new("Converter".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::domain_service(),
                TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                vec![method_with_generic_params],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.content().as_ref();
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
        doc.insert_type(
            CatalogueEntryKey::try_new("OldToken".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Delete,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );

        // Type with a field pointing at the deleted type — edge must be silently skipped.
        let field =
            FieldDecl::new(FieldName::new("token").unwrap(), TypeRef::new("OldToken").unwrap());
        doc.insert_type(
            CatalogueEntryKey::try_new("Session".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::entity().unwrap(),
                TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![field], has_stripped_fields: false },
                    None,
                )),
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.content().as_ref();
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
            doc.insert_type(
                CatalogueEntryKey::try_new(type_name.to_owned()).unwrap(),
                TypeEntry::new(
                    ItemAction::Add,
                    DataRole::value_object(),
                    TypeKindV2::Struct(StructKind::new(
                        StructShape::Plain { fields: vec![], has_stripped_fields: false },
                        None,
                    )),
                    vec![],
                    vec![],
                    vec![],
                    ModulePath::root(),
                    None,
                    vec![],
                    vec![],
                ),
            );
        }

        // Method returning `Result<ContractMapContent, ContractMapRendererError>`.
        let render_method = MethodDeclaration::new(
            MethodName::new("render").unwrap(),
            Some(SelfReceiver::SharedRef),
            vec![],
            TypeRef::new("Result<ContractMapContent, ContractMapRendererError>").unwrap(),
            false,
            false,
            vec![],
            vec![],
            vec![],
            ItemAction::Add,
            None,
        );
        doc.insert_type(
            CatalogueEntryKey::try_new("ContractMapRenderer".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::domain_service(),
                TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                vec![render_method],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.content().as_ref();
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

            doc.insert_type(
                CatalogueEntryKey::try_new(target_type.to_owned()).unwrap(),
                TypeEntry::new(
                    ItemAction::Add,
                    DataRole::entity().unwrap(),
                    TypeKindV2::Struct(StructKind::new(
                        StructShape::Plain { fields: vec![], has_stripped_fields: false },
                        None,
                    )),
                    vec![],
                    vec![],
                    vec![],
                    ModulePath::root(),
                    None,
                    vec![],
                    vec![],
                ),
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
                false,
                vec![],
                vec![],
                vec![],
                ItemAction::Add,
                None,
            );
            doc.insert_type(
                CatalogueEntryKey::try_new(owner_type.to_owned()).unwrap(),
                TypeEntry::new(
                    ItemAction::Add,
                    DataRole::domain_service(),
                    TypeKindV2::Struct(StructKind::new(
                        StructShape::Plain { fields: vec![], has_stripped_fields: false },
                        None,
                    )),
                    vec![method],
                    vec![],
                    vec![],
                    ModulePath::root(),
                    None,
                    vec![],
                    vec![],
                ),
            );

            let result = adapter.render(&[doc], &[layer], &opts).unwrap();
            let output = result.content().as_ref();
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
            false,
            vec![],
            vec![],
            vec![],
            ItemAction::Add,
            None,
        );
        doc.insert_type(
            CatalogueEntryKey::try_new("Store".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::domain_service(),
                TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                vec![method],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.content().as_ref();
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
        doc.insert_trait(
            CatalogueEntryKey::try_new("ContractMapRenderer".to_owned()).unwrap(),
            make_empty_trait_entry(),
        );

        // Declare an adapter with a constructor method returning Self.
        let new_method = MethodDeclaration::new(
            MethodName::new("new").unwrap(),
            None, // no self receiver — constructor
            vec![],
            TypeRef::new("Self").unwrap(),
            false,
            false,
            vec![],
            vec![],
            vec![],
            ItemAction::Add,
            None,
        );
        doc.insert_type(
            CatalogueEntryKey::try_new("ContractMapRendererAdapter".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::SecondaryAdapter,
                TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                vec![new_method],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );

        // Trait impl: ContractMapRendererAdapter implements ContractMapRenderer.
        doc.push_trait_impl(TraitImplDeclV2::new(
            TypeRef::new("ContractMapRenderer").unwrap(),
            TypeRef::new("ContractMapRendererAdapter").unwrap(),
        ));

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.content().as_ref();

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

        doc.insert_type(
            CatalogueEntryKey::try_new("MyType".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );
        doc.insert_trait(
            CatalogueEntryKey::try_new("MyTrait".to_owned()).unwrap(),
            make_empty_trait_entry(),
        );

        let result = adapter.render(&[doc], &[layer], &opts).unwrap();
        let output = result.content().as_ref();

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
    // dyn Trait TypeRef and trait_impl resolution (AC-01–AC-07)
    // -----------------------------------------------------------------------

    #[test]
    fn test_contract_map_dyn_trait_all_type_ref_positions_emit_trait_edges() {
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, CrateName::new("domain").unwrap(), layer.clone());

        for trait_name in [
            "DeclaredPort",
            "FieldPort",
            "VariantPort",
            "AliasPort",
            "FunctionParamPort",
            "FunctionReturnPort",
            "MethodParamPort",
        ] {
            doc.insert_trait(
                CatalogueEntryKey::try_new(trait_name.to_owned()).unwrap(),
                make_empty_trait_entry(),
            );
        }

        let factory_method = MethodDeclaration::new(
            MethodName::new("build").unwrap(),
            Some(SelfReceiver::SharedRef),
            vec![ParamDeclaration::new(
                domain::tddd::catalogue_v2::identifiers::ParamName::new("port").unwrap(),
                TypeRef::new("Arc<dyn MethodParamPort>").unwrap(),
            )],
            TypeRef::new("Arc<dyn DeclaredPort>").unwrap(),
            false,
            false,
            vec![],
            vec![],
            vec![],
            ItemAction::Add,
            None,
        );
        doc.insert_trait(
            CatalogueEntryKey::try_new("Factory".to_owned()).unwrap(),
            make_trait_entry_with_methods(vec![factory_method]),
        );
        doc.insert_type(
            CatalogueEntryKey::try_new("FieldOwner".to_owned()).unwrap(),
            make_plain_struct_entry(
                vec![FieldDecl::new(
                    FieldName::new("port").unwrap(),
                    TypeRef::new("Arc<dyn FieldPort>").unwrap(),
                )],
                vec![],
            ),
        );
        doc.insert_type(
            CatalogueEntryKey::try_new("VariantOwner".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::Enum {
                    variants: vec![VariantDecl::tuple(
                        VariantName::new("Port").unwrap(),
                        vec![TypeRef::new("Arc<dyn VariantPort>").unwrap()],
                    )],
                },
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );
        doc.insert_type(
            CatalogueEntryKey::try_new("AliasOwner".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Add,
                DataRole::value_object(),
                TypeKindV2::TypeAlias {
                    target: TypeRef::new("Arc<dyn AliasPort>").unwrap(),
                    generics: vec![],
                },
                vec![],
                vec![],
                vec![],
                ModulePath::root(),
                None,
                vec![],
                vec![],
            ),
        );
        doc.insert_function(
            FunctionPath::at_root(
                CrateName::new("domain").unwrap(),
                FunctionName::new("build_ports").unwrap(),
            ),
            FunctionEntry::new(
                ItemAction::Add,
                FunctionRole::FreeFunction,
                vec![ParamDeclaration::new(
                    domain::tddd::catalogue_v2::identifiers::ParamName::new("port").unwrap(),
                    TypeRef::new("Arc<dyn FunctionParamPort>").unwrap(),
                )],
                TypeRef::new("Arc<dyn FunctionReturnPort>").unwrap(),
                false,
                vec![],
                vec![],
                None,
                vec![],
                vec![],
            ),
        );

        let rendered = render_and_scan(&[doc], &[layer]);
        let factory_build =
            format!("{}_build", render::trait_node_id("domain", "domain", "Factory"));
        assert_edge_count(
            &rendered,
            &factory_build,
            "--o",
            None,
            &render::trait_rep_node_id("domain", "domain", "MethodParamPort"),
            1,
        );
        assert_edge_count(
            &rendered,
            &factory_build,
            "-->",
            None,
            &render::trait_rep_node_id("domain", "domain", "DeclaredPort"),
            1,
        );
        assert_edge_count(
            &rendered,
            &render::type_rep_node_id("domain", "domain", "FieldOwner"),
            "--o",
            Some("port"),
            &render::trait_rep_node_id("domain", "domain", "FieldPort"),
            1,
        );
        assert_edge_count(
            &rendered,
            &format!("{}_Port", render::type_node_id("domain", "domain", "VariantOwner")),
            "--o",
            None,
            &render::trait_rep_node_id("domain", "domain", "VariantPort"),
            1,
        );
        assert_edge_count(
            &rendered,
            &render::type_rep_node_id("domain", "domain", "AliasOwner"),
            "---",
            Some("alias_of"),
            &render::trait_rep_node_id("domain", "domain", "AliasPort"),
            1,
        );
        let build_ports = render::function_node_id("domain", "domain", "domain::build_ports");
        assert_edge_count(
            &rendered,
            &build_ports,
            "--o",
            None,
            &render::trait_rep_node_id("domain", "domain", "FunctionParamPort"),
            1,
        );
        assert_edge_count(
            &rendered,
            &build_ports,
            "-->",
            None,
            &render::trait_rep_node_id("domain", "domain", "FunctionReturnPort"),
            1,
        );
    }

    #[test]
    fn test_contract_map_dyn_external_trait_silently_skips_edge() {
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, CrateName::new("domain").unwrap(), layer.clone());
        let method = MethodDeclaration::new(
            MethodName::new("inspect").unwrap(),
            Some(SelfReceiver::SharedRef),
            vec![],
            TypeRef::new("Arc<dyn std::fmt::Debug>").unwrap(),
            false,
            false,
            vec![],
            vec![],
            vec![],
            ItemAction::Add,
            None,
        );
        doc.insert_trait(
            CatalogueEntryKey::try_new("Inspector".to_owned()).unwrap(),
            make_trait_entry_with_methods(vec![method]),
        );

        let rendered = render_and_scan(&[doc], &[layer]);
        assert!(
            rendered.edge_lines.iter().all(|line| !line.contains("Debug")),
            "undeclared external traits must not produce edges: {}",
            rendered.output
        );
    }

    #[test]
    fn test_contract_map_qualified_duplicate_module_types_resolve_to_distinct_nodes() {
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, CrateName::new("domain").unwrap(), layer.clone());
        for key in ["domain::alpha::Shared", "domain::beta::Shared"] {
            doc.insert_type(
                CatalogueEntryKey::try_new(key.to_owned()).unwrap(),
                make_plain_struct_entry(vec![], vec![]),
            );
        }
        doc.insert_type(
            CatalogueEntryKey::try_new("Owner".to_owned()).unwrap(),
            make_plain_struct_entry(
                vec![
                    FieldDecl::new(
                        FieldName::new("alpha").unwrap(),
                        TypeRef::new("domain::alpha::Shared").unwrap(),
                    ),
                    FieldDecl::new(
                        FieldName::new("beta").unwrap(),
                        TypeRef::new("domain::beta::Shared").unwrap(),
                    ),
                ],
                vec![],
            ),
        );

        let rendered = render_and_scan(&[doc], &[layer]);
        let owner = render::type_rep_node_id("domain", "domain", "Owner");
        for (field, target_name) in
            [("alpha", "domain::alpha::Shared"), ("beta", "domain::beta::Shared")]
        {
            assert_edge_count(
                &rendered,
                &owner,
                "--o",
                Some(field),
                &render::type_rep_node_id("domain", "domain", target_name),
                1,
            );
        }
    }

    #[test]
    fn test_contract_map_duplicate_module_impl_nodes_and_edges_remain_distinct() {
        let layer = LayerId::try_new("domain").unwrap();
        let build_doc = |modules: &[&str]| {
            let mut doc =
                CatalogueDocument::new(5, CrateName::new("domain").unwrap(), layer.clone());
            for module in modules {
                let module_path = ModulePath::from_segments(vec![(*module).to_owned()]).unwrap();
                doc.insert_type(
                    CatalogueEntryKey::try_new(format!("{module}::Input")).unwrap(),
                    TypeEntry::new(
                        ItemAction::Add,
                        DataRole::value_object(),
                        TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
                        vec![],
                        vec![],
                        vec![],
                        module_path.clone(),
                        None,
                        vec![],
                        vec![],
                    ),
                );
                doc.insert_trait(
                    CatalogueEntryKey::try_new(format!("{module}::Port")).unwrap(),
                    TraitEntry::new(
                        ItemAction::Add,
                        ContractRole::SpecificationPort,
                        vec![],
                        vec![],
                        vec![],
                        vec![],
                        vec![],
                        vec![],
                        module_path,
                        None,
                        vec![],
                        vec![],
                    ),
                );
                doc.push_trait_impl(TraitImplDeclV2::from_parts(
                    ItemAction::Add,
                    TypeRef::new(format!("domain::{module}::Port<domain::{module}::Input>"))
                        .unwrap(),
                    TypeRef::new(format!("domain::{module}::Input<T>")).unwrap(),
                    vec![domain::tddd::catalogue_v2::methods::MethodGenericParam {
                        name: domain::tddd::catalogue_v2::ParamName::new("T").unwrap(),
                        bounds: vec![],
                    }],
                    vec![],
                ));
            }
            doc
        };

        let forward =
            render_and_scan(&[build_doc(&["alpha", "beta"])], std::slice::from_ref(&layer));
        let reverse =
            render_and_scan(&[build_doc(&["beta", "alpha"])], std::slice::from_ref(&layer));
        let mut forward_edges = forward.edge_lines.clone();
        let mut reverse_edges = reverse.edge_lines.clone();
        forward_edges.sort();
        reverse_edges.sort();
        assert_eq!(
            forward_edges, reverse_edges,
            "duplicate-module resolution must not depend on catalogue insertion order"
        );

        let alpha_source = render::type_rep_node_id("domain", "domain", "alpha::Input");
        let beta_source = render::type_rep_node_id("domain", "domain", "beta::Input");
        let alpha_target = render::trait_rep_node_id("domain", "domain", "alpha::Port");
        let beta_target = render::trait_rep_node_id("domain", "domain", "beta::Port");
        for node_id in [&alpha_source, &beta_source, &alpha_target, &beta_target] {
            assert!(
                forward.output.contains(node_id),
                "fully qualified node id must be rendered exactly once: {node_id}\n{}",
                forward.output
            );
        }

        assert_edge_count(&forward, &alpha_source, "-.impl.->", None, &alpha_target, 1);
        assert_edge_count(&forward, &beta_source, "-.impl.->", None, &beta_target, 1);
        assert_edge_count(&forward, &alpha_source, "-.impl.->", None, &beta_target, 0);
        assert_edge_count(&forward, &beta_source, "-.impl.->", None, &alpha_target, 0);
        let impl_edge_count =
            forward.edge_lines.iter().filter(|line| line.contains(" -.impl.->")).count();
        assert_eq!(
            impl_edge_count, 2,
            "only the two qualified impl edges should be emitted: {:?}",
            forward.edge_lines
        );
    }

    #[test]
    fn test_contract_map_dyn_same_name_type_and_trait_resolve_separately() {
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, CrateName::new("domain").unwrap(), layer.clone());
        doc.insert_type(
            CatalogueEntryKey::try_new("Foo".to_owned()).unwrap(),
            make_plain_struct_entry(vec![], vec![]),
        );
        doc.insert_trait(
            CatalogueEntryKey::try_new("Foo".to_owned()).unwrap(),
            make_empty_trait_entry(),
        );
        doc.insert_type(
            CatalogueEntryKey::try_new("Owner".to_owned()).unwrap(),
            make_plain_struct_entry(
                vec![
                    FieldDecl::new(FieldName::new("plain").unwrap(), TypeRef::new("Foo").unwrap()),
                    FieldDecl::new(
                        FieldName::new("dynamic").unwrap(),
                        TypeRef::new("Arc<dyn Foo>").unwrap(),
                    ),
                ],
                vec![],
            ),
        );

        let rendered = render_and_scan(&[doc], &[layer]);
        let owner = render::type_rep_node_id("domain", "domain", "Owner");
        assert_edge_count(
            &rendered,
            &owner,
            "--o",
            Some("plain"),
            &render::type_rep_node_id("domain", "domain", "Foo"),
            1,
        );
        assert_edge_count(
            &rendered,
            &owner,
            "--o",
            Some("dynamic"),
            &render::trait_rep_node_id("domain", "domain", "Foo"),
            1,
        );
    }

    #[test]
    fn test_contract_map_dyn_multiple_bounds_resolve_declared_traits_only() {
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, CrateName::new("domain").unwrap(), layer.clone());
        for trait_name in ["DeclaredPort", "DeclaredMarker"] {
            doc.insert_trait(
                CatalogueEntryKey::try_new(trait_name.to_owned()).unwrap(),
                make_empty_trait_entry(),
            );
        }
        doc.insert_type(
            CatalogueEntryKey::try_new("Owner".to_owned()).unwrap(),
            make_plain_struct_entry(
                vec![FieldDecl::new(
                    FieldName::new("ports").unwrap(),
                    TypeRef::new("Arc<dyn DeclaredPort + DeclaredMarker + Send>").unwrap(),
                )],
                vec![],
            ),
        );

        let rendered = render_and_scan(&[doc], &[layer]);
        let owner = render::type_rep_node_id("domain", "domain", "Owner");
        for trait_name in ["DeclaredPort", "DeclaredMarker"] {
            assert_edge_count(
                &rendered,
                &owner,
                "--o",
                Some("ports"),
                &render::trait_rep_node_id("domain", "domain", trait_name),
                1,
            );
        }
        assert!(
            rendered.edge_lines.iter().all(|line| !line.contains("Send")),
            "undeclared marker traits must be silently skipped: {}",
            rendered.output
        );
    }

    #[test]
    fn test_contract_map_dyn_generic_and_associated_types_emit_deduplicated_edges() {
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, CrateName::new("domain").unwrap(), layer.clone());
        doc.insert_trait(
            CatalogueEntryKey::try_new("DeclaredPort".to_owned()).unwrap(),
            make_empty_trait_entry(),
        );
        for type_name in ["GenericType", "AssociatedType"] {
            doc.insert_type(
                CatalogueEntryKey::try_new(type_name.to_owned()).unwrap(),
                make_plain_struct_entry(vec![], vec![]),
            );
        }
        doc.insert_type(
            CatalogueEntryKey::try_new("Owner".to_owned()).unwrap(),
            make_plain_struct_entry(
                vec![FieldDecl::new(
                    FieldName::new("port").unwrap(),
                    TypeRef::new("Arc<dyn DeclaredPort<GenericType, Item = AssociatedType>>")
                        .unwrap(),
                )],
                vec![],
            ),
        );

        let rendered = render_and_scan(&[doc], &[layer]);
        let owner = render::type_rep_node_id("domain", "domain", "Owner");
        for target in [
            render::trait_rep_node_id("domain", "domain", "DeclaredPort"),
            render::type_rep_node_id("domain", "domain", "GenericType"),
            render::type_rep_node_id("domain", "domain", "AssociatedType"),
        ] {
            assert_edge_count(&rendered, &owner, "--o", Some("port"), &target, 1);
        }
    }

    #[test]
    fn test_contract_map_dyn_accepted_qualified_forms_resolve_to_trait() {
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, CrateName::new("domain").unwrap(), layer.clone());
        doc.insert_trait(
            CatalogueEntryKey::try_new("domain::port::DeclaredPort".to_owned()).unwrap(),
            make_empty_trait_entry(),
        );
        doc.insert_type(
            CatalogueEntryKey::try_new("Owner".to_owned()).unwrap(),
            make_plain_struct_entry(
                vec![
                    FieldDecl::new(
                        FieldName::new("crate_port").unwrap(),
                        TypeRef::new("Arc<dyn crate::port::DeclaredPort>").unwrap(),
                    ),
                    FieldDecl::new(
                        FieldName::new("module_port").unwrap(),
                        TypeRef::new("Arc<dyn port::DeclaredPort>").unwrap(),
                    ),
                    FieldDecl::new(
                        FieldName::new("qualified_port").unwrap(),
                        TypeRef::new("Arc<dyn domain::port::DeclaredPort>").unwrap(),
                    ),
                ],
                vec![],
            ),
        );

        let rendered = render_and_scan(&[doc], &[layer]);
        let owner = render::type_rep_node_id("domain", "domain", "Owner");
        let target = render::trait_rep_node_id("domain", "domain", "domain::port::DeclaredPort");
        for field_name in ["crate_port", "module_port", "qualified_port"] {
            assert_edge_count(&rendered, &owner, "--o", Some(field_name), &target, 1);
        }
    }

    #[test]
    fn test_contract_map_trait_impl_shared_resolver_preserves_and_extends_resolution() {
        let domain_layer = LayerId::try_new("domain").unwrap();
        let adapter_layer = LayerId::try_new("infrastructure").unwrap();
        let mut domain_doc =
            CatalogueDocument::new(3, CrateName::new("domain").unwrap(), domain_layer.clone());
        domain_doc.insert_trait(
            CatalogueEntryKey::try_new("domain::port::DeclaredPort".to_owned()).unwrap(),
            make_empty_trait_entry(),
        );
        for type_name in ["CrateAdapter", "QualifiedAdapter", "BareAdapter"] {
            domain_doc.insert_type(
                CatalogueEntryKey::try_new(type_name.to_owned()).unwrap(),
                make_plain_struct_entry(vec![], vec![]),
            );
        }
        for (trait_ref, for_type) in [
            ("crate::port::DeclaredPort", "CrateAdapter"),
            ("domain::port::DeclaredPort", "QualifiedAdapter"),
            ("DeclaredPort", "BareAdapter"),
        ] {
            domain_doc.push_trait_impl(TraitImplDeclV2::new(
                TypeRef::new(trait_ref).unwrap(),
                TypeRef::new(for_type).unwrap(),
            ));
        }

        let mut adapter_doc =
            CatalogueDocument::new(3, CrateName::new("adapter").unwrap(), adapter_layer.clone());
        adapter_doc.insert_type(
            CatalogueEntryKey::try_new("CrossAdapter".to_owned()).unwrap(),
            make_plain_struct_entry(vec![], vec![]),
        );
        adapter_doc.insert_type(
            CatalogueEntryKey::try_new("ExternalAdapter".to_owned()).unwrap(),
            make_plain_struct_entry(vec![], vec![]),
        );
        adapter_doc.push_trait_impl(TraitImplDeclV2::new(
            TypeRef::new("domain::port::DeclaredPort").unwrap(),
            TypeRef::new("CrossAdapter").unwrap(),
        ));
        adapter_doc.push_trait_impl(TraitImplDeclV2::new(
            TypeRef::new("std::fmt::Debug").unwrap(),
            TypeRef::new("ExternalAdapter").unwrap(),
        ));

        let rendered = render_and_scan(&[domain_doc, adapter_doc], &[domain_layer, adapter_layer]);
        let target = render::trait_rep_node_id("domain", "domain", "domain::port::DeclaredPort");
        for source in [
            render::type_rep_node_id("domain", "domain", "CrateAdapter"),
            render::type_rep_node_id("domain", "domain", "QualifiedAdapter"),
            render::type_rep_node_id("domain", "domain", "BareAdapter"),
            render::type_rep_node_id("infrastructure", "adapter", "CrossAdapter"),
        ] {
            assert_edge_count(&rendered, &source, "-.impl.->", None, &target, 1);
        }
        assert!(
            rendered.edge_lines.iter().all(|line| !line.contains("Debug")),
            "workspace-external trait impls must stay silently skipped: {}",
            rendered.output
        );

        let type_ref_source = include_str!("render/type_ref.rs");
        assert_eq!(
            type_ref_source.matches("fn resolve_trait_ref_node_id").count(),
            1,
            "the four-step trait resolver must have one implementation"
        );
        assert!(
            type_ref_source
                .contains("resolve_trait_ref_node_id(candidate, current_crate, trait_index)"),
            "dyn Trait candidates must delegate to the shared resolver"
        );
        assert!(
            type_ref_source
                .contains("resolve_trait_ref_node_id(trait_ref_str, current_crate, trait_index)"),
            "trait_impl references must delegate to the shared resolver"
        );
    }

    // -----------------------------------------------------------------------
    // Helper constructors for tests
    // -----------------------------------------------------------------------

    struct RenderedContractMap {
        output: String,
        edge_lines: Vec<String>,
    }

    fn render_and_scan(
        catalogues: &[CatalogueDocument],
        layer_order: &[LayerId],
    ) -> RenderedContractMap {
        let tmp = tempfile::tempdir().unwrap();
        let path = write_style_config(tmp.path(), FULL_VALID_CONFIG);
        let adapter = ContractMapRendererAdapter::new(path);
        let output = adapter
            .render(catalogues, layer_order, &ContractMapRenderOptions::default())
            .unwrap()
            .content()
            .as_ref()
            .to_string();
        let edge_lines = output
            .lines()
            .map(str::trim)
            .filter(|line| {
                line.contains(" -->")
                    || line.contains(" --o")
                    || line.contains(" ---")
                    || line.contains(" -.impl.->")
                    || line.contains(" ==>")
            })
            .map(str::to_string)
            .collect();

        RenderedContractMap { output, edge_lines }
    }

    fn assert_edge_count(
        rendered: &RenderedContractMap,
        source: &str,
        arrow: &str,
        label: Option<&str>,
        target: &str,
        expected_count: usize,
    ) {
        let expected = match label {
            Some(label) => format!("{source} {arrow}|{label}| {target}"),
            None => format!("{source} {arrow} {target}"),
        };
        let actual_count =
            rendered.edge_lines.iter().filter(|line| line.as_str() == expected).count();
        assert_eq!(
            actual_count, expected_count,
            "expected {expected_count} edge(s) `{expected}`; output: {}",
            rendered.output
        );
    }

    fn make_plain_struct_entry(
        fields: Vec<FieldDecl>,
        methods: Vec<MethodDeclaration>,
    ) -> TypeEntry {
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields, has_stripped_fields: false },
                None,
            )),
            methods,
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        )
    }

    fn make_empty_trait_entry() -> TraitEntry {
        make_trait_entry_with_methods(vec![])
    }

    fn make_trait_entry_with_methods(methods: Vec<MethodDeclaration>) -> TraitEntry {
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SecondaryPort,
            methods,
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            ModulePath::root(),
            None,
            vec![],
            vec![],
        )
    }
}
