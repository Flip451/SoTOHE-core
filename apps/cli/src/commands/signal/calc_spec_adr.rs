//! `signal calc-spec-adr` — compute and persist spec-adr signals (chain ①).

use std::path::PathBuf;

use clap::Args;
use cli_composition::{CommandOutcome, CompositionError, SignalCompositionRoot};

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
    app: &SignalCompositionRoot,
    args: CalcSpecAdrArgs,
) -> Result<CommandOutcome, CompositionError> {
    app.signal_calc_spec_adr(args.spec_json, args.workspace_root)
}
