//! `signal calc-spec-adr` — compute and persist spec-adr signals (chain ①).

use std::path::PathBuf;

use clap::Args;
use cli_composition::{CommandOutcome, CompositionError};
use cli_driver::signal::{SignalDriver, SignalInput};

/// Arguments for `signal calc-spec-adr`.
#[derive(Args, Debug)]
pub struct CalcSpecAdrArgs {
    /// Path to `spec.json`. When omitted, defaults to
    /// `track/items/<active-track>/spec.json` under the resolved workspace root.
    #[arg(long)]
    pub spec_json: Option<PathBuf>,

    /// Path to workspace root. When omitted, defaults to the git-discovered
    /// repository root.
    #[arg(long)]
    pub workspace_root: Option<PathBuf>,
}

/// Execute `signal calc-spec-adr`.
pub fn run(
    driver: &SignalDriver,
    args: CalcSpecAdrArgs,
) -> Result<CommandOutcome, CompositionError> {
    Ok(driver.handle(SignalInput::CalcSpecAdr {
        spec_json_path: args.spec_json,
        workspace_root: args.workspace_root,
    }))
}
