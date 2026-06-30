//! Track phase resolution — determines user-facing phase, next command, and blockers.
//!
//! The phase is a user-facing concept derived from `TrackStatus`, branch state,
//! and schema version. It drives the recommended next command in status displays
//! and registry rendering.
//!
//! This module also hosts the fixpoint-resolution domain types (`FixpointStep`,
//! `ReviewScopeSet`, `ReviewScopeSetError`) used by the usecase layer's mechanized
//! convergence driver (D2 / IN-02 / AC-03).

use std::collections::BTreeSet;
use std::fmt;

use thiserror::Error;

use crate::{ImplPlanDocument, TrackMetadata, TrackStatus, derive_track_status};

// ── ReviewScopeSet ────────────────────────────────────────────────────────────

/// Non-empty deterministic set of review scopes required by the RFP gate.
///
/// Scope labels are opaque `String` values owned by the orchestrator or review
/// infrastructure; this value object only rejects the empty set so
/// [`FixpointStep::RunRfp`] cannot represent "run RFP for no scopes".
/// [`BTreeSet`] keeps CLI output deterministic.
///
/// # Errors
///
/// [`try_new`](ReviewScopeSet::try_new) returns [`ReviewScopeSetError::Empty`] when
/// the supplied set is empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewScopeSet {
    scopes: BTreeSet<String>,
}

impl ReviewScopeSet {
    /// Construct a [`ReviewScopeSet`] from a non-empty [`BTreeSet<String>`].
    ///
    /// # Errors
    ///
    /// Returns [`ReviewScopeSetError::Empty`] when `scopes` is empty.
    pub fn try_new(scopes: BTreeSet<String>) -> Result<Self, ReviewScopeSetError> {
        if scopes.is_empty() {
            return Err(ReviewScopeSetError::Empty);
        }
        Ok(Self { scopes })
    }

    /// Returns a reference to the underlying set of scope labels.
    #[must_use]
    pub fn as_set(&self) -> &BTreeSet<String> {
        &self.scopes
    }
}

// ── ReviewScopeSetError ───────────────────────────────────────────────────────

/// Validation error for [`ReviewScopeSet`] construction.
///
/// [`Empty`](ReviewScopeSetError::Empty) means the review gate attempted to
/// request RFP with no target scopes, which is not a valid
/// [`FixpointStep::RunRfp`] payload.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ReviewScopeSetError {
    /// The supplied scope set was empty.
    #[error("review scope set must be non-empty")]
    Empty,
}

// ── FixpointStep ──────────────────────────────────────────────────────────────

/// Output of the mechanized fixpoint resolver (D2).
///
/// Each variant names the one phase the orchestrator must execute next to move
/// toward convergence:
///
/// - [`RunDfp`](FixpointStep::RunDfp) — dry gate is open; run DFP.
/// - [`RunRfp`](FixpointStep::RunRfp) — one or more review scopes are open;
///   run RFP for all named scopes.
/// - [`RunRefVerify`](FixpointStep::RunRefVerify) — ref-verify gate is open.
/// - [`Commit`](FixpointStep::Commit) — all gates are green; safe to commit.
///
/// `scopes` is a non-empty [`ReviewScopeSet`] of opaque orchestrator-assigned
/// review-group labels; the domain validates only non-emptiness, not naming.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FixpointStep {
    /// Dry gate open — run the DRY fix phase (DFP).
    RunDfp,
    /// One or more review scopes are open — run the review fix phase (RFP).
    RunRfp {
        /// Non-empty set of opaque review-group scope labels that must run RFP.
        scopes: ReviewScopeSet,
    },
    /// Ref-verify gate open — run `ref-verify`.
    RunRefVerify,
    /// All gates green — safe to commit.
    Commit,
}

/// User-facing workflow phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackPhase {
    /// Branch materialized, all tasks still `todo`.
    Planning,
    /// At least one task is `in_progress`.
    InProgress,
    /// All tasks resolved (done/skipped).
    ReadyToShip,
    /// Blocked by a `StatusOverride`.
    Blocked,
    /// Cancelled by a `StatusOverride`.
    Cancelled,
    /// Track is archived.
    Archived,
}

