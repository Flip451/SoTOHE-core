//! `signal calc-impl-catalog` — compute and persist impl-catalog signals (chain ③).

use clap::Args;
use cli_composition::{CliApp, CommandOutcome};

/// Arguments for `signal calc-impl-catalog`.
///
/// Argless command: active track and layer enumeration are resolved from the
/// current git branch and `architecture-rules.json` via the usecase orchestrator.
#[derive(Args, Debug)]
pub struct CalcImplCatalogArgs {}

/// Execute `signal calc-impl-catalog`.
pub fn run(app: &CliApp, _args: CalcImplCatalogArgs) -> Result<CommandOutcome, String> {
    app.signal_calc_impl_catalog()
}
