//! Track phase resolution — determines user-facing phase, next command, and blockers.
//!
//! The phase is a user-facing concept derived from `TrackStatus`, branch state,
//! and schema version. It drives the recommended next command in status displays
//! and registry rendering.

use std::fmt;

use crate::{TrackMetadata, TrackStatus};

/// User-facing workflow phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackPhase {
    /// Branch materialized, all tasks still `todo`.
    Planning,
    /// Planning-only track (`branch=null`, schema v3) awaiting activation.
    ReadyToActivate,
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
            Self::ReadyToActivate => write!(f, "Ready to Activate"),
            Self::InProgress => write!(f, "In Progress"),
            Self::ReadyToShip => write!(f, "Ready to Ship"),
            Self::Blocked => write!(f, "Blocked"),
            Self::Cancelled => write!(f, "Cancelled"),
            Self::Archived => write!(f, "Archived"),
        }
    }
}

/// Phase resolution result with routing guidance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackPhaseInfo {
    pub phase: TrackPhase,
    pub reason: String,
    pub next_command: String,
    pub blocker: Option<String>,
}

/// Resolves the user-facing phase for a track.
///
/// `schema_version` is required because `TrackMetadata` does not carry it
/// (it is a serialization concern in the codec layer).
#[must_use]
pub fn resolve_phase(track: &TrackMetadata, schema_version: u32) -> TrackPhaseInfo {
    let status = track.status();
    let is_branchless_v3 = schema_version == 3 && track.branch().is_none();

    match status {
        TrackStatus::Planned if is_branchless_v3 => TrackPhaseInfo {
            phase: TrackPhase::ReadyToActivate,
            reason: "track exists, status is planned, branch is not materialized yet".to_owned(),
            next_command: format!("/track:activate {}", track.id()),
            blocker: Some(
                "implementation commands are disabled until activation completes".to_owned(),
            ),
        },
        TrackStatus::Planned => TrackPhaseInfo {
            phase: TrackPhase::Planning,
            reason: "track is planned with branch materialized".to_owned(),
            next_command: "/track:implement".to_owned(),
            blocker: None,
        },
        TrackStatus::InProgress => TrackPhaseInfo {
            phase: TrackPhase::InProgress,
            reason: "track has unresolved tasks".to_owned(),
            next_command: "/track:implement".to_owned(),
            blocker: None,
        },
        TrackStatus::Done => TrackPhaseInfo {
            phase: TrackPhase::ReadyToShip,
            reason: "all tasks completed".to_owned(),
            next_command: "/track:done".to_owned(),
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
                next_command: "/track:status".to_owned(),
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
                next_command: "/track:plan <feature>".to_owned(),
                blocker: None,
            }
        }
        TrackStatus::Archived => TrackPhaseInfo {
            phase: TrackPhase::Archived,
            reason: "track is archived".to_owned(),
            next_command: "/track:plan <feature>".to_owned(),
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
    track_id: &str,
    status: &str,
    has_branch: bool,
    schema_version: u32,
    override_reason: Option<&str>,
) -> TrackPhaseInfo {
    let is_branchless_v3 = schema_version == 3 && !has_branch;

    match status {
        "planned" if is_branchless_v3 => TrackPhaseInfo {
            phase: TrackPhase::ReadyToActivate,
            reason: "track exists, status is planned, branch is not materialized yet".to_owned(),
            next_command: format!("/track:activate {track_id}"),
            blocker: Some(
                "implementation commands are disabled until activation completes".to_owned(),
            ),
        },
        "planned" => TrackPhaseInfo {
            phase: TrackPhase::Planning,
            reason: "track is planned with branch materialized".to_owned(),
            next_command: "/track:implement".to_owned(),
            blocker: None,
        },
        "in_progress" => TrackPhaseInfo {
            phase: TrackPhase::InProgress,
            reason: "track has unresolved tasks".to_owned(),
            next_command: "/track:implement".to_owned(),
            blocker: None,
        },
        "done" => TrackPhaseInfo {
            phase: TrackPhase::ReadyToShip,
            reason: "all tasks completed".to_owned(),
            next_command: "/track:done".to_owned(),
            blocker: None,
        },
        "blocked" => {
            let reason = override_reason.unwrap_or("track is blocked").to_owned();
            TrackPhaseInfo {
                phase: TrackPhase::Blocked,
                reason: reason.clone(),
                next_command: "/track:status".to_owned(),
                blocker: Some(reason),
            }
        }
        "cancelled" => {
            let reason = override_reason.unwrap_or("track has been cancelled").to_owned();
            TrackPhaseInfo {
                phase: TrackPhase::Cancelled,
                reason,
                next_command: "/track:plan <feature>".to_owned(),
                blocker: None,
            }
        }
        "archived" => TrackPhaseInfo {
            phase: TrackPhase::Archived,
            reason: "track is archived".to_owned(),
            next_command: "/track:plan <feature>".to_owned(),
            blocker: None,
        },
        _ => TrackPhaseInfo {
            phase: TrackPhase::InProgress,
            reason: format!("unknown status '{status}'"),
            next_command: "/track:status".to_owned(),
            blocker: None,
        },
    }
}

/// Returns the recommended next command string for registry rendering.
#[must_use]
pub fn next_command(track: &TrackMetadata, schema_version: u32) -> String {
    resolve_phase(track, schema_version).next_command
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::{
        PlanSection, PlanView, StatusOverride, TaskId, TaskTransition, TrackBranch, TrackId,
        TrackTask,
    };

    fn planned_track(id: &str, branch: Option<&str>) -> TrackMetadata {
        let task_id = TaskId::new("T1").unwrap();
        let task = TrackTask::new(task_id.clone(), "Implement feature").unwrap();
        let section = PlanSection::new("S1", "Build", Vec::new(), vec![task_id]).unwrap();
        let plan = PlanView::new(Vec::new(), vec![section]);
        TrackMetadata::with_branch(
            TrackId::new(id).unwrap(),
            branch.map(|b| TrackBranch::new(b).unwrap()),
            "Test Track",
            vec![task],
            plan,
            None,
        )
        .unwrap()
    }

    // --- resolve_phase ---

    #[test]
    fn resolve_phase_branchless_v3_planned_returns_ready_to_activate() {
        let track = planned_track("demo", None);
        let info = resolve_phase(&track, 3);
        assert_eq!(info.phase, TrackPhase::ReadyToActivate);
        assert_eq!(info.next_command, "/track:activate demo");
        assert!(info.blocker.is_some());
    }

    #[test]
    fn resolve_phase_materialized_v3_planned_returns_planning() {
        let track = planned_track("demo", Some("track/demo"));
        let info = resolve_phase(&track, 3);
        assert_eq!(info.phase, TrackPhase::Planning);
        assert_eq!(info.next_command, "/track:implement");
        assert!(info.blocker.is_none());
    }

    #[test]
    fn resolve_phase_v2_branchless_planned_returns_planning_not_ready_to_activate() {
        let track = planned_track("demo", None);
        let info = resolve_phase(&track, 2);
        assert_eq!(info.phase, TrackPhase::Planning);
        assert_ne!(info.phase, TrackPhase::ReadyToActivate);
    }

    #[test]
    fn resolve_phase_in_progress_returns_in_progress() {
        let mut track = planned_track("demo", Some("track/demo"));
        track.transition_task(&TaskId::new("T1").unwrap(), TaskTransition::Start).unwrap();
        let info = resolve_phase(&track, 3);
        assert_eq!(info.phase, TrackPhase::InProgress);
        assert_eq!(info.next_command, "/track:implement");
    }

    #[test]
    fn resolve_phase_done_returns_ready_to_ship() {
        let mut track = planned_track("demo", Some("track/demo"));
        track.transition_task(&TaskId::new("T1").unwrap(), TaskTransition::Start).unwrap();
        track
            .transition_task(
                &TaskId::new("T1").unwrap(),
                TaskTransition::Complete { commit_hash: None },
            )
            .unwrap();
        let info = resolve_phase(&track, 3);
        assert_eq!(info.phase, TrackPhase::ReadyToShip);
        assert_eq!(info.next_command, "/track:done");
    }

    #[test]
    fn resolve_phase_blocked_returns_blocked_with_reason() {
        let track = TrackMetadata::with_branch(
            TrackId::new("demo").unwrap(),
            Some(TrackBranch::new("track/demo").unwrap()),
            "Test",
            vec![TrackTask::new(TaskId::new("T1").unwrap(), "task").unwrap()],
            PlanView::new(
                Vec::new(),
                vec![
                    PlanSection::new("S1", "Build", Vec::new(), vec![TaskId::new("T1").unwrap()])
                        .unwrap(),
                ],
            ),
            Some(StatusOverride::blocked("waiting on review")),
        )
        .unwrap();
        let info = resolve_phase(&track, 3);
        assert_eq!(info.phase, TrackPhase::Blocked);
        assert!(info.blocker.unwrap().contains("waiting on review"));
    }

    #[test]
    fn resolve_phase_cancelled_returns_cancelled() {
        let track = TrackMetadata::with_branch(
            TrackId::new("demo").unwrap(),
            Some(TrackBranch::new("track/demo").unwrap()),
            "Test",
            vec![TrackTask::new(TaskId::new("T1").unwrap(), "task").unwrap()],
            PlanView::new(
                Vec::new(),
                vec![
                    PlanSection::new("S1", "Build", Vec::new(), vec![TaskId::new("T1").unwrap()])
                        .unwrap(),
                ],
            ),
            Some(StatusOverride::cancelled("scope changed")),
        )
        .unwrap();
        let info = resolve_phase(&track, 3);
        assert_eq!(info.phase, TrackPhase::Cancelled);
        assert_eq!(info.next_command, "/track:plan <feature>");
    }

    // --- resolve_phase_from_record ---

    #[rstest]
    #[case::branchless_v3_planned_returns_ready_to_activate(
        "planned",
        false,
        3,
        None,
        TrackPhase::ReadyToActivate
    )]
    #[case::materialized_v3_planned_returns_planning(
        "planned",
        true,
        3,
        None,
        TrackPhase::Planning
    )]
    #[case::v2_branchless_returns_planning("planned", false, 2, None, TrackPhase::Planning)]
    #[case::in_progress_returns_in_progress("in_progress", true, 3, None, TrackPhase::InProgress)]
    #[case::done_returns_ready_to_ship("done", true, 3, None, TrackPhase::ReadyToShip)]
    #[case::blocked_returns_blocked(
        "blocked",
        true,
        3,
        Some("waiting on review"),
        TrackPhase::Blocked
    )]
    #[case::cancelled_returns_cancelled(
        "cancelled",
        true,
        3,
        Some("scope changed"),
        TrackPhase::Cancelled
    )]
    #[case::archived_returns_archived("archived", false, 3, None, TrackPhase::Archived)]
    #[case::unknown_status_falls_back_to_in_progress(
        "unknown",
        true,
        3,
        None,
        TrackPhase::InProgress
    )]
    fn resolve_phase_from_record_status_branch_matrix(
        #[case] status: &str,
        #[case] has_branch: bool,
        #[case] schema_version: u32,
        #[case] override_reason: Option<&str>,
        #[case] expected_phase: TrackPhase,
    ) {
        let info =
            resolve_phase_from_record("demo", status, has_branch, schema_version, override_reason);
        assert_eq!(info.phase, expected_phase);
    }

    #[test]
    fn resolve_phase_from_record_branchless_v3_planned_next_command_is_activate() {
        let info = resolve_phase_from_record("demo", "planned", false, 3, None);
        assert_eq!(info.next_command, "/track:activate demo");
    }

    #[test]
    fn resolve_phase_from_record_blocked_blocker_contains_reason() {
        let info = resolve_phase_from_record("demo", "blocked", true, 3, Some("waiting on review"));
        assert!(info.blocker.unwrap().contains("waiting on review"));
    }

    // --- next_command ---

    #[test]
    fn next_command_for_branchless_v3_returns_activate() {
        let track = planned_track("demo", None);
        let cmd = next_command(&track, 3);
        assert_eq!(cmd, "/track:activate demo");
    }

    #[test]
    fn next_command_for_materialized_planned_returns_implement() {
        let track = planned_track("demo", Some("track/demo"));
        let cmd = next_command(&track, 3);
        assert_eq!(cmd, "/track:implement");
    }
}
