//! Round-level types for the cycle-based review model.
//!
//! Contains: `ReviewStalenessReason`, `GroupRoundOutcome`, `StoredFinding`,
//! `NonEmptyFindings`, `GroupRoundVerdict`, `GroupRound`, `CycleGroupState`,
//! and `CycleError`.

use crate::Timestamp;

use super::super::types::{RoundType, Verdict};

// ---------------------------------------------------------------------------
// ReviewStalenessReason
// ---------------------------------------------------------------------------

/// Why a review cycle is considered stale.
///
/// Each variant maps to a distinct cause that requires starting a new cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewStalenessReason {
    /// Base policy (`track/review-scope.json`) changed since cycle start.
    PolicyChanged,
    /// Per-track groups override (`review-groups.json`) changed since cycle start.
    PartitionChanged,
    /// Group-scope hash does not match current code state.
    HashMismatch,
}

// ---------------------------------------------------------------------------
// GroupRoundOutcome
// ---------------------------------------------------------------------------

/// Outcome of a review round execution.
///
/// Modeled as an enum so that success always carries a verdict and failure
/// never does, making impossible states (e.g., `Success` with error message,
/// `Failure` with `ZeroFindings`) structurally unrepresentable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupRoundOutcome {
    /// The reviewer process completed and returned a verdict.
    Success(GroupRoundVerdict),
    /// The reviewer process failed (timeout, crash, etc.).
    Failure { error_message: Option<String> },
}

impl GroupRoundOutcome {
    /// Returns `true` if this is a successful outcome.
    #[must_use]
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success(_))
    }

    /// Returns the verdict if successful, `None` if failed.
    #[must_use]
    pub fn verdict(&self) -> Option<&GroupRoundVerdict> {
        match self {
            Self::Success(v) => Some(v),
            Self::Failure { .. } => None,
        }
    }

    /// Returns the error message if failed, `None` if successful.
    #[must_use]
    pub fn error_message(&self) -> Option<&str> {
        match self {
            Self::Failure { error_message } => error_message.as_deref(),
            Self::Success(_) => None,
        }
    }
}

// ---------------------------------------------------------------------------
// StoredFinding
// ---------------------------------------------------------------------------

/// A single finding preserved from reviewer output.
///
/// Stored as-is in review.json without orchestrator modification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredFinding {
    message: String,
    severity: Option<String>,
    file: Option<String>,
    line: Option<u64>,
}

impl StoredFinding {
    /// Creates a new stored finding.
    #[must_use]
    pub fn new(
        message: impl Into<String>,
        severity: Option<String>,
        file: Option<String>,
        line: Option<u64>,
    ) -> Self {
        Self { message: message.into(), severity, file, line }
    }

    /// Returns the finding message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the severity, if any.
    #[must_use]
    pub fn severity(&self) -> Option<&str> {
        self.severity.as_deref()
    }

    /// Returns the file path, if any.
    #[must_use]
    pub fn file(&self) -> Option<&str> {
        self.file.as_deref()
    }

    /// Returns the line number, if any.
    #[must_use]
    pub fn line(&self) -> Option<u64> {
        self.line
    }
}

// ---------------------------------------------------------------------------
// NonEmptyFindings
// ---------------------------------------------------------------------------

/// A non-empty list of stored findings.
///
/// The inner `Vec` is private and can only be constructed through `new()`,
/// which validates non-emptiness. This prevents `FindingsRemain(vec![])`
/// from bypassing the `findings_remain()` constructor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonEmptyFindings(Vec<StoredFinding>);

impl NonEmptyFindings {
    /// Creates a new non-empty findings list.
    ///
    /// # Errors
    /// Returns `CycleError::InconsistentVerdict` if findings is empty.
    pub fn new(findings: Vec<StoredFinding>) -> Result<Self, CycleError> {
        if findings.is_empty() {
            return Err(CycleError::InconsistentVerdict("findings list must not be empty".into()));
        }
        Ok(Self(findings))
    }

    /// Returns the findings as a slice.
    #[must_use]
    pub fn as_slice(&self) -> &[StoredFinding] {
        &self.0
    }

    /// Returns the number of findings.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Always returns `false` — this type is guaranteed non-empty at construction.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        false
    }
}

// ---------------------------------------------------------------------------
// GroupRoundVerdict
// ---------------------------------------------------------------------------

