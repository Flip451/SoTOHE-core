#[cfg(target_os = "linux")]
use std::io::Read as _;
#[cfg(target_os = "linux")]
use std::os::unix::fs::MetadataExt as _;

#[cfg(target_os = "linux")]
use domain::review_v2::FilePath;
use domain::review_v2::{ReviewHash, ReviewTarget};
#[cfg(target_os = "linux")]
use sha2::Digest;
use usecase::review_v2::{ReviewHasher, ReviewHasherError};

use crate::git_cli::SystemGitRepo;

/// Fixed-size buffer used to hash review inputs without retaining their full
/// contents in memory.
#[cfg(target_os = "linux")]
const REVIEW_HASH_READ_BUFFER_BYTES: usize = 64 * 1024;
/// Maximum content admitted for one review input. This matches the established
/// bounded scan-file policy and limits both a static large file and a file
/// continuously appended while the review gate hashes it.
#[cfg(target_os = "linux")]
const MAX_REVIEW_HASH_FILE_BYTES: u64 = 8 * 1024 * 1024;

/// Review hasher that computes sorted-manifest SHA256 hashes from worktree files.
///
/// Ported from v1 `SystemGitHasher::group_scope_hash` with identical algorithm:
/// 1. Sort file paths alphabetically
/// 2. For each file: open with O_NOFOLLOW, verify within repo root, hash Git file mode and content
/// 3. Missing files → canonical tombstone entry
/// 4. Final manifest SHA256 → `"rvw1:sha256:<hex>"`
/// 5. Empty target → ReviewHash::Empty
pub struct SystemReviewHasher;

impl ReviewHasher for SystemReviewHasher {
    fn calc(&self, target: &ReviewTarget) -> Result<ReviewHash, ReviewHasherError> {
        if target.is_empty() {
            return Ok(ReviewHash::Empty);
        }

        let git = SystemGitRepo::discover()
            .map_err(|e| ReviewHasherError::Failed(format!("git discover: {e}")))?;
        calc_review_hash(&git, target)
    }
}

/// Repository-rooted review hasher for trusted-root state evaluation.
///
/// The public review-v2 composition retains [`SystemReviewHasher`]; this
/// crate-private variant prevents a state check from reading files below an
/// unrelated process CWD.
pub(crate) struct RootedSystemReviewHasher {
    git: SystemGitRepo,
}

impl RootedSystemReviewHasher {
    pub(crate) fn new(git: SystemGitRepo) -> Self {
        Self { git }
    }
}

impl ReviewHasher for RootedSystemReviewHasher {
    fn calc(&self, target: &ReviewTarget) -> Result<ReviewHash, ReviewHasherError> {
        calc_review_hash(&self.git, target)
    }
}

#[cfg(target_os = "linux")]
fn calc_review_hash(
    git: &SystemGitRepo,
    target: &ReviewTarget,
) -> Result<ReviewHash, ReviewHasherError> {
    if target.is_empty() {
        return Ok(ReviewHash::Empty);
    }

    let root = git.root().to_path_buf();

    // Sort for deterministic manifest
    let mut sorted: Vec<&FilePath> = target.files().iter().collect();
    sorted.sort();

    let mut manifest = Vec::new();
    for file_path in &sorted {
        let path = file_path.as_str();
        let abs_path = root.join(path);

        match open_nofollow_read(&abs_path) {
            Ok(mut file) => {
                // Reject non-regular files post-open (TOCTOU safe)
                let meta = file
                    .metadata()
                    .map_err(|e| ReviewHasherError::Failed(format!("stat {path}: {e}")))?;
                if !meta.is_file() {
                    return Err(ReviewHasherError::Failed(format!("not a regular file: {path}")));
                }
                verify_fd_within_root(&file, &root, path)?;

                let file_hash = hash_open_file(&mut file, path)?;
                append_file_manifest_entry(
                    &mut manifest,
                    path,
                    git_tracked_regular_file_mode(meta.mode()),
                    &file_hash,
                );
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                append_deleted_manifest_entry(&mut manifest, path);
            }
            Err(e) => {
                return Err(ReviewHasherError::Failed(format!("open {path}: {e}")));
            }
        }
    }

    let digest = sha2::Sha256::digest(&manifest);
    let hash_str = format!("rvw1:sha256:{digest:x}");
    ReviewHash::computed(hash_str)
        .map_err(|e| ReviewHasherError::Failed(format!("hash format: {e}")))
}

