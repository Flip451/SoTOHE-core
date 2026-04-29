//! `sotp track lint` subcommand — runs catalogue lint rules against a layer
//! catalogue.
//!
//! ADR `tddd-struct-kind-uniformization-and-catalogue-linter` §S3 / IN-05 /
//! AC-05. Composition root: wires `FsCatalogueLoader` (existing) +
//! `InMemoryCatalogueLinter` (T005) + `RunCatalogueLintInteractor` (T006) and
//! runs a hardcoded demo rule set.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use domain::tddd::catalogue_linter::{
    CatalogueLinter, CatalogueLinterRule, CatalogueLinterRuleKind,
};
use domain::tddd::catalogue_ports::CatalogueLoader;
use infrastructure::tddd::contract_map_adapter::FsCatalogueLoader;
use infrastructure::tddd::in_memory_catalogue_linter::InMemoryCatalogueLinter;
use usecase::catalogue_lint_workflow::{
    RunCatalogueLint, RunCatalogueLintCommand, RunCatalogueLintInteractor,
};

use crate::CliError;

/// Execute the `sotp track lint` subcommand.
///
/// Wires `FsCatalogueLoader` + `InMemoryCatalogueLinter` +
/// `RunCatalogueLintInteractor` at the composition root and runs a hardcoded
/// demo rule set against the specified layer catalogue.
///
/// # Errors
///
/// Returns `CliError::Message` when rule construction fails or the interactor
/// returns an error (invalid track / layer / catalogue load failure).
pub fn execute_lint(
    workspace_root: PathBuf,
    track_id: String,
    layer_id: String,
) -> Result<ExitCode, CliError> {
    // Build the hardcoded demo rule set:
    //   1) FieldEmpty on value_object / target_field=expected_methods
    //   2) KindLayerConstraint on domain_service / permitted_layers=[domain, usecase]
    let rule_field_empty = CatalogueLinterRule::try_new(
        CatalogueLinterRuleKind::FieldEmpty,
        "value_object",
        Some("expected_methods".to_owned()),
        vec![],
    )
    .map_err(|e| CliError::Message(format!("failed to construct FieldEmpty rule: {e}")))?;

    let rule_kind_layer = CatalogueLinterRule::try_new(
        CatalogueLinterRuleKind::KindLayerConstraint,
        "domain_service",
        None,
        vec!["domain".to_owned(), "usecase".to_owned()],
    )
    .map_err(|e| CliError::Message(format!("failed to construct KindLayerConstraint rule: {e}")))?;

    let rules = vec![rule_field_empty, rule_kind_layer];

    // Compose secondary ports and interactor.
    let items_dir = workspace_root.join("track/items");
    let rules_path = workspace_root.join("architecture-rules.json");
    let loader: Arc<dyn CatalogueLoader> =
        Arc::new(FsCatalogueLoader::new(items_dir, rules_path, workspace_root.clone()));
    let linter: Arc<dyn CatalogueLinter> = Arc::new(InMemoryCatalogueLinter::new());
    let interactor = RunCatalogueLintInteractor::new(loader, linter);

    // Dispatch through the primary port.
    let runner: &dyn RunCatalogueLint = &interactor;
    let violations = runner
        .execute(RunCatalogueLintCommand { track_id, layer_id, rules })
        .map_err(|e| CliError::Message(format!("catalogue lint failed: {e}")))?;

    // Print per-violation lines.
    for v in &violations {
        println!("{:?} on {}: {}", v.rule_kind(), v.entry_name(), v.message());
    }

    // Print summary to stderr.
    let count = violations.len();
    eprintln!("Found {count} violation(s)");

    if count > 0 { Ok(ExitCode::FAILURE) } else { Ok(ExitCode::SUCCESS) }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_lint_rejects_invalid_track_id() {
        let dir = tempfile::tempdir().unwrap();
        // Write minimal architecture-rules.json so the loader can start up.
        let rules = r#"{"layers":[],"canonical_modules":[]}"#;
        std::fs::write(dir.path().join("architecture-rules.json"), rules).unwrap();

        let result =
            execute_lint(dir.path().to_path_buf(), "../evil".to_owned(), "domain".to_owned());
        assert!(result.is_err(), "path traversal track id must be rejected");
    }
}
