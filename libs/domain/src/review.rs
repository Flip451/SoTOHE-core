//! Review state management for track-level review workflows.
//!
//! Tracks review progress through a state machine:
//! `NotStarted` → `FastPassed` → `Approved`, with `Invalidated` on code changes.

use std::collections::{BTreeMap, HashMap};

use thiserror::Error;

use crate::Timestamp;

/// Errors from review state operations.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ReviewError {
    #[error("final round requires review status fast_passed, but current status is {0}")]
    FinalRequiresFastPassed(ReviewStatus),

    #[error("code hash mismatch: review recorded against {expected}, but current code is {actual}")]
    StaleCodeHash { expected: String, actual: String },

    #[error("review status is {0}, not approved")]
    NotApproved(ReviewStatus),

    #[error("invalid concern: {0}")]
    InvalidConcern(String),

    #[error("review escalation is active for concerns: {concerns:?}")]
    EscalationActive { concerns: Vec<String> },

    #[error("review escalation is not active")]
    EscalationNotActive,

    #[error("resolution evidence is required: {0}")]
    ResolutionEvidenceMissing(&'static str),

    #[error("resolution concerns do not match blocked concerns")]
    ResolutionConcernMismatch { expected: Vec<String>, actual: Vec<String> },
}

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
    groups: Vec<String>,
}

impl ReviewCycleSummary {
    /// Creates a new `ReviewCycleSummary`.
    #[must_use]
    pub fn new(
        round_type: RoundType,
        round: u32,
        timestamp: Timestamp,
        concerns: Vec<ReviewConcern>,
        groups: Vec<String>,
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
    pub fn groups(&self) -> &[String] {
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

/// Details of an escalation block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewEscalationBlock {
    concerns: Vec<ReviewConcern>,
    blocked_at: Timestamp,
}

impl ReviewEscalationBlock {
    /// Creates a new `ReviewEscalationBlock`.
    #[must_use]
    pub fn new(concerns: Vec<ReviewConcern>, blocked_at: Timestamp) -> Self {
        Self { concerns, blocked_at }
    }

    /// Returns the concerns that triggered the escalation block.
    #[must_use]
    pub fn concerns(&self) -> &[ReviewConcern] {
        &self.concerns
    }

    /// Returns the timestamp string of when the block was set.
    #[must_use]
    pub fn blocked_at(&self) -> &str {
        self.blocked_at.as_str()
    }
}

/// Decision made during escalation resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewEscalationDecision {
    /// Adopt a solution already present in the workspace.
    AdoptWorkspaceSolution,
    /// Adopt an external crate to solve the concern.
    AdoptExternalCrate,
    /// Continue with the current self-implementation approach.
    ContinueSelfImplementation,
}

/// Evidence and decision for resolving an escalation block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewEscalationResolution {
    blocked_concerns: Vec<ReviewConcern>,
    workspace_search_ref: String,
    reinvention_check_ref: String,
    decision: ReviewEscalationDecision,
    summary: String,
    resolved_at: Timestamp,
}

impl ReviewEscalationResolution {
    /// Creates a new `ReviewEscalationResolution`.
    #[must_use]
    pub fn new(
        blocked_concerns: Vec<ReviewConcern>,
        workspace_search_ref: impl Into<String>,
        reinvention_check_ref: impl Into<String>,
        decision: ReviewEscalationDecision,
        summary: impl Into<String>,
        resolved_at: Timestamp,
    ) -> Self {
        Self {
            blocked_concerns,
            workspace_search_ref: workspace_search_ref.into(),
            reinvention_check_ref: reinvention_check_ref.into(),
            decision,
            summary: summary.into(),
            resolved_at,
        }
    }

    /// Returns the concerns that were blocked at the time of resolution.
    #[must_use]
    pub fn blocked_concerns(&self) -> &[ReviewConcern] {
        &self.blocked_concerns
    }

    /// Returns the reference path to the workspace search artifact.
    #[must_use]
    pub fn workspace_search_ref(&self) -> &str {
        &self.workspace_search_ref
    }

    /// Returns the reference path to the reinvention-check artifact.
    #[must_use]
    pub fn reinvention_check_ref(&self) -> &str {
        &self.reinvention_check_ref
    }

    /// Returns the decision made during resolution.
    #[must_use]
    pub fn decision(&self) -> ReviewEscalationDecision {
        self.decision
    }

    /// Returns the human-readable summary of the resolution.
    #[must_use]
    pub fn summary(&self) -> &str {
        &self.summary
    }

    /// Returns the timestamp string of when the resolution was recorded.
    #[must_use]
    pub fn resolved_at(&self) -> &str {
        self.resolved_at.as_str()
    }
}

/// ADT representing the escalation phase.
///
/// `Clear` has no associated data. `Blocked` carries the block details directly,
/// making illegal states (e.g., "blocked but with no block data") unrepresentable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EscalationPhase {
    /// No active escalation block.
    Clear,
    /// Escalation is active; subsequent review operations are rejected.
    Blocked(ReviewEscalationBlock),
}

/// Aggregate escalation state composed into `ReviewState`.
///
/// Tracks streaks per concern across closed review cycles and transitions
/// to `EscalationPhase::Blocked` when a concern reaches `threshold` consecutive cycles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewEscalationState {
    threshold: u8,
    phase: EscalationPhase,
    recent_cycles: Vec<ReviewCycleSummary>,
    concern_streaks: BTreeMap<ReviewConcern, ReviewConcernStreak>,
    last_resolution: Option<ReviewEscalationResolution>,
}

impl ReviewEscalationState {
    /// Creates a new `ReviewEscalationState` with default values.
    ///
    /// Threshold is 3, phase is `Clear`, no cycles or streaks recorded.
    #[must_use]
    pub fn new() -> Self {
        Self {
            threshold: 3,
            phase: EscalationPhase::Clear,
            recent_cycles: Vec::new(),
            concern_streaks: BTreeMap::new(),
            last_resolution: None,
        }
    }

    /// Creates a `ReviewEscalationState` with all fields set explicitly.
    ///
    /// Used by codec deserialization.
    #[must_use]
    pub fn with_fields(
        threshold: u8,
        phase: EscalationPhase,
        recent_cycles: Vec<ReviewCycleSummary>,
        concern_streaks: BTreeMap<ReviewConcern, ReviewConcernStreak>,
        last_resolution: Option<ReviewEscalationResolution>,
    ) -> Self {
        Self { threshold, phase, recent_cycles, concern_streaks, last_resolution }
    }

    /// Returns the escalation threshold (number of consecutive cycles before blocking).
    #[must_use]
    pub fn threshold(&self) -> u8 {
        self.threshold
    }

    /// Returns the current escalation phase.
    #[must_use]
    pub fn phase(&self) -> &EscalationPhase {
        &self.phase
    }

    /// Returns the recent closed review cycle summaries (up to 10).
    #[must_use]
    pub fn recent_cycles(&self) -> &[ReviewCycleSummary] {
        &self.recent_cycles
    }

    /// Returns the per-concern streak tracking map.
    #[must_use]
    pub fn concern_streaks(&self) -> &BTreeMap<ReviewConcern, ReviewConcernStreak> {
        &self.concern_streaks
    }

    /// Returns the last escalation resolution record, if any.
    #[must_use]
    pub fn last_resolution(&self) -> Option<&ReviewEscalationResolution> {
        self.last_resolution.as_ref()
    }

    /// Returns `true` if the escalation phase is `Blocked`.
    #[must_use]
    pub fn is_blocked(&self) -> bool {
        matches!(self.phase, EscalationPhase::Blocked(_))
    }
}

impl Default for ReviewEscalationState {
    fn default() -> Self {
        Self::new()
    }
}

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

/// Code hash state for review freshness tracking.
///
/// Replaces `Option<String>` with `"PENDING"` sentinel.
/// `Pending` means a round was recorded but the final hash hasn't been written back yet.
/// `Computed` holds the actual hash (validated non-empty).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodeHash {
    /// Hash computation is pending (two-phase protocol intermediate state).
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
    ///
    /// Returns `None` for the `Pending` variant.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Computed(s) => Some(s),
            Self::Pending => None,
        }
    }

    /// Returns `true` if this is the `Pending` variant.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        matches!(self, Self::Pending)
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
    timestamp: Timestamp,
    concerns: Vec<ReviewConcern>,
}

impl ReviewRoundResult {
    /// Creates a new `ReviewRoundResult` with empty concerns.
    #[must_use]
    pub fn new(round: u32, verdict: Verdict, timestamp: Timestamp) -> Self {
        Self { round, verdict, timestamp, concerns: Vec::new() }
    }

