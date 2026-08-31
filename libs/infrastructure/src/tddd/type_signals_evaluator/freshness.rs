//! Cache identity and bounded rustdoc-input fingerprint helpers.

use std::ffi::OsStr;
use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use domain::tddd::CargoFeatureName;
use domain::tddd::catalogue_v2::{CrateName, RustdocCratePort, RustdocCratePortError};
use domain::tddd::type_signals_doc::{
    RustdocExecutionIdentity, TypeSignalsAuthorityStatus, TypeSignalsCacheKey, TypeSignalsDocument,
    TypeSignalsReuseDecision, TypeSignalsReuseInput, TypeSignalsWorktreeStatus,
    decide_type_signals_reuse,
};

use sha2::Digest as _;

#[path = "environment_fingerprint.rs"]
mod environment_fingerprint;

/// Port-backed rustdoc provider used by the evaluator.
///
/// Rustdoc I/O remains owned by [`RustdocCratePort`]. The identity resolver is
/// only the cache-key side channel and does not read or construct a snapshot.
pub(crate) trait RustdocProvider: RustdocCratePort {
    fn execution_identity(
        &self,
        crate_name: &CrateName,
        features: &[CargoFeatureName],
    ) -> Result<RustdocExecutionIdentity, RustdocCratePortError>;
}

pub(crate) fn decide_reuse_for_recorded_document(
    recorded: Option<&TypeSignalsDocument>,
    current_key: &TypeSignalsCacheKey,
    worktree_clean: bool,
) -> TypeSignalsReuseDecision {
    let Some(recorded) = recorded else {
        return TypeSignalsReuseDecision::ReextractAndEvaluate;
    };
    let Some(input) = TypeSignalsReuseInput::verify(
        recorded.cache_key().clone(),
        current_key.clone(),
        if worktree_clean {
            TypeSignalsWorktreeStatus::Clean
        } else {
            TypeSignalsWorktreeStatus::Dirty
        },
        TypeSignalsAuthorityStatus::Readable,
    ) else {
        return TypeSignalsReuseDecision::ReextractAndEvaluate;
    };
    decide_type_signals_reuse(&input)
}

const MAX_RUSTDOC_INPUT_DEPTH: usize = 64;
const MAX_RUSTDOC_INPUT_ENTRIES: usize = 65_536;
const MAX_RUSTDOC_INPUT_FILES: usize = 32_768;
const MAX_RUSTDOC_INPUT_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_RUSTDOC_INPUT_TOTAL_BYTES: u64 = 512 * 1024 * 1024;
const MAX_RUSTDOC_INPUT_PATH_BYTES: usize = 16 * 1024;
const MAX_CARGO_METADATA_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
const RUSTDOC_INPUT_FINGERPRINT_VERSION: &[u8] = b"sotohe-rustdoc-input-fingerprint-v3\0";

/// A bounded fingerprint failure. No partial digest is returned for any case.
#[derive(Debug)]
pub(crate) enum RustdocInputFingerprintError {
    Io { path: PathBuf, source: String },
    Symlink { path: PathBuf },
    DirectoryDepth { path: PathBuf, maximum: usize },
    EntryCount { maximum: usize },
    FileCount { maximum: usize },
    FileBytes { path: PathBuf, bytes: u64, maximum: u64 },
    TotalBytes { bytes: u64, maximum: u64 },
    PathBytes { path: PathBuf, bytes: usize, maximum: usize },
    EnvironmentBytes { name: String, bytes: usize, maximum: usize },
}

