//! `track` command family — primary adapter driver.
//!
//! `TrackDriver` holds the wired command-context services and exposes
//! `handle(input) -> CommandOutcome`.

use std::path::PathBuf;
use std::sync::Arc;

use usecase::base_merge::{BaseMergeCommand, BaseMergeService};
use usecase::fixpoint_resolve_driver::{
    FixpointResolveDriverInput, FixpointResolveDriverOutcome, FixpointResolveDriverService,
};
use usecase::track_lifecycle::track_add_task::{
    TrackAddTaskCommand, TrackAddTaskError, TrackAddTaskResult, TrackAddTaskService,
};
use usecase::track_lifecycle::track_archive::{
    TrackArchiveCommand, TrackArchiveError, TrackArchiveService,
};
use usecase::track_lifecycle::track_branch_create::{
    TrackBranchCreateCommand, TrackBranchCreateError, TrackBranchCreateService,
};
use usecase::track_lifecycle::track_branch_switch::{
    TrackBranchSwitchCommand, TrackBranchSwitchError, TrackBranchSwitchService,
};
use usecase::track_lifecycle::track_clear_override::TrackClearOverrideService;
use usecase::track_lifecycle::track_init::{TrackInitCommand, TrackInitError, TrackInitService};
use usecase::track_lifecycle::track_next_task::TrackNextTaskService;
use usecase::track_lifecycle::track_resolve::TrackResolveService;
use usecase::track_lifecycle::track_set_commit_hash::TrackSetCommitHashService;
use usecase::track_lifecycle::track_set_override::TrackSetOverrideService;
use usecase::track_lifecycle::track_switch_base::TrackSwitchBaseService;
use usecase::track_lifecycle::track_task_counts::TrackTaskCountsService;
use usecase::track_lifecycle::track_transition::TrackTransitionService;
use usecase::track_lifecycle::track_views_sync::TrackViewsSyncService;
use usecase::track_lifecycle::track_views_validate::TrackViewsValidateService;
use usecase::track_lifecycle::{
    RenderedViewPath, TrackDirectoryPath, TrackItemsDirectory, TrackLifecycleIdInput,
    TrackSelection, TrackViewSyncOutcome,
};

use crate::render::CommandOutcome;
use crate::track_base_merge::render_base_merge_result;
use crate::track_clear_override::render_track_clear_override_outcome;
use crate::track_next_task::render_track_next_task_outcome;
use crate::track_resolve::render_track_resolve_outcome;
use crate::track_set_commit_hash::render_track_set_commit_hash_outcome;
use crate::track_set_override::render_track_set_override_outcome;
use crate::track_switch_base::render_track_switch_base_outcome;
use crate::track_task_counts::render_track_task_counts_outcome;
use crate::track_transition::render_track_transition_outcome;
use crate::track_views_sync::render_track_views_sync_outcome;
use crate::track_views_validate::render_track_views_validate_outcome;

// ---------------------------------------------------------------------------
// Input type
// ---------------------------------------------------------------------------

/// Typed input for the `track` command family.
pub enum TrackInput {
    /// Initialize a new track (write metadata.json).
    Init {
        /// Items directory (`track/items`).
        items_dir: PathBuf,
        /// Track ID string (e.g. `T001`).
        track_id: String,
        /// Short description of the track.
        description: String,
    },
    /// Transition a task to a new status.
    Transition {
        /// Items directory (`track/items`).
        items_dir: PathBuf,
        /// Track ID string.
        track_id: Option<String>,
        /// Task ID (e.g. `T001`).
        task_id: String,
        /// Target status string (e.g. `done`, `in_progress`).
        target_status: String,
        /// Commit hash (required when target_status is `done`, optional otherwise).
        commit_hash: Option<String>,
    },
    /// Resolve the current track phase, next command, and optional blocker.
    Resolve {
        /// Items directory (`track/items`).
        items_dir: PathBuf,
        /// Track ID string (resolved from git branch if `None`).
        track_id: Option<String>,
    },
    /// Create a new track branch from the configured base branch.
    BranchCreate {
        /// Items directory (`track/items`).
        items_dir: PathBuf,
        /// Track ID string.
        track_id: String,
    },
    /// Switch to an existing track branch.
    BranchSwitch {
        /// Items directory (`track/items`).
        items_dir: PathBuf,
        /// Track ID string.
        track_id: String,
    },
    /// Validate metadata.json files under the repository.
    ViewsValidate {
        /// Project root directory.
        project_root: PathBuf,
    },
    /// Render plan.md and registry.md from metadata.json.
    ViewsSync {
        /// Project root directory.
        project_root: PathBuf,
        /// Track ID string (auto-detected from branch if `None`).
        track_id: Option<String>,
    },
    /// Add a new task to a track.
    AddTask {
        /// Items directory (`track/items`).
        items_dir: PathBuf,
        /// Track ID string (resolved from git branch if `None`).
        track_id: Option<String>,
        /// Short task description.
        description: String,
        /// Optional plan section to file the task under.
        section: Option<String>,
        /// Insert after this task ID (e.g. `T003`).
        after: Option<String>,
    },
    /// Set a status override (blocked/cancelled) on a track.
    SetOverride {
        /// Items directory (`track/items`).
        items_dir: PathBuf,
        /// Track ID string (resolved from git branch if `None`).
        track_id: Option<String>,
        /// Override status string.
        status: String,
        /// Human-readable reason for the override.
        reason: String,
    },
    /// Clear a status override on a track.
    ClearOverride {
        /// Items directory (`track/items`).
        items_dir: PathBuf,
        /// Track ID string (resolved from git branch if `None`).
        track_id: Option<String>,
    },
    /// Show the next open task for a track (JSON output).
    NextTask {
        /// Items directory (`track/items`).
        items_dir: PathBuf,
        /// Track ID string (resolved from git branch if `None`).
        track_id: Option<String>,
    },
    /// Show task status counts for a track (JSON output).
    TaskCounts {
        /// Items directory (`track/items`).
        items_dir: PathBuf,
        /// Track ID string (resolved from git branch if `None`).
        track_id: Option<String>,
    },
    /// Archive a completed track.
    Archive {
        /// Items directory (`track/items`).
        items_dir: PathBuf,
        /// Track ID string.
        track_id: String,
    },
    /// Detect the active track ID from the current git branch.
    DetectActive {
        /// Project root directory.
        project_root: PathBuf,
    },
    /// Resolve the next fixpoint step for the active track.
    FixpointResolve {
        /// Active track ID (directory name under `items_dir/<id>`).
        track_id: String,
        /// Current git branch label (e.g. `"track/my-feature-2026"`).
        current_branch: String,
        /// Path to the `track/items` directory.
        items_dir: PathBuf,
    },
    /// Switch to the configured base branch from the active track's
    /// `branch_strategy_snapshot`.
    SwitchBase {
        /// Project root directory.
        project_root: PathBuf,
    },
    /// Run the catalogue lint ruleset across every `tddd.enabled` layer of
    /// the active track and aggregate violations.
    CatalogueLintCheckActiveTrack {
        /// Track ID string (resolved from git branch if `None`).
        track_id: Option<String>,
        /// Workspace root directory (contains `architecture-rules.json` and
        /// `track/items/`).
        workspace_root: PathBuf,
        /// Optional override for the lint config file path (defaults to
        /// `.harness/catalogue-lint/config.json` under `workspace_root`).
        rules_file: Option<PathBuf>,
    },
}

/// Typed input for a guarded base-to-track merge.
pub struct BaseMergeInput {
    /// Workspace root containing the active track checkout.
    pub workspace_root: PathBuf,
}

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

/// Render a [`FixpointResolveDriverOutcome`] into a [`CommandOutcome`].
///
/// Reproduces the exact text contract previously produced by the
/// track fixpoint-resolve formatter:
/// - `RunDfp` → `"run-dfp"`
/// - `RunRfp { scopes }` → `"run-rfp scopes=<s1>,<s2>..."` (scopes already
///   sorted by the usecase layer)
/// - `RunRefVerify` → `"run-ref-verify"`
/// - `Commit` → `"commit"`
/// - `Failure { message }` → failure outcome carrying `message`
fn render_fixpoint_resolve_outcome(outcome: FixpointResolveDriverOutcome) -> CommandOutcome {
    match outcome {
        FixpointResolveDriverOutcome::RunDfp => CommandOutcome::success(Some("run-dfp".to_owned())),
        FixpointResolveDriverOutcome::RunRfp { scopes } => {
            CommandOutcome::success(Some(format!("run-rfp scopes={}", scopes.join(","))))
        }
        FixpointResolveDriverOutcome::RunRefVerify => {
            CommandOutcome::success(Some("run-ref-verify".to_owned()))
        }
        FixpointResolveDriverOutcome::Commit => CommandOutcome::success(Some("commit".to_owned())),
        FixpointResolveDriverOutcome::Failure { message } => CommandOutcome::failure(Some(message)),
    }
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// Primary adapter driver for the `track` command family.
///
/// Holds the wired command-context services and exposes `handle(input) -> CommandOutcome`.
pub struct TrackDriver {
    track_init_service: Arc<dyn TrackInitService>,
    track_transition_service: Arc<dyn TrackTransitionService>,
    track_archive_service: Arc<dyn TrackArchiveService>,
    track_branch_create_service: Arc<dyn TrackBranchCreateService>,
    track_branch_switch_service: Arc<dyn TrackBranchSwitchService>,
    fixpoint_resolve_service: Arc<dyn FixpointResolveDriverService>,
    base_merge_service: Arc<dyn BaseMergeService>,
    track_add_task_service: Arc<dyn TrackAddTaskService>,
    track_next_task_service: Arc<dyn TrackNextTaskService>,
    track_task_counts_service: Arc<dyn TrackTaskCountsService>,
    track_set_override_service: Arc<dyn TrackSetOverrideService>,
    track_clear_override_service: Arc<dyn TrackClearOverrideService>,
    track_set_commit_hash_service: Arc<dyn TrackSetCommitHashService>,
    track_switch_base_service: Arc<dyn TrackSwitchBaseService>,
    track_resolve_service: Arc<dyn TrackResolveService>,
    track_views_sync_service: Arc<dyn TrackViewsSyncService>,
    track_views_validate_service: Arc<dyn TrackViewsValidateService>,
}

impl TrackDriver {
    /// Create a new `TrackDriver` with the given services.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        track_init: Arc<dyn TrackInitService>,
        track_transition: Arc<dyn TrackTransitionService>,
        track_branch_switch: Arc<dyn TrackBranchSwitchService>,
        track_resolve: Arc<dyn TrackResolveService>,
        track_views_validate: Arc<dyn TrackViewsValidateService>,
        track_views_sync: Arc<dyn TrackViewsSyncService>,
        track_add_task: Arc<dyn TrackAddTaskService>,
        track_set_override: Arc<dyn TrackSetOverrideService>,
        track_clear_override: Arc<dyn TrackClearOverrideService>,
        track_next_task: Arc<dyn TrackNextTaskService>,
        track_task_counts: Arc<dyn TrackTaskCountsService>,
        track_archive: Arc<dyn TrackArchiveService>,
        track_branch_create: Arc<dyn TrackBranchCreateService>,
        track_switch_base: Arc<dyn TrackSwitchBaseService>,
        track_set_commit_hash: Arc<dyn TrackSetCommitHashService>,
        fixpoint_resolve_service: Arc<dyn FixpointResolveDriverService>,
        base_merge_service: Arc<dyn BaseMergeService>,
    ) -> Self {
        Self {
            track_init_service: track_init,
            track_transition_service: track_transition,
            track_archive_service: track_archive,
            track_branch_create_service: track_branch_create,
            track_branch_switch_service: track_branch_switch,
            fixpoint_resolve_service,
            base_merge_service,
            track_add_task_service: track_add_task,
            track_next_task_service: track_next_task,
            track_task_counts_service: track_task_counts,
            track_set_override_service: track_set_override,
            track_clear_override_service: track_clear_override,
            track_set_commit_hash_service: track_set_commit_hash,
            track_switch_base_service: track_switch_base,
            track_resolve_service: track_resolve,
            track_views_sync_service: track_views_sync,
            track_views_validate_service: track_views_validate,
        }
    }

    /// Handle a guarded base-to-track merge through the application service.
    #[must_use]
    pub fn handle_base_merge(&self, input: BaseMergeInput) -> CommandOutcome {
        render_base_merge_result(
            self.base_merge_service
                .execute(BaseMergeCommand { workspace_root: input.workspace_root }),
        )
    }

    /// Handle the commit-hash persistence command through the injected service.
    pub fn handle_set_commit_hash(
        &self,
        input: crate::adr_baseline::TrackIdInput,
    ) -> CommandOutcome {
        render_track_set_commit_hash_outcome(&*self.track_set_commit_hash_service, input)
    }

    /// Handle a track command.
    pub fn handle(&self, input: TrackInput) -> CommandOutcome {
        match input {
            TrackInput::Init { items_dir, track_id, description } => render_track_init_outcome(
                &*self.track_init_service,
                items_dir,
                track_id,
                description,
            ),
            TrackInput::Archive { items_dir, track_id } => {
                render_track_archive_outcome(&*self.track_archive_service, items_dir, track_id)
            }
            TrackInput::BranchCreate { items_dir, track_id } => render_track_branch_create_outcome(
                &*self.track_branch_create_service,
                items_dir,
                track_id,
            ),
            TrackInput::BranchSwitch { items_dir, track_id } => render_track_branch_switch_outcome(
                &*self.track_branch_switch_service,
                items_dir,
                track_id,
            ),
            TrackInput::Transition { items_dir, track_id, task_id, target_status, commit_hash } => {
                render_track_transition_outcome(
                    &*self.track_transition_service,
                    items_dir,
                    track_id,
                    task_id,
                    target_status,
                    commit_hash,
                )
            }
            TrackInput::Resolve { items_dir, track_id } => {
                render_track_resolve_outcome(&*self.track_resolve_service, items_dir, track_id)
            }
            TrackInput::ViewsValidate { project_root } => render_track_views_validate_outcome(
                &*self.track_views_validate_service,
                project_root,
            ),
            TrackInput::ViewsSync { project_root, track_id } => render_track_views_sync_outcome(
                &*self.track_views_sync_service,
                project_root,
                track_id,
            ),
            TrackInput::AddTask { items_dir, track_id, description, section, after } => {
                render_track_add_task_outcome(
                    &*self.track_add_task_service,
                    items_dir,
                    track_id,
                    description,
                    section,
                    after,
                )
            }
            TrackInput::SetOverride { items_dir, track_id, status, reason } => {
                render_track_set_override_outcome(
                    &*self.track_set_override_service,
                    items_dir,
                    track_id,
                    status,
                    reason,
                )
            }
            TrackInput::ClearOverride { items_dir, track_id } => {
                render_track_clear_override_outcome(
                    &*self.track_clear_override_service,
                    items_dir,
                    track_id,
                )
            }
            TrackInput::NextTask { items_dir, track_id } => {
                render_track_next_task_outcome(&*self.track_next_task_service, items_dir, track_id)
            }
            TrackInput::TaskCounts { items_dir, track_id } => render_track_task_counts_outcome(
                &*self.track_task_counts_service,
                items_dir,
                track_id,
            ),
            TrackInput::FixpointResolve { track_id, current_branch, items_dir } => {
                let outcome =
                    self.fixpoint_resolve_service.fixpoint_resolve(FixpointResolveDriverInput {
                        track_id,
                        current_branch,
                        items_dir,
                    });
                render_fixpoint_resolve_outcome(outcome)
            }
            TrackInput::SwitchBase { project_root } => {
                render_track_switch_base_outcome(&*self.track_switch_base_service, project_root)
            }
            TrackInput::DetectActive { project_root: _ } => CommandOutcome::failure(Some(
                "detect-active is served by TrackResolutionDriver".to_owned(),
            )),
            TrackInput::CatalogueLintCheckActiveTrack { .. } => CommandOutcome::failure(Some(
                "catalogue-lint check-active-track is served by TrackTdddDriver".to_owned(),
            )),
        }
    }
}

