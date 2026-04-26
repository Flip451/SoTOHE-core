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

use domain::ImplPlanReader;
use domain::tddd::LayerId;
use domain::tddd::catalogue::{TypeDefinitionKind, TypestateTransitions};
use infrastructure::tddd::contract_map_adapter::{FsCatalogueLoader, FsContractMapWriter};
use infrastructure::track::fs_store::{FsTrackStore, read_track_metadata};
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
    let valid_id = domain::TrackId::try_new(&track_id)
        .map_err(|e| CliError::Message(format!("invalid track ID: {e}")))?;

    // Active-track guard (mirrors type-signals / type-graph).
    let (metadata, _doc_meta) = read_track_metadata(&items_dir, &valid_id)
        .map_err(|e| CliError::Message(format!("cannot load metadata for '{track_id}': {e}")))?;
    // Status is derived on demand from impl-plan + status_override.
    // Use FsTrackStore::load_impl_plan (fail-closed) so a corrupt impl-plan.json
    // blocks the guard instead of being silently treated as absent.
    let store = FsTrackStore::new(items_dir.clone());
    let impl_plan = store
        .load_impl_plan(&valid_id)
        .map_err(|e| CliError::Message(format!("cannot load impl-plan for '{track_id}': {e}")))?;
    let effective_status =
        domain::derive_track_status(impl_plan.as_ref(), metadata.status_override());
    ensure_active_track(effective_status, &track_id)?;

    let kind_filter_parsed = kind_filter.as_deref().map(parse_kind_filter).transpose()?;
    let layer_filter_parsed = layers.as_deref().map(parse_layer_filter).transpose()?;

    let rules_path = workspace_root.join("architecture-rules.json");
    let loader = FsCatalogueLoader::new(items_dir.clone(), rules_path, workspace_root.clone());
    let writer = FsContractMapWriter::new(items_dir.clone(), workspace_root);
    let interactor = RenderContractMapInteractor::new(loader, writer);

    // Dispatch through the primary port — CLI does not depend on the
    // concrete `RenderContractMapInteractor` type.
    let renderer: &dyn RenderContractMap = &interactor;
    let cmd = RenderContractMapCommand {
        track_id: valid_id,
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

/// Parses a `--kind-filter` CSV value into a list of
/// `TypeDefinitionKind`s. The renderer compares kinds by `kind_tag`, so
/// variants that carry payload fields are constructed with empty payload
/// placeholders.
fn parse_kind_filter(raw: &str) -> Result<Vec<TypeDefinitionKind>, CliError> {
    let mut kinds = Vec::new();
    for token in raw.split(',') {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            continue;
        }
        let kind = match trimmed.to_ascii_lowercase().as_str() {
            "typestate" => {
                TypeDefinitionKind::Typestate { transitions: TypestateTransitions::Terminal }
            }
            "enum" => TypeDefinitionKind::Enum { expected_variants: Vec::new() },
            "value_object" => TypeDefinitionKind::ValueObject,
            "error_type" => TypeDefinitionKind::ErrorType { expected_variants: Vec::new() },
            "secondary_port" => TypeDefinitionKind::SecondaryPort { expected_methods: Vec::new() },
            "secondary_adapter" => TypeDefinitionKind::SecondaryAdapter { implements: Vec::new() },
            "application_service" => {
                TypeDefinitionKind::ApplicationService { expected_methods: Vec::new() }
            }
            "use_case" => TypeDefinitionKind::UseCase,
            "interactor" => TypeDefinitionKind::Interactor { declares_application_service: None },
            "dto" => TypeDefinitionKind::Dto,
            "command" => TypeDefinitionKind::Command,
            "query" => TypeDefinitionKind::Query,
            "factory" => TypeDefinitionKind::Factory,
            other => {
                return Err(CliError::Message(format!(
                    "unknown --kind-filter value '{other}'; expected one of: \
                     typestate, enum, value_object, error_type, secondary_port, \
                     secondary_adapter, application_service, use_case, interactor, \
                     dto, command, query, factory"
                )));
            }
        };
        kinds.push(kind);
    }
    Ok(kinds)
}

