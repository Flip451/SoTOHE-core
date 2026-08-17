//! Private helpers shared across `CliApp` `review_v2` methods.

use std::path::PathBuf;

use crate::error::CompositionError;

use super::shared::CodexReviewOutcome;
use usecase::review_v2::RunReviewOutput;

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
) -> Result<String, CompositionError> {
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
) -> Result<String, CompositionError> {
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
pub(super) fn resolve_track_id_from_branch(
    items_dir: &std::path::Path,
) -> Result<String, CompositionError> {
    // Use the semantic `SystemGitRepo::current_branch` inherent method (T007
    // "semantic SystemGitRepo current_branch where listed") rather than a raw
    // `git rev-parse --abbrev-ref HEAD`.
    use infrastructure::git_cli::SystemGitRepo;

    let project_root = crate::track::resolve_project_root(items_dir)?;
    let branch = SystemGitRepo::discover_from(&project_root)
        .and_then(|r| r.current_branch())
        .map_err(|e| {
            CompositionError::AdapterInit(format!("failed to detect current branch: {e}"))
        })?
        .unwrap_or_default();

    branch.strip_prefix("track/").map(str::to_owned).ok_or_else(|| {
        CompositionError::WiringFailed(format!(
            "current branch '{branch}' is not a track branch \
                 (expected 'track/<id>')"
        ))
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
) -> Result<String, CompositionError> {
    if let Some(path) = briefing_file {
        if !path.is_file() {
            return Err(CompositionError::WiringFailed(format!(
                "briefing file not found: {}",
                path.display()
            )));
        }
        Ok(format!("Read {} and perform the task described there.", path.display()))
    } else {
        prompt.ok_or_else(|| {
            CompositionError::WiringFailed(
                "either --briefing-file or --prompt is required".to_owned(),
            )
        })
    }
}

/// Converts an internal review outcome into the structured usecase output.
///
/// # Errors
/// Returns `Err` for `SubprocessFailed` (the subprocess was launched but failed).
/// All other variants return `Ok`.
pub(super) fn outcome_to_run_review_output(
    outcome: CodexReviewOutcome,
) -> Result<RunReviewOutput, CompositionError> {
    match outcome {
        CodexReviewOutcome::WithDiagnostics { outcome, .. } => {
            outcome_to_run_review_output(*outcome)
        }
        CodexReviewOutcome::Skipped { .. } => Ok(RunReviewOutput {
            verdict_kind: "skipped".to_owned(),
            skipped: true,
            finding_count: 0,
            summary: Some(r#"{"verdict":"zero_findings","findings":[]}"#.to_owned()),
            exit_code: 0,
        }),
        CodexReviewOutcome::FinalCompleted { verdict_json, exit_code, findings_count, .. } => {
            Ok(RunReviewOutput {
                verdict_kind: if exit_code == 0 { "approved" } else { "rejected" }.to_owned(),
                skipped: false,
                finding_count: findings_count as usize,
                summary: Some(verdict_json),
                exit_code,
            })
        }
        CodexReviewOutcome::FastCompleted { verdict_json, exit_code, findings_count, .. } => {
            Ok(RunReviewOutput {
                verdict_kind: if exit_code == 0 { "approved" } else { "rejected" }.to_owned(),
                skipped: false,
                finding_count: findings_count as usize,
                summary: Some(verdict_json),
                exit_code,
            })
        }
        CodexReviewOutcome::SubprocessFailed { error, .. } => {
            Err(CompositionError::Infrastructure(error))
        }
    }
}

/// Collects diagnostics that belong exclusively to the local-review DTO.
pub(super) fn diagnostics_for_local_review(outcome: &CodexReviewOutcome) -> Vec<String> {
    match outcome {
        CodexReviewOutcome::WithDiagnostics { diagnostics, outcome } => {
            diagnostics.iter().cloned().chain(diagnostics_for_local_review(outcome)).collect()
        }
        CodexReviewOutcome::Skipped { scope_label } => {
            vec![format!("[auto-record] Scope '{scope_label}' is empty, skipping")]
        }
        CodexReviewOutcome::FinalCompleted { .. }
        | CodexReviewOutcome::FastCompleted { .. }
        | CodexReviewOutcome::SubprocessFailed { .. } => Vec::new(),
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
pub(super) fn validate_all_paths(paths: &[String]) -> Result<(), CompositionError> {
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
    if errors.is_empty() { Ok(()) } else { Err(CompositionError::WiringFailed(errors.join("\n"))) }
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
