//! Public command and result types for the test-obligation check use case.

#![allow(clippy::result_large_err)]

use domain::tddd::test_obligation::errors::ObligationCheckError;
use domain::tddd::test_obligation::ids::TestObligationEdgeId;
use domain::tddd::test_obligation::scope::UncitedSpecElementFinding;

use super::TestObligationCatalogueCommandInput;
use super::results::TestObligationStatusLaneSummary;

/// Command input for [`CheckTestObligationsApplicationService`] (IN-08).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckTestObligationsCommand {
    pub(super) input: TestObligationCatalogueCommandInput,
}

impl CheckTestObligationsCommand {
    /// Builds a [`CheckTestObligationsCommand`].
    #[must_use]
    pub fn new(input: TestObligationCatalogueCommandInput) -> Self {
        Self { input }
    }
}

/// Structured output of a passing `check`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckTestObligationsOutcome {
    resolved_edges: Vec<TestObligationEdgeId>,
    uncited_findings: Vec<UncitedSpecElementFinding>,
    status_lane_summaries: Vec<TestObligationStatusLaneSummary>,
}

impl CheckTestObligationsOutcome {
    /// Builds a verified-scope outcome (both artifacts present, all edges fresh).
    #[must_use]
    pub fn new_verified_scope(
        resolved_edges: Vec<TestObligationEdgeId>,
        uncited_findings: Vec<UncitedSpecElementFinding>,
        status_lane_summaries: Vec<TestObligationStatusLaneSummary>,
    ) -> Self {
        Self { resolved_edges, uncited_findings, status_lane_summaries }
    }

    /// Builds an empty-scope outcome (both artifacts absent — zero pairs).
    #[must_use]
    pub fn new_empty_scope(uncited_findings: Vec<UncitedSpecElementFinding>) -> Self {
        Self { resolved_edges: Vec::new(), uncited_findings, status_lane_summaries: Vec::new() }
    }

    /// Returns the edges resolved by a fresh fulfilled / waived verdict.
    #[must_use]
    pub fn resolved_edges(&self) -> &[TestObligationEdgeId] {
        &self.resolved_edges
    }

    /// Returns the uncited `AC` / `CN` spec-element findings.
    #[must_use]
    pub fn uncited_findings(&self) -> &[UncitedSpecElementFinding] {
        &self.uncited_findings
    }

    /// Returns unresolved findings aggregated by task-status lane.
    #[must_use]
    pub fn status_lane_summaries(&self) -> &[TestObligationStatusLaneSummary] {
        &self.status_lane_summaries
    }
}

/// Primary port for `bin/sotp test-obligation check`.
pub trait CheckTestObligationsApplicationService {
    /// Runs the pure-read totality + drift gate.
    ///
    /// # Errors
    ///
    /// Returns [`ObligationCheckError`] for structural artifact failures or
    /// blocking unresolved findings.
    fn execute(
        &self,
        cmd: &CheckTestObligationsCommand,
    ) -> Result<CheckTestObligationsOutcome, ObligationCheckError>;
}
