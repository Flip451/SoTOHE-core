//! POSIX-atomic git index update via private copy + rename.
//!
//! [`PrivateIndex`] copies the real `.git/index` to a temp file, performs all
//! staging operations on that private copy, and only atomically renames it
//! over the real index when every step has succeeded.

use std::path::PathBuf;

use super::{GitError, GitRepository};

/// A private copy of the git index used to stage files without touching the
/// real `.git/index` until all operations succeed.
///
/// All git commands that read or modify the index operate on `temp_path`
/// via `GIT_INDEX_FILE`.  When every step has succeeded, [`swap_into_real`]
/// atomically renames `temp_path` over `real_index_path`.  If the
/// `PrivateIndex` is dropped before [`swap_into_real`] is called, the temp
/// file is removed and the real index is left completely untouched.
///
/// [`swap_into_real`]: PrivateIndex::swap_into_real
pub struct PrivateIndex {
    temp_path: PathBuf,
    real_index_path: PathBuf,
    swapped: bool,
}

impl PrivateIndex {
    /// Copy the current git index to a temp file in the same directory as the
    /// real index (required for `fs::rename` to be atomic on POSIX).
    ///
    /// Uses `git rev-parse --git-path index` to find the real index path so
    /// that linked worktrees are handled correctly.
    pub fn from_current(git: &impl GitRepository) -> Result<Self, String> {
        // Resolve the real index path (worktree-safe).
        let real_index_path = resolve_real_index_path(git)?;

        // Place the temp file in the same directory so rename is atomic.
        let temp_dir = real_index_path.parent().ok_or_else(|| {
            format!("git index path has no parent directory: {}", real_index_path.display())
        })?;
        let temp_path = temp_dir.join(format!(
            "sotp-private-index-{}-{}.tmp",
            std::process::id(),
            // Pointer address of a stack local used as a secondary disambiguator.
            // This is stable within a single call site and avoids collisions
            // when multiple PrivateIndex values are alive concurrently.
            {
                let marker: u8 = 0;
                std::ptr::from_ref(&marker) as usize
            }
        ));

        std::fs::copy(&real_index_path, &temp_path).map_err(|e| {
            format!(
                "failed to copy git index {} -> {}: {e}",
                real_index_path.display(),
                temp_path.display()
            )
        })?;

        Ok(Self { temp_path, real_index_path, swapped: false })
    }

    /// Compute the normalized tree hash from the private index.
    ///
    /// The normalization is identical to `index_tree_hash_normalizing` in
    /// `git_cli/mod.rs`: `review.code_hash` is set to `"PENDING"` and
    /// `updated_at` is set to the Unix epoch.  A second temp copy of the
    /// private index is used for the write-tree operation so the private
    /// index itself is not modified.
    #[allow(clippy::too_many_lines)]
    pub fn normalized_tree_hash(
        &self,
        git: &impl GitRepository,
        metadata_path: &str,
    ) -> Result<String, GitError> {
        use std::io::Write as _;

        // Step 1: Read the metadata.json blob from the private index.
        let show_output = std::process::Command::new("git")
            .args(["show", &format!(":{metadata_path}")])
            .current_dir(git.root())
            .env("GIT_INDEX_FILE", &self.temp_path)
            .output()
            .map_err(|source| GitError::Spawn {
                command: format!("show :{metadata_path}"),
                source,
            })?;
        if !show_output.status.success() {
            let stderr = String::from_utf8_lossy(&show_output.stderr).trim().to_owned();
            let code = show_output.status.code().unwrap_or(-1);
            return Err(GitError::CommandFailed {
                command: format!("show :{metadata_path}"),
                code,
                stderr,
            });
        }
        let blob_content = String::from_utf8_lossy(&show_output.stdout);

        // Step 2: Parse as JSON.
        let mut json: serde_json::Value =
            serde_json::from_str(&blob_content).map_err(|e| GitError::CommandFailed {
                command: format!("parse {metadata_path}"),
                code: -1,
                stderr: e.to_string(),
            })?;

        // Step 3: Normalize volatile fields.
        if let serde_json::Value::Object(obj) = &mut json {
            obj.insert(
                "updated_at".to_owned(),
                serde_json::Value::String("1970-01-01T00:00:00Z".to_owned()),
            );
            let review = obj
                .entry("review")
                .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
            if let serde_json::Value::Object(review_obj) = review {
                review_obj.insert(
                    "code_hash".to_owned(),
                    serde_json::Value::String("PENDING".to_owned()),
                );
            }
        }

        // Step 4: Serialize deterministically.
        let normalized =
            serde_json::to_string_pretty(&json).map_err(|e| GitError::CommandFailed {
                command: format!("serialize {metadata_path}"),
                code: -1,
                stderr: e.to_string(),
            })?;

        // Step 5: Write normalized blob to object store.
        let mut hash_object_child = std::process::Command::new("git")
            .args(["hash-object", "-w", "--stdin"])
            .current_dir(git.root())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|source| GitError::Spawn {
                command: "hash-object -w --stdin".to_owned(),
                source,
            })?;
        if let Some(ref mut stdin) = hash_object_child.stdin {
            stdin.write_all(normalized.as_bytes()).map_err(|source| GitError::Spawn {
                command: "hash-object write stdin".to_owned(),
                source,
            })?;
        }
        let hash_object_output = hash_object_child.wait_with_output().map_err(|source| {
            GitError::Spawn { command: "hash-object -w --stdin (wait)".to_owned(), source }
        })?;
        if !hash_object_output.status.success() {
            let stderr = String::from_utf8_lossy(&hash_object_output.stderr).trim().to_owned();
            let code = hash_object_output.status.code().unwrap_or(-1);
            return Err(GitError::CommandFailed {
                command: "hash-object -w --stdin".to_owned(),
                code,
                stderr,
            });
        }
        let blob_hash = String::from_utf8_lossy(&hash_object_output.stdout).trim().to_owned();

