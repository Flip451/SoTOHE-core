//! Cache identity and bounded rustdoc-input fingerprint helpers.

use std::ffi::OsStr;
use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use domain::tddd::CargoFeatureName;
use domain::tddd::catalogue_v2::{CrateName, RustdocCratePort, RustdocCratePortError};
use domain::tddd::type_signals_doc::{
    ImplementationFingerprint, RustdocExecutionIdentity, Sha256Digest, TypeSignalsAuthorityStatus,
    TypeSignalsCacheKey, TypeSignalsDocument, TypeSignalsReuseDecision, TypeSignalsReuseInput,
    TypeSignalsWorktreeStatus, decide_type_signals_reuse,
};

use sha2::Digest as _;

#[path = "environment_fingerprint.rs"]
mod environment_fingerprint;
#[path = "freshness/fingerprint_io.rs"]
mod fingerprint_io;
#[path = "freshness/workspace_root.rs"]
mod workspace_root;

use fingerprint_io::FingerprintDeadline;

pub(crate) const EVALUATION_START_EXECUTION_TIMEOUT: Duration = Duration::from_secs(120);
pub(crate) const EVALUATION_START_DRAIN_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Clone, Copy)]
pub(crate) struct EvaluationStartTimeouts {
    pub(crate) execution: Duration,
    pub(crate) drain: Duration,
}

impl EvaluationStartTimeouts {
    pub(crate) const fn new(execution: Duration, drain: Duration) -> Self {
        Self { execution, drain }
    }
}

impl Default for EvaluationStartTimeouts {
    fn default() -> Self {
        Self::new(EVALUATION_START_EXECUTION_TIMEOUT, EVALUATION_START_DRAIN_TIMEOUT)
    }
}

/// Port-backed rustdoc provider used by the evaluator.
///
/// Rustdoc I/O remains owned by [`RustdocCratePort`]. The identity resolver is
/// only the cache-key side channel and does not read or construct a snapshot.
pub(crate) trait RustdocProvider: RustdocCratePort {
    /// Captures one current rustdoc graph as an attested snapshot against the
    /// immutable fingerprint taken when the enclosing evaluation started.
    ///
    /// The provider must reject the export as authoritative input when either
    /// the pre-export or post-export implementation fingerprint differs from
    /// `evaluation_start`.
    fn capture_current_with_implementation_fingerprint(
        &self,
        crate_name: &CrateName,
        features: &[CargoFeatureName],
        evaluation_start: &ImplementationFingerprint,
    ) -> Result<domain::tddd::type_signals_doc::AttestedRustdocSnapshot, RustdocCratePortError>;

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

#[derive(Default)]
struct FingerprintBudget {
    files: usize,
    total_bytes: u64,
}

impl FingerprintBudget {
    fn reserve_file_count(&mut self) -> Result<(), RustdocInputFingerprintError> {
        let files = self
            .files
            .checked_add(1)
            .ok_or(RustdocInputFingerprintError::FileCount { maximum: MAX_RUSTDOC_INPUT_FILES })?;
        if files > MAX_RUSTDOC_INPUT_FILES {
            return Err(RustdocInputFingerprintError::FileCount {
                maximum: MAX_RUSTDOC_INPUT_FILES,
            });
        }
        self.files = files;
        Ok(())
    }

    fn reserve_file(&mut self, bytes: u64) -> Result<(), RustdocInputFingerprintError> {
        self.reserve_file_count()?;
        self.reserve_bytes(bytes)?;
        Ok(())
    }

    fn remaining_bytes(&self) -> u64 {
        MAX_RUSTDOC_INPUT_TOTAL_BYTES.saturating_sub(self.total_bytes)
    }

