//! `bin/sotp test-obligation evaluate` — LLM-backed fulfillment / waiver verification.
//!
//! [`EvaluateTestObligationsInteractor`] drives the obligation-fulfillment and
//! waiver lanes through their semantic verifiers, freezing each verdict against a
//! three-component cache key (IN-09 / AC-06 / CN-04 — D6): the fulfillment key is
//! `(bound_tests_set_hash, declaration_hash, anchor_text_hash)` and the waiver key
//! is `(waived_reason_hash, declaration_hash, anchor_text_hash)`. A verdict is
//! reused only when its verifier-prompt fingerprint also matches; otherwise the
//! pair is escalated `fast → final` and the fresh verdict is persisted (CN-03
//! edge-local).

// `ObligationEvaluateError` carries unboxed non-empty payloads
// (`NonEmptyEdgeVerdictRecords`) per the catalogue contract, which makes the
// `Err` variant large. Boxing would diverge from the declared type shape, so the
// size is accepted here rather than boxed.
#![allow(clippy::result_large_err)]

use std::future::Future;
use std::num::{NonZeroU8, NonZeroUsize};
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use domain::tddd::test_obligation::binding::{TestBindingRecord, TestBindingsDocument};
use domain::tddd::test_obligation::drift::{EdgeVerdictRecord, NonEmptyEdgeVerdictRecords};
use domain::tddd::test_obligation::errors::{
    ArtifactCodecError, ObligationEvaluateError, SemanticVerifierError,
};
use domain::tddd::test_obligation::hashes::VerifierPromptFingerprint;
use domain::tddd::test_obligation::obligations::ObligationsDocument;
use domain::tddd::test_obligation::pair::{ObligationFulfillmentPair, WaiverPair};
use domain::tddd::test_obligation::ports::{
    ObligationsArtifactPort, TestBindingsArtifactPort, TestSourceScannerPort, WaiverCachePort,
};
use domain::tddd::test_obligation::verdict::{
    DetectionRatePercent, ObligationFulfillmentCacheKey, ObligationFulfillmentVerdict,
    WaiverCacheKey, WaiverVerdict,
};
use domain::{SpecDocumentLoaderPort, TrackId};

use crate::catalogue_document_loader::AttestedCatalogueDocumentLoaderPort;
use crate::semantic_verdict_core::driver::SemanticEscalationDriverPort;

use super::bound_tests::ResolvedBoundTestsResolver;
use super::hasher::ContentHasherPort;
use super::ports::ObligationFulfillmentCachePort;
use super::{LoadedCatalogueDocument, diag, is_active_branch};

mod cache;
mod calibration;
mod calibration_runner;
mod concurrency;
mod edges;
mod plan;
mod records;
mod verify;

use concurrency::drive_bounded_in_order;
use plan::PlannedAction;

/// Command input for [`EvaluateTestObligationsApplicationService`] (IN-09).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluateTestObligationsCommand {
    track_id: TrackId,
    current_branch: String,
    catalogue_paths: Vec<PathBuf>,
    spec_path: PathBuf,
}

impl EvaluateTestObligationsCommand {
    /// Builds an [`EvaluateTestObligationsCommand`].
    #[must_use]
    pub fn new(
        track_id: TrackId,
        current_branch: String,
        catalogue_paths: Vec<PathBuf>,
        spec_path: PathBuf,
    ) -> Self {
        Self { track_id, current_branch, catalogue_paths, spec_path }
    }
}

/// Validation error for [`TestObligationEvaluateConfig::try_new`] (IN-01 / AC-15).
#[derive(Debug, thiserror::Error)]
pub enum TestObligationEvaluateConfigError {
    /// `injection_rate` exceeded the `0..=100` percentage range.
    #[error("injection_rate must be a percentage in 0..=100, got {value}")]
    InvalidInjectionRate {
        /// The out-of-range injection rate.
        value: u8,
    },
    /// `detection_threshold` was `0` or exceeded `100`.
    #[error("detection_threshold must be a percentage in 1..=100, got {value}")]
    InvalidDetectionThreshold {
        /// The out-of-range detection threshold.
        value: u8,
    },
    /// `parallelism` was `0`.
    #[error("parallelism must be at least 1")]
    InvalidParallelism,
}

