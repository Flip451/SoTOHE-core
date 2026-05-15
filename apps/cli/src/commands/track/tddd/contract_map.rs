//! `sotp track contract-map` — render the catalogue-input contract map
//! for a track.
//!
//! Composition root that wires the usecase interactor
//! (`usecase::contract_map_workflow::RenderContractMapInteractor`) to its
//! three secondary-port adapters (`FsCatalogueLoader` / `ContractMapRendererAdapter` /
//! `FsContractMapWriter`) and dispatches through the `RenderContractMap` primary port
//! so the CLI stays substitutable for tests and future adapters.

use std::path::PathBuf;
use std::process::ExitCode;

use infrastructure::tddd::ContractMapRendererAdapter;
use infrastructure::tddd::contract_map_adapter::{FsCatalogueLoader, FsContractMapWriter};
use infrastructure::track::fs_store::read_track_status_str;
use usecase::contract_map_workflow::{
    RenderContractMap, RenderContractMapCommand, RenderContractMapInteractor,
};

use crate::CliError;

use super::signals::ensure_active_track;

/// Render the Contract Map for a single track.
///
/// # Errors
///
/// Returns `CliError` when the track id is invalid, the track is not
/// active, `--layers` cannot be parsed, or the interactor fails
/// (loader / writer / empty catalogue / unknown layer).
pub fn execute_contract_map(
    items_dir: PathBuf,
    track_id: String,
    workspace_root: PathBuf,
    layers: Option<String>,
) -> Result<ExitCode, CliError> {
    // Validate track_id and derive status without importing domain::ImplPlanReader (CN-01 / AC-03).
    let status_str = read_track_status_str(&items_dir, &track_id).map_err(|e| {
        CliError::Message(format!("cannot load track status for '{track_id}': {e}"))
    })?;
    ensure_active_track(&status_str, &track_id)?;

    let layer_filter_parsed = layers.as_deref().map(parse_layer_filter_strings).transpose()?;

    let rules_path = workspace_root.join("architecture-rules.json");
    let loader = FsCatalogueLoader::new(items_dir.clone(), rules_path, workspace_root.clone());
    let style_config_path = workspace_root.join(".harness/config/contract-map-style.toml");
    let renderer = ContractMapRendererAdapter::new(style_config_path);
    let writer = FsContractMapWriter::new(items_dir.clone(), workspace_root);
    let interactor = RenderContractMapInteractor::new(loader, renderer, writer);

    // Dispatch through the primary port — CLI does not depend on the
    // concrete `RenderContractMapInteractor` type.
    let renderer: &dyn RenderContractMap = &interactor;
    let cmd =
        RenderContractMapCommand { track_id: track_id.clone(), layer_filter: layer_filter_parsed };
    let out = renderer
        .execute(&cmd)
        .map_err(|e| CliError::Message(format!("contract-map render failed: {e}")))?;

    println!(
        "[OK] contract-map: wrote track/items/{track_id}/contract-map.md \
         (layers={}, entries={})",
        out.rendered_layer_count, out.total_entry_count,
    );
    Ok(ExitCode::SUCCESS)
}

