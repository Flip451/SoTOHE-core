//! CLI subcommand for track operations using FsTrackStore.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use clap::{Args, Subcommand};
use domain::{
    CommitHash, TaskId, TaskStatusKind, TaskTransition, TrackBranch, TrackId, TrackReader,
    TrackWriter,
};
use infrastructure::git_cli::{
    GitRepository, SystemGitRepo, TrackBranchRecord, load_explicit_track_branch_from_items_dir,
};
use infrastructure::lock::FsFileLockManager;
use infrastructure::track::codec::{self, DocumentMeta};
use infrastructure::track::fs_store::FsTrackStore;
use infrastructure::track::render;
use usecase::track_activation::{ActivateTrackOutcome, ActivateTrackUseCase};

/// Default timeout for lock acquisition during track operations.
const DEFAULT_LOCK_TIMEOUT: Duration = Duration::from_millis(5000);

fn resolve_project_root(items_dir: &std::path::Path) -> Result<PathBuf, String> {
    let items_name = items_dir.file_name().and_then(|name| name.to_str());
    let track_dir = items_dir.parent();
    let track_name = track_dir.and_then(std::path::Path::file_name).and_then(|name| name.to_str());
    let project_root = track_dir.and_then(std::path::Path::parent);

    match (items_name, track_name, project_root) {
        (Some("items"), Some("track"), Some(root)) => Ok(root.to_path_buf()),
        _ => Err(format!(
            "--items-dir must point to '<project-root>/track/items'; got {}",
            items_dir.display()
        )),
    }
}

#[derive(Debug, Subcommand)]
pub enum TrackCommand {
    /// Transition a task to a new status (atomic read-modify-write).
    Transition {
        /// Path to the track items root directory (e.g., `track/items`).
        #[arg(long)]
        items_dir: PathBuf,

        /// Locks directory for exclusive access.
        #[arg(long, default_value = ".locks")]
        locks_dir: PathBuf,

        /// Track ID (directory name under items_dir).
        track_id: String,

        /// Task ID (e.g., T1, T2).
        task_id: String,

        /// Target status: todo, in_progress, done, skipped.
        target_status: String,

        /// Commit hash (required when target_status is "done", optional).
        #[arg(long)]
        commit_hash: Option<String>,

        /// Skip branch validation (escape hatch for CI/testing).
        #[arg(long, default_value_t = false)]
        skip_branch_check: bool,
    },

    /// Create or switch to a track branch.
    Branch {
        #[command(subcommand)]
        action: BranchAction,
    },

    /// Materialize a planning-only track into its track branch and switch to it.
    Activate(ActivateArgs),

    /// Resolve the current track phase, next command, and blocker.
    Resolve(ResolveArgs),

    /// Validate track metadata and/or regenerate rendered views from metadata.json.
    Views {
        #[command(subcommand)]
        action: ViewAction,
    },
}

#[derive(Debug, Subcommand)]
pub enum BranchAction {
    /// Create a new branch `track/<track-id>` from `main` and switch to it.
    Create(BranchArgs),

    /// Switch to an existing branch `track/<track-id>`.
    Switch(BranchArgs),
}

#[derive(Debug, Args, Clone)]
pub struct BranchArgs {
    /// Path to the track items root directory (e.g., `track/items`).
    #[arg(long, default_value = "track/items")]
    items_dir: PathBuf,

    /// Locks directory for exclusive access.
    #[arg(long, default_value = ".locks")]
    locks_dir: PathBuf,

    /// Track ID used to form the branch name `track/<track-id>`.
    track_id: String,
}

#[derive(Debug, Args, Clone)]
pub struct ResolveArgs {
    /// Path to the track items root directory (e.g., `track/items`).
    #[arg(long, default_value = "track/items")]
    items_dir: PathBuf,

    /// Track ID. If omitted, auto-detects from the current git branch.
    track_id: Option<String>,
}

#[derive(Debug, Args, Clone)]
pub struct ActivateArgs {
    /// Path to the track items root directory (e.g., `track/items`).
    #[arg(long, default_value = "track/items")]
    items_dir: PathBuf,

    /// Locks directory for exclusive access.
    #[arg(long, default_value = ".locks")]
    locks_dir: PathBuf,

    /// Track ID used to form the branch name `track/<track-id>`.
    track_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BranchMode {
    Create,
    Switch,
    Auto,
}

#[derive(Debug, Subcommand)]
pub enum ViewAction {
    /// Validate metadata.json files under the repository.
    Validate {
        /// Project root containing `track/items` and `track/archive`.
        #[arg(long, default_value = ".")]
        project_root: PathBuf,
    },

    /// Render `plan.md` and `registry.md` from metadata.json.
    Sync {
        /// Project root containing `track/items` and `track/archive`.
        #[arg(long, default_value = ".")]
        project_root: PathBuf,

        /// Sync only one active track's `plan.md`.
        #[arg(long)]
        track_id: Option<String>,
    },
}

pub fn execute(cmd: TrackCommand) -> ExitCode {
    match cmd {
        TrackCommand::Transition {
            items_dir,
            locks_dir,
            track_id,
            task_id,
            target_status,
            commit_hash,
            skip_branch_check,
        } => execute_transition(
            items_dir,
            locks_dir,
            track_id,
            task_id,
            target_status,
            commit_hash,
            skip_branch_check,
        ),
        TrackCommand::Branch { action } => execute_branch(action),
        TrackCommand::Activate(args) => execute_activate(args, BranchMode::Auto),
        TrackCommand::Resolve(args) => execute_resolve(args),
        TrackCommand::Views { action } => execute_views(action),
    }
}

fn execute_views(action: ViewAction) -> ExitCode {
    match action {
        ViewAction::Validate { project_root } => {
            match render::validate_track_snapshots(&project_root) {
                Ok(()) => {
                    println!("[OK] Track metadata is valid");
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    eprintln!("track metadata validation failed: {err}");
                    ExitCode::FAILURE
                }
            }
        }
        ViewAction::Sync { project_root, track_id } => {
            match render::sync_rendered_views(&project_root, track_id.as_deref()) {
                Ok(changed) => {
                    if changed.is_empty() {
                        println!("[OK] All views already up to date");
                    } else {
                        for path in changed {
                            match path.strip_prefix(&project_root) {
                                Ok(relative) => println!("[OK] Rendered: {}", relative.display()),
                                Err(_) => println!("[OK] Rendered: {}", path.display()),
                            }
                        }
                    }
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    eprintln!("sync-views failed: {err}");
                    ExitCode::FAILURE
                }
            }
        }
    }
}

fn execute_resolve(args: ResolveArgs) -> ExitCode {
    let ResolveArgs { items_dir, track_id } = args;

    // Validate items_dir structure (must be <root>/track/items).
    if let Err(err) = resolve_project_root(&items_dir) {
        eprintln!("{err}");
        return ExitCode::FAILURE;
    }

    // Auto-detect is only safe when items_dir is the default (track/items
    // relative to CWD), because auto_detect uses SystemGitRepo::discover
    // from CWD.  When a custom --items-dir is supplied, require explicit id.
    let is_default_items_dir = items_dir == std::path::Path::new("track/items");

    let effective_track_id = match track_id {
        Some(id) => id,
        None if !is_default_items_dir => {
            eprintln!("resolve failed: custom --items-dir requires an explicit track-id argument");
            return ExitCode::FAILURE;
        }
        None => match auto_detect_track_id_from_branch() {
            Ok(id) => id,
            Err(err) => {
                eprintln!("resolve failed: {err}");
                return ExitCode::FAILURE;
            }
        },
    };

    let track_id = match TrackId::new(&effective_track_id) {
        Ok(id) => id,
        Err(err) => {
            eprintln!("invalid track id: {err}");
            return ExitCode::FAILURE;
        }
    };

    let (track, meta) = match read_track_metadata(&items_dir, &track_id) {
        Ok(result) => result,
        Err(err) => {
            eprintln!("resolve failed: {err}");
            return ExitCode::FAILURE;
        }
    };

    // Fail-closed: reject branchless v3 tracks that violate planning-only invariants.
    // Both raw status (from JSON) and domain-derived status (from tasks) must be planned.
    if meta.schema_version == 3 && track.branch().is_none() {
        let raw = meta.original_status.as_deref();
        let derived = track.status();
        if raw != Some("planned") {
            eprintln!(
                "resolve failed: track '{track_id}' is branchless v3 but raw status is '{}', \
                 not planned; metadata may be corrupt",
                raw.unwrap_or("(missing)")
            );
            return ExitCode::FAILURE;
        }
        if derived != domain::TrackStatus::Planned {
            eprintln!(
                "resolve failed: track '{track_id}' is branchless v3 but derived status is \
                 '{derived}', not planned; metadata may be corrupt"
            );
            return ExitCode::FAILURE;
        }
    }

    // Note: TrackStatus::Archived is not reachable from domain-derived status();
    // archived tracks live under track/archive/ and are not resolved by this command.
    let info = domain::track_phase::resolve_phase(&track, meta.schema_version);

    println!("Current phase: {}", info.phase);
    println!("Reason: {}", info.reason);
    println!("Recommended next command: {}", info.next_command);
    if let Some(blocker) = &info.blocker {
        println!("Blocker: {blocker}");
    }

    ExitCode::SUCCESS
}

/// Auto-detect track ID from the current git branch.
///
/// Assumes `items_dir` belongs to the same repo as `CWD` (the default
/// `track/items` is relative to CWD, so they always match in practice).
fn auto_detect_track_id_from_branch() -> Result<String, String> {
    let repo = SystemGitRepo::discover()?;
    let branch = repo.current_branch()?;
    match branch.as_deref() {
        Some(b) if b.starts_with("track/") => Ok(b["track/".len()..].to_owned()),
        Some("HEAD") => Err("detached HEAD; provide an explicit track-id".to_owned()),
        Some(b) => Err(format!("not on a track branch (on '{b}'); provide an explicit track-id")),
        None => Err("cannot determine current git branch; provide an explicit track-id".to_owned()),
    }
}

/// Read-only metadata load via codec (no lock manager needed).
fn read_track_metadata(
    items_dir: &std::path::Path,
    track_id: &TrackId,
) -> Result<(domain::TrackMetadata, DocumentMeta), String> {
    let path = items_dir.join(track_id.as_str()).join("metadata.json");
    let json = std::fs::read_to_string(&path)
        .map_err(|err| format!("cannot read {}: {err}", path.display()))?;
    codec::decode(&json).map_err(|err| format!("cannot parse {}: {err}", path.display()))
}

fn execute_transition(
    items_dir: PathBuf,
    locks_dir: PathBuf,
    track_id: String,
    task_id: String,
    target_status: String,
    commit_hash: Option<String>,
    skip_branch_check: bool,
) -> ExitCode {
    // Validate inputs.
    let track_id = match TrackId::new(&track_id) {
        Ok(id) => id,
        Err(err) => {
            eprintln!("invalid track id: {err}");
            return ExitCode::FAILURE;
        }
    };

    let task_id = match TaskId::new(&task_id) {
        Ok(id) => id,
        Err(err) => {
            eprintln!("invalid task id: {err}");
            return ExitCode::FAILURE;
        }
    };

    // Validate commit_hash early if provided.
    let parsed_hash = match commit_hash {
        Some(h) => match CommitHash::new(h) {
            Ok(hash) => Some(hash),
            Err(err) => {
                eprintln!("invalid commit hash: {err}");
                return ExitCode::FAILURE;
            }
        },
        None => None,
    };

    // Preserve items_dir for branch guard before moving into FsTrackStore.
    let repo_dir = items_dir.clone();
    let project_root = match resolve_project_root(&repo_dir) {
        Ok(path) => path,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::FAILURE;
        }
    };