    /// Creates a new `ReviewRoundResult` with associated concerns for escalation tracking.
    #[must_use]
    pub fn new_with_concerns(
        round: u32,
        verdict: Verdict,
        timestamp: Timestamp,
        concerns: Vec<ReviewConcern>,
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
    pub fn timestamp_value(&self) -> &Timestamp {
        &self.timestamp
    }

    /// Returns the concerns associated with this round result.
    #[must_use]
    pub fn concerns(&self) -> &[ReviewConcern] {
        &self.concerns
    }
}

/// State of a named review group, tracking fast and final round results.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReviewGroupState {
    fast: Option<ReviewRoundResult>,
    final_round: Option<ReviewRoundResult>,
}

impl ReviewGroupState {
    #[must_use]
    pub fn fast(&self) -> Option<&ReviewRoundResult> {
        self.fast.as_ref()
    }

    #[must_use]
    pub fn final_round(&self) -> Option<&ReviewRoundResult> {
        self.final_round.as_ref()
    }

    /// Creates a group state with only a fast round result.
    #[must_use]
    pub fn with_fast(result: ReviewRoundResult) -> Self {
        Self { fast: Some(result), final_round: None }
    }

    /// Creates a group state with only a final round result.
    #[must_use]
    pub fn with_final_only(result: ReviewRoundResult) -> Self {
        Self { fast: None, final_round: Some(result) }
    }

    /// Creates a group state with both fast and final round results.
    #[must_use]
    pub fn with_both(fast: ReviewRoundResult, final_round: ReviewRoundResult) -> Self {
        Self { fast: Some(fast), final_round: Some(final_round) }
    }
}

/// Aggregate review state for a track.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewState {
    status: ReviewStatus,
    code_hash: Option<CodeHash>,
    groups: HashMap<String, ReviewGroupState>,
    escalation: ReviewEscalationState,
}

impl Default for ReviewState {
    fn default() -> Self {
        Self::new()
    }
}

impl ReviewState {
    /// Creates a new review state in `NotStarted` status.
    #[must_use]
    pub fn new() -> Self {
        Self {
            status: ReviewStatus::NotStarted,
            code_hash: None,
            groups: HashMap::new(),
            escalation: ReviewEscalationState::new(),
        }
    }

    /// Creates a review state with pre-set fields (used by codec deserialization).
    #[must_use]
    pub fn with_fields(
        status: ReviewStatus,
        code_hash: Option<CodeHash>,
        groups: HashMap<String, ReviewGroupState>,
        escalation: ReviewEscalationState,
    ) -> Self {
        Self { status, code_hash, groups, escalation }
    }

    /// Returns the current review status.
    #[must_use]
    pub fn status(&self) -> ReviewStatus {
        self.status
    }

    /// Returns the stored code hash string, if any.
    ///
    /// Returns `None` when there is no hash or when the hash is in `Pending` state.
    #[must_use]
    pub fn code_hash(&self) -> Option<&str> {
        self.code_hash.as_ref().and_then(|ch| ch.as_str())
    }

    /// Returns the code hash as a string suitable for serialization.
    ///
    /// - `None` → `None`
    /// - `Some(Pending)` → `Some("PENDING")`
    /// - `Some(Computed(s))` → `Some(s)`
    #[must_use]
    pub fn code_hash_for_serialization(&self) -> Option<&str> {
        match &self.code_hash {
            None => None,
            Some(CodeHash::Pending) => Some("PENDING"),
            Some(CodeHash::Computed(s)) => Some(s),
        }
    }

    /// Returns the map of review group states.
    #[must_use]
    pub fn groups(&self) -> &HashMap<String, ReviewGroupState> {
        &self.groups
    }

    /// Returns the escalation state.
    #[must_use]
    pub fn escalation(&self) -> &ReviewEscalationState {
        &self.escalation
    }

    /// Returns a mutable reference to the escalation state.
    pub fn escalation_mut(&mut self) -> &mut ReviewEscalationState {
        &mut self.escalation
    }

    /// Records a review round result for a group.
    ///
    /// Validates escalation block, code hash freshness, and sequential escalation
    /// (fast before final). Promotes status when all expected groups report `zero_findings`.
    ///
    /// # Errors
    ///
    /// - `ReviewError::EscalationActive` if escalation is blocked. Short-circuits before all
    ///   other checks.
    /// - `ReviewError::InvalidConcern` if verdict/concerns are inconsistent:
    ///   `zero_findings` with non-empty concerns, or `findings_remain` with empty concerns.
    /// - `ReviewError::StaleCodeHash` if stored code_hash doesn't match `current_code_hash`.
    ///   Sets status to `Invalidated` as a side effect.
    /// - `ReviewError::FinalRequiresFastPassed` if round_type is `Final` but status is not
    ///   `FastPassed`.
    pub fn record_round(
        &mut self,
        round_type: RoundType,
        group: &str,
        result: ReviewRoundResult,
        expected_groups: &[String],
        current_code_hash: &str,
    ) -> Result<(), ReviewError> {
        // 0. Escalation block check (short-circuit before all other checks).
        if let EscalationPhase::Blocked(ref block) = self.escalation.phase {
            return Err(ReviewError::EscalationActive {
                concerns: block.concerns.iter().map(|c| c.as_ref().to_owned()).collect(),
            });
        }

        // Deduplicate expected_groups to prevent one result satisfying multiple slots.
        let expected_groups: Vec<String> = {
            let mut set = std::collections::BTreeSet::new();
            for g in expected_groups {
                set.insert(g.clone());
            }
            set.into_iter().collect()
        };
        let expected_groups = expected_groups.as_slice();

        // 0b. Verdict/concerns consistency check.
        Self::validate_verdict_concerns(&result)?;

        // 1. Code hash freshness check (applies to all round types).
        // Clear code_hash on mismatch so a subsequent call with the new hash succeeds.
        if let Some(stored) = self.code_hash.take() {
            match &stored {
                CodeHash::Pending => {
                    // Two-phase protocol hasn't completed. Block like the old "PENDING" string.
                    self.status = ReviewStatus::Invalidated;
                    return Err(ReviewError::StaleCodeHash {
                        expected: "PENDING".to_owned(),
                        actual: current_code_hash.to_owned(),
                    });
                }
                CodeHash::Computed(stored_str) => {
                    if stored_str != current_code_hash {
                        self.status = ReviewStatus::Invalidated;
                        return Err(ReviewError::StaleCodeHash {
                            expected: stored_str.clone(),
                            actual: current_code_hash.to_owned(),
                        });
                    }
                }
            }
            // Restore hash if it matched
            self.code_hash = Some(stored);
        }

        // 2. Sequential escalation check (final requires fast_passed or approved)
        if round_type == RoundType::Final
            && self.status != ReviewStatus::FastPassed
            && self.status != ReviewStatus::Approved
        {
            return Err(ReviewError::FinalRequiresFastPassed(self.status));
        }

        // 3. Set/confirm code_hash (validated via computed())
        self.code_hash = Some(
            CodeHash::computed(current_code_hash)
                .map_err(|e| ReviewError::InvalidConcern(format!("invalid code hash: {e}")))?,
        );

        // Save timestamp before result is moved into group state.
        let timestamp = result.timestamp_value().clone();

        // 4. Record round result for the group.
        // When recording a fast round, clear any stale final_round for this group
        // since a new fast cycle invalidates previous final approvals.
        let group_state = self.groups.entry(group.to_owned()).or_default();
        match round_type {
            RoundType::Fast => {
                group_state.fast = Some(result);
                group_state.final_round = None;
            }
            RoundType::Final => group_state.final_round = Some(result),
        }

        // 5. Check promotion/demotion based on aggregated verdicts
        let all_expected_zero = expected_groups.iter().all(|eg| {
            self.groups.get(eg).is_some_and(|gs| {
                let round_result = match round_type {
                    RoundType::Fast => gs.fast.as_ref(),
                    RoundType::Final => gs.final_round.as_ref(),
                };
                round_result.is_some_and(|r| r.verdict.is_zero_findings())
            })
        });

        if all_expected_zero {
            match round_type {
                RoundType::Fast => self.status = ReviewStatus::FastPassed,
                RoundType::Final => self.status = ReviewStatus::Approved,
            }
        } else {
            // Demote if current status is higher than what this round type warrants.
            // A fast round with findings should not leave status at FastPassed or Approved.
            // A final round with findings should not leave status at Approved.
            match round_type {
                RoundType::Fast => {
                    if self.status == ReviewStatus::FastPassed
                        || self.status == ReviewStatus::Approved
                    {
                        self.status = ReviewStatus::NotStarted;
                    }
                }
                RoundType::Final => {
                    if self.status == ReviewStatus::Approved {
                        self.status = ReviewStatus::FastPassed;
                    }
                }
            }
        }

        // 6. Update escalation state after recording.
        self.update_escalation_after_record(round_type, expected_groups, &timestamp);

        Ok(())
    }

