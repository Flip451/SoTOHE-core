//! `bin/sotp test-obligation results` — informational verdict aggregation.
//!
//! [`TestObligationResultsInteractor`] reads the frozen fulfillment / waiver
//! verdict caches and renders a chain × layer summary plus per-edge records for
//! the failing / pending edges (IN-10 / AC-09). It is purely informational: the
//! exit code is always success (`Ok`) — the verdict gate is `check`'s job (CN-09).

use std::sync::Arc;

use domain::SpecDocumentLoaderPort;
pub use domain::TaskStatusKind;
use domain::TrackId;
use domain::tddd::LayerId;
use domain::tddd::catalogue_v2::catalogue_impl_signals_ports::CatalogueDocumentLoaderPort;
pub use domain::tddd::semantic_verify::CatalogueEntryKey;
use domain::tddd::test_obligation::binding::{
    NonEmptyTestLocations, TestBindingRecord, TestBindingsDocument,
};
pub use domain::tddd::test_obligation::drift::{
    EdgeResolutionOutcome, EdgeVerdictRecord, TestObligationDrift,
};
use domain::tddd::test_obligation::errors::{ArtifactCodecError, ObligationResultsError};
use domain::tddd::test_obligation::hashes::VerifierPromptFingerprint;
pub use domain::tddd::test_obligation::ids::{
    TestObligationAnchorId, TestObligationEdgeId, TestObligationId, TestObligationItemIdentifier,
};
use domain::tddd::test_obligation::obligations::ObligationsDocument;
use domain::tddd::test_obligation::ports::{
    ObligationFulfillmentCachePort, ObligationsArtifactPort, TestBindingsArtifactPort,
    TestSourceScannerPort, WaiverCachePort,
};
use domain::tddd::test_obligation::scope::UncitedSpecElementFinding;
use domain::tddd::test_obligation::verdict::{
    ObligationFulfillmentCacheDocument, ObligationFulfillmentVerdict, WaiverCacheDocument,
    WaiverVerdict,
};
pub use domain::tddd::test_obligation::vocab::{FulfillmentFailCategory, TestObligationKind};

use crate::pre_review_gate::{ImplPlanReaderPort, TaskContractReaderPort};

use super::diag;
use super::results_status::collect_status_lane_summaries;

/// Verdict-chain lane discriminant for the results output (IN-10 / AC-09).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestObligationChainLabel {
    /// The obligation-fulfillment verdict chain.
    Fulfillment,
    /// The waiver verdict chain.
    Waiver,
}

/// A single chain × layer summary row (IN-10 / AC-09).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestObligationLaneSummary {
    chain_name: TestObligationChainLabel,
    layer: LayerId,
    pass_count: usize,
    fail_count: usize,
    pending_count: usize,
}

impl TestObligationLaneSummary {
    /// Builds a [`TestObligationLaneSummary`].
    #[must_use]
    pub fn new(
        chain_name: TestObligationChainLabel,
        layer: LayerId,
        pass_count: usize,
        fail_count: usize,
        pending_count: usize,
    ) -> Self {
        Self { chain_name, layer, pass_count, fail_count, pending_count }
    }

    /// Returns the chain this lane summarises.
    #[must_use]
    pub fn chain_name(&self) -> &TestObligationChainLabel {
        &self.chain_name
    }

    /// Returns the layer this lane summarises.
    #[must_use]
    pub fn layer(&self) -> &LayerId {
        &self.layer
    }

    /// Returns the count of passing verdicts in this lane.
    #[must_use]
    pub fn pass_count(&self) -> usize {
        self.pass_count
    }

    /// Returns the count of failing verdicts in this lane.
    #[must_use]
    pub fn fail_count(&self) -> usize {
        self.fail_count
    }

    /// Returns the count of pending verdicts in this lane.
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending_count
    }
}

