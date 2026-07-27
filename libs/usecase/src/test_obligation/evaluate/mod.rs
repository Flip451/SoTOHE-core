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
use std::num::NonZeroU8;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use domain::tddd::catalogue_v2::catalogue_impl_signals_ports::CatalogueDocumentLoaderPort;
use domain::tddd::semantic_verify::ModelTier;
use domain::tddd::test_obligation::binding::{TestBindingRecord, TestBindingsDocument};
use domain::tddd::test_obligation::drift::{EdgeVerdictRecord, NonEmptyEdgeVerdictRecords};
use domain::tddd::test_obligation::errors::{
    ArtifactCodecError, ObligationEvaluateError, SemanticVerifierError,
};
use domain::tddd::test_obligation::hashes::{
    AnchorTextHash, BoundTestsSetHash, DeclarationHash, VerifierPromptFingerprint,
};
use domain::tddd::test_obligation::obligations::ObligationsDocument;
use domain::tddd::test_obligation::pair::{
    AnchorText, EntryDeclaration, ObligationFulfillmentPair, TestsSource, WaiverPair,
};
use domain::tddd::test_obligation::ports::{
    ObligationsArtifactPort, TestBindingsArtifactPort, TestSourceScannerPort, WaiverCachePort,
};
use domain::tddd::test_obligation::verdict::{
    DetectionRatePercent, ObligationFulfillmentCacheKey, ObligationFulfillmentVerdict,
    WaiverCacheKey, WaiverVerdict,
};
use domain::{SpecDocumentLoaderPort, TrackId};

use crate::semantic_verdict_core::driver::SemanticEscalationDriverPort;

use super::bound_tests::ResolvedBoundTestsResolver;
use super::hasher::ContentHasherPort;
use super::ports::ObligationFulfillmentCachePort;
use super::{LoadedCatalogueDocument, diag, is_active_branch};

mod cache;
mod calibration;
mod concurrency;
mod edges;
mod plan;
mod records;
mod verify;

use calibration::{CategoryTally, calibration_probe_count, probe_shape_for};
use concurrency::drive_bounded_in_order;
use plan::PlannedAction;
use verify::map_verifier_error;

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

/// Structured output of [`EvaluateTestObligationsInteractor`] (IN-09 / AC-06).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluateTestObligationsOutcome {
    pass_count: usize,
    fail_count: usize,
    pending_count: usize,
    known_bad_detection_rate: DetectionRatePercent,
}

impl EvaluateTestObligationsOutcome {
    /// Builds an [`EvaluateTestObligationsOutcome`].
    #[must_use]
    pub fn new(
        pass_count: usize,
        fail_count: usize,
        pending_count: usize,
        known_bad_detection_rate: DetectionRatePercent,
    ) -> Self {
        Self { pass_count, fail_count, pending_count, known_bad_detection_rate }
    }

    /// Returns the count of passing verdicts.
    #[must_use]
    pub fn pass_count(&self) -> usize {
        self.pass_count
    }

    /// Returns the count of failing verdicts.
    #[must_use]
    pub fn fail_count(&self) -> usize {
        self.fail_count
    }

