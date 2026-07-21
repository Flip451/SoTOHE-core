//! Per-layer implementation-input hashing for type-signal freshness.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use sha2::Digest;

use domain::tddd::type_signals_doc::Sha256Digest;

use super::EvaluateSignalsError;

const MAX_SOURCE_FILES: usize = 10_000;
const MAX_SOURCE_ENTRIES: usize = 20_000;
const MAX_SOURCE_DEPTH: usize = 32;
const MAX_SOURCE_FILE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_TOTAL_SOURCE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_TOOLCHAIN_COMMAND_OUTPUT_BYTES: usize = 64 * 1024;
const MAX_TOOLCHAIN_COMMAND_DURATION: Duration = Duration::from_secs(10);

/// Hashes exactly one crate's source contents, its manifest, optional build
/// script, the workspace manifest and lockfile, and the active nightly
/// toolchain identifier.
///
/// # Errors
///
/// Returns an error when any required input cannot be read. Callers must then
/// re-extract rather than reuse an existing rustdoc result.
pub(super) fn hash_implementation_inputs(
    workspace_root: &Path,
    target_crate: &str,
) -> Result<Sha256Digest, EvaluateSignalsError> {
    let toolchain_identifier = nightly_toolchain_identifier(workspace_root)?;
    hash_implementation_inputs_with_toolchain_identifier(
        workspace_root,
        target_crate,
        &toolchain_identifier,
    )
}

fn hash_implementation_inputs_with_toolchain_identifier(
    workspace_root: &Path,
    target_crate: &str,
    toolchain_identifier: &[u8],
) -> Result<Sha256Digest, EvaluateSignalsError> {
    let source_root = crate_source_root(workspace_root, target_crate)?;
    let crate_root = source_root.parent().ok_or_else(|| {
        EvaluateSignalsError(format!(
            "cannot determine crate root from source directory '{}'",
            source_root.display()
        ))
    })?;
    let mut source_files = Vec::new();
    let mut visited_entries = 0usize;
    collect_source_files(&source_root, 0, &mut visited_entries, &mut source_files)?;
    source_files.sort();

    let mut hasher = sha2::Sha256::new();
    let mut remaining_budget = MAX_TOTAL_SOURCE_BYTES;
    for path in source_files {
        let relative = path.strip_prefix(workspace_root).map_err(|_| {
            EvaluateSignalsError(format!(
                "crate source '{}' is outside the workspace",
                path.display()
            ))
        })?;
        append_component(&mut hasher, b"source-path", relative.as_os_str().as_encoded_bytes());
        append_component(
            &mut hasher,
            b"source-content",
            &read_regular_source_file(&path, MAX_SOURCE_FILE_BYTES, &mut remaining_budget)?,
        );
    }
    append_component(
        &mut hasher,
        b"crate-manifest",
        &read_regular_source_file(
            &crate_root.join("Cargo.toml"),
            MAX_SOURCE_FILE_BYTES,
            &mut remaining_budget,
        )?,
    );
    let build_script = crate_root.join("build.rs");
    match std::fs::symlink_metadata(&build_script) {
        Ok(_) => append_component(
            &mut hasher,
            b"crate-build-script",
            &read_regular_source_file(&build_script, MAX_SOURCE_FILE_BYTES, &mut remaining_budget)?,
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(EvaluateSignalsError(format!(
                "cannot stat optional crate build script '{}': {error}",
                build_script.display()
            )));
        }
    }
    append_component(
        &mut hasher,
        b"workspace-manifest",
        &read_regular_source_file(
            &workspace_root.join("Cargo.toml"),
            MAX_SOURCE_FILE_BYTES,
            &mut remaining_budget,
        )?,
    );
    append_component(
        &mut hasher,
        b"lockfile",
        &read_regular_source_file(
            &workspace_root.join("Cargo.lock"),
            MAX_SOURCE_FILE_BYTES,
            &mut remaining_budget,
        )?,
    );
    append_component(&mut hasher, b"toolchain", toolchain_identifier);

    Sha256Digest::try_new(format!("{:x}", hasher.finalize())).map_err(|error| {
        EvaluateSignalsError(format!("failed to construct implementation-input digest: {error}"))
    })
}