/// Validated configuration for [`EvaluateTestObligationsInteractor`] (IN-01 / AC-15).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestObligationEvaluateConfig {
    injection_rate: u8,
    detection_threshold: NonZeroU8,
    parallelism: usize,
}

impl TestObligationEvaluateConfig {
    /// Validates and builds a [`TestObligationEvaluateConfig`].
    ///
    /// # Errors
    ///
    /// Returns a [`TestObligationEvaluateConfigError`] when `injection_rate`
    /// exceeds 100, `detection_threshold` is 0 or exceeds 100, or `parallelism`
    /// is 0.
    pub fn try_new(
        injection_rate: u8,
        detection_threshold: u8,
        parallelism: usize,
    ) -> Result<Self, TestObligationEvaluateConfigError> {
        if injection_rate > 100 {
            return Err(TestObligationEvaluateConfigError::InvalidInjectionRate {
                value: injection_rate,
            });
        }
        let detection_threshold = NonZeroU8::new(detection_threshold)
            .filter(|value| value.get() <= 100)
            .ok_or(TestObligationEvaluateConfigError::InvalidDetectionThreshold {
                value: detection_threshold,
            })?;
        if parallelism == 0 {
            return Err(TestObligationEvaluateConfigError::InvalidParallelism);
        }
        Ok(Self { injection_rate, detection_threshold, parallelism })
    }

    /// Returns the configured calibration-probe injection rate.
    #[must_use]
    pub fn injection_rate(&self) -> u8 {
        self.injection_rate
    }

    /// Returns the configured known-bad detection threshold.
    #[must_use]
    pub fn detection_threshold(&self) -> NonZeroU8 {
        self.detection_threshold
    }

    /// Returns the configured evaluation parallelism.
    #[must_use]
    pub fn parallelism(&self) -> usize {
        self.parallelism
    }
}

impl Default for TestObligationEvaluateConfig {
    fn default() -> Self {
        // This is the sole default concurrency bound for every evaluation
        // fan-out (calibration, fulfillment, and waiver).
        const DEFAULT_PARALLELISM: usize = 4;
        const DEFAULT_THRESHOLD: NonZeroU8 = match NonZeroU8::new(90) {
            Some(value) => value,
            None => NonZeroU8::MIN,
        };
        Self {
            injection_rate: 10,
            detection_threshold: DEFAULT_THRESHOLD,
            parallelism: DEFAULT_PARALLELISM,
        }
    }
}

/// Configured-provider calibration result exposed by the evaluation use case
/// (IN-06 / AC-12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfiguredProviderCalibrationOutcome {
    /// Calibration was disabled by the configured injection rate.
    SkippedByConfiguration,
    /// Calibration ran against the configured provider.
    Executed {
        /// Detection rate for the known-bad calibration probes.
        known_bad_detection_rate: DetectionRatePercent,
    },
}

/// A validated non-zero production-verdict category count (AC-12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonZeroProductionVerdictCount {
    value: NonZeroUsize,
}

impl NonZeroProductionVerdictCount {
    /// Builds a count, rejecting zero while preserving every non-zero `usize`.
    #[must_use]
    pub fn try_new(value: usize) -> Option<Self> {
        NonZeroUsize::new(value).map(|value| Self { value })
    }

    /// Returns the exact validated count.
    #[must_use]
    pub fn get(&self) -> usize {
        self.value.get()
    }
}

/// Exact, non-empty production-verdict counts (IN-06 / AC-12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProductionVerdictCounts {
    /// At least one production verdict passed.
    Passing {
        /// Exact number of passing verdicts.
        pass_count: NonZeroProductionVerdictCount,
        /// Exact number of failing verdicts.
        fail_count: usize,
        /// Exact number of pending verdicts.
        pending_count: usize,
    },
    /// No verdict passed, and at least one failed.
    Failing {
        /// Exact number of failing verdicts.
        fail_count: NonZeroProductionVerdictCount,
        /// Exact number of pending verdicts.
        pending_count: usize,
    },
    /// Only pending verdicts were recorded.
    Pending {
        /// Exact number of pending verdicts.
        pending_count: NonZeroProductionVerdictCount,
    },
}

