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
