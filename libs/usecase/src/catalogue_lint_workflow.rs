//! Catalogue lint workflow (ADR 2026-04-28-0135 §S3 / IN-05 / AC-05).
//!
//! Hexagonal composition:
//!
//! * [`RunCatalogueLint`] — **primary port** (application service trait).
//!   CLI and future adapters depend on this trait, not on the concrete
//!   interactor below.
//! * [`RunCatalogueLintInteractor`] — orchestrates the secondary ports
//!   (`CatalogueLoader`, `CatalogueLinter`) and returns the lint violations.
//!   It implements [`RunCatalogueLint`].
//!
//! The usecase layer stays pure-orchestrator per
//! `knowledge/conventions/hexagonal-architecture.md` §Usecase Purity:
//! no `std::fs`, no `println!`, no `chrono`, no env access.
//! All I/O flows through the domain ports.

use std::sync::Arc;

use domain::TrackId;
use domain::tddd::catalogue_linter::{
    CatalogueLintViolation, CatalogueLinter, CatalogueLinterError, CatalogueLinterRule,
};
use domain::tddd::catalogue_ports::{CatalogueLoader, CatalogueLoaderError};
use thiserror::Error;

/// Command input for [`RunCatalogueLint::execute`].
///
/// Carries the track, layer, and rule set to lint against.
#[derive(Debug, Clone)]
pub struct RunCatalogueLintCommand {
    /// Track identifier (used to locate the layer catalogue file).
    pub track_id: String,
    /// Layer identifier (e.g. `"domain"`, `"usecase"`, `"infrastructure"`).
    /// Must be one of the TDDD-enabled layers known to the [`CatalogueLoader`].
    pub layer_id: String,
    /// Set of lint rules to evaluate against the catalogue.
    pub rules: Vec<CatalogueLinterRule>,
}

