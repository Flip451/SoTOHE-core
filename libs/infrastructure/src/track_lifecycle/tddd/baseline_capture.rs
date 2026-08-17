//! System adapter for the Track TDDD baseline-capture port.

use std::sync::Arc;

use domain::SymlinkGuardPort;
use domain::TrackId;
use domain::tddd::catalogue_v2::TdddLayerBindingsPort;
use usecase::baseline_capture::{
    BaselineCaptureInteractor, BaselineCaptureRequest, BaselineCaptureService,
};
use usecase::track_lifecycle::TrackLayerSelection;
use usecase::track_lifecycle::tddd::baseline_capture::{
    TrackBaselineCaptureCommand, TrackBaselineCaptureError, TrackBaselineCaptureLayerResult,
    TrackBaselineCapturePort, TrackBaselineCaptureResult,
};

/// System-backed adapter for the Track TDDD baseline-capture operation.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemTrackBaselineCaptureAdapter;

impl TrackBaselineCapturePort for SystemTrackBaselineCaptureAdapter {
    fn execute(
        &self,
        track_id: TrackId,
        command: TrackBaselineCaptureCommand,
    ) -> Result<TrackBaselineCaptureResult, TrackBaselineCaptureError> {
        let symlink_guard: Arc<dyn SymlinkGuardPort> = Arc::new(crate::FsSymlinkGuard::new());
        symlink_guard.reject_symlinks_from_root(command.workspace_root.as_path()).map_err(
            |error| execution_failed(format!("workspace root symlink guard failed: {error}")),
        )?;

        let items_dir = command.workspace_root.as_path().join("track").join("items");
        let layer_filter = match &command.layer {
            TrackLayerSelection::All => None,
            TrackLayerSelection::One(layer) => Some(layer.as_ref()),
        };
        let bindings = infrastructure_bindings(&command.workspace_root, layer_filter)?;
        let existing = bindings
            .iter()
            .map(|binding| items_dir.join(track_id.as_ref()).join(&binding.baseline_file).is_file())
            .collect::<Vec<_>>();

        let baseline_capture = BaselineCaptureInteractor::new(
            symlink_guard,
            Arc::new(crate::tddd::tddd_layer_bindings_adapter::FsTdddLayerBindingsAdapter::new()),
            Arc::new(
                crate::tddd::rustdoc_baseline_capture_adapter::RustdocBaselineCaptureAdapter::new(),
            ),
            Arc::new(
                crate::tddd::feature_declaration_adapter::FsTdddFeatureDeclarationAdapter::new(),
            ),
        );
        baseline_capture
            .run(BaselineCaptureRequest {
                track_id: track_id.as_ref().to_owned(),
                workspace_root: command.workspace_root.as_path().to_path_buf(),
                source_workspace: command
                    .source_workspace
                    .as_ref()
                    .map(|workspace| workspace.as_path().to_path_buf()),
                layer: layer_filter.map(str::to_owned),
            })
            .map_err(|error| execution_failed(error.to_string()))?;

        let layers = bindings
            .into_iter()
            .zip(existing)
            .map(|(binding, existed)| {
                let layer = domain::tddd::LayerId::try_new(binding.layer_id)
                    .map_err(|error| execution_failed(format!("invalid TDDD layer id: {error}")))?;
                Ok(if existed {
                    TrackBaselineCaptureLayerResult::AlreadyExists { layer }
                } else {
                    TrackBaselineCaptureLayerResult::Captured { layer }
                })
            })
            .collect::<Result<Vec<_>, TrackBaselineCaptureError>>()?;

        Ok(TrackBaselineCaptureResult { layers })
    }
}

fn infrastructure_bindings(
    workspace_root: &usecase::track_lifecycle::TrackWorkspaceRoot,
    layer_filter: Option<&str>,
) -> Result<Vec<domain::tddd::catalogue_v2::TdddLayerBinding>, TrackBaselineCaptureError> {
    crate::tddd::tddd_layer_bindings_adapter::FsTdddLayerBindingsAdapter::new()
        .load(workspace_root.as_path(), layer_filter)
        .map_err(|error| execution_failed(format!("layer bindings load failed: {error}")))
}