impl std::fmt::Display for RustdocInputFingerprintError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(formatter, "cannot fingerprint '{}': {source}", path.display())
            }
            Self::Symlink { path } => {
                write!(formatter, "rustdoc input is a symlink: '{}'", path.display())
            }
            Self::DirectoryDepth { path, maximum } => write!(
                formatter,
                "rustdoc input directory depth exceeds {maximum}: '{}'",
                path.display()
            ),
            Self::EntryCount { maximum } => {
                write!(formatter, "rustdoc input entry count exceeds {maximum}")
            }
            Self::FileCount { maximum } => {
                write!(formatter, "rustdoc input file count exceeds {maximum}")
            }
            Self::FileBytes { path, bytes, maximum } => write!(
                formatter,
                "rustdoc input '{}' is {bytes} bytes; maximum is {maximum}",
                path.display()
            ),
            Self::TotalBytes { bytes, maximum } => {
                write!(formatter, "rustdoc input corpus is {bytes} bytes; maximum is {maximum}")
            }
            Self::PathBytes { path, bytes, maximum } => write!(
                formatter,
                "rustdoc input path '{}' is {bytes} bytes; maximum is {maximum}",
                path.display()
            ),
            Self::EnvironmentBytes { name, bytes, maximum } => write!(
                formatter,
                "rustdoc environment input '{name}' is {bytes} bytes; maximum is {maximum}"
            ),
        }
    }
}

/// Computes a complete, bounded implementation fingerprint.
pub(crate) fn rustdoc_input_fingerprint(
    workspace_root: &Path,
) -> Result<String, RustdocInputFingerprintError> {
    let cargo_inputs = validate_authoritative_cargo_inputs(workspace_root)?;
    let paths = collect_rustdoc_input_paths(workspace_root, &cargo_inputs.target_directory)?;
    let mut canonical = Vec::from(RUSTDOC_INPUT_FINGERPRINT_VERSION);
    append_len_prefixed_bytes(&mut canonical, b"cargo-metadata-no-deps-locked");
    append_len_prefixed_bytes(&mut canonical, &cargo_inputs.metadata_bytes);
    let mut total_bytes = 0_u64;
    for path in paths {
        let relative = path.strip_prefix(workspace_root).unwrap_or(&path);
        let metadata = std::fs::symlink_metadata(&path).map_err(|error| io_error(&path, error))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(RustdocInputFingerprintError::Symlink { path });
        }
        check_file_size(&path, metadata.len())?;
        let bytes = crate::tddd::tddd_catalogue_document_loader::read_optional_regular_file_bytes(
            &path,
            Some(workspace_root),
            MAX_RUSTDOC_INPUT_FILE_BYTES,
        )
        .map_err(|error| io_error(&path, error))?
        .ok_or_else(|| io_error(&path, Error::new(ErrorKind::NotFound, "input disappeared")))?;
        check_file_size(&path, bytes.len() as u64)?;
        let after = std::fs::symlink_metadata(&path).map_err(|error| io_error(&path, error))?;
        if after.file_type().is_symlink()
            || !after.is_file()
            || metadata_generation(&metadata) != metadata_generation(&after)
            || after.len() != bytes.len() as u64
        {
            return Err(io_error(
                &path,
                Error::other("rustdoc input changed while fingerprinting"),
            ));
        }
        total_bytes = total_bytes.checked_add(bytes.len() as u64).ok_or(
            RustdocInputFingerprintError::TotalBytes {
                bytes: u64::MAX,
                maximum: MAX_RUSTDOC_INPUT_TOTAL_BYTES,
            },
        )?;
        if total_bytes > MAX_RUSTDOC_INPUT_TOTAL_BYTES {
            return Err(RustdocInputFingerprintError::TotalBytes {
                bytes: total_bytes,
                maximum: MAX_RUSTDOC_INPUT_TOTAL_BYTES,
            });
        }
        append_len_prefixed_bytes(&mut canonical, &path_bytes(relative));
        append_len_prefixed_bytes(&mut canonical, &sha256_bytes(&bytes));
    }
    environment_fingerprint::append_environment_identity(&mut canonical, workspace_root)?;
    Ok(hex_digest(&sha256_bytes(&canonical)))
}

struct AuthoritativeCargoInputs {
    target_directory: PathBuf,
    metadata_bytes: Vec<u8>,
}

