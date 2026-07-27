//! `signal calc-catalog-spec` — compute and persist catalog-spec signals (chain ②).

use clap::Args;
use cli_composition::{CommandOutcome, CompositionError};
use cli_driver::signal::{SignalDriver, SignalInput};

/// Arguments for `signal calc-catalog-spec`.
///
/// Argless command: active track and layer enumeration are resolved from the
/// current git branch and `architecture-rules.json` via the usecase orchestrator.
#[derive(Args, Debug)]
pub struct CalcCatalogSpecArgs {}

/// Execute `signal calc-catalog-spec`.
pub fn run(
    driver: &SignalDriver,
    _args: CalcCatalogSpecArgs,
) -> Result<CommandOutcome, CompositionError> {
    Ok(driver.handle(SignalInput::CalcCatalogSpec))
}
