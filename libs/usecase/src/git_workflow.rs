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

/// General-purpose newtype wrapping an unstructured diagnostic string.
///
/// Shared between `GitWorkflowError` and `infrastructure::git_cli::SyncError`
/// variant fields. Consumers: IN-06 / IN-08 / CN-02.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticText(String);

impl DiagnosticText {
    #[must_use]
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DiagnosticText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Errors returned by the git workflow orchestration surface.
#[derive(Debug, Error)]
pub enum GitWorkflowError {
    #[error("{0}")]
    Validation(DiagnosticText),
    #[error("cannot determine current git branch")]
    NoBranch,
    #[error("detached HEAD — {0}")]
    DetachedHead(DiagnosticText),
    #[error("branch mismatch: current '{current}' does not match expected '{expected}'")]
    BranchMismatch { current: DiagnosticText, expected: DiagnosticText },
    #[error("{0}")]
    Message(DiagnosticText),
    /// I/O or infrastructure failure (e.g. git process error, file read failure).
    #[error("git workflow unavailable: {0}")]
    Unavailable(DiagnosticText),
    /// Upstream not configured for the current branch. IN-06 / CN-02 / AC-08.
    #[error("current branch has no upstream configured")]
    SyncUpstreamNotSet,
    /// Non-fast-forward pull refused. IN-06 / CN-02 / AC-08.
    #[error("git pull refused: not a fast-forward: {stderr}")]
    SyncNonFastForward { stderr: DiagnosticText },
    /// Worktree not resolvable (dirty or operation in progress). IN-06 / CN-02 / AC-08.
    #[error("git pull refused: worktree unresolvable: {stderr}")]
    SyncWorktreeUnresolved { stderr: DiagnosticText },
    /// Filesystem operation failure surfaced from `TrackArchiveFsPort`. IN-10 / CN-03.
    #[error("filesystem operation failed: {detail}")]
    Fs { detail: DiagnosticText },
    /// `git switch <branch>` exited non-zero. Carries the target branch and the
    /// raw process exit code so composition callers can render the legacy
    /// checkout-failure outcome without string-sniffing. IN-06 / CN-02 / CN-05 / AC-04.
    #[error("git switch to '{branch}' failed with exit code {exit_code}")]
    SwitchFailed { branch: DiagnosticText, exit_code: i32 },
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
            return Err(GitWorkflowError::Validation(DiagnosticText::new(format!(
                "Stage path list must use repo-relative paths: {entry}"
            ))));
        }
        if entry_path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err(GitWorkflowError::Validation(DiagnosticText::new(format!(
                "Stage path list cannot escape the repo root: {entry}"
            ))));
        }
        if matches!(entry, "." | "./") {
            return Err(GitWorkflowError::Validation(DiagnosticText::new(format!(
                "Stage path list cannot use whole-worktree pathspecs: {entry}"
            ))));
        }
        if entry.starts_with(':') {
            return Err(GitWorkflowError::Validation(DiagnosticText::new(format!(
                "Stage path list cannot use git pathspec magic or shorthand: {entry}"
            ))));
        }
        if entry.chars().any(|ch| GLOB_MAGIC_CHARS.contains(&ch)) {
            return Err(GitWorkflowError::Validation(DiagnosticText::new(format!(
                "Stage path list cannot use glob patterns: {entry}"
            ))));
        }
        if transient_paths
            .iter()
            .any(|transient| entry_path == *transient || transient.starts_with(&entry_path))
        {
            return Err(GitWorkflowError::Validation(DiagnosticText::new(format!(
                "Stage path list cannot include transient automation files or their parent directories: {entry}"
            ))));
        }
        if transient_dirs.iter().any(|transient_dir| {
            entry_path == *transient_dir
                || entry_path.starts_with(transient_dir)
                || transient_dir.starts_with(&entry_path)
        }) {
            return Err(GitWorkflowError::Validation(DiagnosticText::new(format!(
                "Stage path list cannot include transient automation directories or their contents: {entry}"
            ))));
        }

        stage_paths.push(entry.to_owned());
    }

    if stage_paths.is_empty() {
        return Err(GitWorkflowError::Validation(DiagnosticText::new(
            "Stage path list file has no usable entries",
        )));
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
        Some("HEAD") => Err(GitWorkflowError::DetachedHead(DiagnosticText::new(format!(
            "expected branch '{expected_branch}', cannot verify"
        )))),
        Some(branch) if branch != expected_branch => Err(GitWorkflowError::BranchMismatch {
            current: DiagnosticText::new(branch),
            expected: DiagnosticText::new(expected_branch),
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
        return Err(GitWorkflowError::DetachedHead(DiagnosticText::new(
            "cannot verify track branch",
        )));
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

        return Err(GitWorkflowError::Message(DiagnosticText::new(format!(
            "on branch '{branch}' but no track claims this branch in metadata.json"
        ))));
    }

    if matches.len() > 1 {
        let names =
            matches.iter().map(|claim| claim.track_name.clone()).collect::<Vec<_>>().join(", ");
        return Err(GitWorkflowError::Message(DiagnosticText::new(format!(
            "multiple tracks claim branch '{branch}': {names}"
        ))));
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
        None => Err(GitWorkflowError::Message(DiagnosticText::new(
            "internal error: expected exactly one branch match",
        ))),
    }
}

// ── GitPrimitivePort / TrackArchiveFsPort ─────────────────────────────────────

use std::path::Path;
use std::sync::Arc;

/// Secondary port exposing the atomic git primitives required by the
/// usecase-layer interactors ([`GitWorkflowInteractor`], [`TrackGitInteractor`],
/// [`PrGitInteractor`], [`ReviewGitInteractor`]).
///
/// Every method takes an explicit `project_root: Option<&Path>` first parameter
/// so callers that anchor to a specific worktree (e.g. `--project-root` CLI
/// flag) never depend on the process CWD; `None` means "discover from CWD".
///
/// Implemented by `infrastructure::git_cli::workflow_adapter::FsGitWorkflowAdapter`.
///
/// IN-06 / IN-07 / IN-08 / IN-09 / CN-02 / CN-07 / AC-08.
pub trait GitPrimitivePort: Send + Sync {
    /// Read the current git branch name.
    fn current_branch(
        &self,
        project_root: Option<&Path>,
    ) -> Result<Option<String>, GitWorkflowError>;

    /// Fast-forward pull the current branch (ff-only) — the atomic `sync`
    /// primitive. IN-01 / IN-06 / CN-01 / AC-08.
    fn sync_current_branch(&self, project_root: Option<&Path>) -> Result<(), GitWorkflowError>;

    /// Check out (switch to) an existing branch.
    fn switch_branch(
        &self,
        project_root: Option<&Path>,
        branch: &str,
    ) -> Result<(), GitWorkflowError>;

    /// Create a new branch off `base_branch` and check it out.
    fn create_branch(
        &self,
        project_root: Option<&Path>,
        new_branch: &str,
        base_branch: &str,
    ) -> Result<(), GitWorkflowError>;

    /// Does the given branch (local or remote) exist?
    fn branch_exists(
        &self,
        project_root: Option<&Path>,
        branch: &str,
    ) -> Result<bool, GitWorkflowError>;

    /// `git mv src dst`.
    fn move_path(
        &self,
        project_root: Option<&Path>,
        src: &Path,
        dst: &Path,
    ) -> Result<(), GitWorkflowError>;

    /// `git fetch origin <branch>`.
    fn fetch_branch(
        &self,
        project_root: Option<&Path>,
        branch: &str,
    ) -> Result<(), GitWorkflowError>;

    /// `git show <git_ref>:<path>` — read the file at `git_ref`.
    fn show_file_at_ref(
        &self,
        project_root: Option<&Path>,
        git_ref: &str,
        path: &Path,
    ) -> Result<String, GitWorkflowError>;

    /// Resolve a git rev (e.g. `HEAD`, `origin/main`) to a [`domain::CommitHash`].
    fn resolve_commit(
        &self,
        project_root: Option<&Path>,
        rev: &str,
    ) -> Result<Option<domain::CommitHash>, GitWorkflowError>;

    /// Resolve the repository root path (discovered from `project_root` when
    /// given, otherwise from the process CWD). IN-10 / CN-03 / AC-09.
    fn resolve_repo_root(&self, project_root: Option<&Path>) -> Result<PathBuf, GitWorkflowError>;

    /// Stage every worktree change (equivalent to `git add -A`, excluding
    /// transient automation scratch files).
    fn stage_all(&self, project_root: Option<&Path>) -> Result<(), GitWorkflowError>;

    /// Stage repo-relative paths listed in `path`.
    fn stage_from_file(
        &self,
        project_root: Option<&Path>,
        path: &Path,
        cleanup: bool,
    ) -> Result<(), GitWorkflowError>;

    /// Create a commit using the message stored in `path`.
    fn commit_from_message_file(
        &self,
        project_root: Option<&Path>,
        path: &Path,
        cleanup: bool,
    ) -> Result<(), GitWorkflowError>;

    /// Attach a git note using the contents of `path`.
    fn note_from_file(
        &self,
        project_root: Option<&Path>,
        path: &Path,
        cleanup: bool,
    ) -> Result<(), GitWorkflowError>;

    /// Unstage paths (remove from git index without discarding worktree changes).
    fn unstage(
        &self,
        project_root: Option<&Path>,
        paths: &[PathBuf],
    ) -> Result<(), GitWorkflowError>;

    /// Read the explicit `track/items/<id>` metadata into an
    /// [`ExplicitTrackBranch`] (branch guard input).
    fn read_explicit_track_branch(
        &self,
        project_root: Option<&Path>,
        track_dir: &Path,
    ) -> Result<ExplicitTrackBranch, GitWorkflowError>;

    /// Collect the branch claims declared by all tracks (items + archive) into
    /// a `Vec<TrackBranchClaim>` (branch guard input).
    fn collect_track_branch_claims(
        &self,
        project_root: Option<&Path>,
    ) -> Result<Vec<TrackBranchClaim>, GitWorkflowError>;
}

/// Filesystem secondary port for the track-archive orchestration's non-git
/// steps. Failures map to [`GitWorkflowError::Fs`].
///
/// Implemented by `infrastructure::git_cli::workflow_adapter::FsWorkspaceAdapter`.
///
/// IN-08 / IN-10 / CN-03 / AC-09.
pub trait TrackArchiveFsPort: Send + Sync {
    /// fs: is path a directory?
    fn path_is_dir(&self, path: &Path) -> Result<bool, GitWorkflowError>;

    /// fs: does path exist?
    fn path_exists(&self, path: &Path) -> Result<bool, GitWorkflowError>;

    /// fs: create path and missing parents.
    fn create_dir_all(&self, path: &Path) -> Result<(), GitWorkflowError>;

    /// fs: rename src to dst.
    fn rename_path(&self, src: &Path, dst: &Path) -> Result<(), GitWorkflowError>;

    /// fs: list the immediate entries of path.
    fn list_dir_file_names(&self, path: &Path) -> Result<Vec<PathBuf>, GitWorkflowError>;

    /// fs: remove an empty directory.
    fn remove_dir(&self, path: &Path) -> Result<(), GitWorkflowError>;
}

// ── GitWorkflowService ────────────────────────────────────────────────────────

/// Application service trait for the `bin/sotp git` command family.
///
/// Implemented by [`GitWorkflowInteractor`], which composes the atomic
/// [`GitPrimitivePort`] primitives + the pure branch-guard validators
/// (`verify_explicit_track_branch` / `verify_auto_detected_branch`).
///
/// IN-01 / IN-04 / IN-06 / IN-08 / CN-01 / AC-01 / AC-06.
pub trait GitWorkflowService: Send + Sync {
    /// Stage the whole worktree except transient automation scratch files.
    fn stage_all(&self) -> Result<(), GitWorkflowError>;

    /// Stage repo-relative paths listed in a file.
    ///
    /// If `cleanup` is true, the file is removed after staging.
    fn stage_from_file(&self, path: &Path, cleanup: bool) -> Result<(), GitWorkflowError>;

    /// Create a commit using the message stored in a file.
    ///
    /// If `cleanup` is true, the file is removed after committing.
    /// `track_dir` is used for branch guard validation (optional). AC-06.
    fn commit_from_file(
        &self,
        path: &Path,
        cleanup: bool,
        track_dir: Option<&Path>,
    ) -> Result<(), GitWorkflowError>;

    /// Attach a git note using the contents of a file.
    ///
    /// If `cleanup` is true, the file is removed after attaching.
    fn note_from_file(&self, path: &Path, cleanup: bool) -> Result<(), GitWorkflowError>;

    /// Unstage paths (remove from git index without discarding worktree changes).
    fn unstage(&self, paths: &[PathBuf]) -> Result<(), GitWorkflowError>;

    /// Resolve the current branch's track ID into a [`domain::TrackId`].
    ///
    /// - `Ok(Some(id))` → on a valid `track/<id>` branch.
    /// - `Ok(None)`     → on a non-track branch.
    /// - `Err(_)`       → detached/unknown branch, or validation failure
    ///   (invalid slug). IN-08 / AC-06.
    fn current_branch_track_id(&self) -> Result<Option<domain::TrackId>, GitWorkflowError>;

    /// Fast-forward pull the current branch (ff-only). IN-01 / CN-01.
    fn sync_current_branch(&self) -> Result<(), GitWorkflowError>;
}

// ── GitWorkflowInteractor ─────────────────────────────────────────────────────

/// Concrete interactor implementing [`GitWorkflowService`] by composing
/// [`GitPrimitivePort`] atomic primitives + the pure branch-guard rules.
pub struct GitWorkflowInteractor {
    port: Arc<dyn GitPrimitivePort>,
}

impl GitWorkflowInteractor {
    /// Construct the interactor over the git-primitive port.
    #[must_use]
    pub fn new(port: Arc<dyn GitPrimitivePort>) -> Self {
        Self { port }
    }
}

impl GitWorkflowService for GitWorkflowInteractor {
    fn stage_all(&self) -> Result<(), GitWorkflowError> {
        self.port.stage_all(None)
    }

    fn stage_from_file(&self, path: &Path, cleanup: bool) -> Result<(), GitWorkflowError> {
        self.port.stage_from_file(None, path, cleanup)
    }

    fn commit_from_file(
        &self,
        path: &Path,
        cleanup: bool,
        track_dir: Option<&Path>,
    ) -> Result<(), GitWorkflowError> {
        // Fail-closed branch guard (AC-06): reject non-track-branch commits, then
        // apply either the explicit-track guard or the auto-detected guard.
        let current = self.port.current_branch(None)?;
        match current.as_deref() {
            Some(branch) if branch.starts_with("track/") => {}
            Some("HEAD") => {
                return Err(GitWorkflowError::DetachedHead(DiagnosticText::new(
                    "switch to a track branch before committing",
                )));
            }
            Some(_) => {
                return Err(GitWorkflowError::Message(DiagnosticText::new(
                    "non-track branch: switch to a track branch before committing",
                )));
            }
            None => return Err(GitWorkflowError::NoBranch),
        }

        if let Some(track_dir) = track_dir {
            let explicit = self.port.read_explicit_track_branch(None, track_dir)?;
            verify_explicit_track_branch(current.as_deref(), &explicit)?;
        } else {
            let claims = self.port.collect_track_branch_claims(None)?;
            verify_auto_detected_branch(current.as_deref(), &claims)?;
        }

        self.port.commit_from_message_file(None, path, cleanup)
    }

    fn note_from_file(&self, path: &Path, cleanup: bool) -> Result<(), GitWorkflowError> {
        self.port.note_from_file(None, path, cleanup)
    }

    fn unstage(&self, paths: &[PathBuf]) -> Result<(), GitWorkflowError> {
        self.port.unstage(None, paths)
    }

    fn current_branch_track_id(&self) -> Result<Option<domain::TrackId>, GitWorkflowError> {
        let branch = match self.port.current_branch(None)? {
            Some(b) => b,
            None => return Err(GitWorkflowError::NoBranch),
        };
        match crate::track_resolution::resolve_track_id_from_branch(Some(&branch)) {
            Ok(id) => {
                let track_id = domain::TrackId::try_new(id).map_err(|e| {
                    GitWorkflowError::Validation(DiagnosticText::new(e.to_string()))
                })?;
                Ok(Some(track_id))
            }
            Err(crate::track_resolution::TrackResolutionError::InvalidTrackId(slug, err)) => {
                Err(GitWorkflowError::Validation(DiagnosticText::new(format!(
                    "current branch 'track/{slug}' has an invalid track id: {err}"
                ))))
            }
            Err(crate::track_resolution::TrackResolutionError::DetachedHead) => {
                Err(GitWorkflowError::DetachedHead(DiagnosticText::new(
                    "switch to a track branch before resolving the current track id",
                )))
            }
            Err(crate::track_resolution::TrackResolutionError::NoBranch) => {
                Err(GitWorkflowError::NoBranch)
            }
            Err(crate::track_resolution::TrackResolutionError::NotTrackBranch(_)) => Ok(None),
            Err(other) => {
                Err(GitWorkflowError::Unavailable(DiagnosticText::new(other.to_string())))
            }
        }
    }

    fn sync_current_branch(&self) -> Result<(), GitWorkflowError> {
        self.port.sync_current_branch(None)
    }
}

// ── TrackGitInteractor / PrGitInteractor / ReviewGitInteractor ────────────────

mod flow_interactors;
pub use flow_interactors::{PrGitInteractor, ReviewGitInteractor, TrackGitInteractor};

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::{
        DiagnosticText, ExplicitTrackBranch, GitPrimitivePort, GitWorkflowError,
        GitWorkflowInteractor, GitWorkflowService, TrackArchiveFsPort, TrackBranchClaim,
        validate_stage_path_entries, verify_auto_detected_branch, verify_explicit_track_branch,
    };

    /// Compile-time check: both new secondary ports must be dyn-safe so that
    /// interactors can hold `Arc<dyn GitPrimitivePort>` / `Arc<dyn TrackArchiveFsPort>`.
    /// IN-08.
    #[test]
    fn ports_are_dyn_safe() {
        fn _assert_dyn_safe(_git: &dyn GitPrimitivePort, _fs: &dyn TrackArchiveFsPort) {}
    }

    #[test]
    fn diagnostic_text_new_preserves_input() {
        let d = DiagnosticText::new("upstream diverged");
        assert_eq!(d.as_str(), "upstream diverged");
    }

    #[test]
    fn diagnostic_text_display_delegates_to_inner_str() {
        let d = DiagnosticText::new(String::from("hello"));
        assert_eq!(format!("{d}"), "hello");
    }

    #[test]
    fn sync_non_fast_forward_display_embeds_stderr() {
        let err = GitWorkflowError::SyncNonFastForward {
            stderr: DiagnosticText::new("hint: upstream ahead"),
        };
        let msg = err.to_string();
        assert!(msg.contains("not a fast-forward"));
        assert!(msg.contains("hint: upstream ahead"));
    }

    #[test]
    fn sync_upstream_not_set_display_is_stable() {
        let err = GitWorkflowError::SyncUpstreamNotSet;
        assert_eq!(err.to_string(), "current branch has no upstream configured");
    }

    #[test]
    fn fs_variant_display_embeds_detail() {
        let err = GitWorkflowError::Fs { detail: DiagnosticText::new("archive rollback failed") };
        assert!(err.to_string().contains("archive rollback failed"));
    }

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

    // ── Interactor mock-port tests ────────────────────────────────────────

    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    use super::{PrGitInteractor, ReviewGitInteractor, TrackGitInteractor};

    /// Minimal in-memory [`GitPrimitivePort`] mock. Every method returns a
    /// canned value taken from the recorded state; callers program the state
    /// with `set_*` helpers before running the interactor.
    #[derive(Default)]
    struct MockGitPrimitive {
        current_branch: Mutex<Option<String>>,
        branch_exists: Mutex<std::collections::BTreeMap<String, bool>>,
        sync_result: Mutex<Option<GitWorkflowError>>,
        sync_calls: Mutex<usize>,
        commits: Mutex<std::collections::BTreeMap<String, Option<domain::CommitHash>>>,
        move_paths: Mutex<Vec<(PathBuf, PathBuf)>>,
        move_results: Mutex<std::collections::VecDeque<Result<(), GitWorkflowError>>>,
        created_branches: Mutex<Vec<String>>,
        switched_branches: Mutex<Vec<String>>,
        fetched: Mutex<Vec<String>>,
        show_file_at_ref: Mutex<std::collections::BTreeMap<(String, PathBuf), String>>,
    }

    impl GitPrimitivePort for MockGitPrimitive {
        fn current_branch(
            &self,
            _project_root: Option<&Path>,
        ) -> Result<Option<String>, GitWorkflowError> {
            Ok(self.current_branch.lock().unwrap().clone())
        }

        fn sync_current_branch(
            &self,
            _project_root: Option<&Path>,
        ) -> Result<(), GitWorkflowError> {
            *self.sync_calls.lock().unwrap() += 1;
            if let Some(err) = self.sync_result.lock().unwrap().take() {
                return Err(err);
            }
            Ok(())
        }

        fn switch_branch(
            &self,
            _project_root: Option<&Path>,
            branch: &str,
        ) -> Result<(), GitWorkflowError> {
            self.switched_branches.lock().unwrap().push(branch.to_owned());
            *self.current_branch.lock().unwrap() = Some(branch.to_owned());
            Ok(())
        }

        fn create_branch(
            &self,
            _project_root: Option<&Path>,
            new_branch: &str,
            _base_branch: &str,
        ) -> Result<(), GitWorkflowError> {
            self.created_branches.lock().unwrap().push(new_branch.to_owned());
            self.branch_exists.lock().unwrap().insert(new_branch.to_owned(), true);
            Ok(())
        }

        fn branch_exists(
            &self,
            _project_root: Option<&Path>,
            branch: &str,
        ) -> Result<bool, GitWorkflowError> {
            Ok(self.branch_exists.lock().unwrap().get(branch).copied().unwrap_or(false))
        }

        fn move_path(
            &self,
            _project_root: Option<&Path>,
            src: &Path,
            dst: &Path,
        ) -> Result<(), GitWorkflowError> {
            self.move_paths.lock().unwrap().push((src.to_path_buf(), dst.to_path_buf()));
            if let Some(result) = self.move_results.lock().unwrap().pop_front() {
                return result;
            }
            Ok(())
        }

        fn fetch_branch(
            &self,
            _project_root: Option<&Path>,
            branch: &str,
        ) -> Result<(), GitWorkflowError> {
            self.fetched.lock().unwrap().push(branch.to_owned());
            Ok(())
        }

        fn show_file_at_ref(
            &self,
            _project_root: Option<&Path>,
            git_ref: &str,
            path: &Path,
        ) -> Result<String, GitWorkflowError> {
            match self
                .show_file_at_ref
                .lock()
                .unwrap()
                .get(&(git_ref.to_owned(), path.to_path_buf()))
                .cloned()
            {
                Some(content) => Ok(content),
                None => Err(GitWorkflowError::Unavailable(DiagnosticText::new(format!(
                    "no fixture for {git_ref}:{}",
                    path.display()
                )))),
            }
        }

        fn resolve_commit(
            &self,
            _project_root: Option<&Path>,
            rev: &str,
        ) -> Result<Option<domain::CommitHash>, GitWorkflowError> {
            Ok(self.commits.lock().unwrap().get(rev).cloned().unwrap_or(None))
        }

        fn resolve_repo_root(
            &self,
            project_root: Option<&Path>,
        ) -> Result<PathBuf, GitWorkflowError> {
            Ok(project_root.map_or_else(|| PathBuf::from("/repo"), Path::to_path_buf))
        }

        fn stage_all(&self, _project_root: Option<&Path>) -> Result<(), GitWorkflowError> {
            Ok(())
        }

        fn stage_from_file(
            &self,
            _project_root: Option<&Path>,
            _path: &Path,
            _cleanup: bool,
        ) -> Result<(), GitWorkflowError> {
            Ok(())
        }

        fn commit_from_message_file(
            &self,
            _project_root: Option<&Path>,
            _path: &Path,
            _cleanup: bool,
        ) -> Result<(), GitWorkflowError> {
            Ok(())
        }

        fn note_from_file(
            &self,
            _project_root: Option<&Path>,
            _path: &Path,
            _cleanup: bool,
        ) -> Result<(), GitWorkflowError> {
            Ok(())
        }

        fn unstage(
            &self,
            _project_root: Option<&Path>,
            _paths: &[PathBuf],
        ) -> Result<(), GitWorkflowError> {
            Ok(())
        }

        fn read_explicit_track_branch(
            &self,
            _project_root: Option<&Path>,
            _track_dir: &Path,
        ) -> Result<ExplicitTrackBranch, GitWorkflowError> {
            Ok(ExplicitTrackBranch {
                display_path: String::new(),
                expected_branch: None,
                status: None,
            })
        }

        fn collect_track_branch_claims(
            &self,
            _project_root: Option<&Path>,
        ) -> Result<Vec<TrackBranchClaim>, GitWorkflowError> {
            Ok(Vec::new())
        }
    }

    /// Minimal in-memory [`TrackArchiveFsPort`] mock.
    #[derive(Default)]
    struct MockFsPort {
        dirs: Mutex<std::collections::BTreeSet<PathBuf>>,
        files: Mutex<std::collections::BTreeSet<PathBuf>>,
        renames: Mutex<Vec<(PathBuf, PathBuf)>>,
        removed_dirs: Mutex<Vec<PathBuf>>,
        path_is_dir_results:
            Mutex<std::collections::BTreeMap<PathBuf, std::collections::VecDeque<bool>>>,
        path_exists_results:
            Mutex<std::collections::BTreeMap<PathBuf, std::collections::VecDeque<bool>>>,
        list_entries: Mutex<std::collections::BTreeMap<PathBuf, Vec<PathBuf>>>,
        rename_result: Mutex<Option<GitWorkflowError>>,
    }

    impl MockFsPort {
        fn add_dir(&self, path: &Path) {
            self.dirs.lock().unwrap().insert(path.to_path_buf());
        }

        fn queue_path_is_dir(&self, path: &Path, results: impl IntoIterator<Item = bool>) {
            self.path_is_dir_results
                .lock()
                .unwrap()
                .insert(path.to_path_buf(), results.into_iter().collect());
        }

        fn queue_path_exists(&self, path: &Path, results: impl IntoIterator<Item = bool>) {
            self.path_exists_results
                .lock()
                .unwrap()
                .insert(path.to_path_buf(), results.into_iter().collect());
        }

        fn set_list_entries(&self, path: &Path, entries: Vec<PathBuf>) {
            self.list_entries.lock().unwrap().insert(path.to_path_buf(), entries);
        }
    }

    impl TrackArchiveFsPort for MockFsPort {
        fn path_is_dir(&self, path: &Path) -> Result<bool, GitWorkflowError> {
            if let Some(result) = self
                .path_is_dir_results
                .lock()
                .unwrap()
                .get_mut(path)
                .and_then(std::collections::VecDeque::pop_front)
            {
                return Ok(result);
            }
            Ok(self.dirs.lock().unwrap().contains(path))
        }

        fn path_exists(&self, path: &Path) -> Result<bool, GitWorkflowError> {
            if let Some(result) = self
                .path_exists_results
                .lock()
                .unwrap()
                .get_mut(path)
                .and_then(std::collections::VecDeque::pop_front)
            {
                return Ok(result);
            }
            let dirs = self.dirs.lock().unwrap();
            let files = self.files.lock().unwrap();
            Ok(dirs.contains(path) || files.contains(path))
        }

        fn create_dir_all(&self, path: &Path) -> Result<(), GitWorkflowError> {
            self.dirs.lock().unwrap().insert(path.to_path_buf());
            Ok(())
        }

        fn rename_path(&self, src: &Path, dst: &Path) -> Result<(), GitWorkflowError> {
            self.renames.lock().unwrap().push((src.to_path_buf(), dst.to_path_buf()));
            if let Some(err) = self.rename_result.lock().unwrap().take() {
                return Err(err);
            }
            let mut dirs = self.dirs.lock().unwrap();
            if dirs.remove(src) {
                dirs.insert(dst.to_path_buf());
            }
            Ok(())
        }

        fn list_dir_file_names(&self, path: &Path) -> Result<Vec<PathBuf>, GitWorkflowError> {
            Ok(self.list_entries.lock().unwrap().get(path).cloned().unwrap_or_default())
        }

        fn remove_dir(&self, path: &Path) -> Result<(), GitWorkflowError> {
            self.removed_dirs.lock().unwrap().push(path.to_path_buf());
            self.dirs.lock().unwrap().remove(path);
            Ok(())
        }
    }

    fn track_id(s: &str) -> domain::TrackId {
        domain::TrackId::try_new(s).unwrap()
    }

    // GitWorkflowInteractor tests -------------------------------------------

    #[test]
    fn git_workflow_commit_from_file_preserves_auto_branch_guard_error() {
        let git = Arc::new(MockGitPrimitive::default());
        *git.current_branch.lock().unwrap() = Some("track/feature-2026-07-04".to_owned());
        let interactor = GitWorkflowInteractor::new(git);

        let err = interactor
            .commit_from_file(Path::new("tmp/track-commit/commit-message.txt"), false, None)
            .unwrap_err();

        assert!(matches!(err, GitWorkflowError::Message(_)));
    }

    #[test]
    fn git_workflow_current_branch_track_id_fails_on_detached_head() {
        let git = Arc::new(MockGitPrimitive::default());
        *git.current_branch.lock().unwrap() = Some("HEAD".to_owned());
        let interactor = GitWorkflowInteractor::new(git);

        let err = interactor.current_branch_track_id().unwrap_err();

        assert!(matches!(err, GitWorkflowError::DetachedHead(_)));
    }

    #[test]
    fn git_workflow_current_branch_track_id_fails_when_branch_unknown() {
        let git = Arc::new(MockGitPrimitive::default());
        let interactor = GitWorkflowInteractor::new(git);

        let err = interactor.current_branch_track_id().unwrap_err();

        assert!(matches!(err, GitWorkflowError::NoBranch));
    }

    #[test]
    fn git_workflow_current_branch_track_id_parses_valid_track_branch() {
        let git = Arc::new(MockGitPrimitive::default());
        *git.current_branch.lock().unwrap() = Some("track/feature-2026-07-04".to_owned());
        let interactor = GitWorkflowInteractor::new(git);

        let track_id = interactor.current_branch_track_id().unwrap().unwrap();

        assert_eq!(track_id.as_ref(), "feature-2026-07-04");
    }

    #[test]
    fn git_workflow_current_branch_track_id_returns_none_on_non_track_branch() {
        let git = Arc::new(MockGitPrimitive::default());
        *git.current_branch.lock().unwrap() = Some("main".to_owned());
        let interactor = GitWorkflowInteractor::new(git);

        let track_id = interactor.current_branch_track_id().unwrap();

        assert_eq!(track_id, None);
    }

    #[test]
    fn git_workflow_sync_current_branch_delegates_to_port() {
        let git = Arc::new(MockGitPrimitive::default());
        let interactor = GitWorkflowInteractor::new(git.clone());

        interactor.sync_current_branch().unwrap();

        assert_eq!(*git.sync_calls.lock().unwrap(), 1);
    }

    // TrackGitInteractor tests --------------------------------------------

    #[test]
    fn track_git_create_track_branch_happy_creates_branch_off_base() {
        let git = Arc::new(MockGitPrimitive::default());
        let fs = Arc::new(MockFsPort::default());
        *git.current_branch.lock().unwrap() = Some("main".to_owned());
        let interactor = TrackGitInteractor::new(git.clone(), fs);

        interactor
            .create_track_branch(Path::new("/repo"), &track_id("feature-2026-07-04"), "main")
            .unwrap();

        assert_eq!(
            git.created_branches.lock().unwrap().as_slice(),
            &["track/feature-2026-07-04".to_owned()]
        );
    }

    #[test]
    fn track_git_create_track_branch_fails_when_not_on_base_branch() {
        let git = Arc::new(MockGitPrimitive::default());
        let fs = Arc::new(MockFsPort::default());
        *git.current_branch.lock().unwrap() = Some("track/other".to_owned());
        let interactor = TrackGitInteractor::new(git, fs);

        let err = interactor
            .create_track_branch(Path::new("/repo"), &track_id("feature-2026-07-04"), "main")
            .unwrap_err();

        assert!(err.to_string().contains("branch create must start from 'main'"));
    }

    #[test]
    fn track_git_create_track_branch_rejects_existing_branch() {
        let git = Arc::new(MockGitPrimitive::default());
        let fs = Arc::new(MockFsPort::default());
        *git.current_branch.lock().unwrap() = Some("main".to_owned());
        git.branch_exists.lock().unwrap().insert("track/feature-2026-07-04".to_owned(), true);
        let interactor = TrackGitInteractor::new(git, fs);

        let err = interactor
            .create_track_branch(Path::new("/repo"), &track_id("feature-2026-07-04"), "main")
            .unwrap_err();

        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn track_git_switch_to_track_branch_happy_switches_branch() {
        let git = Arc::new(MockGitPrimitive::default());
        let fs = Arc::new(MockFsPort::default());
        git.branch_exists.lock().unwrap().insert("track/feature-2026-07-04".to_owned(), true);
        let interactor = TrackGitInteractor::new(git.clone(), fs);

        let msg = interactor
            .switch_to_track_branch(Path::new("/repo"), &track_id("feature-2026-07-04"))
            .unwrap();

        assert!(msg.contains("track/feature-2026-07-04"));
        assert_eq!(
            git.switched_branches.lock().unwrap().as_slice(),
            &["track/feature-2026-07-04".to_owned()]
        );
    }

    #[test]
    fn track_git_switch_to_track_branch_fails_when_branch_missing() {
        let git = Arc::new(MockGitPrimitive::default());
        let fs = Arc::new(MockFsPort::default());
        let interactor = TrackGitInteractor::new(git, fs);

        let err = interactor
            .switch_to_track_branch(Path::new("/repo"), &track_id("feature-2026-07-04"))
            .unwrap_err();

        assert!(err.to_string().contains("does not exist"));
    }

    #[test]
    fn track_git_switch_to_base_happy_switches_then_syncs() {
        let git = Arc::new(MockGitPrimitive::default());
        let fs = Arc::new(MockFsPort::default());
        let interactor = TrackGitInteractor::new(git.clone(), fs);

        let msg = interactor.switch_to_base(Path::new("/repo"), "main").unwrap();

        assert!(msg.contains("[OK] On main"));
        assert_eq!(git.switched_branches.lock().unwrap().as_slice(), &["main".to_owned()]);
    }

    #[test]
    fn track_git_switch_to_base_downgrades_sync_upstream_not_set_to_warning() {
        let git = Arc::new(MockGitPrimitive::default());
        let fs = Arc::new(MockFsPort::default());
        *git.sync_result.lock().unwrap() = Some(GitWorkflowError::SyncUpstreamNotSet);
        let interactor = TrackGitInteractor::new(git, fs);

        let msg = interactor.switch_to_base(Path::new("/repo"), "main").unwrap();

        assert!(msg.contains("[WARN] Pull failed"));
    }

    #[test]
    fn track_git_switch_to_base_downgrades_non_fast_forward_to_warning() {
        let git = Arc::new(MockGitPrimitive::default());
        let fs = Arc::new(MockFsPort::default());
        *git.sync_result.lock().unwrap() = Some(GitWorkflowError::SyncNonFastForward {
            stderr: DiagnosticText::new("origin/main diverged"),
        });
        let interactor = TrackGitInteractor::new(git, fs);

        let msg = interactor.switch_to_base(Path::new("/repo"), "main").unwrap();

        assert!(msg.contains("[WARN] Pull failed"));
    }

    #[test]
    fn track_git_switch_to_base_downgrades_spawn_unavailable_to_warning() {
        // CN-05 regression guard: `SyncError::Spawn` (auth / network / process
        // spawn failure) surfaces as `GitWorkflowError::Unavailable` from the
        // adapter. The base checkout has already succeeded, so the previous
        // inline `git_switch_and_pull_impl` folded every pull failure into
        // `[WARN] Pull failed` and returned exit 0. `switch_to_base` must
        // preserve that contract or the `/track:done` flow gets stranded on a
        // transient pull failure.
        let git = Arc::new(MockGitPrimitive::default());
        let fs = Arc::new(MockFsPort::default());
        *git.sync_result.lock().unwrap() =
            Some(GitWorkflowError::Unavailable(DiagnosticText::new("git process spawn failed")));
        let interactor = TrackGitInteractor::new(git, fs);

        let msg = interactor.switch_to_base(Path::new("/repo"), "main").unwrap();

        assert!(msg.contains("[WARN] Pull failed"), "expected WARN fold, got: {msg}");
    }

    #[test]
    fn track_git_archive_track_happy_moves_track_directory() {
        let git = Arc::new(MockGitPrimitive::default());
        let fs = Arc::new(MockFsPort::default());
        let project_root = Path::new("/repo");
        fs.add_dir(&project_root.join("track/items/feature-2026-07-04"));
        let interactor = TrackGitInteractor::new(git.clone(), fs);

        let msg = interactor.archive_track(project_root, &track_id("feature-2026-07-04")).unwrap();

        assert!(msg.contains("Archived track"));
        let moves = git.move_paths.lock().unwrap();
        assert_eq!(moves.len(), 1);
        let (src, dst) = moves.first().unwrap();
        assert_eq!(src, &project_root.join("track/items/feature-2026-07-04"));
        assert_eq!(dst, &project_root.join("track/archive/feature-2026-07-04"));
    }

    #[test]
    fn track_git_archive_track_fails_when_source_missing() {
        let git = Arc::new(MockGitPrimitive::default());
        let fs = Arc::new(MockFsPort::default());
        let interactor = TrackGitInteractor::new(git, fs);

        let err = interactor
            .archive_track(Path::new("/repo"), &track_id("feature-2026-07-04"))
            .unwrap_err();

        assert!(err.to_string().contains("track directory not found"));
    }

    #[test]
    fn track_git_archive_track_fails_when_destination_exists() {
        let git = Arc::new(MockGitPrimitive::default());
        let fs = Arc::new(MockFsPort::default());
        let project_root = Path::new("/repo");
        fs.add_dir(&project_root.join("track/items/feature-2026-07-04"));
        fs.add_dir(&project_root.join("track/archive/feature-2026-07-04"));
        let interactor = TrackGitInteractor::new(git, fs);

        let err =
            interactor.archive_track(project_root, &track_id("feature-2026-07-04")).unwrap_err();

        assert!(err.to_string().contains("archive destination already exists"));
    }

    #[test]
    fn track_git_archive_track_reports_rollback_failure_after_logs_rename_failure() {
        let git = Arc::new(MockGitPrimitive::default());
        let fs = Arc::new(MockFsPort::default());
        let project_root = Path::new("/repo");
        let src_dir = project_root.join("track/items/feature-2026-07-04");
        let dst_dir = project_root.join("track/archive/feature-2026-07-04");
        fs.add_dir(&src_dir);
        fs.add_dir(&src_dir.join("logs"));
        fs.queue_path_exists(&dst_dir, [false, true]);
        fs.set_list_entries(&dst_dir, vec![dst_dir.join("tracked.txt")]);
        *fs.rename_result.lock().unwrap() =
            Some(GitWorkflowError::Fs { detail: DiagnosticText::new("rename failed") });
        git.move_results.lock().unwrap().push_back(Ok(()));
        git.move_results
            .lock()
            .unwrap()
            .push_back(Err(GitWorkflowError::Unavailable(DiagnosticText::new("rollback failed"))));
        let interactor = TrackGitInteractor::new(git.clone(), fs);

        let err =
            interactor.archive_track(project_root, &track_id("feature-2026-07-04")).unwrap_err();

        assert!(matches!(err, GitWorkflowError::Fs { .. }));
        let msg = err.to_string();
        assert!(msg.contains("rename failed"));
        assert!(msg.contains("rollback failed"));
        assert_eq!(
            git.move_paths.lock().unwrap().as_slice(),
            &[
                (src_dir.clone(), dst_dir.clone()),
                (dst_dir.join("tracked.txt"), src_dir.join("tracked.txt")),
            ]
        );
    }

    #[test]
    fn track_git_archive_track_rolls_back_when_logs_disappear_after_git_move() {
        let git = Arc::new(MockGitPrimitive::default());
        let fs = Arc::new(MockFsPort::default());
        let project_root = Path::new("/repo");
        let src_dir = project_root.join("track/items/feature-2026-07-04");
        let dst_dir = project_root.join("track/archive/feature-2026-07-04");
        let src_logs = src_dir.join("logs");
        fs.add_dir(&src_dir);
        fs.queue_path_is_dir(&src_logs, [true, false]);
        fs.queue_path_exists(&dst_dir, [false, true]);
        fs.set_list_entries(&dst_dir, vec![dst_dir.join("tracked.txt")]);
        git.move_results.lock().unwrap().push_back(Ok(()));
        git.move_results.lock().unwrap().push_back(Ok(()));
        let interactor = TrackGitInteractor::new(git.clone(), fs.clone());

        let err =
            interactor.archive_track(project_root, &track_id("feature-2026-07-04")).unwrap_err();

        assert!(matches!(err, GitWorkflowError::Fs { .. }));
        let msg = err.to_string();
        assert!(msg.contains("logs/ was present before archive"));
        assert!(msg.contains("rollback succeeded"));
        assert_eq!(
            git.move_paths.lock().unwrap().as_slice(),
            &[
                (src_dir.clone(), dst_dir.clone()),
                (dst_dir.join("tracked.txt"), src_dir.join("tracked.txt")),
            ]
        );
        assert_eq!(fs.removed_dirs.lock().unwrap().as_slice(), &[dst_dir]);
    }

    // PrGitInteractor tests -----------------------------------------------

    #[test]
    fn pr_git_fetch_and_read_metadata_at_ref_happy_returns_metadata() {
        let git = Arc::new(MockGitPrimitive::default());
        git.show_file_at_ref.lock().unwrap().insert(
            (
                "origin/track/feature-2026-07-04".to_owned(),
                Path::new("track/items/feature-2026-07-04/metadata.json").to_path_buf(),
            ),
            "{\"branch\":\"track/feature-2026-07-04\"}".to_owned(),
        );
        let interactor = PrGitInteractor::new(git.clone());

        let json = interactor
            .fetch_and_read_metadata_at_ref(
                "track/feature-2026-07-04",
                &track_id("feature-2026-07-04"),
            )
            .unwrap();

        assert!(json.contains("track/feature-2026-07-04"));
        assert_eq!(
            git.fetched.lock().unwrap().as_slice(),
            &["track/feature-2026-07-04".to_owned()]
        );
    }

    #[test]
    fn pr_git_fetch_and_read_metadata_at_ref_propagates_missing_fixture_error() {
        let git = Arc::new(MockGitPrimitive::default());
        let interactor = PrGitInteractor::new(git);

        let err = interactor
            .fetch_and_read_metadata_at_ref(
                "track/feature-2026-07-04",
                &track_id("feature-2026-07-04"),
            )
            .unwrap_err();

        assert!(matches!(err, GitWorkflowError::Unavailable(_)));
    }

    #[test]
    fn pr_git_resolve_head_returns_commit_hash_when_head_is_set() {
        let git = Arc::new(MockGitPrimitive::default());
        let hash = domain::CommitHash::try_new("0123456789abcdef0123456789abcdef01234567").unwrap();
        git.commits.lock().unwrap().insert("HEAD".to_owned(), Some(hash.clone()));
        let interactor = PrGitInteractor::new(git);

        assert_eq!(interactor.resolve_head().unwrap(), Some(hash));
    }

    #[test]
    fn pr_git_resolve_head_returns_none_for_empty_repo() {
        let git = Arc::new(MockGitPrimitive::default());
        let interactor = PrGitInteractor::new(git);

        assert!(interactor.resolve_head().unwrap().is_none());
    }

    // ReviewGitInteractor tests --------------------------------------------

    #[test]
    fn review_git_resolve_head_for_track_branch_happy_returns_commit_hash() {
        let git = Arc::new(MockGitPrimitive::default());
        let hash = domain::CommitHash::try_new("0123456789abcdef0123456789abcdef01234567").unwrap();
        git.commits.lock().unwrap().insert("HEAD".to_owned(), Some(hash.clone()));
        let interactor = ReviewGitInteractor::new(git);

        assert_eq!(
            interactor.resolve_head_for_track_branch(&track_id("feature-2026-07-04")).unwrap(),
            hash
        );
    }

    #[test]
    fn review_git_resolve_head_for_track_branch_fails_when_head_missing() {
        let git = Arc::new(MockGitPrimitive::default());
        let interactor = ReviewGitInteractor::new(git);

        let err =
            interactor.resolve_head_for_track_branch(&track_id("feature-2026-07-04")).unwrap_err();

        assert!(err.to_string().contains("cannot resolve HEAD"));
    }

    #[test]
    fn review_git_resolve_diff_base_returns_hash_or_none() {
        let git = Arc::new(MockGitPrimitive::default());
        let hash = domain::CommitHash::try_new("abcdef0123456789abcdef0123456789abcdef01").unwrap();
        git.commits.lock().unwrap().insert("origin/main".to_owned(), Some(hash.clone()));
        let interactor = ReviewGitInteractor::new(git);

        assert_eq!(interactor.resolve_diff_base("origin/main").unwrap(), Some(hash));
        assert!(interactor.resolve_diff_base("missing").unwrap().is_none());
    }
}