impl ProductionVerdictCounts {
    /// Builds exact counts, returning `None` only when all categories are zero.
    #[must_use]
    pub fn try_new(pass_count: usize, fail_count: usize, pending_count: usize) -> Option<Self> {
        if let Some(pass_count) = NonZeroProductionVerdictCount::try_new(pass_count) {
            return Some(Self::Passing { pass_count, fail_count, pending_count });
        }
        if let Some(fail_count) = NonZeroProductionVerdictCount::try_new(fail_count) {
            return Some(Self::Failing { fail_count, pending_count });
        }
        NonZeroProductionVerdictCount::try_new(pending_count)
            .map(|pending_count| Self::Pending { pending_count })
    }

    /// Returns the exact number of passing verdicts.
    #[must_use]
    pub fn pass_count(&self) -> usize {
        match self {
            Self::Passing { pass_count, .. } => pass_count.get(),
            Self::Failing { .. } | Self::Pending { .. } => 0,
        }
    }

    /// Returns the exact number of failing verdicts.
    #[must_use]
    pub fn fail_count(&self) -> usize {
        match self {
            Self::Passing { fail_count, .. } => *fail_count,
            Self::Failing { fail_count, .. } => fail_count.get(),
            Self::Pending { .. } => 0,
        }
    }

    /// Returns the exact number of pending verdicts.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        match self {
            Self::Passing { pending_count, .. } | Self::Failing { pending_count, .. } => {
                *pending_count
            }
            Self::Pending { pending_count } => pending_count.get(),
        }
    }
}

/// Structured output of [`EvaluateTestObligationsInteractor`] (IN-06 / AC-12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvaluateTestObligationsOutcome {
    /// No production action was planned or recorded.
    NoProductionPairs,
    /// Production verdicts were recorded and calibration has an explicit
    /// configured-provider result.
    ProductionPairs {
        /// Exact production verdict counts.
        verdicts: ProductionVerdictCounts,
        /// Actual configured-provider calibration result.
        configured_provider_calibration: ConfiguredProviderCalibrationOutcome,
    },
}

impl EvaluateTestObligationsOutcome {
    /// Builds an outcome for an empty production scope.
    #[must_use]
    pub fn new_no_production_pairs() -> Self {
        Self::NoProductionPairs
    }

    /// Builds an outcome containing production verdicts and calibration.
    #[must_use]
    pub fn new_production_pairs(
        verdicts: ProductionVerdictCounts,
        configured_provider_calibration: ConfiguredProviderCalibrationOutcome,
    ) -> Self {
        Self::ProductionPairs { verdicts, configured_provider_calibration }
    }

    /// Returns the count of passing verdicts.
    #[must_use]
    pub fn pass_count(&self) -> usize {
        match self {
            Self::NoProductionPairs => 0,
            Self::ProductionPairs { verdicts, .. } => verdicts.pass_count(),
        }
    }

    /// Returns the count of failing verdicts.
    #[must_use]
    pub fn fail_count(&self) -> usize {
        match self {
            Self::NoProductionPairs => 0,
            Self::ProductionPairs { verdicts, .. } => verdicts.fail_count(),
        }
    }

    /// Returns the count of pending verdicts.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        match self {
            Self::NoProductionPairs => 0,
            Self::ProductionPairs { verdicts, .. } => verdicts.pending_count(),
        }
    }

    /// Returns the configured-provider calibration result when production
    /// pairs were evaluated.
    #[must_use]
    pub fn configured_provider_calibration(&self) -> Option<&ConfiguredProviderCalibrationOutcome> {
        match self {
            Self::NoProductionPairs => None,
            Self::ProductionPairs { configured_provider_calibration, .. } => {
                Some(configured_provider_calibration)
            }
        }
    }
}

/// Boxed, `Send` future returned by
/// [`EvaluateTestObligationsApplicationService::execute`] (mirrors the
/// `SemanticEscalationFuture` precedent in `semantic_verdict_core::driver`).
pub type EvaluateTestObligationsFuture<'a> = Pin<
    Box<
        dyn Future<Output = Result<EvaluateTestObligationsOutcome, ObligationEvaluateError>>
            + Send
            + 'a,
    >,
>;