/// Structured output of [`TestObligationResultsInteractor`] (IN-10 / AC-09).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestObligationResultsOutput {
    lane_summaries: Vec<TestObligationLaneSummary>,
    records: Vec<EdgeVerdictRecord>,
    uncited_findings: Vec<UncitedSpecElementFinding>,
    status_lane_summaries: Result<
        Vec<TestObligationStatusLaneSummary>,
        domain::tddd::test_obligation::ids::DiagnosticMessage,
    >,
}

impl TestObligationResultsOutput {
    /// Builds a [`TestObligationResultsOutput`].
    #[must_use]
    pub fn new(
        lane_summaries: Vec<TestObligationLaneSummary>,
        records: Vec<EdgeVerdictRecord>,
        uncited_findings: Vec<UncitedSpecElementFinding>,
        status_lane_summaries: Result<
            Vec<TestObligationStatusLaneSummary>,
            domain::tddd::test_obligation::ids::DiagnosticMessage,
        >,
    ) -> Self {
        Self { lane_summaries, records, uncited_findings, status_lane_summaries }
    }

    /// Returns the chain × layer lane summaries.
    #[must_use]
    pub fn lane_summaries(&self) -> &[TestObligationLaneSummary] {
        &self.lane_summaries
    }

    /// Returns the per-edge records for the failing / pending edges.
    #[must_use]
    pub fn records(&self) -> &[EdgeVerdictRecord] {
        &self.records
    }

    /// Returns the uncited spec-element findings carried through the output.
    #[must_use]
    pub fn uncited_findings(&self) -> &[UncitedSpecElementFinding] {
        &self.uncited_findings
    }

    /// Returns unresolved findings grouped by task-status lane, or the
    /// diagnostic explaining why that independent lane is unavailable.
    pub fn status_lane_summaries(
        &self,
    ) -> Result<
        &[TestObligationStatusLaneSummary],
        &domain::tddd::test_obligation::ids::DiagnosticMessage,
    > {
        self.status_lane_summaries.as_deref()
    }
}

/// Informational unresolved counts for one task-status lane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestObligationStatusLaneSummary {
    task_status: TaskStatusKind,
    missing_count: usize,
    stale_count: usize,
    verdict_absent_count: usize,
}

impl TestObligationStatusLaneSummary {
    /// Builds a task-status unresolved summary.
    #[must_use]
    pub fn new(
        task_status: TaskStatusKind,
        missing_count: usize,
        stale_count: usize,
        verdict_absent_count: usize,
    ) -> Self {
        Self { task_status, missing_count, stale_count, verdict_absent_count }
    }

    /// Returns the task-status lane represented by this summary.
    #[must_use]
    pub fn task_status(&self) -> TaskStatusKind {
        self.task_status
    }

    /// Returns the number of missing bindings or test sources in this lane.
    #[must_use]
    pub fn missing_count(&self) -> usize {
        self.missing_count
    }

    /// Returns the number of hash-stale edges in this lane.
    #[must_use]
    pub fn stale_count(&self) -> usize {
        self.stale_count
    }

    /// Returns the number of edges lacking a current verdict in this lane.
    #[must_use]
    pub fn verdict_absent_count(&self) -> usize {
        self.verdict_absent_count
    }
}

/// Command input for [`TestObligationResultsApplicationService`] (IN-10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestObligationResultsCommand {
    track_id: TrackId,
    catalogue_paths: Vec<std::path::PathBuf>,
}

impl TestObligationResultsCommand {
    /// Builds a [`TestObligationResultsCommand`].
    #[must_use]
    pub fn new(track_id: TrackId, catalogue_paths: Vec<std::path::PathBuf>) -> Self {
        Self { track_id, catalogue_paths }
    }
}

