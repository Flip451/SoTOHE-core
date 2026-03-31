//! Review workflow UseCase orchestrators.
//!
//! - `check_approved` and `resolve_escalation`: thin orchestrators using
//!   domain's `TrackReader` / `TrackWriter` ports directly.
//! - `record_round`: delegates to `RecordRoundProtocol` — a genuinely complex
//!   infrastructure protocol (two-phase git index commit) that cannot be
//!   decomposed into simple Load→domain→Save.

use std::path::PathBuf;

pub use domain::{
    DomainError, ReviewConcern, ReviewEscalationDecision, ReviewGroupName, ReviewStatus, RoundType,
    Timestamp, TrackId, TrackReadError, TrackReader, TrackWriteError, TrackWriter, Verdict,
};

// ---------------------------------------------------------------------------
// Application-level port traits (implemented by infrastructure)
// ---------------------------------------------------------------------------

/// Port for computing review-scope content hashes.
pub trait GitHasher {
    /// Computes a deterministic hash over the worktree file contents for the given
    /// scope files (repo-relative paths).
    ///
    /// The hash is computed from a sorted manifest of `(path, sha256_of_content)`
    /// entries read from the worktree. Empty scope returns a deterministic empty hash.
    ///
    /// # Errors
    ///
    /// Returns a human-readable error string on failure.
    fn group_scope_hash(&self, scope: &[String]) -> Result<String, String>;
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
    /// Current partition snapshot (policy hashes + group partition) for staleness
    /// detection. When `Some`, `check_cycle_approved` performs full staleness +
    /// partition drift + per-group hash verification. When `None`, falls back to
    /// per-group hash check only (no staleness detection).
    pub current_snapshot: Option<crate::review_workflow::groups::ReviewPartitionSnapshot>,
}

