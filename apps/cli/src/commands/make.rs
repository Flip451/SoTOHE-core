//! Replaces Makefile.toml `script_runner = "@shell"` wrappers with safe Rust dispatch.
//!
//! Each task accepts raw arguments from `cargo make ${@}` and handles them safely
//! without shell string interpolation. The handler decides how to interpret the
//! arguments: some tasks treat them as a single value, others split into multiple
//! positional arguments.

use std::process::ExitCode;

use clap::{Args, ValueEnum};
use cli_composition::CliApp;

use crate::CliError;

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
    /// Run the review-fix-lead fixer via `sotp review fix-local`.
    TrackLocalReviewFixCodex,
    /// Run the dry-fix-lead fixer via `sotp dry fix-local`.
    TrackLocalDryFix,
    /// Show per-scope review results (state summary by default).
    TrackReviewResults,
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
        MakeTask::TrackLocalReviewFixCodex => dispatch_track_local_review_fix_codex(&args.raw_args),
        MakeTask::TrackLocalDryFix => dispatch_track_local_dry_fix(&args.raw_args),
        MakeTask::TrackReviewResults => dispatch_track_review_results(&args.raw_args),
        MakeTask::TrackCheckApproved => dispatch_track_check_approved(&args.raw_args),
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
    // Passthrough: inject --items-dir and forward all remaining args verbatim.
    // If the caller passes --track-id <id>, it flows through; otherwise the
    // underlying command self-resolves from the current branch (D1, D6).
    let args =
        build_forwarded_args(&["track", "next-task", "--items-dir", "track/items"], raw_args);
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    run_sotp(&refs)
}

fn dispatch_track_task_counts(raw_args: &[String]) -> Result<ExitCode, CliError> {
    // Passthrough: inject --items-dir and forward all remaining args verbatim.
    // If the caller passes --track-id <id>, it flows through; otherwise the
    // underlying command self-resolves from the current branch (D1, D6).
    let args =
        build_forwarded_args(&["track", "task-counts", "--items-dir", "track/items"], raw_args);
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    run_sotp(&refs)
}

fn dispatch_track_transition(raw_args: &[String]) -> Result<ExitCode, CliError> {
    // Passthrough: inject --items-dir and forward all remaining args verbatim.
    // Callers pass task_id, status, and optional --commit-hash / --track-id as flags.
    // --track-id is forwarded if present; omitting it triggers self-resolve (D1, D6).
    let args =
        build_forwarded_args(&["track", "transition", "--items-dir", "track/items"], raw_args);
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    run_sotp(&refs)
}

fn dispatch_track_add_task(raw_args: &[String]) -> Result<ExitCode, CliError> {
    // Passthrough: inject --items-dir and forward all remaining args verbatim.
    // If the caller passes --track-id <id>, it flows through; otherwise the
    // underlying command self-resolves from the current branch (D1, D6).
    let args = build_forwarded_args(&["track", "add-task", "--items-dir", "track/items"], raw_args);
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    run_sotp(&refs)
}