fn render_track_init_outcome(
    service: &dyn TrackInitService,
    items_dir: PathBuf,
    track_id: String,
    title: String,
) -> CommandOutcome {
    let items_dir = match TrackItemsDirectory::try_new(items_dir) {
        Ok(items_dir) => items_dir,
        Err(error) => return CommandOutcome::failure(Some(error.to_string())),
    };
    let track_id = match TrackLifecycleIdInput::try_new(track_id) {
        Ok(track_id) => track_id,
        Err(error) => return CommandOutcome::failure(Some(error.to_string())),
    };
    let command = match TrackInitCommand::try_new(items_dir, track_id, title) {
        Ok(command) => command,
        Err(error) => return track_init_error_to_outcome(error),
    };
    service
        .execute(command)
        .map(|_| CommandOutcome::success(None))
        .unwrap_or_else(track_init_error_to_outcome)
}

fn track_init_error_to_outcome(error: TrackInitError) -> CommandOutcome {
    CommandOutcome::failure(Some(error.to_string()))
}

fn render_track_archive_outcome(
    service: &dyn TrackArchiveService,
    items_dir: PathBuf,
    track_id: String,
) -> CommandOutcome {
    let items_dir = match TrackItemsDirectory::try_new(items_dir) {
        Ok(items_dir) => items_dir,
        Err(error) => return CommandOutcome::failure(Some(error.to_string())),
    };
    let track_id = match TrackLifecycleIdInput::try_new(track_id) {
        Ok(track_id) => track_id,
        Err(error) => return CommandOutcome::failure(Some(error.to_string())),
    };
    let command = TrackArchiveCommand::new(items_dir, track_id);
    service
        .execute(command)
        .map(render_track_archive_result)
        .unwrap_or_else(track_archive_error_to_outcome)
}

fn render_track_archive_result(
    result: usecase::track_lifecycle::track_archive::TrackArchiveResult,
) -> CommandOutcome {
    let source: &TrackDirectoryPath = &result.source;
    let destination: &TrackDirectoryPath = &result.destination;
    CommandOutcome::success(Some(format!(
        "[OK] Archived track '{}': {} → {}",
        result.track_id,
        source.as_path().display(),
        destination.as_path().display(),
    )))
}

fn track_archive_error_to_outcome(error: TrackArchiveError) -> CommandOutcome {
    CommandOutcome::failure(Some(error.to_string()))
}

fn render_track_branch_create_outcome(
    service: &dyn TrackBranchCreateService,
    items_dir: PathBuf,
    track_id: String,
) -> CommandOutcome {
    let track_id = match TrackLifecycleIdInput::try_new(track_id) {
        Ok(track_id) => track_id,
        Err(error) => return track_branch_create_invalid_track_id(error),
    };

    let items_dir_for_error = items_dir.clone();
    let items_dir = match TrackItemsDirectory::try_new(items_dir) {
        Ok(items_dir) => items_dir,
        Err(_) => return track_branch_create_invalid_items_dir(&items_dir_for_error),
    };
    let command = TrackBranchCreateCommand::new(items_dir, track_id);
    service
        .execute(command)
        .map(|_| CommandOutcome::success(None))
        .unwrap_or_else(track_branch_create_error_to_outcome)
}

fn track_branch_create_error_to_outcome(error: TrackBranchCreateError) -> CommandOutcome {
    track_branch_create_failure(error)
}

fn track_branch_create_failure(error: impl std::fmt::Display) -> CommandOutcome {
    CommandOutcome::failure(Some(format!("[ERROR] {error}")))
}

fn track_branch_create_invalid_track_id(error: impl std::fmt::Display) -> CommandOutcome {
    let error = error.to_string();
    let legacy_error = error.strip_prefix("invalid track id: ").unwrap_or(&error);
    track_branch_create_failure(legacy_error)
}

fn track_branch_create_invalid_items_dir(items_dir: &std::path::Path) -> CommandOutcome {
    CommandOutcome::failure(Some(format!(
        "[ERROR] --items-dir must point to '<project-root>/track/items'; got {}",
        items_dir.display()
    )))
}

fn render_track_branch_switch_outcome(
    service: &dyn TrackBranchSwitchService,
    items_dir: PathBuf,
    track_id: String,
) -> CommandOutcome {
    let track_id = match TrackLifecycleIdInput::try_new(track_id) {
        Ok(track_id) => track_id,
        Err(error) => return track_branch_switch_invalid_track_id(error),
    };

    let items_dir_for_error = items_dir.clone();
    let items_dir = match TrackItemsDirectory::try_new(items_dir) {
        Ok(items_dir) => items_dir,
        Err(_) => return track_branch_switch_invalid_items_dir(&items_dir_for_error),
    };
    let command = TrackBranchSwitchCommand::new(items_dir, track_id);
    service
        .execute(command)
        .map(|result| {
            CommandOutcome::success(Some(format!("[OK] Switched to branch: {}", result.branch)))
        })
        .unwrap_or_else(track_branch_switch_error_to_outcome)
}

fn track_branch_switch_error_to_outcome(error: TrackBranchSwitchError) -> CommandOutcome {
    track_branch_switch_failure(error)
}

fn track_branch_switch_failure(error: impl std::fmt::Display) -> CommandOutcome {
    CommandOutcome::failure(Some(format!("[ERROR] {error}")))
}

fn track_branch_switch_invalid_track_id(error: impl std::fmt::Display) -> CommandOutcome {
    let error = error.to_string();
    let legacy_error = error.strip_prefix("invalid track id: ").unwrap_or(&error);
    track_branch_switch_failure(legacy_error)
}

fn track_branch_switch_invalid_items_dir(items_dir: &std::path::Path) -> CommandOutcome {
    CommandOutcome::failure(Some(format!(
        "[ERROR] --items-dir must point to '<project-root>/track/items'; got {}",
        items_dir.display()
    )))
}

fn render_track_add_task_outcome(
    service: &dyn TrackAddTaskService,
    items_dir: PathBuf,
    track_id: Option<String>,
    description: String,
    section: Option<String>,
    after: Option<String>,
) -> CommandOutcome {
    let items_dir_for_error = items_dir.clone();
    let items_dir = match TrackItemsDirectory::try_new(items_dir) {
        Ok(items_dir) => items_dir,
        Err(_) => return track_add_task_invalid_items_dir(&items_dir_for_error),
    };
    let track = match track_id
        .map(TrackLifecycleIdInput::try_new)
        .transpose()
        .map_err(|error| error.to_string())
    {
        Ok(track_id) => TrackSelection::from_input(track_id),
        Err(error) => return track_add_task_invalid_track_id(error),
    };
    let project_root = project_root_for_items(items_dir.as_path());
    let command = match TrackAddTaskCommand::try_new(items_dir, track, description, section, after)
    {
        Ok(command) => command,
        Err(error) => return track_add_task_error_to_outcome(error),
    };
    service
        .execute(command)
        .map(|result| render_track_add_task_result(result, &project_root))
        .unwrap_or_else(track_add_task_error_to_outcome)
}

fn render_track_add_task_result(
    result: TrackAddTaskResult,
    project_root: &std::path::Path,
) -> CommandOutcome {
    let mut lines = vec![format!(
        "[OK] Added task {}: {} (track status: {})",
        result.task_id, result.description, result.derived_status
    )];
    match result.view_sync {
        TrackViewSyncOutcome::Synchronized(rendered_views) => {
            lines.extend(render_rendered_views(project_root, rendered_views));
        }
        TrackViewSyncOutcome::Warning { rendered_views, diagnostic } => {
            lines.extend(render_rendered_views(project_root, rendered_views));
            lines.push(format!("warning: operation persisted but sync-views failed: {diagnostic}"));
        }
    }
    CommandOutcome::success(Some(lines.join("\n")))
}

fn render_rendered_views(
    project_root: &std::path::Path,
    rendered_views: Vec<RenderedViewPath>,
) -> Vec<String> {
    rendered_views
        .into_iter()
        .map(|path| match path.as_path().strip_prefix(project_root) {
            Ok(relative) => format!("[OK] Rendered: {}", relative.display()),
            Err(_) => format!("[OK] Rendered: {}", path.as_path().display()),
        })
        .collect()
}

