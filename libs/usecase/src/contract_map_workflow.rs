//! Contract Map render workflow (ADR 2026-04-17-1528 §D1).
//!
//! Hexagonal composition:
//!
//! * [`RenderContractMap`] — **primary port** (application_service trait).
//!   CLI and future adapters depend on this trait, not on the concrete
//!   interactor below, so composition roots stay substitutable.
//! * [`RenderContractMapInteractor`] — the interactor that orchestrates
//!   the secondary ports (`CatalogueLoader`, `ContractMapRenderer`,
//!   `ContractMapWriter`) and dispatches rendering through the domain port.
//!   It implements [`RenderContractMap`].
//!
//! The usecase layer stays pure-orchestrator per
//! `knowledge/conventions/hexagonal-architecture.md` §Usecase Purity:
//! no `std::fs`, no `println!`, no `chrono::Utc::now`, no env access.
//! All I/O flows through the domain ports.

use domain::TrackId;
use domain::tddd::catalogue_ports::{
    CatalogueLoader, CatalogueLoaderError, ContractMapWriter, ContractMapWriterError,
};
use domain::tddd::{
    ContractMapRenderOptions, ContractMapRenderer, ContractMapRendererError, LayerId,
};

/// Command input for [`RenderContractMap::execute`].
///
/// All fields accept raw strings so callers (e.g. the CLI) never need to
/// import domain types (`TrackId`, `LayerId`).
/// The interactor validates and converts them internally.
///
/// Fields mirror the CLI arguments (`sotp track contract-map <track-id>
/// [--layers l1,l2]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderContractMapCommand {
    /// Raw track identifier string (validated by the interactor).
    pub track_id: String,
    /// If `Some`, restricts rendering to the listed layer identifier
    /// strings. The interactor fails with
    /// [`RenderContractMapError::LayerNotFound`] when any supplied string
    /// is absent from the loader's output set, guarding against silent
    /// typos in CLI input.
    pub layer_filter: Option<Vec<String>>,
}

/// Output DTO returned by [`RenderContractMap::execute`] on success.
///
/// Carries lightweight metrics so CLI callers can report a post-write
/// summary without re-reading the generated markdown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderContractMapOutput {
    pub rendered_layer_count: usize,
    pub total_entry_count: usize,
}