    /// Records a review round with code_hash set to "PENDING" sentinel.
    ///
    /// Used in the normalized hash protocol (method D):
    /// 1. Caller computes pre-update normalized hash
    /// 2. This method: freshness check → record round → set code_hash to "PENDING"
    /// 3. Caller re-stages, computes post-update normalized hash H1
    /// 4. Caller calls set_code_hash(H1) to write back the real hash
    ///
    /// # Errors
    ///
    /// - `ReviewError::EscalationActive` if escalation is blocked. Short-circuits before all
    ///   other checks.
    /// - `ReviewError::StaleCodeHash` if stored code_hash doesn't match `pre_update_hash`.
    ///   Skipped when stored code_hash is None (first round).
    /// - `ReviewError::FinalRequiresFastPassed` if round_type is Final but status is not
    ///   FastPassed/Approved.
    pub fn record_round_with_pending(
        &mut self,
        round_type: RoundType,
        group: &str,
        result: ReviewRoundResult,
        expected_groups: &[String],
        pre_update_hash: &str,
    ) -> Result<(), ReviewError> {
        // 0. Escalation block check (short-circuit before all other checks).
        if let EscalationPhase::Blocked(ref block) = self.escalation.phase {
            return Err(ReviewError::EscalationActive {
                concerns: block.concerns.iter().map(|c| c.as_ref().to_owned()).collect(),
            });
        }

        // Deduplicate expected_groups to prevent one result satisfying multiple slots.
        let expected_groups: Vec<String> = {
            let mut set = std::collections::BTreeSet::new();
            for g in expected_groups {
                set.insert(g.clone());
            }
            set.into_iter().collect()
        };
        let expected_groups = expected_groups.as_slice();

        // 0b. Verdict/concerns consistency check.
        Self::validate_verdict_concerns(&result)?;

        // 1. Code hash freshness check — skip if None (first round).
        let taken_hash = self.code_hash.take();
        if let Some(ref stored) = taken_hash {
            match stored {
                CodeHash::Pending => {
                    // Previous two-phase protocol hasn't completed.
                    // Block: the old "PENDING" string always mismatched, so preserve that behavior.
                    self.status = ReviewStatus::Invalidated;
                    return Err(ReviewError::StaleCodeHash {
                        expected: "PENDING".to_owned(),
                        actual: pre_update_hash.to_owned(),
                    });
                }
                CodeHash::Computed(stored_str) => {
                    if stored_str != pre_update_hash {
                        self.status = ReviewStatus::Invalidated;
                        return Err(ReviewError::StaleCodeHash {
                            expected: stored_str.clone(),
                            actual: pre_update_hash.to_owned(),
                        });
                    }
                }
            }
            // hash matched — code_hash cleared by take(); will be set to PENDING below
        }

        // 2. Sequential escalation check (final requires fast_passed or approved).
        // Restore code_hash on failure so the next retry still has a valid
        // freshness baseline rather than skipping the check as if first-round.
        if round_type == RoundType::Final
            && self.status != ReviewStatus::FastPassed
            && self.status != ReviewStatus::Approved
        {
            self.code_hash = taken_hash;
            return Err(ReviewError::FinalRequiresFastPassed(self.status));
        }

        // 3. Set code_hash to the PENDING sentinel
        self.code_hash = Some(CodeHash::Pending);

        // Save timestamp before result is moved into group state.
        let timestamp = result.timestamp_value().clone();

        // 4. Record round result for the group.
        let group_state = self.groups.entry(group.to_owned()).or_default();
        match round_type {
            RoundType::Fast => {
                group_state.fast = Some(result);
                group_state.final_round = None;
            }
            RoundType::Final => group_state.final_round = Some(result),
        }

        // 5. Check promotion/demotion based on aggregated verdicts
        let all_expected_zero = expected_groups.iter().all(|eg| {
            self.groups.get(eg).is_some_and(|gs| {
                let round_result = match round_type {
                    RoundType::Fast => gs.fast.as_ref(),
                    RoundType::Final => gs.final_round.as_ref(),
                };
                round_result.is_some_and(|r| r.verdict.is_zero_findings())
            })
        });

        if all_expected_zero {
            match round_type {
                RoundType::Fast => self.status = ReviewStatus::FastPassed,
                RoundType::Final => self.status = ReviewStatus::Approved,
            }
        } else {
            match round_type {
                RoundType::Fast => {
                    if self.status == ReviewStatus::FastPassed
                        || self.status == ReviewStatus::Approved
                    {
                        self.status = ReviewStatus::NotStarted;
                    }
                }
                RoundType::Final => {
                    if self.status == ReviewStatus::Approved {
                        self.status = ReviewStatus::FastPassed;
                    }
                }
            }
        }

        // 6. Update escalation state after recording.
        self.update_escalation_after_record(round_type, expected_groups, &timestamp);

        Ok(())
    }

    /// Validates that verdict and concerns are consistent.
    ///
    /// # Errors
    ///
    /// - `ReviewError::InvalidConcern` if `zero_findings` verdict has non-empty concerns.
    /// - `ReviewError::InvalidConcern` if `findings_remain` verdict has empty concerns.
    fn validate_verdict_concerns(result: &ReviewRoundResult) -> Result<(), ReviewError> {
        if result.verdict().is_zero_findings() && !result.concerns().is_empty() {
            return Err(ReviewError::InvalidConcern(
                "zero_findings verdict must have empty concerns".to_owned(),
            ));
        }
        if result.verdict() == Verdict::FindingsRemain && result.concerns().is_empty() {
            return Err(ReviewError::InvalidConcern(
                "findings_remain verdict must have non-empty concerns".to_owned(),
            ));
        }
        Ok(())
    }

    /// Called after recording a round result. Checks if a closed cycle is complete
    /// and updates escalation state accordingly.
    fn update_escalation_after_record(
        &mut self,
        round_type: RoundType,
        expected_groups: &[String],
        timestamp: &Timestamp,
    ) {
        // 1. Check if cycle is closed: all expected groups have recorded this round_type
        //    with the same round number.
        let round_numbers: Vec<Option<u32>> = expected_groups
            .iter()
            .map(|g| {
                self.groups.get(g).and_then(|gs| {
                    let rr = match round_type {
                        RoundType::Fast => gs.fast(),
                        RoundType::Final => gs.final_round(),
                    };
                    rr.map(|r| r.round())
                })
            })
            .collect();

        // All groups must have a result, and all must have the same round number.
        let first = match round_numbers.first() {
            Some(Some(n)) => *n,
            _ => return,
        };
        if !round_numbers.iter().all(|n| *n == Some(first)) {
            return;
        }

        // 1b. Duplicate cycle detection: if this (round_type, round) was already counted,
        //     skip to prevent double-counting when a group re-records the same round.
        let already_counted = self
            .escalation
            .recent_cycles
            .iter()
            .any(|c| c.round_type() == round_type && c.round() == first);
        if already_counted {
            return;
        }

        // 2. Collect concerns from all groups for this cycle (union via BTreeSet for dedup).
        let mut cycle_concerns_set = std::collections::BTreeSet::new();
        let mut group_names = Vec::new();
        for g in expected_groups {
            group_names.push(g.clone());
            if let Some(gs) = self.groups.get(g) {
                let rr = match round_type {
                    RoundType::Fast => gs.fast(),
                    RoundType::Final => gs.final_round(),
                };
                if let Some(r) = rr {
                    for c in r.concerns() {
                        cycle_concerns_set.insert(c.clone());
                    }
                }
            }
        }
        let cycle_concerns_vec: Vec<ReviewConcern> = cycle_concerns_set.iter().cloned().collect();

        // 3. Update concern_streaks.
        // Increment streaks for concerns present in this cycle.
        for concern in &cycle_concerns_vec {
            let streak =
                self.escalation.concern_streaks.entry(concern.clone()).or_insert_with(|| {
                    ReviewConcernStreak::new(0, round_type, first, timestamp.clone())
                });
            *streak = ReviewConcernStreak::new(
                streak.consecutive_rounds().saturating_add(1),
                round_type,
                first,
                timestamp.clone(),
            );
        }
        // Reset streaks for concerns NOT present in this cycle.
        self.escalation.concern_streaks.retain(|k, _| cycle_concerns_set.contains(k));

        // 4. Add to recent_cycles (FIFO, max 10).
        let summary = ReviewCycleSummary::new(
            round_type,
            first,
            timestamp.clone(),
            cycle_concerns_vec,
            group_names,
        );
        self.escalation.recent_cycles.push(summary);
        if self.escalation.recent_cycles.len() > 10 {
            self.escalation.recent_cycles.remove(0);
        }

        // 5. Check threshold → transition to Blocked if any concern streak >= threshold.
        let threshold = self.escalation.threshold;
        let blocked_concerns: Vec<ReviewConcern> = self
            .escalation
            .concern_streaks
            .iter()
            .filter(|(_, s)| s.consecutive_rounds() >= threshold)
            .map(|(k, _)| k.clone())
            .collect();

        if !blocked_concerns.is_empty() {
            self.escalation.phase = EscalationPhase::Blocked(ReviewEscalationBlock::new(
                blocked_concerns,
                timestamp.clone(),
            ));
        }
    }