/// Primary port for `bin/sotp test-obligation evaluate` (IN-09 / AC-06 / AC-07).
pub trait EvaluateTestObligationsApplicationService {
    /// Runs the fulfillment / waiver semantic verification and freezes verdicts.
    ///
    /// # Errors
    ///
    /// Returns [`ObligationEvaluateError`] when the branch is not the active
    /// track branch, a verifier port fails, or a verdict cache cannot be
    /// persisted.
    fn execute<'a>(
        &'a self,
        cmd: &'a EvaluateTestObligationsCommand,
    ) -> EvaluateTestObligationsFuture<'a>;
}

/// Running tally of edge verdicts.
#[derive(Default)]
struct Tally {
    pass: usize,
    fail: usize,
    pending: usize,
    failure_records: Vec<EdgeVerdictRecord>,
    pending_records: Vec<EdgeVerdictRecord>,
}

/// Interactor implementing [`EvaluateTestObligationsApplicationService`] (IN-09).
pub struct EvaluateTestObligationsInteractor {
    obligations_port: Arc<dyn ObligationsArtifactPort + Send + Sync>,
    bindings_port: Arc<dyn TestBindingsArtifactPort + Send + Sync>,
    fulfillment_driver: Arc<
        dyn SemanticEscalationDriverPort<
                ObligationFulfillmentPair,
                ObligationFulfillmentCacheKey,
                ObligationFulfillmentVerdict,
                SemanticVerifierError,
            > + Send
            + Sync,
    >,
    waiver_driver: Arc<
        dyn SemanticEscalationDriverPort<
                WaiverPair,
                WaiverCacheKey,
                WaiverVerdict,
                SemanticVerifierError,
            > + Send
            + Sync,
    >,
    fulfillment_cache: Arc<dyn ObligationFulfillmentCachePort + Send + Sync>,
    waiver_cache: Arc<dyn WaiverCachePort + Send + Sync>,
    fulfillment_verifier_fingerprint: VerifierPromptFingerprint,
    waiver_verifier_fingerprint: VerifierPromptFingerprint,
    config: TestObligationEvaluateConfig,
    spec_reader: Arc<dyn SpecDocumentLoaderPort + Send + Sync>,
    catalogue_reader: Arc<dyn AttestedCatalogueDocumentLoaderPort + Send + Sync>,
    hasher: Arc<dyn ContentHasherPort + Send + Sync>,
    resolved_bound_tests_resolver: Arc<ResolvedBoundTestsResolver>,
}

impl EvaluateTestObligationsInteractor {
    /// Builds an [`EvaluateTestObligationsInteractor`] from its injected ports.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        obligations_port: Arc<dyn ObligationsArtifactPort + Send + Sync>,
        bindings_port: Arc<dyn TestBindingsArtifactPort + Send + Sync>,
        source_scanner: Arc<dyn TestSourceScannerPort + Send + Sync>,
        fulfillment_driver: Arc<
            dyn SemanticEscalationDriverPort<
                    ObligationFulfillmentPair,
                    ObligationFulfillmentCacheKey,
                    ObligationFulfillmentVerdict,
                    SemanticVerifierError,
                > + Send
                + Sync,
        >,
        waiver_driver: Arc<
            dyn SemanticEscalationDriverPort<
                    WaiverPair,
                    WaiverCacheKey,
                    WaiverVerdict,
                    SemanticVerifierError,
                > + Send
                + Sync,
        >,
        fulfillment_cache: Arc<dyn ObligationFulfillmentCachePort + Send + Sync>,
        waiver_cache: Arc<dyn WaiverCachePort + Send + Sync>,
        fulfillment_verifier_fingerprint: VerifierPromptFingerprint,
        waiver_verifier_fingerprint: VerifierPromptFingerprint,
        config: TestObligationEvaluateConfig,
        spec_reader: Arc<dyn SpecDocumentLoaderPort + Send + Sync>,
        catalogue_reader: Arc<dyn AttestedCatalogueDocumentLoaderPort + Send + Sync>,
        hasher: Arc<dyn ContentHasherPort + Send + Sync>,
    ) -> Self {
        // Keep every fulfillment-cache hash on the one injected strategy:
        // callers cannot supply a resolver built with a different hasher.
        let resolved_bound_tests_resolver =
            Arc::new(ResolvedBoundTestsResolver::new(source_scanner, Arc::clone(&hasher)));
        Self {
            obligations_port,
            bindings_port,
            fulfillment_driver,
            waiver_driver,
            fulfillment_cache,
            waiver_cache,
            fulfillment_verifier_fingerprint,
            waiver_verifier_fingerprint,
            config,
            spec_reader,
            catalogue_reader,
            hasher,
            resolved_bound_tests_resolver,
        }
    }

    /// Loads every catalogue document named in the command.
    fn load_catalogues(
        &self,
        cmd: &EvaluateTestObligationsCommand,
    ) -> Result<Vec<LoadedCatalogueDocument>, ObligationEvaluateError> {
        let mut catalogues = Vec::with_capacity(cmd.catalogue_paths.len());
        for path in &cmd.catalogue_paths {
            let doc = self
                .catalogue_reader
                .load(path)
                .map_err(ObligationEvaluateError::CatalogueLoad)?
                .into_document();
            catalogues.push(LoadedCatalogueDocument::new(path, doc));
        }
        Ok(catalogues)
    }
}