impl fmt::Display for TrackPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Planning => write!(f, "Planning"),
            Self::InProgress => write!(f, "In Progress"),
            Self::ReadyToShip => write!(f, "Ready to Ship"),
            Self::Blocked => write!(f, "Blocked"),
            Self::Cancelled => write!(f, "Cancelled"),
            Self::Archived => write!(f, "Archived"),
        }
    }
}

/// Recommended next command for the track workflow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NextCommand {
    Implement,
    Done,
    PlanNewFeature,
    Status,
}

impl fmt::Display for NextCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Implement => f.write_str("/track:implement"),
            Self::Done => f.write_str("/track:done"),
            Self::PlanNewFeature => f.write_str("/track:plan <feature>"),
            Self::Status => f.write_str("/track:status"),
        }
    }
}

/// Phase resolution result with routing guidance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackPhaseInfo {
    pub phase: TrackPhase,
    pub reason: String,
    pub next_command: NextCommand,
    pub blocker: Option<String>,
}

/// Resolves the user-facing phase for a track.
///
/// `impl_plan` is required to derive the track's current status.
/// Pass `None` for tracks that have not yet generated `impl-plan.json`.
#[must_use]
pub fn resolve_phase(
    track: &TrackMetadata,
    impl_plan: Option<&ImplPlanDocument>,
) -> TrackPhaseInfo {
    // Derive status on demand from impl-plan.json + status_override.
    let status = derive_track_status(impl_plan, track.status_override());

    match status {
        TrackStatus::Planned => TrackPhaseInfo {
            phase: TrackPhase::Planning,
            reason: "track is planned".to_owned(),
            next_command: NextCommand::Implement,
            blocker: None,
        },
        TrackStatus::InProgress => TrackPhaseInfo {
            phase: TrackPhase::InProgress,
            reason: "track has unresolved tasks".to_owned(),
            next_command: NextCommand::Implement,
            blocker: None,
        },
        TrackStatus::Done => TrackPhaseInfo {
            phase: TrackPhase::ReadyToShip,
            reason: "all tasks completed".to_owned(),
            next_command: NextCommand::Done,
            blocker: None,
        },
        TrackStatus::Blocked => {
            let override_reason = track
                .status_override()
                .map(|o| o.reason().to_owned())
                .unwrap_or_else(|| "track is blocked".to_owned());
            TrackPhaseInfo {
                phase: TrackPhase::Blocked,
                reason: override_reason.clone(),
                next_command: NextCommand::Status,
                blocker: Some(override_reason),
            }
        }
        TrackStatus::Cancelled => {
            let override_reason = track
                .status_override()
                .map(|o| o.reason().to_owned())
                .unwrap_or_else(|| "track has been cancelled".to_owned());
            TrackPhaseInfo {
                phase: TrackPhase::Cancelled,
                reason: override_reason,
                next_command: NextCommand::PlanNewFeature,
                blocker: None,
            }
        }
        TrackStatus::Archived => TrackPhaseInfo {
            phase: TrackPhase::Archived,
            reason: "track is archived".to_owned(),
            next_command: NextCommand::PlanNewFeature,
            blocker: None,
        },
    }
}

/// Resolves phase from lightweight record fields (no full `TrackMetadata` needed).
///
/// Useful when the caller only has raw metadata fields (e.g., from a JSON scan)
/// and does not need to construct a full domain aggregate.
#[must_use]
pub fn resolve_phase_from_record(
    status: TrackStatus,
    override_reason: Option<&str>,
) -> TrackPhaseInfo {
    match status {
        TrackStatus::Planned => TrackPhaseInfo {
            phase: TrackPhase::Planning,
            reason: "track is planned".to_owned(),
            next_command: NextCommand::Implement,
            blocker: None,
        },
        TrackStatus::InProgress => TrackPhaseInfo {
            phase: TrackPhase::InProgress,
            reason: "track has unresolved tasks".to_owned(),
            next_command: NextCommand::Implement,
            blocker: None,
        },
        TrackStatus::Done => TrackPhaseInfo {
            phase: TrackPhase::ReadyToShip,
            reason: "all tasks completed".to_owned(),
            next_command: NextCommand::Done,
            blocker: None,
        },
        TrackStatus::Blocked => {
            let reason = override_reason.unwrap_or("track is blocked").to_owned();
            TrackPhaseInfo {
                phase: TrackPhase::Blocked,
                reason: reason.clone(),
                next_command: NextCommand::Status,
                blocker: Some(reason),
            }
        }
        TrackStatus::Cancelled => {
            let reason = override_reason.unwrap_or("track has been cancelled").to_owned();
            TrackPhaseInfo {
                phase: TrackPhase::Cancelled,
                reason,
                next_command: NextCommand::PlanNewFeature,
                blocker: None,
            }
        }
        TrackStatus::Archived => TrackPhaseInfo {
            phase: TrackPhase::Archived,
            reason: "track is archived".to_owned(),
            next_command: NextCommand::PlanNewFeature,
            blocker: None,
        },
    }
}