/// Parses a `--layers` CSV value into a list of layer identifier strings.
/// Validation that the layer is enabled happens in the interactor.
fn parse_layer_filter_strings(raw: &str) -> Result<Vec<String>, CliError> {
    let mut layers = Vec::new();
    for token in raw.split(',') {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            continue;
        }
        layers.push(trimmed.to_owned());
    }
    Ok(layers)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_layer_filter_strings_single_value_succeeds() {
        let layers = parse_layer_filter_strings("domain").unwrap();
        assert_eq!(layers, ["domain"]);
    }

    #[test]
    fn test_parse_layer_filter_strings_multiple_values_preserves_order() {
        let layers = parse_layer_filter_strings("infrastructure,usecase,domain").unwrap();
        assert_eq!(layers, ["infrastructure", "usecase", "domain"]);
    }

    #[test]
    fn test_parse_layer_filter_strings_trims_whitespace_and_skips_empty() {
        let layers = parse_layer_filter_strings(" domain ,, usecase , ").unwrap();
        assert_eq!(layers, ["domain", "usecase"]);
    }

    #[test]
    fn test_execute_contract_map_with_invalid_track_id_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        std::fs::create_dir_all(&items_dir).unwrap();

        let result = execute_contract_map(items_dir, "../evil".to_owned(), dir.path().into(), None);
        assert!(result.is_err(), "path traversal track id must be rejected");
    }

    #[test]
    fn test_execute_contract_map_rejects_done_track() {
        // Write v5 metadata (no status field) + impl-plan with all tasks done
        // so derive_track_status → Done → guard rejects.
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("test-done");
        std::fs::create_dir_all(&track_dir).unwrap();

        let metadata = r#"{
  "schema_version": 5, "id": "test-done", "branch": "track/test-done",
  "title": "Done",
  "created_at": "2026-04-16T00:00:00Z", "updated_at": "2026-04-16T00:00:00Z"
}"#;
        std::fs::write(track_dir.join("metadata.json"), metadata).unwrap();
        // All tasks done → derive_track_status → Done
        let impl_plan = r#"{"schema_version":1,"tasks":[{"id":"T001","description":"t","status":"done","commit_hash":"abc1234"}],"plan":{"summary":[],"sections":[{"id":"S001","title":"t","description":[],"task_ids":["T001"]}]}}"#;
        std::fs::write(track_dir.join("impl-plan.json"), impl_plan).unwrap();

        let result =
            execute_contract_map(items_dir, "test-done".to_owned(), dir.path().into(), None);
        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("Completed tracks are frozen"), "must reject done track: {msg}");
    }

    /// Smoke test verifying that `execute_contract_map` exercises the
    /// `ContractMapRendererAdapter` code path rather than a placeholder.
    ///
    /// The fixture wires a valid in-progress track with a single TDDD-enabled
    /// layer and an empty catalogue, but omits the style config. The adapter
    /// is fail-closed (CN-03): a missing style config returns
    /// `ContractMapRendererError::StyleConfigNotFound`, which propagates via
    /// `RenderContractMapError::RendererFailed` and surfaces in the CLI error
    /// message.  A placeholder renderer would return `Ok(...)` unconditionally
    /// — observing the `StyleConfigNotFound` error proves the adapter is wired.
    #[test]
    fn test_execute_contract_map_wires_adapter_not_placeholder() {
        let dir = tempfile::tempdir().unwrap();
        let workspace_root = dir.path().to_path_buf();

        // Set up track/items/<track-id>/ with in-progress status.
        let items_dir = workspace_root.join("track/items");
        let track_id = "test-adapter-wire";
        let track_dir = items_dir.join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();

        let metadata = r#"{
  "schema_version": 5, "id": "test-adapter-wire", "branch": "track/test-adapter-wire",
  "title": "Adapter wire smoke test",
  "created_at": "2026-05-15T00:00:00Z", "updated_at": "2026-05-15T00:00:00Z"
}"#;
        std::fs::write(track_dir.join("metadata.json"), metadata).unwrap();
        // One task in_progress → derive_track_status → InProgress (active).
        let impl_plan = r#"{"schema_version":1,"tasks":[{"id":"T001","description":"t","status":"in_progress"}],"plan":{"summary":[],"sections":[{"id":"S001","title":"t","description":[],"task_ids":["T001"]}]}}"#;
        std::fs::write(track_dir.join("impl-plan.json"), impl_plan).unwrap();

        // Minimal architecture-rules.json: one TDDD-enabled layer ("domain").
        let rules_json = r#"{
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
        std::fs::write(workspace_root.join("architecture-rules.json"), rules_json).unwrap();

        // Minimal v3 catalogue (empty entries).
        let catalogue_v3 = r#"{
  "schema_version": 3,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {},
  "functions": {}
}"#;
        std::fs::write(track_dir.join("domain-types.json"), catalogue_v3).unwrap();

        // Intentionally omit .harness/config/contract-map-style.toml so the
        // ContractMapRendererAdapter returns StyleConfigNotFound (fail-closed, CN-03).
        // A placeholder renderer would succeed → observing the error proves adapter is wired.
        let result = execute_contract_map(items_dir, track_id.to_owned(), workspace_root, None);
        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("contract-map-style.toml") || msg.contains("style config"),
            "expected StyleConfigNotFound error proving adapter is wired; got: {msg}"
        );
    }
}
