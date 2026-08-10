//! Bounded implementation-input hashing for type-signal freshness.
//!
//! The source closure is a conservative over-approximation: the committed
//! architecture layer graph selects crate directories, and every regular file
//! in those directories participates in the digest. The former Cargo-manifest
//! and Rust-source precision scanner was intentionally removed; its open-set
//! interpretation could not provide a stable local/branch contract.

use std::io::Read;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

#[cfg(test)]
use std::sync::Mutex;

use sha2::Digest;

use domain::tddd::type_signals_doc::Sha256Digest;

use super::EvaluateSignalsError;
use super::layer_graph::{LayerCrateRoot, LayerGraph};

pub(crate) const MAX_ARCHITECTURE_RULES_BYTES: usize = 4 * 1024 * 1024;
pub(crate) const MAX_SOURCE_FILE_BYTES: u64 = 16 * 1024 * 1024;
pub(crate) const MAX_TOTAL_SOURCE_BYTES: u64 = 64 * 1024 * 1024;
pub(crate) const MAX_SOURCE_ENTRIES: usize = 20_000;
pub(crate) const MAX_SOURCE_FILES: usize = 10_000;
pub(crate) const MAX_SOURCE_DEPTH: usize = 32;
/// A regular file's canonical tree identity: repository-relative path,
/// executable bit, and streamed content digest.
pub(crate) type TreeFileDigest = (Vec<u8>, bool, [u8; 32]);
const HASH_READ_BUFFER_BYTES: usize = 8 * 1024;
const MAX_TOOLCHAIN_COMMAND_OUTPUT_BYTES: usize = 64 * 1024;
const MAX_TOOLCHAIN_COMMAND_DURATION: Duration = Duration::from_secs(10);
#[cfg(test)]
pub(super) static PROCESS_ENVIRONMENT_LOCK: Mutex<()> = Mutex::new(());

/// Hashes one layer's graph-selected crate trees, the workspace manifest and
/// lockfile, and the active nightly toolchain identity.
///
/// `target` is either the architecture-rules layer crate name or the explicit
/// schema-export target from that same document. The graph resolves the latter
/// to its owning layer before selecting the closed crate-tree over-approximation.
pub(crate) fn hash_implementation_inputs_with_toolchain_identifier(
    workspace_root: &Path,
    target: &str,
    toolchain_identifier: &[u8],
) -> Result<Sha256Digest, EvaluateSignalsError> {
    let graph = load_layer_graph(workspace_root)?;
    let roots = graph.crate_roots_for(target).map_err(EvaluateSignalsError::authoritative_input)?;
    let mut remaining_budget = MAX_TOTAL_SOURCE_BYTES;
    let workspace_manifest = read_regular_source_file(
        &workspace_root.join("Cargo.toml"),
        MAX_SOURCE_FILE_BYTES,
        &mut remaining_budget,
    )?;
    let lockfile = read_regular_source_file(
        &workspace_root.join("Cargo.lock"),
        MAX_SOURCE_FILE_BYTES,
        &mut remaining_budget,
    )?;
    let tree_files =
        collect_local_tree_file_digests(workspace_root, &roots, &mut remaining_budget)?;
    hash_implementation_input_components(
        &tree_files,
        &workspace_manifest,
        &lockfile,
        target,
        toolchain_identifier,
    )
}

/// Test-only wrapper retaining the real rustup authority boundary.
#[cfg(test)]
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

/// Hashes the shared, already-acquired components used by both local and
/// branch readers. Tree file content is represented by a streamed SHA-256
/// digest, so neither side buffers an entire crate tree.
pub(crate) fn hash_implementation_input_components(
    tree_files: &[TreeFileDigest],
    workspace_manifest: &[u8],
    lockfile: &[u8],
    schema_export_target: &str,
    toolchain_identifier: &[u8],
) -> Result<Sha256Digest, EvaluateSignalsError> {
    let mut sorted_tree_files = tree_files.to_vec();
    sorted_tree_files.sort_by(|(left, _, _), (right, _, _)| left.cmp(right));

    let mut hasher = sha2::Sha256::new();
    hasher.update(b"sotohe-implementation-inputs-v4\0");
    append_component(&mut hasher, b"schema-export-target", schema_export_target.as_bytes());
    for (path, executable, content_digest) in sorted_tree_files {
        append_component(&mut hasher, b"crate-file-path", &path);
        append_component(&mut hasher, b"crate-file-executable", &[u8::from(executable)]);
        append_component(&mut hasher, b"crate-file-content-sha256", &content_digest);
    }
    append_component(&mut hasher, b"workspace-manifest", workspace_manifest);
    append_component(&mut hasher, b"lockfile", lockfile);
    append_component(&mut hasher, b"toolchain", toolchain_identifier);

    Sha256Digest::try_new(format!("{:x}", hasher.finalize())).map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!(
            "failed to construct implementation-input digest: {error}"
        ))
    })
}

