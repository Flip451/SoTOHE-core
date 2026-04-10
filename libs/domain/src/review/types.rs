//! Core review type definitions: Verdict, CodeHash, ReviewStatus, RoundType,
//! ReviewRoundResult, ReviewGroupState, and pure logic helpers.

use super::error::ReviewError;

/// Review round verdict.
///
/// Only two outcomes exist: zero findings or findings remain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, strum::EnumString)]
#[strum(serialize_all = "snake_case")]
pub enum Verdict {
    /// The reviewer found no issues.
    ZeroFindings,
    /// The reviewer found issues that need to be addressed.
    FindingsRemain,
}

impl Verdict {
    /// Parses a verdict from its string representation.
    ///
    /// # Errors
    /// Returns `ReviewError::InvalidConcern` if the string is not a recognized verdict.
    pub fn parse(s: &str) -> Result<Self, ReviewError> {
        s.parse().map_err(|_| ReviewError::InvalidConcern(format!("unknown verdict: {s}")))
    }

    /// Returns `true` if the verdict is `ZeroFindings`.
    #[must_use]
    pub fn is_zero_findings(self) -> bool {
        self == Self::ZeroFindings
    }
}

/// A validated git commit SHA (40-character hexadecimal string).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovedHead(String);

impl ApprovedHead {
    /// Creates a new `ApprovedHead` from a 40-character hex string.
    ///
    /// # Errors
    /// Returns `ReviewError::InvalidConcern` if validation fails.
    pub fn try_new(sha: impl Into<String>) -> Result<Self, ReviewError> {
        let s = sha.into();
        if s.len() != 40 || !s.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(ReviewError::InvalidConcern(format!(
                "approved_head must be a 40-character hex SHA, got: {s}"
            )));
        }
        Ok(Self(s.to_ascii_lowercase()))
    }

    /// Returns the SHA string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ApprovedHead {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Code hash state for review freshness tracking.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum CodeHash {
    /// No review round has been recorded yet.
    #[default]
    NotRecorded,
    /// Hash computation is pending.
    Pending,
    /// A computed, non-empty hash string.
    Computed(String),
}

impl CodeHash {
    /// Creates a `Computed` variant, validating that the string is non-empty.
    ///
    /// # Errors
    /// Returns `ReviewError::InvalidConcern` if the value is empty.
    pub fn computed(value: impl Into<String>) -> Result<Self, ReviewError> {
        let trimmed = value.into().trim().to_owned();
        if trimmed.is_empty() {
            return Err(ReviewError::InvalidConcern(
                "code hash must not be empty or whitespace-only".to_owned(),
            ));
        }
        if trimmed == "PENDING" {
            return Err(ReviewError::InvalidConcern(
                "code hash must not be the reserved literal \"PENDING\"".to_owned(),
            ));
        }
        Ok(Self::Computed(trimmed))
    }

    /// Returns the hash string if this is a `Computed` variant.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Computed(s) => Some(s),
            Self::NotRecorded | Self::Pending => None,
        }
    }

    /// Returns `true` if this is the `Pending` variant.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        matches!(self, Self::Pending)
    }

    /// Returns `true` if this is the `NotRecorded` variant.
    #[must_use]
    pub fn is_not_recorded(&self) -> bool {
        matches!(self, Self::NotRecorded)
    }
}

/// Review status enum with explicit states (no null).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, strum::Display, strum::EnumString)]
#[strum(serialize_all = "snake_case")]
pub enum ReviewStatus {
    #[default]
    NotStarted,
    Invalidated,
    FastPassed,
    Approved,
}

/// Round type discriminant for review rounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, strum::EnumString)]
#[strum(serialize_all = "snake_case")]
pub enum RoundType {
    Fast,
    Final,
}

/// Result of a single review round for a group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewRoundResult {
    round: u32,
    verdict: Verdict,
    timestamp: crate::Timestamp,
    concerns: Vec<super::concern::ReviewConcern>,
}

impl ReviewRoundResult {
    /// Creates a new `ReviewRoundResult` with empty concerns.
    #[must_use]
    pub fn new(round: u32, verdict: Verdict, timestamp: crate::Timestamp) -> Self {
        Self { round, verdict, timestamp, concerns: Vec::new() }
    }

    /// Creates a new `ReviewRoundResult` with associated concerns for escalation tracking.
    #[must_use]
    pub fn new_with_concerns(
        round: u32,
        verdict: Verdict,
        timestamp: crate::Timestamp,
        concerns: Vec<super::concern::ReviewConcern>,
    ) -> Self {
        Self { round, verdict, timestamp, concerns }
    }