/// Error variants returned by [`RunCatalogueLintInteractor::execute`].
#[derive(Debug, Error)]
pub enum RunCatalogueLintError {
    /// The catalogue loader failed to load the layer catalogue.
    #[error(transparent)]
    CatalogueLoad(#[from] CatalogueLoaderError),

    /// The linter returned an execution error.
    #[error(transparent)]
    LintExecution(#[from] CatalogueLinterError),

    /// The specified `layer_id` is not a TDDD-enabled layer (not present in
    /// the set returned by [`CatalogueLoader::load_all`]).
    #[error("layer '{0}' is not a TDDD-enabled layer")]
    InvalidLayer(String),
}

/// Primary port for the catalogue lint use case.
///
/// The CLI `sotp track lint` subcommand (T007) drives the workflow through
/// this trait, keeping the composition root substitutable.
pub trait RunCatalogueLint: Send + Sync {
    /// Run catalogue lint rules for `cmd.layer_id` within `cmd.track_id`.
    ///
    /// # Errors
    ///
    /// Returns [`RunCatalogueLintError::InvalidLayer`] when `cmd.layer_id`
    /// is not present in the TDDD-enabled layer set returned by the loader.
    /// Returns [`RunCatalogueLintError::CatalogueLoad`] on loader failure.
    /// Returns [`RunCatalogueLintError::LintExecution`] on linter failure.
    fn execute(
        &self,
        cmd: RunCatalogueLintCommand,
    ) -> Result<Vec<CatalogueLintViolation>, RunCatalogueLintError>;
}

/// Default [`RunCatalogueLint`] implementation that composes
/// [`CatalogueLoader`] and [`CatalogueLinter`] secondary ports.
pub struct RunCatalogueLintInteractor {
    catalogue_loader: Arc<dyn CatalogueLoader>,
    linter: Arc<dyn CatalogueLinter>,
}

impl RunCatalogueLintInteractor {
    /// Creates a new interactor wrapping the supplied secondary ports.
    #[must_use]
    pub fn new(
        catalogue_loader: Arc<dyn CatalogueLoader>,
        linter: Arc<dyn CatalogueLinter>,
    ) -> Self {
        Self { catalogue_loader, linter }
    }
}

impl RunCatalogueLint for RunCatalogueLintInteractor {
    fn execute(
        &self,
        cmd: RunCatalogueLintCommand,
    ) -> Result<Vec<CatalogueLintViolation>, RunCatalogueLintError> {
        // Step 1: parse track_id into domain type.
        // We convert here rather than in the command so the command struct
        // stays a plain data carrier (no domain import leak into the command
        // layer boundary).
        let track_id = TrackId::try_new(&cmd.track_id).map_err(|_| {
            // Invalid track IDs surface as a load error because the loader
            // would reject them anyway — we surface early with a descriptive
            // message via CatalogueLoaderError.
            CatalogueLoaderError::LayerDiscoveryFailed {
                reason: format!("invalid track_id '{}'", cmd.track_id),
            }
        })?;

        // Step 2: load all TDDD-enabled layers for this track.
        let (layer_order, catalogues) = self.catalogue_loader.load_all(&track_id)?;

        // Step 3: validate that the requested layer_id is TDDD-enabled.
        let target_layer = layer_order
            .iter()
            .find(|l| l.as_ref() == cmd.layer_id.as_str())
            .ok_or_else(|| RunCatalogueLintError::InvalidLayer(cmd.layer_id.clone()))?;

        // Step 4: retrieve the catalogue for the target layer.
        // The loader contract guarantees every layer in layer_order has a
        // corresponding entry in catalogues, so this is safe (no IndexSlicing).
        let catalogue = catalogues.get(target_layer).ok_or_else(|| {
            // Defensive: should not happen if the loader respects its contract.
            CatalogueLoaderError::LayerDiscoveryFailed {
                reason: format!(
                    "loader returned layer '{}' in order but no catalogue entry",
                    cmd.layer_id
                ),
            }
        })?;

        // Step 5: run the linter against the catalogue.
        let violations = self.linter.run(&cmd.rules, catalogue, &cmd.layer_id)?;

        Ok(violations)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::collections::BTreeMap;

    use domain::TrackId;
    use domain::tddd::catalogue::{TypeAction, TypeCatalogueDocument, TypeCatalogueEntry};
    use domain::tddd::catalogue_linter::{
        CatalogueLintViolation, CatalogueLinterError, CatalogueLinterRule, CatalogueLinterRuleKind,
    };
    use domain::tddd::catalogue_ports::{CatalogueLoader, CatalogueLoaderError};
    use domain::tddd::layer_id::LayerId;
    use mockall::{mock, predicate};

    use super::*;

    // ------------------------------------------------------------------
    // mockall mocks for both secondary ports
    // ------------------------------------------------------------------

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
        pub Linter {}
        impl CatalogueLinter for Linter {
            fn run(
                &self,
                rules: &[CatalogueLinterRule],
                catalogue: &TypeCatalogueDocument,
                layer_id: &str,
            ) -> Result<Vec<CatalogueLintViolation>, CatalogueLinterError>;
        }
    }

    // ------------------------------------------------------------------
    // helpers
    // ------------------------------------------------------------------

    fn layer(name: &str) -> LayerId {
        LayerId::try_new(name.to_owned()).unwrap()
    }

    fn empty_doc() -> TypeCatalogueDocument {
        TypeCatalogueDocument::new(1, Vec::new())
    }

    fn single_entry_doc(name: &str) -> TypeCatalogueDocument {
        use domain::tddd::catalogue::TypeDefinitionKind;
        let entry = TypeCatalogueEntry::new(
            name,
            format!("{name} description"),
            TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
            TypeAction::Add,
            true,
        )
        .unwrap();
        TypeCatalogueDocument::new(1, vec![entry])
    }

    fn three_layer_result(
        target: &str,
    ) -> (Vec<LayerId>, BTreeMap<LayerId, TypeCatalogueDocument>) {
        let domain = layer("domain");
        let usecase = layer("usecase");
        let infra = layer("infrastructure");
        let order = vec![domain.clone(), usecase.clone(), infra.clone()];
        let mut catalogues = BTreeMap::new();
        catalogues.insert(domain.clone(), empty_doc());
        catalogues.insert(usecase.clone(), empty_doc());
        catalogues.insert(infra.clone(), empty_doc());
        // Replace the target layer with a single-entry doc so tests can
        // distinguish whether the correct catalogue was passed.
        let target_layer = layer(target);
        catalogues.insert(target_layer, single_entry_doc("SomeType"));
        (order, catalogues)
    }

    fn field_non_empty_rule() -> CatalogueLinterRule {
        CatalogueLinterRule::try_new(
            CatalogueLinterRuleKind::FieldNonEmpty,
            "secondary_port",
            Some("expected_methods".to_owned()),
            vec![],
        )
        .unwrap()
    }

    fn violation(name: &str) -> CatalogueLintViolation {
        CatalogueLintViolation::new(
            CatalogueLinterRuleKind::FieldNonEmpty,
            name,
            "expected_methods must be non-empty for secondary_port",
        )
    }

    fn cmd(track: &str, layer: &str) -> RunCatalogueLintCommand {
        RunCatalogueLintCommand {
            track_id: track.to_owned(),
            layer_id: layer.to_owned(),
            rules: vec![field_non_empty_rule()],
        }
    }

    // ------------------------------------------------------------------
    // T001: Happy path — linter returns empty violations list
    // ------------------------------------------------------------------

    #[test]
    fn test_execute_happy_path_empty_violations() {
        let (order, catalogues) = three_layer_result("domain");
        let mut loader = MockLoader::new();
        loader
            .expect_load_all()
            .with(predicate::function(|t: &TrackId| t.as_ref() == "my-track"))
            .times(1)
            .returning(move |_| Ok((order.clone(), catalogues.clone())));

        let mut linter = MockLinter::new();
        linter.expect_run().times(1).returning(|_, _, _| Ok(vec![]));

        let interactor = RunCatalogueLintInteractor::new(Arc::new(loader), Arc::new(linter));
        let result = interactor.execute(cmd("my-track", "domain"));

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), vec![]);
    }