        // Step 6: Copy private index to a second temp file for write-tree
        // (write-tree modifies internal index state, so we must not use
        // self.temp_path directly).
        let norm_temp = self.temp_path.with_extension("norm.tmp");
        std::fs::copy(&self.temp_path, &norm_temp).map_err(|source| GitError::Spawn {
            command: "copy private index for write-tree".to_owned(),
            source,
        })?;

        // Step 7: Update the norm copy with the normalized blob.
        let update_output = std::process::Command::new("git")
            .args(["update-index", "--cacheinfo", &format!("100644,{blob_hash},{metadata_path}")])
            .current_dir(git.root())
            .env("GIT_INDEX_FILE", &norm_temp)
            .output()
            .map_err(|source| GitError::Spawn {
                command: "update-index --cacheinfo (norm)".to_owned(),
                source,
            })?;
        if !update_output.status.success() {
            let _ = std::fs::remove_file(&norm_temp);
            let stderr = String::from_utf8_lossy(&update_output.stderr).trim().to_owned();
            let code = update_output.status.code().unwrap_or(-1);
            return Err(GitError::CommandFailed {
                command: "update-index --cacheinfo (norm)".to_owned(),
                code,
                stderr,
            });
        }

        // Step 8: Write tree from the norm copy.
        let write_tree_output = std::process::Command::new("git")
            .args(["write-tree"])
            .current_dir(git.root())
            .env("GIT_INDEX_FILE", &norm_temp)
            .output()
            .map_err(|source| GitError::Spawn {
                command: "write-tree (private-norm)".to_owned(),
                source,
            })?;

        // Step 9: Clean up the norm copy unconditionally.
        let _ = std::fs::remove_file(&norm_temp);

        if !write_tree_output.status.success() {
            let stderr = String::from_utf8_lossy(&write_tree_output.stderr).trim().to_owned();
            let code = write_tree_output.status.code().unwrap_or(-1);
            return Err(GitError::CommandFailed {
                command: "write-tree (private-norm)".to_owned(),
                code,
                stderr,
            });
        }

