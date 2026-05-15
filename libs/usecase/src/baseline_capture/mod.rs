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
/// Mirrors the domain `TrackId::try_new` validation without importing domain.
///
/// # Errors
///
/// Returns `BaselineCaptureError::InvalidTrackId` if the ID is invalid.
pub(crate) fn validate_track_id(id: &str) -> Result<(), BaselineCaptureError> {
    if id.is_empty() {
        return Err(BaselineCaptureError::InvalidTrackId {
            reason: "must not be empty".to_owned(),
        });
    }
    let mut chars = id.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() || first.is_ascii_digit() => {}
        _ => {
            return Err(BaselineCaptureError::InvalidTrackId {
                reason: format!(
                    "invalid track id: '{id}' (must start with lowercase letter or digit)"
                ),
            });
        }
    }
    let mut previous_was_hyphen = false;
    for ch in chars {
        let is_valid = ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-';
        if !is_valid {
            return Err(BaselineCaptureError::InvalidTrackId {
                reason: format!("invalid track id: '{id}' (invalid character '{ch}')"),
            });
        }
        if ch == '-' && previous_was_hyphen {
            return Err(BaselineCaptureError::InvalidTrackId {
                reason: format!("invalid track id: '{id}' (double hyphen not allowed)"),
            });
        }
        previous_was_hyphen = ch == '-';
    }
    if previous_was_hyphen {
        return Err(BaselineCaptureError::InvalidTrackId {
            reason: format!("invalid track id: '{id}' (must not end with hyphen)"),
        });
    }
    Ok(())
}
