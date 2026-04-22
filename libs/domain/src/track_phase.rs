//! Track phase resolution — determines user-facing phase, next command, and blockers.
//!
//! The phase is a user-facing concept derived from `TrackStatus`, branch state,
//! and schema version. It drives the recommended next command in status displays
//! and registry rendering.

use std::fmt;

use crate::{ImplPlanDocument, TrackMetadata, TrackStatus, derive_track_status};

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
///
/// `impl_plan` is required to derive the track's current status.
/// Pass `None` for planning-only tracks that have not yet generated `impl-plan.json`.
#[must_use]
pub fn resolve_phase(
    track: &TrackMetadata,
    schema_version: u32,
    impl_plan: Option<&ImplPlanDocument>,
) -> TrackPhaseInfo {
    // Derive status on demand from impl-plan.json + status_override.
    let status = derive_track_status(impl_plan, track.status_override());
    // Schema versions 3, 4, and 5 represent identity-only / branchless planning shapes.
    // All three require activation before implementation; branchless tracks must activate.
    let is_branchless_activatable = matches!(schema_version, 3..=5) && track.branch().is_none();

    match status {
        TrackStatus::Planned if is_branchless_activatable => TrackPhaseInfo {
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
    // Schema versions 3, 4, and 5 all require activation before implementation
    // when the track is branchless. v5 is the current identity-only format.
    let is_branchless_activatable = matches!(schema_version, 3..=5) && !has_branch;

    match status {
        TrackStatus::Planned if is_branchless_activatable => TrackPhaseInfo {
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
///
/// `impl_plan` is required to derive the track's current status.
#[must_use]
pub fn next_command(
    track: &TrackMetadata,
    schema_version: u32,
    impl_plan: Option<&ImplPlanDocument>,
) -> NextCommand {
    resolve_phase(track, schema_version, impl_plan).next_command
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
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
    fn resolve_phase_branchless_v3_planned_returns_ready_to_activate() {
        let track = planned_track("demo", None);
        let info = resolve_phase(&track, 3, None);
        assert_eq!(info.phase, TrackPhase::ReadyToActivate);
        assert_eq!(info.next_command, NextCommand::ActivateTrack("demo".to_owned()));
        assert!(info.blocker.is_some());
    }

    #[test]
    fn resolve_phase_branchless_v4_planned_returns_ready_to_activate() {
        // Schema v4 is the new identity-only shape; branchless v4 must also activate.
        let track = planned_track("demo", None);
        let info = resolve_phase(&track, 4, None);
        assert_eq!(info.phase, TrackPhase::ReadyToActivate);
        assert_eq!(info.next_command, NextCommand::ActivateTrack("demo".to_owned()));
        assert!(info.blocker.is_some());
    }

    #[test]
    fn resolve_phase_branchless_v5_planned_returns_ready_to_activate() {
        // Schema v5 branchless tracks must also activate.
        let track = planned_track("demo", None);
        let info = resolve_phase(&track, 5, None);
        assert_eq!(info.phase, TrackPhase::ReadyToActivate);
        assert_eq!(info.next_command, NextCommand::ActivateTrack("demo".to_owned()));
        assert!(info.blocker.is_some());
    }

    #[test]
    fn resolve_phase_materialized_v3_planned_returns_planning() {
        let track = planned_track("demo", Some("track/demo"));
        let info = resolve_phase(&track, 3, None);
        assert_eq!(info.phase, TrackPhase::Planning);
        assert_eq!(info.next_command, NextCommand::Implement);
        assert!(info.blocker.is_none());
    }

    #[test]
    fn resolve_phase_materialized_v4_planned_returns_planning() {
        let track = planned_track("demo", Some("track/demo"));
        let info = resolve_phase(&track, 4, None);
        assert_eq!(info.phase, TrackPhase::Planning);
        assert_eq!(info.next_command, NextCommand::Implement);
        assert!(info.blocker.is_none());
    }

    #[test]
    fn resolve_phase_v2_branchless_planned_returns_planning_not_ready_to_activate() {
        let track = planned_track("demo", None);
        let info = resolve_phase(&track, 2, None);
        assert_eq!(info.phase, TrackPhase::Planning);
        assert_ne!(info.phase, TrackPhase::ReadyToActivate);
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
        let info = resolve_phase(&track, 5, Some(&impl_plan));
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
        let info = resolve_phase(&track, 5, Some(&impl_plan));
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
        let info = resolve_phase(&track, 5, None);
        assert_eq!(info.phase, TrackPhase::Blocked);
        assert!(info.blocker.unwrap().contains("waiting on review"));

        // Also verify set_status_override path.
        track.set_status_override(Some(StatusOverride::blocked("new reason").unwrap()));
        let info2 = resolve_phase(&track, 5, None);
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
        let info = resolve_phase(&track, 5, None);
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
    #[case::branchless_v4_planned_returns_ready_to_activate(
        TrackStatus::Planned,
        false,
        4,
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
    #[case::materialized_v4_planned_returns_planning(
        TrackStatus::Planned,
        true,
        4,
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
    fn next_command_for_branchless_v5_returns_activate() {
        let track = planned_track("demo", None);
        let cmd = next_command(&track, 5, None);
        assert_eq!(cmd, NextCommand::ActivateTrack("demo".to_owned()));
    }

    #[test]
    fn next_command_for_materialized_planned_returns_implement() {
        let track = planned_track("demo", Some("track/demo"));
        let cmd = next_command(&track, 5, None);
        assert_eq!(cmd, NextCommand::Implement);
    }
}
