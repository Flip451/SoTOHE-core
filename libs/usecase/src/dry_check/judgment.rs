//! The dry-checker agent's judgment output type.

use domain::dry_check::{DryCheckFinding, Rationale};

/// The dry-checker agent's judgment output.
///
/// Makes illegal states unrepresentable:
/// - A genuine `Violation` always carries both a [`DryCheckFinding`] (with
///   `RefactorProposal` and the changed/candidate `FragmentRef`s) and a
///   [`Rationale`].
/// - `NotAViolation` and `Accepted` carry a [`Rationale`] but no finding.
/// - Every verdict carries a required `Rationale` (validated non-empty by
///   `domain::dry_check::Rationale` newtype) so `DryCheckEntry.rationale` can
///   always be populated (D9/AC-06 mandate rationale on all records).
///
/// The interactor extracts `rationale` from the judgment to build `DryCheckEntry`,
/// and extracts `finding.refactor_proposal` (`RefactorProposal`) from `Violation`
/// variants to build `DryCheckVerdict::Violation { refactor_proposal }` for
/// persistence (enum-first D9), and surfaces findings from `Violation` variants
/// to the caller (IN-03/AC-03).
///
/// Pair identity (`DryCheckPairKey`) for all verdicts (`NotAViolation`,
/// `Accepted`, `Violation`) is derived by the interactor from the actual
/// `CodeFragment` values of the judged pair (`FragmentRef` computed via SHA-256
/// of `content()`) — not from `finding.changed_fragment_ref` or
/// `finding.candidate_fragment_ref`, which are `Violation`-only informational
/// live-output fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DryCheckAgentJudgment {
    /// The agent determined this pair is not a DRY violation.
    NotAViolation {
        /// Non-empty agent rationale (always present, D9/AC-06).
        rationale: Rationale,
    },
    /// The agent determined this duplication is acceptable.
    Accepted {
        /// Non-empty agent rationale (always present, D9/AC-06).
        rationale: Rationale,
    },
    /// Genuine DRY violation with a finding and a rationale.
    Violation {
        /// Non-empty agent rationale (always present, D9/AC-06).
        rationale: Rationale,
        /// The violation finding carrying the refactor proposal and fragment refs.
        finding: DryCheckFinding,
    },
}