    // Build FsTrackStore.
    let lock_manager = match FsFileLockManager::new(&locks_dir) {
        Ok(lm) => Arc::new(lm),
        Err(err) => {
            eprintln!("failed to initialize lock manager: {err}");
            return ExitCode::FAILURE;
        }
    };

    let store = Arc::new(FsTrackStore::new(items_dir.clone(), lock_manager, DEFAULT_LOCK_TIMEOUT));

    if let Err(msg) = reject_branchless_implementation_transition(
        &project_root,
        &items_dir,
        &track_id,
        &target_status,
    ) {
        eprintln!("activation guard: {msg}");
        return ExitCode::FAILURE;
    }

    // Branch guard: reject if current git branch does not match metadata.json branch.
    if !skip_branch_check {
        if let Err(msg) = verify_branch_guard(&*store, &track_id, &repo_dir) {
            eprintln!("branch guard: {msg}");
            return ExitCode::FAILURE;
        }
    }

    // Validate target_status before entering the locked update section.
    if !["todo", "in_progress", "done", "skipped"].contains(&target_status.as_str()) {
        eprintln!("unsupported target status: {target_status}");
        return ExitCode::FAILURE;
    }

    // Use TrackWriter::update directly to resolve the correct transition
    // based on current task status (e.g., "in_progress" from "done" is Reopen, not Start).
    match store.update(&track_id, |track| {
        let task = track.tasks().iter().find(|t| *t.id() == task_id).ok_or_else(|| {
            domain::TransitionError::TaskNotFound { task_id: task_id.to_string() }
        })?;
        let current_kind = task.status().kind();

        // target_status was validated above, so this branch is unreachable in practice.
        let transition = match resolve_transition(&target_status, current_kind, parsed_hash) {
            Ok(t) => t,
            Err(msg) => {
                return Err(domain::ValidationError::InvalidTrackId(msg).into());
            }
        };

        track.transition_task(&task_id, transition)?;
        Ok(())
    }) {
        Ok(track) => {
            println!(
                "[OK] {}: transitioned to {} (track status: {})",
                task_id,
                target_status,
                track.status()
            );
            match render::sync_rendered_views(&project_root, Some(track_id.as_str())) {
                Ok(changed) => {
                    for path in changed {
                        match path.strip_prefix(&project_root) {
                            Ok(relative) => println!("[OK] Rendered: {}", relative.display()),
                            Err(_) => println!("[OK] Rendered: {}", path.display()),
                        }
                    }
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    eprintln!("warning: transition persisted but sync-views failed: {err}");
                    ExitCode::SUCCESS
                }
            }
        }
        Err(err) => {
            eprintln!("transition failed: {err}");
            ExitCode::FAILURE
        }
    }
}

fn execute_branch(action: BranchAction) -> ExitCode {
    match action {
        BranchAction::Create(args) => execute_activate(
            ActivateArgs {
                items_dir: args.items_dir,
                locks_dir: args.locks_dir,
                track_id: args.track_id,
            },
            BranchMode::Create,
        ),
        BranchAction::Switch(args) => execute_activate(
            ActivateArgs {
                items_dir: args.items_dir,
                locks_dir: args.locks_dir,
                track_id: args.track_id,
            },
            BranchMode::Switch,
        ),
    }
}

