//! `BaselineCaptureService` — driving port and error type.
//!
//! Defines the application service trait and the unified error enum for the
//! `bin/sotp track baseline-capture` use case.

use std::path::PathBuf;

use crate::tddd_feature_declaration::TdddBaselineFeatureDeclarationPortError;
use domain::tddd::LayerId;
use domain::tddd::test_obligation::ids::DiagnosticMessage;
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
    #[error("invalid track id: {}", .0.as_str())]
    InvalidTrackId(DiagnosticMessage),
    /// A symlink was found in a guarded path.
    #[error("symlink guard rejected path: {}", .0.display())]
    SymlinkRejected(std::path::PathBuf),
    /// A symlink guard could not inspect a path because of an I/O failure.
    #[error("symlink guard I/O error for path '{}': {}", .0.display(), .1.as_str())]
    SymlinkGuardIo(std::path::PathBuf, DiagnosticMessage),
    /// Failed to load the TDDD layer bindings from `architecture-rules.json`.
    #[error("layer bindings load failed: {}", .0.as_str())]
    LayerBindingsLoad(DiagnosticMessage),
    /// No TDDD-enabled layers found.
    #[error("no TDDD-enabled layers found in architecture-rules.json")]
    NoLayers,
    /// The rustdoc baseline capture failed for a specific layer.
    #[error("baseline capture failed for layer '{}': {}", .0, .1.as_str())]
    CaptureFailed(LayerId, DiagnosticMessage),
    /// The feature declaration could not be loaded or frozen for baseline capture.
    #[error("feature declaration failed: {0}")]
    FeatureDeclaration(TdddBaselineFeatureDeclarationPortError),
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
            BaselineCaptureError::InvalidTrackId(diagnostic("test reason")),
            BaselineCaptureError::SymlinkRejected(std::path::PathBuf::from("/tmp/link")),
            BaselineCaptureError::SymlinkGuardIo(
                std::path::PathBuf::from("/tmp/unreadable"),
                diagnostic("permission denied"),
            ),
            BaselineCaptureError::LayerBindingsLoad(diagnostic("test reason")),
            BaselineCaptureError::NoLayers,
            BaselineCaptureError::CaptureFailed(layer("domain"), diagnostic("test reason")),
            BaselineCaptureError::FeatureDeclaration(
                TdddBaselineFeatureDeclarationPortError::BaselineSnapshotMismatch,
            ),
        ];
        for v in &variants {
            let msg = v.to_string();
            assert!(!msg.is_empty(), "Display must produce non-empty output for {v:?}");
        }
    }

    fn diagnostic(value: &str) -> DiagnosticMessage {
        DiagnosticMessage::try_new(value.to_owned()).unwrap()
    }

    fn layer(value: &str) -> LayerId {
        LayerId::try_new(value.to_owned()).unwrap()
    }
}
