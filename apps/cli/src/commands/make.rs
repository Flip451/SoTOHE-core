//! Replaces Makefile.toml `script_runner = "@shell"` wrappers with safe Rust dispatch.
//!
//! Each task accepts raw arguments from `cargo make ${@}` and handles them safely
//! without shell string interpolation. The handler decides how to interpret the
//! arguments: some tasks treat them as a single value, others split into multiple
//! positional arguments.

use std::process::ExitCode;

use clap::{Args, ValueEnum};
use domain::ConfidenceSignal;
use infrastructure::tddd::type_signals_codec;
use infrastructure::track::symlink_guard;
use infrastructure::verify::tddd_layers::parse_tddd_layers;

use crate::CliError;
use crate::commands::track::tddd::signals::{
    ensure_active_track, execute_type_signals_lenient_with_bindings,
};

/// Arguments for `sotp make <task> [args...]`.
#[derive(Args)]
pub struct MakeArgs {
    /// Task to execute (replaces shell wrapper in Makefile.toml).
    #[arg(value_enum)]
    pub task: MakeTask,

    /// Raw arguments from cargo-make (`${@}`).
    ///
    /// Interpreted per-task: some tasks treat this as a single value,
    /// others split it into multiple arguments.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub raw_args: Vec<String>,
}

/// Supported make tasks. Each variant replaces a shell wrapper in Makefile.toml.
#[derive(Clone, Debug, ValueEnum)]
pub enum MakeTask {
    // Phase 2: High priority (quoting/safety)
    /// Run CI then commit with the given message.
    Commit,
    /// Attach a git note to HEAD.
    Note,
    /// Run CI then commit using tmp/track-commit/commit-message.txt.
    TrackCommitMessage,

    // Phase 3: Arg forwarding
    /// Create a track branch from main.
    TrackBranchCreate,
    /// Switch to an existing track branch.
    TrackBranchSwitch,
    /// Materialize a planning-only track and switch to its branch.
    TrackActivate,
    /// Create a plan/<track-id> branch from main.
    TrackPlanBranch,
    /// Resolve current track phase.
    TrackResolve,
    /// Push current track/plan branch to origin.
    TrackPrPush,
    /// Create or reuse a PR for the current branch.
    TrackPrEnsure,
    /// Push + ensure PR in one step.
    TrackPr,
    /// Run full PR review cycle.
    TrackPrReview,
    /// Wait for PR checks then merge.
    TrackPrMerge,
    /// Show PR check status.
    TrackPrStatus,
    /// Run the local Codex planner.
    TrackLocalPlan,
    /// Run the local Codex reviewer.
    TrackLocalReview,
    /// Show per-scope review status.
    TrackReviewStatus,
    /// Check that the review state is approved and code hash is current.
    TrackCheckApproved,
    /// Switch to main branch and pull latest.
    TrackSwitchMain,
    /// Stage paths from tmp/track-commit/add-paths.txt.
    TrackAddPaths,
    /// Transition a task status.
    TrackTransition,
    /// Add a new task to a track.
    TrackAddTask,
    /// Show the next open task (JSON).
    TrackNextTask,
    /// Show task status counts (JSON).
    TrackTaskCounts,
    /// Set or clear a status override.
    TrackSetOverride,
    /// Render plan.md and registry.md from metadata.json.
    TrackSyncViews,
    /// Attach git note from tmp/track-commit/note.md.
    TrackNote,
    /// Write current HEAD SHA to .commit_hash (set v2 diff base).
    TrackSetCommitHash,
    /// Stage all worktree changes.
    AddAll,
    /// Unstage paths (remove from index without discarding worktree changes).
    Unstage,

    // Phase 4: Exec
    /// Run a cargo make task via tools-daemon exec with WORKER_ID isolation.
    Exec,
}

/// Join raw args into a single string. Used for tasks where the entire
/// argument is a single value (e.g., commit messages, note text).
///
/// # Errors
///
/// Returns `Err` if the raw args are empty when a value is required.
pub fn raw_args_to_single(raw_args: &[String]) -> Result<String, CliError> {
    let joined = raw_args.join(" ");
    let trimmed = joined.trim();
    if trimmed.is_empty() {
        return Err(CliError::Message("missing required argument".to_owned()));
    }
    Ok(trimmed.to_owned())
}

/// Split raw args into individual words. Used for tasks that expect multiple
/// positional arguments (e.g., track-transition: <track_dir> <task_id> <status>).
///
/// When cargo-make passes `${@}` as a single string element, this function
/// splits it on whitespace. When called directly with multiple args (already
/// properly split by the shell), they are returned as-is to preserve quoting.
///
/// **Known limitation**: the cargo-make single-string path cannot preserve
/// shell quoting for multi-word values (e.g., `"fix parser bug"` becomes
/// three separate words). This is inherent to cargo-make's `${@}` expansion
/// which concatenates all args into one string. For tasks needing multi-word
/// positional args, call `bin/sotp make` directly instead of `cargo make`.
pub fn raw_args_to_words(raw_args: &[String]) -> Vec<String> {
    if raw_args.len() == 1 {
        // Single string from cargo-make `${@}` — split on whitespace
        // Safety: len() == 1 guarantees index 0 exists, but use .first() for clippy
        raw_args
            .first()
            .map(|s| s.split_whitespace().map(|w| w.to_owned()).collect())
            .unwrap_or_default()
    } else {
        // Multiple args from direct CLI invocation — already properly split
        raw_args.to_vec()
    }
}

/// Execute a make task.
pub fn execute(args: MakeArgs) -> ExitCode {
    match run(args) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("{e}");
            e.exit_code()
        }
    }
}