/// Error variants surfaced by [`RenderContractMap::execute`].
///
/// Variant inventory matches the `usecase-types.json` declaration for the
/// `contract-map-v3-2026-05-15` track.
#[derive(Debug, thiserror::Error)]
pub enum RenderContractMapError {
    /// Failure inside a [`CatalogueLoader`] implementation.
    #[error(transparent)]
    CatalogueLoaderFailed(#[from] CatalogueLoaderError),

    /// Failure inside a [`ContractMapWriter`] implementation.
    #[error(transparent)]
    ContractMapWriterFailed(#[from] ContractMapWriterError),

    /// Failure inside a [`ContractMapRenderer`] implementation.
    #[error(transparent)]
    RendererFailed(#[from] ContractMapRendererError),

    /// The loader returned an empty layer set — no `tddd.enabled` layers
    /// exist for this track. Rendering an empty Contract Map is not a
    /// useful workflow, so we fail closed.
    #[error(
        "catalogue loader returned no enabled layers for track '{track_id}'; \
         check `architecture-rules.json` tddd blocks"
    )]
    EmptyCatalogue { track_id: String },

    /// The caller's `layer_filter` references a layer that the loader did
    /// not produce — typically a CLI typo or a disabled layer.
    #[error("layer '{layer_id}' is not a tddd.enabled layer for track '{track_id}'")]
    LayerNotFound { track_id: String, layer_id: String },

    /// The `track_id` string is not a valid track identifier.
    #[error("invalid track ID: {reason}")]
    InvalidTrackId { reason: String },
}

/// Primary port for the Contract Map render workflow.
///
/// CLI commands invoke the workflow through this trait so composition
/// roots can swap implementations (e.g., a no-op shim in tests).
pub trait RenderContractMap {
    /// Render the Contract Map for the given command.
    ///
    /// # Errors
    ///
    /// Returns [`RenderContractMapError`] if the loader fails, the
    /// writer fails, the enabled-layer set is empty, or a
    /// `layer_filter` entry does not appear in the loader output.
    /// Both syntactically invalid and absent layer names surface as
    /// [`RenderContractMapError::LayerNotFound`].
    fn execute(
        &self,
        cmd: &RenderContractMapCommand,
    ) -> Result<RenderContractMapOutput, RenderContractMapError>;
}

/// Default [`RenderContractMap`] implementation that composes a
/// [`CatalogueLoader`], a [`ContractMapRenderer`], and a
/// [`ContractMapWriter`].
pub struct RenderContractMapInteractor<L, R, W>
where
    L: CatalogueLoader,
    R: ContractMapRenderer,
    W: ContractMapWriter,
{
    loader: L,
    renderer: R,
    writer: W,
}

impl<L, R, W> RenderContractMapInteractor<L, R, W>
where
    L: CatalogueLoader,
    R: ContractMapRenderer,
    W: ContractMapWriter,
{
    /// Creates a new interactor wrapping the supplied secondary ports.
    #[must_use]
    pub fn new(loader: L, renderer: R, writer: W) -> Self {
        Self { loader, renderer, writer }
    }
}

impl<L, R, W> RenderContractMap for RenderContractMapInteractor<L, R, W>
where
    L: CatalogueLoader,
    R: ContractMapRenderer,
    W: ContractMapWriter,
{
    fn execute(
        &self,
        cmd: &RenderContractMapCommand,
    ) -> Result<RenderContractMapOutput, RenderContractMapError> {
        // Validate and convert the track_id string to the domain type.
        let track_id = TrackId::try_new(cmd.track_id.clone())
            .map_err(|e| RenderContractMapError::InvalidTrackId { reason: e.to_string() })?;

        let (layer_order, catalogues) = self.loader.load_all(&track_id)?;

        if layer_order.is_empty() {
            return Err(RenderContractMapError::EmptyCatalogue { track_id: cmd.track_id.clone() });
        }

        // Resolve layer_filter strings against the loaded layer set.
        // An absent or syntactically invalid layer name both produce
        // LayerNotFound — if a string cannot be found in layer_order it is
        // not a TDDD-enabled layer regardless of whether it would parse as a
        // valid LayerId, so the distinction is not meaningful to the caller.
        let layer_filter: Option<Vec<LayerId>> = cmd
            .layer_filter
            .as_ref()
            .map(|names| {
                names
                    .iter()
                    .map(|name| {
                        layer_order
                            .iter()
                            .find(|l| l.as_ref() == name.as_str())
                            .cloned()
                            .ok_or_else(|| RenderContractMapError::LayerNotFound {
                                track_id: cmd.track_id.clone(),
                                layer_id: name.clone(),
                            })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;

        // Collect catalogues from BTreeMap into a Vec<CatalogueDocument> for the renderer port.
        // CatalogueLoader.load_all() returns BTreeMap<LayerId, CatalogueDocument> (1 layer 1 crate).
        // The renderer port accepts &[CatalogueDocument] to allow 1 layer N crate in the future
        // (T010 acceptance test). In the production path, slice length equals the number of layers.
        let docs = catalogues.values().cloned().collect::<Vec<_>>();

        // Build render options: pass the active layer filter (or empty for all layers).
        let opts = ContractMapRenderOptions { layers: layer_filter.clone().unwrap_or_default() };

        let content = self.renderer.render(&docs, &layer_order, &opts)?;

        self.writer.write(&track_id, &content)?;

        // `rendered_layer_count` reflects only the layers that were actually rendered
        // (respecting `layer_filter`), while `total_entry_count` reflects the full
        // loader catalogue volume regardless of any filter — it is a coarse "how many
        // types does the track catalogue contain?" metric, not a post-filter count.
        let active: Vec<&LayerId> = match layer_filter.as_deref() {
            Some(f) if !f.is_empty() => layer_order.iter().filter(|l| f.contains(l)).collect(),
            _ => layer_order.iter().collect(),
        };
        let total_entry_count: usize =
            catalogues.values().map(|d| d.types.len() + d.traits.len() + d.functions.len()).sum();

        Ok(RenderContractMapOutput { rendered_layer_count: active.len(), total_entry_count })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use std::collections::BTreeMap;

    use domain::tddd::ContractMapContent;
    use domain::tddd::catalogue_v2::document::CatalogueDocument;
    use domain::tddd::catalogue_v2::entries::{FunctionEntry, TraitEntry, TypeEntry};
    use domain::tddd::catalogue_v2::identifiers::{
        CrateName, FunctionName, FunctionPath, TraitName, TypeName,
    };
    use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole, FunctionRole, ItemAction};
    use domain::tddd::catalogue_v2::{MethodDeclaration, ModulePath, TypeKindV2};
    use mockall::{mock, predicate};

    use super::*;

    mock! {
        pub Loader {}
        impl CatalogueLoader for Loader {
            fn load_all(
                &self,
                track_id: &TrackId,
            ) -> Result<
                (Vec<LayerId>, BTreeMap<LayerId, CatalogueDocument>),
                CatalogueLoaderError,
            >;
        }
    }

    mock! {
        pub Writer {}
        impl ContractMapWriter for Writer {
            fn write(
                &self,
                track_id: &TrackId,
                content: &ContractMapContent,
            ) -> Result<(), ContractMapWriterError>;
        }
    }

    mock! {
        pub Renderer {}
        impl ContractMapRenderer for Renderer {
            fn render(
                &self,
                catalogues: &[domain::tddd::catalogue_v2::document::CatalogueDocument],
                layer_order: &[LayerId],
                opts: &ContractMapRenderOptions,
            ) -> Result<ContractMapContent, ContractMapRendererError>;
        }
    }

    fn layer(name: &str) -> LayerId {
        LayerId::try_new(name.to_owned()).unwrap()
    }

    fn empty_v3_doc(crate_name: &str) -> CatalogueDocument {
        CatalogueDocument::new(3, CrateName::new(crate_name).unwrap(), layer(crate_name))
    }

    fn three_layer_catalogues() -> (Vec<LayerId>, BTreeMap<LayerId, CatalogueDocument>) {
        let domain_layer = layer("domain");
        let usecase_layer = layer("usecase");
        let infra_layer = layer("infrastructure");
        let order = vec![domain_layer.clone(), usecase_layer.clone(), infra_layer.clone()];
        let mut catalogues = BTreeMap::new();

        // domain: 1 type entry (User)
        let mut domain_doc = empty_v3_doc("domain");
        domain_doc.types.insert(
            TypeName::new("User").unwrap(),
            TypeEntry {
                action: ItemAction::Add,
                role: DataRole::Entity,
                kind: TypeKindV2::PlainStruct {
                    fields: vec![],
                    has_stripped_fields: false,
                    typestate: None,
                },
                methods: vec![],
                trait_impls: vec![],
                module_path: ModulePath::root(),
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );
        catalogues.insert(domain_layer, domain_doc);

        // usecase: 2 trait entries (RegisterUser, RegisterUserCommand)
        let mut usecase_doc = empty_v3_doc("usecase");
        for trait_name in ["RegisterUser", "RegisterUserCommand"] {
            usecase_doc.traits.insert(
                TraitName::new(trait_name).unwrap(),
                TraitEntry {
                    action: ItemAction::Add,
                    role: ContractRole::ApplicationService,
                    methods: vec![MethodDeclaration::new(
                        domain::tddd::catalogue_v2::identifiers::MethodName::new("execute")
                            .unwrap(),
                        None,
                        vec![],
                        domain::tddd::catalogue_v2::identifiers::TypeRef::new("()").unwrap(),
                        false,
                        None,
                    )],
                    supertrait_bounds: vec![],
                    module_path: ModulePath::root(),
                    docs: None,
                    spec_refs: vec![],
                    informal_grounds: vec![],
                },
            );
        }
        catalogues.insert(usecase_layer, usecase_doc);

        // infra: 1 function entry — exercises the d.functions.len() branch of total_entry_count
        let mut infra_doc = empty_v3_doc("infrastructure");
        let fn_crate = CrateName::new("infrastructure").unwrap();
        let fn_path =
            FunctionPath::at_root(fn_crate, FunctionName::new("render_contract_map").unwrap());
        infra_doc.functions.insert(
            fn_path,
            FunctionEntry {
                action: ItemAction::Add,
                role: FunctionRole::FreeFunction,
                params: vec![],
                returns: domain::tddd::catalogue_v2::identifiers::TypeRef::new(
                    "ContractMapContent",
                )
                .unwrap(),
                is_async: false,
                generics: vec![],
                where_predicates: vec![],
                docs: None,
                spec_refs: vec![],
                informal_grounds: vec![],
            },
        );
        catalogues.insert(infra_layer, infra_doc);

        (order, catalogues)
    }

    #[test]
    fn test_execute_happy_path_calls_writer_and_returns_metrics() {
        let (order, catalogues) = three_layer_catalogues();
        let mut loader = MockLoader::new();
        loader
            .expect_load_all()
            .with(predicate::function(|t: &TrackId| t.as_ref() == "t001"))
            .times(1)
            .returning(move |_: &TrackId| Ok((order.clone(), catalogues.clone())));

        let mut renderer = MockRenderer::new();
        renderer.expect_render().times(1).returning(|_catalogues, _layer_order, _opts| {
            Ok(ContractMapContent::new("```mermaid\nflowchart LR\n```\n"))
        });

        let mut writer = MockWriter::new();
        writer.expect_write().times(1).returning(|_: &TrackId, content: &ContractMapContent| {
            assert!(content.as_ref().contains("flowchart LR"));
            Ok(())
        });

        let interactor = RenderContractMapInteractor::new(loader, renderer, writer);
        let cmd = RenderContractMapCommand { track_id: "t001".to_owned(), layer_filter: None };
        let out = interactor.execute(&cmd).unwrap();
        assert_eq!(out.rendered_layer_count, 3);
        assert_eq!(out.total_entry_count, 4); // 1 type (domain) + 2 traits (usecase) + 1 function (infra)
    }

    #[test]
    fn test_execute_propagates_loader_error() {
        let mut loader = MockLoader::new();
        loader.expect_load_all().returning(|_: &TrackId| {
            Err(CatalogueLoaderError::LayerDiscoveryFailed { reason: "boom".to_owned() })
        });
        // Renderer and Writer must NOT run when the loader fails — enforce via mockall.
        let mut renderer = MockRenderer::new();
        renderer.expect_render().times(0);
        let mut writer = MockWriter::new();
        writer.expect_write().times(0);

        let interactor = RenderContractMapInteractor::new(loader, renderer, writer);
        let cmd = RenderContractMapCommand { track_id: "t002".to_owned(), layer_filter: None };
        let err = interactor.execute(&cmd).unwrap_err();
        assert!(matches!(err, RenderContractMapError::CatalogueLoaderFailed(_)));
    }

    #[test]
    fn test_execute_propagates_renderer_error_as_renderer_failed() {
        let (order, catalogues) = three_layer_catalogues();
        let mut loader = MockLoader::new();
        loader
            .expect_load_all()
            .returning(move |_: &TrackId| Ok((order.clone(), catalogues.clone())));

        let mut renderer = MockRenderer::new();
        renderer.expect_render().returning(|_catalogues, _layer_order, _opts| {
            Err(ContractMapRendererError::RenderFailed { reason: "malformed TypeRef".to_owned() })
        });

        // Writer must NOT run when the renderer fails — enforce via mockall.
        let mut writer = MockWriter::new();
        writer.expect_write().times(0);

        let interactor = RenderContractMapInteractor::new(loader, renderer, writer);
        let cmd = RenderContractMapCommand { track_id: "t006".to_owned(), layer_filter: None };
        let err = interactor.execute(&cmd).unwrap_err();
        assert!(
            matches!(
                err,
                RenderContractMapError::RendererFailed(
                    ContractMapRendererError::RenderFailed { .. }
                )
            ),
            "expected RendererFailed(RenderFailed {{ .. }}), got {err:?}"
        );
    }

    #[test]
    fn test_execute_propagates_writer_error() {
        let (order, catalogues) = three_layer_catalogues();
        let mut loader = MockLoader::new();
        loader
            .expect_load_all()
            .returning(move |_: &TrackId| Ok((order.clone(), catalogues.clone())));

        let mut renderer = MockRenderer::new();
        renderer.expect_render().returning(|_catalogues, _layer_order, _opts| {
            Ok(ContractMapContent::new("```mermaid\nflowchart LR\n```\n"))
        });

        let mut writer = MockWriter::new();
        writer.expect_write().returning(|_: &TrackId, _: &ContractMapContent| {
            Err(ContractMapWriterError::IoError {
                path: std::path::PathBuf::from("/tmp/fail"),
                reason: "disk full".to_owned(),
            })
        });

        let interactor = RenderContractMapInteractor::new(loader, renderer, writer);
        let cmd = RenderContractMapCommand { track_id: "t003".to_owned(), layer_filter: None };
        let err = interactor.execute(&cmd).unwrap_err();
        assert!(matches!(err, RenderContractMapError::ContractMapWriterFailed(_)));
    }

    #[test]
    fn test_execute_empty_catalogue_returns_empty_catalogue_error() {
        let mut loader = MockLoader::new();
        loader.expect_load_all().returning(|_: &TrackId| Ok((Vec::new(), BTreeMap::new())));
        // Renderer and Writer must NOT run on the empty-catalogue path.
        let mut renderer = MockRenderer::new();
        renderer.expect_render().times(0);
        let mut writer = MockWriter::new();
        writer.expect_write().times(0);

        let interactor = RenderContractMapInteractor::new(loader, renderer, writer);
        let cmd = RenderContractMapCommand { track_id: "t004".to_owned(), layer_filter: None };
        let err = interactor.execute(&cmd).unwrap_err();
        match err {
            RenderContractMapError::EmptyCatalogue { track_id } => {
                assert_eq!(track_id, "t004");
            }
            other => panic!("expected EmptyCatalogue, got {other:?}"),
        }
    }

    #[test]
    fn test_execute_layer_filter_with_unknown_layer_returns_layer_not_found() {
        let (order, catalogues) = three_layer_catalogues();
        let mut loader = MockLoader::new();
        loader
            .expect_load_all()
            .returning(move |_: &TrackId| Ok((order.clone(), catalogues.clone())));
        // Renderer and Writer must NOT run when layer validation fails.
        let mut renderer = MockRenderer::new();
        renderer.expect_render().times(0);
        let mut writer = MockWriter::new();
        writer.expect_write().times(0);

        let interactor = RenderContractMapInteractor::new(loader, renderer, writer);
        let cmd = RenderContractMapCommand {
            track_id: "t005".to_owned(),
            layer_filter: Some(vec!["typo-layer".to_owned()]),
        };
        let err = interactor.execute(&cmd).unwrap_err();
        match err {
            RenderContractMapError::LayerNotFound { track_id, layer_id } => {
                assert_eq!(track_id, "t005");
                assert_eq!(layer_id, "typo-layer");
            }
            other => panic!("expected LayerNotFound, got {other:?}"),
        }
    }
}
