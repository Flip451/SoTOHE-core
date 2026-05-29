//! `track` command family — core CliApp impl methods.

mod ops;
mod resolution;
mod tddd;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::{CliApp, CommandOutcome};

// ---------------------------------------------------------------------------
// Track ID resolution helpers (mirrors apps/cli/src/commands/track/mod.rs)
// ---------------------------------------------------------------------------

/// Validates a track ID string (lowercase slug).
pub(crate) fn validate_track_id_str(value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("invalid track id: '{value}' (must not be empty)"));
    }
    let mut chars = value.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() || first.is_ascii_digit() => {}
        _ => {
            return Err(format!(
                "invalid track id: '{value}' (must start with lowercase letter or digit)"
            ));
        }
    }
    let mut previous_was_hyphen = false;
    for ch in chars {
        let is_valid = ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-';
        if !is_valid {
            return Err(format!("invalid track id: '{value}' (invalid character '{ch}')"));
        }
        if ch == '-' && previous_was_hyphen {
            return Err(format!("invalid track id: '{value}' (double hyphen not allowed)"));
        }
        previous_was_hyphen = ch == '-';
    }
    if previous_was_hyphen {
        return Err(format!("invalid track id: '{value}' (must not end with hyphen)"));
    }
    Ok(())
}

/// Resolves `<project-root>/track/items` → `<project-root>`.
pub(crate) fn resolve_project_root(items_dir: &Path) -> Result<PathBuf, String> {
    let items_name = items_dir.file_name().and_then(|n| n.to_str());
    let track_dir = items_dir.parent();
    let track_name = track_dir.and_then(Path::file_name).and_then(|n| n.to_str());
    let project_root = track_dir.and_then(Path::parent);
    match (items_name, track_name, project_root) {
        (Some("items"), Some("track"), Some(root)) => {
            if root.as_os_str().is_empty() {
                Ok(PathBuf::from("."))
            } else {
                Ok(root.to_path_buf())
            }
        }
        _ => Err(format!(
            "--items-dir must point to '<project-root>/track/items'; got {}",
            items_dir.display()
        )),
    }
}

/// Resolve track ID for READ (explicit overrides discovery).
pub(crate) fn resolve_track_id(
    explicit_id: Option<String>,
    items_dir: &Path,
) -> Result<String, String> {
    if let Some(id) = explicit_id {
        return Ok(id);
    }
    let project_root = resolve_project_root(items_dir)?;
    resolve_track_id_inner(None, &project_root, false)
}

/// Resolve track ID for READ, anchored to workspace_root.
pub(crate) fn resolve_track_id_from_root(
    explicit_id: Option<String>,
    workspace_root: &Path,
) -> Result<String, String> {
    resolve_track_id_inner(explicit_id, workspace_root, false)
}

/// Resolve track ID for WRITE (branch always read, explicit must match).
fn resolve_track_id_for_write(
    explicit_id: Option<String>,
    items_dir: &Path,
) -> Result<String, String> {
    let project_root = resolve_project_root(items_dir)?;
    resolve_track_id_inner(explicit_id, &project_root, true)
}

fn resolve_track_id_inner(
    explicit_id: Option<String>,
    anchor: &Path,
    write_mode: bool,
) -> Result<String, String> {
    use usecase::track_resolution::{ActiveTrackResolveInteractor, ActiveTrackResolveService as _};

    if !write_mode {
        if let Some(id) = explicit_id {
            return Ok(id);
        }
    }

    // Write mode with explicit ID: validate format first (before any git I/O),
    // then verify the ID matches the current branch.  Git discovery failure is
    // treated as a hard error so that callers outside a git repository cannot
    // bypass the branch guard by supplying an arbitrary explicit ID.
    if write_mode {
        if let Some(ref id) = explicit_id {
            validate_track_id_str(id)?;
            let repo =
                infrastructure::git_cli::SystemGitRepo::discover_from(anchor).map_err(|e| {
                    format!(
                        "cannot discover git repository from {}: {e} \
                         (write operations require a git repository)",
                        anchor.display()
                    )
                })?;
            let interactor = ActiveTrackResolveInteractor::new(Arc::new(repo));
            return interactor.resolve_for_write(explicit_id).map_err(|e| e.to_string());
        }
    }

    let repo = infrastructure::git_cli::SystemGitRepo::discover_from(anchor)
        .map_err(|e| format!("cannot discover git repository from {}: {e}", anchor.display()))?;
    let interactor = ActiveTrackResolveInteractor::new(Arc::new(repo));
    if write_mode {
        interactor.resolve_for_write(explicit_id).map_err(|e| e.to_string())
    } else {
        interactor.resolve_for_read(None).map_err(|e| e.to_string())
    }
}