    /// Sets the code_hash to the given computed value.
    ///
    /// Validates via `CodeHash::computed()`: trims whitespace, rejects empty strings
    /// and the reserved literal `"PENDING"`.
    ///
    /// Used in the normalized hash protocol to write back the computed hash
    /// after record_round_with_pending + re-stage + hash computation.
    ///
    /// # Errors
    ///
    /// Returns `ReviewError::InvalidConcern` if the hash is empty, whitespace-only,
    /// or the reserved `"PENDING"` literal.
    pub fn set_code_hash(&mut self, hash: String) -> Result<(), ReviewError> {
        self.code_hash = Some(CodeHash::computed(hash)?);
        Ok(())
    }

    /// Checks if the review state is ready for commit.
    ///
    /// # Errors
    ///
    /// - `ReviewError::EscalationActive` if escalation is blocked. Short-circuits before all
    ///   other checks.
    /// - `ReviewError::NotApproved` if status is not `Approved`.
    /// - `ReviewError::StaleCodeHash` if code_hash doesn't match. Sets status to
    ///   `Invalidated` as a side effect.
    pub fn check_commit_ready(&mut self, current_code_hash: &str) -> Result<(), ReviewError> {
        // 0. Escalation block check (short-circuit before all other checks).
        if let EscalationPhase::Blocked(ref block) = self.escalation.phase {
            return Err(ReviewError::EscalationActive {
                concerns: block.concerns.iter().map(|c| c.as_ref().to_owned()).collect(),
            });
        }

        if self.status != ReviewStatus::Approved {
            return Err(ReviewError::NotApproved(self.status));
        }
        match &self.code_hash {
            Some(CodeHash::Pending) => {
                // Pending means the two-phase protocol hasn't completed.
                // Reject commit — the real hash was never written back.
                self.status = ReviewStatus::Invalidated;
                return Err(ReviewError::StaleCodeHash {
                    expected: "PENDING".to_owned(),
                    actual: current_code_hash.to_owned(),
                });
            }
            Some(CodeHash::Computed(stored_str)) => {
                if stored_str != current_code_hash {
                    self.status = ReviewStatus::Invalidated;
                    return Err(ReviewError::StaleCodeHash {
                        expected: stored_str.clone(),
                        actual: current_code_hash.to_owned(),
                    });
                }
            }
            None => {}
        }
        Ok(())
    }

    /// Invalidates the review state (e.g., when code changes are detected).
    ///
    /// Clears `code_hash` so that a subsequent `record_round` with the new hash
    /// is accepted (fresh start), preventing permanent deadlock.
    pub fn invalidate(&mut self) {
        self.status = ReviewStatus::Invalidated;
        self.code_hash = None;
    }

    /// Resolves an active escalation block.
    ///
    /// Requires evidence references and a decision. On success:
    /// - clears streak state
    /// - stores the resolution record
    /// - sets `ReviewStatus::Invalidated` and clears `code_hash` (fresh start)
    ///
    /// # Errors
    ///
    /// - `ReviewError::EscalationNotActive` if no escalation block is active.
    /// - `ReviewError::ResolutionEvidenceMissing` if `workspace_search_ref` or
    ///   `reinvention_check_ref` is empty.
    /// - `ReviewError::ResolutionConcernMismatch` if the resolution's `blocked_concerns`
    ///   do not match the active block's concerns.
    pub fn resolve_escalation(
        &mut self,
        resolution: ReviewEscalationResolution,
    ) -> Result<(), ReviewError> {
        // Verify escalation is active
        let block = match &self.escalation.phase {
            EscalationPhase::Blocked(b) => b.clone(),
            EscalationPhase::Clear => return Err(ReviewError::EscalationNotActive),
        };

        // Validate evidence references and required fields
        if resolution.workspace_search_ref.trim().is_empty() {
            return Err(ReviewError::ResolutionEvidenceMissing("workspace_search_ref"));
        }
        if resolution.reinvention_check_ref.trim().is_empty() {
            return Err(ReviewError::ResolutionEvidenceMissing("reinvention_check_ref"));
        }
        if resolution.summary.trim().is_empty() {
            return Err(ReviewError::ResolutionEvidenceMissing("summary"));
        }
        // resolved_at is a Timestamp — validity is guaranteed by its constructor.

        // Validate concerns match (order-insensitive: sort both before comparing).
        let mut expected: Vec<String> =
            block.concerns.iter().map(|c| c.as_ref().to_owned()).collect();
        let mut actual: Vec<String> =
            resolution.blocked_concerns.iter().map(|c| c.as_ref().to_owned()).collect();
        expected.sort();
        actual.sort();
        if expected != actual {
            return Err(ReviewError::ResolutionConcernMismatch { expected, actual });
        }

        // Apply resolution: clear streaks, store resolution, invalidate review
        self.escalation.concern_streaks.clear();
        self.escalation.phase = EscalationPhase::Clear;
        self.escalation.last_resolution = Some(resolution);
        self.status = ReviewStatus::Invalidated;
        self.code_hash = None;

        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn ts(s: &str) -> Timestamp {
        Timestamp::new(s).unwrap()
    }

    fn zero() -> ReviewRoundResult {
        ReviewRoundResult::new(1, Verdict::ZeroFindings, ts("2026-03-18T00:00:00Z"))
    }

    fn findings() -> ReviewRoundResult {
        let concern = ReviewConcern::try_new("test-concern").unwrap();
        ReviewRoundResult::new_with_concerns(
            1,
            Verdict::FindingsRemain,
            ts("2026-03-18T00:00:00Z"),
            vec![concern],
        )
    }

    // --- ReviewStatus tests ---

    #[test]
    fn test_review_status_default_is_not_started() {
        assert_eq!(ReviewStatus::default(), ReviewStatus::NotStarted);
    }

    #[test]
    fn test_review_status_display() {
        assert_eq!(ReviewStatus::NotStarted.to_string(), "not_started");
        assert_eq!(ReviewStatus::Invalidated.to_string(), "invalidated");
        assert_eq!(ReviewStatus::FastPassed.to_string(), "fast_passed");
        assert_eq!(ReviewStatus::Approved.to_string(), "approved");
    }

    // --- ReviewState::new tests ---

    #[test]
    fn test_review_state_new_has_not_started_status() {
        let state = ReviewState::new();
        assert_eq!(state.status(), ReviewStatus::NotStarted);
        assert!(state.code_hash().is_none());
        assert!(state.groups().is_empty());
    }

    // --- record_round: fast round recording ---

    #[test]
    fn test_record_fast_zero_findings_for_single_group_promotes_to_fast_passed() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "abc123").unwrap();