    fn reserve_bytes(&mut self, bytes: u64) -> Result<(), RustdocInputFingerprintError> {
        let total_bytes = self.total_bytes.checked_add(bytes).ok_or(
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
        self.total_bytes = total_bytes;
        Ok(())
    }
}

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
    TimedOut { operation: &'static str, maximum: Duration },
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
            Self::TimedOut { operation, maximum } => {
                write!(formatter, "evaluation-start {operation} timed out after {maximum:?}")
            }
        }
    }
}

fn rustdoc_input_digest(
    workspace_root: &Path,
    timeouts: EvaluationStartTimeouts,
) -> Result<Sha256Digest, RustdocInputFingerprintError> {
    let deadline = FingerprintDeadline::new(timeouts.execution);
    deadline.check("authoritative input capture")?;
    workspace_root::validate_workspace_root_for_fingerprint(workspace_root, &deadline)?;
    let mut budget = FingerprintBudget::default();
    let cargo_inputs = validate_authoritative_cargo_inputs(
        workspace_root,
        &mut budget,
        &deadline,
        timeouts.drain,
    )?;
    let paths = collect_rustdoc_input_paths(
        workspace_root,
        &cargo_inputs.target_directory,
        &mut budget,
        &deadline,
    )?;
    let mut canonical = Vec::from(RUSTDOC_INPUT_FINGERPRINT_VERSION);
    append_len_prefixed_bytes(&mut canonical, b"cargo-metadata-no-deps-locked");
    append_len_prefixed_bytes(&mut canonical, &cargo_inputs.metadata_bytes);
    for input in paths {
        deadline.check("workspace walk")?;
        let path = input.path;
        let relative = path.strip_prefix(workspace_root).unwrap_or(&path);
        let metadata_path = path.clone();
        let metadata = deadline.run_io("workspace walk", path.clone(), move || {
            std::fs::symlink_metadata(&metadata_path)
        })?;
        deadline.check("workspace walk")?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(RustdocInputFingerprintError::Symlink { path });
        }
        check_file_size(&path, metadata.len())?;
        if metadata.len() != input.discovered_bytes
            || metadata_generation(&metadata) != input.discovered_generation
        {
            return Err(io_error(
                &path,
                Error::other("rustdoc input changed between discovery and fingerprint read"),
            ));
        }
        budget.reserve_bytes(metadata.len())?;
        deadline.check("workspace walk")?;
        let read_path = path.clone();
        let trusted_root = workspace_root.to_owned();
        let maximum_bytes = metadata.len();
        let bytes = deadline
            .run_io("workspace walk", path.clone(), move || {
                crate::tddd::tddd_catalogue_document_loader::read_optional_regular_file_bytes(
                    &read_path,
                    Some(&trusted_root),
                    maximum_bytes,
                )
            })?
            .ok_or_else(|| io_error(&path, Error::new(ErrorKind::NotFound, "input disappeared")))?;
        deadline.check("workspace walk")?;
        check_file_size(&path, bytes.len() as u64)?;
        let after_path = path.clone();
        let after = deadline.run_io("workspace walk", path.clone(), move || {
            std::fs::symlink_metadata(&after_path)
        })?;
        deadline.check("workspace walk")?;
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
        append_len_prefixed_bytes(&mut canonical, &path_bytes(relative));
        append_len_prefixed_bytes(&mut canonical, &sha256_bytes(&bytes));
    }
    environment_fingerprint::append_environment_identity(
        &mut canonical,
        workspace_root,
        &mut budget,
        &deadline,
        timeouts.drain,
    )?;
    deadline.check("fingerprint assembly")?;
    Sha256Digest::try_new(hex_digest(&sha256_bytes(&canonical))).map_err(|error| {
        io_error(workspace_root, Error::other(format!("invalid fingerprint: {error}")))
    })
}

/// Computes a complete, bounded implementation fingerprint.
#[cfg(test)]
pub(crate) fn rustdoc_input_fingerprint(
    workspace_root: &Path,
) -> Result<String, RustdocInputFingerprintError> {
    rustdoc_input_digest(workspace_root, EvaluationStartTimeouts::default())
        .map(|digest| digest.as_str().to_owned())
}

