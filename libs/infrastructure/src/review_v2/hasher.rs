use std::io::Read as _;

use domain::review_v2::{FilePath, ReviewHash, ReviewTarget};
use sha2::Digest;
use usecase::review_v2::{ReviewHasher, ReviewHasherError};

use crate::git_cli::{GitRepository, SystemGitRepo};

/// Review hasher that computes sorted-manifest SHA256 hashes from worktree files.
///
/// Ported from v1 `SystemGitHasher::group_scope_hash` with identical algorithm:
/// 1. Sort file paths alphabetically
/// 2. For each file: open with O_NOFOLLOW, verify within repo root, hash content
/// 3. Missing files → tombstone entry ("path\tDELETED\n")
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
        let root = git.root().to_path_buf();

        // Sort for deterministic manifest
        let mut sorted: Vec<&FilePath> = target.files().iter().collect();
        sorted.sort();

        let mut manifest = String::new();
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
                        return Err(ReviewHasherError::Failed(format!(
                            "not a regular file: {path}"
                        )));
                    }
                    // Post-open repo root verification
                    verify_fd_within_root(&file, &root, path)?;

                    let mut bytes = Vec::new();
                    file.read_to_end(&mut bytes)
                        .map_err(|e| ReviewHasherError::Failed(format!("read {path}: {e}")))?;
                    let file_hash = sha2::Sha256::digest(&bytes);
                    manifest.push_str(&format!("{path}\t{file_hash:x}\n"));
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    manifest.push_str(&format!("{path}\tDELETED\n"));
                }
                Err(e) => {
                    return Err(ReviewHasherError::Failed(format!("open {path}: {e}")));
                }
            }
        }

        let digest = sha2::Sha256::digest(manifest.as_bytes());
        let hash_str = format!("rvw1:sha256:{digest:x}");
        ReviewHash::computed(hash_str)
            .map_err(|e| ReviewHasherError::Failed(format!("hash format: {e}")))
    }
}

/// Opens a file for reading, rejecting symlinks atomically via `O_NOFOLLOW`.
fn open_nofollow_read(path: &std::path::Path) -> Result<std::fs::File, std::io::Error> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW).open(path)
    }
    #[cfg(not(unix))]
    {
        match std::fs::symlink_metadata(path) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("path is a symlink: {}", path.display()),
                ));
            }
            Ok(_) | Err(_) => {}
        }
        std::fs::File::open(path)
    }
}

/// Verifies that an opened fd refers to a file inside `root`.
fn verify_fd_within_root(
    file: &std::fs::File,
    root: &std::path::Path,
    scope_path: &str,
) -> Result<(), ReviewHasherError> {
    #[cfg(unix)]
    {
        use std::os::unix::io::AsRawFd;
        let fd = file.as_raw_fd();
        let proc_path = format!("/proc/self/fd/{fd}");
        match std::fs::read_link(&proc_path) {
            Ok(real_path) => {
                let canon_root = root.canonicalize().map_err(|e| {
                    ReviewHasherError::Failed(format!("canonicalize repo root: {e}"))
                })?;
                if !real_path.starts_with(&canon_root) {
                    return Err(ReviewHasherError::Failed(format!(
                        "scope path escapes repo root via symlink: {scope_path} (resolved to {})",
                        real_path.display()
                    )));
                }
            }
            Err(_) => {
                // /proc not available (macOS, FreeBSD) — fallback to canonicalize check
                let abs_path = root.join(scope_path);
                if let Ok(resolved) = abs_path.canonicalize() {
                    let canon_root = root.canonicalize().map_err(|e| {
                        ReviewHasherError::Failed(format!("canonicalize repo root: {e}"))
                    })?;
                    if !resolved.starts_with(&canon_root) {
                        return Err(ReviewHasherError::Failed(format!(
                            "scope path escapes repo root via symlink: {scope_path}"
                        )));
                    }
                }
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (file, root, scope_path);
    }
    Ok(())
}