        assert_eq!(state.status(), ReviewStatus::FastPassed);
        assert_eq!(state.code_hash(), Some("abc123"));
        assert!(state.groups().get("group-a").unwrap().fast().is_some());
    }

    #[test]
    fn test_record_fast_partial_groups_does_not_promote() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned(), "group-b".to_owned()];
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "abc123").unwrap();

        // Only one of two expected groups recorded — no promotion
        assert_eq!(state.status(), ReviewStatus::NotStarted);
    }

    #[test]
    fn test_record_fast_all_groups_zero_findings_promotes() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned(), "group-b".to_owned()];
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Fast, "group-b", zero(), &expected, "abc123").unwrap();

        assert_eq!(state.status(), ReviewStatus::FastPassed);
    }

    #[test]
    fn test_record_fast_findings_remain_blocks_promotion() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned(), "group-b".to_owned()];
        state.record_round(RoundType::Fast, "group-a", findings(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Fast, "group-b", zero(), &expected, "abc123").unwrap();

        // group-a has findings_remain — no promotion
        assert_eq!(state.status(), ReviewStatus::NotStarted);
    }

    #[test]
    fn test_record_fast_does_not_overwrite_other_groups() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned(), "group-b".to_owned()];
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Fast, "group-b", zero(), &expected, "abc123").unwrap();

        // Both groups should exist
        assert!(state.groups().get("group-a").unwrap().fast().is_some());
        assert!(state.groups().get("group-b").unwrap().fast().is_some());
    }

    // --- record_round: final round recording ---

    #[test]
    fn test_record_final_after_fast_passed_succeeds() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::FastPassed);

        state.record_round(RoundType::Final, "group-a", zero(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::Approved);
    }

    #[test]
    fn test_record_final_without_fast_passed_fails() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];
        let result = state.record_round(RoundType::Final, "group-a", zero(), &expected, "abc123");

        assert!(matches!(
            result,
            Err(ReviewError::FinalRequiresFastPassed(ReviewStatus::NotStarted))
        ));
    }

    #[test]
    fn test_record_final_partial_groups_does_not_promote() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned(), "group-b".to_owned()];
        // Fast pass both
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Fast, "group-b", zero(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::FastPassed);

        // Final only for group-a
        state.record_round(RoundType::Final, "group-a", zero(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::FastPassed); // Not promoted yet
    }

    #[test]
    fn test_record_final_findings_remain_blocks_promotion() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned(), "group-b".to_owned()];
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Fast, "group-b", zero(), &expected, "abc123").unwrap();

        state.record_round(RoundType::Final, "group-a", findings(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Final, "group-b", zero(), &expected, "abc123").unwrap();

        assert_eq!(state.status(), ReviewStatus::FastPassed); // findings in A blocks
    }

    // --- record_round: code hash validation ---

    #[test]
    fn test_record_round_stale_code_hash_rejects_and_invalidates() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "abc123").unwrap();

        let result = state.record_round(RoundType::Fast, "group-a", zero(), &expected, "def456");

        assert!(matches!(
            result,
            Err(ReviewError::StaleCodeHash { ref expected, ref actual })
                if expected == "abc123" && actual == "def456"
        ));
        assert_eq!(state.status(), ReviewStatus::Invalidated);
    }

    #[test]
    fn test_record_round_first_round_sets_code_hash() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "abc123").unwrap();

        assert_eq!(state.code_hash(), Some("abc123"));
    }

    #[test]
    fn test_record_final_stale_code_hash_rejects() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::FastPassed);

        let result = state.record_round(RoundType::Final, "group-a", zero(), &expected, "new-hash");
        assert!(matches!(result, Err(ReviewError::StaleCodeHash { .. })));
        assert_eq!(state.status(), ReviewStatus::Invalidated);
    }

    // --- check_commit_ready ---

    #[test]
    fn test_check_commit_ready_approved_with_matching_hash_succeeds() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Final, "group-a", zero(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::Approved);

        assert!(state.check_commit_ready("abc123").is_ok());
    }

    #[test]
    fn test_check_commit_ready_not_approved_fails() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "abc123").unwrap();

        let result = state.check_commit_ready("abc123");
        assert!(matches!(result, Err(ReviewError::NotApproved(ReviewStatus::FastPassed))));
    }

    #[test]
    fn test_check_commit_ready_stale_hash_rejects_and_invalidates() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Final, "group-a", zero(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::Approved);

        let result = state.check_commit_ready("new-hash");
        assert!(matches!(result, Err(ReviewError::StaleCodeHash { .. })));
        assert_eq!(state.status(), ReviewStatus::Invalidated);
    }

    // --- demotion on findings_remain ---

    #[test]
    fn test_fast_findings_after_fast_passed_demotes_to_not_started() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::FastPassed);

        // Record a new fast round with findings — should demote
        state.record_round(RoundType::Fast, "group-a", findings(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::NotStarted);
    }

    #[test]
    fn test_fast_findings_after_approved_demotes_to_not_started() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Final, "group-a", zero(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::Approved);

        // Fast round with findings on approved track — demotes to not_started
        state.record_round(RoundType::Fast, "group-a", findings(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::NotStarted);
    }

    #[test]
    fn test_final_findings_after_approved_demotes_to_fast_passed() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned(), "group-b".to_owned()];
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Fast, "group-b", zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Final, "group-a", zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Final, "group-b", zero(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::Approved);

        // Final round with findings — demotes to fast_passed
        state.record_round(RoundType::Final, "group-a", findings(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::FastPassed);
    }

    #[test]
    fn test_fast_rerun_clears_stale_final_round() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned(), "group-b".to_owned()];

        // Full approval cycle
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Fast, "group-b", zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Final, "group-a", zero(), &expected, "abc123").unwrap();
        state.record_round(RoundType::Final, "group-b", zero(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::Approved);

        // Re-run fast for group-a with findings — should clear group-a's final_round
        state.record_round(RoundType::Fast, "group-a", findings(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::NotStarted);
        assert!(state.groups().get("group-a").unwrap().final_round().is_none());
        // group-b's final_round should still be intact
        assert!(state.groups().get("group-b").unwrap().final_round().is_some());

        // Now re-pass fast for group-a and try to go to final
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "abc123").unwrap();
        // group-a still has no final_round, so final aggregation should NOT promote
        // even though group-b has an old final zero_findings
        assert_eq!(state.status(), ReviewStatus::FastPassed);

        // After group-a's fast rerun cleared group-a's final, re-record group-a's final.
        // group-b's final is still valid (its fast was not re-run, same code_hash).
        // Both groups now have final zero_findings → promotes to Approved.
        state.record_round(RoundType::Final, "group-a", zero(), &expected, "abc123").unwrap();
        assert_eq!(state.status(), ReviewStatus::Approved);
    }

    // --- invalidate ---

    #[test]
    fn test_invalidate_sets_status_to_invalidated() {
        let mut state = ReviewState::new();
        state.invalidate();
        assert_eq!(state.status(), ReviewStatus::Invalidated);
    }

    #[test]
    fn test_record_round_after_stale_hash_invalidation_accepts_new_hash() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];

        // First round sets code_hash
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "hash1").unwrap();
        assert_eq!(state.status(), ReviewStatus::FastPassed);

        // Stale hash → invalidation + code_hash cleared
        let err = state.record_round(RoundType::Fast, "group-a", zero(), &expected, "hash2");
        assert!(matches!(err, Err(ReviewError::StaleCodeHash { .. })));
        assert_eq!(state.status(), ReviewStatus::Invalidated);
        assert!(state.code_hash().is_none());

        // Re-run with new hash should succeed (fresh start)
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "hash2").unwrap();
        assert_eq!(state.status(), ReviewStatus::FastPassed);
        assert_eq!(state.code_hash(), Some("hash2"));
    }

    // --- record_round_with_pending ---

    #[test]
    fn test_record_round_with_pending_sets_code_hash_to_pending() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];
        state
            .record_round_with_pending(RoundType::Fast, "group-a", zero(), &expected, "pre-hash")
            .unwrap();
        // code_hash() returns None for Pending; use code_hash_for_serialization() to get "PENDING"
        assert!(state.code_hash().is_none());
        assert_eq!(state.code_hash_for_serialization(), Some("PENDING"));
    }

    #[test]
    fn test_record_round_with_pending_first_round_skips_freshness_check() {
        let mut state = ReviewState::new();
        // code_hash is None initially — freshness check must be skipped
        let expected = vec!["group-a".to_owned()];
        let result = state.record_round_with_pending(
            RoundType::Fast,
            "group-a",
            zero(),
            &expected,
            "any-hash",
        );
        assert!(result.is_ok());
        assert!(state.code_hash().is_none());
        assert_eq!(state.code_hash_for_serialization(), Some("PENDING"));
    }

    #[test]
    fn test_record_round_with_pending_subsequent_round_checks_freshness() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];
        // Set up a code_hash via record_round first
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "hash1").unwrap();
        assert_eq!(state.code_hash(), Some("hash1"));

        // Passing correct pre_update_hash should succeed
        let result =
            state.record_round_with_pending(RoundType::Fast, "group-a", zero(), &expected, "hash1");
        assert!(result.is_ok());
        assert!(state.code_hash().is_none());
        assert_eq!(state.code_hash_for_serialization(), Some("PENDING"));
    }

    #[test]
    fn test_record_round_with_pending_stale_hash_rejects_and_invalidates() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];
        // Set up a code_hash
        state.record_round(RoundType::Fast, "group-a", zero(), &expected, "hash1").unwrap();

        // Wrong pre_update_hash → should fail
        let result = state.record_round_with_pending(
            RoundType::Fast,
            "group-a",
            zero(),
            &expected,
            "wrong-hash",
        );
        assert!(matches!(
            result,
            Err(ReviewError::StaleCodeHash { ref expected, ref actual })
                if expected == "hash1" && actual == "wrong-hash"
        ));
        assert_eq!(state.status(), ReviewStatus::Invalidated);
    }

    #[test]
    fn test_record_round_with_pending_final_without_fast_passed_fails() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];
        let result = state.record_round_with_pending(
            RoundType::Final,
            "group-a",
            zero(),
            &expected,
            "any-hash",
        );
        assert!(matches!(
            result,
            Err(ReviewError::FinalRequiresFastPassed(ReviewStatus::NotStarted))
        ));
    }

    #[test]
    fn test_record_round_with_pending_records_group_result() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];
        state
            .record_round_with_pending(RoundType::Fast, "group-a", zero(), &expected, "pre-hash")
            .unwrap();
        assert!(state.groups().get("group-a").is_some());
        assert!(state.groups().get("group-a").unwrap().fast().is_some());
    }

    // --- set_code_hash ---

    #[test]
    fn test_set_code_hash_sets_value() {
        let mut state = ReviewState::new();
        state.set_code_hash("computed-hash".to_owned()).unwrap();
        assert_eq!(state.code_hash(), Some("computed-hash"));
    }

    #[test]
    fn test_set_code_hash_overwrites_existing_value() {
        let mut state = ReviewState::new();
        state.set_code_hash("old-hash".to_owned()).unwrap();
        state.set_code_hash("new-hash".to_owned()).unwrap();
        assert_eq!(state.code_hash(), Some("new-hash"));
    }

    #[test]
    fn test_two_phase_hash_protocol_full_flow() {
        // Simulate the full two-phase protocol:
        // 1. record_round_with_pending with pre_update_hash
        // 2. set_code_hash with the computed post-update hash
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];

        // Phase 1: record with PENDING
        state
            .record_round_with_pending(
                RoundType::Fast,
                "group-a",
                zero(),
                &expected,
                "pre-update-hash",
            )
            .unwrap();
        // code_hash() returns None for Pending; serialization gives "PENDING"
        assert!(state.code_hash().is_none());
        assert_eq!(state.code_hash_for_serialization(), Some("PENDING"));

        // Phase 2: write back real hash
        state.set_code_hash("post-update-hash".to_owned()).unwrap();
        assert_eq!(state.code_hash(), Some("post-update-hash"));
        assert_eq!(state.status(), ReviewStatus::FastPassed);
    }

    // --- ReviewConcern tests ---

    #[test]
    fn test_review_concern_new_with_valid_slug_succeeds() {
        let c = ReviewConcern::try_new("domain.review").unwrap();
        assert_eq!(c.as_ref(), "domain.review");
    }

    #[test]
    fn test_review_concern_new_with_empty_string_fails() {
        let result = ReviewConcern::try_new("");
        assert!(matches!(result, Err(ReviewError::InvalidConcern(_))));
    }

    #[test]
    fn test_review_concern_new_with_whitespace_only_fails() {
        let result = ReviewConcern::try_new("   ");
        assert!(matches!(result, Err(ReviewError::InvalidConcern(_))));
    }

    #[test]
    fn test_review_concern_normalizes_to_lowercase() {
        let c = ReviewConcern::try_new("Domain.Review").unwrap();
        assert_eq!(c.as_ref(), "domain.review");
    }

    #[test]
    fn test_review_concern_trims_whitespace() {
        let c = ReviewConcern::try_new("  shell-parsing  ").unwrap();
        assert_eq!(c.as_ref(), "shell-parsing");
    }

    #[test]
    fn test_review_concern_ord_is_lexicographic() {
        let a = ReviewConcern::try_new("aaa").unwrap();
        let b = ReviewConcern::try_new("bbb").unwrap();
        assert!(a < b);
    }

    // --- ReviewEscalationState tests ---

    #[test]
    fn test_escalation_state_new_is_clear() {
        let state = ReviewEscalationState::new();
        assert_eq!(state.threshold(), 3);
        assert_eq!(state.phase(), &EscalationPhase::Clear);
        assert!(state.recent_cycles().is_empty());
        assert!(state.concern_streaks().is_empty());
        assert!(state.last_resolution().is_none());
    }

    #[test]
    fn test_escalation_state_is_blocked_returns_false_when_clear() {
        let state = ReviewEscalationState::new();
        assert!(!state.is_blocked());
    }

    #[test]
    fn test_escalation_state_is_blocked_returns_true_when_blocked() {
        let concern = ReviewConcern::try_new("domain.review").unwrap();
        let block = ReviewEscalationBlock::new(vec![concern], ts("2026-03-19T00:00:00Z"));
        let state = ReviewEscalationState::with_fields(
            3,
            EscalationPhase::Blocked(block),
            Vec::new(),
            BTreeMap::new(),
            None,
        );
        assert!(state.is_blocked());
    }

    // --- EscalationActive gate tests ---

    fn blocked_review_state() -> ReviewState {
        let concern = ReviewConcern::try_new("domain.review").unwrap();
        let block = ReviewEscalationBlock::new(vec![concern], ts("2026-03-19T00:00:00Z"));
        let escalation = ReviewEscalationState::with_fields(
            3,
            EscalationPhase::Blocked(block),
            Vec::new(),
            BTreeMap::new(),
            None,
        );
        ReviewState::with_fields(ReviewStatus::NotStarted, None, HashMap::new(), escalation)
    }

    #[test]
    fn test_record_round_rejects_when_escalation_blocked() {
        let mut state = blocked_review_state();
        let expected = vec!["group-a".to_owned()];
        let result = state.record_round(RoundType::Fast, "group-a", zero(), &expected, "abc123");
        assert!(
            matches!(result, Err(ReviewError::EscalationActive { ref concerns }) if !concerns.is_empty()),
            "expected EscalationActive, got {result:?}"
        );
    }

    #[test]
    fn test_record_round_with_pending_rejects_when_escalation_blocked() {
        let mut state = blocked_review_state();
        let expected = vec!["group-a".to_owned()];
        let result = state.record_round_with_pending(
            RoundType::Fast,
            "group-a",
            zero(),
            &expected,
            "abc123",
        );
        assert!(
            matches!(result, Err(ReviewError::EscalationActive { ref concerns }) if !concerns.is_empty()),
            "expected EscalationActive, got {result:?}"
        );
    }

    #[test]
    fn test_check_commit_ready_rejects_when_escalation_blocked() {
        let concern = ReviewConcern::try_new("domain.review").unwrap();
        let block = ReviewEscalationBlock::new(vec![concern], ts("2026-03-19T00:00:00Z"));
        let escalation = ReviewEscalationState::with_fields(
            3,
            EscalationPhase::Blocked(block),
            Vec::new(),
            BTreeMap::new(),
            None,
        );
        // Use Approved status so the only block is escalation
        let mut state = ReviewState::with_fields(
            ReviewStatus::Approved,
            Some(CodeHash::Computed("abc123".to_owned())),
            HashMap::new(),
            escalation,
        );
        let result = state.check_commit_ready("abc123");
        assert!(
            matches!(result, Err(ReviewError::EscalationActive { ref concerns }) if !concerns.is_empty()),
            "expected EscalationActive, got {result:?}"
        );
    }

    #[test]
    fn test_escalation_check_happens_before_hash_check() {
        // Set up a state with BOTH stale hash AND blocked escalation.
        // The method must return EscalationActive (not StaleCodeHash).
        let concern = ReviewConcern::try_new("domain.review").unwrap();
        let block = ReviewEscalationBlock::new(vec![concern], ts("2026-03-19T00:00:00Z"));
        let escalation = ReviewEscalationState::with_fields(
            3,
            EscalationPhase::Blocked(block),
            Vec::new(),
            BTreeMap::new(),
            None,
        );
        let mut state = ReviewState::with_fields(
            ReviewStatus::NotStarted,
            Some(CodeHash::Computed("old-hash".to_owned())),
            HashMap::new(),
            escalation,
        );
        let expected = vec!["group-a".to_owned()];
        let result = state.record_round(RoundType::Fast, "group-a", zero(), &expected, "new-hash");
        assert!(
            matches!(result, Err(ReviewError::EscalationActive { .. })),
            "expected EscalationActive before StaleCodeHash, got {result:?}"
        );
    }

    // --- ReviewRoundResult concerns tests ---

    #[test]
    fn test_review_round_result_new_has_empty_concerns() {
        let result = ReviewRoundResult::new(1, Verdict::ZeroFindings, ts("2026-03-19T00:00:00Z"));
        assert!(result.concerns().is_empty());
    }

    #[test]
    fn test_review_round_result_new_with_concerns() {
        let concern = ReviewConcern::try_new("domain.review").unwrap();
        let result = ReviewRoundResult::new_with_concerns(
            1,
            Verdict::FindingsRemain,
            ts("2026-03-19T00:00:00Z"),
            vec![concern.clone()],
        );
        assert_eq!(result.concerns(), &[concern]);
    }

    // --- with_fields ---

    #[test]
    fn test_with_fields_preserves_all_fields() {
        let mut groups = HashMap::new();
        groups.insert(
            "g1".to_owned(),
            ReviewGroupState::with_fast(ReviewRoundResult::new(
                1,
                Verdict::ZeroFindings,
                ts("2026-03-19T00:00:00Z"),
            )),
        );

        let state = ReviewState::with_fields(
            ReviewStatus::FastPassed,
            Some(CodeHash::Computed("hash123".to_owned())),
            groups.clone(),
            ReviewEscalationState::default(),
        );

        assert_eq!(state.status(), ReviewStatus::FastPassed);
        assert_eq!(state.code_hash(), Some("hash123"));
        assert_eq!(state.groups(), &groups);
    }

    // --- resolve_escalation tests ---

    fn blocked_state() -> ReviewState {
        let block = ReviewEscalationBlock::new(
            vec![ReviewConcern::try_new("shell-parsing").unwrap()],
            ts("2026-03-19T00:00:00Z"),
        );
        let escalation = ReviewEscalationState::with_fields(
            3,
            EscalationPhase::Blocked(block),
            Vec::new(),
            BTreeMap::new(),
            None,
        );
        ReviewState::with_fields(ReviewStatus::NotStarted, None, HashMap::new(), escalation)
    }

    fn valid_resolution() -> ReviewEscalationResolution {
        ReviewEscalationResolution::new(
            vec![ReviewConcern::try_new("shell-parsing").unwrap()],
            "search.md".to_owned(),
            "reinvention.md".to_owned(),
            ReviewEscalationDecision::ContinueSelfImplementation,
            "Justified: no suitable crate".to_owned(),
            ts("2026-03-19T01:00:00Z"),
        )
    }

    #[test]
    fn test_resolve_escalation_succeeds_with_valid_evidence() {
        let mut state = blocked_state();
        assert!(state.resolve_escalation(valid_resolution()).is_ok());
        assert_eq!(state.status(), ReviewStatus::Invalidated);
        assert!(state.code_hash().is_none());
        assert!(!state.escalation().is_blocked());
        assert!(state.escalation().last_resolution().is_some());
    }

    #[test]
    fn test_resolve_escalation_rejects_when_not_blocked() {
        let mut state = ReviewState::new();
        let result = state.resolve_escalation(valid_resolution());
        assert!(matches!(result, Err(ReviewError::EscalationNotActive)));
    }

    #[test]
    fn test_resolve_escalation_rejects_empty_workspace_search_ref() {
        let mut state = blocked_state();
        let mut res = valid_resolution();
        res.workspace_search_ref = "".to_owned();
        let result = state.resolve_escalation(res);
        assert!(matches!(
            result,
            Err(ReviewError::ResolutionEvidenceMissing("workspace_search_ref"))
        ));
    }

    #[test]
    fn test_resolve_escalation_rejects_empty_reinvention_check_ref() {
        let mut state = blocked_state();
        let mut res = valid_resolution();
        res.reinvention_check_ref = "  ".to_owned();
        let result = state.resolve_escalation(res);
        assert!(matches!(
            result,
            Err(ReviewError::ResolutionEvidenceMissing("reinvention_check_ref"))
        ));
    }

    #[test]
    fn test_resolve_escalation_rejects_empty_summary() {
        let mut state = blocked_state();
        let mut res = valid_resolution();
        res.summary = "".to_owned();
        let result = state.resolve_escalation(res);
        assert!(matches!(result, Err(ReviewError::ResolutionEvidenceMissing("summary"))));
    }

    #[test]
    fn test_resolve_escalation_rejects_mismatched_concerns() {
        let mut state = blocked_state();
        let mut res = valid_resolution();
        res.blocked_concerns = vec![ReviewConcern::try_new("different-concern").unwrap()];
        let result = state.resolve_escalation(res);
        assert!(matches!(result, Err(ReviewError::ResolutionConcernMismatch { .. })));
    }

    // --- Finding 1: expected_groups deduplication ---

    #[test]
    fn test_record_round_deduplicates_expected_groups() {
        // Passing duplicate expected_groups must not cause false cycle detection
        // (one result satisfying multiple slots).
        let mut state = ReviewState::new();
        // "group-a" appears twice in expected_groups — must be deduplicated to one entry.
        let expected_with_dups =
            vec!["group-a".to_owned(), "group-a".to_owned(), "group-b".to_owned()];

        // Record only group-a — with duplicates this might incorrectly satisfy
        // both "group-a" slots and cause a false promotion.
        state
            .record_round(RoundType::Fast, "group-a", zero(), &expected_with_dups, "abc123")
            .unwrap();

        // After dedup, expected_groups is ["group-a", "group-b"].
        // Only group-a has recorded, so promotion must NOT happen.
        assert_eq!(
            state.status(),
            ReviewStatus::NotStarted,
            "duplicate expected_groups must not cause false promotion when only one unique group recorded"
        );

        // Now record group-b as well — both unique groups have zero_findings, so promote.
        state
            .record_round(RoundType::Fast, "group-b", zero(), &expected_with_dups, "abc123")
            .unwrap();
        assert_eq!(state.status(), ReviewStatus::FastPassed);
    }

    // --- Finding 1: verdict/concerns consistency ---

    #[test]
    fn test_record_round_rejects_zero_findings_with_concerns() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];
        let concern = ReviewConcern::try_new("some-concern").unwrap();
        let result_with_concern = ReviewRoundResult::new_with_concerns(
            1,
            Verdict::ZeroFindings,
            ts("2026-03-19T00:00:00Z"),
            vec![concern],
        );
        let result = state.record_round(
            RoundType::Fast,
            "group-a",
            result_with_concern,
            &expected,
            "abc123",
        );
        assert!(
            matches!(result, Err(ReviewError::InvalidConcern(_))),
            "expected InvalidConcern for zero_findings with non-empty concerns, got {result:?}"
        );
    }

    #[test]
    fn test_record_round_rejects_findings_remain_without_concerns() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];
        let result_no_concerns =
            ReviewRoundResult::new(1, Verdict::FindingsRemain, ts("2026-03-19T00:00:00Z"));
        let result =
            state.record_round(RoundType::Fast, "group-a", result_no_concerns, &expected, "abc123");
        assert!(
            matches!(result, Err(ReviewError::InvalidConcern(_))),
            "expected InvalidConcern for findings_remain with empty concerns, got {result:?}"
        );
    }

    // --- Finding 2: resolve_escalation order-insensitive concern comparison ---

    #[test]
    fn test_resolve_escalation_accepts_reordered_concerns() {
        // Block with concerns [a, b]
        let block = ReviewEscalationBlock::new(
            vec![ReviewConcern::try_new("aaa").unwrap(), ReviewConcern::try_new("bbb").unwrap()],
            ts("2026-03-19T00:00:00Z"),
        );
        let escalation = ReviewEscalationState::with_fields(
            3,
            EscalationPhase::Blocked(block),
            Vec::new(),
            BTreeMap::new(),
            None,
        );
        let mut state =
            ReviewState::with_fields(ReviewStatus::NotStarted, None, HashMap::new(), escalation);

        // Resolution with concerns in reverse order [b, a]
        let resolution = ReviewEscalationResolution::new(
            vec![ReviewConcern::try_new("bbb").unwrap(), ReviewConcern::try_new("aaa").unwrap()],
            "search.md",
            "reinvention.md",
            ReviewEscalationDecision::ContinueSelfImplementation,
            "justified",
            ts("2026-03-19T01:00:00Z"),
        );
        let result = state.resolve_escalation(resolution);
        assert!(result.is_ok(), "expected Ok for reordered concerns, got {result:?}");
    }

    // --- Finding 3: escalation state updates after record_round ---

    fn round_with_concern(round: u32, concern: &str, ts_str: &str) -> ReviewRoundResult {
        let c = ReviewConcern::try_new(concern).unwrap();
        ReviewRoundResult::new_with_concerns(round, Verdict::FindingsRemain, ts(ts_str), vec![c])
    }

    fn zero_round(round: u32, ts_str: &str) -> ReviewRoundResult {
        ReviewRoundResult::new(round, Verdict::ZeroFindings, ts(ts_str))
    }

    #[test]
    fn test_escalation_triggers_after_3_consecutive_same_concern_cycles() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];

        // Round 1: fast findings with "bad-pattern"
        let r1 = round_with_concern(1, "bad-pattern", "2026-03-19T01:00:00Z");
        state.record_round(RoundType::Fast, "group-a", r1, &expected, "hash1").unwrap();

        // Round 2: fast findings with "bad-pattern" again — streak = 2
        let r2 = round_with_concern(2, "bad-pattern", "2026-03-19T02:00:00Z");
        state.record_round(RoundType::Fast, "group-a", r2, &expected, "hash1").unwrap();

        // Not yet blocked
        assert!(!state.escalation().is_blocked(), "should not be blocked after 2 rounds");

        // Round 3: fast findings with "bad-pattern" — streak = 3 → Blocked
        let r3 = round_with_concern(3, "bad-pattern", "2026-03-19T03:00:00Z");
        state.record_round(RoundType::Fast, "group-a", r3, &expected, "hash1").unwrap();

        assert!(state.escalation().is_blocked(), "should be blocked after 3 consecutive rounds");
        if let EscalationPhase::Blocked(ref block) = *state.escalation().phase() {
            assert_eq!(block.concerns().len(), 1);
            assert_eq!(block.concerns()[0].as_ref(), "bad-pattern");
        } else {
            panic!("expected Blocked phase");
        }
    }

    #[test]
    fn test_escalation_does_not_trigger_with_interrupted_streak() {
        // A → B → A → A: streak for A is 2 (reset when B appeared in round 2)
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];

        // Round 1: concern A
        let r1 = round_with_concern(1, "concern-a", "2026-03-19T01:00:00Z");
        state.record_round(RoundType::Fast, "group-a", r1, &expected, "hash1").unwrap();

        // Round 2: concern B (different) — resets A's streak
        let r2 = round_with_concern(2, "concern-b", "2026-03-19T02:00:00Z");
        state.record_round(RoundType::Fast, "group-a", r2, &expected, "hash1").unwrap();

        // Round 3: concern A again — streak for A is 1 (reset happened in round 2)
        let r3 = round_with_concern(3, "concern-a", "2026-03-19T03:00:00Z");
        state.record_round(RoundType::Fast, "group-a", r3, &expected, "hash1").unwrap();

        // Round 4: concern A — streak for A is 2 (not yet 3)
        let r4 = round_with_concern(4, "concern-a", "2026-03-19T04:00:00Z");
        state.record_round(RoundType::Fast, "group-a", r4, &expected, "hash1").unwrap();

        assert!(
            !state.escalation().is_blocked(),
            "should not be blocked: A streak is only 2 (was reset by B in round 2)"
        );
    }

    #[test]
    fn test_escalation_cycle_requires_all_groups() {
        let mut state = ReviewState::new();
        // Two expected groups — cycle only closes when both record
        let expected = vec!["group-a".to_owned(), "group-b".to_owned()];

        // Only group-a records 3 rounds (group-b never records)
        for i in 1u32..=3 {
            let ts_str = format!("2026-03-19T0{i}:00:00Z");
            let r = round_with_concern(i, "bad-pattern", &ts_str);
            state.record_round(RoundType::Fast, "group-a", r, &expected, "hash1").unwrap();
        }

        // Cycle never closes because group-b hasn't recorded → no escalation
        assert!(
            !state.escalation().is_blocked(),
            "partial group recording should not close a cycle or trigger escalation"
        );
    }

    #[test]
    fn test_recent_cycles_fifo_trims_at_10() {
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned()];

        // Record 12 fast rounds with zero_findings (each closes a cycle)
        for i in 1u32..=12 {
            let ts_str = format!("2026-03-19T{:02}:00:00Z", i);
            let r = zero_round(i, &ts_str);
            state.record_round(RoundType::Fast, "group-a", r, &expected, "hash1").unwrap();
        }

        assert_eq!(
            state.escalation().recent_cycles().len(),
            10,
            "recent_cycles should be trimmed to 10 (FIFO)"
        );
        // The oldest (round 1, 2) should have been evicted; round 12 should be present
        let last = state.escalation().recent_cycles().last().unwrap();
        assert_eq!(last.round(), 12);
    }

    // --- Finding 1: duplicate cycle detection ---

    #[test]
    fn test_escalation_rerecording_same_round_does_not_double_count() {
        // Two expected groups. Group A records round 1, then group B records round 1
        // → cycle closes. If group A then re-records round 1 (e.g., overwriting),
        // the cycle must NOT be counted again.
        let mut state = ReviewState::new();
        let expected = vec!["group-a".to_owned(), "group-b".to_owned()];

        // Both groups record findings_remain round 1 — cycle closes once
        let c = ReviewConcern::try_new("bad-pattern").unwrap();
        let r1a = ReviewRoundResult::new_with_concerns(
            1,
            Verdict::FindingsRemain,
            ts("2026-03-19T01:00:00Z"),
            vec![c.clone()],
        );
        let r1b = ReviewRoundResult::new_with_concerns(
            1,
            Verdict::FindingsRemain,
            ts("2026-03-19T02:00:00Z"),
            vec![c.clone()],
        );
        state.record_round(RoundType::Fast, "group-a", r1a, &expected, "hash1").unwrap();
        state.record_round(RoundType::Fast, "group-b", r1b, &expected, "hash1").unwrap();

        // After both groups record round 1, exactly 1 cycle should be counted
        assert_eq!(
            state.escalation().recent_cycles().len(),
            1,
            "one cycle after both groups record"
        );
        let streak_after_first = state
            .escalation()
            .concern_streaks()
            .get(&c)
            .map(|s| s.consecutive_rounds())
            .unwrap_or(0);
        assert_eq!(streak_after_first, 1, "streak should be 1 after first cycle");

        // Group A re-records the same round 1 (overwrite scenario)
        let r1a_again = ReviewRoundResult::new_with_concerns(
            1,
            Verdict::FindingsRemain,
            ts("2026-03-19T03:00:00Z"),
            vec![c.clone()],
        );
        state.record_round(RoundType::Fast, "group-a", r1a_again, &expected, "hash1").unwrap();

        // The cycle for (Fast, round=1) was already counted — must NOT be double-counted
        assert_eq!(
            state.escalation().recent_cycles().len(),
            1,
            "re-recording same round must not add a second cycle"
        );
        let streak_after_rerecord = state
            .escalation()
            .concern_streaks()
            .get(&c)
            .map(|s| s.consecutive_rounds())
            .unwrap_or(0);
        assert_eq!(
            streak_after_rerecord, 1,
            "streak must not increment on re-recording same round"
        );
    }

    // --- Verdict tests ---

    #[test]
    fn test_verdict_display_zero_findings() {
        assert_eq!(Verdict::ZeroFindings.to_string(), "zero_findings");
    }

    #[test]
    fn test_verdict_display_findings_remain() {
        assert_eq!(Verdict::FindingsRemain.to_string(), "findings_remain");
    }

    #[test]
    fn test_verdict_parse_valid() {
        assert_eq!(Verdict::parse("zero_findings").unwrap(), Verdict::ZeroFindings);
        assert_eq!(Verdict::parse("findings_remain").unwrap(), Verdict::FindingsRemain);
    }

    #[test]
    fn test_verdict_parse_invalid_returns_error() {
        let result = Verdict::parse("unknown_verdict");
        assert!(
            matches!(result, Err(ReviewError::InvalidConcern(_))),
            "expected InvalidConcern for unknown verdict, got {result:?}"
        );
    }

    #[test]
    fn test_verdict_is_zero_findings() {
        assert!(Verdict::ZeroFindings.is_zero_findings());
        assert!(!Verdict::FindingsRemain.is_zero_findings());
    }
}
