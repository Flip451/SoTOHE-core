//! `sotp track catalogue-impl-signals` — diagnose SoT Chain ③ (catalogue ↔ implementation).
//!
//! Thin CLI adapter: delegates all orchestration to the composition root in `cli_composition`.

use std::path::PathBuf;
use std::process::ExitCode;

use cli_composition::TrackCompositionRoot;
use cli_driver::adr_baseline::TrackIdInput;
use cli_driver::track_tddd::{
    TrackLayerInput, TrackTdddCatalogueImplSignalsInput, TrackTdddInput, TrackWorkspaceRootInput,
};

use crate::CliError;

/// Execute the `track catalogue-impl-signals` command.
///
/// # Errors
///
/// Returns `CliError` when the underlying Track TDDD boundary fails.
pub fn execute_catalogue_impl_signals(
    track_id: String,
    workspace_root: PathBuf,
    layer: Option<String>,
) -> Result<ExitCode, CliError> {
    let track_id = track_id
        .parse::<TrackIdInput>()
        .map_err(|error| CliError::Message(format!("invalid track id: {error}")))?;
    let workspace_root =
        TrackWorkspaceRootInput::try_from(workspace_root).map_err(CliError::Message)?;
    let layer = layer
        .map(TrackLayerInput::try_from)
        .transpose()
        .map_err(|error| CliError::Message(error.to_string()))?;
    let outcome = TrackCompositionRoot::new().track_tddd_driver().handle(
        TrackTdddInput::CatalogueImplSignals(TrackTdddCatalogueImplSignalsInput {
            track_id: Some(track_id),
            workspace_root,
            layer,
        }),
    );
    super::emit_driver_outcome!(outcome, &mut std::io::stdout(), &mut std::io::stderr())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    #[cfg(unix)]
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    #[cfg(unix)]
    use std::path::Path;

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
    fn make_executable(path: &Path) {
        let mut permissions = fs::metadata(path).expect("script metadata exists").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("script is executable");
    }

    #[cfg(unix)]
    fn write_catalogue_impl_signals_fixture(
        root: &Path,
        track_id: &str,
        target_directory: &Path,
    ) -> Result<(), String> {
        let track_dir = root.join("track/items").join(track_id);
        fs::create_dir_all(root.join("libs/domain/src")).map_err(|error| error.to_string())?;
        fs::create_dir_all(&track_dir).map_err(|error| error.to_string())?;
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"libs/domain\"]\nresolver = \"2\"\n",
        )
        .map_err(|error| error.to_string())?;
        fs::write(
            root.join("libs/domain/Cargo.toml"),
            "[package]\nname = \"domain\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .map_err(|error| error.to_string())?;
        fs::write(root.join("libs/domain/src/lib.rs"), "").map_err(|error| error.to_string())?;
        fs::write(
            root.join("architecture-rules.json"),
            r#"{"version":2,"layers":[{"crate":"domain","path":"libs/domain","tddd":{"enabled":true,"catalogue_file":"domain-types.json","schema_export":{"method":"rustdoc","targets":["domain"]}}}]}"#,
        )
        .map_err(|error| error.to_string())?;
        fs::write(
            track_dir.join("domain-types.json"),
            r#"{"schema_version":5,"crate_name":"domain","layer":"domain","types":{},"traits":{},"functions":{}}"#,
        )
        .map_err(|error| error.to_string())?;
        fs::write(track_dir.join("domain-types-baseline.json"), minimal_rustdoc_json())
            .map_err(|error| error.to_string())?;
        fs::write(
            track_dir.join("tddd-features.json"),
            r#"{"schema_version":1,"layers":{"domain":[]}}"#,
        )
        .map_err(|error| error.to_string())?;
        fs::write(
            track_dir.join("tddd-features-baseline.json"),
            r#"{"schema_version":1,"layers":{"domain":[]}}"#,
        )
        .map_err(|error| error.to_string())?;
        fs::create_dir_all(target_directory).map_err(|error| error.to_string())?;
        let sentinel = root.join("target-sentinel");
        fs::write(&sentinel, "target input must remain excluded")
            .map_err(|error| error.to_string())?;
        std::os::unix::fs::symlink(&sentinel, target_directory.join("untrusted-input"))
            .map_err(|error| error.to_string())?;
        Ok(())
    }

    #[cfg(unix)]
    fn cargo_and_rustup_shims(
        root: &Path,
        target_directory: &Path,
    ) -> Result<(tempfile::TempDir, std::ffi::OsString, String), String> {
        let commands = tempfile::tempdir().map_err(|error| error.to_string())?;
        let rustup = commands.path().join("rustup");
        fs::write(
            &rustup,
            "#!/bin/sh\nif [ \"$1\" = which ]; then\nprintf '%s\\n' /bin/sh\nfi\nexit 0\n",
        )
        .map_err(|error| error.to_string())?;
        make_executable(&rustup);

        let manifest_path = root.join("libs/domain/Cargo.toml");
        let manifest_path = manifest_path
            .to_str()
            .ok_or_else(|| "fixture manifest path is not valid UTF-8".to_owned())?;
        let target_directory = target_directory
            .to_str()
            .ok_or_else(|| "fixture target directory is not valid UTF-8".to_owned())?;
        let metadata = format!(
            r#"{{"packages":[{{"name":"domain","manifest_path":{},"targets":[{{"kind":["lib"],"name":"domain"}}]}}],"target_directory":{}}}"#,
            serde_json::to_string(manifest_path).map_err(|error| error.to_string())?,
            serde_json::to_string(target_directory).map_err(|error| error.to_string())?
        );
        let cargo = commands.path().join("cargo");
        fs::write(
            &cargo,
            format!(
                r#"#!/bin/sh
set -eu
if [ "$1" = metadata ]; then
printf '%s\n' "$SOTP_TEST_CARGO_METADATA"
exit 0
fi
mkdir -p "$CARGO_TARGET_DIR/doc"
printf '%s' '{rustdoc_json}' > "$CARGO_TARGET_DIR/doc/domain.json"
"#,
                rustdoc_json = minimal_rustdoc_json()
            ),
        )
        .map_err(|error| error.to_string())?;
        make_executable(&cargo);

        let path = std::env::join_paths(
            std::iter::once(commands.path().to_path_buf())
                .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())),
        )
        .map_err(|error| error.to_string())?;
        Ok((commands, path, metadata))
    }

    /// Symlinked workspace_root must be rejected before any I/O.
    #[cfg(unix)]
    #[test]
    fn test_symlinked_workspace_root_is_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let real_dir = tmp.path().join("real");
        std::fs::create_dir_all(&real_dir).unwrap();
        let link_dir = tmp.path().join("link");
        std::os::unix::fs::symlink(&real_dir, &link_dir).unwrap();

        let result =
            execute_catalogue_impl_signals("test-track-2026-01-01".to_owned(), link_dir, None);
        assert!(result.is_err(), "symlinked workspace_root must return Err");
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("symlink guard"), "error message must mention symlink guard: {msg}");
    }

    /// An invalid track ID must be rejected by the interactor's validation.
    #[test]
    fn test_invalid_track_id_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        // No architecture-rules.json — but invalid track ID fails before file I/O.
        let result = execute_catalogue_impl_signals(
            "bad track id!!".to_owned(),
            tmp.path().to_path_buf(),
            None,
        );
        assert!(result.is_err(), "invalid track ID must return Err");
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("invalid track id") || msg.contains("invalid track ID"),
            "error must mention invalid track id: {msg}"
        );
    }

    /// Missing architecture-rules.json at workspace_root must produce an error
    /// (fail-closed: the layer-bindings port cannot enumerate TDDD layers without it).
    #[test]
    fn test_missing_architecture_rules_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        // workspace_root is a real directory (not a symlink) but has no architecture-rules.json.
        let result = execute_catalogue_impl_signals(
            "test-track-2026-01-01".to_owned(),
            tmp.path().to_path_buf(),
            None,
        );
        assert!(result.is_err(), "missing architecture-rules.json must return Err");
    }

    /// Symlinked `track/items` directory must be rejected by the items_dir guard.
    #[cfg(unix)]
    #[test]
    fn test_symlinked_items_dir_is_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        let real_items = tmp.path().join("real_items");
        std::fs::create_dir_all(&real_items).unwrap();
        // Create workspace_root/track/ and symlink items → real_items.
        let track_dir = tmp.path().join("track");
        std::fs::create_dir_all(&track_dir).unwrap();
        let items_link = track_dir.join("items");
        std::os::unix::fs::symlink(&real_items, &items_link).unwrap();

        let result = execute_catalogue_impl_signals(
            "test-track-2026-01-01".to_owned(),
            tmp.path().to_path_buf(),
            None,
        );
        assert!(result.is_err(), "symlinked track/items must return Err");
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("symlink guard"), "error message must mention symlink guard: {msg}");
    }

    #[cfg(unix)]
    #[test]
    fn test_catalogue_impl_signals_excludes_target_for_default_relative_and_absolute_roots()
    -> Result<(), String> {
        let _guard = crate::commands::track::test_support::process_env_lock()
            .lock()
            .map_err(|_| "process environment lock is poisoned".to_owned())?;
        let workspace = tempfile::tempdir().map_err(|error| error.to_string())?;
        let root = workspace.path().join("workspace'root");
        fs::create_dir_all(&root).map_err(|error| error.to_string())?;
        let target_directory = root.join("target");
        let track_id = "signals-track-2026-01-01";
        write_catalogue_impl_signals_fixture(&root, track_id, &target_directory)?;
        let (_shims, path, metadata) = cargo_and_rustup_shims(&root, &target_directory)?;
        let home = tempfile::tempdir().map_err(|error| error.to_string())?;
        let relative_root = root
            .file_name()
            .ok_or_else(|| "fixture root has no final component".to_owned())?
            .to_owned();
        let parent = root.parent().ok_or_else(|| "fixture root has no parent".to_owned())?;

        temp_env::with_vars(
            [
                ("PATH", Some(path.as_os_str())),
                ("SOTP_TEST_CARGO_METADATA", Some(std::ffi::OsStr::new(metadata.as_str()))),
                ("CARGO_HOME", None),
                ("CARGO_TARGET_DIR", Some(target_directory.as_os_str())),
                ("HOME", Some(home.path().as_os_str())),
                ("RUSTC", None),
                ("RUSTDOC", None),
                ("RUSTC_WRAPPER", None),
                ("RUSTC_WORKSPACE_WRAPPER", None),
            ],
            || {
                let default_root = crate::commands::track::test_support::run_in_dir(&root, || {
                    execute_catalogue_impl_signals(
                        track_id.to_owned(),
                        PathBuf::from("."),
                        Some("domain".to_owned()),
                    )
                });
                assert!(
                    default_root.is_ok(),
                    "default workspace root must succeed: {default_root:?}"
                );
                assert_eq!(default_root.unwrap(), ExitCode::SUCCESS);

                let explicit_relative_root =
                    crate::commands::track::test_support::run_in_dir(parent, || {
                        execute_catalogue_impl_signals(
                            track_id.to_owned(),
                            PathBuf::from(relative_root.clone()),
                            Some("domain".to_owned()),
                        )
                    });
                assert!(
                    explicit_relative_root.is_ok(),
                    "explicit relative workspace root must succeed: {explicit_relative_root:?}"
                );
                assert_eq!(explicit_relative_root.unwrap(), ExitCode::SUCCESS);

                let absolute_root =
                    crate::commands::track::test_support::run_in_dir(parent, || {
                        execute_catalogue_impl_signals(
                            track_id.to_owned(),
                            root.to_path_buf(),
                            Some("domain".to_owned()),
                        )
                    });
                assert!(
                    absolute_root.is_ok(),
                    "absolute workspace root must succeed: {absolute_root:?}"
                );
                assert_eq!(absolute_root.unwrap(), ExitCode::SUCCESS);
            },
        );
        Ok(())
    }

    #[test]
    fn test_execute_catalogue_impl_signals_success_report_emits_stdout_and_zero_exit() {
        let outcome = cli_driver::CommandOutcome::success(Some("catalogue report".to_owned()));
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let result = super::super::emit_driver_outcome!(outcome, &mut stdout, &mut stderr);

        assert_eq!(result.unwrap(), ExitCode::SUCCESS);
        assert_eq!(String::from_utf8(stdout).unwrap(), "catalogue report\n");
        assert!(stderr.is_empty());
    }

    #[test]
    fn test_execute_catalogue_impl_signals_red_report_emits_stdout_and_failure_exit() {
        let outcome = cli_driver::CommandOutcome {
            stdout: Some("## Layer: usecase\n🔴 Red".to_owned()),
            stderr: None,
            exit_code: 1,
        };
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let result = super::super::emit_driver_outcome!(outcome, &mut stdout, &mut stderr);

        assert_eq!(result.unwrap(), ExitCode::FAILURE);
        assert!(String::from_utf8(stdout).unwrap().contains("🔴 Red"));
        assert!(stderr.is_empty());
    }
}
