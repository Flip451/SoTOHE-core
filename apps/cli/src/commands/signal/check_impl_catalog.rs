//! `signal check-impl-catalog` — evaluate impl↔catalog gate (chain ③).

use std::path::PathBuf;

use clap::Args;
use cli_composition::{CliApp, CommandOutcome};

use super::CheckFlags;

/// Arguments for `signal check-impl-catalog`.
#[derive(Args, Debug)]
pub struct CheckImplCatalogArgs {
    /// Path to the `<layer>-type-signals.json` signals file.
    #[arg(long)]
    pub signals_path: PathBuf,

    /// SHA-256 hex digest of the current `<layer>-types.json` bytes.
    #[arg(long)]
    pub catalog_hash: String,

    #[command(flatten)]
    pub flags: CheckFlags,
}

/// Execute `signal check-impl-catalog`.
pub fn run(app: &CliApp, args: CheckImplCatalogArgs) -> Result<CommandOutcome, String> {
    let gate = args.flags.gate_name();
    app.signal_check_impl_catalog(
        args.signals_path,
        args.catalog_hash,
        args.flags.strict,
        gate,
        args.flags.workspace_root,
    )
}