/// Parses a `--layers` CSV value into a list of `LayerId`s.
fn parse_layer_filter(raw: &str) -> Result<Vec<LayerId>, CliError> {
    let mut layers = Vec::new();
    for token in raw.split(',') {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            continue;
        }
        let id = LayerId::try_new(trimmed.to_owned()).map_err(|e| {
            CliError::Message(format!("invalid layer id '{trimmed}' in --layers: {e}"))
        })?;
        layers.push(id);
    }
    Ok(layers)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_kind_filter_single_value_succeeds() {
        let kinds = parse_kind_filter("secondary_port").unwrap();
        assert_eq!(kinds.len(), 1);
        assert_eq!(kinds[0].kind_tag(), "secondary_port");
    }

    #[test]
    fn test_parse_kind_filter_multiple_values_succeeds() {
        let kinds = parse_kind_filter("use_case,secondary_port,error_type").unwrap();
        let tags: Vec<&str> = kinds.iter().map(TypeDefinitionKind::kind_tag).collect();
        assert_eq!(tags, ["use_case", "secondary_port", "error_type"]);
    }

    #[test]
    fn test_parse_kind_filter_all_13_variants_round_trip() {
        let all = "typestate,enum,value_object,error_type,secondary_port,secondary_adapter,\
                   application_service,use_case,interactor,dto,command,query,factory";
        let kinds = parse_kind_filter(all).unwrap();
        assert_eq!(kinds.len(), 13);
    }

    #[test]
    fn test_parse_kind_filter_trims_whitespace_and_skips_empty() {
        let kinds = parse_kind_filter(" use_case ,, command , ").unwrap();
        let tags: Vec<&str> = kinds.iter().map(TypeDefinitionKind::kind_tag).collect();
        assert_eq!(tags, ["use_case", "command"]);
    }

    #[test]
    fn test_parse_kind_filter_case_insensitive() {
        // Uppercase tokens must match the lowercase canonical `kind_tag`s.
        let kinds = parse_kind_filter("USE_CASE,SECONDARY_PORT").unwrap();
        let tags: Vec<&str> = kinds.iter().map(TypeDefinitionKind::kind_tag).collect();
        assert_eq!(tags, ["use_case", "secondary_port"]);
    }

    #[test]
    fn test_parse_kind_filter_unknown_returns_error_listing_valid_options() {
        let err = parse_kind_filter("bogus").unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("bogus"));
        for valid in ["typestate", "secondary_port", "factory"] {
            assert!(msg.contains(valid), "error should list '{valid}': {msg}");
        }
    }

    #[test]
    fn test_parse_kind_filter_empty_string_returns_empty_vec() {
        let kinds = parse_kind_filter("").unwrap();
        assert!(kinds.is_empty());
    }

    #[test]
    fn test_parse_layer_filter_single_value_succeeds() {
        let layers = parse_layer_filter("domain").unwrap();
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].as_ref(), "domain");
    }

    #[test]
    fn test_parse_layer_filter_multiple_values_preserves_order() {
        let layers = parse_layer_filter("infrastructure,usecase,domain").unwrap();
        let names: Vec<&str> = layers.iter().map(LayerId::as_ref).collect();
        assert_eq!(names, ["infrastructure", "usecase", "domain"]);
    }

    #[test]
    fn test_parse_layer_filter_trims_whitespace_and_skips_empty() {
        let layers = parse_layer_filter(" domain ,, usecase , ").unwrap();
        let names: Vec<&str> = layers.iter().map(LayerId::as_ref).collect();
        assert_eq!(names, ["domain", "usecase"]);
    }

    #[test]
    fn test_parse_layer_filter_invalid_id_returns_error() {
        let err = parse_layer_filter("bad layer id!").unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("bad layer id!"), "error should mention the bad token: {msg}");
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
