//! Private helpers shared across `CliApp` `review_v2` methods.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;

use crate::CommandOutcome;

use super::shared::CodexReviewOutcome;

pub(crate) fn record_instant_once(slot: &Mutex<Option<Instant>>) {
    if let Ok(mut recorded_at) = slot.lock() {
        if recorded_at.is_none() {
            *recorded_at = Some(Instant::now());
        }
    }
}

// ---------------------------------------------------------------------------
// Track-ID resolution
// ---------------------------------------------------------------------------

/// Resolves a track ID: uses the provided string if `Some`, otherwise
/// resolves from the current git branch name (`track/<id>`).
///
/// # Errors
/// Returns `Err` when branch detection fails or the branch is not a track branch.
pub(super) fn resolve_track_id_or_branch(
    track_id: Option<String>,
    items_dir: &std::path::Path,
) -> Result<String, String> {
    if let Some(id) = track_id {
        return Ok(id);
    }
    resolve_track_id_from_branch(items_dir)
}

/// Resolves a track ID for write operations (branch-guard variant).
///
/// When `track_id` is `Some`, validates that it matches the current branch.
/// When `None`, resolves from the current branch. Fail-closed on non-track
/// branches.
///
/// Git discovery is anchored to the repository root derived from `items_dir`
/// (stripping the trailing `track/items` segments), so that a relative
/// `items_dir` like `"track/items"` discovers the correct repo root even when
/// the process is invoked from a repo subdirectory.
///
/// # Errors
/// Returns `Err` when the explicit track ID does not match the current branch,
/// or when the current branch is not a track branch.
pub(super) fn resolve_track_id_or_branch_write(
    track_id: Option<String>,
    items_dir: &std::path::Path,
) -> Result<String, String> {
    crate::TrackCompositionRoot::new().track_resolve_id_for_write(track_id, items_dir.to_path_buf())
}

/// Resolves the current track ID from the active git branch (`track/<id>`).
///
/// Git discovery is anchored to the repository root derived from `items_dir`
/// (stripping the trailing `track/items` segments), matching the same anchor
/// strategy used by the write-guard variant and the pre-migration resolver.
///
/// # Errors
/// Returns `Err` when git discovery fails or the branch is not a track branch.
pub(super) fn resolve_track_id_from_branch(items_dir: &std::path::Path) -> Result<String, String> {
    use infrastructure::git_cli::{GitRepository, SystemGitRepo};

    let project_root = crate::track::resolve_project_root(items_dir)?;
    let output = SystemGitRepo::discover_from(&project_root)
        .and_then(|r| r.output(&["rev-parse", "--abbrev-ref", "HEAD"]))
        .map_err(|e| format!("failed to detect current branch: {e}"))?;

    let branch = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    branch.strip_prefix("track/").map(str::to_owned).ok_or_else(|| {
        format!(
            "current branch '{branch}' is not a track branch \
                 (expected 'track/<id>')"
        )
    })
}

// ---------------------------------------------------------------------------
// Prompt / outcome helpers
// ---------------------------------------------------------------------------

/// Builds the base prompt from an optional briefing file path or inline prompt.
///
/// # Errors
/// Returns `Err` when neither is provided or the briefing file does not exist.
pub(super) fn build_base_prompt_from_input(
    briefing_file: Option<PathBuf>,
    prompt: Option<String>,
) -> Result<String, String> {
    if let Some(path) = briefing_file {
        if !path.is_file() {
            return Err(format!("briefing file not found: {}", path.display()));
        }
        Ok(format!("Read {} and perform the task described there.", path.display()))
    } else {
        prompt.ok_or_else(|| "either --briefing-file or --prompt is required".to_owned())
    }
}

