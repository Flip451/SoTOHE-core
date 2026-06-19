//! `signal calc-impl-catalog` — compute and persist impl-catalog signals (chain ③).

use std::path::PathBuf;

use clap::Args;
use cli_composition::{CliApp, CommandOutcome};

/// Arguments for `signal calc-impl-catalog`.
#[derive(Args, Debug)]
pub struct CalcImplCatalogArgs {
    /// Path to the `<layer>-type-signals.json` signals file.
    #[arg(long)]
    pub signals_path: PathBuf,

    /// SHA-256 hex digest of the current `<layer>-types.json` bytes.
    #[arg(long)]
    pub catalog_hash: String,
}

/// Execute `signal calc-impl-catalog`.
pub fn run(app: &CliApp, args: CalcImplCatalogArgs) -> Result<CommandOutcome, String> {
    app.signal_calc_impl_catalog(args.signals_path, args.catalog_hash)
}
