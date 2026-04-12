use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde::Deserialize;
use thiserror::Error;

pub mod private_index;
pub(crate) mod show;

/// Structured error type for git CLI operations.
#[derive(Debug, Error)]
pub enum GitError {
    #[error("failed to determine current directory: {0}")]
    CurrentDir(#[source] std::io::Error),
    #[error("failed to run git {command}: {source}")]
    Spawn {
        command: String,
        #[source]
        source: std::io::Error,
    },
    #[error("git {command} failed (exit {code}): {stderr}")]
    CommandFailed { command: String, code: i32, stderr: String },
    #[error("git rev-parse --show-toplevel returned an empty path")]
    EmptyRepoRoot,
}

pub trait GitRepository {
    fn root(&self) -> &Path;
    fn status(&self, args: &[&str]) -> Result<i32, GitError>;
    fn output(&self, args: &[&str]) -> Result<Output, GitError>;

    fn resolve_path(&self, path: &Path) -> PathBuf {
        resolve_repo_path(self.root(), path)
    }

    fn current_branch(&self) -> Result<Option<String>, GitError> {
        let output = self.output(&["rev-parse", "--abbrev-ref", "HEAD"])?;
        if !output.status.success() {
            return Ok(None);
        }
        Ok(Some(String::from_utf8_lossy(&output.stdout).trim().to_owned()))
    }

    /// Push the given branch to origin with tracking (`-u`).
    ///
    /// # Errors
    ///
    /// Returns [`GitError::CommandFailed`] if `git push` fails.
    fn push_branch(&self, branch: &str) -> Result<(), GitError> {
        let command = format!("push -u origin {branch}");
        let output = self.output(&["push", "-u", "origin", branch])?;
        if output.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let code = output.status.code().unwrap_or(-1);
        Err(GitError::CommandFailed { command, code, stderr })
    }

    /// Returns the tree hash of the current index (staged state).
    ///
    /// Uses `git write-tree` to compute the tree object hash from the index,
    /// which reflects staged changes (not just the last commit). This is the
    /// correct hash for review state tracking since it captures what will
    /// actually be committed.
    ///
    /// # Errors
    ///
    /// Returns [`GitError::CommandFailed`] if `git write-tree` fails.
    fn index_tree_hash(&self) -> Result<String, GitError> {
        let output = self.output(&["write-tree"])?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            let code = output.status.code().unwrap_or(-1);
            return Err(GitError::CommandFailed { command: "write-tree".to_owned(), code, stderr });
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    }

    /// Computes the tree hash of the current index with metadata.json fields normalized.
    ///
    /// Normalizes two fields in the metadata.json blob:
    /// - `review.code_hash` → `"PENDING"` (self-referential field)
    /// - `updated_at` → `"1970-01-01T00:00:00Z"` (varies between writes)
    ///
    /// This ensures the hash is deterministic regardless of these volatile fields.
    ///
    /// # Arguments
    /// * `metadata_path` - repo-relative path to metadata.json (e.g., `"track/items/my-track/metadata.json"`)
    ///
    /// # Errors
    /// Returns [`GitError::CommandFailed`] if any git command fails.
    fn index_tree_hash_normalizing(&self, metadata_path: &str) -> Result<String, GitError> {
        let _ = metadata_path;
        Err(GitError::CommandFailed {
            command: "index_tree_hash_normalizing".to_owned(),
            code: -1,
            stderr: "not implemented for this GitRepository".to_owned(),
        })
    }

    /// Stage all worktree changes using `git add -A`, excluding the given pathspecs.
    ///
    /// Tolerates gitignore warnings when the only stderr lines match a known
    /// benign pattern ("ignored by …" + hint lines + listed dir names).
    ///
    /// # Errors
    ///
    /// Returns [`GitError::CommandFailed`] if `git add` fails for a reason other than
    /// the gitignore warning, or if the index cannot be verified after staging.
    fn stage_all_excluding(
        &self,
        exclude_files: &[&str],
        exclude_dirs: &[&str],
    ) -> Result<(), GitError> {
        let mut owned_args =
            vec!["add".to_owned(), "-A".to_owned(), "--".to_owned(), ".".to_owned()];
        owned_args.extend(exclude_files.iter().map(|p| format!(":(exclude){p}")));
        owned_args.extend(exclude_dirs.iter().map(|p| format!(":(exclude){p}")));
        let args: Vec<&str> = owned_args.iter().map(String::as_str).collect();

        let output = self.output(&args)?;
        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        let is_only_ignored_warning = !stderr.is_empty()
            && stderr.contains("ignored by")
            && stderr.lines().filter(|line| !line.trim().is_empty()).all(|line| {
                line.contains("ignored by")
                    || line.starts_with("hint:")
                    || exclude_dirs.iter().any(|dir| line.trim() == *dir)
                    || exclude_files.iter().any(|file| line.trim() == *file)
            });

        if is_only_ignored_warning {
            // git add -A updates the index for all non-ignored paths even when
            // it emits this warning and returns exit 1. The warning is advisory
            // only — the staging operation completes successfully for all
            // trackable files. No post-add verification is needed.
            return Ok(());
        }

        let code = output.status.code().unwrap_or(-1);
        let stderr = stderr.trim().to_owned();
        Err(GitError::CommandFailed { command: "add -A".to_owned(), code, stderr })
    }
}

#[derive(Debug, Clone)]
pub struct SystemGitRepo {
    root: PathBuf,
}

impl SystemGitRepo {
    /// Discover the git repository root from the current working directory.
    ///
    /// # Errors
    ///
    /// Returns [`GitError`] if the current directory cannot be determined,
    /// git cannot be spawned, the command fails, or the root path is empty.
    pub fn discover() -> Result<Self, GitError> {
        let cwd = std::env::current_dir().map_err(GitError::CurrentDir)?;
        let output = Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .current_dir(&cwd)
            .output()
            .map_err(|source| GitError::Spawn {
                command: "rev-parse --show-toplevel".to_owned(),
                source,
            })?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            let code = output.status.code().unwrap_or(-1);
            return Err(GitError::CommandFailed {
                command: "rev-parse --show-toplevel".to_owned(),
                code,
                stderr,
            });
        }

        let root = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if root.is_empty() {
            return Err(GitError::EmptyRepoRoot);
        }

        Ok(Self { root: PathBuf::from(root) })
    }
}

