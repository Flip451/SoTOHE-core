//! `template` command family — composition root.
//!
//! [`TemplateCompositionRoot`] wires the filesystem boundary-manifest adapter
//! (`FsTemplateBoundaryManifestAdapter`), the filesystem export adapter
//! (`FsTemplateExportAdapter`), the self-binary transplant adapter
//! (`FsSelfBinaryTransplantAdapter`), the use-case interactor
//! (`TemplateExportInteractor`), and [`cli_driver::template_export::TemplateDriver`]
//! for the `sotp template export` subcommand.
//!
//! `handle` accepts the driver input DTO already assembled by the `cli` layer.
//! See IN-01, AC-01, and ADR 2026-07-08-0541 D1.

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
    /// Wire chain: `FsTemplateBoundaryManifestAdapter` +
    /// `FsTemplateExportAdapter` + `FsSelfBinaryTransplantAdapter` →
    /// `TemplateExportInteractor` → `TemplateDriver`.
    #[must_use]
    pub fn template_driver(&self) -> TemplateDriver {
        use infrastructure::template_export::{
            FsSelfBinaryTransplantAdapter, FsTemplateBoundaryManifestAdapter,
            FsTemplateExportAdapter,
        };
        use usecase::template_export::{
            SelfBinaryTransplantPort, TemplateBoundaryManifestPort, TemplateExportInteractor,
            TemplateExportPort, TemplateExportService,
        };

        let manifest_port: Arc<dyn TemplateBoundaryManifestPort> =
            Arc::new(FsTemplateBoundaryManifestAdapter::new());
        let export_port: Arc<dyn TemplateExportPort> = Arc::new(FsTemplateExportAdapter::new());
        let transplant_port: Arc<dyn SelfBinaryTransplantPort> =
            Arc::new(FsSelfBinaryTransplantAdapter::new());
        let service: Arc<dyn TemplateExportService> =
            Arc::new(TemplateExportInteractor::new(manifest_port, export_port, transplant_port));
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
    use std::path::{Path, PathBuf};

    use cli_driver::template_export::{TemplateExportInput, TemplateInput};
    use tempfile::TempDir;

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

    fn write_file(root: &Path, rel: &str, content: &str) {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    /// End-to-end composition-root smoke test (ADR 2026-07-08-0541 D1, spec
    /// AC-01, AC-02, AC-09, AC-10): running the wired stack against a
    /// tempdir-backed workspace fixture must produce `<output_dir>/bin/sotp`
    /// byte-identical to the running binary and (on unix) executable. Verifies
    /// that the composition root injects `FsSelfBinaryTransplantAdapter` and
    /// that the transplant step runs after the export succeeds. The `--help`
    /// exit-0 check is covered by the Makefile smoke gate; running the test
    /// binary with `--help` would not necessarily behave like `sotp --help`.
    #[test]
    fn export_transplants_running_binary_into_bin_sotp() {
        let dir = TempDir::new().unwrap();
        let root_dir = dir.path();
        // Minimal workspace tree: an included subtree, an overlay-classified
        // file with an overlay counterpart, and an excluded directory.
        write_file(root_dir, "workspace/libs/domain/src/lib.rs", "// domain\n");
        write_file(root_dir, "workspace/Makefile.toml", "# real\n");
        write_file(root_dir, "workspace/vendor/blob.bin", "excluded\n");
        write_file(root_dir, "overlay/Makefile.toml", "# template\n");
        write_file(
            root_dir,
            "boundary.json",
            r#"{
  "schema_version": 1,
  "entries": [
    { "pattern": "libs/domain", "classification": "include" },
    { "pattern": "Makefile.toml", "classification": "overlay" },
    { "pattern": "vendor", "classification": "exclude" }
  ]
}"#,
        );

        let output_dir = root_dir.join("out");
        let composition_root = TemplateCompositionRoot::new();
        let input = TemplateInput::Export(TemplateExportInput {
            workspace_root: root_dir.join("workspace"),
            manifest_path: root_dir.join("boundary.json"),
            overlay_dir: root_dir.join("overlay"),
            output_dir: output_dir.clone(),
        });

        let outcome = composition_root.handle(input).unwrap();
        assert_eq!(outcome.exit_code, 0, "successful export must exit 0: {outcome:?}");

        // AC-01: `bin/sotp` exists and is byte-identical to the running binary.
        let transplanted = output_dir.join("bin/sotp");
        assert!(transplanted.exists(), "bin/sotp must exist under the output tree");
        let source = std::env::current_exe().unwrap();
        assert_eq!(
            std::fs::read(&source).unwrap(),
            std::fs::read(&transplanted).unwrap(),
            "bin/sotp must be byte-identical to the running binary",
        );

        // AC-01 unix arm: executable permission preserved.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            let mode = std::fs::metadata(&transplanted).unwrap().permissions().mode();
            assert!(mode & 0o100 != 0, "bin/sotp must be executable: mode={mode:o}");
        }
    }
}
