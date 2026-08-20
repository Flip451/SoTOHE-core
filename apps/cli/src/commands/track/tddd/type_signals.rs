//! sotp track type-signals — evaluate per-layer implementation signals.

use std::path::PathBuf;
use std::process::ExitCode;

use cli_composition::TrackCompositionRoot;
use cli_driver::adr_baseline::TrackIdInput;
use cli_driver::track_tddd::{
    TrackLayerInput, TrackTdddInput, TrackTdddTypeSignalsInput, TrackWorkspaceRootInput,
};

use crate::CliError;

/// Execute the track type-signals command.
///
/// The type-signals operation writes the per-layer evaluation artifacts while
/// preserving the legacy empty stdout/stderr success contract.
pub fn execute_type_signals(
    track_id: Option<String>,
    workspace_root: PathBuf,
    layer: Option<String>,
) -> Result<ExitCode, CliError> {
    let track_id = track_id
        .map(|value| value.parse::<TrackIdInput>())
        .transpose()
        .map_err(|error| CliError::Message(format!("invalid track id: {error}")))?;
    let workspace_root =
        TrackWorkspaceRootInput::try_from(workspace_root).map_err(CliError::Message)?;
    let layer = layer
        .map(TrackLayerInput::try_from)
        .transpose()
        .map_err(|error| CliError::Message(error.to_string()))?;
    let outcome = TrackCompositionRoot::new().track_tddd_driver().handle(
        TrackTdddInput::TypeSignals(TrackTdddTypeSignalsInput { track_id, workspace_root, layer }),
    );
    super::emit_driver_outcome!(outcome, &mut std::io::stdout(), &mut std::io::stderr())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::fs;
    use std::process::Command;

    use super::*;

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
    fn make_executable(path: &std::path::Path) {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(path).expect("script metadata exists").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("script is executable");
    }

    fn seed_track_branch(root: &std::path::Path, track_id: &str) {
        let track_dir = root.join("track/items").join(track_id);
        fs::create_dir_all(&track_dir).expect("track directory exists");
        run_git(root, &["init", "-q"]);
        run_git(root, &["checkout", "-B", &format!("track/{track_id}")]);
        run_git(root, &["commit", "--allow-empty", "-m", "init", "--no-gpg-sign"]);
    }

    fn run_git(root: &std::path::Path, args: &[&str]) {
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

    fn handle_domain_type_signals(
        root: &std::path::Path,
        track_id: &str,
    ) -> cli_driver::CommandOutcome {
        let track_id = track_id.parse::<TrackIdInput>().expect("track id is valid");
        let workspace_root =
            TrackWorkspaceRootInput::try_from(root.to_path_buf()).expect("workspace is valid");
        let layer = TrackLayerInput::try_from("domain".to_owned()).expect("layer is valid");
        TrackCompositionRoot::new().track_tddd_driver().handle(TrackTdddInput::TypeSignals(
            TrackTdddTypeSignalsInput {
                track_id: Some(track_id),
                workspace_root,
                layer: Some(layer),
            },
        ))
    }

    #[cfg(unix)]
    fn write_type_signals_success_fixture(
        root: &std::path::Path,
        track_id: &str,
    ) -> std::path::PathBuf {
        let track_dir = root.join("track/items").join(track_id);
        fs::create_dir_all(root.join("libs/domain/src")).expect("domain source exists");
        fs::create_dir_all(&track_dir).expect("track directory exists");
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
        run_git(root, &["init", "-q"]);
        run_git(root, &["checkout", "-B", &format!("track/{track_id}")]);
        run_git(root, &["add", "."]);
        run_git(root, &["commit", "--quiet", "-m", "fixture", "--no-gpg-sign"]);
        track_dir
    }

    #[cfg(unix)]
    fn cargo_and_rustup_shims() -> (tempfile::TempDir, std::ffi::OsString) {
        let commands = tempfile::tempdir().expect("command shim directory exists");
        let rustup = commands.path().join("rustup");
        fs::write(&rustup, "#!/bin/sh\nexit 0\n").expect("rustup shim is written");
        make_executable(&rustup);
        let cargo = commands.path().join("cargo");
        fs::write(
            &cargo,
            "#!/bin/sh\nset -eu\nif [ \"$1\" = metadata ]; then\nprintf '%s\\n' '{\"packages\":[{\"name\":\"domain\",\"targets\":[{\"kind\":[\"lib\"],\"name\":\"domain\"}]}],\"target_directory\":\"target\"}'\nexit 0\nfi\nmkdir -p \"$CARGO_TARGET_DIR/doc\"\nprintf '%s' '{\"root\":0,\"crate_version\":null,\"includes_private\":false,\"index\":{},\"paths\":{},\"external_crates\":{},\"format_version\":57,\"target\":{\"triple\":\"\",\"target_features\":[]}}' > \"$CARGO_TARGET_DIR/doc/domain.json\"\n",
        )
        .expect("cargo shim is written");
        make_executable(&cargo);
        let path = std::env::join_paths(
            std::iter::once(commands.path().to_path_buf())
                .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())),
        )
        .expect("test PATH is valid");
        (commands, path)
    }

    #[test]
    fn test_execute_type_signals_rejects_invalid_track_id_before_execution() {
        let result =
            execute_type_signals(Some("../escape".to_owned()), PathBuf::from("workspace"), None);

        assert!(result.is_err(), "invalid track id must be rejected");
        assert!(result.unwrap_err().to_string().contains("invalid track id"));
    }

    #[test]
    fn test_track_type_signals_call_site_preserves_failure_contract_without_persisting() {
        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        let track_id = "signals-track".to_owned();
        seed_track_branch(workspace.path(), &track_id);
        let argv_workspace = workspace.path().to_path_buf();
        let argv_track_id = Some(track_id.clone());
        let argv_layer = Some("domain".to_owned());

        let (result, captured_stderr) =
            crate::commands::track::test_support::capture_stderr(|| {
                execute_type_signals(
                    argv_track_id.clone(),
                    argv_workspace.clone(),
                    argv_layer.clone(),
                )
            });

        assert!(result.is_err(), "missing architecture rules must fail closed");
        let message = result.unwrap_err().to_string();
        assert!(message.contains("layer bindings load failed"));
        assert!(
            captured_stderr.is_empty(),
            "failure is returned as CliError, not process stderr: {captured_stderr:?}"
        );
        assert_eq!(argv_workspace, workspace.path());
        assert_eq!(argv_track_id.as_deref(), Some("signals-track"));
        assert_eq!(argv_layer.as_deref(), Some("domain"));
        let outcome = handle_domain_type_signals(workspace.path(), &track_id);
        assert_ne!(outcome.exit_code, 0, "failed type-signals must keep a non-zero exit");
        assert_eq!(outcome.stdout, None, "failed type-signals must not write stdout");
        assert!(
            outcome
                .stderr
                .as_deref()
                .is_some_and(|stderr| stderr.contains("layer bindings load failed")),
            "failed type-signals must keep the recovery hint on stderr: {:?}",
            outcome.stderr
        );
        assert!(
            !workspace.path().join("track/items/signals-track/domain-type-signals.json").exists(),
            "a failed type-signals call must not persist an artifact"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_track_type_signals_call_site_persists_signal_artifact_and_returns_success() {
        let _guard = crate::commands::track::test_support::process_env_lock().lock().unwrap();
        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        let root = workspace.path();
        let track_id = "signals-track";
        let track_dir = write_type_signals_success_fixture(root, track_id);
        let (_shims, path) = cargo_and_rustup_shims();
        let target_dir = root.join("target");
        let argv_workspace = root.to_path_buf();
        let argv_layer = Some("domain".to_owned());
        let ((exit, outcome), captured_stderr) =
            crate::commands::track::test_support::capture_stderr(|| {
                temp_env::with_vars(
                    [
                        ("PATH", Some(path.as_os_str())),
                        ("CARGO_TARGET_DIR", Some(target_dir.as_os_str())),
                    ],
                    || {
                        let exit = execute_type_signals(
                            Some(track_id.to_owned()),
                            argv_workspace.clone(),
                            argv_layer.clone(),
                        );
                        let outcome = handle_domain_type_signals(root, track_id);
                        (exit, outcome)
                    },
                )
            });
        let exit = exit.expect("type-signals call site succeeds");

        assert_eq!(exit, ExitCode::SUCCESS);
        assert!(
            captured_stderr.is_empty(),
            "successful type-signals must not write process stderr: {captured_stderr:?}"
        );
        assert_eq!(argv_workspace, root);
        assert_eq!(argv_layer.as_deref(), Some("domain"));
        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stderr, None, "successful type-signals must not write stderr");
        assert_eq!(outcome.stdout, None, "successful type-signals keeps empty stdout");
        let signal_path = track_dir.join("domain-type-signals.json");
        assert!(signal_path.is_file(), "type-signals operation must persist its artifact");
        let persisted = fs::read_to_string(signal_path).expect("persisted signal file is readable");
        assert!(persisted.contains("\"signals\": []"));
    }
}
