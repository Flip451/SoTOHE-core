//! Verdict types: `DryCheckVerdict`, `VerdictFilter`, `DryCheckApprovalVerdict`.

use std::fmt;

use super::value_objects::RefactorProposal;

// ── DryCheckVerdict ───────────────────────────────────────────────────────────

/// Per-pair DRY check verdict (enum-first design, D9).
///
/// - `NotAViolation`: false positive rejected by agent.
/// - `Accepted`: agent-determined acceptable duplication.
/// - `Violation`: genuine DRY violation with a mandatory non-empty refactor proposal.
///
/// "Violation without a proposal" and "non-violation with a proposal" are
/// structurally unrepresentable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DryCheckVerdict {
    /// False positive: the agent determined this pair is not a DRY violation.
    NotAViolation,
    /// Agent-approved duplication: acceptable similarity (cross-layer mirror,
    /// tests, boilerplate, etc.).
    Accepted,
    /// Genuine DRY violation carrying the mandatory non-empty refactor proposal.
    Violation {
        /// Non-empty refactor proposal from the agent.
        refactor_proposal: RefactorProposal,
    },
}

impl fmt::Display for DryCheckVerdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAViolation => f.write_str("not-a-violation"),
            Self::Accepted => f.write_str("accepted"),
            Self::Violation { refactor_proposal } => {
                write!(f, "violation({})", refactor_proposal.as_str())
            }
        }
    }
}

// ── VerdictFilter ─────────────────────────────────────────────────────────────

/// Classification filter for the dry-check read path.
///
/// Passed to `DryCheckResultsService::get_results()` to limit results to a
/// specific verdict class or return all records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerdictFilter {
    /// Return all records.
    All,
    /// Return only `NotAViolation` records.
    NotAViolation,
    /// Return only `Accepted` records.
    Accepted,
    /// Return only `Violation` records.
    Violation,
}

// ── DryCheckApprovalVerdict ───────────────────────────────────────────────────

/// Domain verdict for the dry-check gate (D7/D10).
///
/// - `Approved`: all above-threshold pairs verified as not-a-violation or accepted.
/// - `Blocked`: unresolved violations or unverified pairs remain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DryCheckApprovalVerdict {
    /// All above-threshold pairs are verified (not-a-violation or accepted).
    Approved,
    /// Unresolved violations or unverified pairs remain.
    Blocked {
        /// Number of unresolved pairs.
        unresolved_pair_count: usize,
    },
}