fn crate_source_root(
    workspace_root: &Path,
    target_crate: &str,
) -> Result<PathBuf, EvaluateSignalsError> {
    let crate_root = match target_crate {
        "domain" | "usecase" | "infrastructure" => workspace_root.join("libs").join(target_crate),
        "cli" => workspace_root.join("apps/cli"),
        "cli_driver" => workspace_root.join("apps/cli-driver"),
        "cli_composition" => workspace_root.join("apps/cli-composition"),
        _ => {
            return Err(EvaluateSignalsError(format!(
                "unsupported TDDD target crate '{target_crate}' for implementation-input hashing"
            )));
        }
    };
    let source_root = crate_root.join("src");
    // Reject symlinks on EVERY component up to the root, not just the final
    // `src` segment — a symlinked `libs/` or crate directory would otherwise
    // let the traversal hash sources outside the trusted workspace.
    crate::track::symlink_guard::reject_symlinks_up_to_root(&source_root).map_err(|error| {
        EvaluateSignalsError(format!(
            "refusing crate source directory '{}': {error}",
            source_root.display()
        ))
    })?;
    let metadata = std::fs::symlink_metadata(&source_root).map_err(|error| {
        EvaluateSignalsError(format!(
            "cannot stat crate source directory '{}': {error}",
            source_root.display()
        ))
    })?;
    if !metadata.is_dir() {
        return Err(EvaluateSignalsError(format!(
            "crate source directory '{}' is unavailable",
            source_root.display()
        )));
    }
    Ok(source_root)
}

fn collect_source_files(
    directory: &Path,
    depth: usize,
    visited_entries: &mut usize,
    files: &mut Vec<PathBuf>,
) -> Result<(), EvaluateSignalsError> {
    for entry in std::fs::read_dir(directory).map_err(|error| {
        EvaluateSignalsError(format!(
            "cannot read source directory '{}': {error}",
            directory.display()
        ))
    })? {
        // Every visited entry counts against the budget — directories too, so
        // a tree of empty directories cannot cause unbounded traversal work.
        *visited_entries += 1;
        if *visited_entries > MAX_SOURCE_ENTRIES {
            return Err(EvaluateSignalsError(format!(
                "crate source traversal exceeds maximum of {MAX_SOURCE_ENTRIES} entries"
            )));
        }
        let path = entry
            .map_err(|error| EvaluateSignalsError(format!("cannot read source entry: {error}")))?
            .path();
        let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
            EvaluateSignalsError(format!("cannot stat crate source '{}': {error}", path.display()))
        })?;
        if metadata.file_type().is_symlink() {
            return Err(EvaluateSignalsError(format!(
                "cannot read crate source '{}': symlinks are unsupported",
                path.display()
            )));
        }
        if metadata.is_dir() {
            if depth >= MAX_SOURCE_DEPTH {
                return Err(EvaluateSignalsError(format!(
                    "crate source traversal exceeds maximum depth of {MAX_SOURCE_DEPTH} at '{}'",
                    path.display()
                )));
            }
            collect_source_files(&path, depth + 1, visited_entries, files)?;
        } else if metadata.is_file() {
            if files.len() >= MAX_SOURCE_FILES {
                return Err(EvaluateSignalsError(format!(
                    "crate source traversal exceeds maximum of {MAX_SOURCE_FILES} files"
                )));
            }
            files.push(path);
        } else {
            return Err(EvaluateSignalsError(format!(
                "cannot read crate source '{}': not a regular file or directory",
                path.display()
            )));
        }
    }
    Ok(())
}

