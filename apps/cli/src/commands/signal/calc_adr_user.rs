//! `signal calc-adr-user` — compute ADR grounding signals live (chain ⓪).

use std::path::PathBuf;

use clap::Args;
use cli_composition::{CliApp, CommandOutcome};

/// Arguments for `signal calc-adr-user`.
#[derive(Args, Debug)]
pub struct CalcAdrUserArgs {
    /// Project root directory (scans `<root>/knowledge/adr/`).
    #[arg(long, default_value = ".")]
    pub project_root: PathBuf,
}

/// Execute `signal calc-adr-user`.
pub fn run(app: &CliApp, args: CalcAdrUserArgs) -> Result<CommandOutcome, String> {
    app.signal_calc_adr_user(args.project_root)
}