fn execution_failed(message: impl Into<String>) -> TrackBaselineCaptureError {
    TrackBaselineCaptureError::ExecutionFailed(usecase::git_workflow::DiagnosticText::new(message))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::fs;

    use rustdoc_types::FORMAT_VERSION;
    use usecase::track_lifecycle::tddd::baseline_capture::TrackBaselineCapturePort as _;
    use usecase::track_lifecycle::{TrackSelection, TrackWorkspaceRoot};

    use super::*;

    fn minimal_rustdoc_json() -> String {
        format!(
            r#"{{
                "root": 0,
                "crate_version": null,
                "includes_private": false,
                "index": {{}},
                "paths": {{}},
                "external_crates": {{}},
                "format_version": {FORMAT_VERSION},
                "target": {{"triple": "", "target_features": []}}
            }}"#
        )
    }

    fn write_fixture(root: &std::path::Path) {
        let track_dir = root.join("track/items/capture-track");
        fs::create_dir_all(&track_dir).expect("track directory exists");
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"libs/domain\"]\nresolver = \"2\"\n",
        )
        .expect("workspace manifest is written");
        fs::create_dir_all(root.join("libs/domain")).expect("domain crate directory exists");
        fs::write(
            root.join("libs/domain/Cargo.toml"),
            "[package]\nname = \"domain\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .expect("domain manifest is written");
        fs::write(
            root.join("architecture-rules.json"),
            r#"{
              "version": 2,
              "layers": [
                {"crate": "domain", "tddd": {"enabled": true}}
              ]
            }"#,
        )
        .expect("architecture rules are written");
        let declaration = r#"{"schema_version":1,"layers":{"domain":[]}}"#;
        fs::write(track_dir.join("tddd-features.json"), declaration)
            .expect("feature declaration is written");
        fs::write(track_dir.join("tddd-features-baseline.json"), declaration)
            .expect("feature declaration snapshot is written");
        fs::write(track_dir.join("domain-types-baseline.json"), minimal_rustdoc_json())
            .expect("baseline is written");
    }

    fn command(root: &std::path::Path) -> TrackBaselineCaptureCommand {
        TrackBaselineCaptureCommand {
            track: TrackSelection::Explicit(
                TrackId::try_new("capture-track").expect("track id is valid"),
            ),
            workspace_root: TrackWorkspaceRoot::try_new(root.to_path_buf())
                .expect("workspace root is valid"),
            source_workspace: None,
            layer: TrackLayerSelection::All,
        }
    }

    #[test]
    fn test_system_track_baseline_capture_adapter_existing_baseline_returns_already_exists() {
        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        write_fixture(workspace.path());

        let result = SystemTrackBaselineCaptureAdapter
            .execute(
                TrackId::try_new("capture-track").expect("track id is valid"),
                command(workspace.path()),
            )
            .expect("existing baseline is idempotent");

        assert!(matches!(
            result.layers.as_slice(),
            [TrackBaselineCaptureLayerResult::AlreadyExists { layer }] if layer.as_ref() == "domain"
        ));
    }

    #[test]
    fn test_system_track_baseline_capture_adapter_missing_rules_returns_execution_error() {
        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        let error = SystemTrackBaselineCaptureAdapter
            .execute(
                TrackId::try_new("capture-track").expect("track id is valid"),
                command(workspace.path()),
            )
            .expect_err("missing rules must fail");

        assert!(error.to_string().contains("layer bindings load failed"));
    }

    #[cfg(unix)]
    #[test]
    fn test_system_track_baseline_capture_adapter_missing_baseline_returns_captured() {
        const CHILD_STATE_ENV: &str = "TRACK_BASELINE_CAPTURE_TEST_STATE";
        const TEST_NAME: &str = concat!(
            "infrastructure::track_lifecycle::tddd::baseline_capture::tests::",
            "test_system_track_baseline_capture_adapter_missing_baseline_returns_captured"
        );

        if let Some(state_dir) = std::env::var_os(CHILD_STATE_ENV) {
            let workspace = std::path::Path::new(&state_dir).join("workspace");
            write_fixture(&workspace);
            fs::remove_file(workspace.join("track/items/capture-track/domain-types-baseline.json"))
                .expect("the captured baseline starts absent");

            let result = SystemTrackBaselineCaptureAdapter
                .execute(
                    TrackId::try_new("capture-track").expect("track id is valid"),
                    command(workspace.as_path()),
                )
                .expect("the missing baseline is captured");

            assert!(matches!(
                result.layers.as_slice(),
                [TrackBaselineCaptureLayerResult::Captured { layer }] if layer.as_ref() == "domain"
            ));
            assert!(
                workspace.join("track/items/capture-track/domain-types-baseline.json").is_file(),
                "capture must write the missing baseline"
            );
            return;
        }

        let state = tempfile::tempdir().expect("temporary test state exists");
        let commands_dir = state.path().join("commands");
        let target_dir = state.path().join("target");
        let workspace = state.path().join("workspace");
        fs::create_dir_all(&commands_dir).expect("command directory exists");
        write_fixture(&workspace);
        fs::remove_file(workspace.join("track/items/capture-track/domain-types-baseline.json"))
            .expect("the captured baseline starts absent");

        let rustup = commands_dir.join("rustup");
        fs::write(&rustup, "#!/bin/sh\nexit 0\n").expect("rustup shim is written");
        let cargo = commands_dir.join("cargo");
        fs::write(
            &cargo,
            format!(
                "#!/bin/sh\nif [ \"$1\" = \"metadata\" ]; then\nprintf '{{\"packages\":[{{\"name\":\"domain\",\"targets\":[{{\"kind\":[\"lib\"],\"name\":\"domain\"}}]}}],\"target_directory\":\"%s\"}}\\n' \"$CARGO_TARGET_DIR\"\nexit 0\nfi\nmkdir -p \"$CARGO_TARGET_DIR/doc\"\nprintf '%s' '{}' > \"$CARGO_TARGET_DIR/doc/domain.json\"\n",
                minimal_rustdoc_json()
            ),
        )
        .expect("cargo shim is written");

        use std::os::unix::fs::PermissionsExt as _;
        for command in [&rustup, &cargo] {
            let mut permissions =
                fs::metadata(command).expect("command metadata is available").permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(command, permissions).expect("command is executable");
        }

        let mut path_entries = vec![commands_dir];
        path_entries.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()));
        let mut child = std::process::Command::new(
            std::env::current_exe().expect("current test executable is available"),
        );
        child
            .args(["--exact", TEST_NAME, "--nocapture"])
            .env(CHILD_STATE_ENV, state.path())
            .env("CARGO_TARGET_DIR", &target_dir)
            .env("PATH", std::env::join_paths(path_entries).expect("PATH is valid"));
        assert!(child.status().expect("child test starts").success());
    }
}