/// Converts a `CodexReviewOutcome` into a `CommandOutcome`.
///
/// The verdict JSON is written to stdout; the exit code is propagated directly.
///
/// # Errors
/// Returns `Err` for `SubprocessFailed` (the subprocess was launched but failed).
/// All other variants return `Ok`.
pub(super) fn outcome_to_command_outcome(
    outcome: CodexReviewOutcome,
) -> Result<CommandOutcome, String> {
    match outcome {
        CodexReviewOutcome::Skipped { scope_label } => {
            eprintln!("[auto-record] Scope '{scope_label}' is empty, skipping");
            Ok(CommandOutcome {
                stdout: Some(r#"{"verdict":"zero_findings","findings":[]}"#.to_owned()),
                stderr: None,
                exit_code: 0,
            })
        }
        CodexReviewOutcome::FinalCompleted { verdict_json, exit_code, .. } => {
            Ok(CommandOutcome { stdout: Some(verdict_json), stderr: None, exit_code })
        }
        CodexReviewOutcome::FastCompleted { verdict_json, exit_code, .. } => {
            Ok(CommandOutcome { stdout: Some(verdict_json), stderr: None, exit_code })
        }
        CodexReviewOutcome::SubprocessFailed { error, .. } => Err(error),
    }
}

// ---------------------------------------------------------------------------
// Path validation helpers
// ---------------------------------------------------------------------------

/// Returns `true` if `path` is safe to inject into a reviewer prompt.
///
/// Rejects: empty strings, control characters, line separators (U+2028/U+2029),
/// backticks, absolute paths (Unix/Windows/UNC), Windows drive-letter prefixes,
/// and `..` traversal components.
pub(super) fn is_safe_briefing_path(path: &str) -> bool {
    is_prompt_token_safe(path)
        && domain::review_v2::FilePath::new(path).is_ok()
        && !has_windows_drive_prefix(path)
}

fn is_prompt_token_safe(path: &str) -> bool {
    !path.is_empty()
        && path
            .chars()
            .all(|c| c != '`' && !c.is_control() && !matches!(c, '\u{2028}' | '\u{2029}'))
}

fn has_windows_drive_prefix(path: &str) -> bool {
    matches!(
        (path.as_bytes().first(), path.as_bytes().get(1)),
        (Some(first), Some(second)) if *second == b':' && first.is_ascii_alphabetic()
    )
}

/// Validates all paths and returns a joined error if any fail.
///
/// Mirrors `domain::FilePath::new` validation and rejects platform-specific
/// absolute forms: empty, Unix/UNC absolute, Windows drive-prefixed, and `..`
/// traversal paths are rejected.
///
/// # Errors
/// Returns a newline-joined string of all validation errors when any path fails.
pub(super) fn validate_all_paths(paths: &[String]) -> Result<(), String> {
    let mut errors: Vec<String> = Vec::new();
    for raw in paths {
        if raw.is_empty() {
            errors.push("invalid path: empty string".to_owned());
        } else if raw.starts_with('/') || raw.starts_with('\\') || has_windows_drive_prefix(raw) {
            errors.push(format!(
                "invalid path '{raw}': absolute paths are not allowed (use repo-relative)"
            ));
        } else {
            let has_traversal = raw.split(&['/', '\\'][..]).any(|seg| seg == "..");
            if has_traversal {
                errors.push(format!(
                    "invalid path '{raw}': '..' traversal components are not allowed"
                ));
            }
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors.join("\n")) }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub(crate) mod process_guards {
    use std::ffi::{OsStr, OsString};
    use std::process::Command;

    struct RestoreGuard {
        restore: Option<Box<dyn FnMut()>>,
    }

    impl RestoreGuard {
        fn new(restore: impl FnMut() + 'static) -> Self {
            Self { restore: Some(Box::new(restore)) }
        }
    }

    impl Drop for RestoreGuard {
        fn drop(&mut self) {
            if let Some(mut restore) = self.restore.take() {
                restore();
            }
        }
    }

    pub(crate) type CwdGuard = ScopedOverride;
    pub(crate) type EnvGuard = ScopedOverride;

    pub(crate) struct ScopedOverride {
        _restore: RestoreGuard,
    }

    impl ScopedOverride {
        pub(crate) fn save_current() -> Self {
            let original = std::env::current_dir().unwrap();
            Self::from_restore(move || {
                let _ = std::env::set_current_dir(&original);
            })
        }

        pub(crate) fn set(key: &'static str, value: impl Into<OsString>) -> Self {
            Self { _restore: env_restore_guard(key, Some(value.into())) }
        }

        pub(crate) fn remove(key: &'static str) -> Self {
            Self { _restore: env_restore_guard(key, None) }
        }

        fn from_restore(restore: impl FnMut() + 'static) -> Self {
            Self { _restore: RestoreGuard::new(restore) }
        }
    }

    fn env_restore_guard(key: &'static str, value: Option<OsString>) -> RestoreGuard {
        let previous = std::env::var_os(key);
        apply_env_value(key, value.as_deref());
        RestoreGuard::new(move || apply_env_value(key, previous.as_deref()))
    }

    fn apply_env_value(key: &'static str, value: Option<&OsStr>) {
        // Safety: tests that mutate process environment hold process_env_lock
        // for the full guard lifetime, so env mutation is serialized.
        unsafe {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }

    pub(crate) struct GitRunner<'a> {
        root: &'a std::path::Path,
    }

    impl<'a> GitRunner<'a> {
        pub(crate) fn at(root: &'a std::path::Path) -> Self {
            Self { root }
        }

        pub(crate) fn assert_success(self, args: &[&str]) {
            let status = Command::new("git").current_dir(self.root).args(args).status().unwrap();
            assert!(status.success(), "git {:?} exited with {status}", args);
        }
    }
}
