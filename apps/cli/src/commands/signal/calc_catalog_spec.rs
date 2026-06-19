//! `signal calc-catalog-spec` — compute and persist catalog-spec signals (chain ②).

use std::path::PathBuf;

use clap::Args;
use cli_composition::{CliApp, CommandOutcome};

/// Arguments for `signal calc-catalog-spec`.
#[derive(Args, Debug)]
pub struct CalcCatalogSpecArgs {
    /// Path to the `<layer>-catalogue-spec-signals.json` signals file.
    #[arg(long)]
    pub signals_path: PathBuf,

    /// SHA-256 hex digest of the current `<layer>-types.json` bytes.
    #[arg(long)]
    pub catalog_hash: String,
}

/// Execute `signal calc-catalog-spec`.
pub fn run(app: &CliApp, args: CalcCatalogSpecArgs) -> Result<CommandOutcome, String> {
    app.signal_calc_catalog_spec(args.signals_path, args.catalog_hash)
}
