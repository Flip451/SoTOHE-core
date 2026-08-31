//! Integration tests for `RustdocSchemaExporter`.
//!
//! These tests require nightly toolchain and are marked `#[ignore]` by default.
//! Run with: `cargo test --test '*' -- --ignored` or `cargo nextest run --run-ignored ignored-only`

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use std::collections::BTreeSet;

    use domain::FreeText;
    use domain::schema::{SchemaExporter, TypeKind};
    use domain::tddd::{CargoFeatureName, catalogue_v2::CrateName};
    use serde_json::Value;

    use crate::schema_export::RustdocSchemaExporter;
    use crate::schema_export_codec;

    fn workspace_root() -> std::path::PathBuf {
        let output = std::process::Command::new("cargo")
            .args(["locate-project", "--workspace", "--message-format", "plain"])
            .output()
            .unwrap();
        let manifest = String::from_utf8_lossy(&output.stdout);
        std::path::PathBuf::from(manifest.trim()).parent().unwrap().to_owned()
    }

    #[test]
    fn test_bin_target_resolution_canonicalizes_rustdoc_root_path_for_catalogue() {
        let resolution = crate::schema_export::bin_target::resolve_rustdoc_root_name(
            &workspace_root(),
            &CrateName::new("cli").unwrap(),
        )
        .unwrap();
        let path = vec![
            resolution.rustdoc_root_name().as_str().to_owned(),
            "commands".to_owned(),
            "run".to_owned(),
        ];

        assert_eq!(
            resolution.canonicalize_rustdoc_path(&path),
            vec!["cli".to_owned(), "commands".to_owned(), "run".to_owned()]
        );
    }

    #[test]
    #[ignore = "requires nightly toolchain"]
    fn export_domain_crate_contains_known_types() {
        let exporter = RustdocSchemaExporter::new(workspace_root());
        let schema = exporter.export("domain").unwrap();

        assert_eq!(schema.crate_name(), "domain");
        assert!(!schema.types().is_empty(), "expected types to be non-empty");

        // Check for well-known domain types
        let type_names: Vec<&str> = schema.types().iter().map(|t| t.name()).collect();
        assert!(
            type_names.contains(&"TrackStatus"),
            "expected TrackStatus in types, got: {type_names:?}"
        );
        assert!(
            type_names.contains(&"TaskStatus"),
            "expected TaskStatus in types, got: {type_names:?}"
        );

        // TrackStatus should be an enum
        let track_status = schema.types().iter().find(|t| t.name() == "TrackStatus").unwrap();
        assert_eq!(track_status.kind(), &TypeKind::Enum);
        assert!(!track_status.members().is_empty(), "expected TrackStatus to have variants");
    }

    #[test]
    #[ignore = "requires nightly toolchain"]
    fn export_domain_crate_contains_traits() {
        let exporter = RustdocSchemaExporter::new(workspace_root());
        let schema = exporter.export("domain").unwrap();

        assert!(!schema.traits().is_empty(), "expected traits to be non-empty");

        let trait_names: Vec<&str> = schema.traits().iter().map(|t| t.name()).collect();
        assert!(
            trait_names.contains(&"TrackReader"),
            "expected TrackReader in traits, got: {trait_names:?}"
        );
    }

    #[test]
    #[ignore = "requires nightly toolchain"]
    fn export_domain_crate_has_impls() {
        let exporter = RustdocSchemaExporter::new(workspace_root());
        let schema = exporter.export("domain").unwrap();

        assert!(!schema.impls().is_empty(), "expected impls to be non-empty");
    }

    #[test]
    #[ignore = "requires nightly toolchain"]
    fn test_export_infrastructure_with_semantic_dup_exposes_catalogued_public_surface() {
        let exporter = RustdocSchemaExporter::new(workspace_root());
        let crate_name = CrateName::new("infrastructure".to_owned()).unwrap();
        let features = [CargoFeatureName::try_new("semantic-dup".to_owned()).unwrap()];

        let json_bytes =
            exporter.export_rustdoc_json_with_features(&crate_name, &features).unwrap();
        let document: Value = serde_json::from_slice(&json_bytes).unwrap();
        let item_names = document["index"]
            .as_object()
            .unwrap()
            .values()
            .filter_map(|item| item["name"].as_str())
            .collect::<BTreeSet<_>>();

        for expected in [
            "CodeFragmentExtractorAdapter",
            "ExtractError",
            "FastEmbedAdapter",
            "LanceDbSemanticIndexAdapter",
            "NoopSemanticIndexPort",
            "NullInsertIndexProxy",
            "PersistentIndexLock",
            "PersistentIndexLockError",
            "extract_code_fragments",
            "acquire_persistent_index_lock",
            "persistent_index_lock_path",
        ] {
            assert!(
                item_names.contains(expected),
                "semantic-dup rustdoc surface must include {expected}; found: {item_names:?}"
            );
        }
    }

    #[test]
    #[ignore = "requires nightly toolchain"]
    fn export_schema_encode_produces_parseable_json() {
        let exporter = RustdocSchemaExporter::new(workspace_root());
        let schema = exporter.export("domain").unwrap();

        let json = schema_export_codec::encode(&schema, false).unwrap();
        assert!(!json.is_empty());
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["crate_name"], "domain");
    }

    #[test]
    fn export_nonexistent_crate_returns_error() {
        let exporter = RustdocSchemaExporter::new(workspace_root());
        let result = exporter.export("nonexistent-crate-xyz");

        assert!(result.is_err(), "expected error for nonexistent crate");
    }

    #[test]
    #[ignore = "requires nightly toolchain"]
    fn export_types_are_sorted_by_name() {
        let exporter = RustdocSchemaExporter::new(workspace_root());
        let schema = exporter.export("domain").unwrap();

        let names: Vec<&str> = schema.types().iter().map(|t| t.name()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted, "types should be sorted by name");
    }

    #[cfg(unix)]
    fn decode_fixture_rustdoc(
        bytes: &[u8],
    ) -> Result<rustdoc_types::Crate, domain::tddd::catalogue_v2::RustdocCratePortError> {
        serde_json::from_slice(bytes).map_err(|error| {
            domain::tddd::catalogue_v2::RustdocCratePortError::ParseFailed {
                crate_name: CrateName::new("fixture").unwrap(),
                reason: FreeText::new(error.to_string()),
            }
        })
    }

    #[cfg(unix)]
    #[test]
    fn test_rustdoc_exporter_expected_path_and_snapshot_capture_succeeds() {
        use std::os::unix::fs::PermissionsExt as _;

        crate::tddd::type_signals_evaluator::with_process_environment_lock(|| {
            let workspace = tempfile::tempdir().unwrap();
            let commands = workspace.path().join("commands");
            std::fs::create_dir_all(&commands).unwrap();
            let rustup = commands.join("rustup");
            std::fs::write(&rustup, "#!/bin/sh\nexit 0\n").unwrap();
            std::fs::set_permissions(&rustup, std::fs::Permissions::from_mode(0o755)).unwrap();
            let rustdoc_json = format!(
                r#"{{"root":0,"crate_version":"generation-a","includes_private":false,"index":{{}},"paths":{{}},"external_crates":{{}},"format_version":{},"target":{{"triple":"","target_features":[]}}}}"#,
                rustdoc_types::FORMAT_VERSION
            );
            let cargo = commands.join("cargo");
            std::fs::write(
                &cargo,
                format!(
                    r#"#!/bin/sh
if [ "$1" = "metadata" ]; then
  printf '%s\n' '{{"packages":[{{"name":"fixture","targets":[{{"kind":["lib"],"name":"fixture"}}]}}]}}'
  exit 0
fi
mkdir -p "$CARGO_TARGET_DIR/doc"
printf '%s\n' '{rustdoc_json}' > "$CARGO_TARGET_DIR/doc/fixture.json"
"#
                ),
            )
            .unwrap();
            std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o755)).unwrap();
            let target_directory = workspace.path().join("cargo-target");
            let mut path_entries = vec![commands];
            path_entries
                .extend(std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()));
            let path = std::env::join_paths(path_entries).unwrap();
            temp_env::with_vars(
                [
                    ("CARGO_TARGET_DIR", Some(target_directory.as_os_str())),
                    ("PATH", Some(path.as_os_str())),
                ],
                || {
                    let exporter = RustdocSchemaExporter::new(workspace.path().to_path_buf());
                    let crate_name = CrateName::new("fixture".to_owned()).unwrap();
                    let snapshot = exporter
                        .capture_rustdoc_snapshot(&crate_name, &[], decode_fixture_rustdoc)
                        .unwrap();
                    assert_eq!(snapshot.execution_identity().crate_name(), &crate_name);
                    assert_eq!(
                        snapshot.crate_data().crate_version.as_deref(),
                        Some("generation-a")
                    );
                },
            );
        });
    }

    #[cfg(unix)]
    #[test]
    fn test_exporter_capture_waits_on_held_exclusive_lock() {
        use std::os::unix::fs::PermissionsExt as _;
        use std::time::Duration;

        crate::tddd::type_signals_evaluator::with_process_environment_lock(|| {
            let workspace = tempfile::tempdir().unwrap();
            std::fs::write(
                workspace.path().join("Cargo.toml"),
                "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
            )
            .unwrap();
            std::fs::write(workspace.path().join("Cargo.lock"), "version = 4\n").unwrap();
            std::fs::create_dir_all(workspace.path().join("src")).unwrap();
            std::fs::write(workspace.path().join("src/lib.rs"), "pub struct Fixture;\n").unwrap();
            let commands = workspace.path().join("commands");
            std::fs::create_dir_all(&commands).unwrap();
            let rustup = commands.join("rustup");
            std::fs::write(&rustup, "#!/bin/sh\nexit 0\n").unwrap();
            std::fs::set_permissions(&rustup, std::fs::Permissions::from_mode(0o755)).unwrap();
            let rustdoc_json = format!(
                r#"{{"root":0,"crate_version":"generation-a","includes_private":false,"index":{{}},"paths":{{}},"external_crates":{{}},"format_version":{},"target":{{"triple":"","target_features":[]}}}}"#,
                rustdoc_types::FORMAT_VERSION
            );
            let cargo = commands.join("cargo");
            std::fs::write(
                &cargo,
                format!(
                    r#"#!/bin/sh
if [ "$1" = "metadata" ]; then
  printf '%s\n' '{{"packages":[{{"name":"fixture","targets":[{{"kind":["lib"],"name":"fixture"}}]}}]}}'
  exit 0
fi
mkdir -p "$CARGO_TARGET_DIR/doc"
printf '%s\n' '{json}' > "$CARGO_TARGET_DIR/doc/fixture.json"
"#,
                    json = rustdoc_json
                ),
            )
            .unwrap();
            std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o755)).unwrap();
            let cargo_target = workspace.path().join("cargo-target");
            let mut path_entries = vec![commands];
            path_entries
                .extend(std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()));
            let path = std::env::join_paths(path_entries).unwrap();
            temp_env::with_vars(
                [
                    ("CARGO_TARGET_DIR", Some(cargo_target.as_os_str())),
                    ("PATH", Some(path.as_os_str())),
                ],
                || {
                    let exporter = RustdocSchemaExporter::new(workspace.path().to_path_buf());
                    let crate_name = CrateName::new("fixture".to_owned()).unwrap();
                    let (identity, _) =
                        exporter.rustdoc_execution_identity(&crate_name, &[]).unwrap();
                    let exclusive = identity.target_directory().as_path().to_path_buf();
                    let held =
                        crate::tddd::rustdoc_output_lock::RustdocOutputLock::acquire(&exclusive)
                            .unwrap();
                    let exporter_for_contender =
                        RustdocSchemaExporter::new(workspace.path().to_path_buf());
                    let crate_for_contender = crate_name.clone();
                    let contender = std::thread::spawn(move || {
                        exporter_for_contender.capture_rustdoc_snapshot(
                            &crate_for_contender,
                            &[],
                            decode_fixture_rustdoc,
                        )
                    });
                    std::thread::sleep(Duration::from_millis(80));
                    assert!(
                        !contender.is_finished(),
                        "RustdocSchemaExporter must wait on the exclusive lock through capture"
                    );
                    drop(held);
                    let snapshot = contender.join().unwrap().unwrap();
                    assert_eq!(
                        snapshot.crate_data().crate_version.as_deref(),
                        Some("generation-a")
                    );
                },
            );
        });
    }

    #[cfg(unix)]
    #[test]
    fn test_exporter_holds_lock_before_expected_path_selection() {
        use std::os::unix::fs::PermissionsExt as _;
        use std::sync::{Arc, Barrier};
        use std::time::Duration;

        crate::tddd::type_signals_evaluator::with_process_environment_lock(|| {
            let workspace = tempfile::tempdir().unwrap();
            let commands = workspace.path().join("commands");
            std::fs::create_dir_all(&commands).unwrap();
            let rustup = commands.join("rustup");
            std::fs::write(&rustup, "#!/bin/sh\nexit 0\n").unwrap();
            std::fs::set_permissions(&rustup, std::fs::Permissions::from_mode(0o755)).unwrap();
            let target_directory = workspace.path().join("cargo-target");
            let first_json = format!(
                r#"{{"root":0,"crate_version":"generation-a","includes_private":false,"index":{{}},"paths":{{}},"external_crates":{{}},"format_version":{},"target":{{"triple":"","target_features":[]}}}}"#,
                rustdoc_types::FORMAT_VERSION
            );
            let cargo = commands.join("cargo");
            std::fs::write(
                &cargo,
                format!(
                    r#"#!/bin/sh
if [ "$1" = "metadata" ]; then
  printf '%s\n' '{{"packages":[{{"name":"fixture","targets":[{{"kind":["lib"],"name":"fixture"}}]}}]}}'
  exit 0
fi
mkdir -p "$CARGO_TARGET_DIR/doc"
printf '%s\n' '{first_json}' > "$CARGO_TARGET_DIR/doc/fixture.json"
"#
                ),
            )
            .unwrap();
            std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o755)).unwrap();
            let mut path_entries = vec![commands];
            path_entries
                .extend(std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()));
            let path = std::env::join_paths(path_entries).unwrap();

            temp_env::with_vars(
                [
                    ("CARGO_TARGET_DIR", Some(target_directory.as_os_str())),
                    ("PATH", Some(path.as_os_str())),
                ],
                || {
                    let crate_name = CrateName::new("fixture").unwrap();
                    let identity_exporter =
                        RustdocSchemaExporter::new(workspace.path().to_path_buf());
                    let (identity, _) =
                        identity_exporter.rustdoc_execution_identity(&crate_name, &[]).unwrap();
                    let exclusive = identity.target_directory().as_path().to_path_buf();
                    let before_selection = Arc::new(Barrier::new(2));
                    let release_selection = Arc::new(Barrier::new(2));
                    let before_selection_for_hook = Arc::clone(&before_selection);
                    let release_selection_for_hook = Arc::clone(&release_selection);
                    let first_exporter = RustdocSchemaExporter::with_before_expected_path_selection(
                        workspace.path().to_path_buf(),
                        Arc::new(move || {
                            before_selection_for_hook.wait();
                            release_selection_for_hook.wait();
                        }),
                    );
                    let first_crate = crate_name.clone();
                    let first = std::thread::spawn(move || {
                        first_exporter.capture_rustdoc_snapshot(
                            &first_crate,
                            &[],
                            decode_fixture_rustdoc,
                        )
                    });

                    before_selection.wait();
                    let contender = std::thread::spawn(move || {
                        crate::tddd::rustdoc_output_lock::RustdocOutputLock::acquire_for_test(
                            &exclusive,
                            Duration::from_millis(100),
                        )
                    });
                    let contention = contender.join().unwrap();
                    release_selection.wait();
                    let first_snapshot = first.join().unwrap().unwrap();
                    assert!(
                        contention.is_err(),
                        "a contender must be blocked before expected path selection"
                    );
                    let contention_error = contention.unwrap_err();
                    assert!(
                        contention_error.to_string().contains("timed out"),
                        "the pre-selection lock must reject a competing acquisition: {contention_error}"
                    );
                    assert_eq!(
                        first_snapshot.crate_data().crate_version.as_deref(),
                        Some("generation-a")
                    );
                },
            );
        });
    }

    #[cfg(unix)]
    #[test]
    fn test_exporter_rejects_output_path_differing_from_expected() {
        use std::os::unix::fs::PermissionsExt as _;
        use std::sync::Arc;

        crate::tddd::type_signals_evaluator::with_process_environment_lock(|| {
            let workspace = tempfile::tempdir().unwrap();
            let commands = workspace.path().join("commands");
            std::fs::create_dir_all(&commands).unwrap();
            let rustup = commands.join("rustup");
            std::fs::write(&rustup, "#!/bin/sh\nexit 0\n").unwrap();
            std::fs::set_permissions(&rustup, std::fs::Permissions::from_mode(0o755)).unwrap();
            let target_directory = workspace.path().join("cargo-target");
            let rustdoc_json = format!(
                r#"{{"root":0,"crate_version":"generation-a","includes_private":false,"index":{{}},"paths":{{}},"external_crates":{{}},"format_version":{},"target":{{"triple":"","target_features":[]}}}}"#,
                rustdoc_types::FORMAT_VERSION
            );
            let cargo = commands.join("cargo");
            std::fs::write(
                &cargo,
                format!(
                    r#"#!/bin/sh
if [ "$1" = "metadata" ]; then
  printf '%s\n' '{{"packages":[{{"name":"fixture","targets":[{{"kind":["lib"],"name":"fixture"}}]}}]}}'
  exit 0
fi
mkdir -p "$CARGO_TARGET_DIR/doc"
printf '%s\n' '{rustdoc_json}' > "$CARGO_TARGET_DIR/doc/fixture.json"
"#,
                    rustdoc_json = rustdoc_json
                ),
            )
            .unwrap();
            std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o755)).unwrap();
            let mut path_entries = vec![commands];
            path_entries
                .extend(std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()));
            let path = std::env::join_paths(path_entries).unwrap();

            temp_env::with_vars(
                [
                    ("CARGO_TARGET_DIR", Some(target_directory.as_os_str())),
                    ("PATH", Some(path.as_os_str())),
                ],
                || {
                    let crate_name = CrateName::new("fixture").unwrap();
                    let exporter = RustdocSchemaExporter::with_output_path_rewriter(
                        workspace.path().to_path_buf(),
                        Arc::new(|path: std::path::PathBuf| path.with_file_name("unexpected.json")),
                    );
                    let result = exporter.capture_rustdoc_json(&crate_name, &[]);
                    assert!(result.is_err(), "a changed rustdoc output path must be rejected");
                    if let Err(error) = result {
                        assert!(
                            error
                                .to_string()
                                .contains("rustdoc JSON output path changed during export"),
                            "unexpected path mismatch error: {error}"
                        );
                    }
                },
            );
        });
    }

    #[cfg(unix)]
    #[test]
    fn test_exporter_holds_lock_at_post_export_before_byte_copy() {
        use std::os::unix::fs::PermissionsExt as _;
        use std::sync::{Arc, Barrier};
        use std::time::Duration;

        crate::tddd::type_signals_evaluator::with_process_environment_lock(|| {
            let workspace = tempfile::tempdir().unwrap();
            let commands = workspace.path().join("commands");
            std::fs::create_dir_all(&commands).unwrap();
            let rustup = commands.join("rustup");
            std::fs::write(&rustup, "#!/bin/sh\nexit 0\n").unwrap();
            std::fs::set_permissions(&rustup, std::fs::Permissions::from_mode(0o755)).unwrap();
            let target_directory = workspace.path().join("cargo-target");
            let first_json = format!(
                r#"{{"root":0,"crate_version":"generation-a","includes_private":false,"index":{{}},"paths":{{}},"external_crates":{{}},"format_version":{},"target":{{"triple":"","target_features":[]}}}}"#,
                rustdoc_types::FORMAT_VERSION
            );
            let cargo = commands.join("cargo");
            std::fs::write(
                &cargo,
                format!(
                    r#"#!/bin/sh
if [ "$1" = "metadata" ]; then
  printf '%s\n' '{{"packages":[{{"name":"fixture","targets":[{{"kind":["lib"],"name":"fixture"}}]}}]}}'
  exit 0
fi
mkdir -p "$CARGO_TARGET_DIR/doc"
printf '%s\n' '{first_json}' > "$CARGO_TARGET_DIR/doc/fixture.json"
"#
                ),
            )
            .unwrap();
            std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o755)).unwrap();
            let mut path_entries = vec![commands];
            path_entries
                .extend(std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()));
            let path = std::env::join_paths(path_entries).unwrap();

            temp_env::with_vars(
                [
                    ("CARGO_TARGET_DIR", Some(target_directory.as_os_str())),
                    ("PATH", Some(path.as_os_str())),
                ],
                || {
                    let crate_name = CrateName::new("fixture").unwrap();
                    let identity_exporter =
                        RustdocSchemaExporter::new(workspace.path().to_path_buf());
                    let (identity, _) =
                        identity_exporter.rustdoc_execution_identity(&crate_name, &[]).unwrap();
                    let exclusive = identity.target_directory().as_path().to_path_buf();
                    let post_export = Arc::new(Barrier::new(2));
                    let release_read = Arc::new(Barrier::new(2));
                    let post_export_for_hook = Arc::clone(&post_export);
                    let release_read_for_hook = Arc::clone(&release_read);
                    let first_exporter = RustdocSchemaExporter::with_before_read(
                        workspace.path().to_path_buf(),
                        Arc::new(move || {
                            post_export_for_hook.wait();
                            release_read_for_hook.wait();
                        }),
                    );
                    let first_crate = crate_name.clone();
                    let first = std::thread::spawn(move || {
                        first_exporter.capture_rustdoc_snapshot(
                            &first_crate,
                            &[],
                            decode_fixture_rustdoc,
                        )
                    });

                    post_export.wait();
                    let contender = std::thread::spawn(move || {
                        crate::tddd::rustdoc_output_lock::RustdocOutputLock::acquire_for_test(
                            &exclusive,
                            Duration::from_millis(100),
                        )
                    });
                    let contention = contender.join().unwrap();
                    release_read.wait();
                    let first_snapshot = first.join().unwrap().unwrap();
                    assert!(
                        contention.is_err(),
                        "a contender must be blocked before the first byte read"
                    );
                    let contention_error = contention.unwrap_err();
                    assert!(
                        contention_error.to_string().contains("timed out"),
                        "the post-export lock must reject a competing acquisition: {contention_error}"
                    );
                    assert_eq!(
                        first_snapshot.crate_data().crate_version.as_deref(),
                        Some("generation-a")
                    );
                },
            );
        });
    }

    #[cfg(unix)]
    #[test]
    fn test_exporter_locked_byte_capture_is_immutable_after_output_replacement() {
        let workspace = tempfile::tempdir().unwrap();
        let exclusive = workspace.path().join(".sotp-rustdoc").join("selection");
        let output = exclusive.join("doc/domain.json");
        std::fs::create_dir_all(output.parent().unwrap()).unwrap();
        let first = format!(
            r#"{{"root":0,"crate_version":"generation-a","includes_private":false,"index":{{}},"paths":{{}},"external_crates":{{}},"format_version":{},"target":{{"triple":"","target_features":[]}}}}"#,
            rustdoc_types::FORMAT_VERSION
        );
        std::fs::write(&output, first.as_bytes()).unwrap();
        let lock =
            crate::tddd::rustdoc_output_lock::RustdocOutputLock::acquire(&exclusive).unwrap();
        let bytes = lock.read_bytes(&output, 64 * 1024 * 1024).unwrap();
        drop(lock);
        std::fs::write(&output, b"generation-b").unwrap();
        assert_eq!(bytes, first.as_bytes());
        let decoded: rustdoc_types::Crate = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded.crate_version.as_deref(), Some("generation-a"));
    }
}