fn project_root_for_items(items_dir: &std::path::Path) -> PathBuf {
    items_dir
        .parent()
        .and_then(std::path::Path::parent)
        .filter(|root| !root.as_os_str().is_empty())
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn track_add_task_error_to_outcome(error: TrackAddTaskError) -> CommandOutcome {
    track_add_task_failure(error)
}

fn track_add_task_failure(error: impl std::fmt::Display) -> CommandOutcome {
    CommandOutcome::failure(Some(format!("[ERROR] {error}")))
}

fn track_add_task_invalid_track_id(error: impl std::fmt::Display) -> CommandOutcome {
    let error = error.to_string();
    let legacy_error = error.strip_prefix("invalid track id: ").unwrap_or(&error);
    track_add_task_failure(legacy_error)
}

fn track_add_task_invalid_items_dir(items_dir: &std::path::Path) -> CommandOutcome {
    CommandOutcome::failure(Some(format!(
        "[ERROR] --items-dir must point to '<project-root>/track/items'; got {}",
        items_dir.display()
    )))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unreachable, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn test_track_driver_production_has_no_shim_or_reverse_delegation() {
        let source = include_str!("track.rs");
        let production =
            source.split("#[cfg(test)]").next().expect("production source precedes tests");
        assert!(production.contains("pub fn handle"));
        assert!(production.contains("CommandOutcome"));
        assert!(production.contains("TrackInput"));
        assert!(!production.contains("TrackServiceImpl"));
        assert!(!production.contains("ServiceImpl"));
        assert!(!production.contains("compatibility shim"));
        assert!(!production.contains("composition_root"));
        assert!(!production.contains("cli_composition"));
        assert!(!production.contains("TrackCompositionRoot"));
        assert!(!production.contains(".handle("));
        for needle in [
            "track_init_service",
            "track_transition_service",
            "track_archive_service",
            "track_branch_create_service",
            "track_branch_switch_service",
            "fixpoint_resolve_service",
            "track_add_task_service",
            "track_next_task_service",
            "track_task_counts_service",
            "track_set_override_service",
            "track_clear_override_service",
            "track_set_commit_hash_service",
            "track_switch_base_service",
            "track_resolve_service",
            "track_views_sync_service",
            "track_views_validate_service",
            "base_merge_service",
            "TrackInput::Transition",
            "TrackInput::SetOverride",
            "TrackInput::ClearOverride",
            "TrackInput::FixpointResolve",
            "TrackInput::Resolve",
            "TrackInput::ViewsSync",
            "TrackInput::ViewsValidate",
            "TrackInput::SwitchBase",
        ] {
            assert!(production.contains(needle), "missing injected one-way path {needle}");
        }
    }

    use domain::{
        BaseMergeDirection, BranchStrategySnapshot, CommitHash, MergeMethod, NonEmptyString,
        TrackBranch, TrackId, TrackMetadata,
    };
    use usecase::base_merge::{
        BaseMergeAttemptOutcome, BaseMergeCleanupPort, BaseMergeCleanupRequest,
        BaseMergeContextError, BaseMergeContextPort, BaseMergeGitError, BaseMergeGitPort,
        BaseMergeOutcome, BaselineReplacementError, PostMergeCleanupError, ViewsRegenerationError,
    };
    use usecase::track_lifecycle::TrackCommitHashPort;
    use usecase::track_lifecycle::track_clear_override::{
        TrackClearOverrideCommand, TrackClearOverrideError, TrackClearOverrideResult,
    };
    use usecase::track_lifecycle::track_next_task::{
        TrackNextTaskCommand, TrackNextTaskError, TrackNextTaskResult,
    };
    use usecase::track_lifecycle::track_resolve::{
        TrackResolveCommand, TrackResolveError, TrackResolveResult, TrackResolveService,
    };
    use usecase::track_lifecycle::track_set_commit_hash::{
        TrackSetCommitHashCommand, TrackSetCommitHashError, TrackSetCommitHashResult,
        TrackSetCommitHashService,
    };
    use usecase::track_lifecycle::track_set_override::{
        TrackSetOverrideCommand, TrackSetOverrideError, TrackSetOverrideResult,
        TrackSetOverrideService,
    };
    use usecase::track_lifecycle::track_task_counts::{
        TrackTaskCountsCommand, TrackTaskCountsError, TrackTaskCountsResult, TrackTaskCountsService,
    };
    use usecase::track_lifecycle::track_transition::{
        TrackTransitionCommand, TrackTransitionError, TrackTransitionResult,
    };

    struct NoopCommitRecord;

    impl TrackCommitHashPort for NoopCommitRecord {
        fn persist_current_for_track(
            &self,
            _track_id: &TrackId,
        ) -> Result<CommitHash, usecase::git_workflow::DiagnosticText> {
            Ok(CommitHash::try_new("0123456789abcdef").unwrap())
        }
    }

    struct UnusedTrackInitService;

    struct UnusedTrackArchiveService;

    struct UnusedTrackBranchCreateService;

    struct UnusedTrackBranchSwitchService;

    impl TrackInitService for UnusedTrackInitService {
        fn execute(
            &self,
            _: TrackInitCommand,
        ) -> Result<usecase::track_lifecycle::track_init::TrackInitResult, TrackInitError> {
            unreachable!()
        }
    }

    impl TrackArchiveService for UnusedTrackArchiveService {
        fn execute(
            &self,
            _: TrackArchiveCommand,
        ) -> Result<usecase::track_lifecycle::track_archive::TrackArchiveResult, TrackArchiveError>
        {
            unreachable!()
        }
    }

    impl TrackBranchCreateService for UnusedTrackBranchCreateService {
        fn execute(
            &self,
            _: TrackBranchCreateCommand,
        ) -> Result<
            usecase::track_lifecycle::track_branch_create::TrackBranchCreateResult,
            TrackBranchCreateError,
        > {
            unreachable!()
        }
    }

    impl TrackBranchSwitchService for UnusedTrackBranchSwitchService {
        fn execute(
            &self,
            _: TrackBranchSwitchCommand,
        ) -> Result<
            usecase::track_lifecycle::track_branch_switch::TrackBranchSwitchResult,
            TrackBranchSwitchError,
        > {
            unreachable!()
        }
    }

    struct RecordingTrackBranchSwitchService {
        calls: Mutex<Vec<(PathBuf, String)>>,
        error: Option<String>,
    }

    impl TrackBranchSwitchService for RecordingTrackBranchSwitchService {
        fn execute(
            &self,
            command: TrackBranchSwitchCommand,
        ) -> Result<
            usecase::track_lifecycle::track_branch_switch::TrackBranchSwitchResult,
            TrackBranchSwitchError,
        > {
            self.calls
                .lock()
                .unwrap()
                .push((command.items_dir.as_path().to_path_buf(), command.track_id.to_string()));
            match &self.error {
                Some(error) => Err(TrackBranchSwitchError::ExecutionFailed(
                    usecase::git_workflow::DiagnosticText::new(error),
                )),
                None => {
                    Ok(usecase::track_lifecycle::track_branch_switch::TrackBranchSwitchResult {
                        branch: TrackBranch::try_new(format!("track/{}", command.track_id))
                            .expect("switched branch is valid"),
                    })
                }
            }
        }
    }

    struct RecordingTrackBranchCreateService {
        calls: Mutex<Vec<(PathBuf, String)>>,
        error: Option<String>,
    }

    impl TrackBranchCreateService for RecordingTrackBranchCreateService {
        fn execute(
            &self,
            command: TrackBranchCreateCommand,
        ) -> Result<
            usecase::track_lifecycle::track_branch_create::TrackBranchCreateResult,
            TrackBranchCreateError,
        > {
            self.calls
                .lock()
                .unwrap()
                .push((command.items_dir.as_path().to_path_buf(), command.track_id.to_string()));
            match &self.error {
                Some(error) => Err(TrackBranchCreateError::ExecutionFailed(
                    usecase::git_workflow::DiagnosticText::new(error),
                )),
                None => {
                    Ok(usecase::track_lifecycle::track_branch_create::TrackBranchCreateResult {
                        branch: TrackBranch::try_new(format!("track/{}", command.track_id))
                            .expect("created branch is valid"),
                    })
                }
            }
        }
    }

    struct RecordingTrackArchiveService {
        calls: Mutex<Vec<(PathBuf, String)>>,
        result: Result<(), String>,
    }

    impl TrackArchiveService for RecordingTrackArchiveService {
        fn execute(
            &self,
            command: TrackArchiveCommand,
        ) -> Result<usecase::track_lifecycle::track_archive::TrackArchiveResult, TrackArchiveError>
        {
            self.calls
                .lock()
                .unwrap()
                .push((command.items_dir.as_path().to_path_buf(), command.track_id.to_string()));
            match &self.result {
                Ok(()) => Ok(usecase::track_lifecycle::track_archive::TrackArchiveResult {
                    track_id: command.track_id,
                    source: TrackDirectoryPath::try_new(PathBuf::from(
                        "/workspace/track/items/archive-track",
                    ))
                    .expect("source path is valid"),
                    destination: TrackDirectoryPath::try_new(PathBuf::from(
                        "/workspace/track/archive/archive-track",
                    ))
                    .expect("destination path is valid"),
                }),
                Err(error) => Err(TrackArchiveError::ExecutionFailed(
                    usecase::git_workflow::DiagnosticText::new(error),
                )),
            }
        }
    }

    struct RecordingTrackInitService {
        calls: Mutex<Vec<(PathBuf, String, String)>>,
        result: Result<usecase::track_lifecycle::track_init::TrackInitResult, TrackInitError>,
    }

    impl TrackInitService for RecordingTrackInitService {
        fn execute(
            &self,
            command: TrackInitCommand,
        ) -> Result<usecase::track_lifecycle::track_init::TrackInitResult, TrackInitError> {
            self.calls.lock().unwrap().push((
                command.items_dir.as_path().to_path_buf(),
                command.track_id.to_string(),
                command.title.to_string(),
            ));
            match &self.result {
                Ok(_) => Ok(usecase::track_lifecycle::track_init::TrackInitResult),
                Err(error) => Err(error.clone()),
            }
        }
    }

    struct UnusedTrackAddTaskService;

    impl TrackAddTaskService for UnusedTrackAddTaskService {
        fn execute(&self, _: TrackAddTaskCommand) -> Result<TrackAddTaskResult, TrackAddTaskError> {
            unreachable!()
        }
    }

    struct UnusedTrackNextTaskService;

    struct UnusedTrackTaskCountsService;

    struct UnusedTrackTransitionService;

    struct UnusedTrackSetOverrideService;

    struct UnusedTrackClearOverrideService;

    struct UnusedTrackSetCommitHashService;

    struct UnusedTrackSwitchBaseService;

    struct UnusedTrackResolveService;

    struct UnusedTrackViewsSyncService;

    struct UnusedTrackViewsValidateService;

    impl TrackNextTaskService for UnusedTrackNextTaskService {
        fn execute(
            &self,
            _: TrackNextTaskCommand,
        ) -> Result<TrackNextTaskResult, TrackNextTaskError> {
            unreachable!()
        }
    }

    impl TrackTaskCountsService for UnusedTrackTaskCountsService {
        fn execute(
            &self,
            _: TrackTaskCountsCommand,
        ) -> Result<TrackTaskCountsResult, TrackTaskCountsError> {
            unreachable!()
        }
    }

    impl TrackTransitionService for UnusedTrackTransitionService {
        fn execute(
            &self,
            _: TrackTransitionCommand,
        ) -> Result<TrackTransitionResult, TrackTransitionError> {
            unreachable!()
        }
    }

    impl TrackSetOverrideService for UnusedTrackSetOverrideService {
        fn execute(
            &self,
            _: TrackSetOverrideCommand,
        ) -> Result<TrackSetOverrideResult, TrackSetOverrideError> {
            unreachable!()
        }
    }

    impl TrackClearOverrideService for UnusedTrackClearOverrideService {
        fn execute(
            &self,
            _: TrackClearOverrideCommand,
        ) -> Result<TrackClearOverrideResult, TrackClearOverrideError> {
            unreachable!()
        }
    }

    impl TrackSetCommitHashService for UnusedTrackSetCommitHashService {
        fn execute(
            &self,
            _: TrackSetCommitHashCommand,
        ) -> Result<TrackSetCommitHashResult, TrackSetCommitHashError> {
            unreachable!()
        }
    }

    impl TrackSwitchBaseService for UnusedTrackSwitchBaseService {
        fn execute(
            &self,
            _: usecase::track_lifecycle::track_switch_base::TrackSwitchBaseCommand,
        ) -> Result<
            usecase::track_lifecycle::track_switch_base::TrackSwitchBaseResult,
            usecase::track_lifecycle::track_switch_base::TrackSwitchBaseError,
        > {
            unreachable!()
        }
    }

    impl TrackResolveService for UnusedTrackResolveService {
        fn execute(&self, _: TrackResolveCommand) -> Result<TrackResolveResult, TrackResolveError> {
            unreachable!()
        }
    }

    impl TrackViewsSyncService for UnusedTrackViewsSyncService {
        fn execute(
            &self,
            _: usecase::track_lifecycle::track_views_sync::TrackViewsSyncCommand,
        ) -> Result<
            usecase::track_lifecycle::track_views_sync::TrackViewsSyncResult,
            usecase::track_lifecycle::track_views_sync::TrackViewsSyncError,
        > {
            unreachable!()
        }
    }

    impl TrackViewsValidateService for UnusedTrackViewsValidateService {
        fn execute(
            &self,
            _: usecase::track_lifecycle::track_views_validate::TrackViewsValidateCommand,
        ) -> Result<
            usecase::track_lifecycle::track_views_validate::TrackViewsValidateResult,
            usecase::track_lifecycle::track_views_validate::TrackViewsValidateError,
        > {
            unreachable!()
        }
    }

    struct RecordingTrackNextTaskService {
        calls: Mutex<Vec<(PathBuf, TrackSelection)>>,
        result: TrackNextTaskResult,
    }

    impl TrackNextTaskService for RecordingTrackNextTaskService {
        fn execute(
            &self,
            command: TrackNextTaskCommand,
        ) -> Result<TrackNextTaskResult, TrackNextTaskError> {
            self.calls
                .lock()
                .expect("command lock is available")
                .push((command.items_dir.as_path().to_path_buf(), command.track.clone()));
            match &self.result {
                TrackNextTaskResult::Found { task_id, description, status } => {
                    Ok(TrackNextTaskResult::Found {
                        task_id: task_id.clone(),
                        description: description.clone(),
                        status: *status,
                    })
                }
                TrackNextTaskResult::NoOpenTask => Ok(TrackNextTaskResult::NoOpenTask),
            }
        }
    }

    struct RecordingTrackAddTaskService {
        calls: Mutex<Vec<(PathBuf, TrackSelection, String)>>,
        error: Option<String>,
    }

    impl TrackAddTaskService for RecordingTrackAddTaskService {
        fn execute(
            &self,
            command: TrackAddTaskCommand,
        ) -> Result<TrackAddTaskResult, TrackAddTaskError> {
            let TrackAddTaskCommand { items_dir, track, description, .. } = command;
            self.calls.lock().expect("command lock is available").push((
                items_dir.as_path().to_path_buf(),
                track.clone(),
                description.to_string(),
            ));
            if let Some(error) = &self.error {
                return Err(TrackAddTaskError::ExecutionFailed(
                    usecase::git_workflow::DiagnosticText::new(error),
                ));
            }
            Ok(TrackAddTaskResult {
                track_id: TrackId::try_new("add-task-track").expect("track id is valid"),
                task_id: domain::TaskId::try_new("T002").expect("task id is valid"),
                description,
                status: domain::TaskStatusKind::Todo,
                derived_status: domain::TrackStatus::InProgress,
                view_sync: TrackViewSyncOutcome::Synchronized(Vec::new()),
            })
        }
    }

    struct UnusedFixpointResolveService;

    impl FixpointResolveDriverService for UnusedFixpointResolveService {
        fn fixpoint_resolve(&self, _: FixpointResolveDriverInput) -> FixpointResolveDriverOutcome {
            unreachable!()
        }
    }

    struct StubBaseMergeService {
        result: Mutex<Option<Result<BaseMergeOutcome, usecase::base_merge::BaseMergeError>>>,
        workspaces: Mutex<Vec<PathBuf>>,
    }

    impl StubBaseMergeService {
        fn new(result: Result<BaseMergeOutcome, usecase::base_merge::BaseMergeError>) -> Self {
            Self { result: Mutex::new(Some(result)), workspaces: Mutex::new(Vec::new()) }
        }
    }

    impl BaseMergeService for StubBaseMergeService {
        fn execute(
            &self,
            command: BaseMergeCommand,
        ) -> Result<BaseMergeOutcome, usecase::base_merge::BaseMergeError> {
            self.workspaces.lock().unwrap().push(command.workspace_root);
            self.result.lock().unwrap().take().unwrap()
        }
    }

    fn base_merge_driver<T: BaseMergeService + 'static>(service: Arc<T>) -> TrackDriver {
        TrackDriver::new(
            Arc::new(UnusedTrackInitService),
            Arc::new(UnusedTrackTransitionService),
            Arc::new(UnusedTrackBranchSwitchService),
            Arc::new(UnusedTrackResolveService),
            Arc::new(UnusedTrackViewsValidateService),
            Arc::new(UnusedTrackViewsSyncService),
            Arc::new(UnusedTrackAddTaskService),
            Arc::new(UnusedTrackSetOverrideService),
            Arc::new(UnusedTrackClearOverrideService),
            Arc::new(UnusedTrackNextTaskService),
            Arc::new(UnusedTrackTaskCountsService),
            Arc::new(UnusedTrackArchiveService),
            Arc::new(UnusedTrackBranchCreateService),
            Arc::new(UnusedTrackSwitchBaseService),
            Arc::new(UnusedTrackSetCommitHashService),
            Arc::new(UnusedFixpointResolveService),
            service,
        )
    }

    #[test]
    fn test_track_driver_init_valid_input_routes_to_track_init_service() {
        let service = Arc::new(RecordingTrackInitService {
            calls: Mutex::new(Vec::new()),
            result: Ok(usecase::track_lifecycle::track_init::TrackInitResult),
        });
        let driver = TrackDriver::new(
            service.clone(),
            Arc::new(UnusedTrackTransitionService),
            Arc::new(UnusedTrackBranchSwitchService),
            Arc::new(UnusedTrackResolveService),
            Arc::new(UnusedTrackViewsValidateService),
            Arc::new(UnusedTrackViewsSyncService),
            Arc::new(UnusedTrackAddTaskService),
            Arc::new(UnusedTrackSetOverrideService),
            Arc::new(UnusedTrackClearOverrideService),
            Arc::new(UnusedTrackNextTaskService),
            Arc::new(UnusedTrackTaskCountsService),
            Arc::new(UnusedTrackArchiveService),
            Arc::new(UnusedTrackBranchCreateService),
            Arc::new(UnusedTrackSwitchBaseService),
            Arc::new(UnusedTrackSetCommitHashService),
            Arc::new(UnusedFixpointResolveService),
            Arc::new(StubBaseMergeService::new(Ok(BaseMergeOutcome::Completed))),
        );

        let outcome = driver.handle(TrackInput::Init {
            items_dir: PathBuf::from("track/items"),
            track_id: "new-track".to_owned(),
            description: "New Track".to_owned(),
        });

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout, None);
        assert_eq!(outcome.stderr, None);
        assert_eq!(
            service.calls.lock().unwrap().as_slice(),
            &[(PathBuf::from("track/items"), "new-track".to_owned(), "New Track".to_owned(),)]
        );
    }

    #[test]
    fn test_track_driver_init_service_error_maps_to_stderr_and_exit() {
        let service = Arc::new(RecordingTrackInitService {
            calls: Mutex::new(Vec::new()),
            result: Err(TrackInitError::ExecutionFailed(
                usecase::git_workflow::DiagnosticText::new("disk full"),
            )),
        });
        let driver = TrackDriver::new(
            service.clone(),
            Arc::new(UnusedTrackTransitionService),
            Arc::new(UnusedTrackBranchSwitchService),
            Arc::new(UnusedTrackResolveService),
            Arc::new(UnusedTrackViewsValidateService),
            Arc::new(UnusedTrackViewsSyncService),
            Arc::new(UnusedTrackAddTaskService),
            Arc::new(UnusedTrackSetOverrideService),
            Arc::new(UnusedTrackClearOverrideService),
            Arc::new(UnusedTrackNextTaskService),
            Arc::new(UnusedTrackTaskCountsService),
            Arc::new(UnusedTrackArchiveService),
            Arc::new(UnusedTrackBranchCreateService),
            Arc::new(UnusedTrackSwitchBaseService),
            Arc::new(UnusedTrackSetCommitHashService),
            Arc::new(UnusedFixpointResolveService),
            Arc::new(StubBaseMergeService::new(Ok(BaseMergeOutcome::Completed))),
        );

        let outcome = driver.handle(TrackInput::Init {
            items_dir: PathBuf::from("track/items"),
            track_id: "new-track".to_owned(),
            description: "New Track".to_owned(),
        });

        assert_eq!(outcome.stdout, None);
        assert_eq!(outcome.stderr.as_deref(), Some("disk full"));
        assert_eq!(outcome.exit_code, 1);
        assert_eq!(service.calls.lock().unwrap().len(), 1);
    }

    #[test]
    fn test_track_driver_init_invalid_track_id_returns_failure_without_service_call() {
        let service = Arc::new(RecordingTrackInitService {
            calls: Mutex::new(Vec::new()),
            result: Ok(usecase::track_lifecycle::track_init::TrackInitResult),
        });
        let driver = TrackDriver::new(
            service.clone(),
            Arc::new(UnusedTrackTransitionService),
            Arc::new(UnusedTrackBranchSwitchService),
            Arc::new(UnusedTrackResolveService),
            Arc::new(UnusedTrackViewsValidateService),
            Arc::new(UnusedTrackViewsSyncService),
            Arc::new(UnusedTrackAddTaskService),
            Arc::new(UnusedTrackSetOverrideService),
            Arc::new(UnusedTrackClearOverrideService),
            Arc::new(UnusedTrackNextTaskService),
            Arc::new(UnusedTrackTaskCountsService),
            Arc::new(UnusedTrackArchiveService),
            Arc::new(UnusedTrackBranchCreateService),
            Arc::new(UnusedTrackSwitchBaseService),
            Arc::new(UnusedTrackSetCommitHashService),
            Arc::new(UnusedFixpointResolveService),
            Arc::new(StubBaseMergeService::new(Ok(BaseMergeOutcome::Completed))),
        );

        let outcome = driver.handle(TrackInput::Init {
            items_dir: PathBuf::from("track/items"),
            track_id: "../escape".to_owned(),
            description: "New Track".to_owned(),
        });

        assert!(outcome.stderr.unwrap().contains("invalid track id"));
        assert_eq!(outcome.exit_code, 1);
        assert!(service.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn test_track_driver_add_task_valid_input_routes_to_add_task_service() {
        let service =
            Arc::new(RecordingTrackAddTaskService { calls: Mutex::new(Vec::new()), error: None });
        let driver = TrackDriver::new(
            Arc::new(UnusedTrackInitService),
            Arc::new(UnusedTrackTransitionService),
            Arc::new(UnusedTrackBranchSwitchService),
            Arc::new(UnusedTrackResolveService),
            Arc::new(UnusedTrackViewsValidateService),
            Arc::new(UnusedTrackViewsSyncService),
            service.clone(),
            Arc::new(UnusedTrackSetOverrideService),
            Arc::new(UnusedTrackClearOverrideService),
            Arc::new(UnusedTrackNextTaskService),
            Arc::new(UnusedTrackTaskCountsService),
            Arc::new(UnusedTrackArchiveService),
            Arc::new(UnusedTrackBranchCreateService),
            Arc::new(UnusedTrackSwitchBaseService),
            Arc::new(UnusedTrackSetCommitHashService),
            Arc::new(UnusedFixpointResolveService),
            Arc::new(StubBaseMergeService::new(Ok(BaseMergeOutcome::Completed))),
        );

        let outcome = driver.handle(TrackInput::AddTask {
            items_dir: PathBuf::from("workspace/track/items"),
            track_id: Some("add-task-track".to_owned()),
            description: "new task".to_owned(),
            section: Some("work".to_owned()),
            after: Some("T001".to_owned()),
        });

        assert_eq!(outcome.exit_code, 0);
        assert!(outcome.stdout.expect("add-task output is printed").contains("Added task T002"));
        let calls = service.calls.lock().expect("command lock is available");
        assert_eq!(calls.len(), 1);
        let call = calls.first().expect("one add-task command is recorded");
        assert_eq!(call.0, PathBuf::from("workspace/track/items"));
        assert!(matches!(
            &call.1,
            TrackSelection::Explicit(track_id) if track_id.as_ref() == "add-task-track"
        ));
        assert_eq!(call.2, "new task");
    }

    #[test]
    fn test_track_driver_add_task_invalid_after_returns_failure_without_service_call() {
        let service =
            Arc::new(RecordingTrackAddTaskService { calls: Mutex::new(Vec::new()), error: None });
        let driver = TrackDriver::new(
            Arc::new(UnusedTrackInitService),
            Arc::new(UnusedTrackTransitionService),
            Arc::new(UnusedTrackBranchSwitchService),
            Arc::new(UnusedTrackResolveService),
            Arc::new(UnusedTrackViewsValidateService),
            Arc::new(UnusedTrackViewsSyncService),
            service.clone(),
            Arc::new(UnusedTrackSetOverrideService),
            Arc::new(UnusedTrackClearOverrideService),
            Arc::new(UnusedTrackNextTaskService),
            Arc::new(UnusedTrackTaskCountsService),
            Arc::new(UnusedTrackArchiveService),
            Arc::new(UnusedTrackBranchCreateService),
            Arc::new(UnusedTrackSwitchBaseService),
            Arc::new(UnusedTrackSetCommitHashService),
            Arc::new(UnusedFixpointResolveService),
            Arc::new(StubBaseMergeService::new(Ok(BaseMergeOutcome::Completed))),
        );

        let outcome = driver.handle(TrackInput::AddTask {
            items_dir: PathBuf::from("workspace/track/items"),
            track_id: Some("add-task-track".to_owned()),
            description: "new task".to_owned(),
            section: None,
            after: Some("not-a-task".to_owned()),
        });

        assert_eq!(outcome.exit_code, 1);
        assert!(outcome.stderr.expect("validation error is printed").contains("invalid --after"));
        assert!(service.calls.lock().expect("command lock is available").is_empty());
    }

    #[test]
    fn test_track_driver_add_task_invalid_items_dir_preserves_error_prefix() {
        let service =
            Arc::new(RecordingTrackAddTaskService { calls: Mutex::new(Vec::new()), error: None });
        let driver = TrackDriver::new(
            Arc::new(UnusedTrackInitService),
            Arc::new(UnusedTrackTransitionService),
            Arc::new(UnusedTrackBranchSwitchService),
            Arc::new(UnusedTrackResolveService),
            Arc::new(UnusedTrackViewsValidateService),
            Arc::new(UnusedTrackViewsSyncService),
            service.clone(),
            Arc::new(UnusedTrackSetOverrideService),
            Arc::new(UnusedTrackClearOverrideService),
            Arc::new(UnusedTrackNextTaskService),
            Arc::new(UnusedTrackTaskCountsService),
            Arc::new(UnusedTrackArchiveService),
            Arc::new(UnusedTrackBranchCreateService),
            Arc::new(UnusedTrackSwitchBaseService),
            Arc::new(UnusedTrackSetCommitHashService),
            Arc::new(UnusedFixpointResolveService),
            Arc::new(StubBaseMergeService::new(Ok(BaseMergeOutcome::Completed))),
        );

        let outcome = driver.handle(TrackInput::AddTask {
            items_dir: PathBuf::from("wrong/items"),
            track_id: Some("add-task-track".to_owned()),
            description: "new task".to_owned(),
            section: None,
            after: None,
        });

        let expected_stderr =
            "[ERROR] --items-dir must point to '<project-root>/track/items'; got wrong/items";
        assert_eq!(outcome.stderr.as_deref(), Some(expected_stderr));
        assert_eq!(outcome.exit_code, 1);
        assert!(service.calls.lock().expect("command lock is available").is_empty());
    }

    #[test]
    fn test_track_driver_add_task_invalid_track_id_preserves_legacy_diagnostic() {
        let service =
            Arc::new(RecordingTrackAddTaskService { calls: Mutex::new(Vec::new()), error: None });
        let driver = TrackDriver::new(
            Arc::new(UnusedTrackInitService),
            Arc::new(UnusedTrackTransitionService),
            Arc::new(UnusedTrackBranchSwitchService),
            Arc::new(UnusedTrackResolveService),
            Arc::new(UnusedTrackViewsValidateService),
            Arc::new(UnusedTrackViewsSyncService),
            service.clone(),
            Arc::new(UnusedTrackSetOverrideService),
            Arc::new(UnusedTrackClearOverrideService),
            Arc::new(UnusedTrackNextTaskService),
            Arc::new(UnusedTrackTaskCountsService),
            Arc::new(UnusedTrackArchiveService),
            Arc::new(UnusedTrackBranchCreateService),
            Arc::new(UnusedTrackSwitchBaseService),
            Arc::new(UnusedTrackSetCommitHashService),
            Arc::new(UnusedFixpointResolveService),
            Arc::new(StubBaseMergeService::new(Ok(BaseMergeOutcome::Completed))),
        );

        let outcome = driver.handle(TrackInput::AddTask {
            items_dir: PathBuf::from("workspace/track/items"),
            track_id: Some("INVALID".to_owned()),
            description: "new task".to_owned(),
            section: None,
            after: None,
        });

        assert_eq!(
            outcome.stderr.as_deref(),
            Some("[ERROR] track id 'INVALID' must be a lowercase slug")
        );
        assert_eq!(outcome.exit_code, 1);
        assert!(service.calls.lock().expect("command lock is available").is_empty());
    }

    #[test]
    fn test_track_driver_next_task_valid_input_routes_to_next_task_service() {
        let service = Arc::new(RecordingTrackNextTaskService {
            calls: Mutex::new(Vec::new()),
            result: TrackNextTaskResult::Found {
                task_id: domain::TaskId::try_new("T002".to_owned()).expect("task id is valid"),
                description: domain::NonEmptyString::try_new("next work".to_owned())
                    .expect("description is valid"),
                status: domain::TaskStatusKind::Todo,
            },
        });
        let driver = TrackDriver::new(
            Arc::new(UnusedTrackInitService),
            Arc::new(UnusedTrackTransitionService),
            Arc::new(UnusedTrackBranchSwitchService),
            Arc::new(UnusedTrackResolveService),
            Arc::new(UnusedTrackViewsValidateService),
            Arc::new(UnusedTrackViewsSyncService),
            Arc::new(UnusedTrackAddTaskService),
            Arc::new(UnusedTrackSetOverrideService),
            Arc::new(UnusedTrackClearOverrideService),
            service.clone(),
            Arc::new(UnusedTrackTaskCountsService),
            Arc::new(UnusedTrackArchiveService),
            Arc::new(UnusedTrackBranchCreateService),
            Arc::new(UnusedTrackSwitchBaseService),
            Arc::new(UnusedTrackSetCommitHashService),
            Arc::new(UnusedFixpointResolveService),
            Arc::new(StubBaseMergeService::new(Ok(BaseMergeOutcome::Completed))),
        );

        let outcome = driver.handle(TrackInput::NextTask {
            items_dir: PathBuf::from("workspace/track/items"),
            track_id: Some("next-track".to_owned()),
        });

        assert_eq!(outcome.exit_code, 0);
        let stdout = outcome.stdout.expect("next-task output is printed");
        assert!(stdout.contains("\"task_id\":\"T002\""));
        assert!(stdout.contains("\"description\":\"next work\""));
        assert!(stdout.contains("\"status\":\"todo\""));
        let calls = service.calls.lock().expect("command lock is available");
        assert_eq!(calls.len(), 1);
    }

    #[test]
    fn test_track_driver_next_task_in_progress_status_preserves_json_contract() {
        let service = Arc::new(RecordingTrackNextTaskService {
            calls: Mutex::new(Vec::new()),
            result: TrackNextTaskResult::Found {
                task_id: domain::TaskId::try_new("T003".to_owned()).expect("task id is valid"),
                description: domain::NonEmptyString::try_new("current work".to_owned())
                    .expect("description is valid"),
                status: domain::TaskStatusKind::InProgress,
            },
        });
        let driver = TrackDriver::new(
            Arc::new(UnusedTrackInitService),
            Arc::new(UnusedTrackTransitionService),
            Arc::new(UnusedTrackBranchSwitchService),
            Arc::new(UnusedTrackResolveService),
            Arc::new(UnusedTrackViewsValidateService),
            Arc::new(UnusedTrackViewsSyncService),
            Arc::new(UnusedTrackAddTaskService),
            Arc::new(UnusedTrackSetOverrideService),
            Arc::new(UnusedTrackClearOverrideService),
            service,
            Arc::new(UnusedTrackTaskCountsService),
            Arc::new(UnusedTrackArchiveService),
            Arc::new(UnusedTrackBranchCreateService),
            Arc::new(UnusedTrackSwitchBaseService),
            Arc::new(UnusedTrackSetCommitHashService),
            Arc::new(UnusedFixpointResolveService),
            Arc::new(StubBaseMergeService::new(Ok(BaseMergeOutcome::Completed))),
        );

        let outcome = driver.handle(TrackInput::NextTask {
            items_dir: PathBuf::from("workspace/track/items"),
            track_id: Some("next-track".to_owned()),
        });

        assert_eq!(outcome.exit_code, 0);
        let stdout = outcome.stdout.expect("next-task output is printed");
        assert!(stdout.contains("\"task_id\":\"T003\""));
        assert!(stdout.contains("\"status\":\"in_progress\""));
    }

    #[test]
    fn test_track_driver_next_task_no_open_task_preserves_null_json_contract() {
        let service = Arc::new(RecordingTrackNextTaskService {
            calls: Mutex::new(Vec::new()),
            result: TrackNextTaskResult::NoOpenTask,
        });
        let driver = TrackDriver::new(
            Arc::new(UnusedTrackInitService),
            Arc::new(UnusedTrackTransitionService),
            Arc::new(UnusedTrackBranchSwitchService),
            Arc::new(UnusedTrackResolveService),
            Arc::new(UnusedTrackViewsValidateService),
            Arc::new(UnusedTrackViewsSyncService),
            Arc::new(UnusedTrackAddTaskService),
            Arc::new(UnusedTrackSetOverrideService),
            Arc::new(UnusedTrackClearOverrideService),
            service,
            Arc::new(UnusedTrackTaskCountsService),
            Arc::new(UnusedTrackArchiveService),
            Arc::new(UnusedTrackBranchCreateService),
            Arc::new(UnusedTrackSwitchBaseService),
            Arc::new(UnusedTrackSetCommitHashService),
            Arc::new(UnusedFixpointResolveService),
            Arc::new(StubBaseMergeService::new(Ok(BaseMergeOutcome::Completed))),
        );

        let outcome = driver.handle(TrackInput::NextTask {
            items_dir: PathBuf::from("workspace/track/items"),
            track_id: Some("next-track".to_owned()),
        });

        assert_eq!(outcome.exit_code, 0);
        let stdout = outcome.stdout.expect("next-task output is printed");
        assert!(stdout.contains("\"task_id\":null"));
        assert!(stdout.contains("\"description\":null"));
        assert!(stdout.contains("\"status\":null"));
    }

    struct RecordingTrackTaskCountsService {
        calls: Mutex<Vec<(PathBuf, TrackSelection)>>,
        result: Result<TrackTaskCountsResult, String>,
    }

    impl TrackTaskCountsService for RecordingTrackTaskCountsService {
        fn execute(
            &self,
            command: TrackTaskCountsCommand,
        ) -> Result<TrackTaskCountsResult, TrackTaskCountsError> {
            self.calls
                .lock()
                .expect("command lock is available")
                .push((command.items_dir.as_path().to_path_buf(), command.track));
            match &self.result {
                Ok(result) => Ok(TrackTaskCountsResult {
                    total: result.total,
                    todo: result.todo,
                    in_progress: result.in_progress,
                    done: result.done,
                    skipped: result.skipped,
                }),
                Err(error) => Err(TrackTaskCountsError::ExecutionFailed(
                    usecase::git_workflow::DiagnosticText::new(error),
                )),
            }
        }
    }

    fn task_counts_result(
        total: u64,
        todo: u64,
        in_progress: u64,
        done: u64,
        skipped: u64,
    ) -> TrackTaskCountsResult {
        use usecase::track_lifecycle::TaskCount;
        TrackTaskCountsResult {
            total: TaskCount::new(total),
            todo: TaskCount::new(todo),
            in_progress: TaskCount::new(in_progress),
            done: TaskCount::new(done),
            skipped: TaskCount::new(skipped),
        }
    }

    fn task_counts_driver(service: Arc<RecordingTrackTaskCountsService>) -> TrackDriver {
        TrackDriver::new(
            Arc::new(UnusedTrackInitService),
            Arc::new(UnusedTrackTransitionService),
            Arc::new(UnusedTrackBranchSwitchService),
            Arc::new(UnusedTrackResolveService),
            Arc::new(UnusedTrackViewsValidateService),
            Arc::new(UnusedTrackViewsSyncService),
            Arc::new(UnusedTrackAddTaskService),
            Arc::new(UnusedTrackSetOverrideService),
            Arc::new(UnusedTrackClearOverrideService),
            Arc::new(UnusedTrackNextTaskService),
            service,
            Arc::new(UnusedTrackArchiveService),
            Arc::new(UnusedTrackBranchCreateService),
            Arc::new(UnusedTrackSwitchBaseService),
            Arc::new(UnusedTrackSetCommitHashService),
            Arc::new(UnusedFixpointResolveService),
            Arc::new(StubBaseMergeService::new(Ok(BaseMergeOutcome::Completed))),
        )
    }

    #[test]
    fn test_track_driver_task_counts_valid_input_routes_to_task_counts_service() {
        let service = Arc::new(RecordingTrackTaskCountsService {
            calls: Mutex::new(Vec::new()),
            result: Ok(task_counts_result(10, 2, 1, 3, 4)),
        });
        let driver = task_counts_driver(service.clone());

        let outcome = driver.handle(TrackInput::TaskCounts {
            items_dir: PathBuf::from("workspace/track/items"),
            track_id: Some("counts-track".to_owned()),
        });

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(
            outcome.stdout.as_deref(),
            Some(r#"{"total":10,"todo":2,"in_progress":1,"done":3,"skipped":4}"#)
        );
        assert_eq!(outcome.stderr, None);
        let calls = service.calls.lock().expect("command lock is available");
        assert_eq!(calls.len(), 1);
        let (items_dir, track) = calls.first().expect("one task-counts command is recorded");
        assert_eq!(items_dir, &PathBuf::from("workspace/track/items"));
        assert!(matches!(
            track,
            TrackSelection::Explicit(track_id) if track_id.as_ref() == "counts-track"
        ));
    }

    #[test]
    fn test_track_driver_task_counts_zero_counts_preserves_json_contract() {
        let driver = task_counts_driver(Arc::new(RecordingTrackTaskCountsService {
            calls: Mutex::new(Vec::new()),
            result: Ok(task_counts_result(0, 0, 0, 0, 0)),
        }));

        let outcome = driver.handle(TrackInput::TaskCounts {
            items_dir: PathBuf::from("workspace/track/items"),
            track_id: Some("counts-track".to_owned()),
        });

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(
            outcome.stdout.as_deref(),
            Some(r#"{"total":0,"todo":0,"in_progress":0,"done":0,"skipped":0}"#)
        );
        assert_eq!(outcome.stderr, None);
    }

    #[test]
    fn test_track_driver_task_counts_service_error_maps_to_stderr_and_exit() {
        let service = Arc::new(RecordingTrackTaskCountsService {
            calls: Mutex::new(Vec::new()),
            result: Err("store failed".to_owned()),
        });
        let driver = task_counts_driver(service.clone());

        let outcome = driver.handle(TrackInput::TaskCounts {
            items_dir: PathBuf::from("workspace/track/items"),
            track_id: Some("counts-track".to_owned()),
        });

        assert_eq!(outcome.stdout, None);
        assert_eq!(outcome.stderr.as_deref(), Some("[ERROR] store failed"));
        assert_eq!(outcome.exit_code, 1);
        assert_eq!(service.calls.lock().expect("command lock is available").len(), 1);
    }

    #[test]
    fn test_track_driver_task_counts_invalid_track_id_returns_failure_without_service_call() {
        let service = Arc::new(RecordingTrackTaskCountsService {
            calls: Mutex::new(Vec::new()),
            result: Ok(task_counts_result(0, 0, 0, 0, 0)),
        });
        let driver = task_counts_driver(service.clone());

        let outcome = driver.handle(TrackInput::TaskCounts {
            items_dir: PathBuf::from("workspace/track/items"),
            track_id: Some("INVALID ID".to_owned()),
        });

        assert_eq!(outcome.exit_code, 1);
        assert!(outcome.stderr.expect("validation error is printed").contains("track id"));
        assert!(service.calls.lock().expect("command lock is available").is_empty());
    }

    #[test]
    fn test_track_driver_branch_create_invalid_items_dir_preserves_error_prefix() {
        let service = Arc::new(RecordingTrackBranchCreateService {
            calls: Mutex::new(Vec::new()),
            error: None,
        });
        let driver = TrackDriver::new(
            Arc::new(UnusedTrackInitService),
            Arc::new(UnusedTrackTransitionService),
            Arc::new(UnusedTrackBranchSwitchService),
            Arc::new(UnusedTrackResolveService),
            Arc::new(UnusedTrackViewsValidateService),
            Arc::new(UnusedTrackViewsSyncService),
            Arc::new(UnusedTrackAddTaskService),
            Arc::new(UnusedTrackSetOverrideService),
            Arc::new(UnusedTrackClearOverrideService),
            Arc::new(UnusedTrackNextTaskService),
            Arc::new(UnusedTrackTaskCountsService),
            Arc::new(UnusedTrackArchiveService),
            service.clone(),
            Arc::new(UnusedTrackSwitchBaseService),
            Arc::new(UnusedTrackSetCommitHashService),
            Arc::new(UnusedFixpointResolveService),
            Arc::new(StubBaseMergeService::new(Ok(BaseMergeOutcome::Completed))),
        );

        let outcome = driver.handle(TrackInput::BranchCreate {
            items_dir: PathBuf::from("wrong/items"),
            track_id: "branch-track".to_owned(),
        });

        let expected_stderr =
            "[ERROR] --items-dir must point to '<project-root>/track/items'; got wrong/items";
        assert_eq!(outcome.stderr.as_deref(), Some(expected_stderr));
        assert_eq!(outcome.exit_code, 1);
        assert!(service.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn test_track_driver_archive_valid_input_routes_to_archive_service() {
        let service = Arc::new(RecordingTrackArchiveService {
            calls: Mutex::new(Vec::new()),
            result: Ok(()),
        });
        let driver = TrackDriver::new(
            Arc::new(UnusedTrackInitService),
            Arc::new(UnusedTrackTransitionService),
            Arc::new(UnusedTrackBranchSwitchService),
            Arc::new(UnusedTrackResolveService),
            Arc::new(UnusedTrackViewsValidateService),
            Arc::new(UnusedTrackViewsSyncService),
            Arc::new(UnusedTrackAddTaskService),
            Arc::new(UnusedTrackSetOverrideService),
            Arc::new(UnusedTrackClearOverrideService),
            Arc::new(UnusedTrackNextTaskService),
            Arc::new(UnusedTrackTaskCountsService),
            service.clone(),
            Arc::new(UnusedTrackBranchCreateService),
            Arc::new(UnusedTrackSwitchBaseService),
            Arc::new(UnusedTrackSetCommitHashService),
            Arc::new(UnusedFixpointResolveService),
            Arc::new(StubBaseMergeService::new(Ok(BaseMergeOutcome::Completed))),
        );

        let outcome = driver.handle(TrackInput::Archive {
            items_dir: PathBuf::from("track/items"),
            track_id: "archive-track".to_owned(),
        });

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(
            outcome.stdout.as_deref(),
            Some(
                "[OK] Archived track 'archive-track': /workspace/track/items/archive-track → \
/workspace/track/archive/archive-track"
            )
        );
        assert_eq!(
            service.calls.lock().unwrap().as_slice(),
            &[(PathBuf::from("track/items"), "archive-track".to_owned())]
        );
    }

    #[test]
    fn test_track_driver_archive_rejects_parent_relative_items_path_without_filesystem_access() {
        let service = Arc::new(RecordingTrackArchiveService {
            calls: Mutex::new(Vec::new()),
            result: Ok(()),
        });
        let driver = TrackDriver::new(
            Arc::new(UnusedTrackInitService),
            Arc::new(UnusedTrackTransitionService),
            Arc::new(UnusedTrackBranchSwitchService),
            Arc::new(UnusedTrackResolveService),
            Arc::new(UnusedTrackViewsValidateService),
            Arc::new(UnusedTrackViewsSyncService),
            Arc::new(UnusedTrackAddTaskService),
            Arc::new(UnusedTrackSetOverrideService),
            Arc::new(UnusedTrackClearOverrideService),
            Arc::new(UnusedTrackNextTaskService),
            Arc::new(UnusedTrackTaskCountsService),
            service.clone(),
            Arc::new(UnusedTrackBranchCreateService),
            Arc::new(UnusedTrackSwitchBaseService),
            Arc::new(UnusedTrackSetCommitHashService),
            Arc::new(UnusedFixpointResolveService),
            Arc::new(StubBaseMergeService::new(Ok(BaseMergeOutcome::Completed))),
        );

        let outcome = driver.handle(TrackInput::Archive {
            items_dir: PathBuf::from("/workspace/anchor/../track/items"),
            track_id: "archive-track".to_owned(),
        });

        assert_eq!(outcome.exit_code, 1);
        assert!(outcome.stderr.unwrap().contains("parent traversal"));
        assert_eq!(service.calls.lock().unwrap().as_slice(), &[]);
    }

    #[test]
    fn test_track_driver_archive_invalid_items_dir_returns_failure_without_service_call() {
        let service = Arc::new(RecordingTrackArchiveService {
            calls: Mutex::new(Vec::new()),
            result: Ok(()),
        });
        let driver = TrackDriver::new(
            Arc::new(UnusedTrackInitService),
            Arc::new(UnusedTrackTransitionService),
            Arc::new(UnusedTrackBranchSwitchService),
            Arc::new(UnusedTrackResolveService),
            Arc::new(UnusedTrackViewsValidateService),
            Arc::new(UnusedTrackViewsSyncService),
            Arc::new(UnusedTrackAddTaskService),
            Arc::new(UnusedTrackSetOverrideService),
            Arc::new(UnusedTrackClearOverrideService),
            Arc::new(UnusedTrackNextTaskService),
            Arc::new(UnusedTrackTaskCountsService),
            service.clone(),
            Arc::new(UnusedTrackBranchCreateService),
            Arc::new(UnusedTrackSwitchBaseService),
            Arc::new(UnusedTrackSetCommitHashService),
            Arc::new(UnusedFixpointResolveService),
            Arc::new(StubBaseMergeService::new(Ok(BaseMergeOutcome::Completed))),
        );

        let outcome = driver.handle(TrackInput::Archive {
            items_dir: PathBuf::from("wrong/items"),
            track_id: "archive-track".to_owned(),
        });

        assert_eq!(outcome.exit_code, 1);
        assert!(outcome.stderr.unwrap().contains("track/items"));
        assert!(service.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn test_track_driver_archive_service_error_maps_to_failure_outcome() {
        let service = Arc::new(RecordingTrackArchiveService {
            calls: Mutex::new(Vec::new()),
            result: Err("archive failed".to_owned()),
        });
        let driver = TrackDriver::new(
            Arc::new(UnusedTrackInitService),
            Arc::new(UnusedTrackTransitionService),
            Arc::new(UnusedTrackBranchSwitchService),
            Arc::new(UnusedTrackResolveService),
            Arc::new(UnusedTrackViewsValidateService),
            Arc::new(UnusedTrackViewsSyncService),
            Arc::new(UnusedTrackAddTaskService),
            Arc::new(UnusedTrackSetOverrideService),
            Arc::new(UnusedTrackClearOverrideService),
            Arc::new(UnusedTrackNextTaskService),
            Arc::new(UnusedTrackTaskCountsService),
            service,
            Arc::new(UnusedTrackBranchCreateService),
            Arc::new(UnusedTrackSwitchBaseService),
            Arc::new(UnusedTrackSetCommitHashService),
            Arc::new(UnusedFixpointResolveService),
            Arc::new(StubBaseMergeService::new(Ok(BaseMergeOutcome::Completed))),
        );

        let outcome = driver.handle(TrackInput::Archive {
            items_dir: PathBuf::from("track/items"),
            track_id: "archive-track".to_owned(),
        });

        assert_eq!(outcome.stderr.as_deref(), Some("archive failed"));
        assert_eq!(outcome.exit_code, 1);
    }

    #[test]
    fn test_track_driver_branch_create_valid_input_routes_to_branch_create_service() {
        let service = Arc::new(RecordingTrackBranchCreateService {
            calls: Mutex::new(Vec::new()),
            error: None,
        });
        let driver = TrackDriver::new(
            Arc::new(UnusedTrackInitService),
            Arc::new(UnusedTrackTransitionService),
            Arc::new(UnusedTrackBranchSwitchService),
            Arc::new(UnusedTrackResolveService),
            Arc::new(UnusedTrackViewsValidateService),
            Arc::new(UnusedTrackViewsSyncService),
            Arc::new(UnusedTrackAddTaskService),
            Arc::new(UnusedTrackSetOverrideService),
            Arc::new(UnusedTrackClearOverrideService),
            Arc::new(UnusedTrackNextTaskService),
            Arc::new(UnusedTrackTaskCountsService),
            Arc::new(UnusedTrackArchiveService),
            service.clone(),
            Arc::new(UnusedTrackSwitchBaseService),
            Arc::new(UnusedTrackSetCommitHashService),
            Arc::new(UnusedFixpointResolveService),
            Arc::new(StubBaseMergeService::new(Ok(BaseMergeOutcome::Completed))),
        );

        let outcome = driver.handle(TrackInput::BranchCreate {
            items_dir: PathBuf::from("track/items"),
            track_id: "branch-track".to_owned(),
        });

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout, None);
        assert_eq!(
            service.calls.lock().unwrap().as_slice(),
            &[(PathBuf::from("track/items"), "branch-track".to_owned())]
        );
    }

    #[test]
    fn test_track_driver_branch_create_invalid_input_returns_failure_without_service_call() {
        let service = Arc::new(RecordingTrackBranchCreateService {
            calls: Mutex::new(Vec::new()),
            error: None,
        });
        let driver = TrackDriver::new(
            Arc::new(UnusedTrackInitService),
            Arc::new(UnusedTrackTransitionService),
            Arc::new(UnusedTrackBranchSwitchService),
            Arc::new(UnusedTrackResolveService),
            Arc::new(UnusedTrackViewsValidateService),
            Arc::new(UnusedTrackViewsSyncService),
            Arc::new(UnusedTrackAddTaskService),
            Arc::new(UnusedTrackSetOverrideService),
            Arc::new(UnusedTrackClearOverrideService),
            Arc::new(UnusedTrackNextTaskService),
            Arc::new(UnusedTrackTaskCountsService),
            Arc::new(UnusedTrackArchiveService),
            service.clone(),
            Arc::new(UnusedTrackSwitchBaseService),
            Arc::new(UnusedTrackSetCommitHashService),
            Arc::new(UnusedFixpointResolveService),
            Arc::new(StubBaseMergeService::new(Ok(BaseMergeOutcome::Completed))),
        );

        let outcome = driver.handle(TrackInput::BranchCreate {
            items_dir: PathBuf::from("track/items"),
            track_id: "../escape".to_owned(),
        });

        let expected_stderr = "[ERROR] track id '../escape' must be a lowercase slug";
        assert_eq!(outcome.stderr.as_deref(), Some(expected_stderr));
        assert_eq!(outcome.exit_code, 1);
        assert!(service.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn test_track_driver_branch_create_empty_track_id_preserves_validation_diagnostic() {
        let service = Arc::new(RecordingTrackBranchCreateService {
            calls: Mutex::new(Vec::new()),
            error: None,
        });
        let driver = TrackDriver::new(
            Arc::new(UnusedTrackInitService),
            Arc::new(UnusedTrackTransitionService),
            Arc::new(UnusedTrackBranchSwitchService),
            Arc::new(UnusedTrackResolveService),
            Arc::new(UnusedTrackViewsValidateService),
            Arc::new(UnusedTrackViewsSyncService),
            Arc::new(UnusedTrackAddTaskService),
            Arc::new(UnusedTrackSetOverrideService),
            Arc::new(UnusedTrackClearOverrideService),
            Arc::new(UnusedTrackNextTaskService),
            Arc::new(UnusedTrackTaskCountsService),
            Arc::new(UnusedTrackArchiveService),
            service.clone(),
            Arc::new(UnusedTrackSwitchBaseService),
            Arc::new(UnusedTrackSetCommitHashService),
            Arc::new(UnusedFixpointResolveService),
            Arc::new(StubBaseMergeService::new(Ok(BaseMergeOutcome::Completed))),
        );

        let outcome = driver.handle(TrackInput::BranchCreate {
            items_dir: PathBuf::from("track/items"),
            track_id: String::new(),
        });

        assert_eq!(outcome.stderr.as_deref(), Some("[ERROR] string must not be empty"));
        assert_eq!(outcome.exit_code, 1);
        assert!(service.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn test_track_driver_branch_create_service_error_maps_to_failure_outcome() {
        let service = Arc::new(RecordingTrackBranchCreateService {
            calls: Mutex::new(Vec::new()),
            error: Some("branch creation failed".to_owned()),
        });
        let driver = TrackDriver::new(
            Arc::new(UnusedTrackInitService),
            Arc::new(UnusedTrackTransitionService),
            Arc::new(UnusedTrackBranchSwitchService),
            Arc::new(UnusedTrackResolveService),
            Arc::new(UnusedTrackViewsValidateService),
            Arc::new(UnusedTrackViewsSyncService),
            Arc::new(UnusedTrackAddTaskService),
            Arc::new(UnusedTrackSetOverrideService),
            Arc::new(UnusedTrackClearOverrideService),
            Arc::new(UnusedTrackNextTaskService),
            Arc::new(UnusedTrackTaskCountsService),
            Arc::new(UnusedTrackArchiveService),
            service,
            Arc::new(UnusedTrackSwitchBaseService),
            Arc::new(UnusedTrackSetCommitHashService),
            Arc::new(UnusedFixpointResolveService),
            Arc::new(StubBaseMergeService::new(Ok(BaseMergeOutcome::Completed))),
        );

        let outcome = driver.handle(TrackInput::BranchCreate {
            items_dir: PathBuf::from("track/items"),
            track_id: "branch-track".to_owned(),
        });

        assert_eq!(outcome.stderr.as_deref(), Some("[ERROR] branch creation failed"));
        assert_eq!(outcome.exit_code, 1);
    }

    #[test]
    fn test_track_driver_branch_switch_valid_input_routes_to_branch_switch_service() {
        let service = Arc::new(RecordingTrackBranchSwitchService {
            calls: Mutex::new(Vec::new()),
            error: None,
        });
        let driver = TrackDriver::new(
            Arc::new(UnusedTrackInitService),
            Arc::new(UnusedTrackTransitionService),
            service.clone(),
            Arc::new(UnusedTrackResolveService),
            Arc::new(UnusedTrackViewsValidateService),
            Arc::new(UnusedTrackViewsSyncService),
            Arc::new(UnusedTrackAddTaskService),
            Arc::new(UnusedTrackSetOverrideService),
            Arc::new(UnusedTrackClearOverrideService),
            Arc::new(UnusedTrackNextTaskService),
            Arc::new(UnusedTrackTaskCountsService),
            Arc::new(UnusedTrackArchiveService),
            Arc::new(UnusedTrackBranchCreateService),
            Arc::new(UnusedTrackSwitchBaseService),
            Arc::new(UnusedTrackSetCommitHashService),
            Arc::new(UnusedFixpointResolveService),
            Arc::new(StubBaseMergeService::new(Ok(BaseMergeOutcome::Completed))),
        );

        let outcome = driver.handle(TrackInput::BranchSwitch {
            items_dir: PathBuf::from("track/items"),
            track_id: "switch-track".to_owned(),
        });

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout.as_deref(), Some("[OK] Switched to branch: track/switch-track"));
        assert_eq!(
            service.calls.lock().unwrap().as_slice(),
            &[(PathBuf::from("track/items"), "switch-track".to_owned())]
        );
    }

    #[test]
    fn test_track_driver_branch_switch_invalid_track_id_preserves_legacy_diagnostic() {
        let service = Arc::new(RecordingTrackBranchSwitchService {
            calls: Mutex::new(Vec::new()),
            error: None,
        });
        let driver = TrackDriver::new(
            Arc::new(UnusedTrackInitService),
            Arc::new(UnusedTrackTransitionService),
            service.clone(),
            Arc::new(UnusedTrackResolveService),
            Arc::new(UnusedTrackViewsValidateService),
            Arc::new(UnusedTrackViewsSyncService),
            Arc::new(UnusedTrackAddTaskService),
            Arc::new(UnusedTrackSetOverrideService),
            Arc::new(UnusedTrackClearOverrideService),
            Arc::new(UnusedTrackNextTaskService),
            Arc::new(UnusedTrackTaskCountsService),
            Arc::new(UnusedTrackArchiveService),
            Arc::new(UnusedTrackBranchCreateService),
            Arc::new(UnusedTrackSwitchBaseService),
            Arc::new(UnusedTrackSetCommitHashService),
            Arc::new(UnusedFixpointResolveService),
            Arc::new(StubBaseMergeService::new(Ok(BaseMergeOutcome::Completed))),
        );

        let outcome = driver.handle(TrackInput::BranchSwitch {
            items_dir: PathBuf::from("track/items"),
            track_id: "../escape".to_owned(),
        });

        assert_eq!(
            outcome.stderr.as_deref(),
            Some("[ERROR] track id '../escape' must be a lowercase slug")
        );
        assert_eq!(outcome.exit_code, 1);
        assert!(service.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn test_track_driver_branch_switch_service_error_maps_to_prefixed_failure() {
        let service = Arc::new(RecordingTrackBranchSwitchService {
            calls: Mutex::new(Vec::new()),
            error: Some("branch switch failed".to_owned()),
        });
        let driver = TrackDriver::new(
            Arc::new(UnusedTrackInitService),
            Arc::new(UnusedTrackTransitionService),
            service,
            Arc::new(UnusedTrackResolveService),
            Arc::new(UnusedTrackViewsValidateService),
            Arc::new(UnusedTrackViewsSyncService),
            Arc::new(UnusedTrackAddTaskService),
            Arc::new(UnusedTrackSetOverrideService),
            Arc::new(UnusedTrackClearOverrideService),
            Arc::new(UnusedTrackNextTaskService),
            Arc::new(UnusedTrackTaskCountsService),
            Arc::new(UnusedTrackArchiveService),
            Arc::new(UnusedTrackBranchCreateService),
            Arc::new(UnusedTrackSwitchBaseService),
            Arc::new(UnusedTrackSetCommitHashService),
            Arc::new(UnusedFixpointResolveService),
            Arc::new(StubBaseMergeService::new(Ok(BaseMergeOutcome::Completed))),
        );

        let outcome = driver.handle(TrackInput::BranchSwitch {
            items_dir: PathBuf::from("track/items"),
            track_id: "switch-track".to_owned(),
        });

        assert_eq!(outcome.stderr.as_deref(), Some("[ERROR] branch switch failed"));
        assert_eq!(outcome.exit_code, 1);
    }

    fn base_merge_direction() -> BaseMergeDirection {
        let track_id = TrackId::try_new("merge-track").unwrap();
        let branch = TrackBranch::try_new("track/merge-track").unwrap();
        let metadata = TrackMetadata::with_branch(
            track_id,
            Some(branch),
            "Merge track",
            None,
            BranchStrategySnapshot::new(
                NonEmptyString::try_new("develop").unwrap(),
                NonEmptyString::try_new("develop").unwrap(),
                MergeMethod::Merge,
            ),
        )
        .unwrap();
        domain::derive_base_merge_direction(&metadata).unwrap()
    }

    struct RecordingContext {
        direction: Option<BaseMergeDirection>,
        mismatch: Option<(TrackBranch, TrackBranch)>,
        workspaces: Arc<Mutex<Vec<PathBuf>>>,
    }

    impl BaseMergeContextPort for RecordingContext {
        fn load_direction(
            &self,
            workspace_root: &std::path::Path,
        ) -> Result<BaseMergeDirection, BaseMergeContextError> {
            self.workspaces.lock().unwrap().push(workspace_root.to_path_buf());
            match (&self.direction, &self.mismatch) {
                (Some(direction), None) => Ok(direction.clone()),
                (None, Some((current, expected))) => {
                    Err(BaseMergeContextError::ActiveTrackMismatch {
                        current: current.clone(),
                        expected: expected.clone(),
                    })
                }
                _ => Err(BaseMergeContextError::Unavailable(
                    usecase::git_workflow::DiagnosticText::new("invalid test context"),
                )),
            }
        }
    }

    struct SnapshotSourceMismatchContext {
        workspaces: Arc<Mutex<Vec<PathBuf>>>,
    }

    impl BaseMergeContextPort for SnapshotSourceMismatchContext {
        fn load_direction(
            &self,
            workspace_root: &std::path::Path,
        ) -> Result<BaseMergeDirection, BaseMergeContextError> {
            self.workspaces.lock().unwrap().push(workspace_root.to_path_buf());
            Err(BaseMergeContextError::Unavailable(usecase::git_workflow::DiagnosticText::new(
                "requested source 'release' differs from snapshot base 'develop'",
            )))
        }
    }

    struct RecordingGitPort {
        calls: Arc<Mutex<Vec<(PathBuf, BaseMergeDirection)>>>,
    }

    impl BaseMergeGitPort for RecordingGitPort {
        fn ensure_worktree_clean(
            &self,
            _workspace_root: &std::path::Path,
        ) -> Result<(), BaseMergeGitError> {
            Ok(())
        }

        fn merge_base(
            &self,
            workspace_root: &std::path::Path,
            direction: &BaseMergeDirection,
        ) -> Result<BaseMergeAttemptOutcome, BaseMergeGitError> {
            self.calls.lock().unwrap().push((workspace_root.to_path_buf(), direction.clone()));
            Ok(BaseMergeAttemptOutcome::Clean {
                base_commit: CommitHash::try_new("fedcba9876543210").unwrap(),
            })
        }
    }

    struct RecordingCleanupPort {
        calls: Arc<Mutex<Vec<(&'static str, BaseMergeCleanupRequest)>>>,
    }

    impl RecordingCleanupPort {
        fn record(&self, stage: &'static str, request: &BaseMergeCleanupRequest) {
            self.calls.lock().unwrap().push((stage, request.clone()));
        }
    }

    impl BaseMergeCleanupPort for RecordingCleanupPort {
        fn regenerate_views(
            &self,
            request: &BaseMergeCleanupRequest,
        ) -> Result<(), ViewsRegenerationError> {
            self.record("views", request);
            Ok(())
        }

        fn replace_baselines(
            &self,
            request: &BaseMergeCleanupRequest,
        ) -> Result<(), BaselineReplacementError> {
            self.record("baseline", request);
            Ok(())
        }
    }

    // ── render_fixpoint_resolve_outcome ─────────────────────────────────────
    //
    // Relocated from `apps/cli-composition/src/track/fixpoint_resolve.rs`'s
    // `format_fixpoint_step` tests, adapted to exercise the render logic
    // directly with `FixpointResolveDriverOutcome` inputs instead of a domain
    // `FixpointStep` (cli_driver may only depend on usecase, not domain).

    #[test]
    fn test_render_fixpoint_resolve_outcome_run_dfp() {
        let outcome = render_fixpoint_resolve_outcome(FixpointResolveDriverOutcome::RunDfp);
        assert_eq!(outcome.stdout.as_deref(), Some("run-dfp"));
        assert_eq!(outcome.exit_code, 0);
    }

    #[test]
    fn test_render_fixpoint_resolve_outcome_run_rfp_single_scope() {
        let outcome = render_fixpoint_resolve_outcome(FixpointResolveDriverOutcome::RunRfp {
            scopes: vec!["impl-plan".to_owned()],
        });
        assert_eq!(outcome.stdout.as_deref(), Some("run-rfp scopes=impl-plan"));
    }

    #[test]
    fn test_render_fixpoint_resolve_outcome_run_rfp_multiple_scopes_in_btreeset_order() {
        // "code" < "impl-plan" in BTreeSet order.
        let outcome = render_fixpoint_resolve_outcome(FixpointResolveDriverOutcome::RunRfp {
            scopes: vec!["code".to_owned(), "impl-plan".to_owned()],
        });
        assert_eq!(outcome.stdout.as_deref(), Some("run-rfp scopes=code,impl-plan"));
    }

    #[test]
    fn test_render_fixpoint_resolve_outcome_run_ref_verify() {
        let outcome = render_fixpoint_resolve_outcome(FixpointResolveDriverOutcome::RunRefVerify);
        assert_eq!(outcome.stdout.as_deref(), Some("run-ref-verify"));
    }

    #[test]
    fn test_render_fixpoint_resolve_outcome_commit() {
        let outcome = render_fixpoint_resolve_outcome(FixpointResolveDriverOutcome::Commit);
        assert_eq!(outcome.stdout.as_deref(), Some("commit"));
    }

    #[test]
    fn test_render_fixpoint_resolve_outcome_failure() {
        let outcome = render_fixpoint_resolve_outcome(FixpointResolveDriverOutcome::Failure {
            message: "boom".to_owned(),
        });
        assert_eq!(outcome.stderr.as_deref(), Some("boom"));
        assert_eq!(outcome.exit_code, 1);
    }

    #[test]
    fn test_render_base_merge_result_completed_is_success() {
        let outcome = render_base_merge_result(Ok(BaseMergeOutcome::Completed));

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout.as_deref(), Some("base merge completed"));
    }

    #[test]
    fn test_render_base_merge_result_conflicted_requires_recovery() {
        let outcome = render_base_merge_result(Ok(BaseMergeOutcome::Conflicted));

        assert_eq!(outcome.exit_code, 1);
        assert_eq!(
            outcome.stderr.as_deref(),
            Some(
                "base merge conflicted; continue with the recover workflow (/track:recover on Claude Code, $track-recover on Codex)"
            )
        );
    }

    #[test]
    fn test_render_base_merge_result_error_is_failure() {
        let outcome = render_base_merge_result(Err(usecase::base_merge::BaseMergeError::Context(
            usecase::git_workflow::DiagnosticText::new("missing active track"),
        )));

        assert_eq!(outcome.exit_code, 1);
        assert_eq!(
            outcome.stderr.as_deref(),
            Some("base merge failed: base-merge context failed: missing active track")
        );
    }

    #[test]
    fn test_render_base_merge_result_conflicted_cleanup_failure_keeps_recovery_handoff() {
        let error = usecase::base_merge::BaseMergeError::ConflictedCleanupFailed(
            PostMergeCleanupError::Views(ViewsRegenerationError::Regeneration(
                usecase::git_workflow::DiagnosticText::new("views failed"),
            )),
        );
        let outcome = render_base_merge_result(Err(error));

        assert_eq!(outcome.exit_code, 1);
        let stderr = outcome.stderr.as_deref().expect("conflict failure must render stderr");
        assert!(stderr.contains("base merge conflicted"));
        assert!(stderr.contains("views failed"));
        assert!(stderr.contains("/track:recover on Claude Code"));
        assert!(stderr.contains("$track-recover on Codex"));
    }

    #[test]
    fn test_track_driver_handle_base_merge_renders_conflicted_cleanup_failure_handoff() {
        let error = usecase::base_merge::BaseMergeError::ConflictedCleanupFailed(
            PostMergeCleanupError::Views(ViewsRegenerationError::Regeneration(
                usecase::git_workflow::DiagnosticText::new("views failed"),
            )),
        );
        let outcome = base_merge_driver(Arc::new(StubBaseMergeService::new(Err(error))))
            .handle_base_merge(BaseMergeInput { workspace_root: PathBuf::from("/workspace") });

        assert_eq!(outcome.exit_code, 1);
        let stderr = outcome.stderr.as_deref().expect("conflict failure must render stderr");
        assert!(stderr.contains("base merge conflicted"));
        assert!(stderr.contains("views failed"));
        assert!(stderr.contains("/track:recover on Claude Code"));
        assert!(stderr.contains("$track-recover on Codex"));
        assert_ne!(outcome.stdout.as_deref(), Some("base merge completed"));
    }

    #[test]
    fn test_track_driver_handle_base_merge_delegates_workspace_and_renders_completed() {
        let service = Arc::new(StubBaseMergeService::new(Ok(BaseMergeOutcome::Completed)));
        let driver = base_merge_driver(Arc::clone(&service));
        let workspace = PathBuf::from("/workspace/unchanged");

        let outcome =
            driver.handle_base_merge(BaseMergeInput { workspace_root: workspace.clone() });

        assert_eq!(service.workspaces.lock().unwrap().as_slice(), [workspace]);
        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout.as_deref(), Some("base merge completed"));
    }

    #[test]
    fn test_track_driver_handle_base_merge_real_interactor_orders_cleanup_at_exact_commit() {
        let workspace = PathBuf::from("/workspace/track-driver");
        let context_workspaces = Arc::new(Mutex::new(Vec::new()));
        let git_calls = Arc::new(Mutex::new(Vec::new()));
        let cleanup_calls = Arc::new(Mutex::new(Vec::new()));
        let direction = base_merge_direction();
        let interactor = usecase::base_merge::BaseMergeInteractor::new(
            Arc::new(RecordingContext {
                direction: Some(direction.clone()),
                mismatch: None,
                workspaces: Arc::clone(&context_workspaces),
            }),
            Arc::new(RecordingGitPort { calls: Arc::clone(&git_calls) }),
            Arc::new(RecordingCleanupPort { calls: Arc::clone(&cleanup_calls) }),
            Arc::new(NoopCommitRecord),
        );
        let driver = base_merge_driver(Arc::new(interactor));

        let outcome =
            driver.handle_base_merge(BaseMergeInput { workspace_root: workspace.clone() });

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout.as_deref(), Some("base merge completed"));
        assert_eq!(*context_workspaces.lock().unwrap(), vec![workspace.clone()]);
        assert_eq!(git_calls.lock().unwrap().as_slice(), [(workspace.clone(), direction)]);
        let cleanup_calls = cleanup_calls.lock().unwrap();
        assert_eq!(
            cleanup_calls.iter().map(|(stage, _)| *stage).collect::<Vec<_>>(),
            vec!["baseline", "views"]
        );
        for (_, request) in cleanup_calls.iter() {
            assert_eq!(request.workspace_root, workspace);
            assert_eq!(request.base_commit.as_ref(), "fedcba9876543210");
        }
    }

    #[test]
    fn test_track_driver_handle_base_merge_real_interactor_surfaces_active_track_guard_before_ports()
     {
        let context_workspaces = Arc::new(Mutex::new(Vec::new()));
        let git_calls = Arc::new(Mutex::new(Vec::new()));
        let cleanup_calls = Arc::new(Mutex::new(Vec::new()));
        let interactor = usecase::base_merge::BaseMergeInteractor::new(
            Arc::new(RecordingContext {
                direction: None,
                mismatch: Some((
                    TrackBranch::try_new("track/other").unwrap(),
                    TrackBranch::try_new("track/merge-track").unwrap(),
                )),
                workspaces: Arc::clone(&context_workspaces),
            }),
            Arc::new(RecordingGitPort { calls: Arc::clone(&git_calls) }),
            Arc::new(RecordingCleanupPort { calls: Arc::clone(&cleanup_calls) }),
            Arc::new(NoopCommitRecord),
        );
        let driver = base_merge_driver(Arc::new(interactor));

        let outcome = driver.handle_base_merge(BaseMergeInput {
            workspace_root: PathBuf::from("/workspace/guarded"),
        });

        assert_eq!(outcome.exit_code, 1);
        assert!(outcome.stderr.as_deref().unwrap().contains("does not match active track"));
        assert_eq!(context_workspaces.lock().unwrap().len(), 1);
        assert!(git_calls.lock().unwrap().is_empty());
        assert!(cleanup_calls.lock().unwrap().is_empty());
    }

    #[test]
    fn test_track_driver_handle_base_merge_real_interactor_rejects_snapshot_source_before_ports() {
        let workspace = PathBuf::from("/workspace/snapshot-source-guard");
        let context_workspaces = Arc::new(Mutex::new(Vec::new()));
        let git_calls = Arc::new(Mutex::new(Vec::new()));
        let cleanup_calls = Arc::new(Mutex::new(Vec::new()));
        let interactor = usecase::base_merge::BaseMergeInteractor::new(
            Arc::new(SnapshotSourceMismatchContext { workspaces: Arc::clone(&context_workspaces) }),
            Arc::new(RecordingGitPort { calls: Arc::clone(&git_calls) }),
            Arc::new(RecordingCleanupPort { calls: Arc::clone(&cleanup_calls) }),
            Arc::new(NoopCommitRecord),
        );
        let driver = base_merge_driver(Arc::new(interactor));

        let outcome =
            driver.handle_base_merge(BaseMergeInput { workspace_root: workspace.clone() });

        assert_eq!(outcome.exit_code, 1);
        assert!(
            outcome.stderr.as_deref().unwrap().contains("differs from snapshot base 'develop'")
        );
        assert_ne!(outcome.stdout.as_deref(), Some("base merge completed"));
        assert_eq!(*context_workspaces.lock().unwrap(), vec![workspace]);
        assert!(git_calls.lock().unwrap().is_empty());
        assert!(cleanup_calls.lock().unwrap().is_empty());
    }

    #[test]
    fn test_track_driver_handle_base_merge_renders_guard_failures_without_success() {
        let mismatch = usecase::base_merge::BaseMergeError::ActiveTrackMismatch {
            current: domain::TrackBranch::try_new("track/other").unwrap(),
            expected: domain::TrackBranch::try_new("track/active").unwrap(),
        };
        let mismatch_service = Arc::new(StubBaseMergeService::new(Err(mismatch)));
        let mismatch_outcome = base_merge_driver(mismatch_service)
            .handle_base_merge(BaseMergeInput { workspace_root: PathBuf::from("/workspace") });
        assert_eq!(mismatch_outcome.exit_code, 1);
        assert!(
            mismatch_outcome.stderr.as_deref().unwrap().contains("does not match active track")
        );
        assert_ne!(mismatch_outcome.stdout.as_deref(), Some("base merge completed"));

        let stale_service =
            Arc::new(StubBaseMergeService::new(Err(usecase::base_merge::BaseMergeError::Context(
                usecase::git_workflow::DiagnosticText::new("snapshot source is stale"),
            ))));
        let stale_outcome = base_merge_driver(stale_service)
            .handle_base_merge(BaseMergeInput { workspace_root: PathBuf::from("/workspace") });
        assert_eq!(stale_outcome.exit_code, 1);
        assert!(stale_outcome.stderr.as_deref().unwrap().contains("snapshot source is stale"));
        assert_ne!(stale_outcome.stdout.as_deref(), Some("base merge completed"));
    }

    #[test]
    fn test_track_driver_handle_base_merge_renders_conflict_recovery_handoff() {
        let service = Arc::new(StubBaseMergeService::new(Ok(BaseMergeOutcome::Conflicted)));

        let outcome = base_merge_driver(service)
            .handle_base_merge(BaseMergeInput { workspace_root: PathBuf::from("/workspace") });

        assert_eq!(outcome.exit_code, 1);
        assert_eq!(
            outcome.stderr.as_deref(),
            Some(
                "base merge conflicted; continue with the recover workflow (/track:recover on Claude Code, $track-recover on Codex)"
            )
        );
        assert_ne!(outcome.stdout.as_deref(), Some("base merge completed"));

        let workspace_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .unwrap();
        let workflow =
            std::fs::read_to_string(workspace_root.join(".harness/workflows/track/recover.md"))
                .unwrap();
        let claude_adapter =
            std::fs::read_to_string(workspace_root.join(".claude/commands/track/recover.md"))
                .unwrap();
        let codex_adapter =
            std::fs::read_to_string(workspace_root.join(".agents/skills/track-recover/SKILL.md"))
                .unwrap();
        assert!(workflow.contains("Step 1: Confirm the recovery context"));
        assert!(workflow.contains("Step 2: Resolve the conflict"));
        assert!(workflow.contains("Step 3: Verify and review"));
        assert!(workflow.contains("Step 4: Guarded commit"));
        assert!(claude_adapter.contains("Operational SSoT: `.harness/workflows/track/recover.md`"));
        assert!(claude_adapter.contains("free of recovery sequence"));
        assert!(
            codex_adapter
                .contains("canonical recover workflow is `.harness/workflows/track/recover.md`")
        );
        assert!(codex_adapter.contains("must not duplicate its state machine"));
    }

    struct RecordingTrackSetCommitHashService {
        result: Result<TrackSetCommitHashResult, String>,
        calls: Mutex<Vec<TrackSetCommitHashCommand>>,
    }

    impl TrackSetCommitHashService for RecordingTrackSetCommitHashService {
        fn execute(
            &self,
            command: TrackSetCommitHashCommand,
        ) -> Result<TrackSetCommitHashResult, TrackSetCommitHashError> {
            self.calls.lock().expect("service lock is available").push(command);
            match &self.result {
                Ok(result) => {
                    Ok(TrackSetCommitHashResult { commit_hash: result.commit_hash.clone() })
                }
                Err(error) => Err(TrackSetCommitHashError::ExecutionFailed(
                    usecase::git_workflow::DiagnosticText::new(error),
                )),
            }
        }
    }

    fn set_commit_hash_driver(service: Arc<RecordingTrackSetCommitHashService>) -> TrackDriver {
        TrackDriver::new(
            Arc::new(UnusedTrackInitService),
            Arc::new(UnusedTrackTransitionService),
            Arc::new(UnusedTrackBranchSwitchService),
            Arc::new(UnusedTrackResolveService),
            Arc::new(UnusedTrackViewsValidateService),
            Arc::new(UnusedTrackViewsSyncService),
            Arc::new(UnusedTrackAddTaskService),
            Arc::new(UnusedTrackSetOverrideService),
            Arc::new(UnusedTrackClearOverrideService),
            Arc::new(UnusedTrackNextTaskService),
            Arc::new(UnusedTrackTaskCountsService),
            Arc::new(UnusedTrackArchiveService),
            Arc::new(UnusedTrackBranchCreateService),
            Arc::new(UnusedTrackSwitchBaseService),
            service,
            Arc::new(UnusedFixpointResolveService),
            Arc::new(StubBaseMergeService::new(Ok(BaseMergeOutcome::Completed))),
        )
    }

    #[test]
    fn test_track_driver_set_commit_hash_valid_input_preserves_output_contract() {
        let service = Arc::new(RecordingTrackSetCommitHashService {
            result: Ok(TrackSetCommitHashResult {
                commit_hash: CommitHash::try_new("a".repeat(40)).expect("hash is valid"),
            }),
            calls: Mutex::new(Vec::new()),
        });
        let driver = set_commit_hash_driver(service.clone());
        let input = "commit-track".parse().expect("track id is valid");

        let outcome = driver.handle_set_commit_hash(input);

        assert_eq!(outcome.exit_code, 0);
        assert!(outcome.stdout.as_deref().is_some_and(|stdout| stdout.contains(".commit_hash")));
        assert!(outcome.stderr.as_deref().is_some_and(|stderr| stderr.contains("Recorded")));
        assert_eq!(service.calls.lock().expect("service lock is available").len(), 1);
    }

    #[test]
    fn test_track_driver_set_commit_hash_service_error_returns_recovery_failure() {
        let service = Arc::new(RecordingTrackSetCommitHashService {
            result: Err("current branch mismatch".to_owned()),
            calls: Mutex::new(Vec::new()),
        });
        let driver = set_commit_hash_driver(service);
        let input = "commit-track".parse().expect("track id is valid");

        let outcome = driver.handle_set_commit_hash(input);

        assert_eq!(outcome.exit_code, 1);
        assert!(outcome.stderr.as_deref().is_some_and(|stderr| {
            stderr.contains("current branch mismatch") && stderr.contains("Recovery")
        }));
    }

    struct RecordingTrackSwitchBaseService {
        result: Result<usecase::track_lifecycle::track_switch_base::TrackSwitchBaseResult, String>,
        calls: Mutex<Vec<PathBuf>>,
    }

    impl TrackSwitchBaseService for RecordingTrackSwitchBaseService {
        fn execute(
            &self,
            command: usecase::track_lifecycle::track_switch_base::TrackSwitchBaseCommand,
        ) -> Result<
            usecase::track_lifecycle::track_switch_base::TrackSwitchBaseResult,
            usecase::track_lifecycle::track_switch_base::TrackSwitchBaseError,
        > {
            self.calls
                .lock()
                .expect("service lock is available")
                .push(command.workspace_root.as_path().to_path_buf());
            match &self.result {
                Ok(usecase::track_lifecycle::track_switch_base::TrackSwitchBaseResult::Synced {
                    branch,
                }) => Ok(usecase::track_lifecycle::track_switch_base::TrackSwitchBaseResult::Synced {
                    branch: branch.clone(),
                }),
                Ok(usecase::track_lifecycle::track_switch_base::TrackSwitchBaseResult::SyncWarning {
                    branch,
                }) => Ok(
                    usecase::track_lifecycle::track_switch_base::TrackSwitchBaseResult::SyncWarning {
                        branch: branch.clone(),
                    },
                ),
                Ok(
                    usecase::track_lifecycle::track_switch_base::TrackSwitchBaseResult::CheckoutFailed {
                        branch,
                        exit_code,
                    },
                ) => Ok(
                    usecase::track_lifecycle::track_switch_base::TrackSwitchBaseResult::CheckoutFailed {
                        branch: branch.clone(),
                        exit_code: *exit_code,
                    },
                ),
                Err(error) => {
                    Err(usecase::track_lifecycle::track_switch_base::TrackSwitchBaseError::ExecutionFailed(
                        usecase::git_workflow::DiagnosticText::new(error),
                    ))
                }
            }
        }
    }

    fn switch_base_driver(service: Arc<RecordingTrackSwitchBaseService>) -> TrackDriver {
        TrackDriver::new(
            Arc::new(UnusedTrackInitService),
            Arc::new(UnusedTrackTransitionService),
            Arc::new(UnusedTrackBranchSwitchService),
            Arc::new(UnusedTrackResolveService),
            Arc::new(UnusedTrackViewsValidateService),
            Arc::new(UnusedTrackViewsSyncService),
            Arc::new(UnusedTrackAddTaskService),
            Arc::new(UnusedTrackSetOverrideService),
            Arc::new(UnusedTrackClearOverrideService),
            Arc::new(UnusedTrackNextTaskService),
            Arc::new(UnusedTrackTaskCountsService),
            Arc::new(UnusedTrackArchiveService),
            Arc::new(UnusedTrackBranchCreateService),
            service,
            Arc::new(UnusedTrackSetCommitHashService),
            Arc::new(UnusedFixpointResolveService),
            Arc::new(StubBaseMergeService::new(Ok(BaseMergeOutcome::Completed))),
        )
    }

    #[test]
    fn test_track_driver_switch_base_valid_input_preserves_output_contract() {
        let service = Arc::new(RecordingTrackSwitchBaseService {
            result: Ok(
                usecase::track_lifecycle::track_switch_base::TrackSwitchBaseResult::Synced {
                    branch: domain::BaseBranchName::try_new("main".to_owned())
                        .expect("base branch is valid"),
                },
            ),
            calls: Mutex::new(Vec::new()),
        });
        let driver = switch_base_driver(service.clone());

        let outcome =
            driver.handle(TrackInput::SwitchBase { project_root: PathBuf::from("workspace") });

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(
            outcome.stdout.as_deref(),
            Some(
                "Switching to main...\nPulling latest from origin/main...\n[OK] On main, up to date."
            )
        );
        assert_eq!(outcome.stderr, None);
        assert_eq!(
            service.calls.lock().expect("service lock is available").as_slice(),
            &[PathBuf::from("workspace")]
        );
    }

    #[test]
    fn test_track_driver_switch_base_checkout_failure_preserves_legacy_result() {
        let service = Arc::new(RecordingTrackSwitchBaseService {
            result: Ok(
                usecase::track_lifecycle::track_switch_base::TrackSwitchBaseResult::CheckoutFailed {
                    branch: domain::BaseBranchName::try_new("main".to_owned())
                        .expect("base branch is valid"),
                    exit_code: usecase::track_lifecycle::ProcessExitCode::new(7),
                },
            ),
            calls: Mutex::new(Vec::new()),
        });
        let driver = switch_base_driver(service);

        let outcome =
            driver.handle(TrackInput::SwitchBase { project_root: PathBuf::from("workspace") });

        assert_eq!(outcome.stdout.as_deref(), Some("Failed to checkout main"));
        assert_eq!(outcome.stderr, None);
        assert_eq!(outcome.exit_code, 7);
    }

    struct RecordingTrackResolveService {
        result: Result<TrackResolveResult, String>,
        calls: Mutex<Vec<(PathBuf, TrackSelection)>>,
    }

    impl TrackResolveService for RecordingTrackResolveService {
        fn execute(
            &self,
            command: TrackResolveCommand,
        ) -> Result<TrackResolveResult, TrackResolveError> {
            self.calls
                .lock()
                .expect("service lock is available")
                .push((command.items_dir.as_path().to_path_buf(), command.track.clone()));
            match &self.result {
                Ok(TrackResolveResult::Ready { phase, reason, next_command }) => {
                    Ok(TrackResolveResult::Ready {
                        phase: phase.clone(),
                        reason: usecase::git_workflow::DiagnosticText::new(reason.as_str()),
                        next_command: next_command.clone(),
                    })
                }
                Ok(TrackResolveResult::Blocked { phase, reason, next_command, blocker }) => {
                    Ok(TrackResolveResult::Blocked {
                        phase: phase.clone(),
                        reason: usecase::git_workflow::DiagnosticText::new(reason.as_str()),
                        next_command: next_command.clone(),
                        blocker: usecase::git_workflow::DiagnosticText::new(blocker.as_str()),
                    })
                }
                Err(error) => Err(TrackResolveError::ExecutionFailed(
                    usecase::git_workflow::DiagnosticText::new(error),
                )),
            }
        }
    }

    fn resolve_driver(service: Arc<RecordingTrackResolveService>) -> TrackDriver {
        TrackDriver::new(
            Arc::new(UnusedTrackInitService),
            Arc::new(UnusedTrackTransitionService),
            Arc::new(UnusedTrackBranchSwitchService),
            service,
            Arc::new(UnusedTrackViewsValidateService),
            Arc::new(UnusedTrackViewsSyncService),
            Arc::new(UnusedTrackAddTaskService),
            Arc::new(UnusedTrackSetOverrideService),
            Arc::new(UnusedTrackClearOverrideService),
            Arc::new(UnusedTrackNextTaskService),
            Arc::new(UnusedTrackTaskCountsService),
            Arc::new(UnusedTrackArchiveService),
            Arc::new(UnusedTrackBranchCreateService),
            Arc::new(UnusedTrackSwitchBaseService),
            Arc::new(UnusedTrackSetCommitHashService),
            Arc::new(UnusedFixpointResolveService),
            Arc::new(StubBaseMergeService::new(Ok(BaseMergeOutcome::Completed))),
        )
    }

    fn resolve_argv(value: &str) -> usecase::operator_command::CommandArgv {
        usecase::operator_command::CommandArgv::try_new(vec![
            usecase::operator_command::CommandArgument::try_new(value.to_owned()),
        ])
        .expect("command argv is valid")
    }

    #[test]
    fn test_track_driver_resolve_valid_input_preserves_output_contract() {
        let service = Arc::new(RecordingTrackResolveService {
            result: Ok(TrackResolveResult::Ready {
                phase: domain::track_phase::TrackPhase::InProgress,
                reason: usecase::git_workflow::DiagnosticText::new("track has unresolved tasks"),
                next_command: resolve_argv("/track:implement"),
            }),
            calls: Mutex::new(Vec::new()),
        });
        let driver = resolve_driver(service.clone());
        let outcome = driver.handle(TrackInput::Resolve {
            items_dir: PathBuf::from("workspace/track/items"),
            track_id: Some("resolve-track".to_owned()),
        });
        assert_eq!(outcome.exit_code, 0);
        assert_eq!(
            outcome.stdout.as_deref(),
            Some(
                "Current phase: In Progress\nReason: track has unresolved tasks\nRecommended next command: /track:implement"
            )
        );
        assert_eq!(outcome.stderr, None);
        assert_eq!(service.calls.lock().expect("service lock is available").len(), 1);
    }

    #[test]
    fn test_track_driver_resolve_service_error_maps_to_failure_outcome() {
        let service = Arc::new(RecordingTrackResolveService {
            result: Err("track not found: missing".to_owned()),
            calls: Mutex::new(Vec::new()),
        });
        let driver = resolve_driver(service);
        let outcome = driver.handle(TrackInput::Resolve {
            items_dir: PathBuf::from("workspace/track/items"),
            track_id: Some("missing".to_owned()),
        });
        assert_eq!(outcome.exit_code, 1);
        assert_eq!(
            outcome.stderr.as_deref(),
            Some("[ERROR] resolve failed: track not found: missing")
        );
    }

    struct RecordingTrackViewsSyncService {
        result: Result<usecase::track_lifecycle::track_views_sync::TrackViewsSyncResult, String>,
        calls: Mutex<Vec<(PathBuf, TrackSelection)>>,
    }

    impl TrackViewsSyncService for RecordingTrackViewsSyncService {
        fn execute(
            &self,
            command: usecase::track_lifecycle::track_views_sync::TrackViewsSyncCommand,
        ) -> Result<
            usecase::track_lifecycle::track_views_sync::TrackViewsSyncResult,
            usecase::track_lifecycle::track_views_sync::TrackViewsSyncError,
        > {
            self.calls
                .lock()
                .expect("service lock is available")
                .push((command.workspace_root.as_path().to_path_buf(), command.scope.clone()));
            match &self.result {
                Ok(usecase::track_lifecycle::track_views_sync::TrackViewsSyncResult::AlreadyCurrent) => {
                    Ok(usecase::track_lifecycle::track_views_sync::TrackViewsSyncResult::AlreadyCurrent)
                }
                Ok(usecase::track_lifecycle::track_views_sync::TrackViewsSyncResult::Rendered(
                    paths,
                )) => Ok(usecase::track_lifecycle::track_views_sync::TrackViewsSyncResult::Rendered(
                    paths
                        .iter()
                        .map(|path| RenderedViewPath::new(path.as_path().to_path_buf()))
                        .collect(),
                )),
                Err(error) => {
                    Err(usecase::track_lifecycle::track_views_sync::TrackViewsSyncError::ExecutionFailed(
                        usecase::git_workflow::DiagnosticText::new(error),
                    ))
                }
            }
        }
    }

    fn views_sync_driver(service: Arc<RecordingTrackViewsSyncService>) -> TrackDriver {
        TrackDriver::new(
            Arc::new(UnusedTrackInitService),
            Arc::new(UnusedTrackTransitionService),
            Arc::new(UnusedTrackBranchSwitchService),
            Arc::new(UnusedTrackResolveService),
            Arc::new(UnusedTrackViewsValidateService),
            service,
            Arc::new(UnusedTrackAddTaskService),
            Arc::new(UnusedTrackSetOverrideService),
            Arc::new(UnusedTrackClearOverrideService),
            Arc::new(UnusedTrackNextTaskService),
            Arc::new(UnusedTrackTaskCountsService),
            Arc::new(UnusedTrackArchiveService),
            Arc::new(UnusedTrackBranchCreateService),
            Arc::new(UnusedTrackSwitchBaseService),
            Arc::new(UnusedTrackSetCommitHashService),
            Arc::new(UnusedFixpointResolveService),
            Arc::new(StubBaseMergeService::new(Ok(BaseMergeOutcome::Completed))),
        )
    }

    #[test]
    fn test_track_driver_views_sync_valid_input_preserves_output_contract() {
        let service = Arc::new(RecordingTrackViewsSyncService {
            result: Ok(
                usecase::track_lifecycle::track_views_sync::TrackViewsSyncResult::AlreadyCurrent,
            ),
            calls: Mutex::new(Vec::new()),
        });
        let driver = views_sync_driver(service.clone());
        let outcome = driver.handle(TrackInput::ViewsSync {
            project_root: PathBuf::from("workspace"),
            track_id: Some("views-track".to_owned()),
        });
        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout.as_deref(), Some("[OK] All views already up to date"));
        assert_eq!(outcome.stderr, None);
        let calls = service.calls.lock().expect("service lock is available");
        assert_eq!(calls.len(), 1);
        assert!(matches!(
            calls.first().map(|(_, selection)| selection),
            Some(usecase::track_lifecycle::TrackSelection::Explicit(_))
        ));
    }

    #[test]
    fn test_track_driver_views_sync_service_error_maps_to_failure_outcome() {
        let service = Arc::new(RecordingTrackViewsSyncService {
            result: Err("disk full".to_owned()),
            calls: Mutex::new(Vec::new()),
        });
        let driver = views_sync_driver(service);
        let outcome = driver.handle(TrackInput::ViewsSync {
            project_root: PathBuf::from("workspace"),
            track_id: Some("views-track".to_owned()),
        });
        assert_eq!(outcome.exit_code, 1);
        assert_eq!(outcome.stderr.as_deref(), Some("[ERROR] sync-views failed: disk full"));
        assert_eq!(outcome.stdout, None);
    }

    struct RecordingTrackViewsValidateService {
        result: Result<
            usecase::track_lifecycle::track_views_validate::TrackViewsValidateResult,
            String,
        >,
        calls: Mutex<Vec<PathBuf>>,
    }

    impl TrackViewsValidateService for RecordingTrackViewsValidateService {
        fn execute(
            &self,
            command: usecase::track_lifecycle::track_views_validate::TrackViewsValidateCommand,
        ) -> Result<
            usecase::track_lifecycle::track_views_validate::TrackViewsValidateResult,
            usecase::track_lifecycle::track_views_validate::TrackViewsValidateError,
        > {
            self.calls
                .lock()
                .expect("service lock is available")
                .push(command.workspace_root.as_path().to_path_buf());
            match &self.result {
                Ok(usecase::track_lifecycle::track_views_validate::TrackViewsValidateResult) => {
                    Ok(usecase::track_lifecycle::track_views_validate::TrackViewsValidateResult)
                }
                Err(error) => Err(
                    usecase::track_lifecycle::track_views_validate::TrackViewsValidateError::ExecutionFailed(
                        usecase::git_workflow::DiagnosticText::new(error),
                    ),
                ),
            }
        }
    }

    fn views_validate_driver(service: Arc<RecordingTrackViewsValidateService>) -> TrackDriver {
        TrackDriver::new(
            Arc::new(UnusedTrackInitService),
            Arc::new(UnusedTrackTransitionService),
            Arc::new(UnusedTrackBranchSwitchService),
            Arc::new(UnusedTrackResolveService),
            service,
            Arc::new(UnusedTrackViewsSyncService),
            Arc::new(UnusedTrackAddTaskService),
            Arc::new(UnusedTrackSetOverrideService),
            Arc::new(UnusedTrackClearOverrideService),
            Arc::new(UnusedTrackNextTaskService),
            Arc::new(UnusedTrackTaskCountsService),
            Arc::new(UnusedTrackArchiveService),
            Arc::new(UnusedTrackBranchCreateService),
            Arc::new(UnusedTrackSwitchBaseService),
            Arc::new(UnusedTrackSetCommitHashService),
            Arc::new(UnusedFixpointResolveService),
            Arc::new(StubBaseMergeService::new(Ok(BaseMergeOutcome::Completed))),
        )
    }

    #[test]
    fn test_track_driver_views_validate_valid_input_preserves_output_contract() {
        let service = Arc::new(RecordingTrackViewsValidateService {
            result: Ok(usecase::track_lifecycle::track_views_validate::TrackViewsValidateResult),
            calls: Mutex::new(Vec::new()),
        });
        let driver = views_validate_driver(service.clone());
        let project_root = PathBuf::from("workspace");
        let outcome =
            driver.handle(TrackInput::ViewsValidate { project_root: project_root.clone() });
        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout.as_deref(), Some("[OK] Track metadata is valid"));
        assert_eq!(outcome.stderr, None);
        let calls = service.calls.lock().expect("service lock is available");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls.first(), Some(&project_root));
    }

    #[test]
    fn test_track_driver_views_validate_service_error_maps_to_failure_outcome() {
        let service = Arc::new(RecordingTrackViewsValidateService {
            result: Err("invalid metadata".to_owned()),
            calls: Mutex::new(Vec::new()),
        });
        let driver = views_validate_driver(service);
        let outcome =
            driver.handle(TrackInput::ViewsValidate { project_root: PathBuf::from("workspace") });
        assert_eq!(outcome.exit_code, 1);
        assert_eq!(
            outcome.stderr.as_deref(),
            Some("[ERROR] track metadata validation failed: invalid metadata")
        );
        assert_eq!(outcome.stdout, None);
    }
}