fn run(args: MakeArgs) -> Result<ExitCode, CliError> {
    match args.task {
        MakeTask::TrackBranchCreate => dispatch_track_branch_create(&args.raw_args),
        MakeTask::TrackBranchSwitch => dispatch_track_branch_switch(&args.raw_args),
        MakeTask::TrackActivate => dispatch_track_activate(&args.raw_args),
        MakeTask::AddAll => dispatch_add_all(),
        MakeTask::Unstage => dispatch_unstage(&args.raw_args),
        MakeTask::TrackAddPaths => dispatch_track_add_paths(),
        MakeTask::TrackNote => dispatch_track_note(),
        MakeTask::TrackSwitchMain => dispatch_track_switch_main(),
        MakeTask::TrackSyncViews => dispatch_track_sync_views(&args.raw_args),
        MakeTask::TrackResolve => dispatch_track_resolve(&args.raw_args),
        MakeTask::TrackPrReview => dispatch_track_pr_review(&args.raw_args),
        MakeTask::TrackPrPush => dispatch_track_pr_push(&args.raw_args),
        MakeTask::TrackPrEnsure => dispatch_track_pr_ensure(&args.raw_args),
        MakeTask::TrackPr => dispatch_track_pr(&args.raw_args),
        MakeTask::TrackPrMerge => dispatch_track_pr_merge(&args.raw_args),
        MakeTask::TrackPrStatus => dispatch_track_pr_status(&args.raw_args),
        MakeTask::TrackNextTask => dispatch_track_next_task(&args.raw_args),
        MakeTask::TrackTaskCounts => dispatch_track_task_counts(&args.raw_args),
        MakeTask::TrackTransition => dispatch_track_transition(&args.raw_args),
        MakeTask::TrackAddTask => dispatch_track_add_task(&args.raw_args),
        MakeTask::TrackSetOverride => dispatch_track_set_override(&args.raw_args),
        MakeTask::TrackLocalPlan => dispatch_track_local_plan(&args.raw_args),
        MakeTask::TrackLocalReview => dispatch_track_local_review(&args.raw_args),
        MakeTask::TrackReviewStatus => dispatch_track_review_status(&args.raw_args),
        MakeTask::TrackCheckApproved => dispatch_track_check_approved(&args.raw_args),
        MakeTask::TrackPlanBranch => dispatch_track_plan_branch(&args.raw_args),
        MakeTask::Commit => dispatch_commit(&args.raw_args),
        MakeTask::Note => dispatch_note(&args.raw_args),
        MakeTask::TrackCommitMessage => dispatch_track_commit_message(),
        MakeTask::TrackSetCommitHash => dispatch_set_commit_hash(&args.raw_args),
        MakeTask::Exec => dispatch_exec(&args.raw_args),
    }
}

// ---------------------------------------------------------------------------
// Dispatch helpers — delegate to existing sotp subcommands via process exec.
//
// Each function builds the correct argv and execs `bin/sotp` or the underlying
// command directly. This replaces shell string interpolation with safe Rust
// argument handling.
// ---------------------------------------------------------------------------

/// Run an external command and return its exit code.
fn run_command(program: &str, args: &[&str]) -> Result<ExitCode, CliError> {
    let status = std::process::Command::new(program).args(args).status()?;
    Ok(ExitCode::from(u8::try_from(status.code().unwrap_or(1)).unwrap_or(1)))
}

/// Run sotp binary with the given args.
fn run_sotp(args: &[&str]) -> Result<ExitCode, CliError> {
    run_command("bin/sotp", args)
}

// --- Phase 3: Arg forwarding dispatchers ---

fn dispatch_track_branch_create(raw_args: &[String]) -> Result<ExitCode, CliError> {
    let track_id = raw_args_to_single(raw_args)
        .map_err(|_| CliError::Message("error: track-id argument required".to_owned()))?;
    run_sotp(&["track", "branch", "create", "--items-dir", "track/items", &track_id])
}

fn dispatch_track_branch_switch(raw_args: &[String]) -> Result<ExitCode, CliError> {
    let track_id = raw_args_to_single(raw_args)
        .map_err(|_| CliError::Message("error: track-id argument required".to_owned()))?;
    run_sotp(&["track", "branch", "switch", "--items-dir", "track/items", &track_id])
}

fn dispatch_track_activate(raw_args: &[String]) -> Result<ExitCode, CliError> {
    let track_id = raw_args_to_single(raw_args)
        .map_err(|_| CliError::Message("error: track-id argument required".to_owned()))?;
    run_sotp(&["track", "activate", "--items-dir", "track/items", &track_id])
}

fn dispatch_track_plan_branch(raw_args: &[String]) -> Result<ExitCode, CliError> {
    let track_id = raw_args_to_single(raw_args)
        .map_err(|_| CliError::Message("error: track-id argument required".to_owned()))?;
    let branch = format!("plan/{track_id}");
    run_command("git", &["switch", "-c", &branch, "main"])
}

fn dispatch_add_all() -> Result<ExitCode, CliError> {
    run_sotp(&["git", "add-all"])
}

fn dispatch_unstage(raw_args: &[String]) -> Result<ExitCode, CliError> {
    if raw_args.is_empty() {
        return Err(CliError::Message("error: at least one path required".to_owned()));
    }
    let mut sotp_args = vec!["git", "unstage", "--"];
    sotp_args.extend(raw_args.iter().map(String::as_str));
    run_sotp(&sotp_args)
}

fn dispatch_track_add_paths() -> Result<ExitCode, CliError> {
    run_sotp(&["git", "add-from-file", "tmp/track-commit/add-paths.txt", "--cleanup"])
}

fn dispatch_track_note() -> Result<ExitCode, CliError> {
    run_sotp(&["git", "note-from-file", "tmp/track-commit/note.md", "--cleanup"])
}

fn dispatch_track_switch_main() -> Result<ExitCode, CliError> {
    run_sotp(&["git", "switch-and-pull", "main"])
}

fn dispatch_track_sync_views(raw_args: &[String]) -> Result<ExitCode, CliError> {
    let words = raw_args_to_words(raw_args);
    let mut args: Vec<&str> = vec!["track", "views", "sync", "--project-root", "."];
    for w in &words {
        args.push(w);
    }
    run_sotp(&args)
}

fn dispatch_track_resolve(raw_args: &[String]) -> Result<ExitCode, CliError> {
    let words = raw_args_to_words(raw_args);
    let mut args: Vec<&str> = vec!["track", "resolve"];
    for w in &words {
        args.push(w);
    }
    run_sotp(&args)
}

