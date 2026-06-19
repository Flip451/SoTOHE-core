//! `signal check-spec-adr` — evaluate spec→ADR gate (chain ①).

use std::path::PathBuf;

use clap::Args;
use cli_composition::{CliApp, CommandOutcome};

use super::CheckFlags;

/// Arguments for `signal check-spec-adr`.
#[derive(Args, Debug)]
pub struct CheckSpecAdrArgs {
    /// Path to `spec.json`.
    #[arg(long)]
    pub spec_json: PathBuf,

    #[command(flatten)]
    pub flags: CheckFlags,
}

/// Execute `signal check-spec-adr`.
pub fn run(app: &CliApp, args: CheckSpecAdrArgs) -> Result<CommandOutcome, String> {
    let gate = args.flags.gate_name();
    app.signal_check_spec_adr(args.spec_json, args.flags.strict, gate, args.flags.workspace_root)
}
