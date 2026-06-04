//! `make` command family — CliApp impl methods.
//!
//! Each method receives `raw_args: Vec<String>` forwarded from cargo-make and
//! forwards them to the real `sotp` binary via `run_sotp`.  This matches the
//! baseline `apps/cli/src/commands/make.rs` dispatch_* pattern exactly, so
//! clap (via the subprocess) owns all argument parsing, validation and defaults.

use std::path::PathBuf;

use crate::{CliApp, CommandOutcome};

/// Join raw args into a single string (same semantics as `apps/cli/src/commands/make.rs`).
fn raw_args_to_single(raw_args: &[String]) -> Result<String, String> {
    let joined = raw_args.join(" ");
    let trimmed = joined.trim();
    if trimmed.is_empty() {
        return Err("missing required argument".to_owned());
    }
    Ok(trimmed.to_owned())
}

/// Split raw args into individual words.
///
/// When cargo-make passes `${@}` as a single string element, this function
/// splits it on whitespace.  When called directly with multiple args (already
/// properly split by the shell), they are returned as-is to preserve quoting.
fn raw_args_to_words(raw_args: &[String]) -> Vec<String> {
    if raw_args.len() == 1 {
        raw_args
            .first()
            .map(|s| s.split_whitespace().map(|w| w.to_owned()).collect())
            .unwrap_or_default()
    } else {
        raw_args.to_vec()
    }
}

/// Strip a leading `"--"` separator and return the remaining words.
///
/// Equivalent to the `skip_while(|s| *s == "--")` step inside `build_forwarded_args`
/// in the baseline `apps/cli/src/commands/make.rs`.
fn strip_leading_separator(raw_args: &[String]) -> Vec<String> {
    let words = raw_args_to_words(raw_args);
    words.into_iter().skip_while(|s| s == "--").collect()
}

fn strip_leading_separator_shell(raw_args: &[String]) -> Result<Vec<String>, String> {
    let words = raw_args_to_shell_words(raw_args)?;
    Ok(words.into_iter().skip_while(|s| s == "--").collect())
}

fn raw_args_to_shell_words(raw_args: &[String]) -> Result<Vec<String>, String> {
    if raw_args.len() == 1 {
        let single =
            raw_args.first().ok_or_else(|| "internal error: missing raw argument".to_owned())?;
        split_shell_words(single)
    } else {
        Ok(raw_args.to_vec())
    }
}

fn split_shell_words(input: &str) -> Result<Vec<String>, String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut in_word = false;
    let mut chars = input.chars();

    while let Some(ch) = chars.next() {
        match (quote, ch) {
            (None, c) if c.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut current));
                    in_word = false;
                }
            }
            (None, '\'' | '"') => {
                quote = Some(ch);
                in_word = true;
            }
            (None, '\\') => {
                if let Some(next) = chars.next() {
                    current.push(next);
                } else {
                    current.push(ch);
                }
                in_word = true;
            }
            (Some('"'), '\\') => {
                if let Some(next) = chars.next() {
                    if matches!(next, '$' | '`' | '"' | '\\' | '\n') {
                        current.push(next);
                    } else {
                        current.push(ch);
                        current.push(next);
                    }
                } else {
                    current.push(ch);
                }
                in_word = true;
            }
            (Some(q), c) if c == q => {
                quote = None;
            }
            (_, c) => {
                current.push(c);
                in_word = true;
            }
        }
    }

    if quote.is_some() {
        return Err("error: unterminated quoted argument".to_owned());
    }
    if in_word {
        words.push(current);
    }
    Ok(words)
}

/// Build the sotp argv for a forwarding dispatcher: `prefix` tokens followed by
/// user args with any leading `"--"` separator stripped.
///
/// Mirrors `build_forwarded_args` in the baseline `apps/cli/src/commands/make.rs`.
fn build_forwarded_args(prefix: &[&str], raw_args: &[String]) -> Vec<String> {
    let filtered = strip_leading_separator(raw_args);
    let mut args: Vec<String> = prefix.iter().map(|s| (*s).to_owned()).collect();
    args.extend(filtered);
    args
}