fn dispatch_track_pr_review(raw_args: &[String]) -> Result<ExitCode, CliError> {
    let words = raw_args_to_words(raw_args);
    let mut args: Vec<&str> = vec!["pr", "review-cycle"];
    for w in &words {
        args.push(w);
    }
    run_sotp(&args)
}

fn dispatch_track_pr_push(raw_args: &[String]) -> Result<ExitCode, CliError> {
    let words = raw_args_to_words(raw_args);
    let mut args: Vec<&str> = vec!["pr", "push"];
    if let Some(track_id) = words.first() {
        args.extend_from_slice(&["--track-id", track_id]);
    }
    // Forward remaining args so Clap rejects unexpected ones
    for w in words.get(1..).unwrap_or_default() {
        args.push(w);
    }
    run_sotp(&args)
}

fn dispatch_track_pr_ensure(raw_args: &[String]) -> Result<ExitCode, CliError> {
    let words = raw_args_to_words(raw_args);
    let mut args: Vec<&str> = vec!["pr", "ensure-pr"];
    if let Some(track_id) = words.first() {
        args.extend_from_slice(&["--track-id", track_id]);
    }
    // Forward remaining args so Clap rejects unexpected ones
    for w in words.get(1..).unwrap_or_default() {
        args.push(w);
    }
    run_sotp(&args)
}

fn dispatch_track_pr(raw_args: &[String]) -> Result<ExitCode, CliError> {
    // push + ensure-pr in one step
    let result = dispatch_track_pr_push(raw_args)?;
    if result != ExitCode::SUCCESS {
        return Ok(result);
    }
    dispatch_track_pr_ensure(raw_args)
}

fn dispatch_track_pr_merge(raw_args: &[String]) -> Result<ExitCode, CliError> {
    let words = raw_args_to_words(raw_args);
    let mut args: Vec<&str> = vec!["pr", "wait-and-merge"];
    for w in &words {
        args.push(w);
    }
    run_sotp(&args)
}

fn dispatch_track_pr_status(raw_args: &[String]) -> Result<ExitCode, CliError> {
    let words = raw_args_to_words(raw_args);
    let mut args: Vec<&str> = vec!["pr", "status"];
    for w in &words {
        args.push(w);
    }
    run_sotp(&args)
}

fn dispatch_track_next_task(raw_args: &[String]) -> Result<ExitCode, CliError> {
    let track_id = raw_args_to_single(raw_args).map_err(|_| {
        CliError::Message("error: usage: sotp make track-next-task <track-id>".to_owned())
    })?;
    run_sotp(&["track", "next-task", "--items-dir", "track/items", &track_id])
}

fn dispatch_track_task_counts(raw_args: &[String]) -> Result<ExitCode, CliError> {
    let track_id = raw_args_to_single(raw_args).map_err(|_| {
        CliError::Message("error: usage: sotp make track-task-counts <track-id>".to_owned())
    })?;
    run_sotp(&["track", "task-counts", "--items-dir", "track/items", &track_id])
}

fn dispatch_track_transition(raw_args: &[String]) -> Result<ExitCode, CliError> {
    let words = raw_args_to_words(raw_args);
    let usage = "error: usage: sotp make track-transition <track_dir> <task_id> <status> [--commit-hash <hash>]";
    let track_dir = words.first().ok_or_else(|| CliError::Message(usage.to_owned()))?;
    let task_id = words.get(1).ok_or_else(|| CliError::Message(usage.to_owned()))?;
    let target_status = words.get(2).ok_or_else(|| CliError::Message(usage.to_owned()))?;
    // Extract items-dir (parent) and track-id (basename) from track_dir
    let path = std::path::Path::new(track_dir.as_str());
    let items_dir = path.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    let track_id_str =
        path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    if items_dir.is_empty() || track_id_str.is_empty() {
        return Err(CliError::Message(
            "error: track_dir must be in the form <items_dir>/<track_id>".to_owned(),
        ));
    }
    let mut args: Vec<&str> = vec![
        "track",
        "transition",
        "--items-dir",
        &items_dir,
        &track_id_str,
        task_id,
        target_status,
    ];
    // Forward remaining args (e.g., --commit-hash <hash>)
    for w in words.get(3..).unwrap_or_default() {
        args.push(w);
    }
    run_sotp(&args)
}

fn dispatch_track_add_task(raw_args: &[String]) -> Result<ExitCode, CliError> {
    let words = raw_args_to_words(raw_args);
    let usage = "error: usage: sotp make track-add-task <track-id> <description> [--section <id>] [--after <task-id>]";
    let track_id = words.first().ok_or_else(|| CliError::Message(usage.to_owned()))?;
    let desc = words.get(1).ok_or_else(|| CliError::Message(usage.to_owned()))?;
    let mut args: Vec<&str> =
        vec!["track", "add-task", "--items-dir", "track/items", track_id, desc];
    for w in words.get(2..).unwrap_or_default() {
        args.push(w);
    }
    run_sotp(&args)
}

fn dispatch_track_set_override(raw_args: &[String]) -> Result<ExitCode, CliError> {
    let words = raw_args_to_words(raw_args);
    let usage = "error: usage: sotp make track-set-override <track-id> <blocked|cancelled|clear> [--reason <text>]";
    let track_id = words.first().ok_or_else(|| CliError::Message(usage.to_owned()))?;
    let status = words.get(1).ok_or_else(|| CliError::Message(usage.to_owned()))?;
    let extra: Vec<&str> = words.get(2..).unwrap_or_default().iter().map(|s| s.as_str()).collect();
    if status == "clear" {
        let mut args: Vec<&str> =
            vec!["track", "clear-override", "--items-dir", "track/items", track_id];
        args.extend_from_slice(&extra);
        run_sotp(&args)
    } else {
        let mut args: Vec<&str> =
            vec!["track", "set-override", "--items-dir", "track/items", track_id, status];
        args.extend_from_slice(&extra);
        run_sotp(&args)
    }
}