fn load_layer_graph(workspace_root: &Path) -> Result<LayerGraph, EvaluateSignalsError> {
    let path = workspace_root.join("architecture-rules.json");
    let bytes = read_bounded_bytes(&path, MAX_ARCHITECTURE_RULES_BYTES).map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!(
            "cannot read architecture-rules.json: {error}"
        ))
    })?;
    LayerGraph::parse(&bytes).map_err(EvaluateSignalsError::authoritative_input)
}

fn collect_local_tree_file_digests(
    workspace_root: &Path,
    roots: &[LayerCrateRoot],
    remaining_budget: &mut u64,
) -> Result<Vec<TreeFileDigest>, EvaluateSignalsError> {
    let mut files = Vec::new();
    let mut visited_entries = 0usize;
    for root in roots {
        let crate_path = workspace_root.join(&root.path);
        crate::track::symlink_guard::reject_symlinks_up_to_root(&crate_path).map_err(|error| {
            EvaluateSignalsError::authoritative_input(format!(
                "cannot inspect layer crate '{}' at '{}': {error}",
                root.crate_name, root.path
            ))
        })?;
        let metadata = std::fs::symlink_metadata(&crate_path).map_err(|error| {
            EvaluateSignalsError::authoritative_input(format!(
                "cannot inspect layer crate '{}' at '{}': {error}",
                root.crate_name, root.path
            ))
        })?;
        if metadata.file_type().is_symlink() {
            return Err(EvaluateSignalsError::authoritative_input(format!(
                "layer crate '{}' at '{}' is a symlink",
                root.crate_name, root.path
            )));
        }
        if !metadata.is_dir() {
            return Err(EvaluateSignalsError::authoritative_input(format!(
                "layer crate '{}' at '{}' is not a directory",
                root.crate_name, root.path
            )));
        }
        collect_local_tree(
            workspace_root,
            &crate_path,
            &root.path,
            &mut visited_entries,
            &mut files,
            remaining_budget,
        )?;
    }
    if files.is_empty() {
        return Err(EvaluateSignalsError::authoritative_input(
            "architecture layer closure contains no regular files".to_owned(),
        ));
    }
    Ok(files)
}

fn collect_local_tree(
    workspace_root: &Path,
    directory: &Path,
    crate_root: &str,
    visited_entries: &mut usize,
    files: &mut Vec<TreeFileDigest>,
    remaining_budget: &mut u64,
) -> Result<(), EvaluateSignalsError> {
    let mut entries = std::fs::read_dir(directory)
        .map_err(|error| local_input_error(directory, format!("cannot read directory: {error}")))?;
    let mut paths = Vec::new();
    for entry in entries.by_ref() {
        let entry = entry.map_err(|error| {
            local_input_error(directory, format!("cannot enumerate directory: {error}"))
        })?;
        *visited_entries = visited_entries.checked_add(1).ok_or_else(|| {
            EvaluateSignalsError::authoritative_input(
                "implementation source entry count overflowed".to_owned(),
            )
        })?;
        if *visited_entries > MAX_SOURCE_ENTRIES {
            return Err(EvaluateSignalsError::authoritative_input(format!(
                "implementation source traversal exceeds maximum of {MAX_SOURCE_ENTRIES} entries"
            )));
        }
        paths.push(entry.path());
    }
    paths.sort();

    for path in paths {
        let relative = workspace_relative_path(workspace_root, &path)?;
        if is_vcs_internal(&relative) {
            continue;
        }
        let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
            local_input_error(&path, format!("cannot stat tree entry: {error}"))
        })?;
        if metadata.is_dir() {
            let depth = crate_relative_depth(crate_root, &relative);
            if depth >= MAX_SOURCE_DEPTH {
                return Err(EvaluateSignalsError::authoritative_input(format!(
                    "implementation source traversal exceeds maximum depth of {MAX_SOURCE_DEPTH} at '{relative}'"
                )));
            }
            collect_local_tree(
                workspace_root,
                &path,
                crate_root,
                visited_entries,
                files,
                remaining_budget,
            )?;
            continue;
        }
        if metadata.file_type().is_symlink() {
            return Err(EvaluateSignalsError::authoritative_input(format!(
                "implementation tree entry '{relative}' is a symlink"
            )));
        }
        if !metadata.is_file() {
            return Err(EvaluateSignalsError::authoritative_input(format!(
                "implementation tree entry '{relative}' is not a regular file"
            )));
        }
        if files.len() >= MAX_SOURCE_FILES {
            return Err(EvaluateSignalsError::authoritative_input(format!(
                "implementation source traversal exceeds maximum of {MAX_SOURCE_FILES} files"
            )));
        }
        let (digest, _, executable) = digest_local_file(&path, &relative, remaining_budget)?;
        files.push((relative.into_bytes(), executable, digest));
    }
    Ok(())
}

