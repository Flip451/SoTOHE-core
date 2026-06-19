//! `signal check-impl-catalog` — evaluate impl↔catalog gate (chain ③).

use clap::Args;
use cli_composition::{CliApp, CommandOutcome};

use super::CheckFlags;

/// Arguments for `signal check-impl-catalog`.
///
/// Path and hash arguments are removed (T020 / D8): the active track and layer
/// enumeration are resolved internally via the usecase orchestrator.
/// Strictness is still configurable via `--strict` or `--gate commit|merge`.
#[derive(Args, Debug)]
pub struct CheckImplCatalogArgs {
    #[command(flatten)]
    pub flags: CheckFlags,
}

/// Execute `signal check-impl-catalog`.
pub fn run(app: &CliApp, args: CheckImplCatalogArgs) -> Result<CommandOutcome, String> {
    let gate = args.flags.gate_name();
    app.signal_check_impl_catalog(args.flags.strict, gate, args.flags.workspace_root)
}
