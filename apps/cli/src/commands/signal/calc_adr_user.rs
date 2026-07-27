//! `signal calc-adr-user` — compute ADR grounding signals live (chain ⓪).

use std::path::PathBuf;

use clap::Args;
use cli_composition::{CommandOutcome, CompositionError};
use cli_driver::signal::{SignalDriver, SignalInput};

/// Arguments for `signal calc-adr-user`.
#[derive(Args, Debug)]
pub struct CalcAdrUserArgs {
    /// Project root directory (scans `<root>/knowledge/adr/`).
    #[arg(long, default_value = ".")]
    pub project_root: PathBuf,
}

/// Execute `signal calc-adr-user`.
pub fn run(
    driver: &SignalDriver,
    args: CalcAdrUserArgs,
) -> Result<CommandOutcome, CompositionError> {
    Ok(driver.handle(SignalInput::CalcAdrUser { project_root: args.project_root }))
}
