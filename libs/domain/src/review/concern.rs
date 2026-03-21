//! Review concern types: ReviewConcern, ReviewCycleSummary, ReviewConcernStreak,
//! and pure concern-slug helpers.

use super::error::ReviewError;
use super::types::RoundType;
use crate::{ReviewGroupName, Timestamp};

fn validate_review_concern(value: &str) -> Result<(), ReviewError> {
    if value.is_empty() {
        Err(ReviewError::InvalidConcern("concern must not be empty or whitespace-only".to_owned()))
    } else {
        Ok(())
    }
}

/// A normalized, non-empty concern identifier used for review escalation tracking.
///
/// Concerns are lowercase-trimmed strings that enable consistent dedup and sort.
///
/// # Errors
///
/// Returns `ReviewError::InvalidConcern` if the value is empty after trimming.
#[nutype::nutype(
    sanitize(with = |s: String| s.trim().to_lowercase()),
    validate(with = validate_review_concern, error = ReviewError),
    derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, AsRef)
)]
pub struct ReviewConcern(String);

/// Summary of a closed review cycle for escalation tracking.
///
/// A cycle closes when all `expected_groups` have recorded the same `round`
/// for the given `round_type`. Stored in `ReviewEscalationState::recent_cycles`
/// (FIFO trim at 10 entries).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewCycleSummary {
    round_type: RoundType,
    round: u32,
    timestamp: Timestamp,
    concerns: Vec<ReviewConcern>,
    groups: Vec<ReviewGroupName>,
}

impl ReviewCycleSummary {
    /// Creates a new `ReviewCycleSummary`.
    #[must_use]
    pub fn new(
        round_type: RoundType,
        round: u32,
        timestamp: Timestamp,
        concerns: Vec<ReviewConcern>,
        groups: Vec<ReviewGroupName>,
    ) -> Self {
        Self { round_type, round, timestamp, concerns, groups }
    }

    /// Returns the round type for this cycle.
    #[must_use]
    pub fn round_type(&self) -> RoundType {
        self.round_type
    }

    /// Returns the round number for this cycle.
    #[must_use]
    pub fn round(&self) -> u32 {
        self.round
    }

    /// Returns the timestamp string for this cycle.
    #[must_use]
    pub fn timestamp(&self) -> &str {
        self.timestamp.as_str()
    }

    /// Returns the concerns raised in this cycle.
    #[must_use]
    pub fn concerns(&self) -> &[ReviewConcern] {
        &self.concerns
    }

    /// Returns the groups that participated in this cycle.
    #[must_use]
    pub fn groups(&self) -> &[ReviewGroupName] {
        &self.groups
    }
}

/// Tracks consecutive rounds a concern has appeared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewConcernStreak {
    consecutive_rounds: u8,
    last_round_type: RoundType,
    last_round: u32,
    last_seen_at: Timestamp,
}

impl ReviewConcernStreak {
    /// Creates a new `ReviewConcernStreak`.
    #[must_use]
    pub fn new(
        consecutive_rounds: u8,
        last_round_type: RoundType,
        last_round: u32,
        last_seen_at: Timestamp,
    ) -> Self {
        Self { consecutive_rounds, last_round_type, last_round, last_seen_at }
    }

    /// Returns the number of consecutive rounds this concern has appeared.
    #[must_use]
    pub fn consecutive_rounds(&self) -> u8 {
        self.consecutive_rounds
    }

    /// Returns the round type for the last occurrence.
    #[must_use]
    pub fn last_round_type(&self) -> RoundType {
        self.last_round_type
    }

    /// Returns the round number for the last occurrence.
    #[must_use]
    pub fn last_round(&self) -> u32 {
        self.last_round
    }

    /// Returns the timestamp string of the last occurrence.
    #[must_use]
    pub fn last_seen_at(&self) -> &str {
        self.last_seen_at.as_str()
    }
}

/// Converts a file path to a concern slug.
///
/// # Examples
///
/// ```
/// // "libs/domain/src/guard/parser.rs" → "domain.guard.parser"
/// // "apps/cli/src/commands/review.rs" → "cli.commands.review"
/// ```
#[must_use]
pub fn file_path_to_concern(path: &str) -> String {
    // Handle absolute paths: find "libs/" or "apps/" anywhere in the path
    let path = if let Some(idx) = path.find("libs/") {
        &path[idx..]
    } else if let Some(idx) = path.find("apps/") {
        &path[idx..]
    } else {
        path
    };
    // Strip known workspace prefixes
    let path = path.trim_start_matches("libs/").trim_start_matches("apps/");
    // Strip .rs extension
    let path = path.trim_end_matches(".rs");
    // Replace "/src/" segments with "."
    let path = path.replace("/src/", ".");
    // Replace remaining "/" with "."
    path.replace('/', ".")
}
