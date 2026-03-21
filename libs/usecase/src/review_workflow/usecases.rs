//! Thin UseCase orchestrators for review workflow operations.
//!
//! Each public function parses raw CLI arguments into domain types, validates
//! them, and delegates the infrastructure-level operations to an injected port
//! trait. This keeps the usecase layer free of infrastructure dependencies
//! while still centralising all business logic.
//!
//! ## Port traits
//!
//! Port traits are defined here so that CLI adapters can implement them without
//! creating a circular dependency (`usecase → infrastructure`). The CLI creates
//! concrete adapter structs that implement these traits using infrastructure
//! types and passes them to the usecase functions.

use std::path::PathBuf;

// Re-export domain types that CLI port-adapter implementations need so that
// `review.rs` can stay free of direct `domain::` imports.
pub use domain::{
    ReviewConcern, ReviewEscalationDecision, ReviewGroupName, RoundType, Timestamp, TrackId,
    Verdict,
};

// ---------------------------------------------------------------------------
// Port traits
// ---------------------------------------------------------------------------

/// Port for the atomic record-round protocol (record-round git staging).
///
/// Implementors are responsible for:
/// 1. Computing the pre-update normalised hash.
/// 2. Locking the track document and calling domain `record_round_with_pending`.
/// 3. Staging the pending state, computing h1, setting `code_hash`, staging
///    the final state.
/// 4. Atomically swapping the private index over the real index.
///
/// # Errors
///
/// Returns `RecordRoundStoreError` on escalation blocks, stale hash, or other
/// failures.
pub trait RecordRoundStore {
    /// Execute the full atomic record-round protocol.
    ///
    /// # Errors
    ///
    /// Returns `RecordRoundStoreError` on protocol failure.
    #[allow(clippy::too_many_arguments)]
    fn execute_atomic(
        &self,
        track_id: &TrackId,
        round_type: RoundType,
        group_name: ReviewGroupName,
        verdict: Verdict,
        concerns: Vec<ReviewConcern>,
        expected_groups: Vec<ReviewGroupName>,
        timestamp: Timestamp,
    ) -> Result<(), RecordRoundStoreError>;
}

/// Errors returned by [`RecordRoundStore::execute_atomic`].
#[derive(Debug)]
pub enum RecordRoundStoreError {
    /// An escalation block prevented the round from being recorded.
    EscalationBlocked(Vec<String>),
    /// The code hash was stale; the review state has been invalidated.
    StaleHash(String),
    /// Any other infrastructure or domain error.
    Other(String),
}

/// Port for the resolve-escalation operation.
///
/// Implementors lock the track document, verify evidence artefacts exist, call
/// domain `resolve_escalation`, and persist the result.
pub trait ResolveEscalationStore {
    /// Resolve an active escalation block.
    ///
    /// # Errors
    ///
    /// Returns a human-readable error string on failure.
    #[allow(clippy::too_many_arguments)]
    fn resolve(
        &self,
        track_id: &TrackId,
        blocked_concerns: Vec<ReviewConcern>,
        workspace_search_ref: String,
        reinvention_check_ref: String,
        decision: ReviewEscalationDecision,
        summary: String,
        timestamp: Timestamp,
    ) -> Result<(), String>;
}

/// Port for the check-approved operation.
///
/// Implementors read the track, compute the normalised git hash, call domain
/// `check_commit_ready`, and optionally persist an invalidation.
pub trait CheckApprovedStore {
    /// Check whether the review is approved and the code hash is current.
    ///
    /// # Errors
    ///
    /// Returns a human-readable error string when the review is not approved or
    /// the code hash has gone stale.
    fn check(&self, track_id: &TrackId) -> Result<(), String>;
}

// ---------------------------------------------------------------------------
// RecordRound
// ---------------------------------------------------------------------------

