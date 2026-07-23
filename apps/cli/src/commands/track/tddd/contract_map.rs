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
    let outcome = TrackCompositionRoot::new()
        .track_contract_map(items_dir, Some(track_id), workspace_root, layers)
        .map_err(|e| CliError::Message(e.to_string()))?;
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
}
