//! Review workflow UseCase orchestrators.
//!
//! - `check_approved` and `resolve_escalation`: thin orchestrators using
//!   domain's `TrackReader` / `TrackWriter` ports directly.
//! - `record_round`: delegates to `RecordRoundProtocol` — a genuinely complex
//!   infrastructure protocol (two-phase git index commit) that cannot be
//!   decomposed into simple Load→domain→Save.

use std::path::{Path, PathBuf};

pub use domain::{
    DomainError, ReviewConcern, ReviewEscalationDecision, ReviewGroupName, ReviewStatus, RoundType,
    Timestamp, TrackId, TrackReadError, TrackReader, TrackWriteError, TrackWriter, Verdict,
};

// ---------------------------------------------------------------------------
// Application-level port traits (implemented by infrastructure)
// ---------------------------------------------------------------------------

/// Port for computing the normalised git tree hash of a track's metadata.
pub trait GitHasher {
    /// Computes the normalised tree hash for the given track.
    ///
    /// # Errors
    ///
    /// Returns a human-readable error string on failure.
    fn normalized_hash(&self, items_dir: &Path, track_id: &TrackId) -> Result<String, String>;
}

/// Port for the atomic two-phase record-round protocol.
///
/// This encapsulates the genuinely complex infrastructure protocol
/// (PrivateIndex + git staging + two-phase hash commit) that cannot be
/// decomposed into simple Load → domain → Save.
pub trait RecordRoundProtocol {
    /// Execute the full atomic record-round protocol.
    ///
    /// # Errors
    ///
    /// Returns `RecordRoundProtocolError` on protocol failure.
    #[allow(clippy::too_many_arguments)]
    fn execute(
        &self,
        track_id: &TrackId,
        round_type: RoundType,
        group_name: ReviewGroupName,
        verdict: Verdict,
        concerns: Vec<ReviewConcern>,
        expected_groups: Vec<ReviewGroupName>,
        timestamp: Timestamp,
    ) -> Result<(), RecordRoundProtocolError>;
}

/// Errors returned by [`RecordRoundProtocol::execute`].
#[derive(Debug)]
pub enum RecordRoundProtocolError {
    /// An escalation block prevented the round from being recorded.
    EscalationBlocked(Vec<String>),
    /// The code hash was stale; the review state has been invalidated.
    StaleHash(String),
    /// Any other infrastructure or domain error.
    Other(String),
}

pub struct RecordRoundInput {
    pub round_type: String,
    pub group: String,
    pub verdict: String,
    pub expected_groups: String,
    pub concerns: String,
    pub items_dir: PathBuf,
    pub track_id: String,
    pub timestamp: Timestamp,
}

#[derive(Debug)]
pub enum RecordRoundError {
    EscalationBlocked(Vec<String>),
    Other(String),
}

impl From<String> for RecordRoundError {
    fn from(s: String) -> Self {
        Self::Other(s)
    }
}

/// Parses raw string arguments and delegates to the infrastructure protocol.
///
/// # Errors
///
/// Returns [`RecordRoundError`] on parse failure or protocol error.
pub fn record_round(
    input: RecordRoundInput,
    protocol: &impl RecordRoundProtocol,
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

    let expected_groups: Vec<ReviewGroupName> = input
        .expected_groups
        .split(',')
        .map(|s| {
            let s = s.trim();
            if s.is_empty() {
                return Err(RecordRoundError::Other(
                    "--expected-groups must be a non-empty comma-separated list".to_owned(),
                ));
            }
            ReviewGroupName::try_new(s)
                .map_err(|e| RecordRoundError::Other(format!("invalid group name: {e}")))
        })
        .collect::<Result<_, _>>()?;

    let group_name = ReviewGroupName::try_new(input.group.as_str())
        .map_err(|e| RecordRoundError::Other(format!("invalid group name: {e}")))?;

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

    protocol
        .execute(
            &track_id,
            round_type,
            group_name,
            verdict,
            concerns,
            expected_groups,
            input.timestamp,
        )
        .map_err(|e| match e {
            RecordRoundProtocolError::EscalationBlocked(c) => {
                RecordRoundError::EscalationBlocked(c)
            }
            RecordRoundProtocolError::StaleHash(msg) | RecordRoundProtocolError::Other(msg) => {
                RecordRoundError::Other(msg)
            }
        })
}

// ---------------------------------------------------------------------------
// resolve-escalation: usecase orchestration via domain ports
// ---------------------------------------------------------------------------

