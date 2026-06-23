//! Demo workflow wrapper.
//!
//! Encapsulates the example track state machine demo so that the CLI
//! composition root (`main.rs`) never imports domain types directly
//! (CN-01 / AC-03).

use std::sync::Arc;

use crate::InMemoryTrackStore;
use usecase::SaveTrackUseCase;

// ---------------------------------------------------------------------------
// Port adapter (T023)
// ---------------------------------------------------------------------------

/// Filesystem adapter that implements [`usecase::demo::DemoPort`].
///
/// Wraps the module-level [`run_example_demo`] free function.
pub struct FsDemoAdapter;

impl FsDemoAdapter {
    /// Create a new `FsDemoAdapter`.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for FsDemoAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl usecase::demo::DemoPort for FsDemoAdapter {
    fn run(&self) -> Result<String, usecase::demo::DemoPortError> {
        run_example_demo().map_err(usecase::demo::DemoPortError::Unavailable)
    }
}

// ---------------------------------------------------------------------------
// Infrastructure function
// ---------------------------------------------------------------------------

/// Runs the example track state machine demo.
///
/// Creates an in-memory track, persists it via `SaveTrackUseCase`, derives
/// its status, and returns a display string.
///
/// # Errors
///
/// Returns a `String` error message if track creation or persistence fails.
pub fn run_example_demo() -> Result<String, String> {
    let store = Arc::new(InMemoryTrackStore::new());
    let save = SaveTrackUseCase::new(Arc::clone(&store));

    let id = domain::TrackId::try_new("track-state-machine")
        .map_err(|e| format!("failed to build example track: {e}"))?;
    let track = domain::TrackMetadata::new(id, "Track state machine", None)
        .map_err(|e| format!("failed to build example track: {e}"))?;

    save.execute(&track).map_err(|e| format!("failed to save example track: {e}"))?;

    let status = domain::derive_track_status(None, track.status_override());
    Ok(format!("SoTOHE-core CLI stub: '{}' is {status}", track.id()))
}