fn dispatch_track_local_plan(raw_args: &[String]) -> Result<ExitCode, CliError> {
    let words = raw_args_to_words(raw_args);
    // Filter out leading "--" separator if present
    let filtered: Vec<&str> = words.iter().map(|s| s.as_str()).skip_while(|s| *s == "--").collect();
    let mut args: Vec<&str> = vec!["plan", "codex-local"];
    args.extend_from_slice(&filtered);
    run_sotp(&args)
}

fn dispatch_track_local_review(raw_args: &[String]) -> Result<ExitCode, CliError> {
    let words = raw_args_to_words(raw_args);
    // Filter out leading "--" separator if present
    let filtered: Vec<&str> = words.iter().map(|s| s.as_str()).skip_while(|s| *s == "--").collect();
    let mut args: Vec<&str> = vec!["review", "codex-local"];
    args.extend_from_slice(&filtered);
    run_sotp(&args)
}

/// Build the sotp argv for a forwarding dispatcher: prefix + user args (with leading "--" stripped).
///
/// Uses `raw_args_to_words` (same as other dispatchers) which handles both
/// cargo-make single-string and direct CLI multi-arg invocations.
fn build_forwarded_args(prefix: &[&str], raw_args: &[String]) -> Vec<String> {
    let words = raw_args_to_words(raw_args);
    let filtered: Vec<&str> = words.iter().map(|s| s.as_str()).skip_while(|s| *s == "--").collect();
    let mut args: Vec<String> = prefix.iter().map(|s| (*s).to_owned()).collect();
    args.extend(filtered.iter().map(|s| (*s).to_owned()));
    args
}

fn dispatch_track_review_status(raw_args: &[String]) -> Result<ExitCode, CliError> {
    let args = build_forwarded_args(&["review", "status"], raw_args);
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    run_sotp(&refs)
}

fn dispatch_track_check_approved(raw_args: &[String]) -> Result<ExitCode, CliError> {
    let args = build_forwarded_args(&["review", "check-approved"], raw_args);
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    run_sotp(&refs)
}

// --- Phase 2: New logic dispatchers ---

fn dispatch_commit(raw_args: &[String]) -> Result<ExitCode, CliError> {
    let message = raw_args_to_single(raw_args)
        .map_err(|_| CliError::Message("error: commit message required".to_owned()))?;
    // Run CI first — propagate actual exit code on failure
    let ci_result = run_command("cargo", &["make", "ci"])?;
    if ci_result != ExitCode::SUCCESS {
        return Ok(ci_result);
    }
    // Commit with the message passed safely as a -m argument (no shell interpolation)
    run_command("git", &["commit", "-m", &message])
}

fn dispatch_note(raw_args: &[String]) -> Result<ExitCode, CliError> {
    let note_text = raw_args_to_single(raw_args)
        .map_err(|_| CliError::Message("error: note text required".to_owned()))?;
    // Pass note text safely as a -m argument (no shell interpolation)
    run_command("git", &["notes", "add", "-f", "-m", &note_text, "HEAD"])
}

fn dispatch_track_commit_message() -> Result<ExitCode, CliError> {
    std::fs::create_dir_all("tmp")
        .map_err(|e| CliError::Message(format!("mkdir tmp failed: {e}")))?;

    // ADR 2026-04-18-1400 §D2: pre-commit auto-recomputation of TDDD type
    // signals runs BEFORE CI so the stale-detection pass in CI
    // (`verify-spec-states-current-local`) always sees a fresh evaluation
    // result. Red signals block the commit here (§D3) with an actionable
    // message, and the commit-message.txt scratch file is preserved.
    if let Some(track_id) = current_branch_track_id_strict()? {
        eprintln!(
            "[track-commit-message] Pre-commit: recomputing type signals for '{track_id}'..."
        );
        let signals_result = run_pre_commit_type_signals(&track_id)?;
        if signals_result != ExitCode::SUCCESS {
            return Ok(signals_result);
        }
    }

    eprintln!("[track-commit-message] Running CI...");
    let log_file = std::fs::File::create("tmp/ci-output.log")
        .map_err(|e| CliError::Message(format!("failed to create tmp/ci-output.log: {e}")))?;
    let log_file_err = log_file
        .try_clone()
        .map_err(|e| CliError::Message(format!("failed to clone log file handle: {e}")))?;
    let ci_status = std::process::Command::new("cargo")
        .args(["make", "ci"])
        .stdout(log_file)
        .stderr(log_file_err)
        .status()?;

    if !ci_status.success() {
        let ci_exit = ci_status.code().unwrap_or(1);
        eprintln!("[track-commit-message] CI FAILED (exit {ci_exit}). Last 20 lines:");
        // Read last 20 lines from ci-output.log
        if let Ok(content) = std::fs::read_to_string("tmp/ci-output.log") {
            let lines: Vec<&str> = content.lines().collect();
            let start = lines.len().saturating_sub(20);
            for line in lines.get(start..).unwrap_or_default() {
                eprintln!("{line}");
            }
        }
        // Propagate the actual CI exit code instead of always returning 1
        return Ok(ExitCode::from(u8::try_from(ci_exit).unwrap_or(1)));
    }
    eprintln!("[track-commit-message] CI PASSED");

    // Review guard: check review.status == approved with current code hash.
    // Resolve track ID from current branch (track/<id>).
    if let Some(track_id) = current_branch_track_id_strict()? {
        eprintln!("[track-commit-message] Checking review approval for track '{track_id}'...");
        let guard_result = run_sotp(&[
            "review",
            "check-approved",
            "--items-dir",
            "track/items",
            "--track-id",
            &track_id,
        ])?;
        if guard_result != ExitCode::SUCCESS {
            eprintln!("[track-commit-message] BLOCKED: review guard rejected commit");
            return Ok(guard_result);
        }
        eprintln!("[track-commit-message] Review approved");
    }

    let commit_result =
        run_sotp(&["git", "commit-from-file", "tmp/track-commit/commit-message.txt", "--cleanup"])?;
    if commit_result != ExitCode::SUCCESS {
        return Ok(commit_result);
    }

    // Post-commit: persist HEAD SHA to .commit_hash for incremental review scope.
    let mut post_commit_failed = false;
    if let Some(ref track_id) = current_branch_track_id_strict()? {
        if let Err(msg) = persist_commit_hash_v2(track_id) {
            eprintln!("[track-commit-message] WARNING: .commit_hash persistence failed: {msg}");
            eprintln!(
                "[track-commit-message] Recovery: run `bin/sotp make track-set-commit-hash \
                 {track_id}` to set the v2 diff base manually."
            );
            post_commit_failed = true;
        }
    }

    if post_commit_failed {
        // Exit code 3 distinguishes "commit succeeded but post-commit step failed" from
        // a real commit failure (exit 1). Automation must not retry the commit on exit 3.
        eprintln!("[track-commit-message] COMMIT_OK but post-commit steps failed (see above)");
        return Ok(ExitCode::from(3));
    }
    Ok(ExitCode::SUCCESS)
}