pub struct ResolveEscalationInput {
    pub track_id: String,
    pub blocked_concerns: String,
    pub workspace_search_ref: String,
    pub reinvention_check_ref: String,
    pub decision: String,
    pub summary: String,
    pub items_dir: PathBuf,
    pub timestamp: Timestamp,
}

/// Orchestrates resolve-escalation using domain's `TrackWriter::update`.
///
/// Returns the decision string on success for the caller to display.
///
/// # Preconditions
///
/// The caller must verify that `workspace_search_ref` and `reinvention_check_ref`
/// paths exist on disk before calling this function. This usecase does not perform
/// file I/O; existence validation is the responsibility of the CLI / composition root.
///
/// # Errors
///
/// Returns a human-readable error string on failure.
pub fn resolve_escalation(
    input: ResolveEscalationInput,
    writer: &impl TrackWriter,
) -> Result<String, String> {
    use domain::ReviewEscalationResolution;

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

    let timestamp = input.timestamp;

    writer
        .update(&track_id, |track| {
            let review = track.review_mut().as_mut().ok_or_else(|| {
                DomainError::Validation(domain::ValidationError::InvalidTaskId(
                    "no review section in metadata.json".to_owned(),
                ))
            })?;

            if !review.escalation().is_blocked() {
                return Err(DomainError::Validation(domain::ValidationError::InvalidTaskId(
                    "no active escalation block; cannot resolve".to_owned(),
                )));
            }

            let resolution = ReviewEscalationResolution::new(
                blocked_concerns.clone(),
                input.workspace_search_ref.clone(),
                input.reinvention_check_ref.clone(),
                decision,
                input.summary.clone(),
                timestamp.clone(),
            )
            .map_err(|e| {
                DomainError::Validation(domain::ValidationError::InvalidTaskId(e.to_string()))
            })?;

            review.resolve_escalation(resolution).map_err(|e| {
                DomainError::Validation(domain::ValidationError::InvalidTaskId(e.to_string()))
            })?;

            Ok(())
        })
        .map_err(|e| {
            let msg = e.to_string();
            if let Some(inner) = msg.strip_prefix("task id '") {
                if let Some(inner) = inner.strip_suffix("' must match the pattern T<digits>") {
                    return format!("resolve-escalation failed: {inner}");
                }
            }
            format!("resolve-escalation failed: {msg}")
        })?;

    Ok(input.decision)
}

// ---------------------------------------------------------------------------
// check-approved: usecase orchestration via domain ports
// ---------------------------------------------------------------------------

pub struct CheckApprovedInput {
    pub items_dir: PathBuf,
    pub track_id: String,
}

/// Orchestrates check-approved: read track → domain check → invalidate if stale.
///
/// # Errors
///
/// Returns a human-readable error string when the review is not approved.
pub fn check_approved(
    input: CheckApprovedInput,
    reader: &impl TrackReader,
    writer: &impl TrackWriter,
    hasher: &impl GitHasher,
) -> Result<(), String> {
    let track_id =
        TrackId::try_new(&input.track_id).map_err(|e| format!("invalid track id: {e}"))?;

    let code_hash = hasher
        .normalized_hash(&input.items_dir, &track_id)
        .map_err(|e| format!("normalized hash error: {e}"))?;

    let track = reader
        .find(&track_id)
        .map_err(|e| format!("failed to read track: {e}"))?
        .ok_or_else(|| format!("track '{}' not found", track_id.as_ref()))?;

    let review = track.review().ok_or("[BLOCKED] no review section in metadata.json")?;

    // Planning-only tracks with no review activity are always approved.
    if review.status() == ReviewStatus::NotStarted && review.groups().is_empty() {
        return Ok(());
    }

    let mut review_check = review.clone();
    match review_check.check_commit_ready(&code_hash) {
        Ok(()) => Ok(()),
        Err(domain::ReviewError::StaleCodeHash { expected, actual }) => {
            // Persist the invalidation — propagate write errors.
            writer
                .update(&track_id, |track| {
                    if let Some(r) = track.review_mut().as_mut() {
                        // Intentionally ignore the domain error here: we already
                        // know the hash is stale. The purpose is to persist the
                        // invalidation side-effect on the review state.
                        let _ = r.check_commit_ready(&code_hash);
                    }
                    Ok(())
                })
                .map_err(|e| format!("failed to persist invalidation: {e}"))?;
            Err(format!(
                "[BLOCKED] code hash mismatch: recorded against {expected}, \
                 current is {actual} — review.status set to invalidated"
            ))
        }
        Err(e) => Err(format!("[BLOCKED] Review guard failed: {e}")),
    }
}