// ---------------------------------------------------------------------------
// Branch reader helper (for TaskOperationInteractor)
// ---------------------------------------------------------------------------

use usecase::track_resolution::{BranchReadError, BranchReaderPort};

struct LazyBranchReader {
    project_root: PathBuf,
}

impl BranchReaderPort for LazyBranchReader {
    fn current_branch(&self) -> Result<Option<String>, BranchReadError> {
        let repo = infrastructure::git_cli::SystemGitRepo::discover_from(&self.project_root)
            .map_err(|e| {
                BranchReadError::ReadFailed(format!("failed to discover git repo: {e}"))
            })?;
        // Use explicit UFCS to disambiguate between BranchReaderPort and GitRepository.
        <infrastructure::git_cli::SystemGitRepo as BranchReaderPort>::current_branch(&repo)
    }
}

fn build_branch_reader(project_root: &Path) -> Option<Arc<dyn BranchReaderPort>> {
    Some(Arc::new(LazyBranchReader { project_root: project_root.to_path_buf() }))
}

// ---------------------------------------------------------------------------
// View sync helper
// ---------------------------------------------------------------------------

fn sync_views_to_stdout(project_root: &Path, track_id: &str) -> Vec<String> {
    match infrastructure::track::render::sync_rendered_views(project_root, Some(track_id)) {
        Ok(changed) => changed
            .iter()
            .map(|path| match path.strip_prefix(project_root) {
                Ok(rel) => format!("[OK] Rendered: {}", rel.display()),
                Err(_) => format!("[OK] Rendered: {}", path.display()),
            })
            .collect(),
        Err(err) => {
            vec![format!("warning: operation persisted but sync-views failed: {err}")]
        }
    }
}

// ---------------------------------------------------------------------------
// CliApp impl
// ---------------------------------------------------------------------------