fn execute_activate(args: ActivateArgs, mode: BranchMode) -> ExitCode {
    let ActivateArgs { items_dir, locks_dir, track_id } = args;

    let track_id = match TrackId::new(&track_id) {
        Ok(id) => id,
        Err(err) => {
            eprintln!("invalid track id: {err}");
            return ExitCode::FAILURE;
        }
    };

    let branch_name = format!("track/{track_id}");
    let branch = match TrackBranch::new(&branch_name) {
        Ok(branch) => branch,
        Err(err) => {
            eprintln!("invalid track branch: {err}");
            return ExitCode::FAILURE;
        }
    };

    let project_root = match resolve_project_root(&items_dir) {
        Ok(path) => path,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::FAILURE;
        }
    };

    let repo = match SystemGitRepo::discover() {
        Ok(repo) => repo,
        Err(err) => {
            eprintln!("failed to discover git repository: {err}");
            return ExitCode::FAILURE;
        }
    };

    let lock_manager = match FsFileLockManager::new(&locks_dir) {
        Ok(lm) => Arc::new(lm),
        Err(err) => {
            eprintln!("failed to initialize lock manager: {err}");
            return ExitCode::FAILURE;
        }
    };
    let store = Arc::new(FsTrackStore::new(items_dir.clone(), lock_manager, DEFAULT_LOCK_TIMEOUT));
    let activation = ActivateTrackUseCase::new(Arc::clone(&store));

    let track_record = match load_track_branch_record(&project_root, &items_dir, &track_id) {
        Ok(record) => record,
        Err(err) => {
            eprintln!("activation failed: {err}");
            return ExitCode::FAILURE;
        }
    };

    if uses_legacy_branch_mode(mode, track_record.schema_version) {
        return execute_legacy_branch_mode(&repo, &branch_name, mode);
    }

    let already_materialized = track_record.branch.is_some();
    let current_branch = match repo.current_branch() {
        Ok(branch) => branch,
        Err(err) => {
            eprintln!("failed to determine current branch before activation: {err}");
            return ExitCode::FAILURE;
        }
    };
    if !already_materialized && current_branch.as_deref() == Some(branch_name.as_str()) {
        eprintln!(
            "activation preflight failed: branch '{branch_name}' is already checked out; rerun /track:activate from a non-track branch so materialized metadata is committed before switching"
        );
        return ExitCode::FAILURE;
    }
    if activation_rejects_invalid_source_branch(
        mode,
        already_materialized,
        current_branch.as_deref(),
    ) {
        eprintln!(
            "activation preflight failed: activation must start from a non-track source branch; switch to 'main' or another non-track branch and rerun"
        );
        return ExitCode::FAILURE;
    }
    if activation_create_requires_main_branch(mode, already_materialized, current_branch.as_deref())
    {
        eprintln!(
            "activation preflight failed: track branch creation must start from 'main'; switch to main or use /track:activate {track_id} instead"
        );
        return ExitCode::FAILURE;
    }

    let resume_allowed = if already_materialized && mode == BranchMode::Auto {
        match activation_resume_allowed(
            &repo,
            &project_root,
            &items_dir,
            &track_id,
            &branch_name,
            track_record.status.as_deref().unwrap_or("planned"),
            current_branch.as_deref(),
        ) {
            Ok(allowed) => allowed,
            Err(err) => {
                eprintln!("activation preflight failed: {err}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        false
    };

    if already_materialized && !allow_materialized_activation(mode, resume_allowed) {
        eprintln!(
            "activation failed: track '{track_id}' is already materialized on branch '{branch_name}'; use that branch directly instead of rerunning /track:activate"
        );
        return ExitCode::FAILURE;
    }

    let should_persist_side_effects =
        should_persist_activation_side_effects(mode, already_materialized, resume_allowed);

    let allowed_dirty_paths = allowed_activation_dirty_paths(
        &project_root,
        &items_dir,
        &track_id,
        mode,
        already_materialized,
        resume_allowed,
    );
    if activation_requires_clean_worktree(mode, already_materialized, resume_allowed) {
        if let Err(err) = ensure_clean_worktree(&repo, &allowed_dirty_paths) {
            eprintln!("activation preflight failed: {err}");
            return ExitCode::FAILURE;
        }
    }

    let branch_exists =
        match preflight_branch_operation(&repo, &branch_name, mode, !already_materialized) {
            Ok(exists) => exists,
            Err(err) => {
                eprintln!("activation preflight failed: {err}");
                return ExitCode::FAILURE;
            }
        };

    let materialized_now = if already_materialized {
        false
    } else {
        match activation.execute(&track_id, &branch, track_record.schema_version) {
            Ok(ActivateTrackOutcome::Materialized(_)) => true,
            Err(err) => {
                eprintln!("activation failed: {err}");
                return ExitCode::FAILURE;
            }
        }
    };

    let created_activation_commit = if should_persist_side_effects {
        let rendered_paths =
            match render::sync_rendered_views(&project_root, Some(track_id.as_str())) {
                Ok(changed) => {
                    for path in &changed {
                        match path.strip_prefix(&project_root) {
                            Ok(relative) => println!("[OK] Rendered: {}", relative.display()),
                            Err(_) => println!("[OK] Rendered: {}", path.display()),
                        }
                    }
                    changed
                }
                Err(err) => {
                    eprintln!("activation persisted but sync-views failed: {err}");
                    return ExitCode::FAILURE;
                }
            };

        match persist_activation_commit(
            &repo,
            &project_root,
            &items_dir,
            &track_id,
            &rendered_paths,
        ) {
            Ok(created) => created,
            Err(err) => {
                eprintln!("activation persisted but activation commit failed: {err}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        false
    };
    let resume_marker_present = activation_resume_marker_exists(&project_root, &track_id);
    let resume_marker_armed =
        if mode == BranchMode::Auto && (materialized_now || created_activation_commit) {
            if let Err(err) = write_activation_resume_marker(&project_root, &track_id) {
                eprintln!("activation failed: {err}");
                return ExitCode::FAILURE;
            }
            true
        } else {
            false
        };

    let create_from = match activation_branch_create_base(
        &repo,
        &track_id,
        &branch_name,
        mode,
        branch_exists,
        materialized_now,
    ) {
        Ok(base) => base,
        Err(err) => {
            eprintln!("activation preflight failed: {err}");
            return ExitCode::FAILURE;
        }
    };
    let git_commands = activation_git_commands(
        mode,
        &branch_name,
        branch_exists,
        materialized_now,
        current_branch.as_deref(),
        create_from.as_deref(),
    );
    for command in &git_commands {
        let args = command.iter().map(String::as_str).collect::<Vec<_>>();
        match repo.status(&args) {
            Ok(0) => {}
            Ok(_) => {
                eprintln!(
                    "git {} failed after metadata materialization; rerun `cargo run --quiet -p cli -- track activate {track_id}` to resume",
                    args.join(" ")
                );
                return ExitCode::FAILURE;
            }
            Err(err) => {
                eprintln!(
                    "failed to run git {} after metadata materialization: {err}. rerun `cargo run --quiet -p cli -- track activate {track_id}` to resume",
                    args.join(" ")
                );
                return ExitCode::FAILURE;
            }
        }
    }
    if mode == BranchMode::Auto && (resume_marker_present || resume_marker_armed) {
        if let Err(err) = clear_activation_resume_marker(&project_root, &track_id) {
            eprintln!("activation succeeded but cleanup failed: {err}");
            return ExitCode::FAILURE;
        }
    }

    if materialized_now {
        println!("[OK] Materialized branch metadata: {branch_name}");
    } else {
        println!("[OK] Branch metadata already materialized: {branch_name}");
    }
    if created_activation_commit {
        println!("[OK] Created activation commit");
    }
    println!(
        "[OK] {} branch: {}",
        activation_switch_label(
            mode,
            branch_exists,
            current_branch.as_deref() == Some(branch_name.as_str())
        ),
        branch_name
    );
    ExitCode::SUCCESS
}

fn ensure_clean_worktree(
    repo: &impl GitRepository,
    allowed_dirty_paths: &std::collections::BTreeSet<String>,
) -> Result<(), String> {
    let dirty_paths = dirty_worktree_paths(repo)?;
    if dirty_paths.is_empty() {
        return Ok(());
    }
    if dirty_paths.iter().all(|path| allowed_dirty_paths.contains(path)) {
        return Ok(());
    }
    if !dirty_paths.is_empty() {
        return Err(
            "activation requires a clean worktree before metadata materialization".to_owned()
        );
    }
    Ok(())
}

fn activation_create_requires_main_branch(
    mode: BranchMode,
    already_materialized: bool,
    current_branch: Option<&str>,
) -> bool {
    mode == BranchMode::Create && !already_materialized && current_branch != Some("main")
}

fn activation_rejects_invalid_source_branch(
    mode: BranchMode,
    already_materialized: bool,
    current_branch: Option<&str>,
) -> bool {
    let invalid = current_branch.is_none()
        || current_branch == Some("HEAD")
        || current_branch.is_some_and(|branch| branch.starts_with("track/"));

    match mode {
        BranchMode::Auto => invalid,
        BranchMode::Switch => !already_materialized && invalid,
        BranchMode::Create => false,
    }
}

fn uses_legacy_branch_mode(mode: BranchMode, schema_version: u32) -> bool {
    schema_version != 3 && !matches!(mode, BranchMode::Auto)
}

fn execute_legacy_branch_mode(
    repo: &impl GitRepository,
    branch_name: &str,
    mode: BranchMode,
) -> ExitCode {
    let branch_exists = match preflight_branch_operation(repo, branch_name, mode, false) {
        Ok(exists) => exists,
        Err(err) => {
            eprintln!("legacy branch preflight failed: {err}");
            return ExitCode::FAILURE;
        }
    };
    let current_branch = match repo.current_branch() {
        Ok(branch) => branch,
        Err(err) => {
            eprintln!("failed to determine current branch: {err}");
            return ExitCode::FAILURE;
        }
    };
    let create_from = matches!(mode, BranchMode::Create).then_some("main");
    let git_commands = activation_git_commands(
        mode,
        branch_name,
        branch_exists,
        false,
        current_branch.as_deref(),
        create_from,
    );
    for command in &git_commands {
        let args = command.iter().map(String::as_str).collect::<Vec<_>>();
        match repo.status(&args) {
            Ok(0) => {}
            Ok(_) => {
                eprintln!("git {} failed", args.join(" "));
                return ExitCode::FAILURE;
            }
            Err(err) => {
                eprintln!("failed to run git {}: {err}", args.join(" "));
                return ExitCode::FAILURE;
            }
        }
    }

    println!("[OK] Legacy track left branch metadata unchanged");
    println!(
        "[OK] {} branch: {}",
        activation_switch_label(
            mode,
            branch_exists,
            current_branch.as_deref() == Some(branch_name)
        ),
        branch_name
    );
    ExitCode::SUCCESS
}

fn load_track_branch_record(
    project_root: &std::path::Path,
    items_dir: &std::path::Path,
    track_id: &TrackId,
) -> Result<TrackBranchRecord, String> {
    let track_dir = items_dir.join(track_id.as_str());
    load_explicit_track_branch_from_items_dir(project_root, items_dir, &track_dir)
}

fn persist_activation_commit(
    repo: &impl GitRepository,
    project_root: &std::path::Path,
    items_dir: &std::path::Path,
    track_id: &TrackId,
    rendered_paths: &[PathBuf],
) -> Result<bool, String> {
    let metadata_path = items_dir
        .join(track_id.as_str())
        .join("metadata.json")
        .strip_prefix(project_root)
        .unwrap_or(&items_dir.join(track_id.as_str()).join("metadata.json"))
        .display()
        .to_string();
    let mut staged = std::collections::BTreeSet::from([metadata_path]);
    for path in rendered_paths {
        let relative = path.strip_prefix(project_root).unwrap_or(path);
        staged.insert(relative.display().to_string());
    }
    let staged_paths = staged.into_iter().collect::<Vec<_>>();

    let mut status_args = vec!["status".to_owned(), "--porcelain".to_owned(), "--".to_owned()];
    status_args.extend(staged_paths.iter().cloned());
    let status_refs = status_args.iter().map(String::as_str).collect::<Vec<_>>();
    let status_output = repo.output(&status_refs)?;
    if !status_output.status.success() {
        return Err("git status failed while checking activation artifacts".to_owned());
    }
    if String::from_utf8_lossy(&status_output.stdout).trim().is_empty() {
        return Ok(false);
    }

    let mut add_args = vec!["add".to_owned(), "--".to_owned()];
    add_args.extend(staged_paths.iter().cloned());
    let add_refs = add_args.iter().map(String::as_str).collect::<Vec<_>>();
    if repo.status(&add_refs)? != 0 {
        return Err("git add failed while preparing activation commit".to_owned());
    }

    let message = format!("track: activate {track_id}");
    let mut commit_args =
        vec!["commit".to_owned(), "-m".to_owned(), message, "--only".to_owned(), "--".to_owned()];
    commit_args.extend(staged_paths);
    let commit_refs = commit_args.iter().map(String::as_str).collect::<Vec<_>>();
    if repo.status(&commit_refs)? != 0 {
        return Err("git commit failed while persisting activation materialization".to_owned());
    }

    Ok(true)
}

fn activation_resume_marker_path(
    project_root: &std::path::Path,
    track_id: &TrackId,
) -> std::path::PathBuf {
    project_root.join("tmp/track-activate").join(format!("{}.pending", track_id.as_str()))
}

fn activation_resume_marker_exists(project_root: &std::path::Path, track_id: &TrackId) -> bool {
    activation_resume_marker_path(project_root, track_id).is_file()
}

fn write_activation_resume_marker(
    project_root: &std::path::Path,
    track_id: &TrackId,
) -> Result<(), String> {
    let marker_path = activation_resume_marker_path(project_root, track_id);
    if let Some(parent) = marker_path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            format!("failed to prepare activation resume marker directory: {err}")
        })?;
    }
    std::fs::write(&marker_path, b"pending\n")
        .map_err(|err| format!("failed to write activation resume marker: {err}"))
}

fn clear_activation_resume_marker(
    project_root: &std::path::Path,
    track_id: &TrackId,
) -> Result<(), String> {
    let marker_path = activation_resume_marker_path(project_root, track_id);
    match std::fs::remove_file(&marker_path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(format!("failed to clear activation resume marker: {err}")),
    }
}

fn activation_git_commands(
    mode: BranchMode,
    branch_name: &str,
    branch_exists: bool,
    materialized_now: bool,
    current_branch: Option<&str>,
    create_from: Option<&str>,
) -> Vec<Vec<String>> {
    if current_branch == Some(branch_name) {
        return Vec::new();
    }

    match (mode, branch_exists, materialized_now) {
        (BranchMode::Create, _, _) => vec![vec![
            "switch".to_owned(),
            "-c".to_owned(),
            branch_name.to_owned(),
            create_from.unwrap_or("main").to_owned(),
        ]],
        (BranchMode::Switch, _, true) => vec![
            vec!["branch".to_owned(), "-f".to_owned(), branch_name.to_owned(), "HEAD".to_owned()],
            vec!["switch".to_owned(), branch_name.to_owned()],
        ],
        (BranchMode::Auto, true, true) => vec![
            vec!["branch".to_owned(), "-f".to_owned(), branch_name.to_owned(), "HEAD".to_owned()],
            vec!["switch".to_owned(), branch_name.to_owned()],
        ],
        (BranchMode::Auto, true, false) if create_from.is_some() => vec![
            vec![
                "branch".to_owned(),
                "-f".to_owned(),
                branch_name.to_owned(),
                create_from.unwrap_or("HEAD").to_owned(),
            ],
            vec!["switch".to_owned(), branch_name.to_owned()],
        ],
        (BranchMode::Switch, _, false) | (BranchMode::Auto, true, false) => {
            vec![vec!["switch".to_owned(), branch_name.to_owned()]]
        }
        (BranchMode::Auto, false, _) => vec![vec![
            "switch".to_owned(),
            "-c".to_owned(),
            branch_name.to_owned(),
            create_from.unwrap_or("HEAD").to_owned(),
        ]],
    }
}

fn activation_branch_create_base(
    repo: &impl GitRepository,
    track_id: &TrackId,
    branch_name: &str,
    mode: BranchMode,
    branch_exists: bool,
    materialized_now: bool,
) -> Result<Option<String>, String> {
    match mode {
        BranchMode::Create => Ok((!branch_exists).then_some("main".to_owned())),
        BranchMode::Switch => Ok(None),
        BranchMode::Auto if materialized_now => {
            if branch_exists {
                Ok(None)
            } else {
                Ok(Some("HEAD".to_owned()))
            }
        }
        BranchMode::Auto => {
            let Some(commit) = find_latest_activation_commit(repo, track_id)? else {
                return if branch_exists {
                    Ok(None)
                } else {
                    Err(format!(
                        "branch 'track/{track_id}' is missing after activation; cannot resume safely without an activation commit"
                    ))
                };
            };

            if !branch_exists {
                return Ok(Some(commit));
            }

            let branch_head = rev_parse_oid(repo, branch_name)?
                .ok_or_else(|| format!("cannot resolve existing branch '{branch_name}'"))?;
            if branch_head == commit {
                return Ok(None);
            }
            if is_ancestor(repo, &branch_head, &commit)? {
                return Ok(Some(commit));
            }
            if is_ancestor(repo, &commit, &branch_head)? {
                return Ok(None);
            }

            Err(format!(
                "branch '{branch_name}' diverged from activation commit; cannot resume safely"
            ))
        }
    }
}

fn activation_resume_allowed(
    repo: &impl GitRepository,
    project_root: &std::path::Path,
    items_dir: &std::path::Path,
    track_id: &TrackId,
    branch_name: &str,
    status: &str,
    current_branch: Option<&str>,
) -> Result<bool, String> {
    if current_branch == Some(branch_name) || status != "planned" {
        return Ok(false);
    }

    if activation_resume_marker_exists(project_root, track_id) {
        return Ok(true);
    }

    if find_latest_activation_commit(repo, track_id)?.is_some() {
        return Ok(false);
    }

    activation_artifacts_dirty(repo, project_root, items_dir, track_id)
}

fn allow_materialized_activation(mode: BranchMode, resume_allowed: bool) -> bool {
    match mode {
        BranchMode::Auto => resume_allowed,
        BranchMode::Switch => true,
        BranchMode::Create => true,
    }
}

fn should_persist_activation_side_effects(
    mode: BranchMode,
    already_materialized: bool,
    resume_allowed: bool,
) -> bool {
    if !already_materialized {
        return true;
    }
    matches!(mode, BranchMode::Auto) && resume_allowed
}

fn activation_requires_clean_worktree(
    mode: BranchMode,
    already_materialized: bool,
    resume_allowed: bool,
) -> bool {
    if !already_materialized {
        return true;
    }
    matches!(mode, BranchMode::Auto) && resume_allowed
}

fn allowed_activation_dirty_paths(
    project_root: &std::path::Path,
    items_dir: &std::path::Path,
    track_id: &TrackId,
    mode: BranchMode,
    already_materialized: bool,
    resume_allowed: bool,
) -> std::collections::BTreeSet<String> {
    if matches!(mode, BranchMode::Auto) && already_materialized && resume_allowed {
        activation_artifact_paths(project_root, items_dir, track_id)
    } else {
        std::collections::BTreeSet::new()
    }
}

fn activation_artifact_paths(
    project_root: &std::path::Path,
    items_dir: &std::path::Path,
    track_id: &TrackId,
) -> std::collections::BTreeSet<String> {
    let metadata_path = items_dir
        .join(track_id.as_str())
        .join("metadata.json")
        .strip_prefix(project_root)
        .unwrap_or(&items_dir.join(track_id.as_str()).join("metadata.json"))
        .display()
        .to_string();
    let plan_path = items_dir
        .join(track_id.as_str())
        .join("plan.md")
        .strip_prefix(project_root)
        .unwrap_or(&items_dir.join(track_id.as_str()).join("plan.md"))
        .display()
        .to_string();
    std::collections::BTreeSet::from([metadata_path, plan_path, "track/registry.md".to_owned()])
}

fn dirty_worktree_paths(repo: &impl GitRepository) -> Result<Vec<String>, String> {
    let output = repo.output(&["status", "--porcelain"])?;
    if !output.status.success() {
        return Err("git status --porcelain failed".to_owned());
    }
    let mut paths = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        if line.len() < 4 {
            continue;
        }
        let path = &line[3..];
        let normalized = path.split_once(" -> ").map(|(_, after)| after).unwrap_or(path).trim();
        if !normalized.is_empty() {
            paths.push(normalized.to_owned());
        }
    }
    Ok(paths)
}

fn activation_artifacts_dirty(
    repo: &impl GitRepository,
    project_root: &std::path::Path,
    items_dir: &std::path::Path,
    track_id: &TrackId,
) -> Result<bool, String> {
    let artifact_paths = activation_artifact_paths(project_root, items_dir, track_id);
    let dirty_paths = dirty_worktree_paths(repo)?;
    Ok(dirty_paths.iter().any(|path| artifact_paths.contains(path)))
}

fn find_latest_activation_commit(
    repo: &impl GitRepository,
    track_id: &TrackId,
) -> Result<Option<String>, String> {
    let message = format!("^track: activate {track_id}$");
    let output = repo.output(&["log", "-n", "1", "--format=%H", "--grep", message.as_str()])?;
    if !output.status.success() {
        return Err("git log failed while locating activation commit".to_owned());
    }

    let commit = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if commit.is_empty() { Ok(None) } else { Ok(Some(commit)) }
}

fn is_ancestor(
    repo: &impl GitRepository,
    ancestor: &str,
    descendant: &str,
) -> Result<bool, String> {
    let output = repo.output(&["merge-base", "--is-ancestor", ancestor, descendant])?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        Some(code) => Err(format!(
            "git merge-base --is-ancestor failed while comparing '{ancestor}' and '{descendant}' (exit {code})"
        )),
        None => Err("git merge-base --is-ancestor terminated by signal".to_owned()),
    }
}

