//! `signal check-adr-user` — evaluate ADR→user gate (chain ⓪).

use std::path::PathBuf;

use clap::Args;
use cli_composition::{CliApp, CommandOutcome, CompositionError};

use super::CheckFlags;

/// Arguments for `signal check-adr-user`.
#[derive(Args, Debug)]
pub struct CheckAdrUserArgs {
    /// Project root directory (scans `<root>/knowledge/adr/`).
    #[arg(long, default_value = ".")]
    pub project_root: PathBuf,

    #[command(flatten)]
    pub flags: CheckFlags,
}

/// Execute `signal check-adr-user`.
pub fn run(app: &CliApp, args: CheckAdrUserArgs) -> Result<CommandOutcome, CompositionError> {
    let gate = args.flags.gate_name();
    app.signal_check_adr_user(args.project_root, args.flags.strict, gate, args.flags.workspace_root)
}
