//! Secondary ports used by the type-signals application service.

use std::fmt;
use std::path::Path;

use domain::TrackId;
use domain::tddd::catalogue_v2::TdddLayerBinding;

/// Opaque diagnostic returned by the infrastructure evaluator adapter.
#[derive(Debug, Clone)]
pub struct TypeSignalsExecutionError(pub String);

impl fmt::Display for TypeSignalsExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
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
    ) -> Result<(), TypeSignalsExecutionError>;
}