/// Maps a validated-input construction failure to a verifier-input error.
///
/// `ObligationEvaluateError` has no dedicated invalid-input variant, so these
/// (in practice unreachable) validation failures surface through the verifier
/// lane — the stage that consumes the malformed value.
fn invalid_input_error(field: &str) -> ObligationEvaluateError {
    ObligationEvaluateError::VerifierPort(SemanticVerifierError::VerifierPort(diag(&format!(
        "invalid evaluate input: {field}"
    ))))
}

fn half_materialized_scope_error(missing: &str) -> ObligationEvaluateError {
    ObligationEvaluateError::ArtifactLoad(ArtifactCodecError::DomainInvariant(diag(&format!(
        "test-obligation scope is half-materialized: {missing} artifact is absent"
    ))))
}

impl EvaluateTestObligationsApplicationService for EvaluateTestObligationsInteractor {
    fn execute<'a>(
        &'a self,
        cmd: &'a EvaluateTestObligationsCommand,
    ) -> EvaluateTestObligationsFuture<'a> {
        Box::pin(async move { self.execute_inner(cmd).await })
    }
}

impl EvaluateTestObligationsInteractor {
    async fn execute_inner(
        &self,
        cmd: &EvaluateTestObligationsCommand,
    ) -> Result<EvaluateTestObligationsOutcome, ObligationEvaluateError> {
        if !is_active_branch(&cmd.track_id, &cmd.current_branch) {
            return Err(ObligationEvaluateError::TrackNotActive {
                branch: diag(&cmd.current_branch),
            });
        }

        let obligations = self
            .obligations_port
            .load(&cmd.track_id)
            .map_err(ObligationEvaluateError::ArtifactLoad)?;
        let bindings = self
            .bindings_port
            .load(&cmd.track_id)
            .map_err(ObligationEvaluateError::ArtifactLoad)?;

        let (obligations, bindings) = match (obligations, bindings) {
            (None, None) => {
                // Existence-based scope: no materialized scope to evaluate
                // (IN-14). Rewrite caches to empty documents so `results` cannot
                // report verdicts from a previous materialized scope.
                self.save_caches(&cmd.track_id, Vec::new(), Vec::new())?;
                return Ok(EvaluateTestObligationsOutcome::new_no_production_pairs());
            }
            (Some(obligations), Some(bindings)) => (obligations, bindings),
            (None, Some(_)) => return Err(half_materialized_scope_error("obligations")),
            (Some(_), None) => return Err(half_materialized_scope_error("test-bindings")),
        };

        validate_voluntary_bindings(&obligations, &bindings)
            .map_err(ObligationEvaluateError::BindingConsistency)?;

        let catalogues = self.load_catalogues(cmd)?;
        let spec =
            self.spec_reader.load(&cmd.spec_path).map_err(ObligationEvaluateError::SpecLoad)?;
        let existing_fulfillment_cache = self
            .fulfillment_cache
            .load(&cmd.track_id)
            .map_err(ObligationEvaluateError::CachePersistence)?;
        let existing_waiver_cache = self
            .waiver_cache
            .load(&cmd.track_id)
            .map_err(ObligationEvaluateError::CachePersistence)?;

        let mut tally = Tally::default();
        let mut fulfillment_entries = Vec::new();
        let mut waiver_entries = Vec::new();

        // Plan every binding record synchronously — this classifies each edge
        // as either an immediate outcome (pending / cache hit) or an LLM
        // task carrying every input the verifier subprocess needs. The plan
        // order is the byte layout downstream cache documents keep.
        let plan = self.plan_binding_records(
            &bindings,
            &obligations,
            &catalogues,
            &spec,
            existing_fulfillment_cache.as_ref(),
            existing_waiver_cache.as_ref(),
        )?;

        // Calibration is classified from the concrete production plan rather
        // than from binding-record arithmetic. This keeps an empty plan from
        // being reported as provider execution and preserves the explicit
        // configuration opt-out for real production pairs.
        let configured_provider_calibration = match calibration::CalibrationExecution::for_inputs(
            plan.len(),
            self.config.injection_rate(),
        ) {
            calibration::CalibrationExecution::SkippedNoProductionPairs => None,
            calibration::CalibrationExecution::SkippedByConfiguration => {
                Some(ConfiguredProviderCalibrationOutcome::SkippedByConfiguration)
            }
            calibration::CalibrationExecution::Provider { .. } => {
                let detection_rate = self.known_bad_detection_rate(plan.len()).await?;
                self.local_responsibility_calibration(plan.len()).await?;
                Some(ConfiguredProviderCalibrationOutcome::Executed {
                    known_bad_detection_rate: detection_rate,
                })
            }
        };

        // Build futures for the LLM tasks in plan order, then fan them out
        // through the bounded multiplexer. Verdict order mirrors the input
        // order regardless of completion order, so `apply_planned` can fold
        // them back in place.
        let mut fulfillment_futures = Vec::new();
        let mut waiver_futures = Vec::new();
        for action in &plan {
            match action {
                PlannedAction::Fulfillment(task) => {
                    fulfillment_futures.push(self.fulfillment_llm_future(task));
                }
                PlannedAction::Waiver(task) => {
                    waiver_futures.push(self.waiver_llm_future(task));
                }
                PlannedAction::Immediate(_) => {}
            }
        }
        let fulfillment_verdicts =
            drive_bounded_in_order(fulfillment_futures, self.config.parallelism()).await?;
        let waiver_verdicts =
            drive_bounded_in_order(waiver_futures, self.config.parallelism()).await?;

        self.apply_planned(
            plan,
            fulfillment_verdicts,
            waiver_verdicts,
            &mut tally,
            &mut fulfillment_entries,
            &mut waiver_entries,
        )?;

        self.save_caches(&cmd.track_id, fulfillment_entries, waiver_entries)?;

        // `try_new` yields the error only when the record set is non-empty, so
        // the gate fails closed on confirmed failures / escalations (empty → skip).
        if let Ok(records) = NonEmptyEdgeVerdictRecords::try_new(tally.failure_records) {
            return Err(ObligationEvaluateError::SemanticFailuresConfirmed { records });
        }
        if let Ok(records) = NonEmptyEdgeVerdictRecords::try_new(tally.pending_records) {
            return Err(ObligationEvaluateError::HumanEscalationRequired { records });
        }

        let Some(verdicts) =
            ProductionVerdictCounts::try_new(tally.pass, tally.fail, tally.pending)
        else {
            return Ok(EvaluateTestObligationsOutcome::new_no_production_pairs());
        };
        let Some(configured_provider_calibration) = configured_provider_calibration else {
            return Err(invalid_input_error("configured_provider_calibration"));
        };
        Ok(EvaluateTestObligationsOutcome::new_production_pairs(
            verdicts,
            configured_provider_calibration,
        ))
    }
}

/// Applies the domain-owned voluntary-binding ownership invariant to every record.
fn validate_voluntary_bindings(
    obligations: &ObligationsDocument,
    bindings: &TestBindingsDocument,
) -> Result<(), domain::tddd::test_obligation::errors::TestBindingConsistencyError> {
    for record in bindings.records() {
        if let TestBindingRecord::VoluntaryBinding { edge_id, .. } = record {
            obligations.validate_voluntary_binding(edge_id)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
