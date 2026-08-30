//! System adapter for the Track TDDD type-signals port.

use std::path::Path;
use std::sync::Arc;

use domain::{ConfidenceSignal, SignalCounts, TrackBranch, TrackId};
use usecase::track_lifecycle::tddd::type_signals::{
    TrackTypeSignalsCommand, TrackTypeSignalsError, TrackTypeSignalsPort, TrackTypeSignalsResult,
};
use usecase::track_lifecycle::{TrackLayerSelection, TrackLayerSignalResult};
use usecase::type_signals::{TypeSignalsInteractor, TypeSignalsRequest, TypeSignalsService};

const MAX_CURRENT_BRANCH_OUTPUT_BYTES: usize = 4096;

/// System-backed adapter for TDDD type-signal evaluation.
pub struct SystemTrackTypeSignalsAdapter;

impl TrackTypeSignalsPort for SystemTrackTypeSignalsAdapter {
    fn execute(
        &self,
        track_id: TrackId,
        command: TrackTypeSignalsCommand,
    ) -> Result<TrackTypeSignalsResult, TrackTypeSignalsError> {
        let workspace_root = command.workspace_root.as_path().to_path_buf();
        let items_dir = workspace_root.join("track").join("items");
        validate_items_dir_within_workspace(&items_dir, &workspace_root)?;

        let layer_filter = match &command.layer {
            TrackLayerSelection::All => None,
            TrackLayerSelection::One(layer) => Some(layer.as_ref().to_owned()),
        };
        let bindings = crate::verify::tddd_layers::load_tddd_layers_from_workspace(&workspace_root)
            .map_err(|error| execution_failed(format!("layer bindings load failed: {error}")))?;
        let selected_bindings = bindings
            .iter()
            .filter(|binding| {
                layer_filter.as_deref().is_none_or(|layer| binding.layer_id() == layer)
            })
            .collect::<Vec<_>>();
        if selected_bindings.is_empty() {
            return Err(execution_failed(
                layer_filter
                    .map(|layer| format!("layer '{layer}' is not tddd.enabled in architecture-rules.json"))
                    .unwrap_or_else(|| {
                        "no tddd.enabled layers found in architecture-rules.json; nothing to evaluate"
                            .to_owned()
                    }),
            ));
        }

        let canonical_root = workspace_root.canonicalize().map_err(|error| {
            execution_failed(format!(
                "cannot resolve workspace root '{}': {error}",
                workspace_root.display()
            ))
        })?;
        let repository = crate::git_cli::SystemGitRepo::discover_from_isolated(&canonical_root)
            .map_err(|error| {
                execution_failed(format!("git repository discovery failed: {error}"))
            })?;
        let output = crate::git_cli::isolated_bounded_git_output(
            repository.root(),
            &["rev-parse", "--abbrev-ref", "HEAD"],
            MAX_CURRENT_BRANCH_OUTPUT_BYTES,
        )
        .map_err(|error| execution_failed(format!("cannot read current branch: {error}")))?;
        if !output.status.success() {
            return Err(execution_failed("cannot read current branch"));
        }
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if branch.is_empty() || branch == "HEAD" {
            return Err(execution_failed("cannot read current branch"));
        }
        let branch = TrackBranch::try_new(branch)
            .map_err(|error| execution_failed(format!("invalid current branch: {error}")))?;

        let layer = layer_filter
            .as_deref()
            .map(|layer| {
                domain::tddd::LayerId::try_new(layer.to_owned())
                    .map_err(|error| execution_failed(format!("invalid TDDD layer id: {error}")))
            })
            .transpose()?;
        let interactor = TypeSignalsInteractor::new(
            Arc::new(crate::tddd::tddd_layer_bindings_adapter::FsTdddLayerBindingsAdapter::new()),
            Arc::new(crate::tddd::type_signals_executor_adapter::TypeSignalsExecutorAdapter::new()),
            Arc::new(
                crate::tddd::feature_declaration_adapter::FsTdddFeatureDeclarationAdapter::new(),
            ),
        );
        interactor
            .run(TypeSignalsRequest {
                items_dir: items_dir.clone(),
                track_id: track_id.clone(),
                branch,
                workspace_root: workspace_root.clone(),
                layer,
            })
            .map_err(|error| execution_failed(error.to_string()))?;

        let layers = selected_bindings
            .into_iter()
            .map(|binding| read_signal_layer(&items_dir, &track_id, binding))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(TrackTypeSignalsResult { layers })
    }
}

