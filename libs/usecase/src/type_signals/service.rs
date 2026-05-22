//! `TypeSignalsService` — application service trait and request/error types for
//! the `sotp track type-signals` use case.

use std::path::PathBuf;

use thiserror::Error;

/// Request DTO for [`TypeSignalsService::run`].
pub struct TypeSignalsRequest {
    /// Track directory root (`workspace_root/track/items`).
    ///
    /// Note: the [`crate::type_signals::TypeSignalsInteractor`] always derives
    /// this value from `workspace_root` and ignores the caller-supplied path.
    /// The field is retained for forward-compatibility and testing convenience.
    pub items_dir: PathBuf,
    /// Track identifier slug (e.g. `"my-track-2026-01-01"`).
    pub track_id: String,
    /// Current git branch (e.g. `"track/my-feature-2026-04-24"`). Used by the
    /// active-track guard (CN-07) to reject non-`track/` branches and
    /// branch/track-id mismatches.
    pub branch: String,
    /// Cargo workspace root used for rustdoc export.
    pub workspace_root: PathBuf,
    /// Optional layer filter (`--layer`). When `None`, all TDDD-enabled layers
    /// are processed.
    pub layer: Option<String>,
    /// When `true`, absent catalogue files are silently skipped instead of
    /// failing (pre-commit lenient mode).
    pub lenient: bool,
}

/// Error variants for [`TypeSignalsService::run`].
#[derive(Debug, Error)]
pub enum TypeSignalsError {
    /// The track ID is not a valid slug.
    #[error("invalid track id: {reason}")]
    InvalidTrackId {
        /// Human-readable reason.
        reason: String,
    },
    /// The supplied branch does not start with `track/`, so the guard
    /// (CN-07) rejects it to keep type-signals off archived / main / plan/
    /// branches.
    #[error("type-signals rejected: branch '{branch}' is not an active track branch (CN-07)")]
    NonActiveTrack {
        /// The branch name that triggered the guard.
        branch: String,
    },
    /// The branch `track/<suffix>` disagrees with the track_id argument.
    /// Safeguards against CLI wrappers that mishandle branch/track_id mapping.
    #[error(
        "type-signals rejected: branch '{branch}' does not match track_id '{track_id}' \
         (expected 'track/{track_id}')"
    )]
    BranchTrackMismatch {
        /// The branch name that triggered the guard.
        branch: String,
        /// The track identifier from the request.
        track_id: String,
    },
    /// `architecture-rules.json` could not be loaded or a specific layer was
    /// not found.
    #[error("layer bindings load failed: {reason}")]
    LayerBindingsLoad {
        /// Human-readable reason.
        reason: String,
    },
    /// No TDDD-enabled layers were found.
    #[error(
        "no tddd.enabled layers found in architecture-rules.json; \
         nothing to evaluate"
    )]
    NoLayers,
    /// Signal evaluation failed for the given layer.
    #[error("type-signals evaluation failed for layer '{layer_id}': {reason}")]
    EvaluationFailed {
        /// Layer id for which evaluation failed.
        layer_id: String,
        /// Human-readable reason.
        reason: String,
    },
    /// The request contains an inconsistent combination of fields.
    #[error("inconsistent request: {reason}")]
    InconsistentRequest {
        /// Human-readable reason.
        reason: String,
    },
}

/// Application service trait for the `sotp track type-signals` use case.
///
/// The interactor [`crate::type_signals::TypeSignalsInteractor`] implements this
/// trait by orchestrating:
/// 1. Track-ID validation.
/// 2. Track-status guard (active-track check).
/// 3. Layer-bindings resolution.
/// 4. Per-layer signal evaluation (strict or lenient).
pub trait TypeSignalsService: Send + Sync {
    /// Runs the type-signals evaluation for the given request.
    ///
    /// # Errors
    ///
    /// Returns [`TypeSignalsError`] on any failure (invalid track id, frozen
    /// track, missing layer binding, or evaluation failure).
    fn run(&self, request: TypeSignalsRequest) -> Result<(), TypeSignalsError>;
}