impl GitRepository for SystemGitRepo {
    fn root(&self) -> &Path {
        &self.root
    }

    fn status(&self, args: &[&str]) -> Result<i32, GitError> {
        let status = Command::new("git")
            .args(args)
            .current_dir(&self.root)
            .status()
            .map_err(|source| GitError::Spawn { command: args.join(" "), source })?;
        Ok(status.code().unwrap_or(1))
    }

    fn output(&self, args: &[&str]) -> Result<Output, GitError> {
        Command::new("git")
            .args(args)
            .current_dir(&self.root)
            .output()
            .map_err(|source| GitError::Spawn { command: args.join(" "), source })
    }

    fn index_tree_hash_normalizing(&self, metadata_path: &str) -> Result<String, GitError> {
        // Step 1: Read the metadata.json blob from the current index.
        let show_output = self.output(&["show", &format!(":{metadata_path}")])?;
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
            // Ensure review.code_hash is set to "PENDING" (insert review section if absent).
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

        // Step 5: Write the normalized JSON to git object store via stdin pipe.
        let mut hash_object_child = Command::new("git")
            .args(["hash-object", "-w", "--stdin"])
            .current_dir(&self.root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
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

        // Step 6: Copy current index to a temp file (stdlib only, no tempfile crate needed).
        // Resolve the index path via Git to support linked worktrees where
        // .git is a pointer file and the real index lives elsewhere.
        let index_path = match std::env::var("GIT_INDEX_FILE") {
            Ok(p) => {
                // Anchor relative paths to the repo root so the copy
                // works regardless of the process's current directory.
                if std::path::Path::new(&p).is_absolute() {
                    p
                } else {
                    self.root.join(&p).to_string_lossy().into_owned()
                }
            }
            Err(_) => {
                let rev_output = self.output(&["rev-parse", "--git-path", "index"])?;
                if !rev_output.status.success() {
                    let stderr = String::from_utf8_lossy(&rev_output.stderr).trim().to_owned();
                    let code = rev_output.status.code().unwrap_or(-1);
                    return Err(GitError::CommandFailed {
                        command: "rev-parse --git-path index".to_owned(),
                        code,
                        stderr,
                    });
                }
                let resolved = String::from_utf8_lossy(&rev_output.stdout).trim().to_owned();
                if std::path::Path::new(&resolved).is_absolute() {
                    resolved
                } else {
                    self.root.join(&resolved).to_string_lossy().into_owned()
                }
            }
        };
        // Use a deterministic-but-unique name under the system temp dir.
        let temp_index_path = std::env::temp_dir().join(format!(
            "sotp-norm-index-{}-{}.tmp",
            std::process::id(),
            // Use the pointer address of self as a secondary disambiguator for nested calls.
            self as *const _ as usize,
        ));
        fs::copy(&index_path, &temp_index_path).map_err(|source| GitError::Spawn {
            command: "copy index to temp".to_owned(),
            source,
        })?;

        // Step 7: Update the temp index with the normalized blob.
        let update_output = Command::new("git")
            .args(["update-index", "--cacheinfo", &format!("100644,{blob_hash},{metadata_path}")])
            .current_dir(&self.root)
            .env("GIT_INDEX_FILE", &temp_index_path)
            .output()
            .map_err(|source| GitError::Spawn {
                command: "update-index --cacheinfo".to_owned(),
                source,
            })?;
        if !update_output.status.success() {
            let _ = fs::remove_file(&temp_index_path);
            let stderr = String::from_utf8_lossy(&update_output.stderr).trim().to_owned();
            let code = update_output.status.code().unwrap_or(-1);
            return Err(GitError::CommandFailed {
                command: "update-index --cacheinfo".to_owned(),
                code,
                stderr,
            });
        }

        // Step 8: Write tree from temp index.
        let write_tree_output = Command::new("git")
            .args(["write-tree"])
            .current_dir(&self.root)
            .env("GIT_INDEX_FILE", &temp_index_path)
            .output()
            .map_err(|source| GitError::Spawn {
                command: "write-tree (normalized)".to_owned(),
                source,
            })?;

        // Step 9: Clean up temp index file unconditionally.
        let _ = fs::remove_file(&temp_index_path);

        if !write_tree_output.status.success() {
            let stderr = String::from_utf8_lossy(&write_tree_output.stderr).trim().to_owned();
            let code = write_tree_output.status.code().unwrap_or(-1);
            return Err(GitError::CommandFailed {
                command: "write-tree (normalized)".to_owned(),
                code,
                stderr,
            });
        }

        // Step 10: Return the tree hash.
        Ok(String::from_utf8_lossy(&write_tree_output.stdout).trim().to_owned())
    }
}

impl domain::WorktreeReader for SystemGitRepo {
    fn porcelain_status(&self) -> Result<String, domain::WorktreeError> {
        let output = self
            .output(&["status", "--porcelain"])
            .map_err(|e| domain::WorktreeError::StatusFailed(e.to_string()))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            return Err(domain::WorktreeError::StatusFailed(if stderr.is_empty() {
                format!(
                    "git status --porcelain failed (exit {})",
                    output.status.code().unwrap_or(-1)
                )
            } else {
                format!(
                    "git status --porcelain failed (exit {}): {stderr}",
                    output.status.code().unwrap_or(-1)
                )
            }));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

pub fn resolve_repo_path(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() { path.to_path_buf() } else { root.join(path) }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackBranchRecord {
    pub display_path: String,
    pub track_name: String,
    pub branch: Option<String>,
    pub status: Option<String>,
    pub schema_version: u32,
}

#[derive(Debug, Deserialize)]
struct BranchMetadata {
    #[serde(default = "default_schema_version")]
    schema_version: u32,
    branch: Option<String>,
    status: Option<String>,
}

const fn default_schema_version() -> u32 {
    2
}

const REQUIRED_V3_METADATA_FIELDS: &[&str] = &[
    "schema_version",
    "branch",
    "id",
    "title",
    "status",
    "created_at",
    "updated_at",
    "tasks",
    "plan",
];

pub fn load_explicit_track_branch(
    root: &Path,
    track_dir: &Path,
) -> Result<TrackBranchRecord, String> {
    load_explicit_track_branch_from_items_dir(root, &root.join("track/items"), track_dir)
}

pub fn load_explicit_track_branch_from_items_dir(
    root: &Path,
    items_dir: &Path,
    track_dir: &Path,
) -> Result<TrackBranchRecord, String> {
    if !track_dir.is_dir() {
        return Err(format!("Track directory not found: {}", track_dir.display()));
    }

    let repo_items_dir = items_dir.canonicalize().map_err(|err| {
        format!(
            "{}/ resolves outside the repository root or is unavailable: {err}",
            items_dir.display()
        )
    })?;
    let resolved = track_dir.canonicalize().map_err(|err| {
        format!("failed to resolve track directory {}: {err}", track_dir.display())
    })?;
    if resolved.parent() != Some(repo_items_dir.as_path()) {
        return Err(format!(
            "Track directory must be exactly {}/<id>: {}",
            items_dir.display(),
            track_dir.display()
        ));
    }

    let metadata = read_metadata(&track_dir.join("metadata.json"))?;
    let display_path = resolved.strip_prefix(root).unwrap_or(track_dir).display().to_string();
    Ok(TrackBranchRecord {
        display_path,
        track_name: track_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_owned(),
        branch: metadata.branch,
        status: metadata.status,
        schema_version: metadata.schema_version,
    })
}

pub fn collect_track_branch_claims(root: &Path) -> Result<Vec<TrackBranchRecord>, String> {
    let mut claims = Vec::new();
    for relative_root in ["track/items", "track/archive"] {
        let claims_root = root.join(relative_root);
        if !claims_root.is_dir() {
            continue;
        }
        for entry in read_directories(&claims_root)? {
            let metadata_path = entry.join("metadata.json");
            if !metadata_path.is_file() {
                continue;
            }
            let metadata = match read_metadata(&metadata_path) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("warning: skipping {}: {e}", metadata_path.display());
                    continue;
                }
            };
            claims.push(TrackBranchRecord {
                display_path: entry.strip_prefix(root).unwrap_or(&entry).display().to_string(),
                track_name: entry
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default()
                    .to_owned(),
                branch: metadata.branch,
                status: metadata.status,
                schema_version: metadata.schema_version,
            });
        }
    }
    Ok(claims)
}

fn read_directories(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut dirs = Vec::new();
    for entry in fs::read_dir(root)
        .map_err(|err| format!("failed to read directory {}: {err}", root.display()))?
    {
        let entry = entry.map_err(|err| format!("failed to read directory entry: {err}"))?;
        if entry.path().is_dir() {
            dirs.push(entry.path());
        }
    }
    dirs.sort();
    Ok(dirs)
}

fn read_metadata(path: &Path) -> Result<BranchMetadata, String> {
    let content = fs::read_to_string(path).map_err(|err| {
        format!("Cannot read or parse metadata.json in {}: {err}", path.display())
    })?;
    let raw: serde_json::Value = serde_json::from_str(&content).map_err(|err| {
        format!("Cannot read or parse metadata.json in {}: {err}", path.display())
    })?;
    if raw.get("schema_version").and_then(serde_json::Value::as_u64) == Some(3) {
        let Some(object) = raw.as_object() else {
            return Err(format!(
                "Cannot read or parse metadata.json in {}: metadata.json must be a JSON object",
                path.display()
            ));
        };
        if let Some(missing) =
            REQUIRED_V3_METADATA_FIELDS.iter().find(|field| !object.contains_key(**field))
        {
            return Err(format!(
                "Cannot read or parse metadata.json in {}: Missing required field '{}'",
                path.display(),
                missing
            ));
        }
    }
    let metadata: BranchMetadata = serde_json::from_value(raw.clone()).map_err(|err| {
        format!("Cannot read or parse metadata.json in {}: {err}", path.display())
    })?;
    if invalid_v3_non_null_branch(&raw) {
        return Err(format!(
            "Cannot read or parse metadata.json in {}: Invalid v3 branch value; expected 'track/<id>' or null",
            path.display()
        ));
    }
    if illegal_v3_branchless_track(&raw, &metadata) {
        return Err(format!(
            "Cannot read or parse metadata.json in {}: Illegal branchless v3 metadata; run /track:activate <track-id>",
            path.display()
        ));
    }
    Ok(metadata)
}

fn illegal_v3_branchless_track(raw: &serde_json::Value, metadata: &BranchMetadata) -> bool {
    if metadata.schema_version != 3 || metadata.branch.is_some() {
        return false;
    }

    if raw.get("status_override").is_some_and(|value| !value.is_null()) {
        return true;
    }

    match metadata.status.as_deref() {
        Some("planned") => match raw.get("tasks") {
            None => false,
            Some(serde_json::Value::Array(tasks)) => !tasks.iter().all(|task| {
                task.as_object()
                    .and_then(|object| object.get("status"))
                    .and_then(serde_json::Value::as_str)
                    == Some("todo")
            }),
            Some(_) => true,
        },
        _ => true,
    }
}

fn invalid_v3_non_null_branch(raw: &serde_json::Value) -> bool {
    if raw.get("schema_version").and_then(serde_json::Value::as_u64) != Some(3) {
        return false;
    }

    let Some(branch) = raw.get("branch") else {
        return false;
    };
    if branch.is_null() {
        return false;
    }

    match branch.as_str() {
        Some(value) => {
            let trimmed = value.trim();
            trimmed.is_empty() || !trimmed.starts_with("track/")
        }
        None => true,
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::{Mutex, OnceLock};

    use rstest::rstest;

    use super::{
        GitRepository, SystemGitRepo, collect_track_branch_claims, load_explicit_track_branch,
        load_explicit_track_branch_from_items_dir, resolve_repo_path,
    };

    fn cwd_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    /// Convenience constructor for a zero-findings ReviewRoundResult in tests.
    fn rrz(round: u32, ts: &str) -> domain::ReviewRoundResult {
        domain::ReviewRoundResult::new(
            round,
            domain::Verdict::ZeroFindings,
            domain::Timestamp::new(ts).unwrap(),
        )
    }

    struct CurrentDirGuard {
        original: PathBuf,
    }

    impl CurrentDirGuard {
        fn change_to(path: &std::path::Path) -> Self {
            let original = std::env::current_dir().unwrap();
            std::env::set_current_dir(path).unwrap();
            Self { original }
        }
    }

    impl Drop for CurrentDirGuard {
        fn drop(&mut self) {
            std::env::set_current_dir(&self.original).unwrap();
        }
    }

    fn run_git(root: &std::path::Path, args: &[&str]) {
        let status = Command::new("git").args(args).current_dir(root).status().unwrap();
        assert!(status.success(), "git command failed: git {}", args.join(" "));
    }

    fn init_git_repo_with_identity(dir: &std::path::Path) {
        run_git(dir, &["init"]);
        run_git(dir, &["config", "user.email", "test@example.com"]);
        run_git(dir, &["config", "user.name", "Test"]);
    }

    #[test]
    fn index_tree_hash_normalizing_returns_deterministic_hash_for_same_content() {
        let _lock = cwd_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        init_git_repo_with_identity(dir.path());

        // Create an initial commit so the repo is in a valid state
        let metadata_path = "track/items/my-track/metadata.json";
        let metadata_dir = dir.path().join("track/items/my-track");
        fs::create_dir_all(&metadata_dir).unwrap();
        let metadata_content = serde_json::json!({
            "schema_version": 3,
            "id": "my-track",
            "updated_at": "2026-03-18T12:00:00Z",
            "review": {
                "status": "fast_passed",
                "code_hash": "some-hash-value"
            }
        })
        .to_string();
        fs::write(dir.path().join(metadata_path), &metadata_content).unwrap();
        run_git(dir.path(), &["add", "."]);
        run_git(dir.path(), &["commit", "--allow-empty-message", "-m", "initial"]);

        // Stage the same content again (no changes)
        run_git(dir.path(), &["add", "."]);

        let _guard = CurrentDirGuard::change_to(dir.path());
        let repo = SystemGitRepo::discover().unwrap();

        let hash1 = repo.index_tree_hash_normalizing(metadata_path).unwrap();
        let hash2 = repo.index_tree_hash_normalizing(metadata_path).unwrap();
        assert_eq!(hash1, hash2, "normalized hash must be deterministic");
        assert!(!hash1.is_empty());
    }

    #[test]
    fn index_tree_hash_normalizing_ignores_volatile_fields() {
        let _lock = cwd_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        init_git_repo_with_identity(dir.path());

        let metadata_path = "track/items/my-track/metadata.json";
        let metadata_dir = dir.path().join("track/items/my-track");
        fs::create_dir_all(&metadata_dir).unwrap();

        // Metadata with one set of volatile field values
        let content_a = serde_json::json!({
            "schema_version": 3,
            "id": "my-track",
            "updated_at": "2026-03-18T12:00:00Z",
            "review": {
                "status": "fast_passed",
                "code_hash": "PENDING"
            }
        })
        .to_string();
        fs::write(dir.path().join(metadata_path), &content_a).unwrap();
        run_git(dir.path(), &["add", "."]);
        run_git(dir.path(), &["commit", "--allow-empty-message", "-m", "commit-a"]);

        let _guard = CurrentDirGuard::change_to(dir.path());
        let repo = SystemGitRepo::discover().unwrap();
        let hash_a = repo.index_tree_hash_normalizing(metadata_path).unwrap();

        drop(_guard);

        // Metadata with different volatile field values but same logical content
        let content_b = serde_json::json!({
            "schema_version": 3,
            "id": "my-track",
            "updated_at": "2026-03-19T00:00:00Z",  // different updated_at
            "review": {
                "status": "fast_passed",
                "code_hash": "some-real-hash"  // different code_hash
            }
        })
        .to_string();
        fs::write(dir.path().join(metadata_path), &content_b).unwrap();
        run_git(dir.path(), &["add", "."]);

        let _guard2 = CurrentDirGuard::change_to(dir.path());
        let repo2 = SystemGitRepo::discover().unwrap();
        let hash_b = repo2.index_tree_hash_normalizing(metadata_path).unwrap();

        // Both hashes should be equal because volatile fields are normalized away
        assert_eq!(hash_a, hash_b, "normalized hash must ignore volatile fields");
    }

    #[test]
    fn index_tree_hash_normalizing_normalizes_missing_review_section() {
        let _lock = cwd_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        init_git_repo_with_identity(dir.path());

        let metadata_path = "track/items/my-track/metadata.json";
        let metadata_dir = dir.path().join("track/items/my-track");
        fs::create_dir_all(&metadata_dir).unwrap();

        // No review section at all
        let content = serde_json::json!({
            "schema_version": 3,
            "id": "my-track",
            "updated_at": "2026-03-18T12:00:00Z"
        })
        .to_string();
        fs::write(dir.path().join(metadata_path), &content).unwrap();
        run_git(dir.path(), &["add", "."]);
        run_git(dir.path(), &["commit", "--allow-empty-message", "-m", "initial"]);

        let _guard = CurrentDirGuard::change_to(dir.path());
        let repo = SystemGitRepo::discover().unwrap();

        // Should succeed even without a review section
        let hash = repo.index_tree_hash_normalizing(metadata_path).unwrap();
        assert!(!hash.is_empty());
    }

    #[test]
    fn system_git_repo_discovers_root_from_nested_directory() {
        let _lock = cwd_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        run_git(dir.path(), &["init"]);
        let nested = dir.path().join("nested/deeper");
        fs::create_dir_all(&nested).unwrap();

        let _guard = CurrentDirGuard::change_to(&nested);
        let repo = SystemGitRepo::discover().unwrap();

        assert_eq!(repo.root(), dir.path());
    }

    #[test]
    fn resolve_repo_path_anchors_relative_paths_at_repo_root() {
        let root = std::path::Path::new("/repo/root");

        assert_eq!(
            resolve_repo_path(root, std::path::Path::new("tmp/track-commit/note.md")),
            PathBuf::from("/repo/root/tmp/track-commit/note.md")
        );
        assert_eq!(
            resolve_repo_path(root, std::path::Path::new("/tmp/note.md")),
            PathBuf::from("/tmp/note.md")
        );
    }

    #[test]
    fn load_explicit_track_branch_rejects_non_track_items_directory() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("track/items")).unwrap();
        let track_dir = dir.path().join("elsewhere/example");
        fs::create_dir_all(&track_dir).unwrap();
        fs::write(track_dir.join("metadata.json"), "{\"branch\":\"track/example\"}").unwrap();

        let err = load_explicit_track_branch(dir.path(), &track_dir).unwrap_err();

        assert!(err.contains("Track directory must be exactly"));
    }

