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

/// Typed record-round entrypoint for internal callers (e.g., auto-record).
///
/// Accepts parsed domain types directly, avoiding string round-trip.
/// The existing `record_round()` remains as the CLI string-adapter.
///
/// # Errors
///
/// Returns `RecordRoundError` on protocol failure or escalation block.
/// Typed record-round entrypoint for internal callers (e.g., auto-record).
///
/// Accepts parsed domain types directly, avoiding string round-trip.
/// The caller constructs `RecordRoundProtocol` with the correct `items_dir`.
///
/// # Errors
///
/// Returns `RecordRoundError` on protocol failure or escalation block.
#[allow(clippy::too_many_arguments)]
pub fn record_round_typed(
    track_id: TrackId,
    round_type: RoundType,
    group_name: ReviewGroupName,
    verdict: Verdict,
    concerns: Vec<ReviewConcern>,
    expected_groups: Vec<ReviewGroupName>,
    timestamp: Timestamp,
    protocol: &impl RecordRoundProtocol,
) -> Result<(), RecordRoundError> {
    if expected_groups.is_empty() {
        return Err(RecordRoundError::Other("expected_groups must not be empty".to_owned()));
    }

    protocol
        .execute(&track_id, round_type, group_name, verdict, concerns, expected_groups, timestamp)
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
    /// When `true`, the staged diff contains only planning-only files (track docs,
    /// config, etc.). The NotStarted+empty-groups fast-path is only allowed in this
    /// case. When `false`, code files are staged and a completed review is required.
    pub planning_only: bool,
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

    // Fail-closed: missing review section always blocks, regardless of planning_only.
    // This prevents bypassing the review guard by deleting the review object from metadata.
    let review = track.review().ok_or("[BLOCKED] no review section in metadata.json")?;

    // Planning-only fast-path: when no review activity exists yet, bypass for
    // planning-only commits. Once a review has been started, it must be completed
    // to Approved even for planning-only commits.
    if review.status() == ReviewStatus::NotStarted && review.groups().is_empty() {
        if input.planning_only {
            return Ok(());
        }
        return Err(
            "[BLOCKED] Review not started: code files are staged but no review has been run. \
             Run /track:review first."
                .to_string(),
        );
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    #![allow(clippy::panic)]
    #![allow(clippy::expect_used)]

    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Mutex;

    use super::*;

    // --- Mock GitHasher ---

    struct FixedHasher(String);

    impl GitHasher for FixedHasher {
        fn normalized_hash(
            &self,
            _items_dir: &Path,
            _track_id: &TrackId,
        ) -> Result<String, String> {
            Ok(self.0.clone())
        }
    }

    // --- Minimal in-memory TrackReader + TrackWriter ---

    #[derive(Default)]
    struct MemStore {
        tracks: Mutex<HashMap<String, domain::TrackMetadata>>,
    }

    impl TrackReader for MemStore {
        fn find(&self, id: &TrackId) -> Result<Option<domain::TrackMetadata>, TrackReadError> {
            Ok(self.tracks.lock().unwrap().get(id.as_ref()).cloned())
        }
    }

    impl TrackWriter for MemStore {
        fn save(&self, track: &domain::TrackMetadata) -> Result<(), TrackWriteError> {
            self.tracks.lock().unwrap().insert(track.id().as_ref().to_owned(), track.clone());
            Ok(())
        }

        fn update<F>(
            &self,
            id: &TrackId,
            mutate: F,
        ) -> Result<domain::TrackMetadata, TrackWriteError>
        where
            F: FnOnce(&mut domain::TrackMetadata) -> Result<(), DomainError>,
        {
            let mut tracks = self.tracks.lock().unwrap();
            let track = tracks.get_mut(id.as_ref()).ok_or_else(|| {
                TrackWriteError::from(domain::RepositoryError::TrackNotFound(
                    id.as_ref().to_owned(),
                ))
            })?;
            mutate(track)?;
            Ok(track.clone())
        }
    }

    // --- Helper: build a minimal track with review state ---

    fn make_track_with_review(
        track_id: &str,
        review: domain::ReviewState,
    ) -> domain::TrackMetadata {
        let tid = TrackId::try_new(track_id).unwrap();
        let task_id = domain::TaskId::try_new("T1").unwrap();
        let task = domain::TrackTask::new(task_id.clone(), "task").unwrap();
        let section = domain::PlanSection::new("S1", "section", vec![], vec![task_id]).unwrap();
        let plan = domain::PlanView::new(vec!["summary".to_string()], vec![section]);
        let mut track =
            domain::TrackMetadata::new(tid, "test track", vec![task], plan, None).unwrap();
        track.set_review(Some(review));
        track
    }

    #[test]
    fn check_approved_planning_only_true_with_not_started_passes() {
        let store = MemStore::default();
        let track = make_track_with_review("test-track", domain::ReviewState::new());
        store.save(&track).unwrap();

        let hasher = FixedHasher("abc123".to_string());
        let input = CheckApprovedInput {
            items_dir: PathBuf::from("track/items"),
            track_id: "test-track".to_string(),
            planning_only: true,
        };

        let result = check_approved(input, &store, &store, &hasher);
        assert!(result.is_ok(), "planning_only=true should bypass: {result:?}");
    }

    #[test]
    fn check_approved_planning_only_false_with_not_started_is_blocked() {
        let store = MemStore::default();
        let track = make_track_with_review("test-track", domain::ReviewState::new());
        store.save(&track).unwrap();

        let hasher = FixedHasher("abc123".to_string());
        let input = CheckApprovedInput {
            items_dir: PathBuf::from("track/items"),
            track_id: "test-track".to_string(),
            planning_only: false,
        };

        let result = check_approved(input, &store, &store, &hasher);
        assert!(result.is_err(), "planning_only=false should block");
        let err = result.unwrap_err();
        assert!(err.contains("[BLOCKED]"), "error should indicate blocked: {err}");
    }

    #[test]
    fn check_approved_planning_only_false_with_approved_review_passes() {
        let code_hash = "abc123";
        let review = domain::ReviewState::with_fields(
            ReviewStatus::Approved,
            domain::CodeHash::computed(code_hash).unwrap(),
            HashMap::new(),
            domain::ReviewEscalationState::new(),
        );
        let store = MemStore::default();
        let track = make_track_with_review("test-track", review);
        store.save(&track).unwrap();

        let hasher = FixedHasher(code_hash.to_string());
        let input = CheckApprovedInput {
            items_dir: PathBuf::from("track/items"),
            track_id: "test-track".to_string(),
            planning_only: false,
        };

        let result = check_approved(input, &store, &store, &hasher);
        assert!(result.is_ok(), "approved review should pass: {result:?}");
    }

    #[test]
    fn check_approved_planning_only_with_started_review_still_requires_approval() {
        // Once review is started, even planning_only commits must have Approved status.
        let review = domain::ReviewState::with_fields(
            ReviewStatus::Approved,
            domain::CodeHash::computed("old-hash").unwrap(),
            HashMap::new(),
            domain::ReviewEscalationState::new(),
        );
        let store = MemStore::default();
        let track = make_track_with_review("test-track", review);
        store.save(&track).unwrap();

        let hasher = FixedHasher("new-hash".to_string());
        let input = CheckApprovedInput {
            items_dir: PathBuf::from("track/items"),
            track_id: "test-track".to_string(),
            planning_only: true,
        };

        let result = check_approved(input, &store, &store, &hasher);
        assert!(result.is_err(), "started review + stale hash should block even planning_only");
    }

    #[test]
    fn check_approved_missing_review_section_always_blocks() {
        // Fail-closed: missing review section blocks even planning_only.
        // Prevents bypass by deleting the review object from metadata.
        let store = MemStore::default();
        let tid = TrackId::try_new("legacy-track").unwrap();
        let task_id = domain::TaskId::try_new("T1").unwrap();
        let task = domain::TrackTask::new(task_id.clone(), "task").unwrap();
        let section = domain::PlanSection::new("S1", "section", vec![], vec![task_id]).unwrap();
        let plan = domain::PlanView::new(vec!["summary".to_string()], vec![section]);
        let track = domain::TrackMetadata::new(tid, "legacy", vec![task], plan, None).unwrap();
        store.save(&track).unwrap();

        let hasher = FixedHasher("abc123".to_string());

        // planning_only=true still blocked
        let input = CheckApprovedInput {
            items_dir: PathBuf::from("track/items"),
            track_id: "legacy-track".to_string(),
            planning_only: true,
        };
        let result = check_approved(input, &store, &store, &hasher);
        assert!(result.is_err(), "missing review should block even planning_only");

        // planning_only=false also blocked
        let input = CheckApprovedInput {
            items_dir: PathBuf::from("track/items"),
            track_id: "legacy-track".to_string(),
            planning_only: false,
        };
        let result = check_approved(input, &store, &store, &hasher);
        assert!(result.is_err(), "missing review should block code commits");
    }

    // ---------------------------------------------------------------------------
    // Stub for RecordRoundProtocol
    // ---------------------------------------------------------------------------

    /// A configurable stub that returns a preset result from `execute`.
    struct StubProtocol {
        result: std::sync::Mutex<Option<Result<(), RecordRoundProtocolError>>>,
        /// Captures the arguments passed to the last `execute` call.
        last_call: std::sync::Mutex<Option<RecordRoundProtocolCallArgs>>,
    }

    struct RecordRoundProtocolCallArgs {
        track_id: TrackId,
        round_type: RoundType,
        group_name: ReviewGroupName,
        verdict: Verdict,
        concerns: Vec<ReviewConcern>,
        expected_groups: Vec<ReviewGroupName>,
    }

    impl StubProtocol {
        fn returning_ok() -> Self {
            Self {
                result: std::sync::Mutex::new(Some(Ok(()))),
                last_call: std::sync::Mutex::new(None),
            }
        }

        fn returning_err(e: RecordRoundProtocolError) -> Self {
            Self {
                result: std::sync::Mutex::new(Some(Err(e))),
                last_call: std::sync::Mutex::new(None),
            }
        }

        fn last_call(&self) -> std::sync::MutexGuard<'_, Option<RecordRoundProtocolCallArgs>> {
            self.last_call.lock().unwrap()
        }
    }

    impl RecordRoundProtocol for StubProtocol {
        fn execute(
            &self,
            track_id: &TrackId,
            round_type: RoundType,
            group_name: ReviewGroupName,
            verdict: Verdict,
            concerns: Vec<ReviewConcern>,
            expected_groups: Vec<ReviewGroupName>,
            _timestamp: Timestamp,
        ) -> Result<(), RecordRoundProtocolError> {
            *self.last_call.lock().unwrap() = Some(RecordRoundProtocolCallArgs {
                track_id: track_id.clone(),
                round_type,
                group_name,
                verdict,
                concerns,
                expected_groups,
            });
            self.result.lock().unwrap().take().expect("StubProtocol called more than once")
        }
    }

    // ---------------------------------------------------------------------------
    // Helper constructors
    // ---------------------------------------------------------------------------

    fn make_track_id(s: &str) -> TrackId {
        // TrackId requires lowercase slug format (e.g. "t001", not "T001").
        TrackId::try_new(s).unwrap()
    }

    fn make_group(s: &str) -> ReviewGroupName {
        ReviewGroupName::try_new(s).unwrap()
    }

    fn make_concern(s: &str) -> ReviewConcern {
        ReviewConcern::try_new(s).unwrap()
    }

    fn make_timestamp() -> Timestamp {
        Timestamp::new("2026-03-25T12:00:00Z").unwrap()
    }

    // ---------------------------------------------------------------------------
    // Tests for record_round_typed
    // ---------------------------------------------------------------------------

    #[test]
    fn test_record_round_typed_zero_findings_delegates_to_protocol() {
        let track_id = make_track_id("t001");
        let group = make_group("codex");
        let expected_groups = vec![make_group("codex")];
        let concerns: Vec<ReviewConcern> = vec![];
        let ts = make_timestamp();
        let stub = StubProtocol::returning_ok();
        let result = record_round_typed(
            track_id.clone(),
            RoundType::Fast,
            group.clone(),
            Verdict::ZeroFindings,
            concerns.clone(),
            expected_groups.clone(),
            ts,
            &stub,
        );

        assert!(result.is_ok(), "zero_findings should succeed: {result:?}");
        let call = stub.last_call();
        let args = call.as_ref().expect("protocol.execute should have been called");
        assert_eq!(args.track_id.as_ref(), "t001");
        assert_eq!(args.round_type, RoundType::Fast);
        assert_eq!(args.group_name, group);
        assert_eq!(args.verdict, Verdict::ZeroFindings);
        assert!(args.concerns.is_empty());
        assert_eq!(args.expected_groups, expected_groups);
    }

    #[test]
    fn test_record_round_typed_findings_remain_delegates_to_protocol() {
        let track_id = make_track_id("t002");
        let group = make_group("codex");
        let expected_groups = vec![make_group("codex")];
        let concerns = vec![make_concern("domain.review"), make_concern("infra.git")];
        let ts = make_timestamp();
        let stub = StubProtocol::returning_ok();
        let result = record_round_typed(
            track_id.clone(),
            RoundType::Final,
            group.clone(),
            Verdict::FindingsRemain,
            concerns.clone(),
            expected_groups.clone(),
            ts,
            &stub,
        );

        assert!(result.is_ok(), "findings_remain with concerns should succeed: {result:?}");
        let call = stub.last_call();
        let args = call.as_ref().expect("protocol.execute should have been called");
        assert_eq!(args.track_id.as_ref(), "t002");
        assert_eq!(args.round_type, RoundType::Final);
        assert_eq!(args.verdict, Verdict::FindingsRemain);
        assert_eq!(args.concerns.len(), 2);
        assert!(args.concerns.contains(&make_concern("domain.review")));
        assert!(args.concerns.contains(&make_concern("infra.git")));
    }

    #[test]
    fn test_record_round_typed_escalation_blocked_returns_error() {
        let track_id = make_track_id("t003");
        let group = make_group("codex");
        let expected_groups = vec![make_group("codex")];
        let blocked_concerns = vec!["domain.review".to_string(), "infra.git".to_string()];
        let ts = make_timestamp();

        let stub = StubProtocol::returning_err(RecordRoundProtocolError::EscalationBlocked(
            blocked_concerns.clone(),
        ));
        let result = record_round_typed(
            track_id,
            RoundType::Fast,
            group,
            Verdict::FindingsRemain,
            vec![],
            expected_groups,
            ts,
            &stub,
        );

        assert!(result.is_err(), "escalation block should propagate as error");
        match result.unwrap_err() {
            RecordRoundError::EscalationBlocked(concerns) => {
                assert_eq!(concerns, blocked_concerns);
            }
            other => panic!("expected EscalationBlocked, got {other:?}"),
        }
    }

    #[test]
    fn test_record_round_typed_stale_hash_returns_other_error() {
        let track_id = make_track_id("t004");
        let group = make_group("codex");
        let expected_groups = vec![make_group("codex")];
        let ts = make_timestamp();

        let stub = StubProtocol::returning_err(RecordRoundProtocolError::StaleHash(
            "hash mismatch: expected abc, got def".to_string(),
        ));
        let result = record_round_typed(
            track_id,
            RoundType::Fast,
            group,
            Verdict::ZeroFindings,
            vec![],
            expected_groups,
            ts,
            &stub,
        );

        assert!(result.is_err(), "stale hash should propagate as error");
        match result.unwrap_err() {
            RecordRoundError::Other(msg) => {
                assert!(
                    msg.contains("hash mismatch"),
                    "error message should contain original message: {msg}"
                );
            }
            other => panic!("expected Other, got {other:?}"),
        }
    }

    #[test]
    fn test_record_round_typed_empty_expected_groups_returns_error() {
        let track_id = make_track_id("t005");
        let group = make_group("codex");
        let ts = make_timestamp();

        let stub = StubProtocol::returning_ok();
        let result = record_round_typed(
            track_id,
            RoundType::Fast,
            group,
            Verdict::ZeroFindings,
            vec![],
            vec![], // empty expected_groups
            ts,
            &stub,
        );

        assert!(result.is_err());
        match result.unwrap_err() {
            RecordRoundError::Other(msg) => {
                assert!(
                    msg.contains("expected_groups"),
                    "error should mention expected_groups: {msg}"
                );
            }
            other => panic!("expected Other, got {other:?}"),
        }
    }
}