/// Runs the ADR 2026-04-18-1400 §D2 pre-commit type-signal recomputation
/// step.
///
/// Steps per ADR §D2 / §D3 / §D7:
/// 1. Fail-closed on missing `architecture-rules.json` (the new pre-commit
///    path explicitly does NOT inherit the legacy synthetic-domain fallback
///    that `sotp track type-signals` uses — see §D2 last paragraph).
/// 2. Delegate the recomputation itself to `execute_type_signals`, which
///    writes `<layer>-type-signals.json` and re-encodes declaration files
///    via the declaration codec (which omits the `signals` field). Symlink guards
///    (§D7) are applied inside `execute_type_signals` on both write paths.
/// 3. After recomputation, read each generated signal file and classify the
///    result: Red → BLOCKED (exit 1, commit-message.txt preserved), Yellow
///    → warning on stderr + proceed, Blue → silent pass.
///
/// The Red / Yellow classification mirrors the spec.md Behavior Truth Table
/// for the pre-commit column. Full-route CI verification still runs after
/// this step (§D2 ordering: recompute → CI → review guard → commit).
#[allow(clippy::too_many_lines)]
fn run_pre_commit_type_signals(track_id: &str) -> Result<ExitCode, CliError> {
    // Resolve the workspace root from the git discovery result, not from the
    // current working directory. Running `/track:commit` (which invokes
    // `bin/sotp make track-commit-message`) from a nested subdirectory must
    // still locate `architecture-rules.json` and `track/items/` at the repo
    // root. `PathBuf::from(".")` would introduce CWD-dependent behavior that
    // silently fail-closes commits launched from subdirectories.
    use infrastructure::git_cli::{GitRepository, SystemGitRepo};
    let workspace_root = SystemGitRepo::discover()
        .map_err(|e| {
            CliError::Message(format!(
                "[track-commit-message] BLOCKED: unable to discover git repository root: {e}"
            ))
        })?
        .root()
        .to_path_buf();

    // ADR §D2 fail-closed: architecture-rules.json must be present and
    // readable. Unlike the legacy `sotp track type-signals` CLI, the
    // pre-commit path does NOT fall back to a synthetic domain binding.
    let rules_path = workspace_root.join("architecture-rules.json");

    // Pre-flight snapshot: read + parse `architecture-rules.json` exactly
    // once. The same parsed binding set drives recompute (via
    // `execute_type_signals_lenient_with_bindings`) AND the post-recompute
    // classification loop below, eliminating the TOCTOU window where
    // `architecture-rules.json` could be edited between separate reads and
    // allow stale signals through pre-commit (PR #106 TOCTOU P1 finding).
    let bindings_snapshot = match symlink_guard::reject_symlinks_below(&rules_path, &workspace_root)
    {
        Ok(true) => {
            let content = std::fs::read_to_string(&rules_path).map_err(|e| {
                CliError::Message(format!(
                    "[track-commit-message] BLOCKED: cannot read {}: {e}",
                    rules_path.display()
                ))
            })?;
            parse_tddd_layers(&content).map_err(|e| {
                CliError::Message(format!(
                    "[track-commit-message] BLOCKED: architecture-rules.json parse error: {e}"
                ))
            })?
        }
        Ok(false) => {
            eprintln!(
                "[track-commit-message] BLOCKED: architecture-rules.json not found. \
                 Pre-commit type-signal recomputation cannot enumerate TDDD layers."
            );
            return Ok(ExitCode::from(1));
        }
        Err(e) => {
            eprintln!(
                "[track-commit-message] BLOCKED: architecture-rules.json symlink rejected: {e}"
            );
            return Ok(ExitCode::from(1));
        }
    };

    let items_dir = workspace_root.join("track").join("items");

    // Active-track guard: Done/Archived tracks have frozen type declarations;
    // there is nothing to recompute. Skip the pre-commit step gracefully rather
    // than delegating to `execute_type_signals`, which correctly rejects Done/Archived
    // tracks with a user-visible error (protecting immutability). The pre-commit path
    // reaching a Done track is valid during the final done-metadata commit (all tasks
    // transitioned to done, metadata written, then committed).
    {
        let valid_id = domain::TrackId::try_new(track_id).map_err(|e| {
            CliError::Message(format!("[track-commit-message] invalid track ID '{track_id}': {e}"))
        })?;
        match infrastructure::track::fs_store::read_track_metadata(&items_dir, &valid_id) {
            Ok((metadata, _doc_meta)) => {
                // Status is derived on demand from impl-plan + status_override.
                // Use FsTrackStore::load_impl_plan (fail-closed) so a corrupt
                // impl-plan.json blocks the commit rather than being treated as absent.
                use domain::ImplPlanReader;
                let store = infrastructure::track::fs_store::FsTrackStore::new(items_dir.clone());
                let impl_plan = store.load_impl_plan(&valid_id).map_err(|e| {
                    CliError::Message(format!(
                        "[track-commit-message] BLOCKED: cannot load impl-plan for \
                         '{track_id}': {e}"
                    ))
                })?;
                // Fail-closed: a track with no impl-plan but non-Planned status (branch set
                // or non-Planned override) is potentially corrupt — block the commit.
                let effective_status =
                    domain::derive_track_status(impl_plan.as_ref(), metadata.status_override());
                if impl_plan.is_none()
                    && (metadata.branch().is_some()
                        || effective_status != domain::TrackStatus::Planned)
                {
                    return Err(CliError::Message(format!(
                        "[track-commit-message] BLOCKED: track '{track_id}' has no \
                         impl-plan.json but is not in planning state \
                         (derived_status={effective_status}); track may be corrupt"
                    )));
                }
                if ensure_active_track(effective_status, track_id).is_err() {
                    // Track is Done or Archived — skip pre-commit type-signal recomputation.
                    // The frozen track's signal files are already correct from when it was active.
                    eprintln!(
                        "[track-commit-message] Pre-commit type signals: skipped \
                         (track '{track_id}' is {effective_status} — declarations are frozen)."
                    );
                    return Ok(ExitCode::SUCCESS);
                }
            }
            Err(e) => {
                // Metadata read failure is fail-closed: block the commit rather than
                // silently skipping the pre-commit gate.
                return Err(CliError::Message(format!(
                    "[track-commit-message] BLOCKED: cannot read metadata for '{track_id}': {e}"
                )));
            }
        }
    }

    // Delegate to the lenient variant so pre-commit matches CI semantics:
    // a layer without a declaration file is treated as "TDDD not active for
    // this layer" and skipped silently, rather than hard-failing the commit.
    // This keeps pre-commit from being stricter than CI / merge gate (ADR
    // §D2 / §D5 symmetry). Pass the pre-flight `bindings_snapshot` so the
    // recompute runs against exactly the same binding set the classification
    // loop below will use — no TOCTOU gap.
    let exec_result = execute_type_signals_lenient_with_bindings(
        items_dir.clone(),
        track_id.to_owned(),
        workspace_root.clone(),
        &bindings_snapshot,
    )?;
    if exec_result != ExitCode::SUCCESS {
        eprintln!("[track-commit-message] BLOCKED: type-signals recomputation returned non-zero");
        return Ok(exec_result);
    }

    // Classify against the same `bindings_snapshot` the recompute saw.
    // Re-reading `architecture-rules.json` here would re-open the TOCTOU
    // window called out by the PR #106 review; keep a single source of
    // truth for the entire pre-commit critical section.
    let bindings_post = &bindings_snapshot;

    let track_dir = items_dir.join(track_id);

    // Stage the regenerated files so they are included in the subsequent
    // `git commit-from-file`. `execute_type_signals` writes the declaration
    // file, rendered Markdown, and signal file for each layer that has a
    // catalogue. Without this step the regenerated working-tree files would
    // be left out of the commit (the index still reflects pre-recomputation
    // content), causing CI to validate a different tree than what gets recorded.
    // Use `bindings_post` (post-recompute re-read) so we stage exactly the files
    // that execute_type_signals processed.
    for binding in bindings_post {
        let catalogue_path = track_dir.join(binding.catalogue_file());
        if !catalogue_path.is_file() {
            continue; // no catalogue → nothing was written → nothing to stage
        }
        // Mirror the lenient executor's multi-target skip: for layers with
        // multiple `schema_export.targets`, `execute_type_signals_lenient`
        // did NOT regenerate catalogue / rendered / signal files, so
        // `git add` here would stage pre-existing working-tree content
        // (including any unrelated unstaged edits) rather than recomputed
        // outputs. Skip staging entirely for those layers — CI / merge-gate
        // still detect staleness via `declaration_hash` comparison on the
        // already-committed signal file.
        if binding.targets().len() > 1 {
            continue;
        }
        // Stage all three files written by execute_type_signals for this layer.
        for rel_path in &[
            track_dir.join(binding.catalogue_file()),
            track_dir.join(binding.rendered_file()),
            track_dir.join(binding.signal_file()),
        ] {
            if rel_path.is_file() {
                let status = std::process::Command::new("git")
                    .args(["add", "--", &rel_path.display().to_string()])
                    .status()
                    .map_err(|e| CliError::Message(format!("pre-commit: git add failed: {e}")))?;
                if !status.success() {
                    return Err(CliError::Message(format!(
                        "pre-commit: git add {} returned non-zero",
                        rel_path.display()
                    )));
                }
            }
        }
    }
    let mut red_names: Vec<String> = Vec::new();
    let mut yellow_names: Vec<String> = Vec::new();
    for binding in bindings_post {
        // Skip layers whose declaration file is absent on this track —
        // matches the CI / merge-gate semantics (`evaluate_layer_catalogue`
        // treats a missing catalogue as "TDDD not active for this layer"
        // and returns `VerifyOutcome::pass()`). Reading an orphan signal
        // file (declaration deleted but signals left behind) would block
        // commits on stale Red signals that the downstream gates silently
        // skip, producing a pre-commit vs CI divergence.
        let catalogue_path = track_dir.join(binding.catalogue_file());
        if !catalogue_path.is_file() {
            continue;
        }

        // Skip multi-target layers here so this post-recompute missing-file
        // gate stays consistent with `execute_type_signals_lenient`, which
        // intentionally bypasses the strict evaluator for layers whose
        // `schema_export.targets` has more than one entry. Without this
        // exemption the "catalogue present but signal file absent" BLOCKED
        // branch below would fire on every pre-commit for multi-target
        // tracks even though the lenient executor chose not to write signals
        // for them. CI / merge-gate detect staleness independently via
        // `declaration_hash` comparison on the persisted signal file.
        if binding.targets().len() > 1 {
            continue;
        }

        let signal_path = track_dir.join(binding.signal_file());
        if !signal_path.is_file() {
            // Catalogue exists but signal file is absent after a successful
            // recompute — something went wrong (e.g. execute_type_signals
            // used fewer bindings than we expect). Treat this as BLOCKED to
            // avoid silently skipping a layer that could contain Red signals.
            eprintln!(
                "[track-commit-message] BLOCKED: {} has a catalogue ({}) but no signal file \
                 ({}) after recomputation. This may indicate a TOCTOU race on \
                 architecture-rules.json.",
                binding.layer_id(),
                binding.catalogue_file(),
                binding.signal_file(),
            );
            return Ok(ExitCode::from(1));
        }
        // ADR §D7 read-path symlink guard: reject symlinks on the signal file
        // before reading so that a symlink-swap after recomputation cannot
        // cause the gate to evaluate attacker-chosen content.
        match symlink_guard::reject_symlinks_below(&signal_path, &track_dir) {
            Ok(true) => {}
            Ok(false) => {
                // File vanished between the is_file() check and this guard.
                // The catalogue presence was already verified above, so this
                // is a missing-after-recompute race — BLOCKED.
                eprintln!(
                    "[track-commit-message] BLOCKED: {} disappeared between existence check \
                     and read.",
                    signal_path.display()
                );
                return Ok(ExitCode::from(1));
            }
            Err(e) => {
                return Err(CliError::Message(format!(
                    "pre-commit: symlink rejected on {}: {e}",
                    signal_path.display()
                )));
            }
        }
        let content = std::fs::read_to_string(&signal_path).map_err(|e| {
            CliError::Message(format!(
                "pre-commit: cannot read {} after recompute: {e}",
                signal_path.display()
            ))
        })?;
        let doc = type_signals_codec::decode(&content).map_err(|e| {
            CliError::Message(format!("pre-commit: decode error on {}: {e}", signal_path.display()))
        })?;
        for signal in doc.signals() {
            match signal.signal() {
                ConfidenceSignal::Red => {
                    red_names.push(format!(
                        "{}: {} ({})",
                        binding.layer_id(),
                        signal.type_name(),
                        signal.kind_tag()
                    ));
                }
                ConfidenceSignal::Yellow => {
                    yellow_names.push(format!(
                        "{}: {} ({})",
                        binding.layer_id(),
                        signal.type_name(),
                        signal.kind_tag()
                    ));
                }
                _ => {}
            }
        }
    }

    if !red_names.is_empty() {
        eprintln!("[track-commit-message] BLOCKED: type-signals Red detected");
        for name in &red_names {
            eprintln!("  Red: {name}");
        }
        eprintln!(
            "[track-commit-message] Fix: run /track:design to update type declarations, \
             then re-run /track:commit"
        );
        eprintln!("[track-commit-message] commit-message.txt is preserved for your next attempt.");
        return Ok(ExitCode::from(1));
    }

    if !yellow_names.is_empty() {
        eprintln!("[track-commit-message] WARN: Yellow type-signals detected (commit proceeds):");
        for name in &yellow_names {
            eprintln!("  Yellow: {name}");
        }
    }

    eprintln!("[track-commit-message] Pre-commit type signals: OK");
    Ok(ExitCode::SUCCESS)
}

