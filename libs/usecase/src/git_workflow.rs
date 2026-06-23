//! Pure workflow rules for guarded git operations.
//!
//! `tmp/track-commit/*` is the primary scratch contract.

use std::path::PathBuf;

use thiserror::Error;

pub const TRANSIENT_AUTOMATION_FILES: &[&str] = &[
    "tmp/track-commit/add-paths.txt",
    "tmp/track-commit/commit-message.txt",
    "tmp/track-commit/note.md",
];

pub const TRANSIENT_AUTOMATION_DIRS: &[&str] = &["tmp"];

const GLOB_MAGIC_CHARS: &[char] = &['*', '?', '[', ']'];

/// Errors returned by git workflow validation functions.
#[derive(Debug, Error)]
pub enum GitWorkflowError {
    #[error("{0}")]
    Validation(String),
    #[error("cannot determine current git branch")]
    NoBranch,
    #[error("detached HEAD — {0}")]
    DetachedHead(String),
    #[error("branch mismatch: current '{current}' does not match expected '{expected}'")]
    BranchMismatch { current: String, expected: String },
    #[error("{0}")]
    Message(String),
    /// I/O or infrastructure failure (e.g. git process error, file read failure).
    #[error("git workflow unavailable: {0}")]
    Unavailable(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExplicitTrackBranch {
    pub display_path: String,
    pub expected_branch: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackBranchClaim {
    pub track_name: String,
    pub branch: Option<String>,
    pub status: Option<String>,
}

pub fn validate_stage_path_entries<'a, I>(entries: I) -> Result<Vec<String>, GitWorkflowError>
where
    I: IntoIterator<Item = &'a str>,
{
    let transient_paths: Vec<PathBuf> =
        TRANSIENT_AUTOMATION_FILES.iter().map(PathBuf::from).collect();
    let transient_dirs: Vec<PathBuf> =
        TRANSIENT_AUTOMATION_DIRS.iter().map(PathBuf::from).collect();
    let mut stage_paths = Vec::new();
    let mut seen = std::collections::BTreeSet::new();

    for raw_line in entries {
        let entry = raw_line.trim();
        if entry.is_empty() || entry.starts_with('#') || !seen.insert(entry.to_owned()) {
            continue;
        }

        let entry_path = PathBuf::from(entry);
        if entry_path.is_absolute() {
            return Err(GitWorkflowError::Validation(format!(
                "Stage path list must use repo-relative paths: {entry}"
            )));
        }
        if entry_path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err(GitWorkflowError::Validation(format!(
                "Stage path list cannot escape the repo root: {entry}"
            )));
        }
        if matches!(entry, "." | "./") {
            return Err(GitWorkflowError::Validation(format!(
                "Stage path list cannot use whole-worktree pathspecs: {entry}"
            )));
        }
        if entry.starts_with(':') {
            return Err(GitWorkflowError::Validation(format!(
                "Stage path list cannot use git pathspec magic or shorthand: {entry}"
            )));
        }
        if entry.chars().any(|ch| GLOB_MAGIC_CHARS.contains(&ch)) {
            return Err(GitWorkflowError::Validation(format!(
                "Stage path list cannot use glob patterns: {entry}"
            )));
        }
        if transient_paths
            .iter()
            .any(|transient| entry_path == *transient || transient.starts_with(&entry_path))
        {
            return Err(GitWorkflowError::Validation(format!(
                "Stage path list cannot include transient automation files or their parent directories: {entry}"
            )));
        }
        if transient_dirs.iter().any(|transient_dir| {
            entry_path == *transient_dir
                || entry_path.starts_with(transient_dir)
                || transient_dir.starts_with(&entry_path)
        }) {
            return Err(GitWorkflowError::Validation(format!(
                "Stage path list cannot include transient automation directories or their contents: {entry}"
            )));
        }

        stage_paths.push(entry.to_owned());
    }

    if stage_paths.is_empty() {
        return Err(GitWorkflowError::Validation(
            "Stage path list file has no usable entries".to_owned(),
        ));
    }

    Ok(stage_paths)
}

pub fn verify_explicit_track_branch(
    current_branch: Option<&str>,
    explicit_track: &ExplicitTrackBranch,
) -> Result<(), GitWorkflowError> {
    let Some(expected_branch) = explicit_track.expected_branch.as_deref() else {
        return Ok(());
    };

    match current_branch {
        None => Err(GitWorkflowError::NoBranch),
        Some("HEAD") => Err(GitWorkflowError::DetachedHead(format!(
            "expected branch '{expected_branch}', cannot verify"
        ))),
        Some(branch) if branch != expected_branch => Err(GitWorkflowError::BranchMismatch {
            current: branch.to_owned(),
            expected: expected_branch.to_owned(),
        }),
        Some(_) => Ok(()),
    }
}

pub fn verify_auto_detected_branch(
    current_branch: Option<&str>,
    claims: &[TrackBranchClaim],
) -> Result<(), GitWorkflowError> {
    let branch = match current_branch {
        Some(branch) => branch,
        None => return Err(GitWorkflowError::NoBranch),
    };
    if branch == "HEAD" {
        return Err(GitWorkflowError::DetachedHead("cannot verify track branch".to_owned()));
    }
    if !branch.starts_with("track/") {
        return Ok(());
    }

    let matches = claims
        .iter()
        .filter(|claim| {
            claim.branch.as_deref() == Some(branch) && claim.status.as_deref() != Some("archived")
        })
        .collect::<Vec<_>>();

    if matches.is_empty() {
        let archived_match = claims.iter().any(|claim| {
            claim.branch.as_deref() == Some(branch) && claim.status.as_deref() == Some("archived")
        });
        if archived_match {
            return Ok(());
        }

        let slug = branch.trim_start_matches("track/");
        let fallback_match = claims.iter().any(|claim| {
            claim.track_name == slug
                && claim.branch.is_none()
                && claim.status.as_deref() != Some("archived")
        });
        if fallback_match {
            return Ok(());
        }

        return Err(GitWorkflowError::Message(format!(
            "on branch '{branch}' but no track claims this branch in metadata.json"
        )));
    }

    if matches.len() > 1 {
        let names =
            matches.iter().map(|claim| claim.track_name.clone()).collect::<Vec<_>>().join(", ");
        return Err(GitWorkflowError::Message(format!(
            "multiple tracks claim branch '{branch}': {names}"
        )));
    }

    match matches.first() {
        Some(claim) => verify_explicit_track_branch(
            Some(branch),
            &ExplicitTrackBranch {
                display_path: claim.track_name.clone(),
                expected_branch: claim.branch.clone(),
                status: claim.status.clone(),
            },
        ),
        None => Err(GitWorkflowError::Message(
            "internal error: expected exactly one branch match".to_owned(),
        )),
    }
}

// ── GitWorkflowService ────────────────────────────────────────────────────────

use std::path::Path;
use std::sync::Arc;

/// Result type for git workflow operations.
pub type GitWorkflowResult<T> = Result<T, GitWorkflowError>;

/// Application service trait for guarded local git operations.
///
/// Abstracts `infrastructure::git_cli::SystemGitRepo` behind the usecase
/// boundary so that `cli_driver` never imports infrastructure directly.
pub trait GitWorkflowService: Send + Sync {
    /// Stage the whole worktree except transient automation scratch files.
    fn stage_all(&self) -> GitWorkflowResult<()>;

    /// Stage repo-relative paths listed in a file.
    ///
    /// If `cleanup` is true, the file is removed after staging.
    fn stage_from_file(&self, path: &Path, cleanup: bool) -> GitWorkflowResult<()>;

    /// Create a commit using the message stored in a file.
    ///
    /// If `cleanup` is true, the file is removed after committing.
    /// `track_dir` is used for branch guard validation (optional).
    fn commit_from_file(
        &self,
        path: &Path,
        cleanup: bool,
        track_dir: Option<&Path>,
    ) -> GitWorkflowResult<()>;

    /// Attach a git note using the contents of a file.
    ///
    /// If `cleanup` is true, the file is removed after attaching.
    fn note_from_file(&self, path: &Path, cleanup: bool) -> GitWorkflowResult<()>;

    /// Switch to a branch and pull latest changes.
    ///
    /// Returns a human-readable status string.
    fn switch_and_pull(&self, branch: &str) -> GitWorkflowResult<String>;

    /// Unstage paths (remove from git index without discarding worktree changes).
    fn unstage(&self, paths: &[PathBuf]) -> GitWorkflowResult<()>;

    /// Resolve the track ID from the current git branch (strict mode).
    ///
    /// - `Ok(Some(id))` → on a valid `track/<id>` branch.
    /// - `Ok(None)`     → on a non-track branch.
    /// - `Err(msg)`     → validation failure.
    fn current_branch_track_id(&self) -> GitWorkflowResult<Option<String>>;
}

// ── GitWorkflowInteractor ─────────────────────────────────────────────────────

/// Concrete interactor implementing [`GitWorkflowService`].
///
/// Delegates every method to the injected port. Infrastructure adapters
/// implementing [`GitWorkflowService`] are injected here so that `cli_driver`
/// never depends on `infrastructure` directly.
pub struct GitWorkflowInteractor {
    port: Arc<dyn GitWorkflowService>,
}

impl GitWorkflowInteractor {
    /// Create a new `GitWorkflowInteractor` with the given port.
    #[must_use]
    pub fn new(port: Arc<dyn GitWorkflowService>) -> Self {
        Self { port }
    }
}

impl GitWorkflowService for GitWorkflowInteractor {
    fn stage_all(&self) -> GitWorkflowResult<()> {
        self.port.stage_all()
    }

    fn stage_from_file(&self, path: &Path, cleanup: bool) -> GitWorkflowResult<()> {
        self.port.stage_from_file(path, cleanup)
    }

    fn commit_from_file(
        &self,
        path: &Path,
        cleanup: bool,
        track_dir: Option<&Path>,
    ) -> GitWorkflowResult<()> {
        self.port.commit_from_file(path, cleanup, track_dir)
    }

    fn note_from_file(&self, path: &Path, cleanup: bool) -> GitWorkflowResult<()> {
        self.port.note_from_file(path, cleanup)
    }

    fn switch_and_pull(&self, branch: &str) -> GitWorkflowResult<String> {
        self.port.switch_and_pull(branch)
    }

    fn unstage(&self, paths: &[PathBuf]) -> GitWorkflowResult<()> {
        self.port.unstage(paths)
    }

    fn current_branch_track_id(&self) -> GitWorkflowResult<Option<String>> {
        self.port.current_branch_track_id()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::{
        ExplicitTrackBranch, GitWorkflowError, TrackBranchClaim, validate_stage_path_entries,
        verify_auto_detected_branch, verify_explicit_track_branch,
    };

    #[test]
    fn validate_stage_path_entries_accepts_unique_repo_relative_paths() {
        let paths =
            validate_stage_path_entries(["src/lib.rs", "# comment", "src/lib.rs", "README.md"])
                .unwrap();

        assert_eq!(paths, vec!["src/lib.rs".to_owned(), "README.md".to_owned()]);
    }

    #[test]
    fn validate_stage_path_entries_rejects_transient_parent_directory() {
        let err = validate_stage_path_entries(["tmp/track-commit"]).unwrap_err();

        assert!(err.to_string().contains("transient automation"));
    }

    #[test]
    fn verify_explicit_track_branch_rejects_mismatch() {
        let err = verify_explicit_track_branch(
            Some("track/other"),
            &ExplicitTrackBranch {
                display_path: "track/items/example".to_owned(),
                expected_branch: Some("track/example".to_owned()),
                status: Some("in_progress".to_owned()),
            },
        )
        .unwrap_err();

        assert!(matches!(err, GitWorkflowError::BranchMismatch { .. }));
    }

    #[test]
    fn verify_auto_detected_branch_accepts_null_branch_fallback() {
        let claims = vec![TrackBranchClaim {
            track_name: "example".to_owned(),
            branch: None,
            status: Some("in_progress".to_owned()),
        }];

        assert!(verify_auto_detected_branch(Some("track/example"), &claims).is_ok());
    }

    #[test]
    fn verify_auto_detected_branch_rejects_archived_null_branch_fallback() {
        let claims = vec![TrackBranchClaim {
            track_name: "example".to_owned(),
            branch: None,
            status: Some("archived".to_owned()),
        }];

        let err = verify_auto_detected_branch(Some("track/example"), &claims).unwrap_err();

        assert!(err.to_string().contains("no track claims this branch"));
    }

    #[test]
    fn verify_auto_detected_branch_accepts_planned_null_branch_fallback() {
        let claims = vec![TrackBranchClaim {
            track_name: "example".to_owned(),
            branch: None,
            status: Some("planned".to_owned()),
        }];

        assert!(verify_auto_detected_branch(Some("track/example"), &claims).is_ok());
    }

    #[test]
    fn verify_auto_detected_branch_rejects_duplicate_claims() {
        let claims = vec![
            TrackBranchClaim {
                track_name: "one".to_owned(),
                branch: Some("track/example".to_owned()),
                status: Some("in_progress".to_owned()),
            },
            TrackBranchClaim {
                track_name: "two".to_owned(),
                branch: Some("track/example".to_owned()),
                status: Some("in_progress".to_owned()),
            },
        ];

        let err = verify_auto_detected_branch(Some("track/example"), &claims).unwrap_err();

        assert!(err.to_string().contains("multiple tracks claim branch"));
    }
}
