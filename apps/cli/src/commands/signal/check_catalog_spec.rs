//! `signal check-catalog-spec` — evaluate catalog→spec gate (chain ②).

use clap::Args;
use cli_composition::{CommandOutcome, CompositionError, SignalCompositionRoot};

use super::CheckFlags;

/// Arguments for `signal check-catalog-spec`.
///
/// Path and hash arguments are removed (T020 / D8): the active track and layer
/// enumeration are resolved internally via the usecase orchestrator.
/// Strictness is still configurable via `--strict` or `--gate commit|merge`.
#[derive(Args, Debug)]
pub struct CheckCatalogSpecArgs {
    #[command(flatten)]
    pub flags: CheckFlags,
}

/// Execute `signal check-catalog-spec`.
pub fn run(
    app: &SignalCompositionRoot,
    args: CheckCatalogSpecArgs,
) -> Result<CommandOutcome, CompositionError> {
    let gate = args.flags.gate_name();
    app.signal_check_catalog_spec(args.flags.strict, gate, args.flags.workspace_root)
}
