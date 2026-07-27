//! `signal check-impl-catalog` — evaluate impl↔catalog gate (chain ③).

use clap::Args;
use cli_composition::{CommandOutcome, CompositionError};
use cli_driver::signal::{SignalDriver, SignalInput};

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
pub fn run(
    driver: &SignalDriver,
    args: CheckImplCatalogArgs,
) -> Result<CommandOutcome, CompositionError> {
    let gate = args.flags.gate_name();
    Ok(driver.handle(SignalInput::CheckImplCatalog {
        strict_override: args.flags.strict,
        gate,
        workspace_root: args.flags.workspace_root,
    }))
}
