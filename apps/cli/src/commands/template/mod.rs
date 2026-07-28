//! `sotp template` subcommand — template export and shipping-check surface.
//!
//! Defines the clap arg surface for `export` and `check-convention-shipping`,
//! converts the parsed args into the matching `cli_driver` input DTO, and
//! dispatches through `TemplateCompositionRoot`. The boundary manifest, overlay
//! directory, and output directory are supplied explicitly so the export is
//! fully deterministic (spec IN-01, AC-01). The export path performs no
//! programmatic file rewriting; it is driven purely by the boundary manifest
//! classification. The check names its two trees just as explicitly (spec IN-11,
//! AC-18).

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Subcommand};
use cli_composition::TemplateCompositionRoot;
use cli_driver::template_conventions::ConventionShippingCheckInput;
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

/// Arguments for `sotp template check-convention-shipping` (spec IN-11, AC-18).
///
/// Both roots are required and neither has a default: the check answers a
/// question about two specific trees, so inferring either from the current
/// directory, from a config file, or from the other argument would leave the
/// subject of the answer ambiguous. It would also make the check's most
/// dangerous failure mode silent — an overlay path resolved relative to the
/// exported tree would compare that tree against itself and pass vacuously.
///
/// `overlay_dir` keeps the name it has on [`TemplateExportArgs`] because it
/// denotes the same directory; diverging would make the two subcommands look
/// like they take different things.
#[derive(Debug, Args)]
pub struct TemplateConventionShippingArgs {
    /// Root of the exported template tree whose shipped conventions are checked.
    #[arg(long)]
    pub exported_root: PathBuf,

    /// Directory holding overlay (template-replacement) files.
    #[arg(long)]
    pub overlay_dir: PathBuf,
}

// ---------------------------------------------------------------------------
// Subcommand
// ---------------------------------------------------------------------------

/// Subcommands for `sotp template`.
#[derive(Debug, Subcommand)]
pub enum TemplateCommand {
    /// Export the workspace as a reusable template tree.
    Export(TemplateExportArgs),
    /// Check that an exported tree ships no convention the overlay does not supply.
    CheckConventionShipping(TemplateConventionShippingArgs),
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
        // The read-only check leaves through its own driver. Folding it into
        // `TemplateInput` would route it through the handler that writes an
        // export tree, and the check is a consumer of that tree rather than a
        // step of producing one.
        TemplateCommand::CheckConventionShipping(args) => {
            let TemplateConventionShippingArgs { exported_root, overlay_dir } = args;
            let driver = TemplateCompositionRoot::new().convention_shipping_check_driver();
            let input = ConventionShippingCheckInput { exported_root, overlay_dir };
            return crate::commands::driver_outcome_to_exit(driver.handle(input));
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

    use super::{TemplateCommand, TemplateConventionShippingArgs, TemplateExportArgs};

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
        }) = cli.cmd
        else {
            panic!("`export` parses into the export variant");
        };
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

    /// `sotp template check-convention-shipping --exported-root … --overlay-dir …`
    /// parses into the check variant with each path on the matching field (spec
    /// IN-11, AC-18).
    #[test]
    fn test_check_convention_shipping_parses_both_tree_roots() {
        let cli = TemplateCli::try_parse_from([
            "sotp",
            "check-convention-shipping",
            "--exported-root",
            "/tmp/template-export-smoke",
            "--overlay-dir",
            "/srv/sotohe/overlay",
        ])
        .unwrap();

        let TemplateCommand::CheckConventionShipping(TemplateConventionShippingArgs {
            exported_root,
            overlay_dir,
        }) = cli.cmd
        else {
            panic!("`check-convention-shipping` parses into the check variant");
        };
        // The two roots are dissimilar so a transposed mapping reads as a
        // mismatch here rather than as an equal pair: the exported tree and the
        // supply it is measured against are different trees, and the check's
        // answer is about the first measured against the second.
        assert_eq!(exported_root, PathBuf::from("/tmp/template-export-smoke"));
        assert_eq!(overlay_dir, PathBuf::from("/srv/sotohe/overlay"));
    }

    /// Both roots are given explicitly: neither is optional, defaulted, or
    /// inferred from the other (spec IN-11, AC-18).
    #[test]
    fn test_check_convention_shipping_infers_neither_tree_root() {
        // Each row supplies one root and omits the other. A default, or a
        // location derived from the surrounding state or from the sibling
        // argument, would let one of these parse — and an overlay path inferred
        // relative to the exported tree would make the check compare that tree
        // against itself and pass vacuously.
        for partial in [
            vec!["sotp", "check-convention-shipping", "--exported-root", "/out"],
            vec!["sotp", "check-convention-shipping", "--overlay-dir", "/ws/overlay"],
            vec!["sotp", "check-convention-shipping"],
        ] {
            let result = TemplateCli::try_parse_from(partial.clone());
            assert!(
                result.is_err(),
                "the check must name both trees explicitly; {partial:?} leaves one unnamed"
            );
        }
    }

    /// The check carries its own argument set: the export's arguments are not
    /// accepted here, so neither subcommand can be invoked with the other's
    /// inputs (spec IN-11, AC-18).
    #[test]
    fn test_check_convention_shipping_does_not_accept_the_exports_arguments() {
        let result = TemplateCli::try_parse_from([
            "sotp",
            "check-convention-shipping",
            "--exported-root",
            "/out",
            "--overlay-dir",
            "/ws/overlay",
            "--workspace-root",
            "/ws",
        ]);
        assert!(
            result.is_err(),
            "the check names an already-produced tree plus its overlay, not the inputs an export \
             consumes"
        );
    }
}