fn dispatch_set_commit_hash(raw_args: &[String]) -> Result<ExitCode, CliError> {
    let track_id = raw_args_to_single(raw_args)
        .map_err(|_| CliError::Message("usage: track-set-commit-hash <track-id>".to_owned()))?;
    match persist_commit_hash_v2(&track_id) {
        Ok(()) => Ok(ExitCode::SUCCESS),
        Err(msg) => {
            eprintln!("{msg}");
            Ok(ExitCode::FAILURE)
        }
    }
}

/// Persists the current HEAD SHA to `.commit_hash` (v2 incremental diff base).
///
/// # Errors
/// Returns a human-readable error string on failure.
fn persist_commit_hash_v2(track_id: &str) -> Result<(), String> {
    use domain::review_v2::CommitHashWriter;
    use infrastructure::git_cli::{GitRepository, SystemGitRepo};

    // Validate track_id as a proper slug before using it as a path segment.
    let validated_id =
        domain::TrackId::try_new(track_id).map_err(|e| format!("invalid track id: {e}"))?;

    let git = SystemGitRepo::discover().map_err(|e| format!("git discover: {e}"))?;
    let root = git.root().to_path_buf();

    // Branch guard: prevent cross-track corruption.
    let branch_output = git
        .output(&["rev-parse", "--abbrev-ref", "HEAD"])
        .map_err(|e| format!("git rev-parse --abbrev-ref HEAD: {e}"))?;
    if !branch_output.status.success() {
        return Err("git rev-parse --abbrev-ref HEAD failed (cannot verify branch)".to_owned());
    }
    let branch = String::from_utf8_lossy(&branch_output.stdout).trim().to_owned();
    let expected = format!("track/{validated_id}");
    if branch != expected {
        return Err(format!(
            "current branch '{branch}' does not match track branch '{expected}'. \
             Run from the correct track branch to prevent cross-track corruption."
        ));
    }

    let head_output =
        git.output(&["rev-parse", "HEAD"]).map_err(|e| format!("git rev-parse HEAD: {e}"))?;
    if !head_output.status.success() {
        return Err("git rev-parse HEAD failed".to_owned());
    }
    let head_sha = String::from_utf8_lossy(&head_output.stdout).trim().to_owned();
    let commit_hash = domain::CommitHash::try_new(&head_sha).map_err(|e| format!("{e}"))?;

    let track_dir = root.join("track/items").join(validated_id.as_ref());
    if !track_dir.is_dir() {
        return Err(format!(
            "track directory '{}' does not exist. \
             Cannot write .commit_hash for non-existent track '{validated_id}'.",
            track_dir.display(),
        ));
    }
    let commit_hash_path = track_dir.join(".commit_hash");
    let store = infrastructure::review_v2::FsCommitHashStore::new(commit_hash_path, root);
    store.write(&commit_hash).map_err(|e| format!("{e}"))?;

    eprintln!("[track-commit-message] Recorded .commit_hash: {head_sha}");
    Ok(())
}