/// Primary port for `bin/sotp test-obligation results` (IN-10 / AC-09).
pub trait TestObligationResultsApplicationService {
    /// Aggregates the frozen verdict caches into an informational report.
    ///
    /// # Errors
    ///
    /// Returns [`ObligationResultsError`] when a cache / artifact cannot be read
    /// or is malformed.
    fn execute(
        &self,
        cmd: &TestObligationResultsCommand,
    ) -> Result<TestObligationResultsOutput, ObligationResultsError>;
}

/// Interactor implementing [`TestObligationResultsApplicationService`] (IN-10).
pub struct TestObligationResultsInteractor {
    obligations_port: Arc<dyn ObligationsArtifactPort + Send + Sync>,
    bindings_port: Arc<dyn TestBindingsArtifactPort + Send + Sync>,
    source_scanner: Arc<dyn TestSourceScannerPort + Send + Sync>,
    fulfillment_cache: Arc<dyn ObligationFulfillmentCachePort + Send + Sync>,
    waiver_cache: Arc<dyn WaiverCachePort + Send + Sync>,
    fulfillment_verifier_fingerprint: VerifierPromptFingerprint,
    waiver_verifier_fingerprint: VerifierPromptFingerprint,
    spec_reader: Arc<dyn SpecDocumentLoaderPort + Send + Sync>,
    catalogue_reader: Arc<dyn CatalogueDocumentLoaderPort + Send + Sync>,
    task_contract_reader: Arc<dyn TaskContractReaderPort>,
    impl_plan_reader: Arc<dyn ImplPlanReaderPort>,
}

impl TestObligationResultsInteractor {
    /// Builds a [`TestObligationResultsInteractor`] from its injected ports.
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        obligations_port: Arc<dyn ObligationsArtifactPort + Send + Sync>,
        bindings_port: Arc<dyn TestBindingsArtifactPort + Send + Sync>,
        source_scanner: Arc<dyn TestSourceScannerPort + Send + Sync>,
        fulfillment_cache: Arc<dyn ObligationFulfillmentCachePort + Send + Sync>,
        waiver_cache: Arc<dyn WaiverCachePort + Send + Sync>,
        fulfillment_verifier_fingerprint: VerifierPromptFingerprint,
        waiver_verifier_fingerprint: VerifierPromptFingerprint,
        spec_reader: Arc<dyn SpecDocumentLoaderPort + Send + Sync>,
        catalogue_reader: Arc<dyn CatalogueDocumentLoaderPort + Send + Sync>,
        task_contract_reader: Arc<dyn TaskContractReaderPort>,
        impl_plan_reader: Arc<dyn ImplPlanReaderPort>,
    ) -> Self {
        Self {
            obligations_port,
            bindings_port,
            source_scanner,
            fulfillment_cache,
            waiver_cache,
            fulfillment_verifier_fingerprint,
            waiver_verifier_fingerprint,
            spec_reader,
            catalogue_reader,
            task_contract_reader,
            impl_plan_reader,
        }
    }
}

impl TestObligationResultsApplicationService for TestObligationResultsInteractor {
    fn execute(
        &self,
        cmd: &TestObligationResultsCommand,
    ) -> Result<TestObligationResultsOutput, ObligationResultsError> {
        let obligations = self.obligations_port.load(&cmd.track_id).map_err(map_artifact_error)?;
        let bindings = self.bindings_port.load(&cmd.track_id).map_err(map_artifact_error)?;
        let fulfillment = self.fulfillment_cache.load(&cmd.track_id).map_err(map_cache_error)?;
        let waiver = self.waiver_cache.load(&cmd.track_id).map_err(map_cache_error)?;

        let mut lane_summaries = Vec::new();
        let mut records = Vec::new();

        if let Some(document) = fulfillment.as_ref() {
            fulfillment_lanes(document, bindings.as_ref(), &mut lane_summaries, &mut records);
        }
        if let Some(document) = waiver.as_ref() {
            waiver_lane(
                document,
                bindings.as_ref(),
                obligations.as_ref(),
                &mut lane_summaries,
                &mut records,
            );
        }

        let status_lane_summaries = collect_status_lane_summaries(
            &cmd.track_id,
            &cmd.catalogue_paths,
            obligations.as_ref(),
            bindings.as_ref(),
            fulfillment.as_ref(),
            waiver.as_ref(),
            self.source_scanner.as_ref(),
            &self.fulfillment_verifier_fingerprint,
            &self.waiver_verifier_fingerprint,
            self.spec_reader.as_ref(),
            self.catalogue_reader.as_ref(),
            self.task_contract_reader.as_ref(),
            self.impl_plan_reader.as_ref(),
        )
        .map_err(status_lane_diagnostic);

        Ok(TestObligationResultsOutput::new(
            lane_summaries,
            records,
            Vec::new(),
            status_lane_summaries,
        ))
    }
}

