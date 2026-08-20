//! `sotp track contract-map` — render the catalogue-input contract map
//! for a track.
//!
//! Composition root that wires the usecase interactor
//! (`usecase::contract_map_workflow::RenderContractMapInteractor`) to its
//! three secondary-port adapters:
//! * `FsCatalogueLoader` — loads per-layer catalogue documents.
//! * `ContractMapRendererAdapter` — renders the mermaid contract map (T003,
//!   Decision P-1 / P-3). Style config at
//!   `.harness/config/contract-map-style.toml` (fail-closed if absent or
//!   invalid, CN-02 / AC-11).
//! * `FsContractMapWriter` — writes `contract-map.md` to the track dir.

use std::io;
use std::path::PathBuf;
use std::process::ExitCode;

use cli_composition::TrackCompositionRoot;
use cli_driver::adr_baseline::TrackIdInput;
use cli_driver::track_tddd::{
    TrackItemsDirectoryInput, TrackLayersInput, TrackTdddContractMapInput, TrackTdddInput,
    TrackWorkspaceRootInput,
};

use crate::CliError;

/// Render the Contract Map for a single track.
///
/// Thin CLI adapter: delegates all orchestration to the composition root in `cli_composition`.
///
/// # Errors
///
/// Returns `CliError` when the underlying composition fails.
pub fn execute_contract_map(
    items_dir: PathBuf,
    track_id: String,
    workspace_root: PathBuf,
    layers: Option<String>,
) -> Result<ExitCode, CliError> {
    let track_id = track_id
        .parse::<TrackIdInput>()
        .map_err(|error| CliError::Message(format!("invalid track id: {error}")))?;
    let items_dir = TrackItemsDirectoryInput::try_new(items_dir)
        .map_err(|error| CliError::Message(error.to_string()))?;
    let workspace_root =
        TrackWorkspaceRootInput::try_from(workspace_root).map_err(CliError::Message)?;
    let layers = layers
        .map(TrackLayersInput::try_new)
        .transpose()
        .map_err(|error| CliError::Message(error.to_string()))?;
    let outcome = TrackCompositionRoot::new().track_tddd_driver().handle(
        TrackTdddInput::ContractMap(TrackTdddContractMapInput {
            track_id: Some(track_id),
            items_dir,
            workspace_root,
            layers,
        }),
    );
    super::emit_command_outcome(&outcome, &mut io::stdout(), &mut io::stderr())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// Verifies that a malformed track ID is rejected before git discovery.
    #[test]
    fn test_execute_contract_map_with_invalid_track_id_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        std::fs::create_dir_all(&items_dir).unwrap();

        let result = execute_contract_map(items_dir, "../evil".to_owned(), dir.path().into(), None);
        let err = result.expect_err("path traversal track id must be rejected");
        let msg = format!("{err}");
        // Error text is the domain form: "track id '...' must be a lowercase slug".
        // Accept either the domain form or legacy "invalid" prefix (behaviour: rejection).
        assert!(
            msg.contains("must be a lowercase slug")
                || msg.contains("invalid track ID")
                || msg.contains("invalid"),
            "error must reject invalid track id, got: {msg}"
        );
    }

    #[test]
    fn test_contract_map_failure_outcome_emits_stderr_and_nonzero_exit() {
        let outcome = cli_composition::CommandOutcome {
            stdout: None,
            stderr: Some("contract-map render failed: missing style config".to_owned()),
            exit_code: 1,
        };
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let exit = super::super::emit_command_outcome(&outcome, &mut stdout, &mut stderr).unwrap();

        assert_eq!(exit, ExitCode::from(1));
        assert!(stdout.is_empty(), "failure must not create stdout output");
        assert_eq!(
            String::from_utf8(stderr).unwrap(),
            "contract-map render failed: missing style config\n"
        );
    }

    #[test]
    fn test_contract_map_warning_outcome_emits_stdout_and_zero_exit() {
        let outcome = cli_composition::CommandOutcome {
            stdout: Some(
                "[OK] contract-map: wrote track/items/test-track/contract-map.md \
                 (layers=1, entries=2, warnings=[UndefinedRoleStyle { role: ValueObject }])"
                    .to_owned(),
            ),
            stderr: None,
            exit_code: 0,
        };
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let exit = super::super::emit_command_outcome(&outcome, &mut stdout, &mut stderr).unwrap();

        assert_eq!(exit, ExitCode::SUCCESS);
        assert!(stderr.is_empty(), "warning outcome must not create stderr output");
        assert_eq!(
            String::from_utf8(stdout).unwrap(),
            "[OK] contract-map: wrote track/items/test-track/contract-map.md \
             (layers=1, entries=2, warnings=[UndefinedRoleStyle { role: ValueObject }])\n"
        );
    }

    const RULES_JSON: &str = r#"{
      "version": 2,
      "layers": [{
        "crate": "domain",
        "path": "libs/domain",
        "may_depend_on": [],
        "deny_reason": "no reverse dep",
        "tddd": {
          "enabled": true,
          "catalogue_file": "domain-types.json",
          "schema_export": {"method": "rustdoc", "targets": ["domain"]}
        }
      }]
    }"#;

    const EMPTY_CATALOGUE: &str = r#"{
      "schema_version": 5,
      "crate_name": "domain",
      "layer": "domain",
      "types": {},
      "traits": {},
      "functions": {}
    }"#;

    const MINIMAL_STYLE_CONFIG: &str = "[filter]\ninclude_function_roles = []\n";

    fn write_file(path: &std::path::Path, content: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    fn list_relative_files(root: &std::path::Path) -> Vec<String> {
        let mut files = Vec::new();
        let mut pending = vec![root.to_path_buf()];
        while let Some(dir) = pending.pop() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    pending.push(path);
                } else {
                    files.push(path.strip_prefix(root).unwrap().display().to_string());
                }
            }
        }
        files.sort();
        files
    }

    fn contract_map_call_site_outcome(root: &std::path::Path) -> cli_driver::CommandOutcome {
        let track_id = "test-track".parse::<TrackIdInput>().expect("track id is valid");
        let items_dir = TrackItemsDirectoryInput::try_new(root.join("track/items"))
            .expect("items directory is valid");
        let workspace_root =
            TrackWorkspaceRootInput::try_from(root.to_path_buf()).expect("workspace is valid");
        TrackCompositionRoot::new().track_tddd_driver().handle(TrackTdddInput::ContractMap(
            TrackTdddContractMapInput {
                track_id: Some(track_id),
                items_dir,
                workspace_root,
                layers: None,
            },
        ))
    }

    #[test]
    fn test_track_contract_map_call_site_preserves_cli_contract_across_migration() {
        let workspace = tempfile::tempdir().expect("temporary workspace");
        let root = workspace.path();
        write_file(&root.join("architecture-rules.json"), RULES_JSON);
        write_file(&root.join(".harness/config/contract-map-style.toml"), MINIMAL_STYLE_CONFIG);
        write_file(&root.join("track/items/test-track/domain-types.json"), EMPTY_CATALOGUE);
        let before = list_relative_files(root);

        let items_dir = root.join("track/items");
        let track_id = "test-track".to_owned();
        let workspace_root = root.to_path_buf();
        let layers = None::<String>;
        let cli_exit = execute_contract_map(
            items_dir.clone(),
            track_id.clone(),
            workspace_root.clone(),
            layers.clone(),
        )
        .expect("legacy CLI argv must remain accepted");
        assert_eq!(cli_exit, ExitCode::from(0));

        let outcome = contract_map_call_site_outcome(root);
        assert_eq!(items_dir, root.join("track/items"));
        assert_eq!(track_id, "test-track");
        assert_eq!(workspace_root, root.to_path_buf());
        assert_eq!(layers, None);

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(
            outcome.stdout.as_deref(),
            Some(
                "[OK] contract-map: wrote track/items/test-track/contract-map.md (layers=1, entries=0)"
            )
        );
        assert_eq!(outcome.stderr, None);
        let after = list_relative_files(root);
        assert!(after.contains(&"track/items/test-track/contract-map.md".to_owned()));
        assert!(
            std::fs::read_to_string(root.join("track/items/test-track/contract-map.md"))
                .unwrap()
                .contains("flowchart LR")
        );
        assert_eq!(after.len(), before.len() + 1);
    }
}
