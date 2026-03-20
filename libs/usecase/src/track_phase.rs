//! Re-exports track phase resolution from the domain layer.
//!
//! The canonical implementation lives in `domain::track_phase`.
//! This module re-exports it for consumers that depend on the usecase crate.

pub use domain::track_phase::{
    NextCommand, TrackPhase, TrackPhaseInfo, next_command, resolve_phase, resolve_phase_from_record,
};