fn crate_relative_depth(crate_root: &str, relative: &str) -> usize {
    relative.strip_prefix(crate_root).unwrap_or(relative).trim_start_matches('/').split('/').count()
}

fn workspace_relative_path(
    workspace_root: &Path,
    path: &Path,
) -> Result<String, EvaluateSignalsError> {
    let relative = path.strip_prefix(workspace_root).map_err(|_| {
        EvaluateSignalsError::authoritative_input(format!(
            "implementation tree entry '{}' escaped the workspace root",
            path.display()
        ))
    })?;
    let relative = relative.to_str().ok_or_else(|| {
        EvaluateSignalsError::authoritative_input(format!(
            "implementation tree entry '{}' is not valid UTF-8",
            path.display()
        ))
    })?;
    Ok(relative.replace(std::path::MAIN_SEPARATOR, "/"))
}

fn is_vcs_internal(path: &str) -> bool {
    path.split('/').any(|component| component == ".git")
}

fn digest_local_file(
    path: &Path,
    relative: &str,
    remaining_budget: &mut u64,
) -> Result<([u8; 32], u64, bool), EvaluateSignalsError> {
    let metadata = std::fs::metadata(path)
        .map_err(|error| local_input_error(path, format!("cannot read file metadata: {error}")))?;
    if metadata.len() > MAX_SOURCE_FILE_BYTES {
        return Err(EvaluateSignalsError::authoritative_input(format!(
            "implementation file '{relative}' exceeds the {MAX_SOURCE_FILE_BYTES}-byte limit"
        )));
    }
    if metadata.len() > *remaining_budget {
        return Err(EvaluateSignalsError::authoritative_input(format!(
            "implementation files exceed the {MAX_TOTAL_SOURCE_BYTES}-byte cumulative limit at '{relative}'"
        )));
    }
    let file = std::fs::File::open(path)
        .map_err(|error| local_input_error(path, format!("cannot read file: {error}")))?;
    let mut reader = file.take(MAX_SOURCE_FILE_BYTES.saturating_add(1));
    let mut buffer = [0_u8; HASH_READ_BUFFER_BYTES];
    let mut hasher = sha2::Sha256::new();
    let mut bytes_seen = 0_u64;
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| local_input_error(path, format!("cannot stream file: {error}")))?;
        if read == 0 {
            break;
        }
        bytes_seen = bytes_seen.saturating_add(read as u64);
        if bytes_seen > MAX_SOURCE_FILE_BYTES {
            return Err(EvaluateSignalsError::authoritative_input(format!(
                "implementation file '{relative}' grew past the {MAX_SOURCE_FILE_BYTES}-byte limit"
            )));
        }
        if bytes_seen > *remaining_budget {
            return Err(EvaluateSignalsError::authoritative_input(format!(
                "implementation files exceed the {MAX_TOTAL_SOURCE_BYTES}-byte cumulative limit at '{relative}'"
            )));
        }
        let chunk = buffer.get(..read).ok_or_else(|| {
            EvaluateSignalsError::authoritative_input("file stream returned an invalid byte count")
        })?;
        hasher.update(chunk);
    }
    *remaining_budget -= bytes_seen;
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 32];
    bytes.copy_from_slice(&digest);
    Ok((bytes, bytes_seen, is_executable(&metadata)?))
}

