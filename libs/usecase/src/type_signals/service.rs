//! `TypeSignalsService` — application service trait and request/error types for
//! the `sotp signal calc-impl-catalog` use case.

use std::path::PathBuf;

use domain::tddd::LayerId;
use domain::{TrackBranch, TrackId};
use thiserror::Error;

use crate::git_workflow::DiagnosticText;
use crate::tddd_feature_declaration::TdddActualFeatureDeclarationPortError;

/// Request DTO for [`TypeSignalsService::run`].
pub struct TypeSignalsRequest {
    /// Track directory root (`workspace_root/track/items`).
    ///
    /// Note: the [`crate::type_signals::TypeSignalsInteractor`] always derives
    /// this value from `workspace_root` and ignores the caller-supplied path.
    /// The field is retained for forward-compatibility and testing convenience.
    pub items_dir: PathBuf,
    /// Track identifier slug (e.g. `"my-track-2026-01-01"`).
    pub track_id: TrackId,
    /// Current git branch (e.g. `"track/my-feature-2026-04-24"`). Used by the
    /// active-track guard (CN-07) to reject non-`track/` branches and
    /// branch/track-id mismatches.
    pub branch: TrackBranch,
    /// Cargo workspace root used for rustdoc export.
    pub workspace_root: PathBuf,
    /// Optional layer filter (`--layer`). When `None`, all TDDD-enabled layers
    /// are processed.
    pub layer: Option<LayerId>,
}

/// Error variants for [`TypeSignalsService::run`].
#[derive(Debug, Error)]
pub enum TypeSignalsError {
    /// The branch `track/<suffix>` disagrees with the track_id argument.
    /// Safeguards against CLI wrappers that mishandle branch/track_id mapping.
    #[error(
        "type-signals rejected: branch '{branch}' does not match track_id '{track_id}' \
         (expected 'track/{track_id}')"
    )]
    BranchTrackMismatch {
        /// The branch name that triggered the guard.
        branch: TrackBranch,
        /// The track identifier from the request.
        track_id: TrackId,
    },
    /// `architecture-rules.json` could not be loaded or a specific layer was
    /// not found.
    #[error("layer bindings load failed: {reason}")]
    LayerBindingsLoad {
        /// Human-readable reason.
        reason: DiagnosticText,
    },
    /// No TDDD-enabled layers were found.
    #[error(
        "no tddd.enabled layers found in architecture-rules.json; \
         nothing to evaluate"
    )]
    NoLayers,
    /// The frozen actual-capture feature declaration could not be loaded or verified.
    #[error("feature declaration error: {0}")]
    FeatureDeclaration(TdddActualFeatureDeclarationPortError),
    /// A required input for the named layer could not be loaded or validated.
    #[error("authoritative input failed for layer '{layer_id}': {reason}")]
    AuthoritativeInputFailed {
        /// Layer whose authoritative input failed.
        layer_id: LayerId,
        /// Human-readable diagnostic.
        reason: DiagnosticText,
    },
    /// Signal evaluation failed for the given layer.
    #[error("type-signals evaluation failed for layer '{layer_id}': {reason}")]
    EvaluationFailed {
        /// Layer id for which evaluation failed.
        layer_id: LayerId,
        /// Human-readable reason.
        reason: DiagnosticText,
    },
    /// A refreshed cache for the named layer could not be persisted.
    #[error("type-signals cache write failed for layer '{layer_id}': {reason}")]
    CacheWriteFailed {
        /// Layer whose cache write failed.
        layer_id: LayerId,
        /// Human-readable diagnostic.
        reason: DiagnosticText,
    },
    /// The request contains an inconsistent combination of fields.
    #[error("inconsistent request: {reason}")]
    InconsistentRequest {
        /// Human-readable reason.
        reason: DiagnosticText,
    },
}

/// Application service trait for the `sotp signal calc-impl-catalog` use case.
///
/// The interactor [`crate::type_signals::TypeSignalsInteractor`] implements this
/// trait by orchestrating:
/// 1. Track-ID validation.
/// 2. Track-status guard (active-track check).
/// 3. Layer-bindings resolution.
/// 4. Per-layer signal evaluation with conservative freshness verification.
pub trait TypeSignalsService: Send + Sync {
    /// Runs the type-signals evaluation for the given request.
    ///
    /// # Errors
    ///
    /// Returns [`TypeSignalsError`] on any failure (invalid track id, frozen
    /// track, missing layer binding, or evaluation failure).
    fn run(&self, request: TypeSignalsRequest) -> Result<(), TypeSignalsError>;
}