fn validate_items_dir_within_workspace(
    items_dir: &Path,
    workspace_root: &Path,
) -> Result<(), TrackTypeSignalsError> {
    let canonical_workspace = workspace_root.canonicalize().map_err(|error| {
        execution_failed(format!(
            "cannot resolve workspace root '{}': {error}",
            workspace_root.display()
        ))
    })?;
    let canonical_items = items_dir.canonicalize().map_err(|error| {
        execution_failed(format!(
            "cannot resolve track items directory '{}': {error}",
            items_dir.display()
        ))
    })?;
    if !canonical_items.starts_with(&canonical_workspace) {
        return Err(execution_failed(format!(
            "track items directory '{}' resolves outside workspace root '{}'; only paths under the workspace are allowed",
            items_dir.display(),
            workspace_root.display()
        )));
    }
    Ok(())
}

fn read_signal_layer(
    items_dir: &Path,
    track_id: &TrackId,
    binding: &crate::verify::tddd_layers::TdddLayerBinding,
) -> Result<TrackLayerSignalResult, TrackTypeSignalsError> {
    let signal_path = items_dir.join(track_id.as_ref()).join(binding.signal_file());
    match crate::track::symlink_guard::reject_symlinks_below(&signal_path, items_dir) {
        Ok(true) => {}
        Ok(false) => {
            return Err(execution_failed(format!(
                "type-signals file not found: {}",
                signal_path.display()
            )));
        }
        Err(error) => {
            return Err(execution_failed(format!(
                "symlink guard: cannot read type-signals file '{}': {error}",
                signal_path.display()
            )));
        }
    }
    let json = crate::capability_exec::bounded_read_utf8_file(&signal_path).map_err(|error| {
        execution_failed(format!(
            "cannot read type-signals file '{}': {error}",
            signal_path.display()
        ))
    })?;
    let document = crate::tddd::type_signals_codec::decode(&json).map_err(|error| {
        execution_failed(format!(
            "cannot decode type-signals file '{}': {error}",
            signal_path.display()
        ))
    })?;
    let mut blue = 0u32;
    let mut yellow = 0u32;
    let mut red = 0u32;
    for signal in document.signals() {
        match signal.signal() {
            ConfidenceSignal::Blue => blue = blue.saturating_add(1),
            ConfidenceSignal::Yellow => yellow = yellow.saturating_add(1),
            ConfidenceSignal::Red => red = red.saturating_add(1),
            _ => red = red.saturating_add(1),
        }
    }
    let layer = domain::tddd::LayerId::try_new(binding.layer_id().to_owned())
        .map_err(|error| execution_failed(format!("invalid TDDD layer id: {error}")))?;
    Ok(TrackLayerSignalResult::Evaluated { layer, counts: SignalCounts::new(blue, yellow, red) })
}