/// Extracts the validated diagnostic for an independently unavailable status lane.
fn status_lane_diagnostic(
    error: ObligationResultsError,
) -> domain::tddd::test_obligation::ids::DiagnosticMessage {
    match error {
        ObligationResultsError::IoError(message)
        | ObligationResultsError::MalformedArtifact(message) => message,
    }
}

/// Maps an obligations / bindings artifact error onto the results vocabulary.
fn map_artifact_error(error: ArtifactCodecError) -> ObligationResultsError {
    match error {
        ArtifactCodecError::Io(message) => ObligationResultsError::IoError(message),
        ArtifactCodecError::MalformedJson(message)
        | ArtifactCodecError::DomainInvariant(message) => {
            ObligationResultsError::MalformedArtifact(message)
        }
    }
}

/// Maps a verdict-cache error onto the informational results error vocabulary.
fn map_cache_error(
    error: domain::tddd::test_obligation::errors::VerifyCacheError,
) -> ObligationResultsError {
    use domain::tddd::test_obligation::errors::VerifyCacheError;
    match error {
        VerifyCacheError::Io(message) => ObligationResultsError::IoError(diag(message.as_str())),
        VerifyCacheError::MalformedJson(message) => {
            ObligationResultsError::MalformedArtifact(diag(message.as_str()))
        }
    }
}

/// Pass / fail / pending tally for one lane.
#[derive(Default, Clone)]
struct Counts {
    pass: usize,
    fail: usize,
    pending: usize,
}

impl Counts {
    /// Returns `true` when at least one verdict landed in this tally.
    fn is_populated(&self) -> bool {
        self.pass > 0 || self.fail > 0 || self.pending > 0
    }
}

/// Accumulates fulfillment lanes grouped by resolved layer plus fail / pending
/// records.
fn fulfillment_lanes(
    document: &ObligationFulfillmentCacheDocument,
    bindings: Option<&TestBindingsDocument>,
    lanes: &mut Vec<TestObligationLaneSummary>,
    records: &mut Vec<EdgeVerdictRecord>,
) {
    let mut buckets: Vec<(LayerId, Counts)> = Vec::new();
    for entry in document.entries() {
        let layer = resolve_layer(entry.obligation_id(), entry.edge_id(), bindings);
        let index = match buckets.iter().position(|(existing, _)| *existing == layer) {
            Some(index) => index,
            None => {
                buckets.push((layer, Counts::default()));
                buckets.len().saturating_sub(1)
            }
        };
        let Some((_, counts)) = buckets.get_mut(index) else {
            continue;
        };
        match entry.verdict() {
            ObligationFulfillmentVerdict::Fulfilled { .. } => counts.pass += 1,
            ObligationFulfillmentVerdict::Fail { category, .. } => {
                counts.fail += 1;
                records.push(fulfillment_record(
                    entry,
                    EdgeResolutionOutcome::Fail(category.clone()),
                    fulfillment_verdict_reason(entry.verdict()),
                    bindings,
                ));
            }
            ObligationFulfillmentVerdict::Pending => {
                counts.pending += 1;
                records.push(fulfillment_record(
                    entry,
                    EdgeResolutionOutcome::Pending,
                    None,
                    bindings,
                ));
            }
        }
    }
    for (layer, counts) in buckets {
        lanes.push(TestObligationLaneSummary::new(
            TestObligationChainLabel::Fulfillment,
            layer,
            counts.pass,
            counts.fail,
            counts.pending,
        ));
    }
}