/// Computes the typed implementation identity used by evaluation-start and
/// per-export snapshot admission.
pub(crate) fn rustdoc_implementation_fingerprint(
    workspace_root: &Path,
) -> Result<ImplementationFingerprint, String> {
    rustdoc_implementation_fingerprint_with_timeouts(
        workspace_root,
        EvaluationStartTimeouts::default(),
    )
}

pub(crate) fn rustdoc_implementation_fingerprint_with_timeouts(
    workspace_root: &Path,
    timeouts: EvaluationStartTimeouts,
) -> Result<ImplementationFingerprint, String> {
    rustdoc_input_digest(workspace_root, timeouts)
        .map(ImplementationFingerprint::new)
        .map_err(|error| error.to_string())
}

struct AuthoritativeCargoInputs {
    target_directory: PathBuf,
    metadata_bytes: Vec<u8>,
}

struct RustdocInputPath {
    path: PathBuf,
    discovered_generation: Vec<u8>,
    discovered_bytes: u64,
}

/// Captures Cargo's ordered metadata result and the target directory used by
/// the bounded workspace walk. The metadata bytes are part of the resulting
/// implementation fingerprint; the walk intentionally does not claim to be a
/// complete Cargo semantic-input model for external path dependencies or
/// build-script I/O.
fn validate_authoritative_cargo_inputs(
    workspace_root: &Path,
    budget: &mut FingerprintBudget,
    deadline: &FingerprintDeadline,
    drain_timeout: Duration,
) -> Result<AuthoritativeCargoInputs, RustdocInputFingerprintError> {
    let manifest = workspace_root.join("Cargo.toml");
    deadline.check("cargo metadata")?;
    let manifest_metadata_path = manifest.clone();
    let manifest_metadata = deadline.run_io("cargo metadata", manifest.clone(), move || {
        std::fs::symlink_metadata(&manifest_metadata_path)
    })?;
    deadline.check("cargo metadata")?;
    if manifest_metadata.file_type().is_symlink() || !manifest_metadata.is_file() {
        return Err(RustdocInputFingerprintError::Symlink { path: manifest });
    }
    let mut command = Command::new("cargo");
    command
        .args(["metadata", "--format-version", "1", "--no-deps", "--locked"])
        .current_dir(workspace_root);
    let execution_timeout = deadline.remaining("cargo metadata")?;
    let output = environment_fingerprint::run_cargo_metadata_with_early_bounded_output(
        command,
        MAX_CARGO_METADATA_OUTPUT_BYTES,
        budget.remaining_bytes(),
        execution_timeout,
        drain_timeout,
    )
    .map_err(|error| io_error(workspace_root, Error::other(error.to_string())))?;
    deadline.check("cargo metadata")?;
    let output_bytes = output.stdout.len().checked_add(output.stderr.len()).ok_or(
        RustdocInputFingerprintError::TotalBytes {
            bytes: u64::MAX,
            maximum: MAX_RUSTDOC_INPUT_TOTAL_BYTES,
        },
    )?;
    budget.reserve_bytes(output_bytes as u64)?;
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
    deadline.check("cargo metadata")?;
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).map_err(|error| {
        io_error(workspace_root, Error::other(format!("cargo metadata JSON is invalid: {error}")))
    })?;
    deadline.check("cargo metadata")?;
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
    deadline.check("cargo metadata")?;
    let target_directory_for_guard = target_directory.clone();
    deadline.run_io("cargo metadata", target_directory.clone(), move || {
        crate::track::symlink_guard::reject_symlinks_up_to_root(&target_directory_for_guard)
    })?;
    deadline.check("cargo metadata")?;
    Ok(AuthoritativeCargoInputs { target_directory, metadata_bytes: output.stdout })
}

