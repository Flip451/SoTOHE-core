//! Round-level types for review.json: GroupRound, GroupRoundVerdict, StoredFinding, etc.

use crate::review::concern::ReviewConcern;
use crate::review::types::RoundType;

// ── StoredFinding ────────────────────────────────────────────────────────────

/// A finding stored in review.json, with optional metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredFinding {
    message: String,
    severity: Option<String>,
    file: Option<String>,
    line: Option<u64>,
    category: Option<String>,
}

impl StoredFinding {
    /// Creates a new finding.
    #[must_use]
    pub fn new(
        message: impl Into<String>,
        severity: Option<String>,
        file: Option<String>,
        line: Option<u64>,
    ) -> Self {
        Self { message: message.into(), severity, file, line, category: None }
    }

    /// Sets the category field.
    #[must_use]
    pub fn with_category(mut self, category: Option<String>) -> Self {
        self.category = category;
        self
    }

    /// Returns the message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the severity.
    #[must_use]
    pub fn severity(&self) -> Option<&str> {
        self.severity.as_deref()
    }

    /// Returns the file.
    #[must_use]
    pub fn file(&self) -> Option<&str> {
        self.file.as_deref()
    }

    /// Returns the line number.
    #[must_use]
    pub fn line(&self) -> Option<u64> {
        self.line
    }

    /// Returns the category.
    #[must_use]
    pub fn category(&self) -> Option<&str> {
        self.category.as_deref()
    }
}

// ── NonEmptyFindings ─────────────────────────────────────────────────────────

/// A non-empty collection of stored findings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonEmptyFindings(Vec<StoredFinding>);

impl NonEmptyFindings {
    /// Creates a new non-empty findings collection.
    ///
    /// # Errors
    /// Returns `CycleError::EmptyFindings` if `findings` is empty.
    pub fn new(findings: Vec<StoredFinding>) -> Result<Self, super::CycleError> {
        if findings.is_empty() {
            return Err(super::CycleError::Internal("findings must not be empty".to_owned()));
        }
        Ok(Self(findings))
    }

    /// Returns the findings as a slice.
    #[must_use]
    pub fn as_slice(&self) -> &[StoredFinding] {
        &self.0
    }

    /// Returns the findings as a vec.
    #[must_use]
    pub fn into_vec(self) -> Vec<StoredFinding> {
        self.0
    }
}

// ── GroupRoundVerdict ────────────────────────────────────────────────────────

/// Verdict for a single review round within a group.
///
/// `FindingsRemain` always carries at least one finding (via `NonEmptyFindings`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupRoundVerdict {
    ZeroFindings,
    FindingsRemain(NonEmptyFindings),
}

impl GroupRoundVerdict {
    /// Creates a `FindingsRemain` verdict with non-empty guarantee.
    ///
    /// # Errors
    /// Returns `CycleError` if `findings` is empty.
    pub fn findings_remain(findings: Vec<StoredFinding>) -> Result<Self, super::CycleError> {
        Ok(Self::FindingsRemain(NonEmptyFindings::new(findings)?))
    }

    /// Returns `true` if this is a `ZeroFindings` verdict.
    #[must_use]
    pub fn is_zero_findings(&self) -> bool {
        matches!(self, Self::ZeroFindings)
    }

    /// Returns the findings slice if this is a `FindingsRemain` verdict.
    #[must_use]
    pub fn findings(&self) -> &[StoredFinding] {
        match self {
            Self::ZeroFindings => &[],
            Self::FindingsRemain(f) => f.as_slice(),
        }
    }
}

// ── GroupRoundOutcome ────────────────────────────────────────────────────────

/// Outcome of a group review round (success with verdict, or failure).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupRoundOutcome {
    Success(GroupRoundVerdict),
    Failure { error_message: Option<String> },
}

impl GroupRoundOutcome {
    /// Returns the verdict if this is a successful outcome.
    #[must_use]
    pub fn verdict(&self) -> Option<&GroupRoundVerdict> {
        match self {
            Self::Success(v) => Some(v),
            Self::Failure { .. } => None,
        }
    }

    /// Returns the error message if this is a failure outcome.
    #[must_use]
    pub fn error_message(&self) -> Option<&str> {
        match self {
            Self::Success(_) => None,
            Self::Failure { error_message } => error_message.as_deref(),
        }
    }
}

// ── GroupRound ───────────────────────────────────────────────────────────────

/// A single review round for a group, with metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupRound {
    round_type: RoundType,
    timestamp: crate::Timestamp,
    hash: String,
    outcome: GroupRoundOutcome,
    concerns: Vec<ReviewConcern>,
}

impl GroupRound {
    /// Creates a successful round.
    ///
    /// # Errors
    /// Returns `CycleError` if the hash is empty.
    pub fn success(
        round_type: RoundType,
        timestamp: crate::Timestamp,
        hash: impl Into<String>,
        verdict: GroupRoundVerdict,
    ) -> Result<Self, super::CycleError> {
        let hash = hash.into();
        if hash.trim().is_empty() {
            return Err(super::CycleError::Internal("round hash must not be empty".to_owned()));
        }
        Ok(Self {
            round_type,
            timestamp,
            hash,
            outcome: GroupRoundOutcome::Success(verdict),
            concerns: Vec::new(),
        })
    }

    /// Creates a failure round.
    ///
    /// # Errors
    /// Returns `CycleError` if the hash is empty.
    pub fn failure(
        round_type: RoundType,
        timestamp: crate::Timestamp,
        hash: impl Into<String>,
        error_message: Option<String>,
    ) -> Result<Self, super::CycleError> {
        let hash = hash.into();
        if hash.trim().is_empty() {
            return Err(super::CycleError::Internal("round hash must not be empty".to_owned()));
        }
        Ok(Self {
            round_type,
            timestamp,
            hash,
            outcome: GroupRoundOutcome::Failure { error_message },
            concerns: Vec::new(),
        })
    }

    /// Sets concerns on the round (builder pattern).
    #[must_use]
    pub fn with_concerns(mut self, concerns: Vec<ReviewConcern>) -> Self {
        self.concerns = concerns;
        self
    }

    /// Returns the round type.
    #[must_use]
    pub fn round_type(&self) -> RoundType {
        self.round_type
    }

    /// Returns the timestamp.
    #[must_use]
    pub fn timestamp(&self) -> &crate::Timestamp {
        &self.timestamp
    }

    /// Returns the hash.
    #[must_use]
    pub fn hash(&self) -> &str {
        &self.hash
    }

    /// Returns the outcome.
    #[must_use]
    pub fn outcome(&self) -> &GroupRoundOutcome {
        &self.outcome
    }

    /// Returns the concerns.
    #[must_use]
    pub fn concerns(&self) -> &[ReviewConcern] {
        &self.concerns
    }

    /// Returns `true` if this round succeeded with a `ZeroFindings` verdict.
    #[must_use]
    pub fn is_successful_zero_findings(&self) -> bool {
        matches!(&self.outcome, GroupRoundOutcome::Success(GroupRoundVerdict::ZeroFindings))
    }
}
