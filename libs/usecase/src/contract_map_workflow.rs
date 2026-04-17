//! Contract Map render workflow (ADR 2026-04-17-1528 §D1).
//!
//! Hexagonal composition:
//!
//! * [`RenderContractMap`] — **primary port** (application_service trait).
//!   CLI and future adapters depend on this trait, not on the concrete
//!   interactor below, so composition roots stay substitutable.
//! * [`RenderContractMapInteractor`] — the interactor that orchestrates
//!   the secondary ports (`CatalogueLoader`, `ContractMapWriter`) and the
//!   pure domain render function (`render_contract_map`). It implements
//!   [`RenderContractMap`].
//!
//! The usecase layer stays pure-orchestrator per
//! `knowledge/conventions/hexagonal-architecture.md` §Usecase Purity:
//! no `std::fs`, no `println!`, no `chrono::Utc::now`, no env access.
//! All I/O flows through the domain ports.

use domain::TrackId;
use domain::tddd::catalogue::TypeDefinitionKind;
use domain::tddd::catalogue_ports::{
    CatalogueLoader, CatalogueLoaderError, ContractMapWriter, ContractMapWriterError,
};
use domain::tddd::{ContractMapRenderOptions, LayerId, render_contract_map};

/// Command input for [`RenderContractMap::execute`].
///
/// Fields mirror the CLI arguments (`sotp track contract-map <track-id>
/// [--kind-filter k1,k2] [--layers l1,l2]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderContractMapCommand {
    pub track_id: TrackId,
    /// If `Some`, only entries whose `kind_tag` matches one of the listed
    /// kinds are rendered. `Some(vec![])` filters every entry out and
    /// produces empty subgraphs (not an error).
    pub kind_filter: Option<Vec<TypeDefinitionKind>>,
    /// If `Some`, restricts rendering to the listed layers. The
    /// interactor fails with [`RenderContractMapError::LayerNotFound`]
    /// when any supplied `LayerId` is absent from the loader's output
    /// set, guarding against silent typos in CLI input.
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
/// Variant inventory matches the `usecase-types.json` declaration for the
/// `tddd-contract-map-phase1-2026-04-17` track.
#[derive(Debug, thiserror::Error)]
pub enum RenderContractMapError {
    /// Failure inside a [`CatalogueLoader`] implementation.
    #[error(transparent)]
    CatalogueLoaderFailed(#[from] CatalogueLoaderError),

    /// Failure inside a [`ContractMapWriter`] implementation.
    #[error(transparent)]
    ContractMapWriterFailed(#[from] ContractMapWriterError),

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
    fn execute(
        &self,
        cmd: &RenderContractMapCommand,
    ) -> Result<RenderContractMapOutput, RenderContractMapError>;
}

/// Default [`RenderContractMap`] implementation that composes a
/// [`CatalogueLoader`], the pure domain renderer, and a
/// [`ContractMapWriter`].
pub struct RenderContractMapInteractor<L, W>
where
    L: CatalogueLoader,
    W: ContractMapWriter,
{
    loader: L,
    writer: W,
}

impl<L, W> RenderContractMapInteractor<L, W>
where
    L: CatalogueLoader,
    W: ContractMapWriter,
{
    /// Creates a new interactor wrapping the supplied secondary ports.
    #[must_use]
    pub fn new(loader: L, writer: W) -> Self {
        Self { loader, writer }
    }
}

impl<L, W> RenderContractMap for RenderContractMapInteractor<L, W>
where
    L: CatalogueLoader,
    W: ContractMapWriter,
{
    fn execute(
        &self,
        cmd: &RenderContractMapCommand,
    ) -> Result<RenderContractMapOutput, RenderContractMapError> {
        let (layer_order, catalogues) = self.loader.load_all(&cmd.track_id)?;

        if layer_order.is_empty() {
            return Err(RenderContractMapError::EmptyCatalogue {
                track_id: cmd.track_id.as_ref().to_owned(),
            });
        }

        if let Some(filter) = &cmd.layer_filter {
            for requested in filter {
                if !layer_order.contains(requested) {
                    return Err(RenderContractMapError::LayerNotFound {
                        track_id: cmd.track_id.as_ref().to_owned(),
                        layer_id: requested.as_ref().to_owned(),
                    });
                }
            }
        }

        let opts = ContractMapRenderOptions {
            layers: cmd.layer_filter.clone().unwrap_or_default(),
            kind_filter: cmd.kind_filter.clone(),
            signal_overlay: false,
            action_overlay: false,
            include_spec_source_edges: false,
        };

        let content = render_contract_map(&catalogues, &layer_order, &opts);

        self.writer.write(&cmd.track_id, &content)?;

        // `rendered_layer_count` reflects only the layers that were actually rendered
        // (respecting `layer_filter`), while `total_entry_count` reflects the full
        // loader catalogue volume regardless of any filter — it is a coarse "how many
        // types does the track catalogue contain?" metric, not a post-filter count.
        let active: Vec<&LayerId> = match cmd.layer_filter.as_deref() {
            Some(f) if !f.is_empty() => layer_order.iter().filter(|l| f.contains(l)).collect(),
            _ => layer_order.iter().collect(),
        };
        let total_entry_count: usize = catalogues.values().map(|d| d.entries().len()).sum();

        Ok(RenderContractMapOutput { rendered_layer_count: active.len(), total_entry_count })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use std::collections::BTreeMap;

    use domain::tddd::ContractMapContent;
    use domain::tddd::catalogue::{TypeAction, TypeCatalogueDocument, TypeCatalogueEntry};
    use mockall::{mock, predicate};

    use super::*;

    mock! {
        pub Loader {}
        impl CatalogueLoader for Loader {
            fn load_all(
                &self,
                track_id: &TrackId,
            ) -> Result<
                (Vec<LayerId>, BTreeMap<LayerId, TypeCatalogueDocument>),
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

    fn track_id(slug: &str) -> TrackId {
        TrackId::try_new(slug.to_owned()).unwrap()
    }

    fn layer(name: &str) -> LayerId {
        LayerId::try_new(name.to_owned()).unwrap()
    }

    fn entry(name: &str, kind: TypeDefinitionKind) -> TypeCatalogueEntry {
        TypeCatalogueEntry::new(name, format!("{name} desc"), kind, TypeAction::Add, true).unwrap()
    }

    fn doc(entries: Vec<TypeCatalogueEntry>) -> TypeCatalogueDocument {
        TypeCatalogueDocument::new(2, entries)
    }

    fn three_layer_catalogues() -> (Vec<LayerId>, BTreeMap<LayerId, TypeCatalogueDocument>) {
        let domain = layer("domain");
        let usecase = layer("usecase");
        let infra = layer("infrastructure");
        let order = vec![domain.clone(), usecase.clone(), infra.clone()];
        let mut catalogues = BTreeMap::new();
        catalogues.insert(domain, doc(vec![entry("User", TypeDefinitionKind::ValueObject)]));
        catalogues.insert(
            usecase,
            doc(vec![
                entry("RegisterUser", TypeDefinitionKind::UseCase),
                entry("RegisterUserCommand", TypeDefinitionKind::Command),
            ]),
        );
        catalogues.insert(infra, doc(vec![]));
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

        let mut writer = MockWriter::new();
        writer.expect_write().times(1).returning(|_: &TrackId, content: &ContractMapContent| {
            assert!(content.as_ref().contains("flowchart LR"));
            Ok(())
        });

        let interactor = RenderContractMapInteractor::new(loader, writer);
        let cmd = RenderContractMapCommand {
            track_id: track_id("t001"),
            kind_filter: None,
            layer_filter: None,
        };
        let out = interactor.execute(&cmd).unwrap();
        assert_eq!(out.rendered_layer_count, 3);
        assert_eq!(out.total_entry_count, 3);
    }

    #[test]
    fn test_execute_propagates_loader_error() {
        let mut loader = MockLoader::new();
        loader.expect_load_all().returning(|_: &TrackId| {
            Err(CatalogueLoaderError::LayerDiscoveryFailed { reason: "boom".to_owned() })
        });
        // Writer must NOT run when the loader fails — enforce via mockall.
        let mut writer = MockWriter::new();
        writer.expect_write().times(0);

        let interactor = RenderContractMapInteractor::new(loader, writer);
        let cmd = RenderContractMapCommand {
            track_id: track_id("t002"),
            kind_filter: None,
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

        let mut writer = MockWriter::new();
        writer.expect_write().returning(|_: &TrackId, _: &ContractMapContent| {
            Err(ContractMapWriterError::IoError {
                path: std::path::PathBuf::from("/tmp/fail"),
                reason: "disk full".to_owned(),
            })
        });

        let interactor = RenderContractMapInteractor::new(loader, writer);
        let cmd = RenderContractMapCommand {
            track_id: track_id("t003"),
            kind_filter: None,
            layer_filter: None,
        };
        let err = interactor.execute(&cmd).unwrap_err();
        assert!(matches!(err, RenderContractMapError::ContractMapWriterFailed(_)));
    }

    #[test]
    fn test_execute_empty_catalogue_returns_empty_catalogue_error() {
        let mut loader = MockLoader::new();
        loader.expect_load_all().returning(|_: &TrackId| Ok((Vec::new(), BTreeMap::new())));
        // Writer must NOT run on the empty-catalogue path.
        let mut writer = MockWriter::new();
        writer.expect_write().times(0);

        let interactor = RenderContractMapInteractor::new(loader, writer);
        let cmd = RenderContractMapCommand {
            track_id: track_id("t004"),
            kind_filter: None,
            layer_filter: None,
        };
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
        let mut writer = MockWriter::new();
        writer.expect_write().times(0);

        let interactor = RenderContractMapInteractor::new(loader, writer);
        let cmd = RenderContractMapCommand {
            track_id: track_id("t005"),
            kind_filter: None,
            layer_filter: Some(vec![layer("typo-layer")]),
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

    #[test]
    fn test_execute_kind_filter_filters_entries_but_still_writes() {
        let (order, catalogues) = three_layer_catalogues();
        let mut loader = MockLoader::new();
        loader
            .expect_load_all()
            .returning(move |_: &TrackId| Ok((order.clone(), catalogues.clone())));

        let mut writer = MockWriter::new();
        writer.expect_write().times(1).returning(|_: &TrackId, content: &ContractMapContent| {
            let text = content.as_ref();
            assert!(text.contains("flowchart LR"));
            assert!(text.contains("usecase_RegisterUser[/RegisterUser/]"));
            assert!(!text.contains("domain_User(User)"));
            Ok(())
        });

        let interactor = RenderContractMapInteractor::new(loader, writer);
        let cmd = RenderContractMapCommand {
            track_id: track_id("t006"),
            kind_filter: Some(vec![TypeDefinitionKind::UseCase]),
            layer_filter: None,
        };
        let out = interactor.execute(&cmd).unwrap();
        // Metrics reflect the loader output, not the post-filter entry count.
        assert_eq!(out.rendered_layer_count, 3);
        assert_eq!(out.total_entry_count, 3);
    }

    #[test]
    fn test_execute_kind_filter_empty_vec_still_writes_empty_subgraphs() {
        let (order, catalogues) = three_layer_catalogues();
        let mut loader = MockLoader::new();
        loader
            .expect_load_all()
            .returning(move |_: &TrackId| Ok((order.clone(), catalogues.clone())));

        let mut writer = MockWriter::new();
        writer.expect_write().times(1).returning(|_: &TrackId, content: &ContractMapContent| {
            let text = content.as_ref();
            assert!(text.contains("flowchart LR"));
            assert!(text.contains("subgraph domain [domain]"));
            assert!(!text.contains("domain_User"));
            assert!(!text.contains("usecase_RegisterUser"));
            Ok(())
        });

        let interactor = RenderContractMapInteractor::new(loader, writer);
        let cmd = RenderContractMapCommand {
            track_id: track_id("t007"),
            kind_filter: Some(Vec::new()),
            layer_filter: None,
        };
        interactor.execute(&cmd).unwrap();
    }
}
