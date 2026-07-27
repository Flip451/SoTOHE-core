//! `signal check-adr-user` — evaluate ADR→user gate (chain ⓪).

use std::path::PathBuf;

use clap::Args;
use cli_composition::{CommandOutcome, CompositionError};
use cli_driver::signal::{SignalDriver, SignalInput};

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
pub fn run(
    driver: &SignalDriver,
    args: CheckAdrUserArgs,
) -> Result<CommandOutcome, CompositionError> {
    let gate = args.flags.gate_name();
    Ok(driver.handle(SignalInput::CheckAdrUser {
        project_root: args.project_root,
        strict_override: args.flags.strict,
        gate,
        workspace_root: args.flags.workspace_root,
    }))
}