fn activation_switch_label(
    mode: BranchMode,
    branch_exists: bool,
    already_on_branch: bool,
) -> &'static str {
    if already_on_branch {
        return "Already on";
    }

    match mode {
        BranchMode::Create => "Created and switched to",
        BranchMode::Switch => "Switched to",
        BranchMode::Auto if branch_exists => "Switched to",
        BranchMode::Auto => "Created and switched to",
    }
}

fn reject_branchless_implementation_transition(
    project_root: &std::path::Path,
    items_dir: &std::path::Path,
    track_id: &TrackId,
    target_status: &str,
) -> Result<(), String> {
    if !matches!(target_status, "in_progress" | "done" | "skipped") {
        return Ok(());
    }

    let track = load_track_branch_record(project_root, items_dir, track_id)?;
    if track.schema_version == 3 && track.branch.is_none() {
        return Err(format!(
            "track '{track_id}' is not activated yet; run /track:activate {track_id}"
        ));
    }

    Ok(())
}

fn branch_exists(repo: &impl GitRepository, branch_name: &str) -> Result<bool, String> {
    let output = repo.output(&["rev-parse", "--verify", "--quiet", branch_name])?;
    Ok(output.status.success())
}

fn rev_parse_oid(repo: &impl GitRepository, rev: &str) -> Result<Option<String>, String> {
    let spec = format!("{rev}^{{commit}}");
    let output = repo.output(&["rev-parse", "--verify", "--quiet", spec.as_str()])?;
    if !output.status.success() {
        return Ok(None);
    }
    Ok(Some(String::from_utf8_lossy(&output.stdout).trim().to_owned()))
}

