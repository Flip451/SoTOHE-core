//! Review cycle types: ReviewJson, ReviewCycle, CycleGroupState, and related types.

pub mod round_types;

pub use round_types::{
    GroupRound, GroupRoundOutcome, GroupRoundVerdict, NonEmptyFindings, StoredFinding,
};

use std::collections::BTreeMap;

use crate::Timestamp;
use crate::ids::ReviewGroupName;
use crate::review::types::ApprovedHead;

// ── CycleError ───────────────────────────────────────────────────────────────

/// Errors from review cycle operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CycleError {
    #[error("cycle already exists")]
    CycleAlreadyExists,
    #[error("no current cycle")]
    NoCurrentCycle,
    #[error("missing mandatory 'other' group")]
    MissingOtherGroup,
    #[error("internal error: {0}")]
    Internal(String),
}

// ── ReviewStalenessReason ────────────────────────────────────────────────────

/// Reason why a review cycle is considered stale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewStalenessReason {
    PolicyChanged,
    PartitionDrifted,
    HashMismatch,
}

// ── CycleGroupState ──────────────────────────────────────────────────────────

/// State of a single review group within a cycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CycleGroupState {
    scope: Vec<String>,
    rounds: Vec<GroupRound>,
}

impl CycleGroupState {
    /// Creates a group state with scope and empty rounds.
    #[must_use]
    pub fn new(scope: Vec<String>) -> Self {
        Self { scope, rounds: Vec::new() }
    }

    /// Creates a group state with scope and rounds.
    #[must_use]
    pub fn with_rounds(scope: Vec<String>, rounds: Vec<GroupRound>) -> Self {
        Self { scope, rounds }
    }

    /// Pushes a round onto the group state.
    pub fn record_round(&mut self, round: GroupRound) {
        self.rounds.push(round);
    }

    /// Returns the scope files.
    #[must_use]
    pub fn scope(&self) -> &[String] {
        &self.scope
    }

    /// Returns the rounds.
    #[must_use]
    pub fn rounds(&self) -> &[GroupRound] {
        &self.rounds
    }

    /// Returns the latest round of the given type.
    #[must_use]
    pub fn latest_round(&self, round_type: crate::review::types::RoundType) -> Option<&GroupRound> {
        self.rounds.iter().rev().find(|r| r.round_type() == round_type)
    }
}

// ── ReviewCycle ──────────────────────────────────────────────────────────────

/// A single review cycle containing per-group round state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewCycle {
    cycle_id: String,
    started_at: Timestamp,
    base_ref: String,
    base_policy_hash: String,
    policy_hash: String,
    approved_head: Option<ApprovedHead>,
    groups: BTreeMap<ReviewGroupName, CycleGroupState>,
}

impl ReviewCycle {
    /// Creates a new review cycle.
    ///
    /// # Errors
    /// Returns `CycleError::MissingOtherGroup` if the groups map does not contain "other".
    pub fn new(
        cycle_id: impl Into<String>,
        started_at: Timestamp,
        base_ref: impl Into<String>,
        base_policy_hash: impl Into<String>,
        policy_hash: impl Into<String>,
        groups: BTreeMap<ReviewGroupName, CycleGroupState>,
    ) -> Result<Self, CycleError> {
        let other_key = ReviewGroupName::try_new("other")
            .map_err(|_| CycleError::Internal("failed to create 'other' key".to_owned()))?;
        if !groups.contains_key(&other_key) {
            return Err(CycleError::MissingOtherGroup);
        }
        Ok(Self {
            cycle_id: cycle_id.into(),
            started_at,
            base_ref: base_ref.into(),
            base_policy_hash: base_policy_hash.into(),
            policy_hash: policy_hash.into(),
            approved_head: None,
            groups,
        })
    }

    /// Returns the cycle ID.
    #[must_use]
    pub fn cycle_id(&self) -> &str {
        &self.cycle_id
    }

    /// Returns the started_at timestamp.
    #[must_use]
    pub fn started_at(&self) -> &Timestamp {
        &self.started_at
    }

    /// Returns the base ref.
    #[must_use]
    pub fn base_ref(&self) -> &str {
        &self.base_ref
    }

    /// Returns the base policy hash.
    #[must_use]
    pub fn base_policy_hash(&self) -> &str {
        &self.base_policy_hash
    }

    /// Returns the policy hash.
    #[must_use]
    pub fn policy_hash(&self) -> &str {
        &self.policy_hash
    }

    /// Returns the approved head, if any.
    #[must_use]
    pub fn approved_head(&self) -> Option<&ApprovedHead> {
        self.approved_head.as_ref()
    }

    /// Sets the approved head.
    pub fn set_approved_head(&mut self, head: ApprovedHead) {
        self.approved_head = Some(head);
    }

    /// Returns the groups map.
    #[must_use]
    pub fn groups(&self) -> &BTreeMap<ReviewGroupName, CycleGroupState> {
        &self.groups
    }

    /// Returns a specific group by name.
    #[must_use]
    pub fn group(&self, name: &ReviewGroupName) -> Option<&CycleGroupState> {
        self.groups.get(name)
    }

    /// Returns a mutable reference to a specific group by name.
    pub fn group_mut(&mut self, name: &ReviewGroupName) -> Option<&mut CycleGroupState> {
        self.groups.get_mut(name)
    }

    /// Returns an iterator over group names.
    pub fn group_names(&self) -> impl Iterator<Item = &ReviewGroupName> {
        self.groups.keys()
    }
}

// ── ReviewJson ───────────────────────────────────────────────────────────────

/// The full review.json aggregate: schema version + ordered list of cycles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewJson {
    schema_version: u32,
    cycles: Vec<ReviewCycle>,
}

impl ReviewJson {
    /// Creates an empty review.json aggregate.
    #[must_use]
    pub fn new() -> Self {
        Self { schema_version: 1, cycles: Vec::new() }
    }

    /// Creates a review.json from parts (used by codec).
    ///
    /// # Errors
    /// Returns `CycleError` if cycles is empty and validation fails.
    pub fn from_parts(schema_version: u32, cycles: Vec<ReviewCycle>) -> Result<Self, CycleError> {
        Ok(Self { schema_version, cycles })
    }

    /// Returns the schema version.
    #[must_use]
    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns all cycles.
    #[must_use]
    pub fn cycles(&self) -> &[ReviewCycle] {
        &self.cycles
    }

    /// Returns `true` if there are no cycles (NoCycle state).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cycles.is_empty()
    }

    /// Returns the current (last) cycle, if any.
    #[must_use]
    pub fn current_cycle(&self) -> Option<&ReviewCycle> {
        self.cycles.last()
    }

    /// Returns a mutable reference to the current (last) cycle, if any.
    pub fn current_cycle_mut(&mut self) -> Option<&mut ReviewCycle> {
        self.cycles.last_mut()
    }

    /// Starts a new cycle.
    ///
    /// # Errors
    /// Returns `CycleError::MissingOtherGroup` if the groups don't include "other".
    pub fn start_cycle(
        &mut self,
        cycle_id: impl Into<String>,
        started_at: Timestamp,
        base_ref: impl Into<String>,
        base_policy_hash: impl Into<String>,
        policy_hash: impl Into<String>,
        groups: BTreeMap<ReviewGroupName, CycleGroupState>,
    ) -> Result<(), CycleError> {
        let cycle = ReviewCycle::new(
            cycle_id,
            started_at,
            base_ref,
            base_policy_hash,
            policy_hash,
            groups,
        )?;
        self.cycles.push(cycle);
        Ok(())
    }
}

impl Default for ReviewJson {
    fn default() -> Self {
        Self::new()
    }
}
