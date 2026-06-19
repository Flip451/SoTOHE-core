//! `signal check-catalog-spec` — evaluate catalog→spec gate (chain ②).

use std::path::PathBuf;

use clap::Args;
use cli_composition::{CliApp, CommandOutcome};

use super::CheckFlags;

/// Arguments for `signal check-catalog-spec`.
#[derive(Args, Debug)]
pub struct CheckCatalogSpecArgs {
    /// Path to the `<layer>-catalogue-spec-signals.json` signals file.
    #[arg(long)]
    pub signals_path: PathBuf,

    /// SHA-256 hex digest of the current `<layer>-types.json` bytes.
    #[arg(long)]
    pub catalog_hash: String,

    #[command(flatten)]
    pub flags: CheckFlags,
}

/// Execute `signal check-catalog-spec`.
pub fn run(app: &CliApp, args: CheckCatalogSpecArgs) -> Result<CommandOutcome, String> {
    let gate = args.flags.gate_name();
    app.signal_check_catalog_spec(
        args.signals_path,
        args.catalog_hash,
        args.flags.strict,
        gate,
        args.flags.workspace_root,
    )
}