fn reject_stale_or_divergent_branch(
    repo: &impl GitRepository,
    branch_name: &str,
    exists: bool,
) -> Result<(), String> {
    if !exists {
        return Ok(());
    }

    if repo.current_branch()?.as_deref() == Some(branch_name) {
        return Ok(());
    }

    let current_head = rev_parse_oid(repo, "HEAD")?
        .ok_or_else(|| "cannot resolve current HEAD for activation preflight".to_owned())?;
    let branch_head = rev_parse_oid(repo, branch_name)?
        .ok_or_else(|| format!("cannot resolve existing branch '{branch_name}'"))?;

    if current_head != branch_head {
        return Err(format!(
            "branch '{branch_name}' exists but does not point at the current HEAD; refuse to activate onto a stale/divergent branch"
        ));
    }

    Ok(())
}

fn preflight_branch_operation(
    repo: &impl GitRepository,
    branch_name: &str,
    mode: BranchMode,
    require_alignment: bool,
) -> Result<bool, String> {
    let exists = branch_exists(repo, branch_name)?;
    if require_alignment {
        reject_stale_or_divergent_branch(repo, branch_name, exists)?;
    }
    match mode {
        BranchMode::Create if exists => Err(format!("branch '{branch_name}' already exists")),
        BranchMode::Switch if !exists => Err(format!("branch '{branch_name}' does not exist")),
        _ => Ok(exists),
    }
}

fn current_git_branch(cwd: &std::path::Path) -> Result<String, String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(cwd)
        .output()
        .map_err(|e| format!("failed to run git: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git rev-parse failed: {stderr}"));
    }

    let branch = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    Ok(branch)
}

/// Verifies that the current git branch matches the track's expected branch.
///
/// Skip policy:
/// - branch=None in metadata → skip (legacy/planning phase)
/// - detached HEAD (`"HEAD"`) → reject (ambiguous state)
/// - mismatch → reject
///
/// # Errors
/// Returns an error message describing the branch mismatch or detection failure.
fn verify_branch_guard<R: TrackReader>(
    reader: &R,
    track_id: &TrackId,
    repo_dir: &std::path::Path,
) -> Result<(), String> {
    let track = reader
        .find(track_id)
        .map_err(|e| format!("failed to read track: {e}"))?
        .ok_or_else(|| format!("track '{track_id}' not found"))?;

    let expected_branch = match track.branch() {
        Some(branch) => branch,
        None => return Ok(()), // branch=null → skip guard
    };

    let actual = current_git_branch(repo_dir)?;

    // Detached HEAD → reject (ambiguous state).
    if actual == "HEAD" {
        return Err(format!("detached HEAD — expected branch '{expected_branch}', cannot verify"));
    }

    if actual != expected_branch.as_str() {
        return Err(format!(
            "current branch '{actual}' does not match expected '{expected_branch}'"
        ));
    }

    Ok(())
}