fn nightly_toolchain_identifier(workspace_root: &Path) -> Result<Vec<u8>, EvaluateSignalsError> {
    let mut command = Command::new("rustup");
    command.args(["run", "nightly", "rustc", "-Vv"]).current_dir(workspace_root);
    let output = crate::capability_exec::process::run_command_with_bounded_output(
        &mut command,
        MAX_TOOLCHAIN_COMMAND_OUTPUT_BYTES,
        MAX_TOOLCHAIN_COMMAND_DURATION,
        "nightly toolchain identity",
    )
    .map_err(|error| EvaluateSignalsError(format!("cannot identify nightly toolchain: {error}")))?;
    if !output.status.success() || output.stdout.is_empty() {
        return Err(EvaluateSignalsError("cannot identify nightly toolchain".to_owned()));
    }
    Ok(output.stdout)
}

fn read_regular_source_file(
    path: &Path,
    per_file_limit: u64,
    remaining_budget: &mut u64,
) -> Result<Vec<u8>, EvaluateSignalsError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        EvaluateSignalsError(format!("cannot stat build input '{}': {error}", path.display()))
    })?;
    if metadata.file_type().is_symlink() {
        return Err(EvaluateSignalsError(format!(
            "cannot read build input '{}': symlinks are unsupported",
            path.display()
        )));
    }
    if !metadata.is_file() {
        return Err(EvaluateSignalsError(format!(
            "cannot read build input '{}': not a regular file",
            path.display()
        )));
    }
    if metadata.len() > per_file_limit {
        return Err(EvaluateSignalsError(format!(
            "build input '{}' exceeds the {per_file_limit}-byte per-file limit; the \
             implementation-input hash is indeterminate",
            path.display()
        )));
    }
    // The take-bound caps the allocation even if the file grows between the
    // stat above and this read; reading one extra byte detects that race.
    let mut bytes = Vec::new();
    let file = std::fs::File::open(path).map_err(|error| {
        EvaluateSignalsError(format!("cannot read build input '{}': {error}", path.display()))
    })?;
    file.take(per_file_limit.saturating_add(1)).read_to_end(&mut bytes).map_err(|error| {
        EvaluateSignalsError(format!("cannot read build input '{}': {error}", path.display()))
    })?;
    if bytes.len() as u64 > per_file_limit {
        return Err(EvaluateSignalsError(format!(
            "build input '{}' grew past the {per_file_limit}-byte per-file limit during \
             hashing; the implementation-input hash is indeterminate",
            path.display()
        )));
    }
    if bytes.len() as u64 > *remaining_budget {
        return Err(EvaluateSignalsError(format!(
            "build inputs exceed the cumulative {MAX_TOTAL_SOURCE_BYTES}-byte budget at '{}'; \
             the implementation-input hash is indeterminate",
            path.display()
        )));
    }
    *remaining_budget -= bytes.len() as u64;
    Ok(bytes)
}

