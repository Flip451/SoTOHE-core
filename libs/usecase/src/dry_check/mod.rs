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
mod errors;
mod interactor;
mod judgment;
mod ports;
mod results;
mod results_interactor;
mod services;
pub(crate) mod shared;

pub use approval_interactor::DryCheckApprovalInteractor;
pub use errors::{DryCheckAgentError, DryCheckCycleError, DryCheckDiffError};
pub use interactor::DryCheckInteractor;
pub use judgment::DryCheckAgentJudgment;
pub use ports::{DryCheckAgentPort, DryCheckCoveragePort, DryCheckDiffSource};
pub use results::DryCheckResults;
pub use results_interactor::DryCheckResultsInteractor;
pub use services::{DryCheckApprovalService, DryCheckResultsService, DryCheckService};

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use domain::dry_check::{
        DryCheckEntry, DryCheckFinding, DryCheckPairKey, DryCheckRecord, DryCheckVerdict,
        FragmentContentHash, FragmentRef, Rationale, RefactorProposal,
    };
    use domain::review_v2::types::FilePath;
    use domain::{CommitHash, SimilarityScore, SimilarityThreshold, Timestamp};

    use super::{DryCheckAgentJudgment, DryCheckResults};

    fn make_rationale(s: &str) -> Rationale {
        Rationale::new(s).unwrap()
    }

    fn make_fragment_ref(path: &str, hash_char: char) -> FragmentRef {
        let hash = hash_char.to_string().repeat(64);
        let file_path = FilePath::new(path).unwrap();
        FragmentRef::new(file_path, FragmentContentHash::new(hash).unwrap())
    }

    fn make_refactor_proposal(s: &str) -> RefactorProposal {
        RefactorProposal::new(s).unwrap()
    }

    fn make_dry_check_finding() -> DryCheckFinding {
        let changed = make_fragment_ref("src/a.rs", 'a');
        let candidate = make_fragment_ref("src/b.rs", 'b');
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
        let low = make_fragment_ref("src/a.rs", 'a');
        let high = make_fragment_ref("src/b.rs", 'b');
        let pair_key = DryCheckPairKey::new(low, high).unwrap();
        let changed_path = FilePath::new("src/a.rs").unwrap();
        let verdict = DryCheckVerdict::Violation {
            refactor_proposal: make_refactor_proposal("Extract common trait."),
        };
        let score = SimilarityScore::new(0.92).unwrap();
        let threshold = SimilarityThreshold::new(0.85).unwrap();
        let base_commit = CommitHash::try_new("a".repeat(40)).unwrap();
        let rationale = make_rationale("Both modules implement identical parsing logic.");
        let entry = DryCheckEntry::new(
            pair_key,
            changed_path,
            verdict,
            score,
            threshold,
            base_commit,
            rationale,
        )
        .unwrap();
        let timestamp = Timestamp::new("2026-06-02T00:00:00Z").unwrap();
        DryCheckRecord::from_entry_and_timestamp(entry, timestamp).unwrap()
    }

    #[test]
    fn dry_check_results_carries_records_with_full_fields() {
        let record = make_dry_check_record();
        let results = DryCheckResults { records: vec![record.clone()] };

        assert_eq!(results.records.len(), 1);
        let r = &results.records[0];

        // pair_key accessible via FragmentRef — path via FilePath::as_str()
        assert_eq!(r.pair_key().low().path().as_str(), "src/a.rs");
        assert_eq!(r.pair_key().low().content_hash().as_str(), "a".repeat(64));
        assert_eq!(r.pair_key().high().path().as_str(), "src/b.rs");
        assert_eq!(r.pair_key().high().content_hash().as_str(), "b".repeat(64));

        // changed_path is display-only
        assert_eq!(r.changed_path().as_str(), "src/a.rs");

        // verdict is DryCheckVerdict::Violation
        assert!(matches!(r.verdict(), DryCheckVerdict::Violation { .. }));

        // rationale accessible (typed Rationale newtype)
        assert_eq!(r.rationale().as_str(), "Both modules implement identical parsing logic.");

        // recorded_at accessible (typed Timestamp)
        assert_eq!(r.recorded_at().as_str(), "2026-06-02T00:00:00Z");
    }
}