    /// Returns the round number.
    #[must_use]
    pub fn round(&self) -> u32 {
        self.round
    }

    /// Returns the verdict for this round result.
    #[must_use]
    pub fn verdict(&self) -> Verdict {
        self.verdict
    }

    /// Returns the timestamp string for this result.
    #[must_use]
    pub fn timestamp(&self) -> &str {
        self.timestamp.as_str()
    }

    /// Returns the `Timestamp` value for this result.
    #[must_use]
    pub fn timestamp_value(&self) -> &crate::Timestamp {
        &self.timestamp
    }

    /// Returns the concerns associated with this round result.
    #[must_use]
    pub fn concerns(&self) -> &[super::concern::ReviewConcern] {
        &self.concerns
    }
}

/// Progress state of a named review group as an ADT.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ReviewGroupState {
    /// No rounds recorded yet.
    #[default]
    NoRounds,
    /// Only a fast round has been recorded.
    FastOnly(ReviewRoundResult),
    /// Only a final round exists (legacy/backward-compat data).
    FinalOnly(ReviewRoundResult),
    /// Both fast and final rounds have been recorded.
    BothRounds { fast: ReviewRoundResult, final_round: ReviewRoundResult },
}

impl ReviewGroupState {
    /// Returns the fast round result, if present.
    #[must_use]
    pub fn fast(&self) -> Option<&ReviewRoundResult> {
        match self {
            Self::FastOnly(r) | Self::BothRounds { fast: r, .. } => Some(r),
            Self::NoRounds | Self::FinalOnly(_) => None,
        }
    }

    /// Returns the final round result, if present.
    #[must_use]
    pub fn final_round(&self) -> Option<&ReviewRoundResult> {
        match self {
            Self::FinalOnly(r) | Self::BothRounds { final_round: r, .. } => Some(r),
            Self::NoRounds | Self::FastOnly(_) => None,
        }
    }

    /// Creates a group state with only a fast round result.
    #[must_use]
    pub fn with_fast(result: ReviewRoundResult) -> Self {
        Self::FastOnly(result)
    }

    /// Creates a group state with only a final round result (legacy data).
    #[must_use]
    pub fn with_final_only(result: ReviewRoundResult) -> Self {
        Self::FinalOnly(result)
    }

    /// Creates a group state with both fast and final round results.
    #[must_use]
    pub fn with_both(fast: ReviewRoundResult, final_round: ReviewRoundResult) -> Self {
        Self::BothRounds { fast, final_round }
    }

    /// Records a fast round result, clearing any stale final round.
    pub fn record_fast(&mut self, result: ReviewRoundResult) {
        *self = Self::FastOnly(result);
    }

    /// Records a final round result, preserving the existing fast round if present.
    pub fn record_final(&mut self, result: ReviewRoundResult) {
        *self = match std::mem::take(self) {
            Self::FastOnly(fast) | Self::BothRounds { fast, .. } => {
                Self::BothRounds { fast, final_round: result }
            }
            Self::NoRounds | Self::FinalOnly(_) => Self::FinalOnly(result),
        };
    }
}

/// Scans content bottom-up for single-line compact JSON candidates.
#[must_use]
pub fn extract_verdict_json_candidates_compact(content: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    for line in content.lines().rev() {
        let trimmed = line.trim();
        if trimmed.starts_with('{')
            && trimmed.contains("\"verdict\"")
            && trimmed.contains("\"findings\"")
        {
            candidates.push(trimmed.to_owned());
        }
    }
    candidates
}

/// Scans content bottom-up for multi-line pretty-printed JSON candidates.
#[must_use]
pub fn extract_verdict_json_candidates_multiline(content: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    let bytes = content.as_bytes();
    let mut end = bytes.len();
    while let Some(close) = content.get(..end).and_then(|s| s.rfind('}')) {
        let mut depth = 0i32;
        let mut start = None;
        for (i, &b) in bytes.get(..=close).iter().flat_map(|s| s.iter().enumerate().rev()) {
            match b {
                b'}' => depth += 1,
                b'{' => {
                    depth -= 1;
                    if depth == 0 {
                        start = Some(i);
                        break;
                    }
                }
                _ => {}
            }
        }
        if let Some(start) = start {
            if let Some(block) = content.get(start..=close) {
                if block.contains("\"verdict\"") && block.contains("\"findings\"") {
                    candidates.push(block.to_owned());
                }
            }
        }
        end = close;
    }
    candidates
}