#[cfg(unix)]
fn is_executable(metadata: &std::fs::Metadata) -> Result<bool, EvaluateSignalsError> {
    use std::os::unix::fs::PermissionsExt;

    Ok(metadata.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(_metadata: &std::fs::Metadata) -> Result<bool, EvaluateSignalsError> {
    Err(EvaluateSignalsError::authoritative_input(
        "cannot determine the canonical Git executable bit on this platform".to_owned(),
    ))
}

fn local_input_error(path: &Path, message: String) -> EvaluateSignalsError {
    EvaluateSignalsError::authoritative_input(format!(
        "implementation input '{}': {message}",
        path.display()
    ))
}

fn read_bounded_bytes(path: &Path, maximum_bytes: usize) -> Result<Vec<u8>, std::io::Error> {
    crate::track::symlink_guard::reject_symlinks_up_to_root(path)
        .map_err(|error| std::io::Error::other(error.to_string()))?;
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(std::io::Error::other("symlinks are unsupported"));
    }
    if !metadata.is_file() {
        return Err(std::io::Error::other("not a regular file"));
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take((maximum_bytes as u64).saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() > maximum_bytes {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "file exceeds maximum size",
        ));
    }
    Ok(bytes)
}

fn read_regular_source_file(
    path: &Path,
    per_file_limit: u64,
    remaining_budget: &mut u64,
) -> Result<Vec<u8>, EvaluateSignalsError> {
    crate::track::symlink_guard::reject_symlinks_up_to_root(path).map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!(
            "cannot read implementation input '{}': {error}",
            path.display()
        ))
    })?;
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!(
            "cannot stat implementation input '{}': {error}",
            path.display()
        ))
    })?;
    if metadata.file_type().is_symlink() {
        return Err(EvaluateSignalsError::authoritative_input(format!(
            "cannot read implementation input '{}': symlinks are unsupported",
            path.display()
        )));
    }
    if !metadata.is_file() {
        return Err(EvaluateSignalsError::authoritative_input(format!(
            "cannot read implementation input '{}': not a regular file",
            path.display()
        )));
    }
    if metadata.len() > per_file_limit {
        return Err(EvaluateSignalsError::authoritative_input(format!(
            "implementation input '{}' exceeds the {per_file_limit}-byte per-file limit",
            path.display()
        )));
    }
    if metadata.len() > *remaining_budget {
        return Err(EvaluateSignalsError::authoritative_input(format!(
            "implementation inputs exceed the {MAX_TOTAL_SOURCE_BYTES}-byte cumulative limit at '{}'",
            path.display()
        )));
    }
    let mut bytes = Vec::new();
    let file = std::fs::File::open(path).map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!(
            "cannot read implementation input '{}': {error}",
            path.display()
        ))
    })?;
    file.take(per_file_limit.saturating_add(1)).read_to_end(&mut bytes).map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!(
            "cannot read implementation input '{}': {error}",
            path.display()
        ))
    })?;
    if bytes.len() as u64 > per_file_limit {
        return Err(EvaluateSignalsError::authoritative_input(format!(
            "implementation input '{}' grew past the per-file limit",
            path.display()
        )));
    }
    if bytes.len() as u64 > *remaining_budget {
        return Err(EvaluateSignalsError::authoritative_input(format!(
            "implementation inputs exceed the {MAX_TOTAL_SOURCE_BYTES}-byte cumulative limit at '{}'",
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

/// Result of probing the local nightly toolchain authority.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum NightlyToolchainProbe {
    Installed(Vec<u8>),
    Absent,
}

/// Enumerates toolchains and reads the installed nightly identity.
///
/// Absent is returned only when enumeration succeeds, parses successfully, and
/// finds no nightly. Probe, execution, decoding, and identity failures remain
/// authoritative-input errors.
pub(crate) fn probe_nightly_toolchain(
    workspace_root: &Path,
) -> Result<NightlyToolchainProbe, EvaluateSignalsError> {
    #[cfg(test)]
    {
        let fixture_identity = workspace_root.join(".test-nightly-toolchain-identity");
        if let Ok(identity) = std::fs::read(&fixture_identity) {
            if !identity.is_empty() {
                return Ok(NightlyToolchainProbe::Installed(identity));
            }
        }
    }

    let mut list_command = Command::new("rustup");
    list_command.args(["toolchain", "list"]).current_dir(workspace_root);
    let list_output = crate::capability_exec::process::run_command_with_bounded_output(
        &mut list_command,
        MAX_TOOLCHAIN_COMMAND_OUTPUT_BYTES,
        MAX_TOOLCHAIN_COMMAND_DURATION,
        "nightly toolchain enumeration",
    )
    .map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!(
            "cannot enumerate nightly toolchains: {error}"
        ))
    })?;
    if !list_output.status.success() {
        return Err(EvaluateSignalsError::authoritative_input(
            "cannot enumerate nightly toolchains".to_owned(),
        ));
    }
    if !toolchain_list_contains_nightly(&list_output.stdout).map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!(
            "cannot parse nightly toolchain enumeration: {error}"
        ))
    })? {
        return Ok(NightlyToolchainProbe::Absent);
    }

    let mut command = Command::new("rustup");
    command.args(["run", "nightly", "rustc", "-Vv"]).current_dir(workspace_root);
    let output = crate::capability_exec::process::run_command_with_bounded_output(
        &mut command,
        MAX_TOOLCHAIN_COMMAND_OUTPUT_BYTES,
        MAX_TOOLCHAIN_COMMAND_DURATION,
        "nightly toolchain identity",
    )
    .map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!(
            "cannot identify nightly toolchain: {error}"
        ))
    })?;
    if !output.status.success()
        || output.stdout.is_empty()
        || output.stdout.iter().all(u8::is_ascii_whitespace)
    {
        return Err(EvaluateSignalsError::authoritative_input(
            "cannot identify nightly toolchain".to_owned(),
        ));
    }
    std::str::from_utf8(&output.stdout).map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!(
            "cannot read nightly toolchain identity: {error}"
        ))
    })?;
    Ok(NightlyToolchainProbe::Installed(output.stdout))
}

