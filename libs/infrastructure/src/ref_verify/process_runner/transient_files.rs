//! Private transient-file support for the ref-verifier process runner.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use usecase::ref_verify::RefVerifyError;

use crate::codex_common::REVIEW_RUNTIME_DIR;

/// RAII handle for a transient file under `tmp/reviewer-runtime/`.
///
/// On drop, the file is removed best-effort. Errors during removal are
/// swallowed because the dropper has no way to surface them and a stale
/// transient file is harmless.
pub(super) struct AutoCleanupFile {
    path: PathBuf,
}

impl AutoCleanupFile {
    pub(super) fn create(
        project_root: &Path,
        prefix: &str,
        ext: &str,
        content: &[u8],
    ) -> Result<Self, RefVerifyError> {
        let canon_root = project_root.canonicalize().map_err(|error| {
            runner_error(format!(
                "cannot canonicalize project root '{}': {error}",
                project_root.display()
            ))
        })?;
        let path = ref_verify_runtime_path(project_root, prefix, ext)?;
        // Use `create_new` so a raced symlink planted after the directory guard cannot redirect
        // the file write to an existing path outside the tree.
        let mut file =
            std::fs::OpenOptions::new().write(true).create_new(true).open(&path).map_err(
                |error| {
                    runner_error(format!(
                        "failed to create transient file '{}': {error}",
                        path.display()
                    ))
                },
            )?;
        // Post-creation guard: verify the opened file resolves within the project root.
        // `canonicalize` on the newly-created path cannot follow a symlink placed after
        // `create_new` succeeded (the file now exists at the inode we created), but it does
        // resolve any symlink in the parent-directory ancestry.
        let canon_path = path.canonicalize().map_err(|error| {
            runner_error(format!(
                "cannot canonicalize transient file '{}': {error}",
                path.display()
            ))
        })?;
        if !canon_path.starts_with(&canon_root) {
            let _ = std::fs::remove_file(&path);
            return Err(runner_error(format!(
                "transient file '{}' resolves to '{}' which escapes project root '{}'",
                path.display(),
                canon_path.display(),
                canon_root.display()
            )));
        }
        if !content.is_empty() {
            file.write_all(content).map_err(|error| {
                runner_error(format!(
                    "failed to write transient file '{}': {error}",
                    path.display()
                ))
            })?;
        }
        Ok(Self { path })
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for AutoCleanupFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn ref_verify_runtime_path(
    project_root: &Path,
    prefix: &str,
    ext: &str,
) -> Result<PathBuf, RefVerifyError> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let canon_root = project_root.canonicalize().map_err(|error| {
        runner_error(format!(
            "cannot canonicalize project root '{}': {error}",
            project_root.display()
        ))
    })?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| runner_error(format!("failed to compute timestamp: {error}")))?
        .as_nanos();
    let sequence = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = project_root
        .join(REVIEW_RUNTIME_DIR)
        .join(format!("{prefix}-{}-{timestamp}-{sequence}.{ext}", std::process::id()));
    let parent = path
        .parent()
        .ok_or_else(|| runner_error(format!("runtime path has no parent: '{}'", path.display())))?;
    std::fs::create_dir_all(parent).map_err(|error| {
        runner_error(format!("failed to create '{}': {error}", parent.display()))
    })?;
    // Guard: verify the created directory resolves within the canonical project root.
    // This catches pre-existing symlinks on `tmp` or `reviewer-runtime` that would redirect
    // writes outside the trusted tree.
    let canon_parent = parent.canonicalize().map_err(|error| {
        runner_error(format!("cannot canonicalize runtime dir '{}': {error}", parent.display()))
    })?;
    if !canon_parent.starts_with(&canon_root) {
        return Err(runner_error(format!(
            "runtime dir '{}' resolves to '{}' which escapes project root '{}'",
            parent.display(),
            canon_parent.display(),
            canon_root.display()
        )));
    }
    Ok(path)
}

fn runner_error(message: impl Into<String>) -> RefVerifyError {
    RefVerifyError::VerifierPort { message: message.into() }
}