/// Raw string input for the record-round operation.
/// Fields mirror `RecordRoundArgs` without any clap coupling.
pub struct RecordRoundInput {
    /// `"fast"` or `"final"`.
    pub round_type: String,
    /// Review group name (e.g. `"infra-domain"`).
    pub group: String,
    /// Verdict JSON string.
    pub verdict: String,
    /// Comma-separated expected group names.
    pub expected_groups: String,
    /// Comma-separated concern slugs (empty string for zero-findings rounds).
    pub concerns: String,
    /// Path to the track items directory.
    pub items_dir: PathBuf,
    /// Track ID.
    pub track_id: String,
}

/// Errors returned by [`record_round`].
#[derive(Debug)]
pub enum RecordRoundError {
    /// An escalation block prevented recording.
    EscalationBlocked(Vec<String>),
    /// Any other failure.
    Other(String),
}

impl From<String> for RecordRoundError {
    fn from(s: String) -> Self {
        Self::Other(s)
    }
}

/// Record a review round result.
///
/// Parses raw string arguments into domain types, validates them, and delegates
/// the atomic git-staging protocol to `store`.
///
/// # Errors
///
/// Returns [`RecordRoundError`] when argument parsing fails, an escalation is
/// active, or the store reports an error.
pub fn record_round(
    input: RecordRoundInput,
    store: &impl RecordRoundStore,
) -> Result<(), RecordRoundError> {
    use crate::review_workflow::{
        ReviewFinalMessageState, ReviewPayloadVerdict, parse_review_final_message,
    };

    let round_type = match input.round_type.as_str() {
        "fast" => RoundType::Fast,
        "final" => RoundType::Final,
        other => {
            return Err(RecordRoundError::Other(format!(
                "unknown round type: {other} (expected 'fast' or 'final')"
            )));
        }
    };

    let expected_groups_raw: Vec<String> =
        input.expected_groups.split(',').map(|s| s.trim().to_owned()).collect();
    if expected_groups_raw.is_empty() || expected_groups_raw.iter().any(|g| g.is_empty()) {
        return Err(RecordRoundError::Other(
            "--expected-groups must be a non-empty comma-separated list".to_owned(),
        ));
    }
    let expected_groups: Vec<ReviewGroupName> = expected_groups_raw
        .iter()
        .map(|s| {
            ReviewGroupName::try_new(s.as_str())
                .map_err(|e| RecordRoundError::Other(format!("invalid group name: {e}")))
        })
        .collect::<Result<_, _>>()?;

    let group_name = ReviewGroupName::try_new(input.group.as_str())
        .map_err(|e| RecordRoundError::Other(format!("invalid group name: {e}")))?;

    // Parse concerns; deduplicate and sort via BTreeSet for canonical order.
    let concerns: Vec<ReviewConcern> = if input.concerns.trim().is_empty() {
        Vec::new()
    } else {
        let parsed: Result<std::collections::BTreeSet<ReviewConcern>, _> =
            input.concerns.split(',').map(|s| ReviewConcern::try_new(s.trim())).collect();
        parsed
            .map_err(|e| RecordRoundError::Other(format!("invalid concern: {e}")))?
            .into_iter()
            .collect()
    };

    // Parse and semantically validate the verdict JSON.
    let final_message_state = parse_review_final_message(Some(&input.verdict));
    let verdict = match &final_message_state {
        ReviewFinalMessageState::Parsed(payload) => match payload.verdict {
            ReviewPayloadVerdict::ZeroFindings => Verdict::ZeroFindings,
            ReviewPayloadVerdict::FindingsRemain => Verdict::FindingsRemain,
        },
        ReviewFinalMessageState::Missing => {
            return Err(RecordRoundError::Other("--verdict is required".to_owned()));
        }
        ReviewFinalMessageState::Invalid { reason } => {
            return Err(RecordRoundError::Other(format!("invalid --verdict: {reason}")));
        }
    };

    let track_id = TrackId::try_new(&input.track_id)
        .map_err(|e| RecordRoundError::Other(format!("invalid track id: {e}")))?;

    let timestamp_str = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let timestamp = Timestamp::new(timestamp_str)
        .map_err(|e| RecordRoundError::Other(format!("invalid timestamp: {e}")))?;

    store
        .execute_atomic(
            &track_id,
            round_type,
            group_name,
            verdict,
            concerns,
            expected_groups,
            timestamp,
        )
        .map_err(|e| match e {
            RecordRoundStoreError::EscalationBlocked(concerns) => {
                RecordRoundError::EscalationBlocked(concerns)
            }
            RecordRoundStoreError::StaleHash(msg) => RecordRoundError::Other(msg),
            RecordRoundStoreError::Other(msg) => RecordRoundError::Other(msg),
        })
}

