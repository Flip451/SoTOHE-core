//! Contract Map render workflow (ADR 2026-04-17-1528 §D1 + ADR 2026-05-20-2221).
//!
//! Hexagonal composition:
//!
//! * [`RenderContractMap`] — **primary port** (application_service trait).
//!   CLI and future adapters depend on this trait, not on the concrete
//!   interactor below, so composition roots stay substitutable.
//! * [`RenderContractMapInteractor`] — the interactor that orchestrates
//!   the secondary ports (`CatalogueLoader`, `ContractMapRenderer`,
//!   `ContractMapWriter`) (Decision P-5 / IN-23). It implements
//!   [`RenderContractMap`].
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
/// `track_id` is a validated [`TrackId`] (CN-12: concept-bearing identity
/// field typed as domain value object). Callers (CLI) construct `TrackId`
/// and `LayerId` at the boundary and pass typed values. This eliminates
/// the need for `RenderContractMapError::InvalidTrackId`.
///
/// Fields mirror the CLI arguments (`sotp track contract-map <track-id>
/// [--layers l1,l2]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderContractMapCommand {
    /// Validated track identifier (CN-12).
    pub track_id: TrackId,
    /// If `Some`, restricts rendering to the listed layer identifiers.
    /// The interactor fails with [`RenderContractMapError::LayerNotFound`]
    /// when any supplied `LayerId` is absent from the loader's output set,
    /// guarding against silent typos in CLI input.
    pub layer_filter: Option<Vec<LayerId>>,
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
/// `InvalidTrackId` variant is absent: the `Command` now receives a validated
/// `TrackId` (CN-12), eliminating the validation step from the interactor.
///
/// (Decision P-5 / IN-23 / CN-12)
#[derive(Debug)]
pub enum RenderContractMapError {
    /// Failure inside a [`CatalogueLoader`] implementation.
    CatalogueLoaderFailed(CatalogueLoaderError),

    /// Failure inside a [`ContractMapWriter`] implementation.
    ContractMapWriterFailed(ContractMapWriterError),

    /// The loader returned an empty layer set — no `tddd.enabled` layers
    /// exist for this track. Rendering an empty Contract Map is not a
    /// useful workflow, so we fail closed.
    ///
    /// `track_id: TrackId` per CN-12.
    EmptyCatalogue { track_id: TrackId },

    /// The caller's `layer_filter` references a layer that the loader did
    /// not produce — typically a CLI typo or a disabled layer.
    ///
    /// `track_id: TrackId` and `layer_id: LayerId` per CN-12.
    LayerNotFound { track_id: TrackId, layer_id: LayerId },

    /// The [`ContractMapRenderer`] implementation returned an error.
    /// Wraps [`ContractMapRendererError`] (Decision P-5 / IN-23).
    RendererFailed(ContractMapRendererError),
}

impl std::fmt::Display for RenderContractMapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CatalogueLoaderFailed(e) => write!(f, "{e}"),
            Self::ContractMapWriterFailed(e) => write!(f, "{e}"),
            Self::EmptyCatalogue { track_id } => write!(
                f,
                "catalogue loader returned no enabled layers for track '{track_id}'; \
                 check `architecture-rules.json` tddd blocks"
            ),
            Self::LayerNotFound { track_id, layer_id } => {
                write!(f, "layer '{layer_id}' is not a tddd.enabled layer for track '{track_id}'")
            }
            Self::RendererFailed(e) => write!(f, "renderer failed: {e}"),
        }
    }
}

impl std::error::Error for RenderContractMapError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CatalogueLoaderFailed(e) => Some(e),
            Self::ContractMapWriterFailed(e) => Some(e),
            Self::RendererFailed(e) => Some(e),
            Self::EmptyCatalogue { .. } | Self::LayerNotFound { .. } => None,
        }
    }
}

impl From<CatalogueLoaderError> for RenderContractMapError {
    fn from(e: CatalogueLoaderError) -> Self {
        Self::CatalogueLoaderFailed(e)
    }
}

impl From<ContractMapWriterError> for RenderContractMapError {
    fn from(e: ContractMapWriterError) -> Self {
        Self::ContractMapWriterFailed(e)
    }
}

/// T002: `impl From<ContractMapRendererError> for RenderContractMapError`
/// (Decision P-5 / IN-23).
impl From<ContractMapRendererError> for RenderContractMapError {
    fn from(e: ContractMapRendererError) -> Self {
        Self::RendererFailed(e)
    }
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
    /// Returns [`RenderContractMapError`] if the loader fails, the renderer
    /// fails, the writer fails, the enabled-layer set is empty, or a
    /// `layer_filter` entry does not appear in the loader output.
    fn execute(
        &self,
        cmd: &RenderContractMapCommand,
    ) -> Result<RenderContractMapOutput, RenderContractMapError>;
}