/// Preserved verdict from reviewer output for a group round.
///
/// Modeled as an enum so that `ZeroFindings` structurally cannot carry
/// findings, and `FindingsRemain` structurally guarantees non-empty findings
/// via the `NonEmptyFindings` newtype.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupRoundVerdict {
    /// The reviewer found no issues.
    ZeroFindings,
    /// The reviewer found issues that need to be addressed.
    FindingsRemain(NonEmptyFindings),
}

impl GroupRoundVerdict {
    /// Creates a `FindingsRemain` verdict, validating that findings is non-empty.
    ///
    /// # Errors
    /// Returns `CycleError::InconsistentVerdict` if findings is empty.
    pub fn findings_remain(findings: Vec<StoredFinding>) -> Result<Self, CycleError> {
        Ok(Self::FindingsRemain(NonEmptyFindings::new(findings)?))
    }

    /// Returns the verdict as the domain `Verdict` enum.
    #[must_use]
    pub fn verdict(&self) -> Verdict {
        match self {
            Self::ZeroFindings => Verdict::ZeroFindings,
            Self::FindingsRemain(_) => Verdict::FindingsRemain,
        }
    }

    /// Returns `true` if the verdict is `ZeroFindings`.
    #[must_use]
    pub fn is_zero_findings(&self) -> bool {
        matches!(self, Self::ZeroFindings)
    }

    /// Returns the stored findings (empty slice for `ZeroFindings`).
    #[must_use]
    pub fn findings(&self) -> &[StoredFinding] {
        match self {
            Self::ZeroFindings => &[],
            Self::FindingsRemain(f) => f.as_slice(),
        }
    }
}

// ---------------------------------------------------------------------------
// GroupRound
// ---------------------------------------------------------------------------

/// A single review round recorded for a group within a cycle.
///
/// Rounds are append-only: once recorded, they are never modified or deleted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupRound {
    round_type: RoundType,
    timestamp: Timestamp,
    hash: String,
    outcome: GroupRoundOutcome,
}

impl GroupRound {
    /// Validates a hash string: must be non-empty after trimming and not "PENDING".
    fn validate_hash(hash: impl Into<String>) -> Result<String, CycleError> {
        let h = hash.into().trim().to_owned();
        if h.is_empty() {
            return Err(CycleError::InvalidHash(
                "hash must not be empty or whitespace-only".into(),
            ));
        }
        if h == "PENDING" {
            return Err(CycleError::InvalidHash(
                "hash must not be the reserved literal \"PENDING\"".into(),
            ));
        }
        Ok(h)
    }

    /// Creates a successful round with a verdict.
    ///
    /// # Errors
    /// Returns `CycleError::InvalidHash` if the hash is empty or "PENDING".
    pub fn success(
        round_type: RoundType,
        timestamp: Timestamp,
        hash: impl Into<String>,
        verdict: GroupRoundVerdict,
    ) -> Result<Self, CycleError> {
        Ok(Self {
            round_type,
            timestamp,
            hash: Self::validate_hash(hash)?,
            outcome: GroupRoundOutcome::Success(verdict),
        })
    }

    /// Creates a failed round with an optional error message.
    ///
    /// # Errors
    /// Returns `CycleError::InvalidHash` if the hash is empty or "PENDING".
    pub fn failure(
        round_type: RoundType,
        timestamp: Timestamp,
        hash: impl Into<String>,
        error_message: Option<String>,
    ) -> Result<Self, CycleError> {
        Ok(Self {
            round_type,
            timestamp,
            hash: Self::validate_hash(hash)?,
            outcome: GroupRoundOutcome::Failure { error_message },
        })
    }

    /// Returns the round type (fast or final).
    #[must_use]
    pub fn round_type(&self) -> RoundType {
        self.round_type
    }

    /// Returns a reference to the outcome.
    #[must_use]
    pub fn outcome(&self) -> &GroupRoundOutcome {
        &self.outcome
    }

    /// Returns the timestamp.
    #[must_use]
    pub fn timestamp(&self) -> &Timestamp {
        &self.timestamp
    }

    /// Returns the group-scope hash at the time of this round.
    #[must_use]
    pub fn hash(&self) -> &str {
        &self.hash
    }

