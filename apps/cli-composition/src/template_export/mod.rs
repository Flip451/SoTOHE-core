//! `template` command family — composition root.
//!
//! [`TemplateCompositionRoot`] wires the filesystem boundary-manifest adapter
//! (`FsTemplateBoundaryManifestAdapter`), the filesystem export adapter
//! (`FsTemplateExportAdapter`), the use-case interactor
//! (`TemplateExportInteractor`), and [`cli_driver::template_export::TemplateDriver`]
//! for the `sotp template export` subcommand.
//!
//! `handle` accepts the driver input DTO already assembled by the `cli` layer.
//! See IN-01, AC-01.

use std::sync::Arc;

use cli_driver::CommandOutcome;
use cli_driver::template_export::{TemplateDriver, TemplateInput};

use crate::error::CompositionError;

/// Composition root for the `template` command family.
///
/// Wires `FsTemplateBoundaryManifestAdapter` + `FsTemplateExportAdapter` →
/// `TemplateExportInteractor` → `TemplateDriver`.
#[derive(Debug, Default)]
pub struct TemplateCompositionRoot;

impl TemplateCompositionRoot {
    /// Create a new `TemplateCompositionRoot`.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Build a fully-wired [`TemplateDriver`].
    ///
    /// Wire chain: `FsTemplateBoundaryManifestAdapter` + `FsTemplateExportAdapter`
    /// → `TemplateExportInteractor` → `TemplateDriver`.
    #[must_use]
    pub fn template_driver(&self) -> TemplateDriver {
        use infrastructure::template_export::{
            FsTemplateBoundaryManifestAdapter, FsTemplateExportAdapter,
        };
        use usecase::template_export::{
            TemplateBoundaryManifestPort, TemplateExportInteractor, TemplateExportPort,
            TemplateExportService,
        };

        let manifest_port: Arc<dyn TemplateBoundaryManifestPort> =
            Arc::new(FsTemplateBoundaryManifestAdapter::new());
        let export_port: Arc<dyn TemplateExportPort> = Arc::new(FsTemplateExportAdapter::new());
        let service: Arc<dyn TemplateExportService> =
            Arc::new(TemplateExportInteractor::new(manifest_port, export_port));
        TemplateDriver::new(service)
    }

    /// Wire and dispatch a `template` command through the full stack.
    ///
    /// # Errors
    ///
    /// Returns [`CompositionError`] if composition fails. Wiring is currently
    /// infallible; the signature preserves room for future wiring errors (e.g.
    /// config loading), matching the sibling composition roots.
    pub fn handle(&self, input: TemplateInput) -> Result<CommandOutcome, CompositionError> {
        Ok(self.template_driver().handle(input))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::path::PathBuf;

    use cli_driver::template_export::{TemplateExportInput, TemplateInput};

    use super::TemplateCompositionRoot;

    /// Composition-root smoke test: the wired stack routes an `Export` input all
    /// the way through `FsTemplateBoundaryManifestAdapter` → interactor →
    /// `TemplateDriver`. A missing manifest surfaces as a mapped failure
    /// (`exit_code == 1` with a stderr message), exercising the full wiring
    /// without a filesystem fixture.
    #[test]
    fn export_with_missing_manifest_wires_and_maps_to_exit_1() {
        let root = TemplateCompositionRoot::new();
        let input = TemplateInput::Export(TemplateExportInput {
            workspace_root: PathBuf::from("/nonexistent/ws"),
            manifest_path: PathBuf::from("/nonexistent/ws/boundary.json"),
            overlay_dir: PathBuf::from("/nonexistent/ws/overlay"),
            output_dir: PathBuf::from("/nonexistent/out"),
        });

        let outcome = root.handle(input).unwrap();

        assert_eq!(outcome.exit_code, 1, "missing manifest must map to exit 1: {outcome:?}");
        assert_eq!(outcome.stdout, None, "failure path must not emit stdout");
        assert!(
            outcome.stderr.is_some(),
            "the underlying manifest-read error must be surfaced on stderr"
        );
    }
}
