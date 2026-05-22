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

use std::path::PathBuf;
use std::process::ExitCode;

use infrastructure::git_cli::{GitRepository, SystemGitRepo};
use infrastructure::tddd::contract_map_adapter::{FsCatalogueLoader, FsContractMapWriter};
use infrastructure::tddd::contract_map_renderer_adapter::ContractMapRendererAdapter;
use usecase::contract_map_workflow::{
    RenderContractMap, RenderContractMapCommand, RenderContractMapInteractor,
};
use usecase::{LayerId, TrackId};

use crate::CliError;

/// Render the Contract Map for a single track.
///
/// # Errors
///
/// Returns `CliError` when the track id is invalid, the current branch does
/// not match `track/<track_id>` (CN-07 guard), `--layers` cannot be parsed,
/// or the interactor fails (loader / renderer / writer / empty catalogue /
/// unknown layer).
pub fn execute_contract_map(
    items_dir: PathBuf,
    track_id: String,
    workspace_root: PathBuf,
    layers: Option<String>,
) -> Result<ExitCode, CliError> {
    // Validate track_id into domain type at the CLI boundary (CN-12).
    // Runs before git discovery so a malformed track_id is always caught
    // here and never masked by "cannot discover git repo" or branch-mismatch
    // errors.
    let typed_track_id = TrackId::try_new(track_id.clone())
        .map_err(|e| CliError::Message(format!("invalid track ID '{track_id}': {e}")))?;

    // Parse and validate layer filter strings into LayerId at the CLI boundary (CN-12).
    let layer_filter_parsed: Option<Vec<LayerId>> =
        layers.as_deref().map(parse_layer_filter_ids).transpose()?;

    // CN-07 active-track guard: resolve the current git branch and verify it
    // matches `track/<track_id>`, rooted at workspace_root so
    // `--workspace-root` is always honoured.
    let branch = SystemGitRepo::discover_from(&workspace_root)
        .map_err(|e| CliError::Message(format!("cannot discover git repo: {e}")))?
        .current_branch()
        .map_err(|e| CliError::Message(format!("cannot read current branch: {e}")))?
        .unwrap_or_default();
    let suffix = branch.strip_prefix("track/").ok_or_else(|| {
        CliError::Message(format!(
            "contract-map rejected: branch '{branch}' is not an active track branch (CN-07)"
        ))
    })?;
    if suffix != track_id.as_str() {
        return Err(CliError::Message(format!(
            "contract-map rejected: branch '{branch}' does not match \
             track_id '{track_id}' (expected 'track/{track_id}')"
        )));
    }

    let rules_path = workspace_root.join("architecture-rules.json");
    let loader = FsCatalogueLoader::new(items_dir.clone(), rules_path, workspace_root.clone());
    let writer = FsContractMapWriter::new(items_dir.clone(), workspace_root.clone());

    // Inject ContractMapRendererAdapter (T003, Decision P-1/P-3).
    // Style config at .harness/config/contract-map-style.toml (fail-closed: absent/invalid
    // config causes the interactor to return RenderContractMapError::RendererFailed, CN-02).
    let style_config_path = workspace_root.join(".harness/config/contract-map-style.toml");
    let renderer = ContractMapRendererAdapter::new(style_config_path);

    let interactor = RenderContractMapInteractor::new(loader, renderer, writer);

    // Dispatch through the primary port — CLI does not depend on the
    // concrete `RenderContractMapInteractor` type.
    let renderer_ref: &dyn RenderContractMap = &interactor;
    let cmd =
        RenderContractMapCommand { track_id: typed_track_id, layer_filter: layer_filter_parsed };
    let out = renderer_ref
        .execute(&cmd)
        .map_err(|e| CliError::Message(format!("contract-map render failed: {e}")))?;

    println!(
        "[OK] contract-map: wrote track/items/{track_id}/contract-map.md \
         (layers={}, entries={})",
        out.rendered_layer_count, out.total_entry_count,
    );
    Ok(ExitCode::SUCCESS)
}

/// Parses a `--layers` CSV value into validated [`LayerId`] values (CN-12).
/// Validation that the layer is enabled happens in the interactor.
///
/// # Errors
///
/// Returns `CliError` if any token is not a valid `LayerId`.
fn parse_layer_filter_ids(raw: &str) -> Result<Vec<LayerId>, CliError> {
    let mut layers = Vec::new();
    for token in raw.split(',') {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            continue;
        }
        let id = LayerId::try_new(trimmed)
            .map_err(|e| CliError::Message(format!("invalid layer id '{trimmed}': {e}")))?;
        layers.push(id);
    }
    Ok(layers)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing, clippy::expect_used)]
mod tests {
    use super::*;

    /// Returns the current git branch's track-id suffix (the part after `track/`)
    /// if the working directory is on a `track/<id>` branch, or `None` otherwise
    /// (e.g. detached HEAD, `main`, non-track branches).
    ///
    /// Tests that require the branch guard to *pass* use this helper to derive
    /// the track_id at runtime, making them independent of which specific branch
    /// name is checked out when the test suite is run.
    fn current_track_id_suffix() -> Option<String> {
        let repo = SystemGitRepo::discover().ok()?;
        let branch = repo.current_branch().ok()??;
        branch.strip_prefix("track/").map(|s| s.to_owned())
    }

