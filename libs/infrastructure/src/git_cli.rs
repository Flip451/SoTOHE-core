use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde::Deserialize;
use thiserror::Error;

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
            let metadata = read_metadata(&metadata_path)?;
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
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
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
    fn collect_track_branch_claims_fails_closed_on_invalid_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let active = dir.path().join("track/items/active");
        fs::create_dir_all(&active).unwrap();
        fs::write(active.join("metadata.json"), "{not-json").unwrap();

        let err = collect_track_branch_claims(dir.path()).unwrap_err();

        assert!(err.contains("Cannot read or parse metadata.json"));
        assert!(err.contains("track/items/active/metadata.json"));
    }
}
