//! `BaselineCaptureService` — driving port and error type.
//!
//! Defines the application service trait and the unified error enum for the
//! `bin/sotp track baseline-capture` use case.

use std::path::PathBuf;

use thiserror::Error;

// ---------------------------------------------------------------------------
// Request type
// ---------------------------------------------------------------------------

/// Input parameters for [`BaselineCaptureService::run`].
#[derive(Debug, Clone)]
pub struct BaselineCaptureRequest {
    /// Track ID slug (e.g. `"tddd-v2-2026-05-08"`).
    pub track_id: String,
    /// Root of the Cargo workspace that owns `track/items/`.
    pub workspace_root: PathBuf,
    /// Cargo workspace from which `cargo +nightly rustdoc` is invoked.
    /// When `None`, defaults to `workspace_root` (standard flow).
    /// When `Some`, differs from `workspace_root` (git-worktree capture flow).
    pub source_workspace: Option<PathBuf>,
    /// Optional layer filter (matches `layers[].crate` in `architecture-rules.json`).
    /// When `None`, all TDDD-enabled layers are processed.
    pub layer: Option<String>,
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Error type for [`BaselineCaptureService::run`].
#[derive(Debug, Error)]
pub enum BaselineCaptureError {
    /// The track ID format is invalid.
    #[error("invalid track id: {reason}")]
    InvalidTrackId {
        /// Human-readable reason from the domain validator.
        reason: String,
    },
    /// A symlink was found in a guarded path.
    #[error("symlink guard rejected path: {path}")]
    SymlinkRejected {
        /// The rejected path (as a string for Display).
        path: String,
    },
    /// Failed to load the TDDD layer bindings from `architecture-rules.json`.
    #[error("layer bindings load failed: {reason}")]
    LayerBindingsLoad {
        /// Human-readable reason.
        reason: String,
    },
    /// No TDDD-enabled layers found.
    #[error("no TDDD-enabled layers found in architecture-rules.json")]
    NoLayers,
    /// The rustdoc baseline capture failed for a specific layer.
    #[error("baseline capture failed for layer '{layer_id}': {reason}")]
    CaptureFailed {
        /// Layer id for which capture failed.
        layer_id: String,
        /// Human-readable reason.
        reason: String,
    },
}

// ---------------------------------------------------------------------------
// Service trait
// ---------------------------------------------------------------------------

/// Application service (driving port) for the `bin/sotp track baseline-capture`
/// use case.
///
/// Orchestrates symlink guards, track-id validation, layer-bindings resolution,
/// and per-layer rustdoc baseline capture. All I/O is performed via injected
/// secondary ports — no direct infrastructure calls.
pub trait BaselineCaptureService: Send + Sync {
    /// Runs the baseline capture for the given request.
    ///
    /// # Errors
    ///
    /// Returns [`BaselineCaptureError`] on any failure (see variant docs).
    fn run(&self, request: BaselineCaptureRequest) -> Result<(), BaselineCaptureError>;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_baseline_capture_error_display_covers_all_variants() {
        let variants = [
            BaselineCaptureError::InvalidTrackId { reason: "test reason".to_owned() },
            BaselineCaptureError::SymlinkRejected { path: "/tmp/link".to_owned() },
            BaselineCaptureError::LayerBindingsLoad { reason: "test reason".to_owned() },
            BaselineCaptureError::NoLayers,
            BaselineCaptureError::CaptureFailed {
                layer_id: "domain".to_owned(),
                reason: "test reason".to_owned(),
            },
        ];
        for v in &variants {
            let msg = v.to_string();
            assert!(!msg.is_empty(), "Display must produce non-empty output for {v:?}");
        }
    }
}
