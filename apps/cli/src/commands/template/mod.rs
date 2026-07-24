//! `sotp template` subcommand — template export surface.
//!
//! Defines the clap arg surface for `export`, converts the parsed args into the
//! `cli_driver::template_export` input DTO, and dispatches through
//! `TemplateCompositionRoot`. The boundary manifest, overlay directory, and
//! output directory are supplied explicitly so the export is fully deterministic
//! (spec IN-01, AC-01). The export path performs no programmatic file rewriting;
//! it is driven purely by the boundary manifest classification.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Subcommand};
use cli_composition::TemplateCompositionRoot;
use cli_driver::template_export::{TemplateExportInput, TemplateInput};

// ---------------------------------------------------------------------------
// Args
// ---------------------------------------------------------------------------

/// Arguments for `sotp template export`.
#[derive(Debug, Args)]
pub struct TemplateExportArgs {
    /// Root of the workspace being exported as a template.
    #[arg(long)]
    pub workspace_root: PathBuf,

    /// Path to the boundary manifest that classifies every workspace path.
    #[arg(long)]
    pub manifest_path: PathBuf,

    /// Directory holding overlay (template-replacement) files.
    #[arg(long)]
    pub overlay_dir: PathBuf,

    /// Directory the exported template tree is written to.
    #[arg(long)]
    pub output_dir: PathBuf,
}

// ---------------------------------------------------------------------------
// Subcommand
// ---------------------------------------------------------------------------

/// Subcommands for `sotp template`.
#[derive(Debug, Subcommand)]
pub enum TemplateCommand {
    /// Export the workspace as a reusable template tree.
    Export(TemplateExportArgs),
}

// ---------------------------------------------------------------------------
// Dispatch
// ---------------------------------------------------------------------------

/// Execute `sotp template <subcommand>`.
pub fn execute(cmd: TemplateCommand) -> ExitCode {
    let input = match cmd {
        TemplateCommand::Export(args) => {
            let TemplateExportArgs { workspace_root, manifest_path, overlay_dir, output_dir } =
                args;
            TemplateInput::Export(TemplateExportInput {
                workspace_root,
                manifest_path,
                overlay_dir,
                output_dir,
            })
        }
    };
    dispatch(input)
}

/// Dispatch an assembled input through the composition root and map the outcome.
pub fn dispatch(input: TemplateInput) -> ExitCode {
    let driver = TemplateCompositionRoot::new().template_driver();
    crate::commands::driver_outcome_to_exit(driver.handle(input))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use std::path::PathBuf;

    use clap::Parser as _;

    use super::{TemplateCommand, TemplateExportArgs};

    /// Minimal parser harness so the `template` subcommand can be parsed in
    /// isolation from the full `Cli` surface.
    #[derive(clap::Parser)]
    struct TemplateCli {
        #[command(subcommand)]
        cmd: TemplateCommand,
    }

    /// `sotp template export --workspace-root … --manifest-path … --overlay-dir …
    /// --output-dir …` must parse into `TemplateCommand::Export` with every path
    /// mapped to the matching field.
    #[test]
    fn test_export_parses_all_path_args() {
        let cli = TemplateCli::try_parse_from([
            "sotp",
            "export",
            "--workspace-root",
            "/ws",
            "--manifest-path",
            "/ws/boundary.json",
            "--overlay-dir",
            "/ws/overlay",
            "--output-dir",
            "/out",
        ])
        .unwrap();

        let TemplateCommand::Export(TemplateExportArgs {
            workspace_root,
            manifest_path,
            overlay_dir,
            output_dir,
        }) = cli.cmd;
        assert_eq!(workspace_root, PathBuf::from("/ws"));
        assert_eq!(manifest_path, PathBuf::from("/ws/boundary.json"));
        assert_eq!(overlay_dir, PathBuf::from("/ws/overlay"));
        assert_eq!(output_dir, PathBuf::from("/out"));
    }

    /// Each of the four path args is required; omitting one is rejected by clap.
    #[test]
    fn test_export_missing_required_arg_is_rejected() {
        let result = TemplateCli::try_parse_from([
            "sotp",
            "export",
            "--workspace-root",
            "/ws",
            "--manifest-path",
            "/ws/boundary.json",
            "--overlay-dir",
            "/ws/overlay",
            // --output-dir intentionally omitted
        ]);
        assert!(result.is_err(), "missing --output-dir must be rejected by clap");
    }

    /// An unrecognized `sotp template` subcommand must be rejected by clap.
    #[test]
    fn test_unknown_subcommand_is_rejected() {
        let result = TemplateCli::try_parse_from(["sotp", "unknown-subcmd"]);
        assert!(result.is_err(), "unrecognized template subcommand must be rejected by clap");
    }
}
