//! `track` command family — core composition-root impl methods.
mod branch_strategy;
pub mod composition_root;
mod ops;
mod resolution;
pub(crate) mod service_impl;
mod set_commit_hash;
mod shim;
mod tddd;
use crate::CommandOutcome;
use crate::error::CompositionError;
use crate::track::composition_root::TrackCompositionRoot;
use std::path::{Path, PathBuf};
use std::sync::Arc;
/// Validates a track ID string by delegating to the canonical domain rule.
///
/// This is the single slug validator for the `cli_composition` crate; all internal
/// callers route through here so the rule lives in exactly one place (`domain::TrackId`).
pub(crate) fn validate_track_id_str(value: &str) -> Result<(), CompositionError> {
    domain::TrackId::try_new(value)
        .map(|_| ())
        .map_err(|e| CompositionError::WiringFailed(e.to_string()))
}
/// Resolves `<project-root>/track/items` → `<project-root>`.
pub(crate) fn resolve_project_root(items_dir: &Path) -> Result<PathBuf, CompositionError> {
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
        _ => Err(CompositionError::WiringFailed(format!(
            "--items-dir must point to '<project-root>/track/items'; got {}",
            items_dir.display()
        ))),
    }
}
/// Resolve track ID for READ (explicit overrides discovery).
pub(crate) fn resolve_track_id(
    explicit_id: Option<String>,
    items_dir: &Path,
) -> Result<String, CompositionError> {
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
) -> Result<String, CompositionError> {
    resolve_track_id_inner(explicit_id, workspace_root, false)
}
/// Resolve track ID for WRITE (branch always read, explicit must match).
fn resolve_track_id_for_write(
    explicit_id: Option<String>,
    items_dir: &Path,
) -> Result<String, CompositionError> {
    let project_root = resolve_project_root(items_dir)?;
    resolve_track_id_inner(explicit_id, &project_root, true)
}
fn resolve_track_id_inner(
    explicit_id: Option<String>,
    anchor: &Path,
    write_mode: bool,
) -> Result<String, CompositionError> {
    use usecase::track_resolution::{ActiveTrackResolveInteractor, ActiveTrackResolveService as _};
    if !write_mode {
        if let Some(id) = explicit_id {
            return Ok(id);
        }
    }
    // Explicit write IDs still need branch proof; fail closed if git discovery fails.
    if write_mode {
        if let Some(ref id) = explicit_id {
            validate_track_id_str(id)?;
            let repo =
                infrastructure::git_cli::SystemGitRepo::discover_from(anchor).map_err(|e| {
                    CompositionError::AdapterInit(format!(
                        "cannot discover git repository from {}: {e} \
                         (write operations require a git repository)",
                        anchor.display()
                    ))
                })?;
            let interactor = ActiveTrackResolveInteractor::new(Arc::new(repo));
            return interactor
                .resolve_for_write(explicit_id)
                .map_err(|e| CompositionError::AdapterInit(e.to_string()));
        }
    }
    let repo = infrastructure::git_cli::SystemGitRepo::discover_from(anchor).map_err(|e| {
        CompositionError::AdapterInit(format!(
            "cannot discover git repository from {}: {e}",
            anchor.display()
        ))
    })?;
    let interactor = ActiveTrackResolveInteractor::new(Arc::new(repo));
    if write_mode {
        interactor
            .resolve_for_write(explicit_id)
            .map_err(|e| CompositionError::AdapterInit(e.to_string()))
    } else {
        interactor.resolve_for_read(None).map_err(|e| CompositionError::AdapterInit(e.to_string()))
    }
}
fn build_branch_reader(
    project_root: &Path,
) -> Option<Arc<dyn usecase::track_resolution::BranchReaderPort>> {
    use infrastructure::git_cli::SystemGitRepo;
    use usecase::track_resolution::BranchReaderPort;
    match SystemGitRepo::discover_from(project_root) {
        Ok(repo) => Some(Arc::new(repo) as Arc<dyn BranchReaderPort>),
        Err(_) => None,
    }
}
/// Wires the task-operation interactor together with the admission
/// collaborators every transition is judged through and the verifier every
/// recorded commit hash is checked by.
fn build_task_operation_interactor(
    store: Arc<infrastructure::track::fs_store::FsTrackStore>,
    branch_reader: Option<Arc<dyn usecase::track_resolution::BranchReaderPort>>,
) -> usecase::task_ops::TaskOperationInteractor<infrastructure::track::fs_store::FsTrackStore> {
    usecase::task_ops::TaskOperationInteractor::new(
        store,
        branch_reader,
        Arc::new(infrastructure::batch_plan_reader::FsBatchPlanReader::new()),
        Arc::new(infrastructure::scope_diff_measure::GitScopeDiffMeasurer::new()),
        Arc::new(infrastructure::review_scope_config_reader::FsReviewScopeConfigReader::new()),
        Arc::new(infrastructure::commit_record_verifier::GitCommitRecordVerifier::new()),
    )
}
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
// The archive rollback helpers (`repo_relative_arg` / `run_git_mv` /
// `rollback_archive_contents_after_logs_error` / `describe_archive_rollback`)
// were removed as part of the T006 cutover — the archive orchestration is now
// entirely inside `usecase::git_workflow::TrackGitInteractor::archive_track`,
// which composes `GitPrimitivePort::move_path` and `TrackArchiveFsPort` fs
// primitives without needing composition-root-side rollback plumbing.
impl TrackCompositionRoot {
    /// Initialize a new track by writing `metadata.json`.
    ///
    /// Creates `track/items/<track_id>/metadata.json` with identity-only content
    /// (no tasks, no status override) and then syncs rendered views.
    ///
    /// # Errors
    /// Returns `Err` when track ID validation, directory creation, or metadata
    /// persistence fails.
    pub fn track_init(
        &self,
        items_dir: PathBuf,
        track_id: String,
        description: String,
    ) -> Result<CommandOutcome, CompositionError> {
        use domain::TrackWriter as _;
        use infrastructure::track::fs_store::FsTrackStore;

        validate_track_id_str(&track_id)?;

        let project_root = resolve_project_root(&items_dir)?;

        let id = domain::TrackId::try_new(&track_id)
            .map_err(|e| CompositionError::WiringFailed(format!("invalid track ID: {e}")))?;
        let snap = branch_strategy::resolve_branch_strategy_snapshot(&project_root)?;
        let track = domain::TrackMetadata::new(id, description, None, snap)
            .map_err(|e| CompositionError::WiringFailed(format!("invalid track metadata: {e}")))?;

        let store = FsTrackStore::new(items_dir);
        store.save(&track).map_err(|e| CompositionError::Usecase(format!("init failed: {e}")))?;

        infrastructure::track::render::sync_rendered_views(&project_root, Some(&track_id))
            .map_err(|e| CompositionError::Usecase(format!("sync-views failed: {e}")))?;

        Ok(CommandOutcome::success(None))
    }

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
    ) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::track::fs_store::FsTrackStore;
        use usecase::task_ops::TaskOperationService as _;
        let effective_track_id = resolve_track_id_for_write(track_id, &items_dir)?;
        validate_track_id_str(&effective_track_id)?;
        let repo_dir = items_dir.clone();
        let project_root = resolve_project_root(&repo_dir)?;
        let store = Arc::new(FsTrackStore::new(items_dir.clone()));
        let branch_reader = build_branch_reader(&project_root);
        let service = build_task_operation_interactor(Arc::clone(&store), branch_reader);
        let cmd = usecase::task_ops::TaskTransitionCommand {
            items_dir,
            track_id: effective_track_id.clone(),
            task_id: task_id.clone(),
            target_status: target_status.clone(),
            commit_hash,
        };
        let outcome = service
            .transition_task(cmd)
            .map_err(|err| CompositionError::Usecase(format!("transition failed: {err}")))?;
        let output = match outcome {
            usecase::task_ops::TaskTransitionOutcome::Transitioned(output) => output,
            // A refused start of work is a verdict, not a failure of the
            // command: the task stays where it was and the reason is reported.
            usecase::task_ops::TaskTransitionOutcome::Rejected(rejection) => {
                return Ok(CommandOutcome::failure(Some(format!(
                    "[BLOCKED] {task_id}: {rejection}"
                ))));
            }
        };
        let mut lines = vec![format!(
            "[OK] {}: transitioned to {} (track status: {})",
            task_id, target_status, output.derived_status,
        )];
        lines.extend(sync_views_to_stdout(&project_root, &output.track_id));
        Ok(CommandOutcome::success(Some(lines.join("\n"))))
    }
    /// Switch to an existing track branch.
    /// # Errors
    /// Returns `Err` when git discovery or branch switch fails.
    pub fn track_branch_switch(
        &self,
        items_dir: PathBuf,
        track_id: String,
    ) -> Result<CommandOutcome, CompositionError> {
        validate_track_id_str(&track_id)?;
        let project_root = resolve_project_root(&items_dir)?;
        let id = domain::TrackId::try_new(&track_id)
            .map_err(|e| CompositionError::WiringFailed(format!("invalid track ID: {e}")))?;
        branch_strategy::track_git_interactor()
            .switch_to_track_branch(&project_root, &id)
            .map(|msg| CommandOutcome::success(Some(msg)))
            .map_err(|e| CompositionError::Infrastructure(e.to_string()))
    }
    /// Resolve the current track phase, next command, and blocker.
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_resolve(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
    ) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::track::fs_store::FsTrackStore;
        use usecase::track_phase::TrackPhaseService as _;
        resolve_project_root(&items_dir)?;
        let effective_track_id = resolve_track_id(track_id, &items_dir)?;
        validate_track_id_str(&effective_track_id)?;
        let store = Arc::new(FsTrackStore::new(items_dir.clone()));
        let service = usecase::track_phase::TrackPhaseInteractor::new(Arc::clone(&store));
        let info = service
            .resolve(effective_track_id, items_dir)
            .map_err(|e| CompositionError::Usecase(format!("resolve failed: {e}")))?;
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
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_views_validate(
        &self,
        project_root: PathBuf,
    ) -> Result<CommandOutcome, CompositionError> {
        infrastructure::track::render::validate_track_snapshots(&project_root).map_err(|e| {
            CompositionError::Infrastructure(format!("track metadata validation failed: {e}"))
        })?;
        Ok(CommandOutcome::success(Some("[OK] Track metadata is valid".to_owned())))
    }
    /// Render plan.md and registry.md from metadata.json.
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_views_sync(
        &self,
        project_root: PathBuf,
        track_id: Option<String>,
    ) -> Result<CommandOutcome, CompositionError> {
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
        .map_err(|e| CompositionError::Infrastructure(format!("sync-views failed: {e}")))?;
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
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_add_task(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
        description: String,
        section: Option<String>,
        after: Option<String>,
    ) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::track::fs_store::FsTrackStore;
        use usecase::task_ops::TaskOperationService as _;
        let effective_track_id = resolve_track_id_for_write(track_id, &items_dir)?;
        validate_track_id_str(&effective_track_id)?;
        let repo_dir = items_dir.clone();
        let project_root = resolve_project_root(&repo_dir)?;
        let store = Arc::new(FsTrackStore::new(items_dir.clone()));
        let branch_reader = build_branch_reader(&project_root);
        let service = build_task_operation_interactor(Arc::clone(&store), branch_reader);
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
                return Err(CompositionError::WiringFailed(format!(
                    "invalid --after value {a:?}: expected T<digits> (e.g. T001)"
                )));
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
        let output = service
            .add_task(cmd)
            .map_err(|e| CompositionError::Usecase(format!("add-task failed: {e}")))?;
        let new_task_id = output.task_id.as_deref().unwrap_or("?");
        let mut lines = vec![format!(
            "[OK] Added task {new_task_id}: {description} (track status: {})",
            output.derived_status
        )];
        lines.extend(sync_views_to_stdout(&project_root, &output.track_id));
        Ok(CommandOutcome::success(Some(lines.join("\n"))))
    }
    /// Set a status override on a track (blocked/cancelled).
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_set_override(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
        status: String,
        reason: String,
    ) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::track::fs_store::FsTrackStore;
        use usecase::task_ops::TaskOperationService as _;
        let effective_track_id = resolve_track_id_for_write(track_id, &items_dir)?;
        validate_track_id_str(&effective_track_id)?;
        let repo_dir = items_dir.clone();
        let project_root = resolve_project_root(&repo_dir)?;
        let store = Arc::new(FsTrackStore::new(items_dir.clone()));
        let branch_reader = build_branch_reader(&project_root);
        let service = build_task_operation_interactor(Arc::clone(&store), branch_reader);
        let cmd = usecase::task_ops::SetOverrideCommand {
            items_dir,
            track_id: effective_track_id.clone(),
            status: status.clone(),
            reason,
        };
        let output = service
            .set_override(cmd)
            .map_err(|e| CompositionError::Usecase(format!("set-override failed: {e}")))?;
        let mut lines = vec![format!(
            "[OK] Override set to '{}' (track status: {})",
            status, output.derived_status
        )];
        lines.extend(sync_views_to_stdout(&project_root, &output.track_id));
        Ok(CommandOutcome::success(Some(lines.join("\n"))))
    }
    /// Clear a status override on a track.
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_clear_override(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
    ) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::track::fs_store::FsTrackStore;
        use usecase::task_ops::TaskOperationService as _;
        let effective_track_id = resolve_track_id_for_write(track_id, &items_dir)?;
        validate_track_id_str(&effective_track_id)?;
        let repo_dir = items_dir.clone();
        let project_root = resolve_project_root(&repo_dir)?;
        let store = Arc::new(FsTrackStore::new(items_dir.clone()));
        let branch_reader = build_branch_reader(&project_root);
        let service = build_task_operation_interactor(Arc::clone(&store), branch_reader);
        let cmd = usecase::task_ops::ClearOverrideCommand {
            items_dir,
            track_id: effective_track_id.clone(),
        };
        let output = service
            .clear_override(cmd)
            .map_err(|e| CompositionError::Usecase(format!("clear-override failed: {e}")))?;
        let mut lines =
            vec![format!("[OK] Override cleared (track status: {})", output.derived_status)];
        lines.extend(sync_views_to_stdout(&project_root, &output.track_id));
        Ok(CommandOutcome::success(Some(lines.join("\n"))))
    }
    /// Show the next open task for a track (JSON output).
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_next_task(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
    ) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::track::fs_store::FsTrackStore;
        use usecase::task_ops::TaskQueryService as _;
        let effective_track_id = resolve_track_id(track_id, &items_dir)?;
        let store = Arc::new(FsTrackStore::new(items_dir.clone()));
        let service = usecase::task_ops::TaskQueryInteractor::new(Arc::clone(&store));
        let next = service
            .next_task(effective_track_id.clone(), items_dir.clone())
            .map_err(|e| CompositionError::Usecase(format!("next-task failed: {e}")))?;
        let payload = match next {
            Some(task) => {
                let counts = service.task_counts(effective_track_id, items_dir).map_err(|e| {
                    CompositionError::Usecase(format!("next-task failed (counts): {e}"))
                })?;
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
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_task_counts(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
    ) -> Result<CommandOutcome, CompositionError> {
        let effective_track_id = resolve_track_id(track_id, &items_dir)?;
        self.track_task_counts_resolved(items_dir, effective_track_id)
    }
    /// Archive a completed track and preserve gitignored telemetry logs when present.
    /// # Errors
    /// Returns `Err` when validation, `git mv`, or the optional `logs/` rename fails.
    pub fn track_archive(
        &self,
        items_dir: PathBuf,
        track_id: String,
    ) -> Result<CommandOutcome, CompositionError> {
        validate_track_id_str(&track_id)?;
        // Anchor discovery to `items_dir` (not the process CWD) so that
        // absolute `--items-dir <path>` invocations from tests or from a
        // nested working directory resolve the correct repo root. When
        // `items_dir` is relative (the CLI default `track/items`),
        // `resolve_project_root` returns `.`, which then `discover_from`
        // rehydrates from the current worktree.
        let project_root_hint = resolve_project_root(&items_dir)?;
        let repo = infrastructure::git_cli::SystemGitRepo::discover_from(&project_root_hint)
            .map_err(|e| {
                CompositionError::AdapterInit(format!("failed to discover git repository: {e}"))
            })?;
        let project_root = repo.root().to_path_buf();
        let id = domain::TrackId::try_new(&track_id)
            .map_err(|e| CompositionError::WiringFailed(format!("invalid track ID: {e}")))?;
        branch_strategy::track_git_interactor()
            .archive_track(&project_root, &id)
            .map(|msg| CommandOutcome::success(Some(msg)))
            .map_err(|e| CompositionError::Infrastructure(e.to_string()))
    }
}
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{resolve_project_root, resolve_track_id_for_write};
    use crate::review_v2::process_guards::{CwdGuard, GitRunner};

    fn change_to(path: &Path) -> CwdGuard {
        let guard = CwdGuard::save_current();
        std::env::set_current_dir(path).unwrap();
        guard
    }

    fn init_git_repo(root: &Path) {
        GitRunner::at(root).assert_success(&["init", "-q"]);
        GitRunner::at(root).assert_success(&["config", "user.email", "test@test.com"]);
        GitRunner::at(root).assert_success(&["config", "user.name", "Test"]);
        GitRunner::at(root).assert_success(&["checkout", "-B", "main"]);
    }

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
        let msg = result.unwrap_err().to_string();
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
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("cannot discover git repository")
                || msg.contains("write operations require a git repository")
                || msg.contains("failed to run git")
                || msg.contains("No such file or directory")
                || msg.contains("rev-parse"),
            "expected error message to mention git failure, got: {msg:?}"
        );
    }

    #[test]
    fn test_track_init_missing_items_root_creates_metadata() {
        let root = tempfile::tempdir().unwrap();
        let items_dir = root.path().join("track").join("items");
        let config_dir = root.path().join(".harness").join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("branch-strategy.json"),
            r#"{
  "base_branch": "main",
  "merge_target": "main",
  "merge_method": "merge"
}"#,
        )
        .unwrap();
        std::fs::write(
            root.path().join("architecture-rules.json"),
            include_str!("../../../../architecture-rules.json"),
        )
        .unwrap();

        assert!(!items_dir.exists(), "fixture must start without track/items");

        let outcome = crate::track::TrackCompositionRoot::new()
            .track_init(items_dir.clone(), "new-track".to_owned(), "New Track".to_owned())
            .unwrap();

        assert_eq!(outcome.exit_code, 0);
        assert!(
            items_dir.join("new-track").join("metadata.json").is_file(),
            "track init must bootstrap track/items and write metadata"
        );
    }

    #[test]
    fn test_track_init_date_prefixed_ids_create_sorted_item_directories() {
        let root = tempfile::tempdir().unwrap();
        let items_dir = root.path().join("track").join("items");
        let config_dir = root.path().join(".harness").join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("branch-strategy.json"),
            r#"{
  "base_branch": "main",
  "merge_target": "main",
  "merge_method": "merge"
}"#,
        )
        .unwrap();
        std::fs::write(
            root.path().join("architecture-rules.json"),
            include_str!("../../../../architecture-rules.json"),
        )
        .unwrap();

        let composition = crate::track::TrackCompositionRoot::new();
        for (track_id, title) in [
            ("2026-07-01-earlier-track", "Earlier Track"),
            ("2026-07-31-later-track", "Later Track"),
        ] {
            let outcome = composition
                .track_init(items_dir.clone(), track_id.to_owned(), title.to_owned())
                .unwrap();
            assert_eq!(outcome.exit_code, 0);
            assert!(
                items_dir.join(track_id).join("metadata.json").is_file(),
                "date-prefixed track ID must remain the item-directory name"
            );
        }

        let mut listed_ids = std::fs::read_dir(&items_dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect::<Vec<_>>();
        listed_ids.sort_unstable();

        assert_eq!(
            listed_ids,
            ["2026-07-01-earlier-track", "2026-07-31-later-track"],
            "ascending item-directory listing must put the earlier date first"
        );
    }

    #[test]
    fn test_track_archive_from_subdir_moves_track_and_logs_under_repo_root() {
        let _guard = crate::test_support::process_env_lock().lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let track_id = "my-track-2026";
        let track_dir = root.join("track").join("items").join(track_id);
        let logs_dir = track_dir.join("logs");
        std::fs::create_dir_all(&logs_dir).unwrap();
        std::fs::write(root.join(".gitignore"), "track/items/*/logs/\n").unwrap();
        std::fs::write(track_dir.join("tracked.txt"), "archive fixture\n").unwrap();
        std::fs::write(logs_dir.join("telemetry.jsonl"), "{}\n").unwrap();

        init_git_repo(root);
        GitRunner::at(root).assert_success(&[
            "add",
            ".gitignore",
            "track/items/my-track-2026/tracked.txt",
        ]);
        GitRunner::at(root).assert_success(&["commit", "-m", "add track", "--no-gpg-sign"]);

        let subdir = root.join("nested").join("workdir");
        std::fs::create_dir_all(&subdir).unwrap();
        let _cwd = change_to(&subdir);

        let outcome = crate::track::TrackCompositionRoot::new()
            .track_archive(PathBuf::from("track/items"), track_id.to_owned())
            .unwrap();

        assert_eq!(outcome.exit_code, 0);
        let archived_dir = root.join("track").join("archive").join(track_id);
        assert!(archived_dir.join("tracked.txt").is_file());
        assert!(archived_dir.join("logs").join("telemetry.jsonl").is_file());
        assert!(!root.join("track").join("items").join(track_id).join("logs").exists());
    }

    #[test]
    fn test_track_archive_without_logs_from_subdir_succeeds_silently() {
        let _guard = crate::test_support::process_env_lock().lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let track_id = "no-logs-track-2026";
        let track_dir = root.join("track").join("items").join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("tracked.txt"), "archive fixture\n").unwrap();

        init_git_repo(root);
        GitRunner::at(root).assert_success(&["add", "track/items/no-logs-track-2026/tracked.txt"]);
        GitRunner::at(root).assert_success(&["commit", "-m", "add track", "--no-gpg-sign"]);

        let subdir = root.join("nested").join("workdir");
        std::fs::create_dir_all(&subdir).unwrap();
        let _cwd = change_to(&subdir);

        let outcome = crate::track::TrackCompositionRoot::new()
            .track_archive(PathBuf::from("track/items"), track_id.to_owned())
            .unwrap();

        assert_eq!(outcome.exit_code, 0);
        let archived_dir = root.join("track").join("archive").join(track_id);
        assert!(archived_dir.join("tracked.txt").is_file());
        assert!(!archived_dir.join("logs").exists());
    }

    #[test]
    fn test_track_archive_missing_track_returns_error() {
        let _guard = crate::test_support::process_env_lock().lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("track").join("items")).unwrap();
        init_git_repo(root);

        let subdir = root.join("nested").join("workdir");
        std::fs::create_dir_all(&subdir).unwrap();
        let _cwd = change_to(&subdir);

        let err = crate::track::TrackCompositionRoot::new()
            .track_archive(PathBuf::from("track/items"), "missing-track-2026".to_owned())
            .unwrap_err()
            .to_string();

        assert!(err.contains("track directory not found"), "unexpected error: {err}");
        assert!(!root.join("track").join("archive").exists());
    }

    #[test]
    fn test_track_archive_untracked_source_returns_git_mv_error() {
        let _guard = crate::test_support::process_env_lock().lock().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let track_id = "untracked-track-2026";
        let track_dir = root.join("track").join("items").join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("untracked.txt"), "archive fixture\n").unwrap();
        init_git_repo(root);

        let subdir = root.join("nested").join("workdir");
        std::fs::create_dir_all(&subdir).unwrap();
        let _cwd = change_to(&subdir);

        let err = crate::track::TrackCompositionRoot::new()
            .track_archive(PathBuf::from("track/items"), track_id.to_owned())
            .unwrap_err()
            .to_string();

        assert!(err.contains("git mv failed"), "unexpected error: {err}");
        assert!(track_dir.join("untracked.txt").is_file());
    }

    // The former `test_rollback_archive_contents_after_logs_error_restores_source_tree`
    // test targeted the private `rollback_archive_contents_after_logs_error`
    // helper, which was deleted as part of the T006 cutover.
    // `TrackGitInteractor::archive_track` now performs the fs-side rename
    // directly through the port; its unit coverage lives in
    // `libs/usecase/src/git_workflow.rs::tests` (T003 mock-port tests).
}
