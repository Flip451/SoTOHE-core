//! `BaselineCaptureInteractor` — implements [`BaselineCaptureService`].
//!
//! Orchestrates symlink guards, track-id validation, layer-bindings resolution,
//! and per-layer rustdoc baseline capture. All I/O is performed via injected
//! secondary ports (no direct infrastructure calls).

use std::path::Path;
use std::sync::Arc;

use domain::SymlinkGuardPort;
use domain::tddd::catalogue_v2::{
    RustdocBaselineCapturePort, TdddLayerBindingsError, TdddLayerBindingsPort,
};

use super::service::{BaselineCaptureError, BaselineCaptureRequest, BaselineCaptureService};
use super::validate_track_id;

// ---------------------------------------------------------------------------
// Interactor
// ---------------------------------------------------------------------------

/// Interactor implementing [`BaselineCaptureService`].
///
/// All I/O is performed via injected ports:
/// - [`SymlinkGuardPort`]: symlink stat checks (usecase-purity rule).
/// - [`TdddLayerBindingsPort`]: reads `architecture-rules.json`.
/// - [`RustdocBaselineCapturePort`]: runs `cargo +nightly rustdoc` and writes
///   the baseline file.
///
/// `apps/cli` constructs the concrete infrastructure adapters at the composition
/// root and injects them.
pub struct BaselineCaptureInteractor {
    symlink_guard: Arc<dyn SymlinkGuardPort>,
    layer_bindings: Arc<dyn TdddLayerBindingsPort>,
    capture: Arc<dyn RustdocBaselineCapturePort>,
}

impl BaselineCaptureInteractor {
    /// Creates a new interactor with the given injected ports.
    #[must_use]
    pub fn new(
        symlink_guard: Arc<dyn SymlinkGuardPort>,
        layer_bindings: Arc<dyn TdddLayerBindingsPort>,
        capture: Arc<dyn RustdocBaselineCapturePort>,
    ) -> Self {
        Self { symlink_guard, layer_bindings, capture }
    }
}

impl BaselineCaptureService for BaselineCaptureInteractor {
    /// Runs the baseline capture.
    ///
    /// Steps:
    /// 1. Validate the track ID format (slug check).
    /// 2. Guard `workspace_root` against path traversal (`..`) and symlinks.
    /// 3. Derive `items_dir = workspace_root/track/items` and guard it against symlinks.
    /// 4. Resolve layer bindings via `TdddLayerBindingsPort`.
    /// 5. Fail-closed if no layers found.
    /// 6. For each layer, call `RustdocBaselineCapturePort::capture`.
    ///
    /// # Errors
    ///
    /// Returns [`BaselineCaptureError`] on any failure.
    fn run(&self, request: BaselineCaptureRequest) -> Result<(), BaselineCaptureError> {
        let BaselineCaptureRequest { track_id, workspace_root, source_workspace, layer } = request;

        // Step 1: validate track_id.
        validate_track_id(&track_id)?;

        // Step 2: dot-dot rejection on workspace_root.
        for component in workspace_root.components() {
            use std::path::Component;
            if matches!(component, Component::ParentDir) {
                return Err(BaselineCaptureError::SymlinkRejected {
                    path: format!(
                        "workspace_root '{}' contains '..' (path traversal rejected)",
                        workspace_root.display()
                    ),
                });
            }
        }

        // Step 2: symlink guard on workspace_root (all ancestors from filesystem root).
        self.symlink_guard
            .reject_symlinks_from_root(&workspace_root)
            .map_err(|e| BaselineCaptureError::SymlinkRejected { path: e.to_string() })?;

        // Step 2b: guard source_workspace when it differs from workspace_root.
        if let Some(ref src) = source_workspace {
            for component in src.components() {
                use std::path::Component;
                if matches!(component, Component::ParentDir) {
                    return Err(BaselineCaptureError::SymlinkRejected {
                        path: format!(
                            "source_workspace '{}' contains '..' (path traversal rejected)",
                            src.display()
                        ),
                    });
                }
            }
            self.symlink_guard
                .reject_symlinks_from_root(src)
                .map_err(|e| BaselineCaptureError::SymlinkRejected { path: e.to_string() })?;
        }

        // Step 3: derive items_dir and guard it.
        let items_dir = workspace_root.join("track").join("items");
        self.symlink_guard
            .reject_symlinks_from_root(&items_dir)
            .map_err(|e| BaselineCaptureError::SymlinkRejected { path: e.to_string() })?;

        // Step 4: resolve layer bindings.
        let bindings =
            self.layer_bindings.load(&workspace_root, layer.as_deref()).map_err(|e| match e {
                TdddLayerBindingsError::LoadFailed { reason } => {
                    BaselineCaptureError::LayerBindingsLoad { reason }
                }
                TdddLayerBindingsError::LayerNotFound { layer_id } => {
                    BaselineCaptureError::LayerBindingsLoad {
                        reason: format!(
                            "layer '{layer_id}' not found or not tddd.enabled in \
                             architecture-rules.json"
                        ),
                    }
                }
                TdddLayerBindingsError::NoLayers => BaselineCaptureError::NoLayers,
            })?;

        // Step 5: fail-closed when no layers found.
        if bindings.is_empty() {
            return Err(BaselineCaptureError::NoLayers);
        }

        // Resolve the rustdoc source workspace (defaults to workspace_root).
        let rustdoc_workspace: &Path = source_workspace.as_deref().unwrap_or(&workspace_root);

        // Step 6: per-layer capture.
        for binding in &bindings {
            let layer_id = binding.layer_id.clone();
            self.capture
                .capture(&items_dir, &track_id, rustdoc_workspace, binding)
                .map_err(|e| BaselineCaptureError::CaptureFailed { layer_id, reason: e.0 })?;
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests (in a sibling file to keep interactor.rs under the module-size limit)
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "interactor_tests.rs"]
mod tests;
