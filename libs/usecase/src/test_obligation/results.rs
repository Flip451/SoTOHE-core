//! `bin/sotp test-obligation results` — informational verdict aggregation.
//!
//! [`TestObligationResultsInteractor`] reads the frozen fulfillment / waiver
//! verdict caches and renders a chain × layer summary plus per-edge records for
//! the failing / pending edges (IN-10 / AC-09). It is purely informational: the
//! exit code is always success (`Ok`) — the verdict gate is `check`'s job (CN-09).

use std::sync::Arc;

use domain::TrackId;
use domain::tddd::LayerId;
use domain::tddd::test_obligation::binding::{TestBindingRecord, TestBindingsDocument};
use domain::tddd::test_obligation::drift::{EdgeResolutionOutcome, EdgeVerdictRecord};
use domain::tddd::test_obligation::errors::{ArtifactCodecError, ObligationResultsError};
use domain::tddd::test_obligation::ids::{TestObligationEdgeId, TestObligationId};
use domain::tddd::test_obligation::ports::{
    ObligationFulfillmentCachePort, ObligationsArtifactPort, TestBindingsArtifactPort,
    WaiverCachePort,
};
use domain::tddd::test_obligation::scope::UncitedSpecElementFinding;
use domain::tddd::test_obligation::verdict::{
    ObligationFulfillmentCacheDocument, ObligationFulfillmentVerdict, WaiverCacheDocument,
    WaiverVerdict,
};
use domain::tddd::test_obligation::vocab::FulfillmentFailCategory;

use super::diag;

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
}

impl TestObligationResultsOutput {
    /// Builds a [`TestObligationResultsOutput`].
    #[must_use]
    pub fn new(
        lane_summaries: Vec<TestObligationLaneSummary>,
        records: Vec<EdgeVerdictRecord>,
        uncited_findings: Vec<UncitedSpecElementFinding>,
    ) -> Self {
        Self { lane_summaries, records, uncited_findings }
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
}

/// Command input for [`TestObligationResultsApplicationService`] (IN-10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestObligationResultsCommand {
    track_id: TrackId,
}

impl TestObligationResultsCommand {
    /// Builds a [`TestObligationResultsCommand`].
    #[must_use]
    pub fn new(track_id: TrackId) -> Self {
        Self { track_id }
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
    fulfillment_cache: Arc<dyn ObligationFulfillmentCachePort + Send + Sync>,
    waiver_cache: Arc<dyn WaiverCachePort + Send + Sync>,
}

impl TestObligationResultsInteractor {
    /// Builds a [`TestObligationResultsInteractor`] from its injected ports.
    #[must_use]
    pub fn new(
        obligations_port: Arc<dyn ObligationsArtifactPort + Send + Sync>,
        bindings_port: Arc<dyn TestBindingsArtifactPort + Send + Sync>,
        fulfillment_cache: Arc<dyn ObligationFulfillmentCachePort + Send + Sync>,
        waiver_cache: Arc<dyn WaiverCachePort + Send + Sync>,
    ) -> Self {
        Self { obligations_port, bindings_port, fulfillment_cache, waiver_cache }
    }
}

impl TestObligationResultsApplicationService for TestObligationResultsInteractor {
    fn execute(
        &self,
        cmd: &TestObligationResultsCommand,
    ) -> Result<TestObligationResultsOutput, ObligationResultsError> {
        // Loaded for completeness of the informational surface; a malformed
        // obligations artifact is surfaced rather than silently ignored.
        let _obligations = self.obligations_port.load(&cmd.track_id).map_err(map_artifact_error)?;
        let bindings = self.bindings_port.load(&cmd.track_id).map_err(map_artifact_error)?;
        let fulfillment = self.fulfillment_cache.load(&cmd.track_id).map_err(map_cache_error)?;
        let waiver = self.waiver_cache.load(&cmd.track_id).map_err(map_cache_error)?;

        let mut lane_summaries = Vec::new();
        let mut records = Vec::new();

        if let Some(document) = fulfillment.as_ref() {
            fulfillment_lanes(document, bindings.as_ref(), &mut lane_summaries, &mut records);
        }
        if let Some(document) = waiver.as_ref() {
            waiver_lane(document, &mut lane_summaries, &mut records);
        }

        Ok(TestObligationResultsOutput::new(lane_summaries, records, Vec::new()))
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
                records.push(EdgeVerdictRecord::new(
                    entry.edge_id().clone(),
                    EdgeResolutionOutcome::Fail(category.clone()),
                    None,
                ));
            }
            ObligationFulfillmentVerdict::Pending => {
                counts.pending += 1;
                records.push(EdgeVerdictRecord::new(
                    entry.edge_id().clone(),
                    EdgeResolutionOutcome::Pending,
                    None,
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
                ));
            }
            WaiverVerdict::Pending => {
                counts.pending += 1;
                records.push(waiver_record(entry.edge_id(), EdgeResolutionOutcome::Pending));
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

/// Builds a waiver [`EdgeVerdictRecord`] with no attached drift.
fn waiver_record(
    edge_id: &TestObligationEdgeId,
    outcome: EdgeResolutionOutcome,
) -> EdgeVerdictRecord {
    EdgeVerdictRecord::new(edge_id.clone(), outcome, None)
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
