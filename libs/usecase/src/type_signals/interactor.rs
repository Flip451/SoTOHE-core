//! `TypeSignalsInteractor` — implements [`TypeSignalsService`].
//!
//! Orchestrates the active-track guard (CN-07), layer-bindings resolution, and
//! per-layer signal evaluation. All I/O is performed via injected ports (no
//! direct infrastructure calls).

use std::sync::Arc;

use domain::tddd::catalogue_v2::{
    TdddLayerBindingsError, TdddLayerBindingsPort, TypeSignalsExecutorPort,
};

use super::service::{TypeSignalsError, TypeSignalsRequest, TypeSignalsService};

// ---------------------------------------------------------------------------
// Validate track_id
// ---------------------------------------------------------------------------

/// Validates a track ID string (lowercase slug).
///
/// Delegates to the canonical domain `TrackId::try_new` validation, mapping the
/// domain `ValidationError` into this module's `InvalidTrackId` variant so the
/// slug rule has a single source of truth (ADR D1).
///
/// # Errors
///
/// Returns `TypeSignalsError::InvalidTrackId` if the ID is invalid.
fn validate_track_id(id: &str) -> Result<(), TypeSignalsError> {
    domain::TrackId::try_new(id)
        .map(|_| ())
        .map_err(|e| TypeSignalsError::InvalidTrackId { reason: e.to_string() })
}

// ---------------------------------------------------------------------------
// Interactor
// ---------------------------------------------------------------------------

/// Interactor implementing [`TypeSignalsService`].
///
/// All I/O is performed via injected ports:
/// - [`TdddLayerBindingsPort`]: reads `architecture-rules.json`.
/// - [`TypeSignalsExecutorPort`]: runs the three-way signal evaluation pipeline
///   for a single layer.
///
/// The active-track guard (CN-07) runs before any I/O: the caller-supplied
/// `branch` string is checked for the `track/` prefix and the suffix is
/// matched against `track_id`. The interactor remains git-unaware — the CLI
/// resolves the current branch and passes it in the request.
///
/// `apps/cli` constructs the concrete infrastructure adapters at the
/// composition root and injects them.
pub struct TypeSignalsInteractor {
    layer_bindings: Arc<dyn TdddLayerBindingsPort>,
    executor: Arc<dyn TypeSignalsExecutorPort>,
}

impl TypeSignalsInteractor {
    /// Creates a new interactor with the given injected ports.
    #[must_use]
    pub fn new(
        layer_bindings: Arc<dyn TdddLayerBindingsPort>,
        executor: Arc<dyn TypeSignalsExecutorPort>,
    ) -> Self {
        Self { layer_bindings, executor }
    }
}

impl TypeSignalsService for TypeSignalsInteractor {
    /// Runs the type-signals evaluation.
    ///
    /// `items_dir` in the request is ignored; the interactor always derives it
    /// as `workspace_root/track/items` to avoid lexical-equality mismatches
    /// between relative and absolute caller-supplied paths.
    ///
    /// Steps:
    /// 1. Validate the track ID format (slug check).
    /// 2. Active-track guard (CN-07): check that `branch` starts with `track/`
    ///    and that the suffix matches `track_id`.
    /// 3. Derive `items_dir = workspace_root/track/items`.
    /// 4. Resolve layer bindings; fail-closed when no layers found.
    /// 5. For each layer, call `TypeSignalsExecutorPort::evaluate_layer`.
    ///    Absent catalogue files are always skipped unconditionally;
    ///    present catalogues are always evaluated strictly.
    ///
    /// # Errors
    ///
    /// Returns [`TypeSignalsError`] on any failure.
    fn run(&self, request: TypeSignalsRequest) -> Result<(), TypeSignalsError> {
        let TypeSignalsRequest { items_dir: _items_dir, track_id, branch, workspace_root, layer } =
            request;

        // Step 1: validate track_id.
        validate_track_id(&track_id)?;

        // Step 2: active-track guard (CN-07).
        // Reject non-`track/` branches and branch/track-id mismatches.
        // This mirrors `RefreshCatalogueSpecSignalsInteractor` exactly.
        let suffix = branch
            .strip_prefix("track/")
            .ok_or_else(|| TypeSignalsError::NonActiveTrack { branch: branch.clone() })?;
        if suffix != track_id.as_str() {
            return Err(TypeSignalsError::BranchTrackMismatch {
                branch: branch.clone(),
                track_id: track_id.clone(),
            });
        }

        // Derive `items_dir` from `workspace_root` so that the interactor is
        // robust to CLI callers that pass relative (`"track/items"`) or absolute
        // paths for these two fields independently.  Comparing raw user-supplied
        // `PathBuf`s with a lexical equality check would reject valid default
        // invocations (e.g. `workspace_root = $PWD`, `items_dir = "track/items"`
        // resolve to the same directory but fail an `==` comparison).
        let items_dir = workspace_root.join("track").join("items");

        // Step 3: resolve layer bindings.
        let bindings =
            self.layer_bindings.load(&workspace_root, layer.as_deref()).map_err(|e| match e {
                TdddLayerBindingsError::LoadFailed { reason } => {
                    TypeSignalsError::LayerBindingsLoad { reason }
                }
                TdddLayerBindingsError::LayerNotFound { layer_id } => {
                    TypeSignalsError::LayerBindingsLoad {
                        reason: format!(
                            "layer '{layer_id}' not found or not tddd.enabled in \
                             architecture-rules.json"
                        ),
                    }
                }
                TdddLayerBindingsError::NoLayers => TypeSignalsError::NoLayers,
            })?;

        if bindings.is_empty() {
            return Err(TypeSignalsError::NoLayers);
        }

        // Step 4: per-layer signal evaluation.
        // Absent catalogue files are always skipped unconditionally (no gate-vs-direct
        // distinction). Present catalogues are always evaluated strictly.
        for binding in &bindings {
            let layer_id = binding.layer_id.clone();
            self.executor.evaluate_layer(&items_dir, &track_id, &workspace_root, binding).map_err(
                |e| TypeSignalsError::EvaluationFailed { layer_id, reason: e.to_string() },
            )?;
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "interactor_tests.rs"]
mod tests;
