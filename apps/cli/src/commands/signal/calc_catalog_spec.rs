//! `signal calc-catalog-spec` — compute and persist catalog-spec signals (chain ②).

use clap::Args;
use cli_composition::{CommandOutcome, CompositionError, SignalCompositionRoot};

/// Arguments for `signal calc-catalog-spec`.
///
/// Argless command: active track and layer enumeration are resolved from the
/// current git branch and `architecture-rules.json` via the usecase orchestrator.
#[derive(Args, Debug)]
pub struct CalcCatalogSpecArgs {}

/// Execute `signal calc-catalog-spec`.
pub fn run(
    app: &SignalCompositionRoot,
    _args: CalcCatalogSpecArgs,
) -> Result<CommandOutcome, CompositionError> {
    app.signal_calc_catalog_spec()
}
