//! `TypeSignalsInteractor` — implements [`TypeSignalsService`].
//!
//! Orchestrates the active-track guard (CN-07), layer-bindings resolution, and
//! per-layer signal evaluation. All I/O is performed via injected ports (no
//! direct infrastructure calls).

use std::sync::Arc;

use domain::tddd::LayerId;
use domain::tddd::catalogue_v2::{TdddLayerBindingsError, TdddLayerBindingsPort};

use crate::git_workflow::DiagnosticText;
use crate::tddd_feature_declaration::TdddActualFeatureDeclarationPort;

use super::ports::TypeSignalsExecutorPort;
use super::service::{TypeSignalsError, TypeSignalsRequest, TypeSignalsService};

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
    feature_declaration: Arc<dyn TdddActualFeatureDeclarationPort>,
}

impl TypeSignalsInteractor {
    /// Creates a new interactor with the given injected ports.
    #[must_use]
    pub fn new(
        layer_bindings: Arc<dyn TdddLayerBindingsPort>,
        executor: Arc<dyn TypeSignalsExecutorPort>,
        feature_declaration: Arc<dyn TdddActualFeatureDeclarationPort>,
    ) -> Self {
        Self { layer_bindings, executor, feature_declaration }
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
    /// 5. Load and verify the frozen actual-capture feature declaration.
    /// 6. For each layer, call `TypeSignalsExecutorPort::evaluate_layer`, which
    ///    verifies its persisted freshness inputs before any reuse.
    ///
    /// # Errors
    ///
    /// Returns [`TypeSignalsError`] on any failure.
    fn run(&self, request: TypeSignalsRequest) -> Result<(), TypeSignalsError> {
        let TypeSignalsRequest { items_dir: _items_dir, track_id, branch, workspace_root, layer } =
            request;

        // `TrackId` and `TrackBranch` make malformed identities unrepresentable.
        // Retain the cross-field guard: the branch still must name this request's
        // track rather than another valid track.
        if branch.as_ref().strip_prefix("track/") != Some(track_id.as_ref()) {
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
        let bindings = self
            .layer_bindings
            .load(&workspace_root, layer.as_ref().map(AsRef::as_ref))
            .map_err(|e| match e {
                TdddLayerBindingsError::LoadFailed { reason } => {
                    TypeSignalsError::LayerBindingsLoad { reason: DiagnosticText::new(reason) }
                }
                TdddLayerBindingsError::LayerNotFound { layer_id } => {
                    TypeSignalsError::LayerBindingsLoad {
                        reason: DiagnosticText::new(format!(
                            "layer '{layer_id}' not found or not tddd.enabled in \
                             architecture-rules.json"
                        )),
                    }
                }
                TdddLayerBindingsError::NoLayers => TypeSignalsError::NoLayers,
            })?;

        if bindings.is_empty() {
            return Err(TypeSignalsError::NoLayers);
        }

        let typed_bindings = bindings
            .iter()
            .map(|binding| {
                LayerId::try_new(binding.layer_id.clone()).map(|layer_id| (binding, layer_id))
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| TypeSignalsError::InconsistentRequest {
                reason: DiagnosticText::new(format!(
                    "layer binding contains an invalid layer id: {error}"
                )),
            })?;

        let declaration_bindings = if layer.is_some() {
            self.layer_bindings.load(&workspace_root, None).map_err(|e| {
                TypeSignalsError::LayerBindingsLoad { reason: DiagnosticText::new(e.to_string()) }
            })?
        } else {
            bindings.clone()
        };
        let track_dir = items_dir.join(track_id.as_ref());
        let declaration = self
            .feature_declaration
            .load_for_actual(&track_dir, &workspace_root, &declaration_bindings)
            .map_err(TypeSignalsError::FeatureDeclaration)?;

        // Step 5: per-layer signal evaluation.
        // Absent catalogue files are always skipped unconditionally (no gate-vs-direct
        // distinction). Present catalogues are always evaluated strictly.
        for (binding, layer_id) in typed_bindings {
            let features = declaration.features_for(&layer_id).map_err(|error| {
                TypeSignalsError::InconsistentRequest {
                    reason: DiagnosticText::new(error.to_string()),
                }
            })?;
            self.executor
                .evaluate_layer(&items_dir, &track_id, &workspace_root, binding, features)
                .map_err(|e| TypeSignalsError::EvaluationFailed {
                    layer_id,
                    reason: DiagnosticText::new(e.to_string()),
                })?;
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;
