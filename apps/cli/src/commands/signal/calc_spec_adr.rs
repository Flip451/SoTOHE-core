//! `signal calc-spec-adr` — compute and persist spec-adr signals (chain ①).

use std::path::PathBuf;

use clap::Args;
use cli_composition::{CliApp, CommandOutcome};

/// Arguments for `signal calc-spec-adr`.
#[derive(Args, Debug)]
pub struct CalcSpecAdrArgs {
    /// Path to `spec.json`.
    #[arg(long)]
    pub spec_json: PathBuf,
}

/// Execute `signal calc-spec-adr`.
pub fn run(app: &CliApp, args: CalcSpecAdrArgs) -> Result<CommandOutcome, String> {
    app.signal_calc_spec_adr(args.spec_json)
}