/// Orchestrates check-approved: read review.json → check cycle approval.
///
/// When review.json does not exist (no review cycle started), planning-only
/// commits are allowed but code commits are blocked.
///
/// # Errors
///
/// Returns a human-readable error string when the review is not approved.
pub fn check_approved(
    input: CheckApprovedInput,
    reader: &impl TrackReader,
    _writer: &impl TrackWriter,
    hasher: &impl GitHasher,
    review_reader: &impl domain::ReviewJsonReader,
) -> Result<(), String> {
    let track_id =
        TrackId::try_new(&input.track_id).map_err(|e| format!("invalid track id: {e}"))?;

    // Verify track exists and check legacy escalation gate.
    let track = reader
        .find(&track_id)
        .map_err(|e| format!("failed to read track: {e}"))?
        .ok_or_else(|| format!("track '{}' not found", track_id.as_ref()))?;

    // Fail-closed: if metadata.json has an active escalation block, reject.
    // Escalation state has not yet been migrated to review.json (T005/T006).
    if let Some(review_state) = track.review() {
        if let domain::EscalationPhase::Blocked(block) = review_state.escalation().phase() {
            let concerns: Vec<_> = block.concerns().iter().map(|c| c.as_ref().to_owned()).collect();
            return Err(format!(
                "[BLOCKED] Review escalation active for concerns: {concerns:?}. \
                 Run `sotp review resolve-escalation` first."
            ));
        }
    }

    // Read review.json (the new cycle-based review state).
    let review_json = review_reader
        .find_review(&track_id)
        .map_err(|e| format!("failed to read review.json: {e}"))?;

    // No review.json → review never started. Allow commit (WF-66: user may
    // choose PR-based review instead of local review).
    let Some(review) = review_json else {
        return Ok(());
    };

    // No current cycle → same as not started.
    let Some(cycle) = review.current_cycle() else {
        return Ok(());
    };

    // Delegate to check_cycle_approved which performs full verification:
    // staleness (policy + partition + hash) + per-group fast/final check.
    if let Some(ref snapshot) = input.current_snapshot {
        // Compute hashes from CURRENT partition (not frozen cycle scope) so that
        // files added to a group after cycle start are reflected in the hash,
        // causing a mismatch with the reviewed round hash.
        let mut current_group_hashes = std::collections::BTreeMap::new();
        for (group_name, paths) in snapshot.partition().groups() {
            let scope: Vec<String> = paths.iter().map(|p| p.as_str().to_owned()).collect();
            let group_hash = hasher.group_scope_hash(&scope).map_err(|e| {
                format!("group scope hash error for '{}': {e}", group_name.as_ref())
            })?;
            current_group_hashes.insert(group_name.clone(), group_hash);
        }

        match crate::review_workflow::cycle::check_cycle_approved(
            &review,
            snapshot,
            &current_group_hashes,
        ) {
            crate::review_workflow::cycle::CheckCycleApprovedResult::Approved => Ok(()),
            crate::review_workflow::cycle::CheckCycleApprovedResult::NotApproved(reason) => {
                Err(format!("[BLOCKED] {reason:?}"))
            }
        }
    } else {
        // Fallback when no snapshot provided: compute hashes from cycle's frozen scope.
        let mut current_group_hashes = std::collections::BTreeMap::new();
        for (group_name, group_state) in cycle.groups() {
            let group_hash = hasher.group_scope_hash(group_state.scope()).map_err(|e| {
                format!("group scope hash error for '{}': {e}", group_name.as_ref())
            })?;
            current_group_hashes.insert(group_name.clone(), group_hash);
        }
        if review.current_cycle().is_some_and(|c| c.all_groups_approved(&current_group_hashes)) {
            Ok(())
        } else {
            Err("[BLOCKED] Review cycle not fully approved (missing fast+final zero_findings for all groups)".to_string())
        }
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

    impl FixedHasher {
        fn new(hash: &str) -> Self {
            Self(hash.to_owned())
        }
    }

    impl GitHasher for FixedHasher {
        fn group_scope_hash(&self, _scope: &[String]) -> Result<String, String> {
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

    // --- Mock ReviewJsonReader ---

    #[derive(Default)]
    struct MemReviewStore {
        reviews: Mutex<HashMap<String, domain::ReviewJson>>,
    }

    impl domain::ReviewJsonReader for MemReviewStore {
        fn find_review(&self, id: &TrackId) -> Result<Option<domain::ReviewJson>, TrackReadError> {
            Ok(self.reviews.lock().unwrap().get(id.as_ref()).cloned())
        }
    }

    impl MemReviewStore {
        fn save_review(&self, id: &TrackId, review: domain::ReviewJson) {
            self.reviews.lock().unwrap().insert(id.as_ref().to_owned(), review);
        }
    }

    // --- Helper: build a minimal track (no review in metadata) ---

    fn make_track(track_id: &str) -> domain::TrackMetadata {
        let tid = TrackId::try_new(track_id).unwrap();
        let task_id = domain::TaskId::try_new("T1").unwrap();
        let task = domain::TrackTask::new(task_id.clone(), "task").unwrap();
        let section = domain::PlanSection::new("S1", "section", vec![], vec![task_id]).unwrap();
        let plan = domain::PlanView::new(vec!["summary".to_string()], vec![section]);
        domain::TrackMetadata::new(tid, "test track", vec![task], plan, None).unwrap()
    }

    fn make_review_with_approved_cycle(code_hash: &str) -> domain::ReviewJson {
        use std::collections::BTreeMap;
        let grn = domain::ReviewGroupName::try_new("other").unwrap();
        let ts = domain::Timestamp::new("2026-03-30T08:00:00Z").unwrap();
        let mut groups = BTreeMap::new();
        let mut gs = domain::CycleGroupState::new(vec![]);
        let fast = domain::GroupRound::success(
            domain::RoundType::Fast,
            ts.clone(),
            code_hash,
            domain::GroupRoundVerdict::ZeroFindings,
        )
        .unwrap();
        gs.record_round(fast);
        let fin = domain::GroupRound::success(
            domain::RoundType::Final,
            ts.clone(),
            code_hash,
            domain::GroupRoundVerdict::ZeroFindings,
        )
        .unwrap();
        gs.record_round(fin);
        groups.insert(grn, gs);
        let mut rj = domain::ReviewJson::new();
        rj.start_cycle("c1", ts, "main", "sha256:none", "sha256:none", groups).unwrap();
        rj
    }

    #[test]
    fn check_approved_planning_only_true_with_no_review_json_passes() {
        let store = MemStore::default();
        let track = make_track("test-track");
        store.save(&track).unwrap();
        let review_store = MemReviewStore::default();

        let hasher = FixedHasher::new("abc123");
        let input = CheckApprovedInput {
            items_dir: PathBuf::from("track/items"),
            track_id: "test-track".to_string(),
            planning_only: true,
            current_snapshot: None,
        };

        let result = check_approved(input, &store, &store, &hasher, &review_store);
        assert!(result.is_ok(), "planning_only=true + no review.json should pass: {result:?}");
    }

    #[test]
    fn check_approved_no_review_json_allows_commit() {
        // WF-66: review never started (NotStarted) should allow commit.
        // User may choose PR-based review instead of local review.
        let store = MemStore::default();
        let track = make_track("test-track");
        store.save(&track).unwrap();
        let review_store = MemReviewStore::default();

        let hasher = FixedHasher::new("abc123");
        let input = CheckApprovedInput {
            items_dir: PathBuf::from("track/items"),
            track_id: "test-track".to_string(),
            planning_only: false,
            current_snapshot: None,
        };

        let result = check_approved(input, &store, &store, &hasher, &review_store);
        assert!(result.is_ok(), "no review.json (NotStarted) should allow commit: {result:?}");
    }

    #[test]
    fn check_approved_with_approved_cycle_passes() {
        let code_hash = "rvw1:sha256:abc123";
        let store = MemStore::default();
        let track = make_track("test-track");
        store.save(&track).unwrap();
        let review_store = MemReviewStore::default();
        let track_id = TrackId::try_new("test-track").unwrap();
        review_store.save_review(&track_id, make_review_with_approved_cycle(code_hash));

        let hasher = FixedHasher::new(code_hash);
        let input = CheckApprovedInput {
            items_dir: PathBuf::from("track/items"),
            track_id: "test-track".to_string(),
            planning_only: false,
            current_snapshot: None,
        };

        let result = check_approved(input, &store, &store, &hasher, &review_store);
        assert!(result.is_ok(), "approved cycle should pass: {result:?}");
    }

    #[test]
    fn check_approved_stale_hash_is_blocked() {
        let store = MemStore::default();
        let track = make_track("test-track");
        store.save(&track).unwrap();
        let review_store = MemReviewStore::default();
        let track_id = TrackId::try_new("test-track").unwrap();
        review_store
            .save_review(&track_id, make_review_with_approved_cycle("rvw1:sha256:old-hash"));

        let hasher = FixedHasher::new("rvw1:sha256:new-hash");
        let input = CheckApprovedInput {
            items_dir: PathBuf::from("track/items"),
            track_id: "test-track".to_string(),
            planning_only: false,
            current_snapshot: None,
        };

        let result = check_approved(input, &store, &store, &hasher, &review_store);
        assert!(result.is_err(), "stale hash should block");
        assert!(result.unwrap_err().contains("[BLOCKED]"));
    }

    #[test]
    fn check_approved_fast_only_without_final_is_blocked() {
        use std::collections::BTreeMap;
        let code_hash = "rvw1:sha256:abc";
        let store = MemStore::default();
        let track = make_track("test-track");
        store.save(&track).unwrap();

        let grn = domain::ReviewGroupName::try_new("other").unwrap();
        let ts = domain::Timestamp::new("2026-03-30T08:00:00Z").unwrap();
        let mut groups = BTreeMap::new();
        let mut gs = domain::CycleGroupState::new(vec![]);
        let fast = domain::GroupRound::success(
            domain::RoundType::Fast,
            ts.clone(),
            code_hash,
            domain::GroupRoundVerdict::ZeroFindings,
        )
        .unwrap();
        gs.record_round(fast);
        groups.insert(grn, gs);
        let mut rj = domain::ReviewJson::new();
        rj.start_cycle("c1", ts, "main", "sha256:none", "sha256:none", groups).unwrap();

        let review_store = MemReviewStore::default();
        let track_id = TrackId::try_new("test-track").unwrap();
        review_store.save_review(&track_id, rj);

        let hasher = FixedHasher::new(code_hash);
        let input = CheckApprovedInput {
            items_dir: PathBuf::from("track/items"),
            track_id: "test-track".to_string(),
            planning_only: false,
            current_snapshot: None,
        };

        let result = check_approved(input, &store, &store, &hasher, &review_store);
        assert!(result.is_err(), "fast-only should block");
        assert!(result.unwrap_err().contains("[BLOCKED]"));
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