    #[test]
    fn load_explicit_track_branch_uses_repo_relative_display_path() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/example");
        fs::create_dir_all(&track_dir).unwrap();
        fs::write(
            track_dir.join("metadata.json"),
            r#"{"schema_version":3,"id":"example","branch":null,"title":"Example","status":"planned","created_at":"2026-03-14T00:00:00Z","updated_at":"2026-03-14T00:00:00Z","tasks":[],"plan":{"summary":[],"sections":[]}}"#,
        )
        .unwrap();

        let record = load_explicit_track_branch(dir.path(), &track_dir).unwrap();

        assert_eq!(record.display_path, "track/items/example");
    }

    #[test]
    fn load_explicit_track_branch_from_items_dir_uses_custom_items_root() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("custom/track/items");
        let track_dir = items_dir.join("example");
        fs::create_dir_all(&track_dir).unwrap();
        fs::write(
            track_dir.join("metadata.json"),
            r#"{"schema_version":3,"id":"example","branch":"track/example","title":"Example","status":"planned","created_at":"2026-03-14T00:00:00Z","updated_at":"2026-03-14T00:00:00Z","tasks":[],"plan":{"summary":[],"sections":[]}}"#,
        )
        .unwrap();

        let record =
            load_explicit_track_branch_from_items_dir(dir.path(), &items_dir, &track_dir).unwrap();