fn append_component(hasher: &mut sha2::Sha256, label: &[u8], content: &[u8]) {
    hasher.update((label.len() as u64).to_be_bytes());
    hasher.update(label);
    hasher.update((content.len() as u64).to_be_bytes());
    hasher.update(content);
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::{
        crate_source_root, hash_implementation_inputs,
        hash_implementation_inputs_with_toolchain_identifier, read_regular_source_file,
    };
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    static PROCESS_ENVIRONMENT_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_read_regular_source_file_enforces_per_file_and_cumulative_limits() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("input.rs");
        std::fs::write(&input, b"0123456789").unwrap();

        let mut budget = 1024u64;
        let oversized = read_regular_source_file(&input, 5, &mut budget).unwrap_err();
        assert!(oversized.0.contains("per-file limit"), "got: {}", oversized.0);

        let mut exhausted = 5u64;
        let over_budget = read_regular_source_file(&input, 64, &mut exhausted).unwrap_err();
        assert!(over_budget.0.contains("cumulative"), "got: {}", over_budget.0);

        let mut remaining = 64u64;
        assert_eq!(read_regular_source_file(&input, 64, &mut remaining).unwrap().len(), 10);
        assert_eq!(remaining, 54, "successful reads consume the cumulative budget");
    }

    fn workspace_with_domain_source() -> tempfile::TempDir {
        let workspace = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(workspace.path().join("libs/domain/src")).unwrap();
        std::fs::write(workspace.path().join("libs/domain/src/lib.rs"), "pub struct First;\n")
            .unwrap();
        std::fs::write(
            workspace.path().join("libs/domain/Cargo.toml"),
            "[package]\nname = \"domain\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        std::fs::write(
            workspace.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"libs/domain\"]\n",
        )
        .unwrap();
        std::fs::write(workspace.path().join("Cargo.lock"), "version = 4\n").unwrap();
        workspace
    }

    #[cfg(unix)]
    fn write_fake_rustup(directory: &Path, script: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;

        let rustup = directory.join("rustup");
        std::fs::write(&rustup, script).unwrap();
        std::fs::set_permissions(&rustup, std::fs::Permissions::from_mode(0o755)).unwrap();
        rustup
    }

    #[test]
    fn test_crate_source_root_resolves_workspace_layers() {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
        assert_eq!(
            crate_source_root(workspace, "infrastructure").unwrap(),
            workspace.join("libs/infrastructure/src")
        );
    }

    #[test]
    fn test_crate_source_root_rejects_unknown_target() {
        assert!(crate_source_root(Path::new("/workspace"), "unknown").is_err());
    }

    #[test]
    fn test_hash_implementation_inputs_includes_source_lockfile_and_toolchain_and_rejects_missing_required_input()
     {
        let workspace = workspace_with_domain_source();
        let root = workspace.path();
        let initial =
            hash_implementation_inputs_with_toolchain_identifier(root, "domain", b"nightly-a")
                .unwrap();

        std::fs::write(root.join("libs/domain/src/lib.rs"), "pub struct Second;\n").unwrap();
        let changed_source =
            hash_implementation_inputs_with_toolchain_identifier(root, "domain", b"nightly-a")
                .unwrap();
        assert_ne!(initial, changed_source, "source content must participate in the hash");

        std::fs::write(root.join("Cargo.lock"), "version = 4\n# changed\n").unwrap();
        let changed_lockfile =
            hash_implementation_inputs_with_toolchain_identifier(root, "domain", b"nightly-a")
                .unwrap();
        assert_ne!(changed_source, changed_lockfile, "Cargo.lock must participate in the hash");

        let changed_toolchain =
            hash_implementation_inputs_with_toolchain_identifier(root, "domain", b"nightly-b")
                .unwrap();
        assert_ne!(changed_lockfile, changed_toolchain, "toolchain identity must participate");

        std::fs::remove_file(root.join("Cargo.lock")).unwrap();
        assert!(
            hash_implementation_inputs_with_toolchain_identifier(root, "domain", b"nightly-b")
                .is_err(),
            "a missing required input must make the implementation hash indeterminate"
        );
    }

    #[test]
    fn test_hash_implementation_inputs_includes_workspace_and_target_manifests_and_optional_build_script_only()
     {
        let workspace = workspace_with_domain_source();
        let root = workspace.path();
        let crate_root = root.join("libs/domain");
        let initial =
            hash_implementation_inputs_with_toolchain_identifier(root, "domain", b"nightly")
                .unwrap();

        std::fs::create_dir_all(root.join("libs/usecase/src")).unwrap();
        std::fs::write(root.join("libs/usecase/Cargo.toml"), "[package]\nname = \"usecase\"\n")
            .unwrap();
        std::fs::write(root.join("libs/usecase/src/lib.rs"), "pub struct Sibling;\n").unwrap();
        let unchanged_by_sibling_crate =
            hash_implementation_inputs_with_toolchain_identifier(root, "domain", b"nightly")
                .unwrap();
        assert_eq!(
            initial, unchanged_by_sibling_crate,
            "sibling crate manifests and sources must stay outside the target crate boundary"
        );

        std::fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"libs/domain\", \"libs/usecase\"]\n",
        )
        .unwrap();
        let changed_workspace_manifest =
            hash_implementation_inputs_with_toolchain_identifier(root, "domain", b"nightly")
                .unwrap();
        assert_ne!(
            initial, changed_workspace_manifest,
            "workspace Cargo.toml must participate in the hash"
        );

        std::fs::write(
            crate_root.join("Cargo.toml"),
            "[package]\nname = \"domain\"\nversion = \"0.2.0\"\n",
        )
        .unwrap();
        let changed_manifest =
            hash_implementation_inputs_with_toolchain_identifier(root, "domain", b"nightly")
                .unwrap();
        assert_ne!(
            changed_workspace_manifest, changed_manifest,
            "target Cargo.toml must participate in the hash"
        );

        std::fs::write(crate_root.join("build.rs"), "fn main() { println!(\"first\"); }\n")
            .unwrap();
        let added_build_script =
            hash_implementation_inputs_with_toolchain_identifier(root, "domain", b"nightly")
                .unwrap();
        std::fs::write(crate_root.join("build.rs"), "fn main() { println!(\"second\"); }\n")
            .unwrap();
        let changed_build_script =
            hash_implementation_inputs_with_toolchain_identifier(root, "domain", b"nightly")
                .unwrap();
        assert_ne!(
            added_build_script, changed_build_script,
            "target build.rs must participate in the hash when present"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_hash_implementation_inputs_rejects_symlinked_source_file() {
        let workspace = workspace_with_domain_source();
        let source_path = workspace.path().join("libs/domain/src/external.rs");
        let outside = workspace.path().join("outside.rs");
        std::fs::write(&outside, "pub struct Outside;\n").unwrap();
        std::os::unix::fs::symlink(&outside, &source_path).unwrap();

        let error = hash_implementation_inputs_with_toolchain_identifier(
            workspace.path(),
            "domain",
            b"nightly",
        )
        .unwrap_err();

        assert!(error.to_string().contains("symlinks are unsupported"));
    }

    #[cfg(unix)]
    #[test]
    fn test_hash_implementation_inputs_uses_rustup_toolchain_identity_and_rejects_unavailable_rustup()
     {
        let _environment_guard = PROCESS_ENVIRONMENT_LOCK.lock().unwrap();
        let workspace = workspace_with_domain_source();
        let fake_bin = tempfile::tempdir().unwrap();

        let rustup = write_fake_rustup(fake_bin.path(), "#!/bin/sh\nprintf 'nightly-a\\n'\n");
        let first = temp_env::with_var("PATH", Some(fake_bin.path().as_os_str()), || {
            hash_implementation_inputs(workspace.path(), "domain")
        })
        .unwrap();

        std::fs::write(&rustup, "#!/bin/sh\nprintf 'nightly-b\\n'\n").unwrap();
        let second = temp_env::with_var("PATH", Some(fake_bin.path().as_os_str()), || {
            hash_implementation_inputs(workspace.path(), "domain")
        })
        .unwrap();
        assert_ne!(first, second, "the rustup-reported toolchain identity must affect the hash");

        std::fs::remove_file(&rustup).unwrap();
        assert!(
            temp_env::with_var("PATH", Some(fake_bin.path().as_os_str()), || {
                hash_implementation_inputs(workspace.path(), "domain")
            })
            .is_err(),
            "an unavailable rustup must make the implementation hash indeterminate"
        );

        write_fake_rustup(fake_bin.path(), "#!/bin/sh\nexit 1\n");
        assert!(
            temp_env::with_var("PATH", Some(fake_bin.path().as_os_str()), || {
                hash_implementation_inputs(workspace.path(), "domain")
            })
            .is_err(),
            "a failing rustup must make the implementation hash indeterminate"
        );
    }
}
