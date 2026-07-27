//! `signal calc-impl-catalog` — compute and persist impl-catalog signals (chain ③).

use clap::Args;
use cli_composition::{CommandOutcome, CompositionError};
use cli_driver::signal::{SignalDriver, SignalInput};

/// Arguments for `signal calc-impl-catalog`.
///
/// Argless command: active track and layer enumeration are resolved from the
/// current git branch and `architecture-rules.json` via the usecase orchestrator.
#[derive(Args, Debug)]
pub struct CalcImplCatalogArgs {}

/// Execute `signal calc-impl-catalog`.
pub fn run(
    driver: &SignalDriver,
    _args: CalcImplCatalogArgs,
) -> Result<CommandOutcome, CompositionError> {
    Ok(driver.handle(SignalInput::CalcImplCatalog))
}