/// Captures Cargo's ordered metadata result and the target directory used by
/// the bounded workspace walk. The metadata bytes are part of the resulting
/// implementation fingerprint; the walk intentionally does not claim to be a
/// complete Cargo semantic-input model for external path dependencies or
/// build-script I/O.
fn validate_authoritative_cargo_inputs(
    workspace_root: &Path,
) -> Result<AuthoritativeCargoInputs, RustdocInputFingerprintError> {
    let manifest = workspace_root.join("Cargo.toml");
    let manifest_metadata =
        std::fs::symlink_metadata(&manifest).map_err(|error| io_error(&manifest, error))?;
    if manifest_metadata.file_type().is_symlink() || !manifest_metadata.is_file() {
        return Err(RustdocInputFingerprintError::Symlink { path: manifest });
    }
    let mut command = Command::new("cargo");
    command
        .args(["metadata", "--format-version", "1", "--no-deps", "--locked"])
        .current_dir(workspace_root);
    let output = crate::capability_exec::process::run_command_with_bounded_output(
        &mut command,
        MAX_CARGO_METADATA_OUTPUT_BYTES,
        Duration::from_secs(120),
        "cargo metadata for rustdoc input validation",
    )
    .map_err(|error| io_error(workspace_root, Error::other(error.to_string())))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(io_error(
            workspace_root,
            Error::other(format!(
                "cargo metadata exited with {}: {}",
                output.status,
                stderr.trim()
            )),
        ));
    }
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|error| {
        io_error(workspace_root, Error::other(format!("cargo metadata JSON is invalid: {error}")))
    })?;
    let target_directory = metadata
        .get("target_directory")
        .and_then(serde_json::Value::as_str)
        .map(PathBuf::from)
        .ok_or_else(|| {
            io_error(
                workspace_root,
                Error::other("cargo metadata has no authoritative target_directory"),
            )
        })?;
    let target_directory = if target_directory.is_absolute() {
        target_directory
    } else {
        workspace_root.join(target_directory)
    };
    let target_directory = crate::verify::path_safety::lexical_normalize(&target_directory);
    crate::track::symlink_guard::reject_symlinks_up_to_root(&target_directory)
        .map_err(|error| io_error(&target_directory, error))?;
    Ok(AuthoritativeCargoInputs { target_directory, metadata_bytes: output.stdout })
}

fn collect_rustdoc_input_paths(
    workspace_root: &Path,
    cargo_target_dir: &Path,
) -> Result<Vec<PathBuf>, RustdocInputFingerprintError> {
    let mut paths = Vec::new();
    walk_rustdoc_inputs(
        workspace_root,
        cargo_target_dir,
        workspace_root,
        0,
        &mut WalkState::default(),
        &mut paths,
    )?;
    paths.sort_by_key(|path| path_bytes(path.strip_prefix(workspace_root).unwrap_or(path)));
    Ok(paths)
}

#[derive(Default)]
struct WalkState {
    entries: usize,
    files: usize,
    total_bytes: u64,
}