pub(crate) fn nightly_toolchain_identifier(
    workspace_root: &Path,
) -> Result<Vec<u8>, EvaluateSignalsError> {
    match probe_nightly_toolchain(workspace_root)? {
        NightlyToolchainProbe::Installed(identity) => Ok(identity),
        NightlyToolchainProbe::Absent => Err(EvaluateSignalsError::authoritative_input(
            "nightly toolchain is not installed".to_owned(),
        )),
    }
}

fn toolchain_list_contains_nightly(output: &[u8]) -> Result<bool, String> {
    let text = std::str::from_utf8(output).map_err(|error| error.to_string())?;
    let mut contains_nightly = false;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut fields = line.split_whitespace();
        let Some(name) = fields.next() else {
            return Err("toolchain entry has no name".to_owned());
        };
        if !is_valid_toolchain_name(name) {
            return Err(format!("invalid toolchain name '{name}'"));
        }
        let status = line[name.len()..].trim();
        if !status.is_empty() && (!status.starts_with('(') || !status.ends_with(')')) {
            return Err(format!("invalid status suffix for toolchain '{name}'"));
        }
        if name == "nightly" || name.starts_with("nightly-") {
            contains_nightly = true;
        }
    }
    Ok(contains_nightly)
}

fn is_valid_toolchain_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::{
        hash_implementation_inputs, hash_implementation_inputs_with_toolchain_identifier,
        read_regular_source_file, toolchain_list_contains_nightly,
    };
    use std::path::{Path, PathBuf};

    fn architecture_rules() -> &'static str {
        r#"{
          "version": 2,
          "module_limits": {"exclude": ["vendor/", ".cache/", "target/", "tmp/"]},
          "layers": [
            {"crate":"domain","path":"libs/domain","may_depend_on":[]},
            {"crate":"usecase","path":"libs/usecase","may_depend_on":["domain"]},
            {"crate":"infrastructure","path":"libs/infrastructure","may_depend_on":["usecase"]},
            {"crate":"outside","path":"libs/outside","may_depend_on":[]}
          ]
        }"#
    }

    fn workspace_with_layer_graph() -> tempfile::TempDir {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path();
        std::fs::write(root.join("architecture-rules.json"), architecture_rules()).unwrap();
        std::fs::write(root.join("Cargo.toml"), "[workspace]\nresolver = \"2\"\n").unwrap();
        std::fs::write(root.join("Cargo.lock"), "version = 4\n").unwrap();
        for crate_name in ["domain", "usecase", "infrastructure", "outside"] {
            let crate_root = root.join("libs").join(crate_name);
            std::fs::create_dir_all(crate_root.join("src")).unwrap();
            std::fs::write(
                crate_root.join("Cargo.toml"),
                format!("[package]\nname = \"{crate_name}\"\nversion = \"0.1.0\"\n"),
            )
            .unwrap();
            std::fs::write(
                crate_root.join("src/lib.rs"),
                format!("pub struct {crate_name}Source;\n"),
            )
            .unwrap();
        }
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
    fn test_read_regular_source_file_enforces_per_file_and_cumulative_limits() {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("input.rs");
        std::fs::write(&input, b"0123456789").unwrap();

        let mut budget = 1024_u64;
        let oversized = read_regular_source_file(&input, 5, &mut budget).unwrap_err();
        assert!(oversized.to_string().contains("per-file limit"), "got: {oversized}");

        let mut exhausted = 5_u64;
        let over_budget = read_regular_source_file(&input, 64, &mut exhausted).unwrap_err();
        assert!(over_budget.to_string().contains("cumulative"), "got: {over_budget}");

        let mut remaining = 64_u64;
        assert_eq!(read_regular_source_file(&input, 64, &mut remaining).unwrap().len(), 10);
        assert_eq!(remaining, 54);
    }

    #[test]
    fn test_toolchain_list_with_malformed_entry_fails_closed() {
        assert!(
            !toolchain_list_contains_nightly(b"stable-x86_64-unknown-linux-gnu (default)\n")
                .unwrap()
        );
        assert!(
            toolchain_list_contains_nightly(
                b"stable-x86_64-unknown-linux-gnu (default)\nnightly-x86_64-unknown-linux-gnu\n"
            )
            .unwrap()
        );
        let error =
            toolchain_list_contains_nightly(b"nightly-x86_64-unknown-linux-gnu\nnot-valid!\n")
                .unwrap_err();
        assert!(error.contains("invalid toolchain name"), "got: {error}");
    }

    #[test]
    fn test_hash_layer_graph_dependency_tree_changes_but_outside_tree_does_not() {
        let workspace = workspace_with_layer_graph();
        let root = workspace.path();
        let initial =
            hash_implementation_inputs_with_toolchain_identifier(root, "usecase", b"nightly")
                .unwrap();

        std::fs::write(root.join("libs/domain/README.md"), "dependency change\n").unwrap();
        let changed_dependency =
            hash_implementation_inputs_with_toolchain_identifier(root, "usecase", b"nightly")
                .unwrap();
        assert_ne!(initial, changed_dependency);

        std::fs::write(root.join("libs/outside/README.md"), "outside change\n").unwrap();
        let changed_outside =
            hash_implementation_inputs_with_toolchain_identifier(root, "usecase", b"nightly")
                .unwrap();
        assert_eq!(
            changed_dependency, changed_outside,
            "a crate outside the target layer closure must not affect the hash"
        );
    }

    #[test]
    fn test_hash_layer_graph_hashes_every_regular_file_and_non_tree_components() {
        let workspace = workspace_with_layer_graph();
        let root = workspace.path();
        let initial =
            hash_implementation_inputs_with_toolchain_identifier(root, "usecase", b"nightly-a")
                .unwrap();

        std::fs::write(root.join("libs/domain/generated.txt"), b"whole tree\n").unwrap();
        let changed_tree =
            hash_implementation_inputs_with_toolchain_identifier(root, "usecase", b"nightly-a")
                .unwrap();
        assert_ne!(initial, changed_tree);

        std::fs::write(root.join("Cargo.lock"), "version = 4\n# changed\n").unwrap();
        let changed_lock =
            hash_implementation_inputs_with_toolchain_identifier(root, "usecase", b"nightly-a")
                .unwrap();
        assert_ne!(changed_tree, changed_lock);

        let changed_toolchain =
            hash_implementation_inputs_with_toolchain_identifier(root, "usecase", b"nightly-b")
                .unwrap();
        assert_ne!(changed_lock, changed_toolchain);

        std::fs::remove_file(root.join("Cargo.lock")).unwrap();
        assert!(
            hash_implementation_inputs_with_toolchain_identifier(root, "usecase", b"nightly-b")
                .is_err()
        );
    }

    #[test]
    fn test_hash_layer_graph_resolves_schema_export_target_alias() {
        let workspace = workspace_with_layer_graph();
        let root = workspace.path();
        let rules = architecture_rules().replace(
            "{\"crate\":\"usecase\",\"path\":\"libs/usecase\",\"may_depend_on\":[\"domain\"]}",
            "{\"crate\":\"usecase\",\"path\":\"libs/usecase\",\"may_depend_on\":[\"domain\"],\"tddd\":{\"enabled\":true,\"schema_export\":{\"targets\":[\"application\"]}}}",
        );
        std::fs::write(root.join("architecture-rules.json"), rules).unwrap();

        let first =
            hash_implementation_inputs_with_toolchain_identifier(root, "application", b"nightly")
                .unwrap();
        let second =
            hash_implementation_inputs_with_toolchain_identifier(root, "application", b"nightly")
                .unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn test_crate_relative_depth_matches_branch_tree_definition() {
        assert_eq!(super::crate_relative_depth("libs/domain", "libs/domain/src"), 1);
        assert_eq!(super::crate_relative_depth("libs/domain", "libs/domain/src/nested/lib.rs"), 3);
    }

    #[cfg(unix)]
    #[test]
    fn test_hash_layer_graph_changes_when_regular_file_executable_mode_changes() {
        use std::os::unix::fs::PermissionsExt;

        let workspace = workspace_with_layer_graph();
        let root = workspace.path();
        let path = root.join("libs/domain/README.md");
        std::fs::write(&path, b"mode-sensitive\n").unwrap();
        let initial =
            hash_implementation_inputs_with_toolchain_identifier(root, "usecase", b"nightly")
                .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let executable =
            hash_implementation_inputs_with_toolchain_identifier(root, "usecase", b"nightly")
                .unwrap();
        assert_ne!(initial, executable);
    }

    #[cfg(unix)]
    #[test]
    fn test_hash_layer_graph_rejects_symlinked_tree_entry() {
        let workspace = workspace_with_layer_graph();
        let root = workspace.path();
        let outside = root.join("outside.txt");
        std::fs::write(&outside, "outside\n").unwrap();
        std::os::unix::fs::symlink(&outside, root.join("libs/domain/link.txt")).unwrap();
        let error =
            hash_implementation_inputs_with_toolchain_identifier(root, "usecase", b"nightly")
                .unwrap_err();
        assert!(error.to_string().contains("symlink"), "got: {error}");
    }

    #[cfg(unix)]
    #[test]
    fn test_hash_layer_graph_rejects_symlinked_crate_ancestor() {
        let workspace = workspace_with_layer_graph();
        let root = workspace.path();
        let real_libs = root.join("real-libs");
        std::fs::rename(root.join("libs"), &real_libs).unwrap();
        std::os::unix::fs::symlink(&real_libs, root.join("libs")).unwrap();

        let error =
            hash_implementation_inputs_with_toolchain_identifier(root, "usecase", b"nightly")
                .unwrap_err();
        assert!(error.to_string().contains("symlink"), "got: {error}");
    }

    #[cfg(unix)]
    #[test]
    fn test_hash_implementation_inputs_uses_rustup_identity_and_rejects_probe_failure() {
        super::super::with_process_environment_lock(|| {
            let workspace = workspace_with_layer_graph();
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
            assert_ne!(first, second);

            std::fs::remove_file(&rustup).unwrap();
            assert!(
                temp_env::with_var("PATH", Some(fake_bin.path().as_os_str()), || {
                    hash_implementation_inputs(workspace.path(), "domain")
                })
                .is_err()
            );
        });
    }
}
