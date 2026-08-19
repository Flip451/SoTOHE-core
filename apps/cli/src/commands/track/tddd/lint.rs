//! `sotp track lint` subcommand — runs catalogue lint rules against a layer catalogue.
//!
//! Thin CLI adapter: delegates all orchestration to the composition root in `cli_composition`.

use std::path::PathBuf;
use std::process::ExitCode;

use cli_composition::TrackCompositionRoot;
use cli_driver::adr_baseline::TrackIdInput;
use cli_driver::track_tddd::{
    TrackLayerInput, TrackLintRulesFileInput, TrackTdddLintInput, TrackWorkspaceRootInput,
};

use crate::CliError;

/// Execute the `sotp track lint` subcommand.
///
/// # Errors
///
/// Returns `CliError::Message` when the underlying `CliApp` composition fails.
pub fn execute_lint(
    workspace_root: PathBuf,
    track_id: String,
    layer_id: String,
    rules_file: Option<PathBuf>,
) -> Result<ExitCode, CliError> {
    let track_id = track_id
        .parse::<TrackIdInput>()
        .map_err(|error| CliError::Message(format!("invalid track id: {error}")))?;
    let workspace_root =
        TrackWorkspaceRootInput::try_from(workspace_root).map_err(CliError::Message)?;
    let layer =
        TrackLayerInput::try_new(layer_id).map_err(|error| CliError::Message(error.to_string()))?;
    let rules_file = rules_file
        .map(TrackLintRulesFileInput::try_new)
        .transpose()
        .map_err(|error| CliError::Message(error.to_string()))?;
    let outcome = TrackCompositionRoot::new().track_tddd_driver().handle_lint(TrackTdddLintInput {
        track_id: Some(track_id),
        workspace_root,
        layer,
        rules_file,
    });
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    crate::commands::track::tddd::emit_driver_outcome!(outcome, &mut stdout, &mut stderr)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing, clippy::expect_used)]
mod tests {
    use super::*;
    use cli_driver::render::CommandOutcome;

    const RULES_JSON: &str = r#"{
  "version": 2,
  "layers": [
    {
      "crate": "domain",
      "path": "libs/domain",
      "may_depend_on": [],
      "deny_reason": "no reverse dep",
      "tddd": {
        "enabled": true,
        "catalogue_file": "domain-types.json",
        "schema_export": {"method": "rustdoc", "targets": ["domain"]}
      }
    }
  ]
}"#;

    const LINT_CONFIG_WITH_INVARIANT_RULE: &str = r#"{
  "schema_version": 1,
  "rules": [
    {
      "target_roles": ["ValueObject"],
      "kind": { "FieldNonEmpty": { "target_field": "invariants" } }
    }
  ]
}"#;

    const CATALOGUE_WITH_INVARIANT: &str = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "MyValueObject": {
      "action": "add",
      "role": {
        "ValueObject": {
          "invariants": [
            { "name": "is_valid", "predicate": { "SelfMethod": "is_valid" } }
          ]
        }
      },
      "kind": {"kind": "struct", "shape": {"kind": "plain"}},
      "methods": [
        {"name": "is_valid", "receiver": "&self", "params": [], "returns": "bool"}
      ],
      "module_path": "",
      "spec_refs": [],
      "informal_grounds": []
    }
  },
  "traits": {},
  "functions": {}
}"#;

    const CATALOGUE_NO_INVARIANTS: &str = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "BareValueObject": {
      "action": "add",
      "role": { "ValueObject": {} },
      "kind": {"kind": "struct", "shape": {"kind": "plain"}},
      "methods": [],
      "module_path": "",
      "spec_refs": [],
      "informal_grounds": []
    }
  },
  "traits": {},
  "functions": {}
}"#;

    fn write_file(path: &std::path::Path, content: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create fixture parent");
        }
        std::fs::write(path, content).expect("write fixture");
    }

    fn list_relative_files(root: &std::path::Path) -> Vec<String> {
        let mut files = Vec::new();
        let mut pending = vec![root.to_path_buf()];
        while let Some(dir) = pending.pop() {
            for entry in std::fs::read_dir(&dir).expect("read fixture dir") {
                let entry = entry.expect("fixture dir entry");
                let path = entry.path();
                if path.is_dir() {
                    pending.push(path);
                } else {
                    files.push(
                        path.strip_prefix(root)
                            .expect("relative fixture path")
                            .display()
                            .to_string(),
                    );
                }
            }
        }
        files.sort();
        files
    }

    fn lint_call_site_outcome(
        workspace: &std::path::Path,
        track_id: &str,
        layer: &str,
        rules_file: Option<PathBuf>,
    ) -> CommandOutcome {
        let track_id = track_id.parse::<TrackIdInput>().expect("track id is valid");
        let workspace_root =
            TrackWorkspaceRootInput::try_from(workspace.to_path_buf()).expect("workspace is valid");
        let layer = TrackLayerInput::try_new(layer.to_owned()).expect("layer is valid");
        let rules_file = rules_file
            .map(TrackLintRulesFileInput::try_new)
            .transpose()
            .expect("rules file is valid");
        TrackCompositionRoot::new().track_tddd_driver().handle_lint(TrackTdddLintInput {
            track_id: Some(track_id),
            workspace_root,
            layer,
            rules_file,
        })
    }

    #[test]
    fn test_execute_lint_rejects_invalid_track_id() {
        let dir = tempfile::tempdir().unwrap();
        // Write minimal architecture-rules.json so the loader can start up.
        let rules = r#"{"layers":[],"canonical_modules":[]}"#;
        std::fs::write(dir.path().join("architecture-rules.json"), rules).unwrap();

        let result =
            execute_lint(dir.path().to_path_buf(), "../evil".to_owned(), "domain".to_owned(), None);
        assert!(result.is_err(), "path traversal track id must be rejected");
    }

    #[test]
    fn test_track_lint_call_site_preserves_cli_contract_across_migration() {
        let workspace = tempfile::tempdir().expect("temp workspace");
        let root = workspace.path();
        write_file(&root.join("architecture-rules.json"), RULES_JSON);
        write_file(
            &root.join(".harness/catalogue-lint/config.json"),
            LINT_CONFIG_WITH_INVARIANT_RULE,
        );
        write_file(
            &root.join("track/items/test-track/domain-types.json"),
            CATALOGUE_WITH_INVARIANT,
        );

        let before = list_relative_files(root);
        let outcome = lint_call_site_outcome(root, "test-track", "domain", None);
        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout, None, "stdout must stay empty on zero violations");
        assert!(
            outcome.stderr.as_deref().is_some_and(|stderr| stderr.contains("Found 0 violation(s)")),
            "stderr must keep the zero-violation summary: {:?}",
            outcome.stderr
        );
        assert_eq!(list_relative_files(root), before, "track lint must not persist extra files");

        write_file(&root.join("track/items/test-track/domain-types.json"), CATALOGUE_NO_INVARIANTS);
        let outcome = lint_call_site_outcome(root, "test-track", "domain", None);
        assert_eq!(outcome.exit_code, 1);
        assert!(
            outcome.stdout.as_deref().is_some_and(|stdout| {
                stdout.contains("FieldNonEmpty") && stdout.contains("BareValueObject")
            }),
            "stdout must keep the violation detail: {:?}",
            outcome.stdout
        );
        assert!(
            outcome.stderr.as_deref().is_some_and(|stderr| stderr.contains("Found 1 violation(s)")),
            "stderr must keep the violation summary: {:?}",
            outcome.stderr
        );
        assert_eq!(list_relative_files(root), before, "track lint must not persist extra files");
    }
}