fn walk_rustdoc_inputs(
    workspace_root: &Path,
    cargo_target_dir: &Path,
    directory: &Path,
    depth: usize,
    state: &mut WalkState,
    paths: &mut Vec<PathBuf>,
) -> Result<(), RustdocInputFingerprintError> {
    let entries = std::fs::read_dir(directory).map_err(|error| io_error(directory, error))?;
    for entry in entries {
        state.entries = state.entries.saturating_add(1);
        if state.entries > MAX_RUSTDOC_INPUT_ENTRIES {
            return Err(RustdocInputFingerprintError::EntryCount {
                maximum: MAX_RUSTDOC_INPUT_ENTRIES,
            });
        }
        let entry = entry.map_err(|error| io_error(directory, error))?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| io_error(&path, error))?;
        if file_type.is_dir() {
            if is_excluded_rustdoc_directory(workspace_root, cargo_target_dir, &path) {
                continue;
            }
            if depth >= MAX_RUSTDOC_INPUT_DEPTH {
                return Err(RustdocInputFingerprintError::DirectoryDepth {
                    path,
                    maximum: MAX_RUSTDOC_INPUT_DEPTH,
                });
            }
            walk_rustdoc_inputs(workspace_root, cargo_target_dir, &path, depth + 1, state, paths)?;
        } else if file_type.is_symlink() {
            if !is_excluded_rustdoc_directory(workspace_root, cargo_target_dir, &path) {
                return Err(RustdocInputFingerprintError::Symlink { path });
            }
        } else if file_type.is_file() {
            let metadata =
                std::fs::symlink_metadata(&path).map_err(|error| io_error(&path, error))?;
            let relative = path.strip_prefix(workspace_root).unwrap_or(&path);
            let length = path_bytes(relative).len();
            if length > MAX_RUSTDOC_INPUT_PATH_BYTES {
                return Err(RustdocInputFingerprintError::PathBytes {
                    path,
                    bytes: length,
                    maximum: MAX_RUSTDOC_INPUT_PATH_BYTES,
                });
            }
            check_file_size(&path, metadata.len())?;
            state.files = state.files.saturating_add(1);
            if state.files > MAX_RUSTDOC_INPUT_FILES {
                return Err(RustdocInputFingerprintError::FileCount {
                    maximum: MAX_RUSTDOC_INPUT_FILES,
                });
            }
            state.total_bytes = state.total_bytes.checked_add(metadata.len()).ok_or(
                RustdocInputFingerprintError::TotalBytes {
                    bytes: u64::MAX,
                    maximum: MAX_RUSTDOC_INPUT_TOTAL_BYTES,
                },
            )?;
            if state.total_bytes > MAX_RUSTDOC_INPUT_TOTAL_BYTES {
                return Err(RustdocInputFingerprintError::TotalBytes {
                    bytes: state.total_bytes,
                    maximum: MAX_RUSTDOC_INPUT_TOTAL_BYTES,
                });
            }
            paths.push(path);
        } else {
            return Err(io_error(&path, Error::other("rustdoc input is not a regular file")));
        }
    }
    Ok(())
}

fn is_excluded_rustdoc_directory(
    workspace_root: &Path,
    cargo_target_dir: &Path,
    path: &Path,
) -> bool {
    path == cargo_target_dir
        || (path.parent() == Some(workspace_root)
            && matches!(
                path.file_name().and_then(OsStr::to_str),
                Some(
                    ".git"
                        | ".harness"
                        | ".codex"
                        | ".claude"
                        | ".agents"
                        | ".cache"
                        | "track"
                        | "tmp",
                )
            ))
}

fn check_file_size(path: &Path, bytes: u64) -> Result<(), RustdocInputFingerprintError> {
    if bytes > MAX_RUSTDOC_INPUT_FILE_BYTES {
        Err(RustdocInputFingerprintError::FileBytes {
            path: path.to_path_buf(),
            bytes,
            maximum: MAX_RUSTDOC_INPUT_FILE_BYTES,
        })
    } else {
        Ok(())
    }
}

fn io_error(path: &Path, error: Error) -> RustdocInputFingerprintError {
    RustdocInputFingerprintError::Io { path: path.to_path_buf(), source: error.to_string() }
}

fn append_len_prefixed_bytes(output: &mut Vec<u8>, bytes: &[u8]) {
    output.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    output.extend_from_slice(bytes);
}

fn sha256_bytes(bytes: &[u8]) -> Vec<u8> {
    sha2::Sha256::digest(bytes).to_vec()
}

fn hex_digest(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn os_bytes(value: &OsStr) -> Vec<u8> {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt as _;
        value.as_bytes().to_vec()
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt as _;
        value.encode_wide().flat_map(u16::to_be_bytes).collect()
    }
    #[cfg(not(any(unix, windows)))]
    {
        value.to_string_lossy().into_owned().into_bytes()
    }
}

fn path_bytes(path: &Path) -> Vec<u8> {
    os_bytes(path.as_os_str())
}

fn metadata_generation(metadata: &std::fs::Metadata) -> Vec<u8> {
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0_u128, |duration| duration.as_nanos());
    let mut generation = modified.to_be_bytes().to_vec();
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        generation.extend_from_slice(&metadata.dev().to_be_bytes());
        generation.extend_from_slice(&metadata.ino().to_be_bytes());
        generation.extend_from_slice(&metadata.ctime().to_be_bytes());
        generation.extend_from_slice(&metadata.ctime_nsec().to_be_bytes());
    }
    generation
}