/// Build the sotp argv for `track-set-override` / `track-clear-override`.
///
/// Finds the first positional (non-flag, non-flag-value) word in `raw_args` as
/// the status, then routes:
/// - `"clear"` → `["track", "clear-override", "--items-dir", "track/items", <rest>]`
/// - other     → `["track", "set-override",   "--items-dir", "track/items", status, <rest>]`
///
/// Only **value-taking** flags (`--track-id`, `--reason`) consume the next token as their
/// value. All other flags (those starting with `-` but not in VALUE_FLAGS) are treated as
/// boolean flags and do not consume the next token. The status word is removed by **index**
/// (not by value), so a flag value that happens to equal the status string is never silently
/// dropped.
///
/// # Errors
///
/// Returns `Err` if no positional word is found (missing status argument).
pub fn build_set_override_args(raw_args: &[String]) -> Result<Vec<String>, CliError> {
    let words = raw_args_to_words(raw_args);
    let filtered: Vec<&str> = words.iter().map(|s| s.as_str()).skip_while(|s| *s == "--").collect();
    let usage = "error: usage: sotp make track-set-override <blocked|cancelled|clear> [--track-id <id>] [--reason <text>]";
    // Only these flags take a value argument; boolean flags do not consume the next token.
    const VALUE_FLAGS: &[&str] = &["--track-id", "--reason"];
    // Walk the filtered tokens. Skip known value-taking flags and their values.
    // Any flag starting with '-' but not in VALUE_FLAGS is treated as boolean.
    let mut status_idx: Option<usize> = None;
    let mut skip_next = false;
    for (i, word) in filtered.iter().enumerate() {
        if skip_next {
            skip_next = false;
            continue;
        }
        if VALUE_FLAGS.contains(word) {
            // This flag takes exactly one value argument.
            skip_next = true;
        } else if !word.starts_with('-') {
            // First non-flag, non-flag-value token is the status.
            status_idx = Some(i);
            break;
        }
        // Any flag starting with '-' but not in VALUE_FLAGS is a boolean flag; skip without consuming next.
    }
    let status_idx = status_idx.ok_or_else(|| CliError::Message(usage.to_owned()))?;
    let status = filtered.get(status_idx).ok_or_else(|| CliError::Message(usage.to_owned()))?;
    // Remaining args: all words except the status word at status_idx, removed by index.
    let rest: Vec<&str> =
        filtered.iter().enumerate().filter(|(i, _)| *i != status_idx).map(|(_, s)| *s).collect();
    if *status == "clear" {
        let mut args: Vec<String> = vec![
            "track".to_owned(),
            "clear-override".to_owned(),
            "--items-dir".to_owned(),
            "track/items".to_owned(),
        ];
        args.extend(rest.iter().map(|s| (*s).to_owned()));
        Ok(args)
    } else {
        let mut args: Vec<String> = vec![
            "track".to_owned(),
            "set-override".to_owned(),
            "--items-dir".to_owned(),
            "track/items".to_owned(),
            (*status).to_owned(),
        ];
        args.extend(rest.iter().map(|s| (*s).to_owned()));
        Ok(args)
    }
}

fn dispatch_track_set_override(raw_args: &[String]) -> Result<ExitCode, CliError> {
    let args = build_set_override_args(raw_args)?;
    let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    run_sotp(&refs)
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
    // Filter out leading "--" separator if present.
    // Route to `sotp review local` which auto-resolves provider/model from
    // agent-profiles.json. The caller does not need to pass --model.
    let filtered: Vec<&str> = words.iter().map(|s| s.as_str()).skip_while(|s| *s == "--").collect();
    let mut args: Vec<&str> = vec!["review", "local"];
    args.extend_from_slice(&filtered);
    run_sotp(&args)
}

fn dispatch_track_local_review_fix_codex(raw_args: &[String]) -> Result<ExitCode, CliError> {
    review_fix::dispatch_track_local_review_fix_codex(raw_args)
}

fn dispatch_track_local_dry_fix(raw_args: &[String]) -> Result<ExitCode, CliError> {
    dry_fix::dispatch_track_local_dry_fix(raw_args)
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

fn dispatch_track_review_results(raw_args: &[String]) -> Result<ExitCode, CliError> {
    let args = build_forwarded_args(&["review", "results"], raw_args);
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

/// Delegate to `CliApp::make_track_commit_message`, which is the single authoritative
/// implementation. The full gate sequence is: stage → CI → review check-approved →
/// **DRY check-approved** (AC-11) → git commit-from-file → persist .commit_hash.
///
/// # Errors
///
/// Propagates any I/O error from the underlying `CliApp` call as a `CliError::Message`.
fn dispatch_track_commit_message() -> Result<ExitCode, CliError> {
    match CliApp::new().make_track_commit_message(vec![]) {
        Ok(outcome) => Ok(ExitCode::from(outcome.exit_code)),
        Err(msg) => {
            eprintln!("{msg}");
            Ok(ExitCode::FAILURE)
        }
    }
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
/// Delegates to `cli_composition::review_v2::persist_commit_hash_for_track` so
/// that this function does not import `domain::CommitHash`, `domain::TrackId`,
/// or `domain::review_v2::CommitHashWriter` directly (CN-01 / AC-03).
///
/// # Errors
/// Returns a human-readable error string on failure.
fn persist_commit_hash_v2(track_id: &str) -> Result<(), String> {
    let head_sha = cli_composition::review_v2::persist_commit_hash_for_track(track_id)?;
    eprintln!("[track-commit-message] Recorded .commit_hash: {head_sha}");
    Ok(())
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

#[path = "make_review_fix.rs"]
mod review_fix;

#[path = "make_dry_fix.rs"]
mod dry_fix;

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
#[path = "make_tests.rs"]
mod tests;