    /// Returns `true` if this round was successful with zero findings.
    #[must_use]
    pub fn is_successful_zero_findings(&self) -> bool {
        matches!(self.outcome, GroupRoundOutcome::Success(GroupRoundVerdict::ZeroFindings))
    }
}

// ---------------------------------------------------------------------------
// CycleGroupState
// ---------------------------------------------------------------------------

/// State of a single named group within a review cycle.
///
/// Contains the frozen scope (file paths assigned to this group at cycle start)
/// and the append-only round history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CycleGroupState {
    scope: Vec<String>,
    rounds: Vec<GroupRound>,
}

impl CycleGroupState {
    /// Creates a new group state with the given frozen scope and no rounds.
    #[must_use]
    pub fn new(scope: Vec<String>) -> Self {
        Self { scope, rounds: Vec::new() }
    }

    /// Creates a group state with pre-existing rounds (for deserialization).
    #[must_use]
    pub fn with_rounds(scope: Vec<String>, rounds: Vec<GroupRound>) -> Self {
        Self { scope, rounds }
    }

    /// Returns the frozen scope (file paths assigned to this group).
    #[must_use]
    pub fn scope(&self) -> &[String] {
        &self.scope
    }

    /// Returns the round history.
    #[must_use]
    pub fn rounds(&self) -> &[GroupRound] {
        &self.rounds
    }

    /// Appends a round to the history (append-only).
    pub fn record_round(&mut self, round: GroupRound) {
        self.rounds.push(round);
    }

    /// Returns the latest round of the given type, regardless of outcome.
    ///
    /// Scans backward through round history to find the most recent round
    /// matching the requested type.
    #[must_use]
    pub fn latest_round(&self, round_type: RoundType) -> Option<&GroupRound> {
        self.rounds.iter().rev().find(|r| r.round_type() == round_type)
    }

    /// Returns the latest successful round of the given type, if any.
    ///
    /// Scans backward through round history to find the most recent round
    /// that matches the requested type and has `RoundOutcome::Success`
    /// with `Verdict::ZeroFindings`.
    ///
    /// Note: For approval checks, prefer using `latest_round()` and verifying
    /// the latest round is successful (fail-closed). This method is useful for
    /// staleness checks where the most recent success matters regardless of
    /// later failures.
    #[must_use]
    pub fn latest_successful_round(&self, round_type: RoundType) -> Option<&GroupRound> {
        self.rounds
            .iter()
            .rev()
            .find(|r| r.round_type() == round_type && r.is_successful_zero_findings())
    }

    /// Returns `true` if the latest final round appears after the latest fast
    /// round in the append-only history.
    ///
    /// This ensures that a re-opened fast review (Fast→Final→Fast) is not
    /// falsely treated as approved.
    #[must_use]
    pub fn final_after_latest_fast(&self) -> bool {
        let fast_idx = self.rounds.iter().rposition(|r| r.round_type() == RoundType::Fast);
        let final_idx = self.rounds.iter().rposition(|r| r.round_type() == RoundType::Final);
        match (fast_idx, final_idx) {
            (Some(f), Some(fi)) => fi > f,
            // No fast round → final alone is sufficient (edge case)
            (None, Some(_)) => true,
            _ => false,
        }
    }

    /// Returns the most recently recorded round regardless of type.
    ///
    /// Since rounds are append-only in chronological order, this is simply
    /// the last element.
    #[must_use]
    pub fn latest_round_any(&self) -> Option<&GroupRound> {
        self.rounds.last()
    }

    /// Returns `true` if this group has no rounds.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rounds.is_empty()
    }
}

// ---------------------------------------------------------------------------
// CycleError
// ---------------------------------------------------------------------------

/// Errors specific to cycle construction and operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CycleError {
    /// The mandatory "other" group is missing from the groups map.
    #[error("mandatory 'other' group is missing from cycle groups")]
    MissingOtherGroup,
    /// Verdict and findings list are inconsistent.
    #[error("inconsistent verdict: {0}")]
    InconsistentVerdict(String),
    /// Invalid hash value (empty, whitespace-only, or reserved).
    #[error("invalid hash: {0}")]
    InvalidHash(String),
    /// Unsupported schema version in review.json.
    #[error("unsupported schema version: expected {expected}, got {actual}")]
    UnsupportedSchemaVersion { expected: u32, actual: u32 },
    /// Internal error during cycle operations.
    #[error("cycle internal error: {0}")]
    Internal(String),
}
