//! Bound-test source helpers for the test-obligation check interactor.

#![allow(clippy::result_large_err)]

use domain::ContentHash;
use domain::tddd::test_obligation::binding::TestLocation;
use domain::tddd::test_obligation::drift::TestObligationDrift;
use domain::tddd::test_obligation::errors::{ObligationCheckError, TestSourceScanError};
use domain::tddd::test_obligation::ids::{TestObligationEdgeId, TestObligationId};

use super::check::CheckTestObligationsInteractor;
use super::check_support::GateState;
use super::status_lanes::StatusLaneFindingKind;
use super::{diag, sha256_content_hash};

impl CheckTestObligationsInteractor {
    /// Hashes the current bound-test bodies for freshness comparison.
    pub(super) fn current_bound_hash(
        &self,
        tests: &[TestLocation],
    ) -> Result<ContentHash, ObligationCheckError> {
        let mut source = String::new();
        for location in tests {
            let body = self
                .source_scanner
                .scan_test_body(location)
                .map_err(ObligationCheckError::SourceScan)?
                .ok_or_else(|| {
                    ObligationCheckError::SourceScan(TestSourceScanError::Io(diag(
                        "bound test source not found",
                    )))
                })?;
            source.push_str(&body);
            source.push('\n');
        }
        Ok(sha256_content_hash(source.as_bytes()))
    }

    /// Verifies that every bound test location still resolves in the worktree.
    pub(super) fn bound_test_sources_exist(
        &self,
        tests: &[TestLocation],
    ) -> Result<bool, ObligationCheckError> {
        for location in tests {
            if self
                .source_scanner
                .scan_test_body(location)
                .map_err(ObligationCheckError::SourceScan)?
                .is_none()
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Classifies an unavailable fulfillment verdict after checking test existence.
    pub(super) fn classify_unavailable_fulfillment_verdict(
        &self,
        edge: &TestObligationEdgeId,
        obligation_id: &TestObligationId,
        tests: &[TestLocation],
        gate: &mut GateState,
    ) -> Result<(), ObligationCheckError> {
        if self.bound_test_sources_exist(tests)? {
            gate.verdict_absent(edge.clone());
        } else {
            gate.status_drift(
                TestObligationDrift::missing_obligation(
                    obligation_id.clone(),
                    diag("bound test source not found"),
                ),
                obligation_id.entry_key().clone(),
                StatusLaneFindingKind::Missing,
            );
        }
        Ok(())
    }
}