/// Accumulates a single waiver lane (waiver caches carry no layer evidence).
fn waiver_lane(
    document: &WaiverCacheDocument,
    bindings: Option<&TestBindingsDocument>,
    obligations: Option<&ObligationsDocument>,
    lanes: &mut Vec<TestObligationLaneSummary>,
    records: &mut Vec<EdgeVerdictRecord>,
) {
    let mut counts = Counts::default();
    for entry in document.entries() {
        match entry.verdict() {
            WaiverVerdict::Waived { .. } => counts.pass += 1,
            WaiverVerdict::Fail { .. } => {
                counts.fail += 1;
                records.push(waiver_record(
                    entry.edge_id(),
                    EdgeResolutionOutcome::Fail(FulfillmentFailCategory::CentralUnverified),
                    waiver_verdict_reason(entry.verdict()),
                    bindings,
                    obligations,
                ));
            }
            WaiverVerdict::Pending => {
                counts.pending += 1;
                records.push(waiver_record(
                    entry.edge_id(),
                    EdgeResolutionOutcome::Pending,
                    None,
                    bindings,
                    obligations,
                ));
            }
        }
    }
    if counts.is_populated() {
        lanes.push(TestObligationLaneSummary::new(
            TestObligationChainLabel::Waiver,
            fallback_layer(),
            counts.pass,
            counts.fail,
            counts.pending,
        ));
    }
}

/// Builds a fulfillment [`EdgeVerdictRecord`] with binding-derived provenance.
fn fulfillment_record(
    entry: &domain::tddd::test_obligation::verdict::ObligationFulfillmentCacheEntry,
    outcome: EdgeResolutionOutcome,
    verdict_reason: Option<domain::tddd::test_obligation::ids::DiagnosticMessage>,
    bindings: Option<&TestBindingsDocument>,
) -> EdgeVerdictRecord {
    let (claim_source, evidence_source) =
        fulfillment_binding_sources(entry.obligation_id(), entry.edge_id(), bindings);
    EdgeVerdictRecord::new(
        Some(entry.obligation_id().clone()),
        entry.edge_id().clone(),
        claim_source,
        evidence_source,
        outcome,
        verdict_reason,
        None,
    )
}

/// Builds a waiver [`EdgeVerdictRecord`] with binding- and obligation-derived provenance.
fn waiver_record(
    edge_id: &TestObligationEdgeId,
    outcome: EdgeResolutionOutcome,
    verdict_reason: Option<domain::tddd::test_obligation::ids::DiagnosticMessage>,
    bindings: Option<&TestBindingsDocument>,
    obligations: Option<&ObligationsDocument>,
) -> EdgeVerdictRecord {
    let obligation_id = obligations
        .and_then(|document| document.owning_obligation(edge_id))
        .map(|obligation| obligation.id().clone());
    let (claim_source, evidence_source) = waiver_binding_sources(edge_id, bindings);
    EdgeVerdictRecord::new(
        obligation_id,
        edge_id.clone(),
        claim_source,
        evidence_source,
        outcome,
        verdict_reason,
        None,
    )
}

/// Returns waiver provenance only when the bindings artifact contains this edge.
fn waiver_binding_sources(
    edge_id: &TestObligationEdgeId,
    bindings: Option<&TestBindingsDocument>,
) -> (
    Option<domain::tddd::test_obligation::ids::DiagnosticMessage>,
    Option<domain::tddd::test_obligation::ids::DiagnosticMessage>,
) {
    let reason = bindings.and_then(|document| {
        document.records().iter().find_map(|record| match record {
            TestBindingRecord::Waiver { edge_id: bound, reason } if bound == edge_id => {
                Some(reason)
            }
            _ => None,
        })
    });
    match reason {
        Some(reason) => (Some(diag("waiver")), Some(diag(reason.as_str()))),
        None => (None, None),
    }
}