/// Normalizes a regular file's filesystem mode to Git's tracked file modes.
///
/// Git records only whether a regular file is executable, rather than host
/// umask-derived read/write permission bits. The caller has already rejected
/// non-regular files, so these two values also encode the admitted file type.
#[cfg(target_os = "linux")]
fn git_tracked_regular_file_mode(filesystem_mode: u32) -> u32 {
    if filesystem_mode & 0o100 == 0 { 0o100644 } else { 0o100755 }
}

/// Appends one unambiguous regular-file record. Path bytes, type, Git-tracked
/// mode, and content hash are NUL-delimited so control characters in a valid
/// Git path cannot collide with another record's encoding.
#[cfg(target_os = "linux")]
fn append_file_manifest_entry(manifest: &mut Vec<u8>, path: &str, mode: u32, file_hash: &str) {
    manifest.extend_from_slice(path.as_bytes());
    manifest.push(b'\0');
    manifest.extend_from_slice(b"regular");
    manifest.push(b'\0');
    manifest.extend_from_slice(format!("{mode:o}").as_bytes());
    manifest.push(b'\0');
    manifest.extend_from_slice(file_hash.as_bytes());
    manifest.push(b'\0');
}

/// Appends one unambiguous tombstone record.
#[cfg(target_os = "linux")]
fn append_deleted_manifest_entry(manifest: &mut Vec<u8>, path: &str) {
    manifest.extend_from_slice(path.as_bytes());
    manifest.push(b'\0');
    manifest.extend_from_slice(b"deleted");
    manifest.push(b'\0');
}