        assert_eq!(record.display_path, "custom/track/items/example");
    }

    #[rstest]
    #[case::missing_branch(
        r#"{"schema_version":3,"status":"planned"}"#,
        "Missing required field 'branch'"
    )]
    #[case::missing_title(
        r#"{"schema_version":3,"id":"example","status":"planned","branch":null,"created_at":"2026-03-14T00:00:00Z","updated_at":"2026-03-14T00:00:00Z","tasks":[],"plan":{"summary":[],"sections":[]}}"#,
        "Missing required field 'title'"
    )]
    fn load_explicit_track_branch_rejects_v3_track_missing_required_field(
        #[case] metadata_json: &str,
        #[case] expected_error: &str,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/example");
        fs::create_dir_all(&track_dir).unwrap();
        fs::write(track_dir.join("metadata.json"), metadata_json).unwrap();

        let err = load_explicit_track_branch(dir.path(), &track_dir).unwrap_err();

        assert!(err.contains(expected_error));
    }

    #[test]
    fn load_explicit_track_branch_rejects_illegal_branchless_v3_track() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/example");
        fs::create_dir_all(&track_dir).unwrap();
        fs::write(
            track_dir.join("metadata.json"),
            r#"{"schema_version":3,"id":"example","branch":null,"title":"Example","status":"in_progress","created_at":"2026-03-14T00:00:00Z","updated_at":"2026-03-14T00:00:00Z","tasks":[],"plan":{"summary":[],"sections":[]}}"#,
        )
        .unwrap();

        let err = load_explicit_track_branch(dir.path(), &track_dir).unwrap_err();

        assert!(err.contains("Illegal branchless v3 metadata"));
    }

    #[test]
    fn load_explicit_track_branch_rejects_branchless_v3_track_with_status_override() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/example");
        fs::create_dir_all(&track_dir).unwrap();
        fs::write(
            track_dir.join("metadata.json"),
            r#"{"schema_version":3,"id":"example","branch":null,"title":"Example","status":"planned","created_at":"2026-03-14T00:00:00Z","updated_at":"2026-03-14T00:00:00Z","tasks":[],"plan":{"summary":[],"sections":[]},"status_override":{"status":"blocked","reason":"waiting"}}"#,
        )
        .unwrap();

        let err = load_explicit_track_branch(dir.path(), &track_dir).unwrap_err();

        assert!(err.contains("Illegal branchless v3 metadata"));
    }

    #[test]
    fn load_explicit_track_branch_allows_branchless_v3_track_with_null_status_override() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/example");
        fs::create_dir_all(&track_dir).unwrap();
        fs::write(
            track_dir.join("metadata.json"),
            r#"{"schema_version":3,"id":"example","branch":null,"title":"Example","status":"planned","created_at":"2026-03-14T00:00:00Z","updated_at":"2026-03-14T00:00:00Z","tasks":[],"plan":{"summary":[],"sections":[]},"status_override":null}"#,
        )
        .unwrap();

        let metadata = load_explicit_track_branch(dir.path(), &track_dir).unwrap();

        assert_eq!(metadata.branch, None);
        assert_eq!(metadata.status.as_deref(), Some("planned"));
    }

    #[test]
    fn load_explicit_track_branch_rejects_invalid_non_track_v3_branch() {
        let dir = tempfile::tempdir().unwrap();
        let track_dir = dir.path().join("track/items/example");
        fs::create_dir_all(&track_dir).unwrap();
        fs::write(
            track_dir.join("metadata.json"),
            r#"{"schema_version":3,"id":"example","branch":"feature/foo","title":"Example","status":"planned","created_at":"2026-03-14T00:00:00Z","updated_at":"2026-03-14T00:00:00Z","tasks":[],"plan":{"summary":[],"sections":[]}}"#,
        )
        .unwrap();

        let err = load_explicit_track_branch(dir.path(), &track_dir).unwrap_err();

        assert!(err.contains("Invalid v3 branch value"));
    }

    #[test]
    fn collect_track_branch_claims_includes_items_and_archive() {
        let dir = tempfile::tempdir().unwrap();
        let active = dir.path().join("track/items/active");
        let archived = dir.path().join("track/archive/archived");
        fs::create_dir_all(&active).unwrap();
        fs::create_dir_all(&archived).unwrap();
        fs::write(
            active.join("metadata.json"),
            "{\"branch\":\"track/active\",\"status\":\"in_progress\"}",
        )
        .unwrap();
        fs::write(archived.join("metadata.json"), "{\"branch\":null,\"status\":\"archived\"}")
            .unwrap();

        let claims = collect_track_branch_claims(dir.path()).unwrap();
        let display_paths =
            claims.iter().map(|claim| claim.display_path.as_str()).collect::<Vec<_>>();

        assert_eq!(claims.len(), 2);
        assert!(display_paths.contains(&"track/items/active"));
        assert!(display_paths.contains(&"track/archive/archived"));
    }

    #[test]
    fn collect_track_branch_claims_skips_invalid_metadata_and_returns_valid() {
        let dir = tempfile::tempdir().unwrap();
        let valid = dir.path().join("track/items/valid");
        let invalid = dir.path().join("track/items/invalid");
        fs::create_dir_all(&valid).unwrap();
        fs::create_dir_all(&invalid).unwrap();
        fs::write(
            valid.join("metadata.json"),
            "{\"branch\":\"track/valid\",\"status\":\"in_progress\"}",
        )
        .unwrap();
        fs::write(invalid.join("metadata.json"), "{not-json").unwrap();

        let claims = collect_track_branch_claims(dir.path()).unwrap();
        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].track_name, "valid");
    }

    // -----------------------------------------------------------------------
    // T005: Integration tests — normalized hash protocol (method D) end-to-end
    // -----------------------------------------------------------------------

    /// Helper: write a full metadata.json for integration tests.
    fn write_full_metadata(dir: &std::path::Path, metadata_path: &str, review_json: &str) {
        let full_path = dir.join(metadata_path);
        let parent = full_path.parent().unwrap();
        fs::create_dir_all(parent).unwrap();
        let content = format!(
            r#"{{
  "schema_version": 3,
  "id": "test-track",
  "branch": "track/test-track",
  "title": "Test Track",
  "status": "in_progress",
  "created_at": "2026-03-18T00:00:00Z",
  "updated_at": "2026-03-18T12:00:00Z",
  "tasks": [{{"id": "T1", "description": "Task 1", "status": "in_progress"}}],
  "plan": {{"summary": [], "sections": [{{"id": "S1", "title": "Sec", "description": [], "task_ids": ["T1"]}}]}}{review_json}
}}"#
        );
        fs::write(full_path, content).unwrap();
    }

    /// Helper: stage a file and return the repo instance.
    fn setup_integration_repo(
        dir: &std::path::Path,
        metadata_path: &str,
        review_json: &str,
    ) -> SystemGitRepo {
        init_git_repo_with_identity(dir);
        write_full_metadata(dir, metadata_path, review_json);
        // Create a source file to represent "real code"
        let src = dir.join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("main.rs"), "fn main() {}").unwrap();
        run_git(dir, &["add", "."]);
        run_git(dir, &["commit", "--allow-empty-message", "-m", "initial"]);
        SystemGitRepo::discover().unwrap()
    }

    #[test]
    fn integration_record_round_then_check_approved_succeeds() {
        // Full protocol: record_round_with_pending → re-stage → normalized hash →
        // set_code_hash → re-stage → check_commit_ready with normalized hash → OK
        let _lock = cwd_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let metadata_path = "track/items/test-track/metadata.json";
        let _guard = CurrentDirGuard::change_to(dir.path());
        let repo = setup_integration_repo(dir.path(), metadata_path, "");

        // Step 1: Compute pre-update normalized hash
        let pre_hash = repo.index_tree_hash_normalizing(metadata_path).unwrap();

        // Step 2: record_round_with_pending
        let mut review = domain::ReviewState::new();
        let result = rrz(1, "2026-03-18T01:00:00Z");
        let default_group = domain::ReviewGroupName::try_new("default").unwrap();
        let groups = vec![default_group.clone()];
        review
            .record_round_with_pending(
                domain::RoundType::Fast,
                &default_group,
                result,
                &groups,
                &pre_hash,
            )
            .unwrap();
        // code_hash() returns None for Pending; serialization gives "PENDING"
        assert!(review.code_hash().is_none());
        assert_eq!(review.code_hash_for_serialization(), Some("PENDING"));

        // Write metadata with PENDING hash and re-stage
        write_full_metadata(
            dir.path(),
            metadata_path,
            r#",
  "review": {
    "status": "fast_passed",
    "code_hash": "PENDING",
    "groups": {"default": {"fast": {"round": 1, "verdict": "zero_findings", "timestamp": "2026-03-18T01:00:00Z"}}}
  }"#,
        );
        run_git(dir.path(), &["add", metadata_path]);

        // Step 3: Compute post-update normalized hash H1
        let post_hash = repo.index_tree_hash_normalizing(metadata_path).unwrap();

        // Step 4: set_code_hash(H1) and re-stage
        review.set_code_hash(post_hash.clone()).unwrap();
        write_full_metadata(
            dir.path(),
            metadata_path,
            &format!(
                r#",
  "review": {{
    "status": "fast_passed",
    "code_hash": "{post_hash}",
    "groups": {{"default": {{"fast": {{"round": 1, "verdict": "zero_findings", "timestamp": "2026-03-18T01:00:00Z"}}}}}}
  }}"#
            ),
        );
        run_git(dir.path(), &["add", metadata_path]);

        // Step 5: Do final round for approval
        let pre_hash2 = repo.index_tree_hash_normalizing(metadata_path).unwrap();
        assert_eq!(pre_hash2, post_hash, "pre-update hash for 2nd round should match stored hash");

        let result2 = rrz(1, "2026-03-18T02:00:00Z");
        review
            .record_round_with_pending(
                domain::RoundType::Final,
                &default_group,
                result2,
                &groups,
                &pre_hash2,
            )
            .unwrap();
        assert_eq!(review.status(), domain::ReviewStatus::Approved);

        write_full_metadata(
            dir.path(),
            metadata_path,
            r#",
  "review": {
    "status": "approved",
    "code_hash": "PENDING",
    "groups": {"default": {"fast": {"round": 1, "verdict": "zero_findings", "timestamp": "2026-03-18T01:00:00Z"}, "final": {"round": 1, "verdict": "zero_findings", "timestamp": "2026-03-18T02:00:00Z"}}}
  }"#,
        );
        run_git(dir.path(), &["add", metadata_path]);

        let final_hash = repo.index_tree_hash_normalizing(metadata_path).unwrap();
        review.set_code_hash(final_hash.clone()).unwrap();

        write_full_metadata(
            dir.path(),
            metadata_path,
            &format!(
                r#",
  "review": {{
    "status": "approved",
    "code_hash": "{final_hash}",
    "groups": {{"default": {{"fast": {{"round": 1, "verdict": "zero_findings", "timestamp": "2026-03-18T01:00:00Z"}}, "final": {{"round": 1, "verdict": "zero_findings", "timestamp": "2026-03-18T02:00:00Z"}}}}}}
  }}"#
            ),
        );
        run_git(dir.path(), &["add", metadata_path]);

        // Step 6: check-approved — recompute normalized hash and verify match
        let check_hash = repo.index_tree_hash_normalizing(metadata_path).unwrap();
        assert_eq!(check_hash, final_hash, "normalized hash must match stored code_hash");
        assert!(review.check_commit_ready(&check_hash).is_ok());
    }

    #[test]
    fn integration_source_code_change_fails_check_approved() {
        let _lock = cwd_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let metadata_path = "track/items/test-track/metadata.json";
        let _guard = CurrentDirGuard::change_to(dir.path());
        let repo = setup_integration_repo(dir.path(), metadata_path, "");

        // Complete a full approval cycle
        let pre_hash = repo.index_tree_hash_normalizing(metadata_path).unwrap();
        let mut review = domain::ReviewState::new();
        let default_group = domain::ReviewGroupName::try_new("default").unwrap();
        let groups = vec![default_group.clone()];

        // Fast round
        let r1 = rrz(1, "2026-03-18T01:00:00Z");
        review
            .record_round_with_pending(
                domain::RoundType::Fast,
                &default_group,
                r1,
                &groups,
                &pre_hash,
            )
            .unwrap();
        write_full_metadata(
            dir.path(),
            metadata_path,
            r#",
  "review": {"status": "fast_passed", "code_hash": "PENDING", "groups": {"default": {"fast": {"round": 1, "verdict": "zero_findings", "timestamp": "2026-03-18T01:00:00Z"}}}}"#,
        );
        run_git(dir.path(), &["add", metadata_path]);
        let h1 = repo.index_tree_hash_normalizing(metadata_path).unwrap();
        review.set_code_hash(h1.clone()).unwrap();

        // Final round
        write_full_metadata(
            dir.path(),
            metadata_path,
            &format!(
                r#",
  "review": {{"status": "fast_passed", "code_hash": "{h1}", "groups": {{"default": {{"fast": {{"round": 1, "verdict": "zero_findings", "timestamp": "2026-03-18T01:00:00Z"}}}}}}}}"#
            ),
        );
        run_git(dir.path(), &["add", metadata_path]);
        let r2 = rrz(1, "2026-03-18T02:00:00Z");
        let pre2 = repo.index_tree_hash_normalizing(metadata_path).unwrap();
        review
            .record_round_with_pending(domain::RoundType::Final, &default_group, r2, &groups, &pre2)
            .unwrap();
        write_full_metadata(
            dir.path(),
            metadata_path,
            r#",
  "review": {"status": "approved", "code_hash": "PENDING", "groups": {"default": {"fast": {"round": 1, "verdict": "zero_findings", "timestamp": "2026-03-18T01:00:00Z"}, "final": {"round": 1, "verdict": "zero_findings", "timestamp": "2026-03-18T02:00:00Z"}}}}"#,
        );
        run_git(dir.path(), &["add", metadata_path]);
        let approved_hash = repo.index_tree_hash_normalizing(metadata_path).unwrap();
        review.set_code_hash(approved_hash.clone()).unwrap();

        // Write approved hash back
        write_full_metadata(
            dir.path(),
            metadata_path,
            &format!(
                r#",
  "review": {{"status": "approved", "code_hash": "{approved_hash}", "groups": {{"default": {{"fast": {{"round": 1, "verdict": "zero_findings", "timestamp": "2026-03-18T01:00:00Z"}}, "final": {{"round": 1, "verdict": "zero_findings", "timestamp": "2026-03-18T02:00:00Z"}}}}}}}}"#
            ),
        );
        run_git(dir.path(), &["add", metadata_path]);

        // Now tamper: change source code
        fs::write(dir.path().join("src/main.rs"), "fn main() { println!(\"tampered\"); }").unwrap();
        run_git(dir.path(), &["add", "."]);

        // Re-check: hash should now differ
        let tampered_hash = repo.index_tree_hash_normalizing(metadata_path).unwrap();
        assert_ne!(tampered_hash, approved_hash, "hash must change after source code tamper");

        // check_commit_ready should fail
        let result = review.check_commit_ready(&tampered_hash);
        assert!(
            matches!(result, Err(domain::ReviewError::StaleCodeHash { .. })),
            "check_commit_ready must fail after source code change"
        );
    }

    #[test]
    fn integration_review_status_tamper_fails_check_approved() {
        let _lock = cwd_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let metadata_path = "track/items/test-track/metadata.json";
        let _guard = CurrentDirGuard::change_to(dir.path());
        let repo = setup_integration_repo(dir.path(), metadata_path, "");

        // Get initial hash and do fast approval
        let pre_hash = repo.index_tree_hash_normalizing(metadata_path).unwrap();
        let mut review = domain::ReviewState::new();
        let default_group = domain::ReviewGroupName::try_new("default").unwrap();
        let groups = vec![default_group.clone()];

        let r1 = rrz(1, "2026-03-18T01:00:00Z");
        review
            .record_round_with_pending(
                domain::RoundType::Fast,
                &default_group,
                r1,
                &groups,
                &pre_hash,
            )
            .unwrap();
        write_full_metadata(
            dir.path(),
            metadata_path,
            r#",
  "review": {"status": "fast_passed", "code_hash": "PENDING", "groups": {"default": {"fast": {"round": 1, "verdict": "zero_findings", "timestamp": "2026-03-18T01:00:00Z"}}}}"#,
        );
        run_git(dir.path(), &["add", metadata_path]);
        let h1 = repo.index_tree_hash_normalizing(metadata_path).unwrap();
        review.set_code_hash(h1.clone()).unwrap();

        // Now tamper: change review.status from fast_passed to approved (without final round)
        write_full_metadata(
            dir.path(),
            metadata_path,
            &format!(
                r#",
  "review": {{"status": "approved", "code_hash": "{h1}", "groups": {{"default": {{"fast": {{"round": 1, "verdict": "zero_findings", "timestamp": "2026-03-18T01:00:00Z"}}}}}}}}"#
            ),
        );
        run_git(dir.path(), &["add", metadata_path]);

        let tampered_hash = repo.index_tree_hash_normalizing(metadata_path).unwrap();
        // The status change is NOT a normalized field, so the hash MUST differ
        assert_ne!(tampered_hash, h1, "hash must change after review.status tamper");
    }

    #[test]
    fn integration_first_round_no_prior_code_hash_succeeds() {
        // First review round: code_hash is None — freshness check should be skipped
        let _lock = cwd_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let metadata_path = "track/items/test-track/metadata.json";
        let _guard = CurrentDirGuard::change_to(dir.path());
        let repo = setup_integration_repo(dir.path(), metadata_path, "");

        let pre_hash = repo.index_tree_hash_normalizing(metadata_path).unwrap();
        let mut review = domain::ReviewState::new();
        assert!(review.code_hash().is_none(), "first round: code_hash must be None");

        let default_group = domain::ReviewGroupName::try_new("default").unwrap();
        let groups = vec![default_group.clone()];
        let r = rrz(1, "2026-03-18T01:00:00Z");
        let result = review.record_round_with_pending(
            domain::RoundType::Fast,
            &default_group,
            r,
            &groups,
            &pre_hash,
        );
        assert!(result.is_ok(), "first round with no prior code_hash must succeed");
        // code_hash() returns None for Pending; serialization gives "PENDING"
        assert!(review.code_hash().is_none());
        assert_eq!(review.code_hash_for_serialization(), Some("PENDING"));
    }

    #[test]
    fn integration_pre_update_freshness_check_detects_code_change_between_rounds() {
        let _lock = cwd_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let metadata_path = "track/items/test-track/metadata.json";
        let _guard = CurrentDirGuard::change_to(dir.path());
        let repo = setup_integration_repo(dir.path(), metadata_path, "");

        // First round
        let pre_hash = repo.index_tree_hash_normalizing(metadata_path).unwrap();
        let mut review = domain::ReviewState::new();
        let default_group = domain::ReviewGroupName::try_new("default").unwrap();
        let groups = vec![default_group.clone()];
        let r1 = rrz(1, "2026-03-18T01:00:00Z");
        review
            .record_round_with_pending(
                domain::RoundType::Fast,
                &default_group,
                r1,
                &groups,
                &pre_hash,
            )
            .unwrap();
        write_full_metadata(
            dir.path(),
            metadata_path,
            r#",
  "review": {"status": "fast_passed", "code_hash": "PENDING", "groups": {"default": {"fast": {"round": 1, "verdict": "zero_findings", "timestamp": "2026-03-18T01:00:00Z"}}}}"#,
        );
        run_git(dir.path(), &["add", metadata_path]);
        let h1 = repo.index_tree_hash_normalizing(metadata_path).unwrap();
        review.set_code_hash(h1.clone()).unwrap();
        write_full_metadata(
            dir.path(),
            metadata_path,
            &format!(
                r#",
  "review": {{"status": "fast_passed", "code_hash": "{h1}", "groups": {{"default": {{"fast": {{"round": 1, "verdict": "zero_findings", "timestamp": "2026-03-18T01:00:00Z"}}}}}}}}"#
            ),
        );
        run_git(dir.path(), &["add", metadata_path]);

        // Now change source code between rounds
        fs::write(dir.path().join("src/main.rs"), "fn main() { /* changed */ }").unwrap();
        run_git(dir.path(), &["add", "."]);

        // Second round: pre-update hash should differ from stored h1
        let pre_hash2 = repo.index_tree_hash_normalizing(metadata_path).unwrap();
        assert_ne!(pre_hash2, h1, "code change between rounds must change hash");

        // record_round_with_pending should detect the stale hash
        let r2 = rrz(2, "2026-03-18T03:00:00Z");
        let result = review.record_round_with_pending(
            domain::RoundType::Fast,
            &default_group,
            r2,
            &groups,
            &pre_hash2,
        );
        assert!(
            matches!(result, Err(domain::ReviewError::StaleCodeHash { .. })),
            "pre-update freshness check must detect code change between rounds"
        );
    }

    #[test]
    fn integration_updated_at_variation_does_not_affect_hash() {
        let _lock = cwd_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let metadata_path = "track/items/test-track/metadata.json";
        let _guard = CurrentDirGuard::change_to(dir.path());
        let _repo = setup_integration_repo(dir.path(), metadata_path, "");

        // Write with updated_at = T1
        write_full_metadata(
            dir.path(),
            metadata_path,
            r#",
  "review": {"status": "fast_passed", "code_hash": "abc", "groups": {}}"#,
        );
        run_git(dir.path(), &["add", metadata_path]);
        let repo1 = SystemGitRepo::discover().unwrap();
        let hash1 = repo1.index_tree_hash_normalizing(metadata_path).unwrap();

        // Overwrite with different updated_at (the timestamp in write_full_metadata is fixed,
        // so we modify it directly)
        let content = fs::read_to_string(dir.path().join(metadata_path)).unwrap();
        let modified = content.replace("2026-03-18T12:00:00Z", "2099-12-31T23:59:59Z");
        fs::write(dir.path().join(metadata_path), modified).unwrap();
        run_git(dir.path(), &["add", metadata_path]);
        let hash2 = repo1.index_tree_hash_normalizing(metadata_path).unwrap();

        assert_eq!(hash1, hash2, "updated_at variation must not affect normalized hash");
    }

    #[test]
    fn integration_multi_group_btreemap_produces_stable_hash() {
        let _lock = cwd_lock().lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let metadata_path = "track/items/test-track/metadata.json";
        let _guard = CurrentDirGuard::change_to(dir.path());
        let _repo = setup_integration_repo(dir.path(), metadata_path, "");

        // Write metadata with groups in reverse alphabetical order
        write_full_metadata(
            dir.path(),
            metadata_path,
            r#",
  "review": {"status": "fast_passed", "code_hash": "PENDING", "groups": {"z-group": {"fast": {"round": 1, "verdict": "zero_findings", "timestamp": "ts"}}, "a-group": {"fast": {"round": 1, "verdict": "zero_findings", "timestamp": "ts"}}}}"#,
        );
        run_git(dir.path(), &["add", metadata_path]);
        let repo1 = SystemGitRepo::discover().unwrap();
        let hash1 = repo1.index_tree_hash_normalizing(metadata_path).unwrap();

        // Write again in a different order (shouldn't matter — normalization is deterministic)
        write_full_metadata(
            dir.path(),
            metadata_path,
            r#",
  "review": {"status": "fast_passed", "code_hash": "PENDING", "groups": {"a-group": {"fast": {"round": 1, "verdict": "zero_findings", "timestamp": "ts"}}, "z-group": {"fast": {"round": 1, "verdict": "zero_findings", "timestamp": "ts"}}}}"#,
        );
        run_git(dir.path(), &["add", metadata_path]);
        let hash2 = repo1.index_tree_hash_normalizing(metadata_path).unwrap();

        assert_eq!(hash1, hash2, "group ordering must not affect normalized hash");
    }
}