/// Resolves the track ID from the current git branch (strict mode).
///
/// Returns `Ok(Some(id))` only when the branch matches `track/<id>` and the
/// id passes [`TrackId`] validation. Plan-phase branches (`plan/<id>`)
/// intentionally resolve to `Ok(None)` because the make-task callers
/// (review check-approved, post-commit hash persistence) only apply once a
/// track has progressed past the planning phase. Non-track branches (e.g.
/// `main`) and git failures also resolve to `Ok(None)`.
///
/// Returns `Err` when the branch matches `track/<id>` but the `<id>` fails
/// validation: in that case the callers must not silently skip the review
/// guard (fail-closed).
///
/// Internally delegates parsing to
/// [`usecase::track_resolution::resolve_track_id_from_branch`] so the
/// branch-name semantics stay consistent with the rest of the workflow.
fn current_branch_track_id_strict() -> Result<Option<String>, CliError> {
    let output =
        std::process::Command::new("git").args(["rev-parse", "--abbrev-ref", "HEAD"]).output().ok();
    let Some(output) = output else { return Ok(None) };
    if !output.status.success() {
        return Ok(None);
    }
    let branch = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    match usecase::track_resolution::resolve_track_id_from_branch(Some(&branch)) {
        Ok(id) => Ok(Some(id)),
        Err(usecase::track_resolution::TrackResolutionError::InvalidTrackId(slug, _)) => {
            Err(CliError::Message(format!(
                "current branch 'track/{slug}' has an invalid track id; \
                 rename the branch or switch to a valid track branch before committing"
            )))
        }
        Err(_) => Ok(None),
    }
}

