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

/// Recommended next command for the track workflow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NextCommand {
    Implement,
    Done,
    ActivateTrack(String),
    PlanNewFeature,
    Status,
}

impl fmt::Display for NextCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Implement => f.write_str("/track:implement"),
            Self::Done => f.write_str("/track:done"),
            Self::ActivateTrack(id) => write!(f, "/track:activate {id}"),
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
            next_command: NextCommand::ActivateTrack(track.id().to_string()),
            blocker: Some(
                "implementation commands are disabled until activation completes".to_owned(),
            ),
        },
        TrackStatus::Planned => TrackPhaseInfo {
            phase: TrackPhase::Planning,
            reason: "track is planned with branch materialized".to_owned(),
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
    track_id: &str,
    status: TrackStatus,
    has_branch: bool,
    schema_version: u32,
    override_reason: Option<&str>,
) -> TrackPhaseInfo {
    let is_branchless_v3 = schema_version == 3 && !has_branch;

    match status {
        TrackStatus::Planned if is_branchless_v3 => TrackPhaseInfo {
            phase: TrackPhase::ReadyToActivate,
            reason: "track exists, status is planned, branch is not materialized yet".to_owned(),
            next_command: NextCommand::ActivateTrack(track_id.to_owned()),
            blocker: Some(
                "implementation commands are disabled until activation completes".to_owned(),
            ),
        },
        TrackStatus::Planned => TrackPhaseInfo {
            phase: TrackPhase::Planning,
            reason: "track is planned with branch materialized".to_owned(),
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
#[must_use]
pub fn next_command(track: &TrackMetadata, schema_version: u32) -> NextCommand {
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
        let task_id = TaskId::try_new("T1").unwrap();
        let task = TrackTask::new(task_id.clone(), "Implement feature").unwrap();
        let section = PlanSection::new("S1", "Build", Vec::new(), vec![task_id]).unwrap();
        let plan = PlanView::new(Vec::new(), vec![section]);
        TrackMetadata::with_branch(
            TrackId::try_new(id).unwrap(),
            branch.map(|b| TrackBranch::try_new(b).unwrap()),
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
        assert_eq!(info.next_command, NextCommand::ActivateTrack("demo".to_owned()));
        assert!(info.blocker.is_some());
    }

    #[test]
    fn resolve_phase_materialized_v3_planned_returns_planning() {
        let track = planned_track("demo", Some("track/demo"));
        let info = resolve_phase(&track, 3);
        assert_eq!(info.phase, TrackPhase::Planning);
        assert_eq!(info.next_command, NextCommand::Implement);
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
        track.transition_task(&TaskId::try_new("T1").unwrap(), TaskTransition::Start).unwrap();
        let info = resolve_phase(&track, 3);
        assert_eq!(info.phase, TrackPhase::InProgress);
        assert_eq!(info.next_command, NextCommand::Implement);
    }

    #[test]
    fn resolve_phase_done_returns_ready_to_ship() {
        let mut track = planned_track("demo", Some("track/demo"));
        track.transition_task(&TaskId::try_new("T1").unwrap(), TaskTransition::Start).unwrap();
        track
            .transition_task(
                &TaskId::try_new("T1").unwrap(),
                TaskTransition::Complete { commit_hash: None },
            )
            .unwrap();
        let info = resolve_phase(&track, 3);
        assert_eq!(info.phase, TrackPhase::ReadyToShip);
        assert_eq!(info.next_command, NextCommand::Done);
    }

    #[test]
    fn resolve_phase_blocked_returns_blocked_with_reason() {
        let track = TrackMetadata::with_branch(
            TrackId::try_new("demo").unwrap(),
            Some(TrackBranch::try_new("track/demo").unwrap()),
            "Test",
            vec![TrackTask::new(TaskId::try_new("T1").unwrap(), "task").unwrap()],
            PlanView::new(
                Vec::new(),
                vec![
                    PlanSection::new(
                        "S1",
                        "Build",
                        Vec::new(),
                        vec![TaskId::try_new("T1").unwrap()],
                    )
                    .unwrap(),
                ],
            ),
            Some(StatusOverride::blocked("waiting on review").unwrap()),
        )
        .unwrap();
        let info = resolve_phase(&track, 3);
        assert_eq!(info.phase, TrackPhase::Blocked);
        assert!(info.blocker.unwrap().contains("waiting on review"));
    }

    #[test]
    fn resolve_phase_cancelled_returns_cancelled() {
        let track = TrackMetadata::with_branch(
            TrackId::try_new("demo").unwrap(),
            Some(TrackBranch::try_new("track/demo").unwrap()),
            "Test",
            vec![TrackTask::new(TaskId::try_new("T1").unwrap(), "task").unwrap()],
            PlanView::new(
                Vec::new(),
                vec![
                    PlanSection::new(
                        "S1",
                        "Build",
                        Vec::new(),
                        vec![TaskId::try_new("T1").unwrap()],
                    )
                    .unwrap(),
                ],
            ),
            Some(StatusOverride::cancelled("scope changed").unwrap()),
        )
        .unwrap();
        let info = resolve_phase(&track, 3);
        assert_eq!(info.phase, TrackPhase::Cancelled);
        assert_eq!(info.next_command, NextCommand::PlanNewFeature);
    }

    // --- resolve_phase_from_record ---

    #[rstest]
    #[case::branchless_v3_planned_returns_ready_to_activate(
        TrackStatus::Planned,
        false,
        3,
        None,
        TrackPhase::ReadyToActivate
    )]
    #[case::materialized_v3_planned_returns_planning(
        TrackStatus::Planned,
        true,
        3,
        None,
        TrackPhase::Planning
    )]
    #[case::v2_branchless_returns_planning(
        TrackStatus::Planned,
        false,
        2,
        None,
        TrackPhase::Planning
    )]
    #[case::in_progress_returns_in_progress(
        TrackStatus::InProgress,
        true,
        3,
        None,
        TrackPhase::InProgress
    )]
    #[case::done_returns_ready_to_ship(TrackStatus::Done, true, 3, None, TrackPhase::ReadyToShip)]
    #[case::blocked_returns_blocked(
        TrackStatus::Blocked,
        true,
        3,
        Some("waiting on review"),
        TrackPhase::Blocked
    )]
    #[case::cancelled_returns_cancelled(
        TrackStatus::Cancelled,
        true,
        3,
        Some("scope changed"),
        TrackPhase::Cancelled
    )]
    #[case::archived_returns_archived(TrackStatus::Archived, false, 3, None, TrackPhase::Archived)]
    fn resolve_phase_from_record_status_branch_matrix(
        #[case] status: TrackStatus,
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
        let info = resolve_phase_from_record("demo", TrackStatus::Planned, false, 3, None);
        assert_eq!(info.next_command, NextCommand::ActivateTrack("demo".to_owned()));
    }

    #[test]
    fn resolve_phase_from_record_blocked_blocker_contains_reason() {
        let info = resolve_phase_from_record(
            "demo",
            TrackStatus::Blocked,
            true,
            3,
            Some("waiting on review"),
        );
        assert!(info.blocker.unwrap().contains("waiting on review"));
    }

    // --- next_command ---

    #[test]
    fn next_command_for_branchless_v3_returns_activate() {
        let track = planned_track("demo", None);
        let cmd = next_command(&track, 3);
        assert_eq!(cmd, NextCommand::ActivateTrack("demo".to_owned()));
    }

    #[test]
    fn next_command_for_materialized_planned_returns_implement() {
        let track = planned_track("demo", Some("track/demo"));
        let cmd = next_command(&track, 3);
        assert_eq!(cmd, NextCommand::Implement);
    }
}