    /// Returns the count of pending verdicts.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending_count
    }

    /// Returns the known-bad calibration-probe detection rate.
    #[must_use]
    pub fn known_bad_detection_rate(&self) -> DetectionRatePercent {
        self.known_bad_detection_rate.clone()
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
    catalogue_reader: Arc<dyn CatalogueDocumentLoaderPort + Send + Sync>,
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
        catalogue_reader: Arc<dyn CatalogueDocumentLoaderPort + Send + Sync>,
        hasher: Arc<dyn ContentHasherPort + Send + Sync>,
        resolved_bound_tests_resolver: Arc<ResolvedBoundTestsResolver>,
    ) -> Self {
        // The application service owns the scanner dependency. Rebind the
        // resolver so its evidence reads and scan errors always come from that
        // same injected port.
        let resolved_bound_tests_resolver =
            Arc::new(resolved_bound_tests_resolver.with_source_scanner(source_scanner));
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
            let doc =
                self.catalogue_reader.load(path).map_err(ObligationEvaluateError::CatalogueLoad)?;
            catalogues.push(LoadedCatalogueDocument::new(path, doc));
        }
        Ok(catalogues)
    }

    async fn known_bad_detection_rate(
        &self,
        production_pair_count: usize,
    ) -> Result<DetectionRatePercent, ObligationEvaluateError> {
        let probe_count =
            calibration_probe_count(production_pair_count, self.config.injection_rate());
        if probe_count == 0 {
            return DetectionRatePercent::try_new(100)
                .map_err(|_| invalid_input_error("known_bad_detection_rate"));
        }

        // Plan every calibration probe up front so their verdict futures can
        // be fanned out through the same bounded multiplexer the production
        // pairs use. Categories are tallied here rather than after the fan-out
        // so the per-category gate remains deterministic w.r.t. probe index.
        let mut category_tally = CategoryTally::default();
        let mut probe_categories: Vec<
            domain::tddd::test_obligation::vocab::FulfillmentFailCategory,
        > = Vec::with_capacity(probe_count);
        let mut probe_futures = Vec::with_capacity(probe_count);
        for index in 0..probe_count {
            let shape = probe_shape_for(index);
            category_tally.record_issued(&shape.category);
            probe_categories.push(shape.category.clone());
            probe_futures.push(self.calibration_probe_future(shape));
        }

        // Fan out the probe verdicts under the configured concurrency ceiling;
        // results come back in `probe_index` order.
        let verdicts = drive_bounded_in_order(probe_futures, self.config.parallelism()).await?;

        let mut detected = 0usize;
        for (verdict, expected_category) in verdicts.into_iter().zip(probe_categories.into_iter()) {
            if let ObligationFulfillmentVerdict::Fail { category, .. } = verdict
                && category == expected_category
            {
                detected += 1;
                category_tally.record_detected(&expected_category);
            }
        }

        // Per-category gate (AC-08): any exercised category that ends with
        // zero detected probes fails the calibration through the same
        // `VerifierPort` path as the threshold breach, but with a message
        // that names the missed category.
        let undetected = category_tally.undetected_categories();
        if !undetected.is_empty() {
            return Err(ObligationEvaluateError::VerifierPort(
                SemanticVerifierError::VerifierPort(diag(&format!(
                    "known-bad calibration detected 0 probes for categories: {}",
                    undetected.join(", ")
                ))),
            ));
        }

        let rate = ((detected * 100) / probe_count) as u8;
        let detection_rate = DetectionRatePercent::try_new(rate)
            .map_err(|_| invalid_input_error("known_bad_detection_rate"))?;
        if detection_rate.value() < self.config.detection_threshold().get() {
            return Err(ObligationEvaluateError::VerifierPort(
                SemanticVerifierError::VerifierPort(diag(&format!(
                    "known-bad detection rate {} below threshold {}",
                    detection_rate.value(),
                    self.config.detection_threshold().get()
                ))),
            ));
        }
        Ok(detection_rate)
    }

    /// Builds one calibration-probe verdict future.
    ///
    /// Split out so the concurrency helper can fan the probes out under the
    /// same bounded ceiling as production pairs; the future's success value
    /// is a fulfillment verdict for the probe's category.
    fn calibration_probe_future<'a>(
        &'a self,
        shape: calibration::CalibrationProbeShape,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ObligationFulfillmentVerdict, ObligationEvaluateError>>
                + Send
                + 'a,
        >,
    > {
        let key = ObligationFulfillmentCacheKey::new(
            BoundTestsSetHash::new(self.hasher.sha256(shape.tests_source.as_bytes())),
            DeclarationHash::new(self.hasher.sha256(shape.declaration.as_bytes())),
            AnchorTextHash::new(self.hasher.sha256(shape.anchor_text.as_bytes())),
        );
        let calibration::CalibrationProbeShape { tests_source, declaration, anchor_text, .. } =
            shape;
        Box::pin(async move {
            let pair = ObligationFulfillmentPair::new(
                TestsSource::try_new(tests_source)
                    .map_err(|_| invalid_input_error("tests_source"))?,
                EntryDeclaration::try_new(declaration.to_owned())
                    .map_err(|_| invalid_input_error("entry_declaration"))?,
                AnchorText::try_new(anchor_text.to_owned())
                    .map_err(|_| invalid_input_error("anchor_text"))?,
            );
            self.fulfillment_driver
                .evaluate_with_escalation(&pair, &key, ModelTier::Fast)
                .await
                .map_err(map_verifier_error)
        })
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

fn production_pair_count(
    obligations: &ObligationsDocument,
    bindings: &TestBindingsDocument,
) -> usize {
    let mut count = 0usize;
    for record in bindings.records() {
        match record {
            TestBindingRecord::Fulfillment { obligation_id, .. } => {
                let edge_count = obligations
                    .obligations()
                    .iter()
                    .find(|obligation| obligation.id() == obligation_id)
                    .map(|obligation| obligation.spec_refs().len())
                    .unwrap_or(1);
                count = count.saturating_add(edge_count.max(1));
            }
            TestBindingRecord::VoluntaryBinding { .. } => {
                // Validation rejects a voluntary binding with any derived
                // owner before this count is read, so every valid voluntary
                // record contributes exactly one catalogue-only edge.
                count = count.saturating_add(1);
            }
            TestBindingRecord::Waiver { edge_id, .. } => {
                // Waivers are adjudicated once per owning obligation, so the
                // calibration budget scales with the owner count (minimum one
                // for catalogue-only edges).
                let owner_count = obligations.owners_of_edge(edge_id).len();
                count = count.saturating_add(owner_count.max(1));
            }
        }
    }
    count
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
                let detection_rate = self.known_bad_detection_rate(0).await?;
                // Existence-based scope: no materialized scope to evaluate
                // (IN-14). Rewrite caches to empty documents so `results` cannot
                // report verdicts from a previous materialized scope.
                self.save_caches(&cmd.track_id, Vec::new(), Vec::new())?;
                return Ok(EvaluateTestObligationsOutcome::new(0, 0, 0, detection_rate));
            }
            (Some(obligations), Some(bindings)) => (obligations, bindings),
            (None, Some(_)) => return Err(half_materialized_scope_error("obligations")),
            (Some(_), None) => return Err(half_materialized_scope_error("test-bindings")),
        };

        validate_voluntary_bindings(&obligations, &bindings)
            .map_err(ObligationEvaluateError::BindingConsistency)?;

        let detection_rate =
            self.known_bad_detection_rate(production_pair_count(&obligations, &bindings)).await?;
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

        Ok(EvaluateTestObligationsOutcome::new(
            tally.pass,
            tally.fail,
            tally.pending,
            detection_rate,
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
