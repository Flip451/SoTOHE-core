//! System adapter for the active-track catalogue-lint port.

use std::path::Path;

use domain::TrackId;
use usecase::catalogue_lint_workflow::{
    RunCatalogueLint, RunCatalogueLintCommand, RunCatalogueLintError, RunCatalogueLintInteractor,
};
use usecase::track_lifecycle::TrackCataloguePath;
use usecase::track_lifecycle::tddd::catalogue_lint_active::{
    TrackCatalogueLintActiveCommand, TrackCatalogueLintActiveError, TrackCatalogueLintActivePort,
    TrackCatalogueLintActiveResult, TrackCatalogueLintLayerResult,
};

use crate::tddd::contract_map_adapter::FsCatalogueLoader;
use crate::tddd::fs_lint_config_loader::FsLintConfigLoader;
use crate::tddd::syn_primitive_occurrence_scanner::SynPrimitiveOccurrenceScanner;
use crate::verify::tddd_layers::{LoadTdddLayersError, load_tddd_layers};

/// System-backed adapter for active-track catalogue linting.
pub struct SystemTrackCatalogueLintActiveAdapter;

impl TrackCatalogueLintActivePort for SystemTrackCatalogueLintActiveAdapter {
    fn execute(
        &self,
        track_id: TrackId,
        command: TrackCatalogueLintActiveCommand,
    ) -> Result<TrackCatalogueLintActiveResult, TrackCatalogueLintActiveError> {
        let workspace_root = command.workspace_root.as_path();
        let rules_path = workspace_root.join("architecture-rules.json");
        let bindings = load_tddd_layers(&rules_path, workspace_root).map_err(|error| {
            execution_failed(format!("layer bindings load failed: {}", layer_bindings_error(error)))
        })?;
        if bindings.is_empty() {
            return Err(execution_failed(
                "no tddd.enabled layers found in architecture-rules.json; nothing to lint",
            ));
        }

        let items_dir = workspace_root.join("track/items");
        let track_dir = items_dir.join(track_id.as_ref());
        let config_path = command
            .rules_file
            .as_ref()
            .map(|rules_file| rules_file.as_path().to_path_buf())
            .unwrap_or_else(|| workspace_root.join(".harness/catalogue-lint/config.json"));
        ensure_config_file(&config_path)?;

        for binding in &bindings {
            let catalogue_path = track_dir.join(binding.catalogue_file());
            match catalogue_path.symlink_metadata() {
                Ok(metadata) if metadata.file_type().is_file() => {}
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    let layer = domain::tddd::LayerId::try_new(binding.layer_id().to_owned())
                        .map_err(|error| {
                            execution_failed(format!("invalid TDDD layer id: {error}"))
                        })?;
                    let path = TrackCataloguePath::try_new(catalogue_path)
                        .map_err(|error| execution_failed(error.to_string()))?;
                    return Ok(TrackCatalogueLintActiveResult::Skipped { layer, path });
                }
                Err(error) => {
                    return Err(execution_failed(format!(
                        "cannot stat catalogue '{}' for layer '{}': {error}",
                        catalogue_path.display(),
                        binding.layer_id(),
                    )));
                }
            }
        }

        let loader = FsCatalogueLoader::new(items_dir, rules_path, workspace_root.to_path_buf());
        let config_loader = FsLintConfigLoader::new(config_path);
        let interactor =
            RunCatalogueLintInteractor::new(loader, config_loader, SynPrimitiveOccurrenceScanner);
        let runner: &dyn RunCatalogueLint = &interactor;
        let mut layers = Vec::new();

        for binding in &bindings {
            let violations = match runner.execute(RunCatalogueLintCommand {
                track_id: track_id.as_ref().to_owned(),
                layer_id: binding.layer_id().to_owned(),
                rules: Vec::new(),
            }) {
                Ok(violations) => violations,
                Err(RunCatalogueLintError::ConfigMissing { path }) => {
                    return Err(execution_failed(lint_config_missing_message(&path)));
                }
                Err(error) => {
                    return Err(execution_failed(format!(
                        "catalogue lint failed for layer '{}': {error}",
                        binding.layer_id()
                    )));
                }
            };
            let layer = domain::tddd::LayerId::try_new(binding.layer_id().to_owned())
                .map_err(|error| execution_failed(format!("invalid TDDD layer id: {error}")))?;
            layers.push(TrackCatalogueLintLayerResult { layer, violations });
        }

        Ok(TrackCatalogueLintActiveResult::Checked { layers })
    }
}

fn ensure_config_file(path: &Path) -> Result<(), TrackCatalogueLintActiveError> {
    match path.symlink_metadata() {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(execution_failed(format!(
            "refusing to load a symlinked lint config: {}",
            path.display()
        ))),
        Ok(metadata) if metadata.is_file() => Ok(()),
        Ok(_) => {
            Err(execution_failed(format!("lint config is not a regular file: {}", path.display())))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(execution_failed(lint_config_missing_message(path)))
        }
        Err(error) => {
            Err(execution_failed(format!("cannot stat lint config '{}': {error}", path.display())))
        }
    }
}

fn lint_config_missing_message(path: &Path) -> String {
    format!(
        "lint config not found at {}. Copy `.harness/catalogue-lint/presets/ddd-strict.json` to that location to enable linting.",
        path.display()
    )
}

