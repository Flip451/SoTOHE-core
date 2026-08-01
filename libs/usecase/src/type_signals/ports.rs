//! Secondary ports used by the type-signals application service.

use std::fmt;
use std::path::Path;

use domain::TrackId;
use domain::tddd::CargoFeatureName;
use domain::tddd::catalogue_v2::TdddLayerBinding;

use crate::git_workflow::DiagnosticText;

/// Failure stage reported by the infrastructure evaluator adapter.
#[derive(Debug, Clone)]
pub enum TypeSignalsExecutionError {
    /// A required authoritative input could not be loaded or validated.
    AuthoritativeInput(DiagnosticText),
    /// Signal evaluation could not complete.
    Evaluation(DiagnosticText),
    /// The refreshed cache could not be persisted.
    CacheWrite(DiagnosticText),
}

impl fmt::Display for TypeSignalsExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AuthoritativeInput(reason)
            | Self::Evaluation(reason)
            | Self::CacheWrite(reason) => formatter.write_str(reason.as_str()),
        }
    }
}

impl std::error::Error for TypeSignalsExecutionError {}

/// Runs the blocking per-layer rustdoc and signal-evaluation pipeline.
pub trait TypeSignalsExecutorPort: Send + Sync {
    /// Evaluates and persists type signals for one resolved layer.
    ///
    /// The adapter decides reuse only after verifying all freshness inputs.
    /// Missing or unverifiable inputs are returned to the application service.
    fn evaluate_layer(
        &self,
        items_dir: &Path,
        track_id: &TrackId,
        workspace_root: &Path,
        binding: &TdddLayerBinding,
        features: &[CargoFeatureName],
    ) -> Result<(), TypeSignalsExecutionError>;
}