// --- Phase 4: Exec dispatcher ---

fn dispatch_exec(raw_args: &[String]) -> Result<ExitCode, CliError> {
    let words = raw_args_to_words(raw_args);
    if words.is_empty() {
        return Err(CliError::Message("error: usage: sotp make exec <local-task-name>".to_owned()));
    }
    // Safety: `.is_empty()` check above guarantees at least one element
    let task_name = words.first().ok_or_else(|| {
        CliError::Message("error: usage: sotp make exec <local-task-name>".to_owned())
    })?;
    let worker_id = std::env::var("WORKER_ID").ok();

    let mut args: Vec<String> = vec!["compose".to_owned(), "exec".to_owned(), "-T".to_owned()];
    if let Some(ref wid) = worker_id {
        args.push("-e".to_owned());
        args.push(format!("CARGO_TARGET_DIR=/workspace/target-{wid}"));
    }
    args.extend_from_slice(&[
        "tools-daemon".to_owned(),
        "cargo".to_owned(),
        "make".to_owned(),
        "--allow-private".to_owned(),
        format!("{task_name}-local"),
    ]);
    // Forward any remaining args after the task name
    for extra in words.get(1..).unwrap_or_default() {
        args.push(extra.clone());
    }
    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    run_command("docker", &arg_refs)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    // --- raw_args_to_single tests ---

    #[test]
    fn test_raw_args_to_single_with_single_element() {
        let args = vec!["my-track-id".to_owned()];
        assert_eq!(raw_args_to_single(&args).unwrap(), "my-track-id");
    }

    #[test]
    fn test_raw_args_to_single_with_spaced_string() {
        let args = vec!["commit message with spaces".to_owned()];
        assert_eq!(raw_args_to_single(&args).unwrap(), "commit message with spaces");
    }

    #[test]
    fn test_raw_args_to_single_with_multiple_elements() {
        let args = vec!["part1".to_owned(), "part2".to_owned()];
        assert_eq!(raw_args_to_single(&args).unwrap(), "part1 part2");
    }

    #[test]
    fn test_raw_args_to_single_empty_returns_error() {
        let args: Vec<String> = vec![];
        assert!(raw_args_to_single(&args).is_err());
    }

    #[test]
    fn test_raw_args_to_single_whitespace_only_returns_error() {
        let args = vec!["  ".to_owned()];
        assert!(raw_args_to_single(&args).is_err());
    }

    // --- raw_args_to_words tests ---

    #[test]
    fn test_raw_args_to_words_single_element() {
        let args = vec!["my-id".to_owned()];
        assert_eq!(raw_args_to_words(&args), vec!["my-id"]);
    }

    #[test]
    fn test_raw_args_to_words_splits_single_string() {
        let args = vec!["track/items/xxx T001 done".to_owned()];
        assert_eq!(raw_args_to_words(&args), vec!["track/items/xxx", "T001", "done"]);
    }

    #[test]
    fn test_raw_args_to_words_multiple_elements_already_split() {
        let args = vec!["track/items/xxx".to_owned(), "T001".to_owned(), "done".to_owned()];
        assert_eq!(raw_args_to_words(&args), vec!["track/items/xxx", "T001", "done"]);
    }

    #[test]
    fn test_raw_args_to_words_empty() {
        let args: Vec<String> = vec![];
        let result: Vec<String> = raw_args_to_words(&args);
        assert!(result.is_empty());
    }

    #[test]
    fn test_raw_args_to_words_with_extra_flags() {
        let args = vec!["track/items/xxx T001 done --commit-hash abc123".to_owned()];
        assert_eq!(
            raw_args_to_words(&args),
            vec!["track/items/xxx", "T001", "done", "--commit-hash", "abc123"]
        );
    }

    // --- build_forwarded_args tests ---

    #[test]
    fn test_build_forwarded_args_prepends_prefix() {
        let raw = vec!["--track-id my-track --round-type fast".to_owned()];
        let args = build_forwarded_args(&["review", "record-round"], &raw);
        assert_eq!(args[0], "review");
        assert_eq!(args[1], "record-round");
        assert_eq!(args[2], "--track-id");
    }

    #[test]
    fn test_build_forwarded_args_strips_leading_double_dash() {
        let raw = vec!["-- --track-id my-track".to_owned()];
        let args = build_forwarded_args(&["review", "check-approved"], &raw);
        assert_eq!(args, vec!["review", "check-approved", "--track-id", "my-track"]);
    }

    #[test]
    fn test_build_forwarded_args_empty_raw() {
        let raw: Vec<String> = vec![];
        let args = build_forwarded_args(&["review", "check-approved"], &raw);
        assert_eq!(args, vec!["review", "check-approved"]);
    }

    #[test]
    fn test_raw_args_to_words_preserves_quoting_in_direct_call() {
        // Direct CLI: bin/sotp make track-add-task track-1 "fix parser bug"
        // Shell splits into two args, preserving the quoted group
        let args = vec!["track-1".to_owned(), "fix parser bug".to_owned()];
        assert_eq!(raw_args_to_words(&args), vec!["track-1", "fix parser bug"]);
    }
}
