//! `TypeSignalsInteractor` — implements [`TypeSignalsService`].
//!
//! Orchestrates track-status guard, layer-bindings resolution, and per-layer
//! signal evaluation. All I/O is performed via injected ports (no direct
//! infrastructure calls).

use std::sync::Arc;

use domain::tddd::catalogue_v2::{
    MissingCataloguePolicy, TdddLayerBindingsError, TdddLayerBindingsPort, TrackStatusReaderPort,
    TypeSignalsExecutorPort,
};

use super::service::{TypeSignalsError, TypeSignalsRequest, TypeSignalsService};

// ---------------------------------------------------------------------------
// Validate track_id
// ---------------------------------------------------------------------------

/// Validates a track ID string (lowercase slug).
///
/// Mirrors the domain `TrackId::try_new` validation without importing domain
/// types (the validation logic is duplicated to avoid a direct domain dep in
/// the module-level helper; the interactor uses it before injecting domain).
///
/// # Errors
///
/// Returns `TypeSignalsError::InvalidTrackId` if the ID is invalid.
fn validate_track_id(id: &str) -> Result<(), TypeSignalsError> {
    if id.is_empty() {
        return Err(TypeSignalsError::InvalidTrackId { reason: "must not be empty".to_owned() });
    }
    let mut chars = id.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() || first.is_ascii_digit() => {}
        _ => {
            return Err(TypeSignalsError::InvalidTrackId {
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
            return Err(TypeSignalsError::InvalidTrackId {
                reason: format!("invalid track id: '{id}' (invalid character '{ch}')"),
            });
        }
        if ch == '-' && previous_was_hyphen {
            return Err(TypeSignalsError::InvalidTrackId {
                reason: format!("invalid track id: '{id}' (double hyphen not allowed)"),
            });
        }
        previous_was_hyphen = ch == '-';
    }
    if previous_was_hyphen {
        return Err(TypeSignalsError::InvalidTrackId {
            reason: format!("invalid track id: '{id}' (must not end with hyphen)"),
        });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Interactor
// ---------------------------------------------------------------------------

/// Interactor implementing [`TypeSignalsService`].
///
/// All I/O is performed via injected ports:
/// - [`TrackStatusReaderPort`]: reads derived track status from metadata.json +
///   impl-plan.json (symlink-guarded).
/// - [`TdddLayerBindingsPort`]: reads `architecture-rules.json`.
/// - [`TypeSignalsExecutorPort`]: runs the three-way signal evaluation pipeline
///   for a single layer.
///
/// `apps/cli` constructs the concrete infrastructure adapters at the
/// composition root and injects them.
pub struct TypeSignalsInteractor {
    status_reader: Arc<dyn TrackStatusReaderPort>,
    layer_bindings: Arc<dyn TdddLayerBindingsPort>,
    executor: Arc<dyn TypeSignalsExecutorPort>,
}

impl TypeSignalsInteractor {
    /// Creates a new interactor with the given injected ports.
    #[must_use]
    pub fn new(
        status_reader: Arc<dyn TrackStatusReaderPort>,
        layer_bindings: Arc<dyn TdddLayerBindingsPort>,
        executor: Arc<dyn TypeSignalsExecutorPort>,
    ) -> Self {
        Self { status_reader, layer_bindings, executor }
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
    /// 2. Derive `items_dir = workspace_root/track/items`.
    /// 3. Read the track status; reject frozen tracks (Done/Archived).
    /// 4. Resolve layer bindings; fail-closed when no layers found.
    /// 5. For each layer, call `TypeSignalsExecutorPort::evaluate_layer`.
    ///
    /// # Errors
    ///
    /// Returns [`TypeSignalsError`] on any failure.
    fn run(&self, request: TypeSignalsRequest) -> Result<(), TypeSignalsError> {
        let TypeSignalsRequest { items_dir: _items_dir, track_id, workspace_root, layer, lenient } =
            request;

        // Step 1: validate track_id.
        validate_track_id(&track_id)?;

        // Derive `items_dir` from `workspace_root` so that the interactor is
        // robust to CLI callers that pass relative (`"track/items"`) or absolute
        // paths for these two fields independently.  Comparing raw user-supplied
        // `PathBuf`s with a lexical equality check would reject valid default
        // invocations (e.g. `workspace_root = $PWD`, `items_dir = "track/items"`
        // resolve to the same directory but fail an `==` comparison).
        let items_dir = workspace_root.join("track").join("items");

        // Step 2: read track status and guard against frozen tracks.
        let status = self
            .status_reader
            .read_status(&items_dir, &track_id)
            .map_err(|e| TypeSignalsError::StatusReadFailed { reason: e.to_string() })?;

        if !status.is_active() {
            return Err(TypeSignalsError::TrackFrozen {
                track_id: track_id.clone(),
                status: status.to_string(),
            });
        }

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
        let policy = if lenient {
            MissingCataloguePolicy::SkipSilently
        } else {
            MissingCataloguePolicy::FailClosed
        };

        for binding in &bindings {
            let layer_id = binding.layer_id.clone();
            self.executor
                .evaluate_layer(&items_dir, &track_id, &workspace_root, binding, policy)
                .map_err(|e| TypeSignalsError::EvaluationFailed {
                    layer_id,
                    reason: e.to_string(),
                })?;
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
