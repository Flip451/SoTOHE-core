//! `sotp track contract-map` — render the catalogue-input contract map
//! for a track.
//!
//! Composition root that wires the usecase interactor
//! (`usecase::contract_map_workflow::RenderContractMapInteractor`) to its
//! two secondary-port adapters (`FsCatalogueLoader` / `FsContractMapWriter`)
//! and dispatches through the `RenderContractMap` primary port so the CLI
//! stays substitutable for tests and future adapters.

use std::path::PathBuf;
use std::process::ExitCode;

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
/// active, `--kind-filter` / `--layers` cannot be parsed, or the
/// interactor fails (loader / writer / empty catalogue / unknown layer).
pub fn execute_contract_map(
    items_dir: PathBuf,
    track_id: String,
    workspace_root: PathBuf,
    kind_filter: Option<String>,
    layers: Option<String>,
) -> Result<ExitCode, CliError> {
    // Validate track_id and derive status without importing domain::ImplPlanReader (CN-01 / AC-03).
    let status_str = read_track_status_str(&items_dir, &track_id).map_err(|e| {
        CliError::Message(format!("cannot load track status for '{track_id}': {e}"))
    })?;
    ensure_active_track(&status_str, &track_id)?;

    let kind_filter_parsed = kind_filter.as_deref().map(parse_kind_filter_strings).transpose()?;
    let layer_filter_parsed = layers.as_deref().map(parse_layer_filter_strings).transpose()?;

    let rules_path = workspace_root.join("architecture-rules.json");
    let loader = FsCatalogueLoader::new(items_dir.clone(), rules_path, workspace_root.clone());
    let writer = FsContractMapWriter::new(items_dir.clone(), workspace_root);
    let interactor = RenderContractMapInteractor::new(loader, writer);

    // Dispatch through the primary port — CLI does not depend on the
    // concrete `RenderContractMapInteractor` type.
    let renderer: &dyn RenderContractMap = &interactor;
    let cmd = RenderContractMapCommand {
        track_id: track_id.clone(),
        kind_filter: kind_filter_parsed,
        layer_filter: layer_filter_parsed,
    };
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

/// Parses a `--kind-filter` CSV value into a list of kind tag strings.
/// Validation that the tag is recognised happens in the interactor.
fn parse_kind_filter_strings(raw: &str) -> Result<Vec<String>, CliError> {
    let mut kinds = Vec::new();
    for token in raw.split(',') {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            continue;
        }
        kinds.push(trimmed.to_ascii_lowercase());
    }
    Ok(kinds)
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
    fn test_parse_kind_filter_strings_single_value_succeeds() {
        let kinds = parse_kind_filter_strings("secondary_port").unwrap();
        assert_eq!(kinds, ["secondary_port"]);
    }

    #[test]
    fn test_parse_kind_filter_strings_multiple_values_succeeds() {
        let kinds = parse_kind_filter_strings("use_case,secondary_port,error_type").unwrap();
        assert_eq!(kinds, ["use_case", "secondary_port", "error_type"]);
    }

    #[test]
    fn test_parse_kind_filter_strings_all_14_variants_round_trip() {
        let all = "typestate,enum,value_object,error_type,secondary_port,secondary_adapter,\
                   application_service,use_case,interactor,dto,command,query,factory,free_function";
        let kinds = parse_kind_filter_strings(all).unwrap();
        assert_eq!(kinds.len(), 14);
    }

    #[test]
    fn test_parse_kind_filter_strings_trims_whitespace_and_skips_empty() {
        let kinds = parse_kind_filter_strings(" use_case ,, command , ").unwrap();
        assert_eq!(kinds, ["use_case", "command"]);
    }

    #[test]
    fn test_parse_kind_filter_strings_case_insensitive() {
        // Uppercase tokens are lowercased.
        let kinds = parse_kind_filter_strings("USE_CASE,SECONDARY_PORT").unwrap();
        assert_eq!(kinds, ["use_case", "secondary_port"]);
    }

    #[test]
    fn test_parse_kind_filter_strings_empty_string_returns_empty_vec() {
        let kinds = parse_kind_filter_strings("").unwrap();
        assert!(kinds.is_empty());
    }

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

        let result =
            execute_contract_map(items_dir, "../evil".to_owned(), dir.path().into(), None, None);
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
            execute_contract_map(items_dir, "test-done".to_owned(), dir.path().into(), None, None);
        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("Completed tracks are frozen"), "must reject done track: {msg}");
    }

    #[test]
    fn test_execute_contract_map_with_invalid_kind_filter_returns_error() {
        // Write v5 metadata (no status field) + impl-plan with in-progress task
        // so derive_track_status → InProgress → guard passes → kind-filter error surfaces.
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("test-track");
        std::fs::create_dir_all(&track_dir).unwrap();

        let metadata = r#"{
  "schema_version": 5, "id": "test-track", "branch": "track/test-track",
  "title": "Test",
  "created_at": "2026-04-17T00:00:00Z", "updated_at": "2026-04-17T00:00:00Z"
}"#;
        std::fs::write(track_dir.join("metadata.json"), metadata).unwrap();
        // In-progress task → derive_track_status → InProgress
        let impl_plan = r#"{"schema_version":1,"tasks":[{"id":"T001","description":"t","status":"in_progress"}],"plan":{"summary":[],"sections":[{"id":"S001","title":"t","description":[],"task_ids":["T001"]}]}}"#;
        std::fs::write(track_dir.join("impl-plan.json"), impl_plan).unwrap();

        let result = execute_contract_map(
            items_dir,
            "test-track".to_owned(),
            dir.path().into(),
            Some("nonexistent_kind".to_owned()),
            None,
        );
        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("nonexistent_kind"), "must surface the bad value: {msg}");
    }
}