#[cfg(target_os = "linux")]
fn hash_open_file(file: &mut std::fs::File, scope_path: &str) -> Result<String, ReviewHasherError> {
    let mut hasher = sha2::Sha256::new();
    let mut buffer = [0_u8; REVIEW_HASH_READ_BUFFER_BYTES];
    let mut bytes_read = 0_u64;
    loop {
        // Once the limit has been consumed, one more byte distinguishes an
        // exact-limit file from an over-limit or continuously growing file.
        let remaining = MAX_REVIEW_HASH_FILE_BYTES.saturating_sub(bytes_read);
        let read_limit = if remaining == 0 {
            1
        } else {
            usize::try_from(remaining.min(buffer.len() as u64)).map_err(|error| {
                ReviewHasherError::Failed(format!(
                    "read {scope_path}: convert bounded read length: {error}"
                ))
            })?
        };
        let read_buffer = buffer.get_mut(..read_limit).ok_or_else(|| {
            ReviewHasherError::Failed(format!(
                "read {scope_path}: bounded read length exceeds the hash buffer"
            ))
        })?;
        let read = file
            .read(read_buffer)
            .map_err(|error| ReviewHasherError::Failed(format!("read {scope_path}: {error}")))?;
        if read == 0 {
            break;
        }
        bytes_read = bytes_read.saturating_add(read as u64);
        if bytes_read > MAX_REVIEW_HASH_FILE_BYTES {
            return Err(ReviewHasherError::Failed(format!(
                "review input exceeds the {MAX_REVIEW_HASH_FILE_BYTES}-byte limit: {scope_path}"
            )));
        }
        let chunk = buffer.get(..read).ok_or_else(|| {
            ReviewHasherError::Failed(format!(
                "read {scope_path}: reader returned an invalid byte count {read}"
            ))
        })?;
        hasher.update(chunk);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Fails closed on targets without an approved compile-time descriptor-path
/// verifier. Checking a pathname before opening is racy, so review-state gates
/// must not hash content whose opened-handle containment cannot be proven.
#[cfg(not(target_os = "linux"))]
fn calc_review_hash(
    _git: &SystemGitRepo,
    _target: &ReviewTarget,
) -> Result<ReviewHash, ReviewHasherError> {
    Err(ReviewHasherError::Failed(
        "review hashing is unsupported on this platform: safe handle containment verification is unavailable"
            .to_owned(),
    ))
}

/// Opens a file for reading, rejecting symlinks atomically via `O_NOFOLLOW`.
#[cfg(target_os = "linux")]
fn open_nofollow_read(path: &std::path::Path) -> Result<std::fs::File, std::io::Error> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
}

/// Verifies that an opened fd refers to a file inside `root`.
#[cfg(target_os = "linux")]
fn verify_fd_within_root(
    file: &std::fs::File,
    root: &std::path::Path,
    scope_path: &str,
) -> Result<(), ReviewHasherError> {
    crate::trusted_file::verify_opened_file_within_root(file, root).map_err(|error| {
        ReviewHasherError::Failed(format!(
            "fd containment verification unavailable for {scope_path}: {error}"
        ))
    })
}

#[cfg(all(test, target_os = "linux"))]
fn read_opened_fd_path(
    fd: std::os::unix::io::RawFd,
    scope_path: &str,
) -> Result<std::path::PathBuf, ReviewHasherError> {
    let proc_path = format!("/proc/self/fd/{fd}");
    std::fs::read_link(&proc_path).map_err(|error| {
        ReviewHasherError::Failed(format!(
            "fd containment verification unavailable for {scope_path}: {error}"
        ))
    })
}

#[cfg(all(test, target_os = "linux"))]
fn verify_resolved_fd_path_within_root(
    root: &std::path::Path,
    resolved_fd_path: &std::path::Path,
    scope_path: &str,
) -> Result<(), ReviewHasherError> {
    let canonical_root = root
        .canonicalize()
        .map_err(|error| ReviewHasherError::Failed(format!("canonicalize repo root: {error}")))?;
    if resolved_fd_path.starts_with(&canonical_root) {
        return Ok(());
    }
    Err(ReviewHasherError::Failed(format!(
        "scope path escapes repo root via symlink: {scope_path} (resolved to {})",
        resolved_fd_path.display()
    )))
}

#[cfg(all(test, target_os = "linux"))]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::fs::{self, File};

    use sha2::Digest as _;

    use super::{
        MAX_REVIEW_HASH_FILE_BYTES, REVIEW_HASH_READ_BUFFER_BYTES, append_file_manifest_entry,
        git_tracked_regular_file_mode, hash_open_file, read_opened_fd_path,
        verify_resolved_fd_path_within_root,
    };

    #[test]
    fn test_verify_resolved_fd_path_within_root_rejects_path_outside_root() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("outside.rs"), "outside").unwrap();

        let error = verify_resolved_fd_path_within_root(
            root.path(),
            &outside.path().join("outside.rs"),
            "linked/outside.rs",
        )
        .unwrap_err();

        assert!(error.to_string().contains("escapes repo root via symlink"));
    }

    #[test]
    fn test_hash_open_file_streams_content_larger_than_its_buffer() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("large.rs");
        let content = vec![b'x'; REVIEW_HASH_READ_BUFFER_BYTES + 1];
        fs::write(&path, &content).unwrap();

        let mut file = File::open(&path).unwrap();
        let hash = hash_open_file(&mut file, "large.rs").unwrap();

        assert_eq!(hash, format!("{:x}", sha2::Sha256::digest(&content)));
    }

    #[test]
    fn test_hash_open_file_content_exceeding_total_bound_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("oversized.rs");
        let file = File::create(&path).unwrap();
        file.set_len(MAX_REVIEW_HASH_FILE_BYTES + 1).unwrap();
        drop(file);

        let mut file = File::open(&path).unwrap();
        let error = hash_open_file(&mut file, "oversized.rs").unwrap_err();

        assert!(error.to_string().contains("exceeds the"));
    }

    #[test]
    fn test_read_opened_fd_path_fails_closed_when_descriptor_cannot_be_verified() {
        let error = read_opened_fd_path(-1, "unverifiable.rs").unwrap_err();

        assert!(error.to_string().contains("fd containment verification unavailable"));
    }

    #[test]
    fn test_manifest_entry_changes_when_executable_mode_changes() {
        let mut regular = Vec::new();
        append_file_manifest_entry(&mut regular, "script.rs", 0o100644, "content-hash");
        let mut executable = Vec::new();
        append_file_manifest_entry(&mut executable, "script.rs", 0o100755, "content-hash");

        assert_ne!(regular, executable);
    }

    #[test]
    fn test_git_tracked_regular_file_mode_ignores_host_permission_bits() {
        assert_eq!(git_tracked_regular_file_mode(0o100644), 0o100644);
        assert_eq!(git_tracked_regular_file_mode(0o100664), 0o100644);
        assert_eq!(git_tracked_regular_file_mode(0o100755), 0o100755);
    }

    #[test]
    fn test_git_tracked_regular_file_mode_uses_owner_execute_bit() {
        assert_eq!(git_tracked_regular_file_mode(0o100644), 0o100644);
        assert_eq!(git_tracked_regular_file_mode(0o100654), 0o100644);
        assert_eq!(git_tracked_regular_file_mode(0o100744), 0o100755);
    }
}
