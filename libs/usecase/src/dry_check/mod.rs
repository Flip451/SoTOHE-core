//! Usecase layer for the DRY violation auto-detection capability.
//!
//! Defines application service traits, secondary port traits, judgment/error
//! types, and query result structs for the dry-check feature
//! (ADR 2026-06-02-0716-dry-checker).
//!
//! Interactor implementations:
//! - `DryCheckInteractor` (T004) — write path
//! - `DryCheckResultsInteractor`, `DryCheckApprovalInteractor` (T005) — read/gate paths

mod approval_interactor;
mod calibration;
mod config;
mod errors;
mod interactor;
mod judgment;
mod known_bad;
mod ports;
mod results;
mod results_interactor;
mod services;
pub(crate) mod shared;

pub use approval_interactor::DryCheckApprovalInteractor;
pub use config::{DryCheckConfig, DryCheckParallelism, DryCheckPercent};
pub use errors::{DryCheckAgentError, DryCheckCycleError, DryCheckDiffError};
pub use interactor::DryCheckInteractor;
pub use judgment::DryCheckAgentJudgment;
pub use ports::{DryCheckAgentPort, DryCheckCoveragePort, DryCheckDiffSource, DryCheckJudgeTier};
pub use results::DryCheckResults;
pub use results_interactor::DryCheckResultsInteractor;
pub use services::{DryCheckApprovalService, DryCheckResultsService, DryCheckService};
pub use shared::fragment_ref_of;

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use domain::dry_check::{
        DryCheckFinding, DryCheckRecord, DryCheckVerdict, Rationale, RefactorProposal,
    };

    use super::{DryCheckAgentJudgment, DryCheckResults};
    use crate::dry_check::shared::test_mocks::{
        assert_record_full_fields, make_dry_check_record_for_tests, make_fragment_ref_for_tests,
    };

    fn make_rationale(s: &str) -> Rationale {
        Rationale::new(s).unwrap()
    }

    fn make_refactor_proposal(s: &str) -> RefactorProposal {
        RefactorProposal::new(s).unwrap()
    }

    fn make_dry_check_finding() -> DryCheckFinding {
        let changed = make_fragment_ref_for_tests("src/a.rs", 'a');
        let candidate = make_fragment_ref_for_tests("src/b.rs", 'b');
        DryCheckFinding::new(changed, candidate, "Extract shared logic into a common module.")
            .unwrap()
    }

    // ── DryCheckAgentJudgment tests ──────────────────────────────────────────

    #[test]
    fn dry_check_agent_judgment_not_a_violation_carries_rationale_and_no_finding() {
        let rationale = make_rationale("These are in different layers and cannot be merged.");
        let judgment = DryCheckAgentJudgment::NotAViolation { rationale: rationale.clone() };

        match judgment {
            DryCheckAgentJudgment::NotAViolation { rationale: r } => {
                assert_eq!(r, rationale);
            }
            other => panic!("expected NotAViolation, got {other:?}"),
        }
    }

    #[test]
    fn dry_check_agent_judgment_accepted_carries_rationale_and_no_finding() {
        let rationale = make_rationale("Accepted: cross-layer mirror pattern is intentional.");
        let judgment = DryCheckAgentJudgment::Accepted { rationale: rationale.clone() };

        match judgment {
            DryCheckAgentJudgment::Accepted { rationale: r } => {
                assert_eq!(r, rationale);
            }
            other => panic!("expected Accepted, got {other:?}"),
        }
    }

    #[test]
    fn dry_check_agent_judgment_violation_carries_rationale_and_finding() {
        let rationale = make_rationale("Genuine duplication — both implement identical parsing.");
        let finding = make_dry_check_finding();
        let judgment = DryCheckAgentJudgment::Violation {
            rationale: rationale.clone(),
            finding: finding.clone(),
        };

        match judgment {
            DryCheckAgentJudgment::Violation { rationale: r, finding: f } => {
                assert_eq!(r, rationale);
                assert_eq!(f, finding);
                // refactor_proposal is a RefactorProposal (validated non-empty newtype)
                let expected = make_refactor_proposal("Extract shared logic into a common module.");
                assert_eq!(f.refactor_proposal().as_str(), expected.as_str());
            }
            other => panic!("expected Violation, got {other:?}"),
        }
    }

    // ── DryCheckResults tests ────────────────────────────────────────────────

    fn make_dry_check_record() -> DryCheckRecord {
        let low = make_fragment_ref_for_tests("src/a.rs", 'a');
        let high = make_fragment_ref_for_tests("src/b.rs", 'b');
        let verdict = DryCheckVerdict::Violation {
            refactor_proposal: make_refactor_proposal("Extract common trait."),
        };
        make_dry_check_record_for_tests(low, high, verdict, "2026-06-02T00:00:00Z")
    }

    #[test]
    fn dry_check_results_carries_records_with_full_fields() {
        let record = make_dry_check_record();
        let results = DryCheckResults { records: vec![record.clone()] };

        assert_eq!(results.records.len(), 1);
        let r = &results.records[0];

        // Known-correct SHA-256 hex constants (independent oracle — not derived
        // via `content_hash_of` at runtime so a broken hash derivation is caught).
        // SHA-256("a") and SHA-256("b") respectively.
        const LOW_HASH: &str = "ca978112ca1bbdcafac231b39a23dc4da786eff8147c4e72b9807785afee48bb";
        const HIGH_HASH: &str = "3e23e8160039594a33894f6564e1b1348bbd7a0088d42c4acb73eeaed59c009d";

        assert_record_full_fields(
            r,
            "src/a.rs",
            LOW_HASH,
            "src/b.rs",
            HIGH_HASH,
            "src/a.rs",
            "test", // rationale fixed by make_dry_check_record_for_tests
            "2026-06-02T00:00:00Z",
            &DryCheckVerdict::Violation {
                refactor_proposal: make_refactor_proposal("Extract common trait."),
            },
            Some("Extract common trait."),
        );
    }
}
