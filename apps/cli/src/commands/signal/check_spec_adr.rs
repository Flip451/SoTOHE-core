//! `signal check-spec-adr` — evaluate spec→ADR gate (chain ①).

use std::path::PathBuf;

use clap::Args;
use cli_composition::{CommandOutcome, CompositionError};
use cli_driver::signal::{SignalDriver, SignalInput};

use super::CheckFlags;

/// Arguments for `signal check-spec-adr`.
#[derive(Args, Debug)]
pub struct CheckSpecAdrArgs {
    /// Path to `spec.json`. When omitted, defaults to
    /// `track/items/<active-track>/spec.json` under the resolved workspace root.
    #[arg(long)]
    pub spec_json: Option<PathBuf>,

    #[command(flatten)]
    pub flags: CheckFlags,
}

/// Execute `signal check-spec-adr`.
pub fn run(
    driver: &SignalDriver,
    args: CheckSpecAdrArgs,
) -> Result<CommandOutcome, CompositionError> {
    let gate = args.flags.gate_name();
    Ok(driver.handle(SignalInput::CheckSpecAdr {
        spec_json_path: args.spec_json,
        strict_override: args.flags.strict,
        gate,
        workspace_root: args.flags.workspace_root,
    }))
}