/// Resolves the correct `TaskTransition` based on target status and current task status.
/// This handles cases like `done -> in_progress` (Reopen) vs `todo -> in_progress` (Start).
fn resolve_transition(
    target_status: &str,
    current_kind: TaskStatusKind,
    commit_hash: Option<CommitHash>,
) -> Result<TaskTransition, String> {
    match target_status {
        "in_progress" => match current_kind {
            TaskStatusKind::Done => Ok(TaskTransition::Reopen),
            _ => Ok(TaskTransition::Start),
        },
        "done" => Ok(TaskTransition::Complete { commit_hash }),
        "todo" => Ok(TaskTransition::ResetToTodo),
        "skipped" => Ok(TaskTransition::Skip),
        other => Err(format!("unsupported target status: {other}")),
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::collections::HashMap;
    use std::fs;
    use std::os::unix::process::ExitStatusExt;
    use std::process::Output;
    use std::sync::Mutex;

    use domain::TrackId;
    use infrastructure::git_cli::GitRepository;

    use super::{
        BranchMode, activation_branch_create_base, activation_create_requires_main_branch,
        activation_git_commands, activation_rejects_invalid_source_branch,
        activation_requires_clean_worktree, activation_resume_allowed,
        activation_resume_marker_path, allow_materialized_activation,
        allowed_activation_dirty_paths, clear_activation_resume_marker, ensure_clean_worktree,
        load_track_branch_record, persist_activation_commit, preflight_branch_operation,
        reject_branchless_implementation_transition, resolve_project_root,
        should_persist_activation_side_effects, uses_legacy_branch_mode,
        write_activation_resume_marker,
    };
    use std::path::Path;

    struct StubRepo {
        current_branch: Option<String>,
        outputs: HashMap<Vec<String>, Output>,
    }

    impl GitRepository for StubRepo {
        fn root(&self) -> &Path {
            Path::new(".")
        }

        fn status(&self, _args: &[&str]) -> Result<i32, String> {
            Ok(0)
        }

        fn output(&self, args: &[&str]) -> Result<Output, String> {
            self.outputs
                .get(&args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>())
                .cloned()
                .ok_or_else(|| format!("unexpected git args: {}", args.join(" ")))
        }

        fn current_branch(&self) -> Result<Option<String>, String> {
            Ok(self.current_branch.clone())
        }
    }

    struct RecordingRepo {
        current_branch: Option<String>,
        outputs: HashMap<Vec<String>, Output>,
        status_calls: Mutex<Vec<Vec<String>>>,
    }

    impl GitRepository for RecordingRepo {
        fn root(&self) -> &Path {
            Path::new(".")
        }

        fn status(&self, args: &[&str]) -> Result<i32, String> {
            self.status_calls
                .lock()
                .unwrap()
                .push(args.iter().map(|arg| (*arg).to_owned()).collect());
            Ok(0)
        }

        fn output(&self, args: &[&str]) -> Result<Output, String> {
            self.outputs
                .get(&args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>())
                .cloned()
                .ok_or_else(|| format!("unexpected git args: {}", args.join(" ")))
        }

        fn current_branch(&self) -> Result<Option<String>, String> {
            Ok(self.current_branch.clone())
        }
    }

    fn write_track_metadata(
        root: &Path,
        schema_version: u32,
        branch: Option<&str>,
    ) -> std::path::PathBuf {
        let track_dir = root.join("track/items/demo");
        fs::create_dir_all(&track_dir).unwrap();
        let branch_json = match branch {
            Some(branch) => format!(r#""{branch}""#),
            None => "null".to_owned(),
        };
        fs::write(
            track_dir.join("metadata.json"),
            format!(
                r#"{{
  "schema_version": {schema_version},
  "id": "demo",
  "branch": {branch_json},
  "title": "Demo",
  "status": "planned",
  "created_at": "2026-03-14T00:00:00Z",
  "updated_at": "2026-03-14T00:00:00Z",
  "tasks": [
    {{
      "id": "T1",
      "description": "Implement activation guard",
      "status": "todo"
    }}
  ],
  "plan": {{
    "summary": [],
    "sections": [
      {{
        "id": "S1",
        "title": "Build",
        "description": [],
        "task_ids": ["T1"]
      }}
    ]
  }}
}}
"#
            ),
        )
        .unwrap();
        track_dir
    }

    fn success_output(stdout: &str) -> Output {
        Output {
            status: std::process::ExitStatus::from_raw(0),
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        }
    }

    fn exit_output(code: i32, stdout: &str) -> Output {
        Output {
            status: std::process::ExitStatus::from_raw(code << 8),
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        }
    }

    #[test]
    fn resolve_project_root_accepts_standard_track_items_layout() {
        assert_eq!(
            resolve_project_root(Path::new("repo/track/items")),
            Ok(std::path::PathBuf::from("repo"))
        );
    }

    #[test]
    fn resolve_project_root_rejects_non_standard_layout() {
        assert!(matches!(
            resolve_project_root(Path::new("repo/custom-items")),
            Err(err) if err.contains("track/items")
        ));
    }

    #[test]
    fn reject_branchless_implementation_transition_blocks_planning_only_tracks() {
        let dir = tempfile::tempdir().unwrap();
        write_track_metadata(dir.path(), 3, None);

        let err = reject_branchless_implementation_transition(
            dir.path(),
            &dir.path().join("track/items"),
            &TrackId::new("demo").unwrap(),
            "in_progress",
        )
        .unwrap_err();

        assert!(err.contains("/track:activate demo"));
    }

    #[test]
    fn reject_branchless_implementation_transition_allows_materialized_tracks() {
        let dir = tempfile::tempdir().unwrap();
        write_track_metadata(dir.path(), 3, Some("track/demo"));

        let result = reject_branchless_implementation_transition(
            dir.path(),
            &dir.path().join("track/items"),
            &TrackId::new("demo").unwrap(),
            "in_progress",
        );

        assert!(result.is_ok());
    }

    #[test]
    fn reject_branchless_implementation_transition_allows_legacy_v2_branchless_tracks() {
        let dir = tempfile::tempdir().unwrap();
        write_track_metadata(dir.path(), 2, None);

        let result = reject_branchless_implementation_transition(
            dir.path(),
            &dir.path().join("track/items"),
            &TrackId::new("demo").unwrap(),
            "in_progress",
        );

        assert!(result.is_ok());
    }

    #[test]
    fn uses_legacy_branch_mode_only_for_non_auto_v2_paths() {
        assert!(uses_legacy_branch_mode(BranchMode::Create, 2));
        assert!(uses_legacy_branch_mode(BranchMode::Switch, 2));
        assert!(!uses_legacy_branch_mode(BranchMode::Auto, 2));
        assert!(!uses_legacy_branch_mode(BranchMode::Create, 3));
    }

    #[test]
    fn activation_git_commands_fast_forward_existing_branch_after_materialization() {
        let commands =
            activation_git_commands(BranchMode::Auto, "track/demo", true, true, Some("main"), None);

        assert_eq!(
            commands,
            vec![
                vec![
                    "branch".to_owned(),
                    "-f".to_owned(),
                    "track/demo".to_owned(),
                    "HEAD".to_owned(),
                ],
                vec!["switch".to_owned(), "track/demo".to_owned()],
            ]
        );
    }

    #[test]
    fn activation_git_commands_switch_fast_forwards_existing_branch_after_materialization() {
        let commands = activation_git_commands(
            BranchMode::Switch,
            "track/demo",
            true,
            true,
            Some("main"),
            None,
        );

        assert_eq!(
            commands,
            vec![
                vec![
                    "branch".to_owned(),
                    "-f".to_owned(),
                    "track/demo".to_owned(),
                    "HEAD".to_owned(),
                ],
                vec!["switch".to_owned(), "track/demo".to_owned()],
            ]
        );
    }

    #[test]
    fn activation_git_commands_switch_existing_materialized_branch_switches_only() {
        let commands = activation_git_commands(
            BranchMode::Switch,
            "track/demo",
            true,
            false,
            Some("main"),
            None,
        );

        assert_eq!(commands, vec![vec!["switch".to_owned(), "track/demo".to_owned()]]);
    }

    #[test]
    fn activation_resume_allowed_only_for_planned_materialization_commit_on_non_track_branch() {
        let dir = tempfile::tempdir().unwrap();
        let track_id = TrackId::new("demo").unwrap();
        write_activation_resume_marker(dir.path(), &track_id).unwrap();
        let repo = StubRepo { current_branch: Some("main".to_owned()), outputs: HashMap::new() };

        assert!(
            activation_resume_allowed(
                &repo,
                dir.path(),
                &dir.path().join("track/items"),
                &track_id,
                "track/demo",
                "planned",
                Some("main"),
            )
            .unwrap()
        );
        assert!(
            !activation_resume_allowed(
                &repo,
                dir.path(),
                &dir.path().join("track/items"),
                &track_id,
                "track/demo",
                "in_progress",
                Some("main"),
            )
            .unwrap()
        );
        assert!(
            !activation_resume_allowed(
                &repo,
                dir.path(),
                &dir.path().join("track/items"),
                &track_id,
                "track/demo",
                "planned",
                Some("track/demo"),
            )
            .unwrap()
        );
    }

    #[test]
    fn activation_resume_allowed_when_head_has_advanced_past_activation_commit() {
        let dir = tempfile::tempdir().unwrap();
        let track_id = TrackId::new("demo").unwrap();
        write_activation_resume_marker(dir.path(), &track_id).unwrap();
        let repo = StubRepo { current_branch: Some("main".to_owned()), outputs: HashMap::new() };

        assert!(
            activation_resume_allowed(
                &repo,
                dir.path(),
                &dir.path().join("track/items"),
                &track_id,
                "track/demo",
                "planned",
                Some("main"),
            )
            .unwrap()
        );
    }

    #[test]
    fn activation_resume_allowed_rejects_clean_existing_branch_without_resume_marker() {
        let dir = tempfile::tempdir().unwrap();
        let track_id = TrackId::new("demo").unwrap();
        let repo = StubRepo {
            current_branch: Some("main".to_owned()),
            outputs: HashMap::from([
                (
                    vec![
                        "log".to_owned(),
                        "-n".to_owned(),
                        "1".to_owned(),
                        "--format=%H".to_owned(),
                        "--grep".to_owned(),
                        "^track: activate demo$".to_owned(),
                    ],
                    success_output("abc1234\n"),
                ),
                (vec!["status".to_owned(), "--porcelain".to_owned()], success_output("")),
            ]),
        };
        assert!(
            !activation_resume_allowed(
                &repo,
                dir.path(),
                &dir.path().join("track/items"),
                &track_id,
                "track/demo",
                "planned",
                Some("main"),
            )
            .unwrap()
        );
    }

    #[test]
    fn activation_resume_allowed_when_activation_artifacts_are_dirty_without_commit() {
        let dir = tempfile::tempdir().unwrap();
        let repo = StubRepo {
            current_branch: Some("main".to_owned()),
            outputs: HashMap::from([
                (
                    vec![
                        "log".to_owned(),
                        "-n".to_owned(),
                        "1".to_owned(),
                        "--format=%H".to_owned(),
                        "--grep".to_owned(),
                        "^track: activate demo$".to_owned(),
                    ],
                    success_output(""),
                ),
                (
                    vec!["status".to_owned(), "--porcelain".to_owned()],
                    success_output(" M track/items/demo/metadata.json\n"),
                ),
            ]),
        };

        assert!(
            activation_resume_allowed(
                &repo,
                dir.path(),
                &dir.path().join("track/items"),
                &TrackId::new("demo").unwrap(),
                "track/demo",
                "planned",
                Some("main"),
            )
            .unwrap()
        );
    }

    #[test]
    fn activation_resume_marker_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let track_id = TrackId::new("demo").unwrap();

        assert!(!activation_resume_marker_path(dir.path(), &track_id).exists());
        write_activation_resume_marker(dir.path(), &track_id).unwrap();
        assert!(activation_resume_marker_path(dir.path(), &track_id).is_file());
        clear_activation_resume_marker(dir.path(), &track_id).unwrap();
        assert!(!activation_resume_marker_path(dir.path(), &track_id).exists());
    }

    #[test]
    fn allow_materialized_activation_only_allows_switch_or_auto_resume() {
        assert!(allow_materialized_activation(BranchMode::Switch, false));
        assert!(allow_materialized_activation(BranchMode::Auto, true));
        assert!(allow_materialized_activation(BranchMode::Create, false));
        assert!(!allow_materialized_activation(BranchMode::Auto, false));
    }

    #[test]
    fn activation_side_effects_skip_for_materialized_switch() {
        assert!(!should_persist_activation_side_effects(BranchMode::Switch, true, false,));
        assert!(should_persist_activation_side_effects(BranchMode::Auto, true, true,));
        assert!(should_persist_activation_side_effects(BranchMode::Auto, false, false,));
    }

    #[test]
    fn activation_resume_requires_clean_worktree() {
        assert!(activation_requires_clean_worktree(BranchMode::Auto, false, false,));
        assert!(activation_requires_clean_worktree(BranchMode::Auto, true, true,));
        assert!(!activation_requires_clean_worktree(BranchMode::Auto, true, false,));
        assert!(!activation_requires_clean_worktree(BranchMode::Switch, true, false,));
    }

    #[test]
    fn activation_auto_requires_non_track_source_branch() {
        assert!(activation_rejects_invalid_source_branch(BranchMode::Auto, false, Some("HEAD"),));
        assert!(activation_rejects_invalid_source_branch(
            BranchMode::Auto,
            false,
            Some("track/other"),
        ));
        assert!(activation_rejects_invalid_source_branch(
            BranchMode::Auto,
            true,
            Some("track/other"),
        ));
        assert!(activation_rejects_invalid_source_branch(BranchMode::Auto, false, None,));
        assert!(!activation_rejects_invalid_source_branch(BranchMode::Auto, false, Some("main"),));
    }

    #[test]
    fn activation_switch_requires_non_track_source_branch_when_materializing() {
        assert!(activation_rejects_invalid_source_branch(BranchMode::Switch, false, Some("HEAD"),));
        assert!(activation_rejects_invalid_source_branch(
            BranchMode::Switch,
            false,
            Some("track/other"),
        ));
        assert!(!activation_rejects_invalid_source_branch(
            BranchMode::Switch,
            true,
            Some("track/other"),
        ));
        assert!(!activation_rejects_invalid_source_branch(BranchMode::Switch, true, Some("main"),));
    }

    #[test]
    fn allowed_activation_dirty_paths_only_open_for_auto_resume() {
        let track_id = TrackId::new("demo").unwrap();
        assert_eq!(
            allowed_activation_dirty_paths(
                Path::new("."),
                Path::new("track/items"),
                &track_id,
                BranchMode::Auto,
                true,
                true
            ),
            std::collections::BTreeSet::from([
                "track/items/demo/metadata.json".to_owned(),
                "track/items/demo/plan.md".to_owned(),
                "track/registry.md".to_owned(),
            ])
        );
        assert!(
            allowed_activation_dirty_paths(
                Path::new("."),
                Path::new("track/items"),
                &track_id,
                BranchMode::Auto,
                false,
                false
            )
            .is_empty()
        );
        assert!(
            allowed_activation_dirty_paths(
                Path::new("."),
                Path::new("track/items"),
                &track_id,
                BranchMode::Switch,
                true,
                true
            )
            .is_empty()
        );
    }

    #[test]
    fn allowed_activation_dirty_paths_respects_items_dir() {
        let track_id = TrackId::new("demo").unwrap();
        assert_eq!(
            allowed_activation_dirty_paths(
                Path::new("."),
                Path::new("custom/track/items"),
                &track_id,
                BranchMode::Auto,
                true,
                true
            ),
            std::collections::BTreeSet::from([
                "custom/track/items/demo/metadata.json".to_owned(),
                "custom/track/items/demo/plan.md".to_owned(),
                "track/registry.md".to_owned(),
            ])
        );
    }

    #[test]
    fn ensure_clean_worktree_allows_only_activation_artifacts_for_resume() {
        let repo = StubRepo {
            current_branch: Some("main".to_owned()),
            outputs: HashMap::from([(
                vec!["status".to_owned(), "--porcelain".to_owned()],
                success_output(" M track/items/demo/metadata.json\n M track/registry.md\n"),
            )]),
        };

        let allowed = std::collections::BTreeSet::from([
            "track/items/demo/metadata.json".to_owned(),
            "track/items/demo/plan.md".to_owned(),
            "track/registry.md".to_owned(),
        ]);

        assert!(ensure_clean_worktree(&repo, &allowed).is_ok());
    }

    #[test]
    fn ensure_clean_worktree_rejects_unrelated_dirty_paths_during_resume() {
        let repo = StubRepo {
            current_branch: Some("main".to_owned()),
            outputs: HashMap::from([(
                vec!["status".to_owned(), "--porcelain".to_owned()],
                success_output(" M track/items/demo/metadata.json\n M src/lib.rs\n"),
            )]),
        };

        let allowed = std::collections::BTreeSet::from([
            "track/items/demo/metadata.json".to_owned(),
            "track/items/demo/plan.md".to_owned(),
            "track/registry.md".to_owned(),
        ]);

        let err = ensure_clean_worktree(&repo, &allowed).unwrap_err();
        assert!(err.contains("clean worktree"));
    }

    #[test]
    fn load_track_branch_record_uses_passed_items_dir() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("custom/track/items");
        let track_dir = items_dir.join("demo");
        fs::create_dir_all(&track_dir).unwrap();
        fs::write(
            track_dir.join("metadata.json"),
            r#"{"schema_version":3,"id":"demo","branch":"track/demo","title":"Demo","status":"planned","created_at":"2026-03-14T00:00:00Z","updated_at":"2026-03-14T00:00:00Z","tasks":[],"plan":{"summary":[],"sections":[]}}"#,
        )
        .unwrap();

        let record =
            load_track_branch_record(dir.path(), &items_dir, &TrackId::new("demo").unwrap())
                .unwrap();

        assert_eq!(record.display_path, "custom/track/items/demo");
    }

    #[test]
    fn activation_git_commands_noop_when_already_on_target_branch() {
        let commands = activation_git_commands(
            BranchMode::Auto,
            "track/demo",
            true,
            true,
            Some("track/demo"),
            None,
        );

        assert!(commands.is_empty());
    }

    #[test]
    fn activation_git_commands_create_track_branch_from_main() {
        let commands = activation_git_commands(
            BranchMode::Create,
            "track/demo",
            false,
            false,
            Some("feature"),
            Some("main"),
        );

        assert_eq!(
            commands,
            vec![vec![
                "switch".to_owned(),
                "-c".to_owned(),
                "track/demo".to_owned(),
                "main".to_owned(),
            ]]
        );
    }

    #[test]
    fn activation_git_commands_resume_missing_branch_from_activation_commit() {
        let commands = activation_git_commands(
            BranchMode::Auto,
            "track/demo",
            false,
            false,
            Some("main"),
            Some("abc1234"),
        );

        assert_eq!(
            commands,
            vec![vec![
                "switch".to_owned(),
                "-c".to_owned(),
                "track/demo".to_owned(),
                "abc1234".to_owned(),
            ]]
        );
    }

    #[test]
    fn activation_git_commands_realign_existing_branch_from_activation_commit_on_resume() {
        let commands = activation_git_commands(
            BranchMode::Auto,
            "track/demo",
            true,
            false,
            Some("main"),
            Some("abc1234"),
        );

        assert_eq!(
            commands,
            vec![
                vec![
                    "branch".to_owned(),
                    "-f".to_owned(),
                    "track/demo".to_owned(),
                    "abc1234".to_owned(),
                ],
                vec!["switch".to_owned(), "track/demo".to_owned()],
            ]
        );
    }

    #[test]
    fn preflight_branch_operation_rejects_existing_divergent_branch_in_auto_mode() {
        let repo = StubRepo {
            current_branch: Some("main".to_owned()),
            outputs: HashMap::from([
                (
                    vec![
                        "rev-parse".to_owned(),
                        "--verify".to_owned(),
                        "--quiet".to_owned(),
                        "track/demo".to_owned(),
                    ],
                    success_output("track/demo\n"),
                ),
                (
                    vec![
                        "rev-parse".to_owned(),
                        "--verify".to_owned(),
                        "--quiet".to_owned(),
                        "HEAD^{commit}".to_owned(),
                    ],
                    success_output("aaa\n"),
                ),
                (
                    vec![
                        "rev-parse".to_owned(),
                        "--verify".to_owned(),
                        "--quiet".to_owned(),
                        "track/demo^{commit}".to_owned(),
                    ],
                    success_output("bbb\n"),
                ),
            ]),
        };

        let err =
            preflight_branch_operation(&repo, "track/demo", BranchMode::Auto, true).unwrap_err();

        assert!(err.contains("stale/divergent"));
    }

    #[test]
    fn preflight_branch_operation_allows_existing_aligned_branch_in_auto_mode() {
        let repo = StubRepo {
            current_branch: Some("main".to_owned()),
            outputs: HashMap::from([
                (
                    vec![
                        "rev-parse".to_owned(),
                        "--verify".to_owned(),
                        "--quiet".to_owned(),
                        "track/demo".to_owned(),
                    ],
                    success_output("track/demo\n"),
                ),
                (
                    vec![
                        "rev-parse".to_owned(),
                        "--verify".to_owned(),
                        "--quiet".to_owned(),
                        "HEAD^{commit}".to_owned(),
                    ],
                    success_output("aaa\n"),
                ),
                (
                    vec![
                        "rev-parse".to_owned(),
                        "--verify".to_owned(),
                        "--quiet".to_owned(),
                        "track/demo^{commit}".to_owned(),
                    ],
                    success_output("aaa\n"),
                ),
            ]),
        };

        let result = preflight_branch_operation(&repo, "track/demo", BranchMode::Auto, true);

        assert!(result.is_ok());
    }

    #[test]
    fn preflight_branch_operation_allows_switch_to_existing_branch_with_different_head() {
        let repo = StubRepo {
            current_branch: Some("main".to_owned()),
            outputs: HashMap::from([(
                vec![
                    "rev-parse".to_owned(),
                    "--verify".to_owned(),
                    "--quiet".to_owned(),
                    "track/demo".to_owned(),
                ],
                success_output("track/demo\n"),
            )]),
        };

        let result = preflight_branch_operation(&repo, "track/demo", BranchMode::Switch, false);

        assert!(result.is_ok());
    }

    #[test]
    fn preflight_branch_operation_rejects_existing_divergent_branch_in_switch_mode_when_materializing()
     {
        let repo = StubRepo {
            current_branch: Some("main".to_owned()),
            outputs: HashMap::from([
                (
                    vec![
                        "rev-parse".to_owned(),
                        "--verify".to_owned(),
                        "--quiet".to_owned(),
                        "track/demo".to_owned(),
                    ],
                    success_output("track/demo\n"),
                ),
                (
                    vec![
                        "rev-parse".to_owned(),
                        "--verify".to_owned(),
                        "--quiet".to_owned(),
                        "HEAD^{commit}".to_owned(),
                    ],
                    success_output("aaa\n"),
                ),
                (
                    vec![
                        "rev-parse".to_owned(),
                        "--verify".to_owned(),
                        "--quiet".to_owned(),
                        "track/demo^{commit}".to_owned(),
                    ],
                    success_output("bbb\n"),
                ),
            ]),
        };

        let err =
            preflight_branch_operation(&repo, "track/demo", BranchMode::Switch, true).unwrap_err();

        assert!(err.contains("stale/divergent"));
    }

    #[test]
    fn activation_branch_create_base_uses_activation_commit_for_resume() {
        let repo = StubRepo {
            current_branch: Some("main".to_owned()),
            outputs: HashMap::from([(
                vec![
                    "log".to_owned(),
                    "-n".to_owned(),
                    "1".to_owned(),
                    "--format=%H".to_owned(),
                    "--grep".to_owned(),
                    "^track: activate demo$".to_owned(),
                ],
                success_output("abc1234\n"),
            )]),
        };

        let base = activation_branch_create_base(
            &repo,
            &TrackId::new("demo").unwrap(),
            "track/demo",
            BranchMode::Auto,
            false,
            false,
        )
        .unwrap();

        assert_eq!(base, Some("abc1234".to_owned()));
    }

    #[test]
    fn activation_create_requires_main_branch_for_new_materialization() {
        assert!(
            activation_create_requires_main_branch(BranchMode::Create, false, Some("feature"),)
        );
        assert!(!activation_create_requires_main_branch(BranchMode::Create, false, Some("main"),));
        assert!(
            !activation_create_requires_main_branch(BranchMode::Create, true, Some("feature"),)
        );
        assert!(!activation_create_requires_main_branch(BranchMode::Auto, false, Some("feature"),));
    }

    #[test]
    fn activation_branch_create_base_uses_activation_commit_to_realign_existing_branch() {
        let repo = StubRepo {
            current_branch: Some("main".to_owned()),
            outputs: HashMap::from([
                (
                    vec![
                        "log".to_owned(),
                        "-n".to_owned(),
                        "1".to_owned(),
                        "--format=%H".to_owned(),
                        "--grep".to_owned(),
                        "^track: activate demo$".to_owned(),
                    ],
                    success_output("abc1234\n"),
                ),
                (
                    vec![
                        "rev-parse".to_owned(),
                        "--verify".to_owned(),
                        "--quiet".to_owned(),
                        "track/demo^{commit}".to_owned(),
                    ],
                    success_output("old1234\n"),
                ),
                (
                    vec![
                        "merge-base".to_owned(),
                        "--is-ancestor".to_owned(),
                        "old1234".to_owned(),
                        "abc1234".to_owned(),
                    ],
                    success_output(""),
                ),
            ]),
        };

        let base = activation_branch_create_base(
            &repo,
            &TrackId::new("demo").unwrap(),
            "track/demo",
            BranchMode::Auto,
            true,
            false,
        )
        .unwrap();

        assert_eq!(base, Some("abc1234".to_owned()));
    }

    #[test]
    fn activation_branch_create_base_does_not_rewind_branch_ahead_of_activation_commit() {
        let repo = StubRepo {
            current_branch: Some("main".to_owned()),
            outputs: HashMap::from([
                (
                    vec![
                        "log".to_owned(),
                        "-n".to_owned(),
                        "1".to_owned(),
                        "--format=%H".to_owned(),
                        "--grep".to_owned(),
                        "^track: activate demo$".to_owned(),
                    ],
                    success_output("abc1234\n"),
                ),
                (
                    vec![
                        "rev-parse".to_owned(),
                        "--verify".to_owned(),
                        "--quiet".to_owned(),
                        "track/demo^{commit}".to_owned(),
                    ],
                    success_output("def5678\n"),
                ),
                (
                    vec![
                        "merge-base".to_owned(),
                        "--is-ancestor".to_owned(),
                        "def5678".to_owned(),
                        "abc1234".to_owned(),
                    ],
                    exit_output(1, ""),
                ),
                (
                    vec![
                        "merge-base".to_owned(),
                        "--is-ancestor".to_owned(),
                        "abc1234".to_owned(),
                        "def5678".to_owned(),
                    ],
                    success_output(""),
                ),
            ]),
        };

        let base = activation_branch_create_base(
            &repo,
            &TrackId::new("demo").unwrap(),
            "track/demo",
            BranchMode::Auto,
            true,
            false,
        )
        .unwrap();

        assert_eq!(base, None);
    }

    #[test]
    fn persist_activation_commit_skips_when_activation_artifacts_are_clean() {
        let repo = RecordingRepo {
            current_branch: Some("main".to_owned()),
            outputs: HashMap::from([(
                vec![
                    "status".to_owned(),
                    "--porcelain".to_owned(),
                    "--".to_owned(),
                    "track/items/demo/metadata.json".to_owned(),
                ],
                success_output(""),
            )]),
            status_calls: Mutex::new(Vec::new()),
        };

        let created = persist_activation_commit(
            &repo,
            Path::new("."),
            Path::new("track/items"),
            &TrackId::new("demo").unwrap(),
            &[],
        )
        .unwrap();

        assert!(!created);
        assert!(repo.status_calls.lock().unwrap().is_empty());
    }

    #[test]
    fn persist_activation_commit_creates_commit_when_activation_artifacts_are_dirty() {
        let repo = RecordingRepo {
            current_branch: Some("main".to_owned()),
            outputs: HashMap::from([(
                vec![
                    "status".to_owned(),
                    "--porcelain".to_owned(),
                    "--".to_owned(),
                    "track/items/demo/metadata.json".to_owned(),
                ],
                success_output(" M track/items/demo/metadata.json\n"),
            )]),
            status_calls: Mutex::new(Vec::new()),
        };

        let created = persist_activation_commit(
            &repo,
            Path::new("."),
            Path::new("track/items"),
            &TrackId::new("demo").unwrap(),
            &[],
        )
        .unwrap();

        assert!(created);
        assert_eq!(
            repo.status_calls.lock().unwrap().as_slice(),
            &[
                vec![
                    "add".to_owned(),
                    "--".to_owned(),
                    "track/items/demo/metadata.json".to_owned(),
                ],
                vec![
                    "commit".to_owned(),
                    "-m".to_owned(),
                    "track: activate demo".to_owned(),
                    "--only".to_owned(),
                    "--".to_owned(),
                    "track/items/demo/metadata.json".to_owned(),
                ],
            ]
        );
    }
}
