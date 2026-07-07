//! `template` command family — primary adapter driver.
//!
//! `TemplateDriver` holds an injected [`usecase::template_export::TemplateExportService`]
//! and exposes `handle(input) -> CommandOutcome`. It converts the typed CLI input
//! into a [`TemplateExportCommand`], invokes the service, and renders the report or
//! error into a `CommandOutcome` (spec IN-01, AC-01).

use std::path::PathBuf;
use std::sync::Arc;

use usecase::template_export::{
    TemplateExportCommand, TemplateExportReport, TemplateExportService,
};

use crate::render::CommandOutcome;

// ---------------------------------------------------------------------------
// Input types
// ---------------------------------------------------------------------------

/// `sotp template export` driver input (IN-01, AC-01).
#[derive(Debug, Clone)]
pub struct TemplateExportInput {
    /// Root of the workspace being exported as a template.
    pub workspace_root: PathBuf,
    /// Path to the boundary manifest that classifies every workspace path.
    pub manifest_path: PathBuf,
    /// Directory holding overlay (template-replacement) files.
    pub overlay_dir: PathBuf,
    /// Directory the exported template is written to.
    pub output_dir: PathBuf,
}

/// `sotp template` driver input enum (IN-01, AC-01).
#[derive(Debug, Clone)]
pub enum TemplateInput {
    /// Export the workspace as a reusable template.
    Export(TemplateExportInput),
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// `TemplateExportService` primary adapter (IN-01, AC-01).
///
/// Holds an injected [`TemplateExportService`]; exposes `handle(input) -> CommandOutcome`.
pub struct TemplateDriver {
    service: Arc<dyn TemplateExportService>,
}

impl TemplateDriver {
    /// Constructor.
    pub fn new(service: Arc<dyn TemplateExportService>) -> Self {
        Self { service }
    }

    /// `TemplateInput` handler (IN-01, AC-01).
    pub fn handle(&self, input: TemplateInput) -> CommandOutcome {
        match input {
            TemplateInput::Export(export_input) => self.template_export(export_input),
        }
    }

    // -----------------------------------------------------------------------
    // Private helpers — translate input fields → service calls
    // -----------------------------------------------------------------------

    fn template_export(&self, input: TemplateExportInput) -> CommandOutcome {
        let command = TemplateExportCommand {
            workspace_root: input.workspace_root,
            manifest_path: input.manifest_path,
            overlay_dir: input.overlay_dir,
            output_dir: input.output_dir,
        };
        match self.service.export(command) {
            Ok(report) => CommandOutcome::success(Some(render_export_report(&report))),
            Err(e) => CommandOutcome::failure(Some(e.to_string())),
        }
    }
}

// ---------------------------------------------------------------------------
// Render helper
// ---------------------------------------------------------------------------

/// Render a successful [`TemplateExportReport`] into human-readable stdout text.
fn render_export_report(report: &TemplateExportReport) -> String {
    format!(
        "template export: {} included, {} excluded, {} overlay-applied → {}",
        report.included_count,
        report.excluded_count,
        report.overlay_applied_count,
        report.output_dir.display(),
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use usecase::template_export::TemplateExportError;

    use super::*;

    /// Hand-rolled `TemplateExportService` double (cli_driver carries no mock deps).
    struct FakeService {
        result: fn() -> Result<TemplateExportReport, TemplateExportError>,
    }

    impl TemplateExportService for FakeService {
        fn export(
            &self,
            _command: TemplateExportCommand,
        ) -> Result<TemplateExportReport, TemplateExportError> {
            (self.result)()
        }
    }

    fn driver(result: fn() -> Result<TemplateExportReport, TemplateExportError>) -> TemplateDriver {
        TemplateDriver::new(Arc::new(FakeService { result }))
    }

    fn export_input() -> TemplateExportInput {
        TemplateExportInput {
            workspace_root: PathBuf::from("/ws"),
            manifest_path: PathBuf::from("/ws/boundary.json"),
            overlay_dir: PathBuf::from("/ws/overlay"),
            output_dir: PathBuf::from("/out"),
        }
    }

    #[test]
    fn test_export_success_maps_to_exit_0_with_counts() {
        let driver = driver(|| {
            Ok(TemplateExportReport {
                included_count: 3,
                excluded_count: 1,
                overlay_applied_count: 2,
                output_dir: PathBuf::from("/out"),
            })
        });

        let outcome = driver.handle(TemplateInput::Export(export_input()));
        let stdout = outcome.stdout.as_deref().unwrap_or("");

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stderr, None);
        assert!(stdout.contains("3 included"), "stdout must include the included count");
        assert!(stdout.contains("1 excluded"), "stdout must include the excluded count");
        assert!(stdout.contains("2 overlay-applied"), "stdout must include the overlay count");
        assert!(stdout.contains("/out"), "stdout must include the output directory");
    }

    #[test]
    fn test_export_error_maps_to_exit_1_with_message() {
        let driver = driver(|| {
            Err(TemplateExportError::from(
                usecase::template_export::TemplateExportPortError::OutputDirExists {
                    path: PathBuf::from("/out"),
                },
            ))
        });

        let outcome = driver.handle(TemplateInput::Export(export_input()));

        assert_eq!(outcome.exit_code, 1);
        assert_eq!(outcome.stdout, None);
        assert!(
            outcome
                .stderr
                .as_deref()
                .is_some_and(|msg| msg.contains("output directory already exists")),
            "stderr must surface the underlying export error message"
        );
    }
}