impl CliApp {
    /// Transition a task to a new status.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_transition(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
        task_id: String,
        target_status: String,
        commit_hash: Option<String>,
    ) -> Result<CommandOutcome, String> {
        use infrastructure::track::fs_store::FsTrackStore;
        use usecase::task_ops::TaskOperationService as _;

        let effective_track_id =
            resolve_track_id_for_write(track_id, &items_dir).map_err(|e| e.to_string())?;
        validate_track_id_str(&effective_track_id)?;

        let repo_dir = items_dir.clone();
        let project_root = resolve_project_root(&repo_dir)?;

        let store = Arc::new(FsTrackStore::new(items_dir.clone()));
        let branch_reader = build_branch_reader(&project_root);
        let service =
            usecase::task_ops::TaskOperationInteractor::new(Arc::clone(&store), branch_reader);

        let cmd = usecase::task_ops::TaskTransitionCommand {
            items_dir,
            track_id: effective_track_id.clone(),
            task_id: task_id.clone(),
            target_status: target_status.clone(),
            commit_hash,
        };
        let output =
            service.transition_task(cmd).map_err(|err| format!("transition failed: {err}"))?;

        let mut lines = vec![format!(
            "[OK] {}: transitioned to {} (track status: {})",
            task_id, target_status, output.derived_status,
        )];
        lines.extend(sync_views_to_stdout(&project_root, &output.track_id));

        Ok(CommandOutcome::success(Some(lines.join("\n"))))
    }

    /// Create a new track branch from main.
    ///
    /// # Errors
    /// Returns `Err` when git discovery or branch creation fails.
    pub fn track_branch_create(
        &self,
        items_dir: PathBuf,
        track_id: String,
    ) -> Result<CommandOutcome, String> {
        use infrastructure::git_cli::GitRepository;

        validate_track_id_str(&track_id)?;
        let branch_name = format!("track/{track_id}");
        resolve_project_root(&items_dir)?;

        let repo = infrastructure::git_cli::SystemGitRepo::discover()
            .map_err(|e| format!("failed to discover git repository: {e}"))?;

        // branch create: must be on main, branch must not exist
        let current = GitRepository::current_branch(&repo).map_err(|e| e.to_string())?;
        if current.as_deref() != Some("main") {
            return Err(format!(
                "branch create must start from 'main'; current branch is {}",
                current.as_deref().unwrap_or("<detached>")
            ));
        }

        let exists_output = repo
            .output(&["rev-parse", "--verify", "--quiet", &branch_name])
            .map_err(|e| e.to_string())?;
        if exists_output.status.success() {
            return Err(format!("branch '{branch_name}' already exists"));
        }

        let code =
            repo.status(&["switch", "-c", &branch_name, "main"]).map_err(|e| e.to_string())?;
        if code == 0 {
            Ok(CommandOutcome::success(Some(format!(
                "[OK] Created and switched to branch: {branch_name}"
            ))))
        } else {
            Err(format!("git switch -c {branch_name} main failed"))
        }
    }

    /// Switch to an existing track branch.
    ///
    /// # Errors
    /// Returns `Err` when git discovery or branch switch fails.
    pub fn track_branch_switch(
        &self,
        items_dir: PathBuf,
        track_id: String,
    ) -> Result<CommandOutcome, String> {
        use infrastructure::git_cli::GitRepository;

        validate_track_id_str(&track_id)?;
        let branch_name = format!("track/{track_id}");
        resolve_project_root(&items_dir)?;

        let repo = infrastructure::git_cli::SystemGitRepo::discover()
            .map_err(|e| format!("failed to discover git repository: {e}"))?;

        // Verify branch exists
        let exists_output = repo
            .output(&["rev-parse", "--verify", "--quiet", &branch_name])
            .map_err(|e| e.to_string())?;
        if !exists_output.status.success() {
            return Err(format!("branch '{branch_name}' does not exist"));
        }

        let code = repo.status(&["switch", &branch_name]).map_err(|e| e.to_string())?;
        if code == 0 {
            Ok(CommandOutcome::success(Some(format!("[OK] Switched to branch: {branch_name}"))))
        } else {
            Err(format!("git switch {branch_name} failed"))
        }
    }

    /// Resolve the current track phase, next command, and blocker.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_resolve(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
    ) -> Result<CommandOutcome, String> {
        use infrastructure::track::fs_store::FsTrackStore;
        use usecase::track_phase::TrackPhaseService as _;

        resolve_project_root(&items_dir)?;
        let effective_track_id =
            resolve_track_id(track_id, &items_dir).map_err(|e| format!("resolve failed: {e}"))?;
        validate_track_id_str(&effective_track_id)
            .map_err(|e| format!("resolve failed: invalid track id: {e}"))?;

        let store = Arc::new(FsTrackStore::new(items_dir.clone()));
        let service = usecase::track_phase::TrackPhaseInteractor::new(Arc::clone(&store));
        let info = service
            .resolve(effective_track_id, items_dir)
            .map_err(|e| format!("resolve failed: {e}"))?;

        let mut lines = vec![
            format!("Current phase: {}", info.phase),
            format!("Reason: {}", info.reason),
            format!("Recommended next command: {}", info.next_command),
        ];
        if let Some(blocker) = &info.blocker {
            lines.push(format!("Blocker: {blocker}"));
        }

        Ok(CommandOutcome::success(Some(lines.join("\n"))))
    }

    /// Validate metadata.json files under the repository.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_views_validate(&self, project_root: PathBuf) -> Result<CommandOutcome, String> {
        infrastructure::track::render::validate_track_snapshots(&project_root)
            .map_err(|e| format!("track metadata validation failed: {e}"))?;
        Ok(CommandOutcome::success(Some("[OK] Track metadata is valid".to_owned())))
    }

    /// Render plan.md and registry.md from metadata.json.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_views_sync(
        &self,
        project_root: PathBuf,
        track_id: Option<String>,
    ) -> Result<CommandOutcome, String> {
        use usecase::track_resolution::{
            ActiveTrackResolveInteractor, ActiveTrackResolveService as _,
        };

        let resolved_track_id = match track_id {
            Some(id) => {
                // WRITE guard: verify explicit id matches the current branch
                Some(resolve_track_id_inner(Some(id), &project_root, true)?)
            }
            None => {
                // Auto-detect from branch — fall back to None for registry-only mode
                infrastructure::git_cli::SystemGitRepo::discover_from(&project_root).ok().and_then(
                    |repo| {
                        let interactor = ActiveTrackResolveInteractor::new(Arc::new(repo));
                        interactor.resolve_active_track().ok()
                    },
                )
            }
        };

        let changed = infrastructure::track::render::sync_rendered_views(
            &project_root,
            resolved_track_id.as_deref(),
        )
        .map_err(|e| format!("sync-views failed: {e}"))?;

        let lines = if changed.is_empty() {
            vec!["[OK] All views already up to date".to_owned()]
        } else {
            changed
                .iter()
                .map(|path| match path.strip_prefix(&project_root) {
                    Ok(rel) => format!("[OK] Rendered: {}", rel.display()),
                    Err(_) => format!("[OK] Rendered: {}", path.display()),
                })
                .collect()
        };

        Ok(CommandOutcome::success(Some(lines.join("\n"))))
    }

    /// Add a new task to a track.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_add_task(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
        description: String,
        section: Option<String>,
        after: Option<String>,
    ) -> Result<CommandOutcome, String> {
        use infrastructure::track::fs_store::FsTrackStore;
        use usecase::task_ops::TaskOperationService as _;

        let effective_track_id =
            resolve_track_id_for_write(track_id, &items_dir).map_err(|e| e.to_string())?;
        validate_track_id_str(&effective_track_id)?;

        let repo_dir = items_dir.clone();
        let project_root = resolve_project_root(&repo_dir)?;

        let store = Arc::new(FsTrackStore::new(items_dir.clone()));
        let branch_reader = build_branch_reader(&project_root);
        let service =
            usecase::task_ops::TaskOperationInteractor::new(Arc::clone(&store), branch_reader);

        let after_task_id = match after {
            Some(ref a)
                if a.strip_prefix('T').is_some_and(|digits| {
                    !digits.is_empty()
                        && digits.chars().all(|ch| ch.is_ascii_digit())
                        && digits.parse::<u64>().is_ok()
                }) =>
            {
                after
            }
            Some(ref a) => {
                return Err(format!("invalid --after value {a:?}: expected T<digits> (e.g. T001)"));
            }
            None => None,
        };

        let cmd = usecase::task_ops::AddTaskCommand {
            items_dir,
            track_id: effective_track_id.clone(),
            description: description.clone(),
            section,
            after_task_id,
        };
        let output = service.add_task(cmd).map_err(|e| format!("add-task failed: {e}"))?;

        let new_task_id = output.task_id.as_deref().unwrap_or("?");
        let mut lines = vec![format!(
            "[OK] Added task {new_task_id}: {description} (track status: {})",
            output.derived_status
        )];
        lines.extend(sync_views_to_stdout(&project_root, &output.track_id));

        Ok(CommandOutcome::success(Some(lines.join("\n"))))
    }

    /// Set a status override on a track (blocked/cancelled).
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_set_override(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
        status: String,
        reason: String,
    ) -> Result<CommandOutcome, String> {
        use infrastructure::track::fs_store::FsTrackStore;
        use usecase::task_ops::TaskOperationService as _;

        let effective_track_id =
            resolve_track_id_for_write(track_id, &items_dir).map_err(|e| e.to_string())?;
        validate_track_id_str(&effective_track_id)?;

        let repo_dir = items_dir.clone();
        let project_root = resolve_project_root(&repo_dir)?;

        let store = Arc::new(FsTrackStore::new(items_dir.clone()));
        let branch_reader = build_branch_reader(&project_root);
        let service =
            usecase::task_ops::TaskOperationInteractor::new(Arc::clone(&store), branch_reader);

        let cmd = usecase::task_ops::SetOverrideCommand {
            items_dir,
            track_id: effective_track_id.clone(),
            status: status.clone(),
            reason,
        };
        let output = service.set_override(cmd).map_err(|e| format!("set-override failed: {e}"))?;

        let mut lines = vec![format!(
            "[OK] Override set to '{}' (track status: {})",
            status, output.derived_status
        )];
        lines.extend(sync_views_to_stdout(&project_root, &output.track_id));

        Ok(CommandOutcome::success(Some(lines.join("\n"))))
    }

    /// Clear a status override on a track.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_clear_override(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
    ) -> Result<CommandOutcome, String> {
        use infrastructure::track::fs_store::FsTrackStore;
        use usecase::task_ops::TaskOperationService as _;

        let effective_track_id =
            resolve_track_id_for_write(track_id, &items_dir).map_err(|e| e.to_string())?;
        validate_track_id_str(&effective_track_id)?;

        let repo_dir = items_dir.clone();
        let project_root = resolve_project_root(&repo_dir)?;

        let store = Arc::new(FsTrackStore::new(items_dir.clone()));
        let branch_reader = build_branch_reader(&project_root);
        let service =
            usecase::task_ops::TaskOperationInteractor::new(Arc::clone(&store), branch_reader);

        let cmd = usecase::task_ops::ClearOverrideCommand {
            items_dir,
            track_id: effective_track_id.clone(),
        };
        let output =
            service.clear_override(cmd).map_err(|e| format!("clear-override failed: {e}"))?;

        let mut lines =
            vec![format!("[OK] Override cleared (track status: {})", output.derived_status)];
        lines.extend(sync_views_to_stdout(&project_root, &output.track_id));

        Ok(CommandOutcome::success(Some(lines.join("\n"))))
    }

    /// Show the next open task for a track (JSON output).
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_next_task(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
    ) -> Result<CommandOutcome, String> {
        use infrastructure::track::fs_store::FsTrackStore;
        use usecase::task_ops::TaskQueryService as _;

        let effective_track_id =
            resolve_track_id(track_id, &items_dir).map_err(|e| e.to_string())?;

        let store = Arc::new(FsTrackStore::new(items_dir.clone()));
        let service = usecase::task_ops::TaskQueryInteractor::new(Arc::clone(&store));

        let next = service
            .next_task(effective_track_id.clone(), items_dir.clone())
            .map_err(|e| format!("next-task failed: {e}"))?;

        let payload = match next {
            Some(task) => {
                let counts = service
                    .task_counts(effective_track_id, items_dir)
                    .map_err(|e| format!("next-task failed (counts): {e}"))?;
                let task_status = if counts.in_progress > 0 { "in_progress" } else { "todo" };
                serde_json::json!({
                    "task_id": task.task_id,
                    "description": task.description,
                    "status": task_status,
                })
            }
            None => serde_json::json!({
                "task_id": null,
                "description": null,
                "status": null,
            }),
        };

        Ok(CommandOutcome::success(Some(payload.to_string())))
    }

    /// Show task status counts for a track (JSON output).
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_task_counts(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
    ) -> Result<CommandOutcome, String> {
        use infrastructure::track::fs_store::FsTrackStore;
        use usecase::task_ops::TaskQueryService as _;

        let effective_track_id =
            resolve_track_id(track_id, &items_dir).map_err(|e| e.to_string())?;

        let store = Arc::new(FsTrackStore::new(items_dir.clone()));
        let service = usecase::task_ops::TaskQueryInteractor::new(Arc::clone(&store));

        let counts = service
            .task_counts(effective_track_id, items_dir)
            .map_err(|e| format!("task-counts failed: {e}"))?;

        let total = counts.todo + counts.in_progress + counts.done + counts.skipped;
        let json = format!(
            r#"{{"total":{total},"todo":{},"in_progress":{},"done":{},"skipped":{}}}"#,
            counts.todo, counts.in_progress, counts.done, counts.skipped
        );

        Ok(CommandOutcome::success(Some(json)))
    }

    /// Evaluate spec.md source tags and store results in metadata.json spec_signals.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_signals(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
    ) -> Result<CommandOutcome, String> {
        use infrastructure::spec::codec as spec_codec;
        use infrastructure::track::atomic_write::atomic_write_file;

        let effective_track_id =
            resolve_track_id(track_id, &items_dir).map_err(|e| e.to_string())?;
        validate_track_id_str(&effective_track_id)?;

        let track_dir = items_dir.join(&effective_track_id);
        let spec_json_path = track_dir.join("spec.json");

        if spec_json_path.is_file() {
            let json_content = std::fs::read_to_string(&spec_json_path)
                .map_err(|e| format!("cannot read {}: {e}", spec_json_path.display()))?;
            let mut doc = spec_codec::decode(&json_content)
                .map_err(|e| format!("spec.json decode error: {e}"))?;
            let counts = doc.evaluate_signals();
            doc.set_signals(counts);
            let encoded =
                spec_codec::encode(&doc).map_err(|e| format!("spec.json encode error: {e}"))?;
            atomic_write_file(&spec_json_path, format!("{encoded}\n").as_bytes())
                .map_err(|e| format!("cannot write {}: {e}", spec_json_path.display()))?;

            let rendered_spec = infrastructure::spec::render::render_spec(&doc);
            let spec_md_path = track_dir.join("spec.md");
            atomic_write_file(&spec_md_path, rendered_spec.as_bytes())
                .map_err(|e| format!("cannot write {}: {e}", spec_md_path.display()))?;

            let total = counts.total();
            let msg = format!(
                "[OK] Signals (spec.json): blue={} yellow={} red={} (total={total})",
                counts.blue(),
                counts.yellow(),
                counts.red()
            );
            Ok(CommandOutcome::success(Some(msg)))
        } else {
            // Legacy path: evaluate from spec.md, store in metadata.json
            ops::execute_signals_legacy(&track_dir)
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::path::Path;

    use super::{resolve_project_root, resolve_track_id_for_write};

    /// `resolve_project_root` strips the trailing `track/items` from a relative path.
    ///
    /// For `"track/items"` the parent of `track` is empty, so the function returns
    /// `"."` (the current-working-directory anchor).  This is the key property used
    /// by `resolve_track_id_from_branch` and `resolve_track_id_or_branch_write` in
    /// `review_v2/mod.rs`: they call `resolve_project_root` first and then pass the
    /// result to `SystemGitRepo::discover_from`, which means a relative items_dir
    /// always discovers from `"."` (the CWD) rather than from `"track/items"` which
    /// may not exist as a filesystem path when the process is inside a subdirectory.
    #[test]
    fn resolve_project_root_returns_dot_for_relative_items_dir() {
        let root = resolve_project_root(Path::new("track/items")).unwrap();
        assert_eq!(root, std::path::Path::new("."));
    }

    /// `resolve_project_root` returns the absolute parent when given an absolute path.
    #[test]
    fn resolve_project_root_strips_track_items_from_absolute_path() {
        let root = resolve_project_root(Path::new("/some/project/track/items")).unwrap();
        assert_eq!(root, std::path::Path::new("/some/project"));
    }

    /// `resolve_project_root` returns an error when the path does not end in `track/items`.
    #[test]
    fn resolve_project_root_rejects_non_canonical_path() {
        let result = resolve_project_root(Path::new("wrong/path"));
        assert!(result.is_err());
        let msg = result.unwrap_err();
        assert!(msg.contains("track/items"), "error should mention 'track/items': {msg}");
    }

    /// When git discovery fails (anchor path does not exist → `current_dir` fails)
    /// AND an explicit track id is supplied, `resolve_track_id_for_write` must
    /// return `Err` rather than the bare id.  This is the fail-closed branch-guard
    /// contract: WRITE operations may not proceed without branch proof.
    #[test]
    fn test_resolve_track_id_for_write_with_git_failure_and_explicit_id_returns_error() {
        // Use a path that satisfies resolve_project_root's structural check
        // (items_dir must end in "track/items") but points to a directory that
        // does not exist, so git discovery returns an error.
        let items_dir = Path::new("/tmp/sotp-test-no-git-repo/track/items");
        let result = resolve_track_id_for_write(Some("my-track-2026".to_owned()), items_dir);
        assert!(
            result.is_err(),
            "expected Err when git discovery fails with explicit track id, got Ok({:?})",
            result.ok()
        );
        let msg = result.unwrap_err();
        assert!(
            msg.contains("cannot discover git repository")
                || msg.contains("write operations require a git repository")
                || msg.contains("failed to run git")
                || msg.contains("No such file or directory")
                || msg.contains("rev-parse"),
            "expected error message to mention git failure, got: {msg:?}"
        );
    }
}