fn collect_rustdoc_input_paths(
    workspace_root: &Path,
    cargo_target_dir: &Path,
    budget: &mut FingerprintBudget,
    deadline: &FingerprintDeadline,
) -> Result<Vec<RustdocInputPath>, RustdocInputFingerprintError> {
    let mut paths = Vec::new();
    let mut state = WalkState::default();
    let mut context = WalkContext {
        workspace_root,
        cargo_target_dir,
        state: &mut state,
        budget,
        paths: &mut paths,
        deadline,
    };
    walk_rustdoc_inputs(workspace_root, 0, &mut context)?;
    deadline.check("workspace walk")?;
    paths.sort_by_key(|input| {
        path_bytes(input.path.strip_prefix(workspace_root).unwrap_or(&input.path))
    });
    Ok(paths)
}

#[derive(Default)]
struct WalkState {
    entries: usize,
}

struct WalkContext<'a> {
    workspace_root: &'a Path,
    cargo_target_dir: &'a Path,
    state: &'a mut WalkState,
    budget: &'a mut FingerprintBudget,
    paths: &'a mut Vec<RustdocInputPath>,
    deadline: &'a FingerprintDeadline,
}

fn walk_rustdoc_inputs(
    directory: &Path,
    depth: usize,
    context: &mut WalkContext<'_>,
) -> Result<(), RustdocInputFingerprintError> {
    context.deadline.check("workspace walk")?;
    let directory_path = directory.to_owned();
    let entries = context.deadline.run_io("workspace walk", directory_path.clone(), move || {
        let mut entries = std::fs::read_dir(&directory_path)?;
        let mut collected = Vec::new();
        while collected.len() <= MAX_RUSTDOC_INPUT_ENTRIES {
            let Some(entry) = entries.next() else {
                break;
            };
            collected.push(entry?);
        }
        Ok(collected)
    })?;
    for entry in entries {
        context.deadline.check("workspace walk")?;
        context.state.entries = context.state.entries.saturating_add(1);
        if context.state.entries > MAX_RUSTDOC_INPUT_ENTRIES {
            return Err(RustdocInputFingerprintError::EntryCount {
                maximum: MAX_RUSTDOC_INPUT_ENTRIES,
            });
        }
        let path = entry.path();
        let file_type =
            context.deadline.run_io("workspace walk", path.clone(), move || entry.file_type())?;
        context.deadline.check("workspace walk")?;
        if file_type.is_dir() {
            if is_excluded_rustdoc_directory(
                context.workspace_root,
                context.cargo_target_dir,
                &path,
            ) {
                continue;
            }
            if depth >= MAX_RUSTDOC_INPUT_DEPTH {
                return Err(RustdocInputFingerprintError::DirectoryDepth {
                    path,
                    maximum: MAX_RUSTDOC_INPUT_DEPTH,
                });
            }
            walk_rustdoc_inputs(&path, depth + 1, context)?;
        } else if file_type.is_symlink() {
            if !is_excluded_rustdoc_directory(
                context.workspace_root,
                context.cargo_target_dir,
                &path,
            ) {
                return Err(RustdocInputFingerprintError::Symlink { path });
            }
        } else if file_type.is_file() {
            let metadata_path = path.clone();
            let metadata = context.deadline.run_io("workspace walk", path.clone(), move || {
                std::fs::symlink_metadata(&metadata_path)
            })?;
            context.deadline.check("workspace walk")?;
            let relative = path.strip_prefix(context.workspace_root).unwrap_or(&path);
            let length = path_bytes(relative).len();
            if length > MAX_RUSTDOC_INPUT_PATH_BYTES {
                return Err(RustdocInputFingerprintError::PathBytes {
                    path,
                    bytes: length,
                    maximum: MAX_RUSTDOC_INPUT_PATH_BYTES,
                });
            }
            check_file_size(&path, metadata.len())?;
            context.budget.reserve_file_count()?;
            context.paths.push(RustdocInputPath {
                discovered_generation: metadata_generation(&metadata),
                discovered_bytes: metadata.len(),
                path,
            });
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