fn execution_failed(message: impl Into<String>) -> TrackTypeSignalsError {
    TrackTypeSignalsError::ExecutionFailed(usecase::git_workflow::DiagnosticText::new(message))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::fs;
    use std::process::Command;

    use super::*;
    use usecase::track_lifecycle::{TrackSelection, TrackWorkspaceRoot};

    fn run_git(root: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(root)
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .status()
            .expect("git command starts");
        assert!(status.success(), "git command failed: {args:?}");
    }

    fn seed_repository(root: &Path, track_id: &str) {
        fs::create_dir_all(root.join("track/items").join(track_id))
            .expect("track directory exists");
        run_git(root, &["init", "-q"]);
        run_git(root, &["checkout", "-B", &format!("track/{track_id}")]);
        run_git(root, &["commit", "--allow-empty", "-m", "init", "--no-gpg-sign"]);
    }

    fn command(root: &Path, track_id: &str) -> TrackTypeSignalsCommand {
        TrackTypeSignalsCommand {
            track: TrackSelection::Explicit(TrackId::try_new(track_id).expect("track id is valid")),
            workspace_root: TrackWorkspaceRoot::try_new(root.to_path_buf())
                .expect("workspace root is valid"),
            layer: TrackLayerSelection::All,
        }
    }

    #[test]
    fn test_system_track_type_signals_adapter_missing_rules_returns_execution_error() {
        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        seed_repository(workspace.path(), "signals-track");

        let error = match SystemTrackTypeSignalsAdapter.execute(
            TrackId::try_new("signals-track").expect("track id is valid"),
            command(workspace.path(), "signals-track"),
        ) {
            Ok(_) => panic!("missing architecture rules must fail closed"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("layer bindings load failed"));
    }

    #[test]
    fn test_system_track_type_signals_adapter_branch_read_ignores_ambient_repository_selection() {
        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        seed_repository(workspace.path(), "signals-track");
        fs::write(
            workspace.path().join("architecture-rules.json"),
            r#"{"version":2,"layers":[{"crate":"domain","tddd":{"enabled":true}}]}"#,
        )
        .expect("architecture rules are written");
        fs::write(
            workspace.path().join("track/items/signals-track/tddd-features.json"),
            r#"{"schema_version":1,"layers":{"domain":[]}}"#,
        )
        .expect("feature declaration is written");
        fs::write(
            workspace.path().join("track/items/signals-track/tddd-features-baseline.json"),
            r#"{"schema_version":1,"layers":{"domain":[]}}"#,
        )
        .expect("feature declaration snapshot is written");

        let elsewhere = tempfile::tempdir().expect("ambient repository exists");
        seed_repository(elsewhere.path(), "Bad-branch");

        let error =
            temp_env::with_var("GIT_DIR", Some(elsewhere.path().join(".git").as_os_str()), || {
                match SystemTrackTypeSignalsAdapter.execute(
                    TrackId::try_new("signals-track").expect("track id is valid"),
                    command(workspace.path(), "signals-track"),
                ) {
                    Ok(_) => panic!("incomplete fixture must not succeed"),
                    Err(error) => error,
                }
            });

        let message = error.to_string();
        assert!(
            !message.contains("Bad-branch"),
            "GIT_DIR must not supply the active-track branch: {message}"
        );
        assert!(
            !message.contains("invalid current branch"),
            "workspace branch must be read in isolation: {message}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_system_track_type_signals_adapter_preserves_original_path_for_symlink_guard() {
        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        let real_root = workspace.path().join("real");
        fs::create_dir_all(&real_root).expect("real workspace exists");
        seed_repository(&real_root, "signals-track");
        fs::write(
            real_root.join("architecture-rules.json"),
            r#"{"version":2,"layers":[{"crate":"domain","tddd":{"enabled":true}}]}"#,
        )
        .expect("architecture rules are written");
        fs::write(
            real_root.join("track/items/signals-track/tddd-features.json"),
            r#"{"schema_version":1,"layers":{"domain":[]}}"#,
        )
        .expect("feature declaration is written");
        fs::write(
            real_root.join("track/items/signals-track/tddd-features-baseline.json"),
            r#"{"schema_version":1,"layers":{"domain":[]}}"#,
        )
        .expect("feature declaration snapshot is written");

        let linked_root = workspace.path().join("linked");
        std::os::unix::fs::symlink(&real_root, &linked_root).expect("workspace symlink exists");
        let error = match SystemTrackTypeSignalsAdapter.execute(
            TrackId::try_new("signals-track").expect("track id is valid"),
            command(&linked_root, "signals-track"),
        ) {
            Ok(_) => panic!("symlinked workspace root must fail closed"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("symlink"), "error must mention symlink guard: {error}");
    }

    #[cfg(unix)]
    fn make_executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(path).expect("script metadata exists").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("script is executable");
    }

    #[cfg(unix)]
    fn minimal_rustdoc_json() -> String {
        format!(
            r#"{{
                "root": 0,
                "crate_version": null,
                "includes_private": false,
                "index": {{}},
                "paths": {{}},
                "external_crates": {{}},
                "format_version": {format_version},
                "target": {{"triple": "", "target_features": []}}
            }}"#,
            format_version = rustdoc_types::FORMAT_VERSION
        )
    }

    #[cfg(unix)]
    #[test]
    fn test_system_track_type_signals_adapter_selected_layer_persists_and_returns_counts() {
        crate::tddd::type_signals_evaluator::with_process_environment_lock(|| {
            let workspace = tempfile::tempdir().expect("temporary workspace exists");
            let root = workspace.path();
            let track_id = "signals-track";
            let track_dir = root.join("track/items").join(track_id);
            fs::create_dir_all(track_dir.join("track-placeholder"))
                .expect("track directory exists");
            fs::remove_dir(track_dir.join("track-placeholder")).expect("placeholder is removed");
            fs::create_dir_all(root.join("libs/domain/src")).expect("domain source exists");
            fs::write(
                root.join("Cargo.toml"),
                "[workspace]\nmembers = [\"libs/domain\"]\nresolver = \"2\"\n",
            )
            .expect("workspace manifest is written");
            fs::write(
                root.join("libs/domain/Cargo.toml"),
                "[package]\nname = \"domain\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
            )
            .expect("domain manifest is written");
            fs::write(root.join("libs/domain/src/lib.rs"), "").expect("domain source is written");
            fs::write(
                root.join("architecture-rules.json"),
                r#"{"version":2,"layers":[{"crate":"domain","path":"libs/domain","tddd":{"enabled":true,"catalogue_file":"domain-types.json","schema_export":{"method":"rustdoc","targets":["domain"]}}}]}"#,
            )
            .expect("architecture rules are written");
            fs::write(
                track_dir.join("domain-types.json"),
                r#"{"schema_version":5,"crate_name":"domain","layer":"domain","types":{},"traits":{},"functions":{}}"#,
            )
            .expect("catalogue is written");
            fs::write(track_dir.join("domain-types-baseline.json"), minimal_rustdoc_json())
                .expect("baseline is written");
            fs::write(
                track_dir.join("tddd-features.json"),
                r#"{"schema_version":1,"layers":{"domain":[]}}"#,
            )
            .expect("feature declaration is written");
            fs::write(
                track_dir.join("tddd-features-baseline.json"),
                r#"{"schema_version":1,"layers":{"domain":[]}}"#,
            )
            .expect("feature declaration snapshot is written");
            fs::write(root.join(".gitignore"), "target/\n").expect("gitignore is written");
            seed_repository(root, track_id);
            run_git(root, &["add", "."]);
            run_git(root, &["commit", "--quiet", "-m", "fixture", "--no-gpg-sign"]);

            let commands = tempfile::tempdir().expect("command shim directory exists");
            let rustup = commands.path().join("rustup");
            fs::write(&rustup, "#!/bin/sh\nexit 0\n").expect("rustup shim is written");
            make_executable(&rustup);
            let cargo = commands.path().join("cargo");
            fs::write(
                &cargo,
                "#!/bin/sh\nset -eu\nif [ \"$1\" = metadata ]; then\nprintf '%s\\n' \"{\\\"packages\\\":[{\\\"name\\\":\\\"domain\\\",\\\"manifest_path\\\":\\\"$PWD/libs/domain/Cargo.toml\\\",\\\"targets\\\":[{\\\"kind\\\":[\\\"lib\\\"],\\\"name\\\":\\\"domain\\\"}]}],\\\"target_directory\\\":\\\"target\\\"}\"\nexit 0\nfi\nmkdir -p \"$CARGO_TARGET_DIR/doc\"\nprintf '%s' '{\"root\":0,\"crate_version\":null,\"includes_private\":false,\"index\":{},\"paths\":{},\"external_crates\":{},\"format_version\":57,\"target\":{\"triple\":\"\",\"target_features\":[]}}' > \"$CARGO_TARGET_DIR/doc/domain.json\"\n",
            )
            .expect("cargo shim is written");
            make_executable(&cargo);
            let path = std::env::join_paths(
                std::iter::once(commands.path().to_path_buf())
                    .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())),
            )
            .expect("test PATH is valid");
            let target_dir = root.join("target");
            let mut command = command(root, track_id);
            command.layer = TrackLayerSelection::One(
                domain::tddd::LayerId::try_new("domain").expect("layer id is valid"),
            );
            let result = temp_env::with_vars(
                [
                    ("PATH", Some(path.as_os_str())),
                    ("CARGO_TARGET_DIR", Some(target_dir.as_os_str())),
                ],
                || {
                    SystemTrackTypeSignalsAdapter
                        .execute(TrackId::try_new(track_id).expect("track id is valid"), command)
                },
            )
            .expect("selected-layer type-signals must succeed");

            assert_eq!(result.layers.len(), 1, "layer filter must evaluate only domain");
            match result.layers.as_slice() {
                [TrackLayerSignalResult::Evaluated { layer, counts }] => {
                    assert_eq!(layer.as_ref(), "domain");
                    assert_eq!(counts.blue(), 0);
                    assert_eq!(counts.yellow(), 0);
                    assert_eq!(counts.red(), 0);
                }
                other => panic!("expected one evaluated domain layer, got {other:?}"),
            }

            let signal_path = track_dir.join("domain-type-signals.json");
            assert!(
                signal_path.is_file(),
                "adapter must persist the selected-layer signal artifact"
            );
            let persisted =
                fs::read_to_string(signal_path).expect("persisted signal file is readable");
            assert!(persisted.contains("\"signals\": []"));
        });
    }
}