/// Build the sotp argv for `track-set-override` / `track-clear-override`.
///
/// Finds the first positional (non-flag, non-flag-value) word in `raw_args` as
/// the status, then routes:
/// - `"clear"` → `["track", "clear-override", "--items-dir", "track/items", <rest>]`
/// - other     → `["track", "set-override",   "--items-dir", "track/items", status, <rest>]`
///
/// Only **value-taking** flags (`--track-id`, `--reason`) consume the next token as their
/// value.  All other flags are treated as boolean flags.  The status word is removed by
/// **index** (not by value), so a flag value that happens to equal the status string is
/// never silently dropped.
///
/// # Errors
///
/// Returns `Err` if no positional word is found (missing status argument).
fn build_set_override_args(raw_args: &[String]) -> Result<Vec<String>, String> {
    let words = raw_args_to_words(raw_args);
    let filtered: Vec<&str> = words.iter().map(|s| s.as_str()).skip_while(|s| *s == "--").collect();
    let usage = "error: usage: sotp make track-set-override <blocked|cancelled|clear> [--track-id <id>] [--reason <text>]";
    // Only these flags take a value argument; boolean flags do not consume the next token.
    const VALUE_FLAGS: &[&str] = &["--track-id", "--reason"];
    let mut status_idx: Option<usize> = None;
    let mut skip_next = false;
    for (i, word) in filtered.iter().enumerate() {
        if skip_next {
            skip_next = false;
            continue;
        }
        if VALUE_FLAGS.contains(word) {
            skip_next = true;
        } else if !word.starts_with('-') {
            status_idx = Some(i);
            break;
        }
    }
    let status_idx = status_idx.ok_or_else(|| usage.to_owned())?;
    let status = filtered.get(status_idx).ok_or_else(|| usage.to_owned())?;
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

impl CliApp {
    /// Run CI then commit with the given message.
    ///
    /// # Errors
    /// Returns `Err` when the CI or commit step fails.
    pub fn make_commit(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let message = raw_args_to_single(&raw_args)
            .map_err(|_| "error: commit message required".to_owned())?;
        // Run CI first
        let ci = run_command("cargo", &["make", "ci"])?;
        if ci.exit_code != 0 {
            return Ok(ci);
        }
        run_command("git", &["commit", "-m", &message])
    }

    /// Attach a git note to HEAD.
    ///
    /// # Errors
    /// Returns `Err` when the git notes command fails.
    pub fn make_note(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let note_text =
            raw_args_to_single(&raw_args).map_err(|_| "error: note text required".to_owned())?;
        run_command("git", &["notes", "add", "-f", "-m", &note_text, "HEAD"])
    }

    /// Run CI then commit using tmp/track-commit/commit-message.txt.
    ///
    /// # Errors
    /// Returns `Err` when the CI, review guard, or commit step fails.
    pub fn make_track_commit_message(
        &self,
        _raw_args: Vec<String>,
    ) -> Result<CommandOutcome, String> {
        std::fs::create_dir_all("tmp").map_err(|e| format!("mkdir tmp failed: {e}"))?;

        eprintln!("[track-commit-message] Pre-commit: staging working tree...");
        let add_result = self.git_add_all()?;
        if add_result.exit_code != 0 {
            eprintln!("[track-commit-message] BLOCKED: git add-all failed");
            return Ok(add_result);
        }

        eprintln!("[track-commit-message] Running CI...");
        let log_file = std::fs::File::create("tmp/ci-output.log")
            .map_err(|e| format!("failed to create tmp/ci-output.log: {e}"))?;
        let log_file_err =
            log_file.try_clone().map_err(|e| format!("failed to clone log file handle: {e}"))?;
        let ci_status = std::process::Command::new("cargo")
            .args(["make", "ci"])
            .stdout(log_file)
            .stderr(log_file_err)
            .status()
            .map_err(|e| e.to_string())?;

        if !ci_status.success() {
            let ci_exit = ci_status.code().unwrap_or(1);
            eprintln!("[track-commit-message] CI FAILED (exit {ci_exit}). Last 20 lines:");
            if let Ok(content) = std::fs::read_to_string("tmp/ci-output.log") {
                let lines: Vec<&str> = content.lines().collect();
                let start = lines.len().saturating_sub(20);
                for line in lines.get(start..).unwrap_or_default() {
                    eprintln!("{line}");
                }
            }
            return Ok(CommandOutcome {
                stdout: None,
                stderr: None,
                exit_code: u8::try_from(ci_exit).unwrap_or(1),
            });
        }
        eprintln!("[track-commit-message] CI PASSED");

        let track_id = self.current_branch_track_id_strict()?.ok_or_else(|| {
            "[track-commit-message] BLOCKED: not on a track/<id> branch; \
             check-approved guard requires a track branch. \
             Switch to your track branch."
                .to_owned()
        })?;
        eprintln!("[track-commit-message] Checking review approval for track '{track_id}'...");
        let guard_result =
            self.review_check_approved(Some(track_id.clone()), PathBuf::from("track/items"))?;
        if guard_result.exit_code != 0 {
            eprintln!("[track-commit-message] BLOCKED: review guard rejected commit");
            return Ok(guard_result);
        }
        eprintln!("[track-commit-message] Review approved");

        let commit_result = self.git_commit_from_file(
            PathBuf::from("tmp/track-commit/commit-message.txt"),
            true,
            None,
        )?;
        if commit_result.exit_code != 0 {
            return Ok(commit_result);
        }

        // Post-commit: persist HEAD SHA to .commit_hash
        let mut post_commit_failed = false;
        if let Ok(Some(ref tid)) = self.current_branch_track_id_strict() {
            if let Err(msg) = crate::review_v2::persist_commit_hash_for_track(tid) {
                eprintln!("[track-commit-message] WARNING: .commit_hash persistence failed: {msg}");
                eprintln!(
                    "[track-commit-message] Recovery: run `bin/sotp make track-set-commit-hash \
                     {tid}` to set the v2 diff base manually."
                );
                post_commit_failed = true;
            }
        }

        if post_commit_failed {
            eprintln!("[track-commit-message] COMMIT_OK but post-commit steps failed (see above)");
            return Ok(CommandOutcome { stdout: None, stderr: None, exit_code: 3 });
        }
        Ok(CommandOutcome::success(None))
    }

    /// Create a track branch from main.
    ///
    /// # Errors
    /// Returns `Err` when branch creation fails.
    pub fn make_track_branch_create(
        &self,
        raw_args: Vec<String>,
    ) -> Result<CommandOutcome, String> {
        let track_id = raw_args_to_single(&raw_args)
            .map_err(|_| "error: track-id argument required".to_owned())?;
        run_sotp(&["track", "branch", "create", "--items-dir", "track/items", &track_id])
    }

    /// Switch to an existing track branch.
    ///
    /// # Errors
    /// Returns `Err` when branch switching fails.
    pub fn make_track_branch_switch(
        &self,
        raw_args: Vec<String>,
    ) -> Result<CommandOutcome, String> {
        let track_id = raw_args_to_single(&raw_args)
            .map_err(|_| "error: track-id argument required".to_owned())?;
        run_sotp(&["track", "branch", "switch", "--items-dir", "track/items", &track_id])
    }

    /// Resolve current track phase.
    ///
    /// # Errors
    /// Returns `Err` when track resolution fails.
    pub fn make_track_resolve(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let args = build_forwarded_args(&["track", "resolve"], &raw_args);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        run_sotp(&refs)
    }

    /// Push current track/plan branch to origin.
    ///
    /// # Errors
    /// Returns `Err` when push fails.
    pub fn make_track_pr_push(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let words = strip_leading_separator(&raw_args);
        let mut args: Vec<&str> = vec!["pr", "push"];
        match words.first() {
            Some(first) if !first.starts_with("--") => {
                // Legacy positional track id: promote to --track-id.
                args.extend_from_slice(&["--track-id", first]);
                for w in words.get(1..).unwrap_or_default() {
                    args.push(w);
                }
            }
            _ => {
                // Flag-first or empty: forward everything verbatim; clap resolves track-id.
                for w in &words {
                    args.push(w);
                }
            }
        }
        run_sotp(&args)
    }

    /// Create or reuse a PR for the current branch.
    ///
    /// # Errors
    /// Returns `Err` when the `sotp pr ensure-pr` invocation fails.
    pub fn make_track_pr_ensure(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let words = strip_leading_separator(&raw_args);
        let mut args: Vec<&str> = vec!["pr", "ensure-pr"];
        match words.first() {
            Some(first) if !first.starts_with("--") => {
                // Legacy positional track id: promote to --track-id.
                args.extend_from_slice(&["--track-id", first]);
                for w in words.get(1..).unwrap_or_default() {
                    args.push(w);
                }
            }
            _ => {
                // Flag-first or empty: forward everything verbatim; clap resolves track-id.
                for w in &words {
                    args.push(w);
                }
            }
        }
        run_sotp(&args)
    }

    /// Push + ensure PR in one step.
    ///
    /// # Errors
    /// Returns `Err` when push or PR creation fails.
    pub fn make_track_pr(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        // push + ensure-pr in one step
        let push_result = self.make_track_pr_push(raw_args.clone())?;
        if push_result.exit_code != 0 {
            return Ok(push_result);
        }
        self.make_track_pr_ensure(raw_args)
    }

    /// Run full PR review cycle.
    ///
    /// # Errors
    /// Returns `Err` when the review cycle fails.
    pub fn make_track_pr_review(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let args = build_forwarded_args(&["pr", "review-cycle"], &raw_args);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        run_sotp(&refs)
    }

    /// Wait for PR checks then merge.
    ///
    /// # Errors
    /// Returns `Err` when the `sotp pr wait-and-merge` invocation fails.
    pub fn make_track_pr_merge(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let words = strip_leading_separator(&raw_args);
        let mut args: Vec<&str> = vec!["pr", "wait-and-merge"];
        for w in &words {
            args.push(w);
        }
        run_sotp(&args)
    }

    /// Show PR check status.
    ///
    /// # Errors
    /// Returns `Err` when status retrieval fails.
    pub fn make_track_pr_status(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let args = build_forwarded_args(&["pr", "status"], &raw_args);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        run_sotp(&refs)
    }

    /// Run the local Codex planner.
    ///
    /// # Errors
    /// Returns `Err` when the planner invocation fails.
    pub fn make_track_local_plan(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let words = strip_leading_separator(&raw_args);
        let mut args: Vec<&str> = vec!["plan", "codex-local"];
        let word_refs: Vec<&str> = words.iter().map(String::as_str).collect();
        args.extend_from_slice(&word_refs);
        run_sotp(&args)
    }

    /// Run the local Codex reviewer.
    ///
    /// # Errors
    /// Returns `Err` when the reviewer invocation fails.
    pub fn make_track_local_review(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let words = strip_leading_separator(&raw_args);
        let mut args: Vec<&str> = vec!["review", "local"];
        let word_refs: Vec<&str> = words.iter().map(String::as_str).collect();
        args.extend_from_slice(&word_refs);
        run_sotp(&args)
    }

    /// Forward args to `sotp review fix-local`.
    /// # Errors
    /// Returns `Err` when the fixer invocation fails.
    pub fn make_track_local_review_fix_codex(
        &self,
        raw_args: Vec<String>,
    ) -> Result<CommandOutcome, String> {
        let words = strip_leading_separator_shell(&raw_args)?;
        let mut args: Vec<&str> = vec!["review", "fix-local"];
        let word_refs: Vec<&str> = words.iter().map(String::as_str).collect();
        args.extend_from_slice(&word_refs);
        run_sotp(&args)
    }

    /// Forward args to `sotp dry fix-local`.
    /// # Errors
    /// Returns `Err` when the fixer invocation fails.
    pub fn make_track_local_dry_fix(
        &self,
        raw_args: Vec<String>,
    ) -> Result<CommandOutcome, String> {
        let words = strip_leading_separator_shell(&raw_args)?;
        let mut args: Vec<&str> = vec!["dry", "fix-local"];
        let word_refs: Vec<&str> = words.iter().map(String::as_str).collect();
        args.extend_from_slice(&word_refs);
        run_sotp(&args)
    }

    /// Show per-scope review results.
    ///
    /// # Errors
    /// Returns `Err` when results retrieval fails.
    pub fn make_track_review_results(
        &self,
        raw_args: Vec<String>,
    ) -> Result<CommandOutcome, String> {
        let args = build_forwarded_args(&["review", "results"], &raw_args);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        run_sotp(&refs)
    }

    /// Check that the review state is approved and code hash is current.
    ///
    /// # Errors
    /// Returns `Err` when the approval check fails.
    pub fn make_track_check_approved(
        &self,
        raw_args: Vec<String>,
    ) -> Result<CommandOutcome, String> {
        let args = build_forwarded_args(&["review", "check-approved"], &raw_args);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        run_sotp(&refs)
    }

    /// Switch to main branch and pull latest.
    ///
    /// # Errors
    /// Returns `Err` when switching or pulling fails.
    pub fn make_track_switch_main(&self, _raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        run_sotp(&["git", "switch-and-pull", "main"])
    }

    /// Stage paths from tmp/track-commit/add-paths.txt.
    ///
    /// # Errors
    /// Returns `Err` when staging fails.
    pub fn make_track_add_paths(&self, _raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        run_sotp(&["git", "add-from-file", "tmp/track-commit/add-paths.txt", "--cleanup"])
    }

    /// Transition a task status.
    ///
    /// # Errors
    /// Returns `Err` when the transition fails.
    pub fn make_track_transition(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let args =
            build_forwarded_args(&["track", "transition", "--items-dir", "track/items"], &raw_args);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        run_sotp(&refs)
    }

    /// Add a new task to a track.
    ///
    /// # Errors
    /// Returns `Err` when the add-task operation fails.
    pub fn make_track_add_task(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let args =
            build_forwarded_args(&["track", "add-task", "--items-dir", "track/items"], &raw_args);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        run_sotp(&refs)
    }

    /// Show the next open task (JSON).
    ///
    /// # Errors
    /// Returns `Err` when task retrieval fails.
    pub fn make_track_next_task(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let args =
            build_forwarded_args(&["track", "next-task", "--items-dir", "track/items"], &raw_args);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        run_sotp(&refs)
    }

    /// Show task status counts (JSON).
    ///
    /// # Errors
    /// Returns `Err` when counts retrieval fails.
    pub fn make_track_task_counts(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let args = build_forwarded_args(
            &["track", "task-counts", "--items-dir", "track/items"],
            &raw_args,
        );
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        run_sotp(&refs)
    }

    /// Set or clear a status override.
    ///
    /// # Errors
    /// Returns `Err` when the override operation fails.
    pub fn make_track_set_override(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let args = build_set_override_args(&raw_args)?;
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        run_sotp(&refs)
    }

    /// Render plan.md and registry.md from metadata.json.
    ///
    /// # Errors
    /// Returns `Err` when the sync-views operation fails.
    pub fn make_track_sync_views(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let args =
            build_forwarded_args(&["track", "views", "sync", "--project-root", "."], &raw_args);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        run_sotp(&refs)
    }

    /// Attach git note from tmp/track-commit/note.md.
    ///
    /// # Errors
    /// Returns `Err` when the note attachment fails.
    pub fn make_track_note(&self, _raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        run_sotp(&["git", "note-from-file", "tmp/track-commit/note.md", "--cleanup"])
    }

    /// Write current HEAD SHA to .commit_hash (set v2 diff base).
    ///
    /// # Errors
    /// Returns `Err` when writing the commit hash fails.
    pub fn make_track_set_commit_hash(
        &self,
        raw_args: Vec<String>,
    ) -> Result<CommandOutcome, String> {
        let track_id = raw_args_to_single(&raw_args)
            .map_err(|_| "usage: track-set-commit-hash <track-id>".to_owned())?;
        match crate::review_v2::persist_commit_hash_for_track(&track_id) {
            Ok(sha) => Ok(CommandOutcome::success(Some(format!("Recorded .commit_hash: {sha}")))),
            Err(msg) => Ok(CommandOutcome::failure(Some(msg))),
        }
    }

    /// Stage all worktree changes.
    ///
    /// # Errors
    /// Returns `Err` when staging fails.
    pub fn make_add_all(&self, _raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        run_sotp(&["git", "add-all"])
    }

    /// Unstage paths (remove from index without discarding worktree changes).
    ///
    /// # Errors
    /// Returns `Err` when unstaging fails.
    pub fn make_unstage(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        if raw_args.is_empty() {
            return Err("error: at least one path required".to_owned());
        }
        let words = raw_args_to_words(&raw_args);
        let mut sotp_args: Vec<&str> = vec!["git", "unstage", "--"];
        sotp_args.extend(words.iter().map(String::as_str));
        run_sotp(&sotp_args)
    }

    /// Run a cargo make task via tools-daemon exec with WORKER_ID isolation.
    ///
    /// # Errors
    /// Returns `Err` when the exec invocation fails.
    pub fn make_exec(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let words = raw_args_to_words(&raw_args);
        if words.is_empty() {
            return Err("error: usage: sotp make exec <local-task-name>".to_owned());
        }
        let task_name = words.first().ok_or_else(|| "missing task name".to_owned())?;
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
        for extra in words.get(1..).unwrap_or_default() {
            args.push(extra.clone());
        }
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        run_command("docker", &arg_refs)
    }
}

/// Run an external command and return a CommandOutcome.
fn run_command(program: &str, args: &[&str]) -> Result<CommandOutcome, String> {
    let status = std::process::Command::new(program)
        .args(args)
        .status()
        .map_err(|e| format!("failed to run {program}: {e}"))?;
    let code = u8::try_from(status.code().unwrap_or(1)).unwrap_or(1);
    Ok(CommandOutcome { stdout: None, stderr: None, exit_code: code })
}

/// Run the sotp binary with the given args.
fn run_sotp(args: &[&str]) -> Result<CommandOutcome, String> {
    run_command("bin/sotp", args)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::fs;

    use super::{
        build_forwarded_args, build_set_override_args, strip_leading_separator,
        strip_leading_separator_shell,
    };

    struct CwdGuard(std::path::PathBuf);
    impl Drop for CwdGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.0);
        }
    }

    #[cfg(unix)]
    fn make_executable(script: &std::path::Path) {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(script).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(script, perms).unwrap();
    }

    #[cfg(unix)]
    fn write_fake_sotp(root: &std::path::Path) {
        let bin_dir = root.join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        let script = bin_dir.join("sotp");
        fs::write(&script, "#!/bin/sh\nprintf '%s\\n' \"$@\" > sotp-args.txt\nexit 0\n").unwrap();
        make_executable(&script);
    }

    // --- build_forwarded_args tests ---

    #[test]
    fn test_build_forwarded_args_prepends_prefix() {
        let raw = vec!["--track-id my-track --round-type fast".to_owned()];
        let args = build_forwarded_args(&["review", "results"], &raw);
        assert_eq!(args.first().map(String::as_str), Some("review"));
        assert_eq!(args.get(1).map(String::as_str), Some("results"));
        assert_eq!(args.get(2).map(String::as_str), Some("--track-id"));
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
    fn test_review_fix_passthrough_preserves_quoted_paths() {
        let raw = vec!["-- --briefing-file '/tmp/a b.md' --scope-files 'apps/x y.rs'".to_owned()];
        let words = strip_leading_separator_shell(&raw).unwrap();
        assert_eq!(words, vec!["--briefing-file", "/tmp/a b.md", "--scope-files", "apps/x y.rs"]);
    }

    #[test]
    fn test_review_fix_passthrough_preserves_backslash_in_double_quotes() {
        let raw = vec!["--briefing-file \"tmp/a\\b.md\"".to_owned()];
        let words = strip_leading_separator_shell(&raw).unwrap();
        assert_eq!(words, vec!["--briefing-file", "tmp/a\\b.md"]);
    }

    #[cfg(unix)]
    #[test]
    fn test_make_track_local_review_fix_codex_forwards_shell_words_to_sotp() {
        let _lock = crate::test_support::process_env_lock().lock().unwrap();

        let dir = tempfile::tempdir().unwrap();
        write_fake_sotp(dir.path());
        let _cwd_guard = CwdGuard(std::env::current_dir().unwrap());
        std::env::set_current_dir(dir.path()).unwrap();

        let outcome = crate::CliApp::new()
            .make_track_local_review_fix_codex(vec![
                "-- --briefing-file 'tmp/a b.md' --scope cli_composition".to_owned(),
            ])
            .unwrap();

        assert_eq!(outcome.exit_code, 0);
        let recorded = fs::read_to_string(dir.path().join("sotp-args.txt")).unwrap();
        let args: Vec<&str> = recorded.lines().collect();
        assert_eq!(
            args,
            vec![
                "review",
                "fix-local",
                "--briefing-file",
                "tmp/a b.md",
                "--scope",
                "cli_composition"
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_make_track_local_dry_fix_forwards_shell_words_to_sotp() {
        let _lock = crate::test_support::process_env_lock().lock().unwrap();

        let dir = tempfile::tempdir().unwrap();
        write_fake_sotp(dir.path());
        let _cwd_guard = CwdGuard(std::env::current_dir().unwrap());
        std::env::set_current_dir(dir.path()).unwrap();

        let outcome = crate::CliApp::new()
            .make_track_local_dry_fix(vec![
                "-- --briefing-file 'tmp/a b.md' --track-id dry-track".to_owned(),
            ])
            .unwrap();

        assert_eq!(outcome.exit_code, 0);
        let recorded = fs::read_to_string(dir.path().join("sotp-args.txt")).unwrap();
        let args: Vec<&str> = recorded.lines().collect();
        assert_eq!(
            args,
            vec!["dry", "fix-local", "--briefing-file", "tmp/a b.md", "--track-id", "dry-track"]
        );
    }

    // --- pr_push / pr_ensure guard logic tests ---
    //
    // These tests exercise the non-flag-first guard: a first word that does not
    // start with "--" is promoted to "--track-id <word> <rest>"; a flag-first or
    // empty word list is forwarded verbatim.

    #[test]
    fn test_pr_push_guard_positional_first_word_promoted() {
        // Legacy positional: "my-track" → "--track-id my-track"
        let words = strip_leading_separator(&["my-track".to_owned()]);
        let mut args: Vec<&str> = vec!["pr", "push"];
        match words.first() {
            Some(first) if !first.starts_with("--") => {
                args.extend_from_slice(&["--track-id", first]);
                for w in words.get(1..).unwrap_or_default() {
                    args.push(w);
                }
            }
            _ => {
                for w in &words {
                    args.push(w);
                }
            }
        }
        assert_eq!(args, vec!["pr", "push", "--track-id", "my-track"]);
    }

    #[test]
    fn test_pr_push_guard_flag_first_forwarded_verbatim() {
        // Flag-first: "--base release" must not be promoted to "--track-id --base release".
        let words = strip_leading_separator(&["--base release".to_owned()]);
        let mut args: Vec<&str> = vec!["pr", "push"];
        match words.first() {
            Some(first) if !first.starts_with("--") => {
                args.extend_from_slice(&["--track-id", first]);
                for w in words.get(1..).unwrap_or_default() {
                    args.push(w);
                }
            }
            _ => {
                for w in &words {
                    args.push(w);
                }
            }
        }
        assert_eq!(args, vec!["pr", "push", "--base", "release"]);
    }

    #[test]
    fn test_pr_push_guard_explicit_track_id_flag_forwarded_verbatim() {
        // Explicit "--track-id foo" must not be doubled to
        // "--track-id --track-id foo".
        let words = strip_leading_separator(&["--track-id foo".to_owned()]);
        let mut args: Vec<&str> = vec!["pr", "push"];
        match words.first() {
            Some(first) if !first.starts_with("--") => {
                args.extend_from_slice(&["--track-id", first]);
                for w in words.get(1..).unwrap_or_default() {
                    args.push(w);
                }
            }
            _ => {
                for w in &words {
                    args.push(w);
                }
            }
        }
        assert_eq!(args, vec!["pr", "push", "--track-id", "foo"]);
    }

    #[test]
    fn test_pr_push_guard_empty_forwarded_verbatim() {
        // Empty args: no "--track-id" injected.
        let words: Vec<String> = vec![];
        let mut args: Vec<&str> = vec!["pr", "push"];
        match words.first() {
            Some(first) if !first.starts_with("--") => {
                args.extend_from_slice(&["--track-id", first]);
                for w in words.get(1..).unwrap_or_default() {
                    args.push(w);
                }
            }
            _ => {
                for w in &words {
                    args.push(w);
                }
            }
        }
        assert_eq!(args, vec!["pr", "push"]);
    }

    // --- build_set_override_args tests ---

    #[test]
    fn test_build_set_override_args_blocked_no_flags() {
        let raw = vec!["blocked".to_owned()];
        let args = build_set_override_args(&raw).unwrap();
        assert_eq!(args, vec!["track", "set-override", "--items-dir", "track/items", "blocked"]);
    }

    #[test]
    fn test_build_set_override_args_clear_routes_to_clear_override() {
        let raw = vec!["clear".to_owned()];
        let args = build_set_override_args(&raw).unwrap();
        assert_eq!(args, vec!["track", "clear-override", "--items-dir", "track/items"]);
    }

    #[test]
    fn test_build_set_override_args_status_after_flags() {
        // Flags before status: "--track-id my-track blocked" →
        // status is "blocked"; --track-id and its value are in rest.
        let raw = vec!["--track-id my-track blocked".to_owned()];
        let args = build_set_override_args(&raw).unwrap();
        assert_eq!(
            args,
            vec![
                "track",
                "set-override",
                "--items-dir",
                "track/items",
                "blocked",
                "--track-id",
                "my-track"
            ]
        );
    }

    #[test]
    fn test_build_set_override_args_reason_with_same_word_as_status_not_dropped() {
        // Status word removed by index (not by value): --reason blocked must survive.
        let raw = vec!["blocked --reason blocked".to_owned()];
        let args = build_set_override_args(&raw).unwrap();
        assert_eq!(
            args,
            vec![
                "track",
                "set-override",
                "--items-dir",
                "track/items",
                "blocked",
                "--reason",
                "blocked"
            ]
        );
    }

    #[test]
    fn test_build_set_override_args_clear_with_track_id() {
        let raw = vec!["clear --track-id my-track".to_owned()];
        let args = build_set_override_args(&raw).unwrap();
        assert_eq!(
            args,
            vec!["track", "clear-override", "--items-dir", "track/items", "--track-id", "my-track"]
        );
    }

    #[test]
    fn test_build_set_override_args_missing_status_returns_error() {
        let raw = vec!["--track-id my-track".to_owned()];
        assert!(build_set_override_args(&raw).is_err());
    }

    #[test]
    fn test_build_set_override_args_boolean_flag_before_status() {
        // Unknown boolean flag (--verbose) must not consume the next token as its value.
        let raw = vec!["--verbose blocked".to_owned()];
        let args = build_set_override_args(&raw).unwrap();
        assert_eq!(
            args,
            vec!["track", "set-override", "--items-dir", "track/items", "blocked", "--verbose"]
        );
    }
}