/// Returns the claim and evidence source recorded for a fulfillment edge.
fn fulfillment_binding_sources(
    obligation_id: &TestObligationId,
    edge_id: &TestObligationEdgeId,
    bindings: Option<&TestBindingsDocument>,
) -> (
    Option<domain::tddd::test_obligation::ids::DiagnosticMessage>,
    Option<domain::tddd::test_obligation::ids::DiagnosticMessage>,
) {
    let Some(document) = bindings else {
        return (None, None);
    };
    for record in document.records() {
        match record {
            TestBindingRecord::Fulfillment { obligation_id: bound, tests }
                if bound == obligation_id =>
            {
                return (Some(diag("fulfillment binding")), Some(bound_tests_source(tests)));
            }
            TestBindingRecord::VoluntaryBinding { edge_id: bound, tests } if bound == edge_id => {
                return (Some(diag("voluntary binding")), Some(bound_tests_source(tests)));
            }
            _ => {}
        }
    }
    (None, None)
}

/// Renders bound test locations compactly for the informational results record.
fn bound_tests_source(
    tests: &NonEmptyTestLocations,
) -> domain::tddd::test_obligation::ids::DiagnosticMessage {
    let locations = tests
        .as_slice()
        .iter()
        .map(|location| {
            let layer: &str = location.layer().as_ref();
            format!(
                "{layer}::{}::{}",
                location.module_path().as_str(),
                location.test_name().as_str()
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    diag(&locations)
}

/// Extracts the failure explanation from a fulfillment verdict.
fn fulfillment_verdict_reason(
    verdict: &ObligationFulfillmentVerdict,
) -> Option<domain::tddd::test_obligation::ids::DiagnosticMessage> {
    match verdict {
        ObligationFulfillmentVerdict::Fail { reason, .. } => Some(reason.clone()),
        ObligationFulfillmentVerdict::Fulfilled { .. } | ObligationFulfillmentVerdict::Pending => {
            None
        }
    }
}

/// Extracts the failure explanation from a waiver verdict.
fn waiver_verdict_reason(
    verdict: &WaiverVerdict,
) -> Option<domain::tddd::test_obligation::ids::DiagnosticMessage> {
    match verdict {
        WaiverVerdict::Fail { reason } => Some(reason.clone()),
        WaiverVerdict::Waived { .. } | WaiverVerdict::Pending => None,
    }
}

/// Resolves the layer of a fulfillment obligation via its binding test
/// locations, falling back to a synthetic workspace layer.
fn resolve_layer(
    obligation_id: &TestObligationId,
    edge_id: &TestObligationEdgeId,
    bindings: Option<&TestBindingsDocument>,
) -> LayerId {
    if let Some(document) = bindings {
        for record in document.records() {
            match record {
                TestBindingRecord::Fulfillment { obligation_id: bound, tests }
                    if bound == obligation_id =>
                {
                    return tests.first().layer().clone();
                }
                TestBindingRecord::VoluntaryBinding { edge_id: bound, tests }
                    if bound == edge_id =>
                {
                    return tests.first().layer().clone();
                }
                _ => {}
            }
        }
    }
    fallback_layer()
}

/// A synthetic workspace layer used when no per-edge layer evidence exists.
fn fallback_layer() -> LayerId {
    let mut name = "workspace".to_owned();
    loop {
        match LayerId::try_new(name) {
            Ok(layer) => return layer,
            // Unreachable: `workspace` is a valid layer id; reset defensively.
            Err(_) => name = "workspace".to_owned(),
        }
    }
}

#[cfg(test)]
#[path = "results_tests.rs"]
mod tests;
