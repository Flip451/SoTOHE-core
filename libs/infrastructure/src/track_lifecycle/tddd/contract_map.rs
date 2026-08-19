//! System adapter for the Track TDDD contract-map port.

use std::path::Path;

use domain::TrackId;
use usecase::contract_map_workflow::{
    RenderContractMap, RenderContractMapCommand, RenderContractMapInteractor,
};
use usecase::track_lifecycle::TrackLayerFilter;
use usecase::track_lifecycle::tddd::contract_map::{
    TrackContractMapCommand, TrackContractMapError, TrackContractMapPort, TrackContractMapResult,
};

/// System-backed adapter for rendering and persisting a contract map.
pub struct SystemTrackContractMapAdapter;

impl TrackContractMapPort for SystemTrackContractMapAdapter {
    fn execute(
        &self,
        track_id: TrackId,
        command: TrackContractMapCommand,
    ) -> Result<TrackContractMapResult, TrackContractMapError> {
        let items_dir = command.items_dir.as_path().to_path_buf();
        let workspace_root = command.workspace_root.as_path().to_path_buf();
        validate_items_dir_within_workspace(&items_dir, &workspace_root)
            .map_err(|error| execution_failed(format!("contract-map render failed: {error}")))?;

        let layer_filter = match command.layers {
            TrackLayerFilter::All => None,
            TrackLayerFilter::Selected(layers) => Some(layers),
        };
        let loader = crate::tddd::contract_map_adapter::FsCatalogueLoader::new(
            items_dir.clone(),
            workspace_root.join("architecture-rules.json"),
            workspace_root.clone(),
        );
        let renderer = crate::tddd::contract_map_renderer_adapter::ContractMapRendererAdapter::new(
            workspace_root.join(".harness/config/contract-map-style.toml"),
        );
        let writer = crate::tddd::contract_map_adapter::FsContractMapWriter::new(
            items_dir,
            workspace_root.clone(),
        );
        let workflow = RenderContractMapInteractor::new(loader, renderer, writer);
        let result = workflow
            .execute(&RenderContractMapCommand { track_id: track_id.clone(), layer_filter })
            .map_err(|error| execution_failed(format!("contract-map render failed: {error}")))?;

        Ok(TrackContractMapResult {
            track_id,
            rendered_layers: usecase::track_lifecycle::TrackRenderedLayerCount::new(
                result.rendered_layer_count,
            ),
            catalogue_entries: usecase::track_lifecycle::TrackCatalogueEntryCount::new(
                result.total_entry_count,
            ),
            warnings: result.warnings,
        })
    }
}

fn validate_items_dir_within_workspace(
    items_dir: &Path,
    workspace_root: &Path,
) -> Result<(), String> {
    let canonical_workspace = workspace_root.canonicalize().map_err(|error| {
        format!("cannot resolve workspace root '{}': {error}", workspace_root.display())
    })?;
    let canonical_items = items_dir.canonicalize().map_err(|error| {
        format!("cannot resolve track items directory '{}': {error}", items_dir.display())
    })?;
    if !canonical_items.starts_with(&canonical_workspace) {
        return Err(format!(
            "track items directory '{}' resolves outside workspace root '{}'; only paths under the workspace are allowed",
            items_dir.display(),
            workspace_root.display()
        ));
    }
    Ok(())
}

fn execution_failed(message: impl Into<String>) -> TrackContractMapError {
    TrackContractMapError::ExecutionFailed(usecase::git_workflow::DiagnosticText::new(message))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::fs;

    use super::*;
    use usecase::track_lifecycle::{TrackItemsDirectory, TrackSelection, TrackWorkspaceRoot};

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

    fn command(root: &std::path::Path) -> TrackContractMapCommand {
        TrackContractMapCommand {
            track: TrackSelection::Explicit(
                TrackId::try_new("contract-map-track").expect("track id is valid"),
            ),
            items_dir: TrackItemsDirectory::try_new(root.join("track/items"))
                .expect("items directory is valid"),
            workspace_root: TrackWorkspaceRoot::try_new(root.to_path_buf())
                .expect("workspace root is valid"),
            layers: TrackLayerFilter::All,
        }
    }

    fn write_valid_fixture(root: &std::path::Path) {
        let track_dir = root.join("track/items/contract-map-track");
        fs::create_dir_all(&track_dir).expect("track directory exists");
        fs::create_dir_all(root.join(".harness/config")).expect("config directory exists");
        fs::write(root.join("architecture-rules.json"), RULES_JSON)
            .expect("architecture rules are written");
        fs::write(track_dir.join("domain-types.json"), EMPTY_CATALOGUE)
            .expect("catalogue is written");
        fs::write(root.join(".harness/config/contract-map-style.toml"), MINIMAL_STYLE_CONFIG)
            .expect("style config is written");
    }

    #[test]
    fn test_system_track_contract_map_adapter_writes_typed_summary() {
        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        write_valid_fixture(workspace.path());

        let result = SystemTrackContractMapAdapter
            .execute(
                TrackId::try_new("contract-map-track").expect("track id is valid"),
                command(workspace.path()),
            )
            .expect("contract map renders");

        assert_eq!(result.track_id.as_ref(), "contract-map-track");
        assert_eq!(result.rendered_layers.value(), 1);
        assert_eq!(result.catalogue_entries.value(), 0);
        assert!(workspace.path().join("track/items/contract-map-track/contract-map.md").is_file());
    }

    #[test]
    fn test_system_track_contract_map_adapter_missing_rules_returns_execution_error() {
        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        fs::create_dir_all(workspace.path().join("track/items/contract-map-track"))
            .expect("track directory exists");

        let error = match SystemTrackContractMapAdapter.execute(
            TrackId::try_new("contract-map-track").expect("track id is valid"),
            command(workspace.path()),
        ) {
            Ok(_) => panic!("missing architecture rules must fail closed"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("contract-map render failed"));
    }

    #[test]
    fn test_system_track_contract_map_adapter_items_dir_outside_workspace_returns_execution_error()
    {
        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        let outside = tempfile::tempdir().expect("outside workspace exists");
        let outside_items = outside.path().join("track/items");
        fs::create_dir_all(&outside_items).expect("outside items directory exists");
        let mut map_command = command(workspace.path());
        map_command.items_dir = TrackItemsDirectory::try_new(outside_items)
            .expect("outside items path uses conventional suffix");

        let error = match SystemTrackContractMapAdapter.execute(
            TrackId::try_new("contract-map-track").expect("track id is valid"),
            map_command,
        ) {
            Ok(_) => panic!("outside items directory must fail closed"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("outside workspace root"));
    }

    #[cfg(unix)]
    #[test]
    fn test_system_track_contract_map_adapter_preserves_original_path_for_symlink_guard() {
        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        let real_items = workspace.path().join("real-items");
        fs::create_dir_all(&real_items).expect("real items directory exists");
        let track_dir = workspace.path().join("track");
        fs::create_dir_all(&track_dir).expect("track directory exists");
        std::os::unix::fs::symlink(&real_items, track_dir.join("items"))
            .expect("items symlink exists");
        let mut map_command = command(workspace.path());
        map_command.items_dir = TrackItemsDirectory::try_new(track_dir.join("items"))
            .expect("symlinked items path uses conventional suffix");
        fs::write(workspace.path().join("architecture-rules.json"), RULES_JSON)
            .expect("architecture rules are written");

        let error = match SystemTrackContractMapAdapter.execute(
            TrackId::try_new("contract-map-track").expect("track id is valid"),
            map_command,
        ) {
            Ok(_) => panic!("symlinked items directory must fail closed"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("contract-map render failed"));
    }
}
