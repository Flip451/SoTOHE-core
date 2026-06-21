//! `BaselineCaptureService` and `BaselineCaptureInteractor`.
//!
//! Application service (driving port) and interactor for the
//! `bin/sotp track baseline-capture` use case.
//!
//! Orchestrates symlink guards, track-id validation, layer-bindings resolution,
//! and per-layer rustdoc baseline capture. All I/O is performed via injected
//! secondary ports — no direct infrastructure calls.

mod interactor;
mod service;

pub use interactor::BaselineCaptureInteractor;
pub use service::{BaselineCaptureError, BaselineCaptureRequest, BaselineCaptureService};

// ---------------------------------------------------------------------------
// Private helpers (shared across submodules)
// ---------------------------------------------------------------------------

/// Validates a track ID string (lowercase slug: `[a-z0-9]([a-z0-9-]*[a-z0-9])?`).
///
/// Delegates to the canonical domain `TrackId::try_new` validation, mapping the
/// domain `ValidationError` into this module's `InvalidTrackId` variant so the
/// slug rule has a single source of truth (ADR D1).
///
/// # Errors
///
/// Returns `BaselineCaptureError::InvalidTrackId` if the ID is invalid.
pub(crate) fn validate_track_id(id: &str) -> Result<(), BaselineCaptureError> {
    domain::TrackId::try_new(id)
        .map(|_| ())
        .map_err(|e| BaselineCaptureError::InvalidTrackId { reason: e.to_string() })
}