fn layer_bindings_error(error: LoadTdddLayersError) -> String {
    match error {
        LoadTdddLayersError::Io { path, source } => format!("{}: {source}", path.display()),
        LoadTdddLayersError::Parse(error) => error.to_string(),
    }
}

fn execution_failed(message: impl Into<String>) -> TrackCatalogueLintActiveError {
    TrackCatalogueLintActiveError::ExecutionFailed(usecase::git_workflow::DiagnosticText::new(
        message,
    ))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::fs;

    use super::*;
    use usecase::track_lifecycle::{TrackSelection, TrackWorkspaceRoot};

    fn command(root: &std::path::Path) -> TrackCatalogueLintActiveCommand {
        TrackCatalogueLintActiveCommand {
            track: TrackSelection::Explicit(
                TrackId::try_new("lint-track").expect("track id is valid"),
            ),
            workspace_root: TrackWorkspaceRoot::try_new(root.to_path_buf())
                .expect("workspace root is valid"),
            rules_file: None,
        }
    }

    fn write_rules(root: &std::path::Path) {
        fs::write(
            root.join("architecture-rules.json"),
            r#"{
              "version": 2,
              "layers": [{
                "crate": "domain",
                "tddd": {
                  "enabled": true,
                  "catalogue_file": "domain-types.json"
                }
              }]
            }"#,
        )
        .expect("architecture rules are written");
    }

    fn write_config(root: &std::path::Path) {
        let config = root.join(".harness/catalogue-lint");
        fs::create_dir_all(&config).expect("lint config directory exists");
        fs::write(config.join("config.json"), r#"{"schema_version":1,"rules":[]}"#)
            .expect("lint config is written");
    }

    #[test]
    fn test_system_track_catalogue_lint_active_adapter_missing_rules_returns_execution_error() {
        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        let error = match SystemTrackCatalogueLintActiveAdapter.execute(
            TrackId::try_new("lint-track").expect("track id is valid"),
            command(workspace.path()),
        ) {
            Ok(_) => panic!("missing architecture rules must fail"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("layer bindings load failed"));
    }

    #[test]
    fn test_system_track_catalogue_lint_active_adapter_absent_catalogue_returns_skipped_layer() {
        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        fs::create_dir_all(workspace.path().join("track/items/lint-track"))
            .expect("track directory exists");
        write_rules(workspace.path());
        write_config(workspace.path());

        let result = SystemTrackCatalogueLintActiveAdapter
            .execute(
                TrackId::try_new("lint-track").expect("track id is valid"),
                command(workspace.path()),
            )
            .expect("absent catalogue is skipped");

        assert!(matches!(
            result,
            TrackCatalogueLintActiveResult::Skipped { layer, path }
                if layer.as_ref() == "domain" && path.as_path().ends_with("domain-types.json")
        ));
    }

    #[test]
    fn test_system_track_catalogue_lint_active_adapter_missing_config_returns_execution_error() {
        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        fs::create_dir_all(workspace.path().join("track/items/lint-track"))
            .expect("track directory exists");
        write_rules(workspace.path());

        let error = match SystemTrackCatalogueLintActiveAdapter.execute(
            TrackId::try_new("lint-track").expect("track id is valid"),
            command(workspace.path()),
        ) {
            Ok(_) => panic!("missing config must fail closed"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("lint config not found"));
    }

    #[test]
    fn test_system_track_catalogue_lint_active_adapter_directory_config_fails_when_catalogue_absent()
     {
        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        fs::create_dir_all(workspace.path().join("track/items/lint-track"))
            .expect("track directory exists");
        write_rules(workspace.path());
        fs::create_dir_all(workspace.path().join(".harness/catalogue-lint/config.json"))
            .expect("directory occupies the lint config path");

        let error = match SystemTrackCatalogueLintActiveAdapter.execute(
            TrackId::try_new("lint-track").expect("track id is valid"),
            command(workspace.path()),
        ) {
            Ok(_) => panic!("directory config must fail closed"),
            Err(error) => error,
        };

        assert!(
            error.to_string().contains("lint config is not a regular file"),
            "directory config must fail closed: {error}"
        );
    }

    #[test]
    fn test_system_track_catalogue_lint_active_adapter_symlink_config_fails_when_catalogue_absent()
    {
        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        fs::create_dir_all(workspace.path().join("track/items/lint-track"))
            .expect("track directory exists");
        write_rules(workspace.path());
        let config_dir = workspace.path().join(".harness/catalogue-lint");
        fs::create_dir_all(&config_dir).expect("lint config directory exists");
        let target = config_dir.join("target.json");
        fs::write(&target, r#"{"schema_version":1,"rules":[]}"#)
            .expect("symlink target is written");
        std::os::unix::fs::symlink(&target, config_dir.join("config.json"))
            .expect("lint config symlink is created");

        let error = match SystemTrackCatalogueLintActiveAdapter.execute(
            TrackId::try_new("lint-track").expect("track id is valid"),
            command(workspace.path()),
        ) {
            Ok(_) => panic!("symlinked config must fail closed"),
            Err(error) => error,
        };

        assert!(
            error.to_string().contains("refusing to load a symlinked lint config"),
            "symlinked config must fail closed: {error}"
        );
    }
}