    #[test]
    fn test_parse_layer_filter_ids_single_value_succeeds() {
        let layers = parse_layer_filter_ids("domain").unwrap();
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].as_ref(), "domain");
    }

    #[test]
    fn test_parse_layer_filter_ids_multiple_values_preserves_order() {
        let layers = parse_layer_filter_ids("infrastructure,usecase,domain").unwrap();
        assert_eq!(layers.len(), 3);
        assert_eq!(layers[0].as_ref(), "infrastructure");
        assert_eq!(layers[1].as_ref(), "usecase");
        assert_eq!(layers[2].as_ref(), "domain");
    }

    #[test]
    fn test_parse_layer_filter_ids_trims_whitespace_and_skips_empty() {
        let layers = parse_layer_filter_ids(" domain ,, usecase , ").unwrap();
        assert_eq!(layers.len(), 2);
        assert_eq!(layers[0].as_ref(), "domain");
        assert_eq!(layers[1].as_ref(), "usecase");
    }

    /// Verifies that `TrackId::try_new` validation runs at the CLI boundary
    /// (CN-12) before git discovery, so a malformed `track_id` is always
    /// caught here and never masked by `cannot discover git repo` or
    /// branch-mismatch errors.
    ///
    /// `../evil` is rejected by `TrackId::try_new` (contains `/`) regardless
    /// of whether `workspace_root` is a valid git repository.
    #[test]
    fn test_execute_contract_map_with_invalid_track_id_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        std::fs::create_dir_all(&items_dir).unwrap();

        let result = execute_contract_map(items_dir, "../evil".to_owned(), dir.path().into(), None);
        let err = result.expect_err("path traversal track id must be rejected by CN-12");
        let msg = format!("{err}");
        assert!(
            msg.contains("invalid track ID"),
            "error must be a CN-12 track ID validation rejection, got: {msg}"
        );
    }

    /// Verifies that `parse_layer_filter_ids` returns an error for an invalid
    /// layer ID token, covering the CN-12 `LayerId::try_new` validation path
    /// directly without going through the CN-07 branch guard.
    #[test]
    fn test_parse_layer_filter_ids_invalid_value_returns_error() {
        // A value with an internal space is rejected by LayerId::try_new (CN-12).
        let result = parse_layer_filter_ids("domain core");
        assert!(result.is_err(), "layer id with space must be rejected");

        // A value starting with a digit is rejected by LayerId::try_new (CN-12).
        let result = parse_layer_filter_ids("1layer");
        assert!(result.is_err(), "layer id starting with digit must be rejected");
    }

    #[test]
    fn test_execute_contract_map_rejects_branch_mismatch_error_message() {
        // CN-07 branch guard: a well-formed track_id that does not match the
        // current git branch suffix must be rejected with a message that mentions
        // "contract-map rejected" (the BranchTrackMismatch or NonActiveTrack path
        // in `execute_contract_map`).
        //
        // Uses the real workspace root (from CARGO_MANIFEST_DIR) so that
        // SystemGitRepo::discover() finds the actual branch. Since the supplied
        // track_id does not match the current branch suffix, the CN-07 guard fires.
        // This pins the branch-forwarding wiring from the CLI into the guard.
        let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root from CARGO_MANIFEST_DIR")
            .to_path_buf();
        let items_dir = workspace_root.join("track/items");

        // A track_id that will never match the real current branch suffix.
        let result = execute_contract_map(
            items_dir,
            "this-id-will-never-match-the-real-branch".to_owned(),
            workspace_root,
            None,
        );
        let err = result.expect_err(
            "contract-map with mismatched track_id must be rejected by CN-07 branch guard",
        );
        let msg = format!("{err}");
        assert!(
            msg.contains("contract-map rejected"),
            "error must be a CN-07 branch guard rejection, got: {msg}"
        );
    }

    #[test]
    fn test_execute_contract_map_rejects_branch_track_id_mismatch() {
        // CN-07: verify that a well-formed track_id tied to a different branch is
        // rejected. The real workspace root is used so SystemGitRepo::discover()
        // finds the actual branch; since "test-done" does not match the current
        // branch suffix (e.g. "contract-map-v3-2026-05-20"), the CN-07 guard fires
        // with "contract-map rejected: branch ... does not match track_id 'test-done'".
        let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root from CARGO_MANIFEST_DIR")
            .to_path_buf();
        let items_dir = workspace_root.join("track/items");

        let result = execute_contract_map(items_dir, "test-done".to_owned(), workspace_root, None);
        let err = result.expect_err("contract-map must reject when branch doesn't match");
        let msg = format!("{err}");
        assert!(
            msg.contains("contract-map rejected"),
            "error must be a CN-07 branch guard rejection, got: {msg}"
        );
    }

    #[test]
    fn test_execute_contract_map_branch_guard_passes_for_current_track() {
        // Verify the branch-forwarding wiring: when track_id matches the current git
        // branch suffix, the CN-07 guard passes and execution reaches the interactor.
        //
        // Reads the current branch at runtime to derive the track_id, so this test is
        // independent of which specific branch name is checked out (not hard-coded to a
        // particular track). Skipped on non-track/ branches (detached HEAD, main, CI).
        let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root from CARGO_MANIFEST_DIR")
            .to_path_buf();
        let items_dir = workspace_root.join("track/items");

        // Derive the track_id from the ambient branch at test runtime.
        let Some(track_id) = current_track_id_suffix() else {
            // Not on a track/ branch (detached HEAD, main, CI) — skip.
            return;
        };

        let result = execute_contract_map(items_dir, track_id, workspace_root, None);

        // The CN-07 guard should pass (branch matches track_id).
        // The result is Ok (contract-map rendered) or Err (loader/renderer issue).
        // Either way, the error must NOT be a CN-07 branch guard rejection.
        if let Err(ref err) = result {
            let msg = format!("{err}");
            assert!(
                !msg.contains("contract-map rejected: branch"),
                "error must NOT be a CN-07 branch guard rejection — guard should pass for the current track, got: {msg}"
            );
        }
        // If Ok, the branch guard and the interactor both passed — ideal.
    }
}