// ---------------------------------------------------------------------------
// ResolveEscalation
// ---------------------------------------------------------------------------

/// Raw string input for the resolve-escalation operation.
pub struct ResolveEscalationInput {
    /// Track ID.
    pub track_id: String,
    /// Comma-separated blocked concern slugs.
    pub blocked_concerns: String,
    /// Path to workspace search artefact (must exist).
    pub workspace_search_ref: String,
    /// Path to reinvention check artefact (must exist).
    pub reinvention_check_ref: String,
    /// Decision string: `"adopt_workspace"`, `"adopt_crate"`, or
    /// `"continue_self"`.
    pub decision: String,
    /// Summary of the decision rationale.
    pub summary: String,
    /// Path to the track items directory.
    pub items_dir: PathBuf,
}

/// Resolve an active review escalation block.
///
/// Parses raw string arguments into domain types and delegates to `store`.
///
/// # Errors
///
/// Returns a human-readable error string on failure.
pub fn resolve_escalation(
    input: ResolveEscalationInput,
    store: &impl ResolveEscalationStore,
) -> Result<(), String> {
    let decision = match input.decision.as_str() {
        "adopt_workspace" => ReviewEscalationDecision::AdoptWorkspaceSolution,
        "adopt_crate" => ReviewEscalationDecision::AdoptExternalCrate,
        "continue_self" => ReviewEscalationDecision::ContinueSelfImplementation,
        other => {
            return Err(format!(
                "unknown decision: {other}. Use: adopt_workspace, adopt_crate, or continue_self"
            ));
        }
    };

    // Parse blocked concerns; deduplicate and sort via BTreeSet.
    let blocked_concerns: Vec<ReviewConcern> = {
        let parsed: Result<std::collections::BTreeSet<_>, _> = input
            .blocked_concerns
            .split(',')
            .map(|s| {
                ReviewConcern::try_new(s.trim())
                    .map_err(|e| format!("invalid blocked concern: {e}"))
            })
            .collect();
        parsed?.into_iter().collect()
    };

    let track_id =
        TrackId::try_new(&input.track_id).map_err(|e| format!("invalid track id: {e}"))?;

    let timestamp_str = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let timestamp = Timestamp::new(timestamp_str).map_err(|e| format!("invalid timestamp: {e}"))?;

    store.resolve(
        &track_id,
        blocked_concerns,
        input.workspace_search_ref,
        input.reinvention_check_ref,
        decision,
        input.summary,
        timestamp,
    )
}

// ---------------------------------------------------------------------------
// CheckApproved
// ---------------------------------------------------------------------------

/// Raw string input for the check-approved operation.
pub struct CheckApprovedInput {
    /// Path to the track items directory.
    pub items_dir: PathBuf,
    /// Track ID.
    pub track_id: String,
}

/// Verify that the review is approved and the code hash is current.
///
/// Parses the track ID and delegates to `store`.
///
/// # Errors
///
/// Returns a human-readable error string when the review is not approved,
/// the code hash is stale, or any I/O error occurs.
pub fn check_approved(
    input: CheckApprovedInput,
    store: &impl CheckApprovedStore,
) -> Result<(), String> {
    let track_id =
        TrackId::try_new(&input.track_id).map_err(|e| format!("invalid track id: {e}"))?;

    store.check(&track_id)
}