    // ------------------------------------------------------------------
    // T002: Happy path — linter returns non-empty violations list
    // ------------------------------------------------------------------

    #[test]
    fn test_execute_happy_path_violations_present() {
        let (order, catalogues) = three_layer_result("domain");
        let mut loader = MockLoader::new();
        loader
            .expect_load_all()
            .times(1)
            .returning(move |_| Ok((order.clone(), catalogues.clone())));

        let v1 = violation("TypeA");
        let v2 = violation("TypeB");
        let expected = [v1.clone(), v2.clone()];
        let mut linter = MockLinter::new();
        linter.expect_run().times(1).returning(move |_, _, _| Ok(vec![v1.clone(), v2.clone()]));

        let interactor = RunCatalogueLintInteractor::new(Arc::new(loader), Arc::new(linter));
        let result = interactor.execute(cmd("my-track", "domain"));

        assert!(result.is_ok());
        let violations = result.unwrap();
        assert_eq!(violations.len(), 2);
        assert_eq!(violations[0].entry_name(), expected[0].entry_name());
        assert_eq!(violations[1].entry_name(), expected[1].entry_name());
    }

    // ------------------------------------------------------------------
    // T003: CatalogueLoader error propagation
    // ------------------------------------------------------------------

    #[test]
    fn test_execute_catalogue_loader_error_propagates() {
        let mut loader = MockLoader::new();
        loader.expect_load_all().times(1).returning(|_| {
            Err(CatalogueLoaderError::LayerDiscoveryFailed { reason: "boom".to_owned() })
        });

        // Linter must NOT be called when loader fails.
        let mut linter = MockLinter::new();
        linter.expect_run().times(0);

        let interactor = RunCatalogueLintInteractor::new(Arc::new(loader), Arc::new(linter));
        let result = interactor.execute(cmd("my-track", "domain"));

        assert!(result.is_err());
        assert!(
            matches!(result, Err(RunCatalogueLintError::CatalogueLoad(_))),
            "expected CatalogueLoad error"
        );
    }

    // ------------------------------------------------------------------
    // T004: CatalogueLinter error propagation
    // ------------------------------------------------------------------

    #[test]
    fn test_execute_linter_error_propagates() {
        let (order, catalogues) = three_layer_result("domain");
        let mut loader = MockLoader::new();
        loader
            .expect_load_all()
            .times(1)
            .returning(move |_| Ok((order.clone(), catalogues.clone())));

        let mut linter = MockLinter::new();
        linter.expect_run().times(1).returning(|_, _, _| {
            Err(CatalogueLinterError::InvalidRuleConfig("contradictory rules".to_owned()))
        });

        let interactor = RunCatalogueLintInteractor::new(Arc::new(loader), Arc::new(linter));
        let result = interactor.execute(cmd("my-track", "domain"));

        assert!(result.is_err());
        assert!(
            matches!(result, Err(RunCatalogueLintError::LintExecution(_))),
            "expected LintExecution error"
        );
    }

    // ------------------------------------------------------------------
    // T005: InvalidLayer — layer_id not in TDDD-enabled layers
    // ------------------------------------------------------------------

    #[test]
    fn test_execute_invalid_layer_returns_error() {
        let (order, catalogues) = three_layer_result("domain");
        let mut loader = MockLoader::new();
        loader
            .expect_load_all()
            .times(1)
            .returning(move |_| Ok((order.clone(), catalogues.clone())));

        // Linter must NOT be called for an unknown layer.
        let mut linter = MockLinter::new();
        linter.expect_run().times(0);

        let interactor = RunCatalogueLintInteractor::new(Arc::new(loader), Arc::new(linter));
        let result = interactor.execute(cmd("my-track", "presentation")); // not in set

        assert!(result.is_err());
        match result {
            Err(RunCatalogueLintError::InvalidLayer(layer_id)) => {
                assert_eq!(layer_id, "presentation");
            }
            other => panic!("expected InvalidLayer, got {other:?}"),
        }
    }
}