/// Default [`RenderContractMap`] implementation that composes a
/// [`CatalogueLoader`], a [`ContractMapRenderer`], and a
/// [`ContractMapWriter`] (Decision P-5 / IN-23).
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
        // track_id is already validated (CN-12): no TrackId::try_new needed.
        let (layer_order, catalogues) = self.loader.load_all(&cmd.track_id)?;

        if layer_order.is_empty() {
            return Err(RenderContractMapError::EmptyCatalogue { track_id: cmd.track_id.clone() });
        }

        // Resolve layer_filter LayerId values against the loaded layer set.
        // An absent LayerId produces LayerNotFound — the layer is not a
        // TDDD-enabled layer for this track.
        let layer_filter: Option<Vec<LayerId>> = cmd
            .layer_filter
            .as_ref()
            .map(|ids| {
                ids.iter()
                    .map(|id| {
                        layer_order.iter().find(|l| *l == id).cloned().ok_or_else(|| {
                            RenderContractMapError::LayerNotFound {
                                track_id: cmd.track_id.clone(),
                                layer_id: id.clone(),
                            }
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;

        // Apply layer_filter to derive the effective layer_order slice.
        // The interactor owns the filtering responsibility (Decision P-5 / T002).
        let filtered_layer_order: Vec<LayerId> = match layer_filter.as_deref() {
            Some(f) if !f.is_empty() => {
                layer_order.iter().filter(|l| f.contains(l)).cloned().collect()
            }
            _ => layer_order.clone(),
        };

        // Build a flat Vec<CatalogueDocument> from the BTreeMap values,
        // preserving the 1-layer = 1-doc contract (Decision A-3' / T002).
        // The loader (`FsCatalogueLoader` / `load_all_catalogues_native`) guarantees
        // that each document's `layer` field matches its BTreeMap key, so flattening
        // to `.values()` here is safe — no entry will render under the wrong layer
        // or be omitted by the renderer's `doc.layer`-based grouping.
        let catalogues_vec: Vec<_> = catalogues.values().cloned().collect();

        // Forward-compatibility stub: opts.layers is not read by the renderer
        // in this track — layer filtering is already done by the interactor above.
        // The opts struct is passed verbatim for forward-compatibility (Decision P-4).
        let opts = ContractMapRenderOptions {
            layers: layer_filter.clone().unwrap_or_default(),
            signal_overlay: false,
            action_overlay: false,
            include_spec_source_edges: false,
        };

        // Delegate rendering to the injected ContractMapRenderer port (T002).
        let content = self.renderer.render(&catalogues_vec, &filtered_layer_order, &opts)?;

        self.writer.write(&cmd.track_id, &content)?;

        // `rendered_layer_count` reflects only the layers that were actually rendered
        // (respecting `layer_filter`), while `total_entry_count` reflects the full
        // loader catalogue volume regardless of any filter.
        let total_entry_count: usize =
            catalogues.values().map(|d| d.types.len() + d.traits.len() + d.functions.len()).sum();

        Ok(RenderContractMapOutput {
            rendered_layer_count: filtered_layer_order.len(),
            total_entry_count,
        })
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
    use domain::tddd::catalogue_v2::{
        MethodDeclaration, ModulePath, StructKind, StructShape, TypeKindV2,
    };
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
        pub Renderer {}
        impl ContractMapRenderer for Renderer {
            fn render(
                &self,
                catalogues: &[CatalogueDocument],
                layer_order: &[LayerId],
                opts: &ContractMapRenderOptions,
            ) -> Result<ContractMapContent, ContractMapRendererError>;
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

    fn layer(name: &str) -> LayerId {
        LayerId::try_new(name.to_owned()).unwrap()
    }

    // Reuse the shared helper from catalogue_traversal so the empty-v3-doc
    // construction knowledge lives in one place (DRY fix).
    use crate::catalogue_traversal::tests::empty_v3_doc;

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
        }
        catalogues.insert(usecase_layer, usecase_doc);

        // infra: 1 function entry
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
    fn test_execute_happy_path_calls_renderer_writer_and_returns_metrics() {
        let (order, catalogues) = three_layer_catalogues();
        let mut loader = MockLoader::new();
        loader
            .expect_load_all()
            .with(predicate::function(|t: &TrackId| t.as_ref() == "t001"))
            .times(1)
            .returning(move |_: &TrackId| Ok((order.clone(), catalogues.clone())));

        let mut renderer = MockRenderer::new();
        renderer.expect_render().times(1).returning(|_catalogues, _layer_order, _opts| {
            Ok(ContractMapContent::new("flowchart LR\n"))
        });

        let mut writer = MockWriter::new();
        writer.expect_write().times(1).returning(|_: &TrackId, content: &ContractMapContent| {
            assert!(content.as_ref().contains("flowchart LR"));
            Ok(())
        });

        let interactor = RenderContractMapInteractor::new(loader, renderer, writer);
        let cmd = RenderContractMapCommand {
            track_id: TrackId::try_new("t001").unwrap(),
            layer_filter: None,
        };
        let out = interactor.execute(&cmd).unwrap();
        assert_eq!(out.rendered_layer_count, 3);
        assert_eq!(out.total_entry_count, 4); // 1 type (domain) + 2 traits (usecase) + 1 fn (infra)
    }

    #[test]
    fn test_execute_propagates_loader_error() {
        let mut loader = MockLoader::new();
        loader.expect_load_all().returning(|_: &TrackId| {
            Err(CatalogueLoaderError::LayerDiscoveryFailed { reason: "boom".to_owned() })
        });
        let mut renderer = MockRenderer::new();
        renderer.expect_render().times(0);
        let mut writer = MockWriter::new();
        writer.expect_write().times(0);

        let interactor = RenderContractMapInteractor::new(loader, renderer, writer);
        let cmd = RenderContractMapCommand {
            track_id: TrackId::try_new("t002").unwrap(),
            layer_filter: None,
        };
        let err = interactor.execute(&cmd).unwrap_err();
        assert!(matches!(err, RenderContractMapError::CatalogueLoaderFailed(_)));
    }

    #[test]
    fn test_execute_propagates_writer_error() {
        let (order, catalogues) = three_layer_catalogues();
        let mut loader = MockLoader::new();
        loader
            .expect_load_all()
            .returning(move |_: &TrackId| Ok((order.clone(), catalogues.clone())));

        let mut renderer = MockRenderer::new();
        renderer.expect_render().returning(|_, _, _| Ok(ContractMapContent::new("flowchart LR\n")));

        let mut writer = MockWriter::new();
        writer.expect_write().returning(|_: &TrackId, _: &ContractMapContent| {
            Err(ContractMapWriterError::IoError {
                path: std::path::PathBuf::from("/tmp/fail"),
                reason: "disk full".to_owned(),
            })
        });

        let interactor = RenderContractMapInteractor::new(loader, renderer, writer);
        let cmd = RenderContractMapCommand {
            track_id: TrackId::try_new("t003").unwrap(),
            layer_filter: None,
        };
        let err = interactor.execute(&cmd).unwrap_err();
        assert!(matches!(err, RenderContractMapError::ContractMapWriterFailed(_)));
    }

    #[test]
    fn test_execute_empty_catalogue_returns_empty_catalogue_error() {
        let mut loader = MockLoader::new();
        loader.expect_load_all().returning(|_: &TrackId| Ok((Vec::new(), BTreeMap::new())));
        let mut renderer = MockRenderer::new();
        renderer.expect_render().times(0);
        let mut writer = MockWriter::new();
        writer.expect_write().times(0);

        let interactor = RenderContractMapInteractor::new(loader, renderer, writer);
        let cmd = RenderContractMapCommand {
            track_id: TrackId::try_new("t004").unwrap(),
            layer_filter: None,
        };
        let err = interactor.execute(&cmd).unwrap_err();
        match err {
            RenderContractMapError::EmptyCatalogue { track_id } => {
                assert_eq!(track_id.as_ref(), "t004");
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
        let mut renderer = MockRenderer::new();
        renderer.expect_render().times(0);
        let mut writer = MockWriter::new();
        writer.expect_write().times(0);

        let interactor = RenderContractMapInteractor::new(loader, renderer, writer);
        // Note: "typo-layer" is not in the layer set, so LayerNotFound fires.
        // LayerId::try_new("typo-layer") is valid syntactically; it is absent from the loader output.
        let cmd = RenderContractMapCommand {
            track_id: TrackId::try_new("t005").unwrap(),
            layer_filter: Some(vec![LayerId::try_new("typo-layer").unwrap()]),
        };
        let err = interactor.execute(&cmd).unwrap_err();
        match err {
            RenderContractMapError::LayerNotFound { track_id, layer_id } => {
                assert_eq!(track_id.as_ref(), "t005");
                assert_eq!(layer_id.as_ref(), "typo-layer");
            }
            other => panic!("expected LayerNotFound, got {other:?}"),
        }
    }

    /// T002 unit test: renderer returning ContractMapRendererError is converted
    /// to RenderContractMapError::RendererFailed (Decision P-5 / IN-23).
    #[test]
    fn test_execute_renderer_error_converts_to_renderer_failed() {
        let (order, catalogues) = three_layer_catalogues();
        let mut loader = MockLoader::new();
        loader
            .expect_load_all()
            .returning(move |_: &TrackId| Ok((order.clone(), catalogues.clone())));

        let mut renderer = MockRenderer::new();
        renderer.expect_render().returning(|_, _, _| {
            Err(ContractMapRendererError::StyleConfigNotFound {
                path: std::path::PathBuf::from("/missing/style.toml"),
            })
        });

        let mut writer = MockWriter::new();
        writer.expect_write().times(0);

        let interactor = RenderContractMapInteractor::new(loader, renderer, writer);
        let cmd = RenderContractMapCommand {
            track_id: TrackId::try_new("t006").unwrap(),
            layer_filter: None,
        };
        let err = interactor.execute(&cmd).unwrap_err();
        assert!(
            matches!(err, RenderContractMapError::RendererFailed(_)),
            "expected RendererFailed, got {err:?}"
        );
    }
}