/// Returns the recommended next command for registry rendering.
///
/// `impl_plan` is required to derive the track's current status.
#[must_use]
pub fn next_command(track: &TrackMetadata, impl_plan: Option<&ImplPlanDocument>) -> NextCommand {
    resolve_phase(track, impl_plan).next_command
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::collections::BTreeSet;

    use rstest::rstest;

    use super::*;
    use crate::{StatusOverride, TrackBranch, TrackId};

    fn planned_track(id: &str, branch: Option<&str>) -> TrackMetadata {
        // Identity-only TrackMetadata; status is derived on demand.
        TrackMetadata::with_branch(
            TrackId::try_new(id).unwrap(),
            branch.map(|b| TrackBranch::try_new(b).unwrap()),
            "Test Track",
            None,
        )
        .unwrap()
    }

    // --- resolve_phase ---

    #[test]
    fn resolve_phase_planned_returns_planning() {
        let track = planned_track("demo", Some("track/demo"));
        let info = resolve_phase(&track, None);
        assert_eq!(info.phase, TrackPhase::Planning);
        assert_eq!(info.next_command, NextCommand::Implement);
        assert!(info.blocker.is_none());
    }

    #[test]
    fn resolve_phase_in_progress_returns_in_progress() {
        use crate::{PlanSection, PlanView, TaskId, TaskStatus, TrackTask};
        // Build an impl-plan with an in-progress task to derive InProgress status.
        let task = TrackTask::with_status(
            TaskId::try_new("T001").unwrap(),
            "Task",
            TaskStatus::InProgress,
        )
        .unwrap();
        let section =
            PlanSection::new("S1", "Section", vec![], vec![TaskId::try_new("T001").unwrap()])
                .unwrap();
        let impl_plan =
            crate::ImplPlanDocument::new(vec![task], PlanView::new(vec![], vec![section])).unwrap();
        let track = planned_track("demo", Some("track/demo"));
        let info = resolve_phase(&track, Some(&impl_plan));
        assert_eq!(info.phase, TrackPhase::InProgress);
        assert_eq!(info.next_command, NextCommand::Implement);
    }

    #[test]
    fn resolve_phase_done_returns_ready_to_ship() {
        use crate::{CommitHash, PlanSection, PlanView, TaskId, TaskStatus, TrackTask};
        // Build an impl-plan with all tasks resolved to derive Done status.
        let hash = CommitHash::try_new("abc1234").unwrap();
        let task = TrackTask::with_status(
            TaskId::try_new("T001").unwrap(),
            "Task",
            TaskStatus::DoneTraced { commit_hash: hash },
        )
        .unwrap();
        let section =
            PlanSection::new("S1", "Section", vec![], vec![TaskId::try_new("T001").unwrap()])
                .unwrap();
        let impl_plan =
            crate::ImplPlanDocument::new(vec![task], PlanView::new(vec![], vec![section])).unwrap();
        let track = planned_track("demo", Some("track/demo"));
        let info = resolve_phase(&track, Some(&impl_plan));
        assert_eq!(info.phase, TrackPhase::ReadyToShip);
        assert_eq!(info.next_command, NextCommand::Done);
    }

    #[test]
    fn resolve_phase_blocked_returns_blocked_with_reason() {
        // Blocked status comes from status_override.
        let mut track = TrackMetadata::with_branch(
            TrackId::try_new("demo").unwrap(),
            Some(TrackBranch::try_new("track/demo").unwrap()),
            "Test",
            Some(StatusOverride::blocked("waiting on review").unwrap()),
        )
        .unwrap();
        let info = resolve_phase(&track, None);
        assert_eq!(info.phase, TrackPhase::Blocked);
        assert!(info.blocker.unwrap().contains("waiting on review"));

        // Also verify set_status_override path.
        track.set_status_override(Some(StatusOverride::blocked("new reason").unwrap()));
        let info2 = resolve_phase(&track, None);
        assert!(info2.blocker.unwrap().contains("new reason"));
    }

    #[test]
    fn resolve_phase_cancelled_returns_cancelled() {
        let track = TrackMetadata::with_branch(
            TrackId::try_new("demo").unwrap(),
            Some(TrackBranch::try_new("track/demo").unwrap()),
            "Test",
            Some(StatusOverride::cancelled("scope changed").unwrap()),
        )
        .unwrap();
        let info = resolve_phase(&track, None);
        assert_eq!(info.phase, TrackPhase::Cancelled);
        assert_eq!(info.next_command, NextCommand::PlanNewFeature);
    }

    // --- resolve_phase_from_record ---

    #[rstest]
    #[case::planned_returns_planning(TrackStatus::Planned, None, TrackPhase::Planning)]
    #[case::in_progress_returns_in_progress(TrackStatus::InProgress, None, TrackPhase::InProgress)]
    #[case::done_returns_ready_to_ship(TrackStatus::Done, None, TrackPhase::ReadyToShip)]
    #[case::blocked_returns_blocked(
        TrackStatus::Blocked,
        Some("waiting on review"),
        TrackPhase::Blocked
    )]
    #[case::cancelled_returns_cancelled(
        TrackStatus::Cancelled,
        Some("scope changed"),
        TrackPhase::Cancelled
    )]
    #[case::archived_returns_archived(TrackStatus::Archived, None, TrackPhase::Archived)]
    fn resolve_phase_from_record_status_matrix(
        #[case] status: TrackStatus,
        #[case] override_reason: Option<&str>,
        #[case] expected_phase: TrackPhase,
    ) {
        let info = resolve_phase_from_record(status, override_reason);
        assert_eq!(info.phase, expected_phase);
    }

    #[test]
    fn resolve_phase_from_record_planned_next_command_is_implement() {
        let info = resolve_phase_from_record(TrackStatus::Planned, None);
        assert_eq!(info.next_command, NextCommand::Implement);
    }

    #[test]
    fn resolve_phase_from_record_blocked_blocker_contains_reason() {
        let info = resolve_phase_from_record(TrackStatus::Blocked, Some("waiting on review"));
        assert!(info.blocker.unwrap().contains("waiting on review"));
    }

    // --- next_command ---

    #[test]
    fn next_command_for_planned_returns_implement() {
        let track = planned_track("demo", Some("track/demo"));
        let cmd = next_command(&track, None);
        assert_eq!(cmd, NextCommand::Implement);
    }

    // --- ReviewScopeSet ---

    #[test]
    fn review_scope_set_try_new_with_empty_set_returns_empty_error() {
        let result = ReviewScopeSet::try_new(BTreeSet::new());
        assert!(matches!(result, Err(ReviewScopeSetError::Empty)));
    }

    #[test]
    fn review_scope_set_try_new_with_single_scope_preserves_it() {
        let mut scopes = BTreeSet::new();
        scopes.insert("impl-plan".to_owned());
        let result = ReviewScopeSet::try_new(scopes.clone());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_set(), &scopes);
    }

    #[test]
    fn review_scope_set_try_new_with_multiple_scopes_preserves_deterministic_order() {
        let mut scopes = BTreeSet::new();
        scopes.insert("code".to_owned());
        scopes.insert("impl-plan".to_owned());
        let result = ReviewScopeSet::try_new(scopes.clone());
        assert!(result.is_ok());
        let set = result.unwrap();
        // BTreeSet iteration order is deterministic (ascending lexicographic).
        let ordered: Vec<&str> = set.as_set().iter().map(String::as_str).collect();
        assert_eq!(ordered, vec!["code", "impl-plan"]);
    }
}