        Ok(String::from_utf8_lossy(&write_tree_output.stdout).trim().to_owned())
    }

    /// Stage raw bytes as a blob for the given repo-relative path in the private index.
    ///
    /// Feeds `content` to `git hash-object -w --stdin` to write a blob to the
    /// object store, then updates the private index entry with
    /// `git update-index --cacheinfo`.
    pub fn stage_bytes(
        &self,
        git: &impl GitRepository,
        rel_path: &str,
        content: &[u8],
    ) -> Result<(), String> {
        use std::io::Write as _;

        // Step 1: Write blob to object store via stdin.
        let mut child = std::process::Command::new("git")
            .args(["hash-object", "-w", "--stdin"])
            .current_dir(git.root())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("failed to spawn git hash-object for {rel_path}: {e}"))?;

        if let Some(ref mut stdin) = child.stdin {
            stdin
                .write_all(content)
                .map_err(|e| format!("failed to write content to hash-object stdin: {e}"))?;
        }

        let hash_output = child
            .wait_with_output()
            .map_err(|e| format!("failed to wait for git hash-object: {e}"))?;
        if !hash_output.status.success() {
            let stderr = String::from_utf8_lossy(&hash_output.stderr).trim().to_owned();
            return Err(format!("git hash-object failed for {rel_path}: {stderr}"));
        }
        let blob_hash = String::from_utf8_lossy(&hash_output.stdout).trim().to_owned();

        // Step 2: Update the private index entry.
        // Always pass --add so that brand-new files (e.g. review.json on first
        // record-round) are accepted. For existing files --add is a no-op.
        let update_output = std::process::Command::new("git")
            .args([
                "update-index",
                "--add",
                "--cacheinfo",
                &format!("100644,{blob_hash},{rel_path}"),
            ])
            .current_dir(git.root())
            .env("GIT_INDEX_FILE", &self.temp_path)
            .output()
            .map_err(|e| format!("failed to update-index for {rel_path}: {e}"))?;
        if !update_output.status.success() {
            let stderr = String::from_utf8_lossy(&update_output.stderr).trim().to_owned();
            return Err(format!("git update-index failed for {rel_path}: {stderr}"));
        }

        Ok(())
    }

    /// Atomically replace the real index using git's own lockfile protocol.
    ///
    /// 1. Create `<index>.lock` with `O_CREAT|O_EXCL` — blocks concurrent
    ///    git operations (they also use this lock before touching the index).
    /// 2. Copy our private index content into the lock file.
    /// 3. Rename `<index>.lock` → `<index>` — atomic on POSIX, and this is
    ///    exactly how git itself commits index changes.
    ///
    /// The lock is held only for the copy+rename duration (microseconds),
    /// so it does not interfere with our earlier `git hash-object` /
    /// `git update-index` calls which operate on the private index.
    pub fn swap_into_real(mut self) -> Result<(), String> {
        let lock_path = self.real_index_path.with_extension("lock");
        // Acquire git's index lock — fails if another git operation holds it.
        std::fs::OpenOptions::new().write(true).create_new(true).open(&lock_path).map_err(|e| {
            format!(
                "failed to acquire index lock at {}: {e}. \
                     A concurrent git operation may be in progress.",
                lock_path.display()
            )
        })?;
        // Copy private index content into the lock file.
        if let Err(e) = std::fs::copy(&self.temp_path, &lock_path) {
            let _ = std::fs::remove_file(&lock_path);
            return Err(format!("failed to write index lock: {e}"));
        }
        // Atomic rename: lock file becomes the real index.
        // This is git's own commit protocol for index updates.
        if let Err(e) = std::fs::rename(&lock_path, &self.real_index_path) {
            let _ = std::fs::remove_file(&lock_path);
            return Err(format!("failed to rename index lock to index: {e}"));
        }
        // Clean up the temp private index.
        let _ = std::fs::remove_file(&self.temp_path);
        self.swapped = true;
        Ok(())
    }
}

impl Drop for PrivateIndex {
    fn drop(&mut self) {
        if !self.swapped {
            let _ = std::fs::remove_file(&self.temp_path);
        }
    }
}

/// Resolve the absolute path of the real git index, worktree-safe.
///
/// Uses `git rev-parse --git-path index` which correctly resolves the index
/// path even in linked worktrees where `.git` is a pointer file.
///
/// # Errors
///
/// Returns `Err(String)` if the git command fails or returns an empty path.
pub fn resolve_real_index_path(git: &impl GitRepository) -> Result<PathBuf, String> {
    let output = git
        .output(&["rev-parse", "--git-path", "index"])
        .map_err(|e| format!("failed to resolve git index path: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        return Err(format!("git rev-parse --git-path index failed: {stderr}"));
    }
    let resolved = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let path = if std::path::Path::new(&resolved).is_absolute() {
        PathBuf::from(resolved)
    } else {
        git.root().join(resolved)
    };
    Ok(path)
}
