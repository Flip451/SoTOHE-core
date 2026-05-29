//! `make` command family — CliApp impl methods.
//!
//! Each method receives `raw_args: Vec<String>` forwarded from cargo-make and
//! delegates in-process to the corresponding `CliApp::*` method or infrastructure
//! layer.  No subprocess (`bin/sotp`) is spawned — `cli-composition` is a library.

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
fn strip_leading_separator(raw_args: &[String]) -> Vec<String> {
    let words = raw_args_to_words(raw_args);
    words.into_iter().skip_while(|s| s == "--").collect()
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
        self.track_branch_create(PathBuf::from("track/items"), track_id)
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
        self.track_branch_switch(PathBuf::from("track/items"), track_id)
    }

    /// Resolve current track phase.
    ///
    /// # Errors
    /// Returns `Err` when track resolution fails.
    pub fn make_track_resolve(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let words = strip_leading_separator(&raw_args);
        let track_id = words.iter().find(|w| !w.starts_with('-')).cloned();
        self.track_resolve(PathBuf::from("track/items"), track_id)
    }

    /// Push current track/plan branch to origin.
    ///
    /// # Errors
    /// Returns `Err` when push fails.
    pub fn make_track_pr_push(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let words = strip_leading_separator(&raw_args);
        let track_id = words.first().cloned();
        self.pr_push(track_id)
    }

    /// Create or reuse a PR for the current branch.
    ///
    /// # Errors
    /// Returns `Err` when the `sotp pr ensure-pr` invocation fails.
    pub fn make_track_pr_ensure(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let words = strip_leading_separator(&raw_args);
        let mut args: Vec<&str> = vec!["pr", "ensure-pr"];
        // Only treat the first word as a positional track-id when it is not a
        // flag (does not start with "--").  Flag-only invocations such as
        // `-- --base release` must flow through unchanged so clap can parse them.
        let start = if words.first().is_some_and(|w| !w.starts_with("--")) {
            if let Some(track_id) = words.first() {
                args.extend_from_slice(&["--track-id", track_id]);
            }
            1
        } else {
            0
        };
        // Forward remaining args so clap rejects unexpected ones.
        for w in words.get(start..).unwrap_or_default() {
            args.push(w);
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
        let words = strip_leading_separator(&raw_args);
        let track_id = require_flag_value(&words, "--track-id")?
            .or_else(|| words.first().filter(|w| !w.starts_with('-')).cloned());
        let resume = words.iter().any(|w| w == "--resume");
        self.pr_review_cycle(track_id, resume)
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
        let words = strip_leading_separator(&raw_args);
        let pr = words.first().cloned().unwrap_or_default();
        self.pr_status(pr)
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

    /// Show per-scope review results.
    ///
    /// # Errors
    /// Returns `Err` when results retrieval fails.
    pub fn make_track_review_results(
        &self,
        raw_args: Vec<String>,
    ) -> Result<CommandOutcome, String> {
        let words = strip_leading_separator(&raw_args);
        let items_dir = PathBuf::from("track/items");
        let track_id = require_flag_value(&words, "--track-id")?;
        let scope = require_flag_value(&words, "--scope")?;
        let round_type = match require_flag_value(&words, "--round-type")? {
            None => "any".to_owned(),
            Some(v) => validate_round_type(v)?,
        };
        let limit = match require_flag_value(&words, "--limit")? {
            None => 0u32,
            Some(v) => parse_limit_arg(&v)?,
        };
        let all = words.iter().any(|w| w == "--all");
        let no_hint = words.iter().any(|w| w == "--no-hint");
        self.review_results(crate::ReviewResultsInput {
            track_id,
            items_dir,
            scope,
            all,
            limit,
            round_type,
            no_hint,
        })
    }

    /// Check that the review state is approved and code hash is current.
    ///
    /// # Errors
    /// Returns `Err` when the approval check fails.
    pub fn make_track_check_approved(
        &self,
        raw_args: Vec<String>,
    ) -> Result<CommandOutcome, String> {
        let words = strip_leading_separator(&raw_args);
        let items_dir = PathBuf::from("track/items");
        let track_id = require_flag_value(&words, "--track-id")?;
        self.review_check_approved(track_id, items_dir)
    }

    /// Switch to main branch and pull latest.
    ///
    /// # Errors
    /// Returns `Err` when switching or pulling fails.
    pub fn make_track_switch_main(&self, _raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        self.git_switch_and_pull("main".to_owned())
    }

    /// Stage paths from tmp/track-commit/add-paths.txt.
    ///
    /// # Errors
    /// Returns `Err` when staging fails.
    pub fn make_track_add_paths(&self, _raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        self.git_add_from_file(PathBuf::from("tmp/track-commit/add-paths.txt"), true)
    }

    /// Transition a task status.
    ///
    /// # Errors
    /// Returns `Err` when the transition fails.
    pub fn make_track_transition(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let words = strip_leading_separator(&raw_args);
        let items_dir = PathBuf::from("track/items");
        let track_id = require_flag_value(&words, "--track-id")?;
        let commit_hash = require_flag_value(&words, "--commit-hash")?;
        // Positional args: task_id, target_status (flag values must be excluded)
        let positional = extract_positionals(&words);
        let task_id = positional.first().cloned().unwrap_or_default();
        let target_status = positional.get(1).cloned().unwrap_or_default();
        self.track_transition(items_dir, track_id, task_id, target_status, commit_hash)
    }

    /// Add a new task to a track.
    ///
    /// # Errors
    /// Returns `Err` when the add-task operation fails.
    pub fn make_track_add_task(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let words = strip_leading_separator(&raw_args);
        let items_dir = PathBuf::from("track/items");
        let track_id = require_flag_value(&words, "--track-id")?;
        let section = require_flag_value(&words, "--section")?;
        let after = require_flag_value(&words, "--after")?;
        // First positional arg is the description (flag values must be excluded)
        let positional = extract_positionals(&words);
        let description = positional.first().cloned().unwrap_or_default();
        self.track_add_task(items_dir, track_id, description, section, after)
    }

    /// Show the next open task (JSON).
    ///
    /// # Errors
    /// Returns `Err` when task retrieval fails.
    pub fn make_track_next_task(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let words = strip_leading_separator(&raw_args);
        let items_dir = PathBuf::from("track/items");
        let track_id = require_flag_value(&words, "--track-id")?;
        self.track_next_task(items_dir, track_id)
    }

    /// Show task status counts (JSON).
    ///
    /// # Errors
    /// Returns `Err` when counts retrieval fails.
    pub fn make_track_task_counts(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let words = strip_leading_separator(&raw_args);
        let items_dir = PathBuf::from("track/items");
        let track_id = require_flag_value(&words, "--track-id")?;
        self.track_task_counts(items_dir, track_id)
    }

    /// Set or clear a status override.
    ///
    /// # Errors
    /// Returns `Err` when the override operation fails.
    pub fn make_track_set_override(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let words = strip_leading_separator(&raw_args);
        let items_dir = PathBuf::from("track/items");
        let track_id = require_flag_value(&words, "--track-id")?;
        let reason = require_flag_value(&words, "--reason")?.unwrap_or_default();
        // First positional arg is the status value (flag values must be excluded)
        let positional = extract_positionals(&words);
        let status = positional.first().cloned().unwrap_or_default();
        if status == "clear" {
            self.track_clear_override(items_dir, track_id)
        } else {
            self.track_set_override(items_dir, track_id, status, reason)
        }
    }

    /// Render plan.md and registry.md from metadata.json.
    ///
    /// # Errors
    /// Returns `Err` when the sync-views operation fails.
    pub fn make_track_sync_views(&self, raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        let words = strip_leading_separator(&raw_args);
        let track_id = require_flag_value(&words, "--track-id")?;
        self.track_views_sync(PathBuf::from("."), track_id)
    }

    /// Attach git note from tmp/track-commit/note.md.
    ///
    /// # Errors
    /// Returns `Err` when the note attachment fails.
    pub fn make_track_note(&self, _raw_args: Vec<String>) -> Result<CommandOutcome, String> {
        self.git_note_from_file(PathBuf::from("tmp/track-commit/note.md"), true)
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
        self.git_add_all()
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
        let paths: Vec<PathBuf> = words.iter().map(PathBuf::from).collect();
        self.git_unstage(paths)
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

// ---------------------------------------------------------------------------
// Subprocess helpers
// ---------------------------------------------------------------------------

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

/// Extract the value following a flag like `--flag-name` from a word list.
///
/// Extract the value following a required flag like `--flag-name` from a word
/// list, failing if the flag is present but has no following value token.
///
/// Returns `Ok(None)` when the flag is absent, `Ok(Some(value))` when the flag
/// is present and has a value, and `Err` when the flag is present but is the
/// last token in the list or the next token is another flag (starts with `"--"`).
fn require_flag_value(words: &[String], flag: &str) -> Result<Option<String>, String> {
    let mut iter = words.iter();
    while let Some(w) = iter.next() {
        if w.as_str() == flag {
            return match iter.next() {
                Some(v) if !v.starts_with("--") => Ok(Some(v.clone())),
                Some(next_flag) => {
                    Err(format!("flag {flag} requires a value but got another flag: {next_flag}"))
                }
                None => Err(format!("flag {flag} requires a value but none was provided")),
            };
        }
    }
    Ok(None)
}

/// Validate the `--round-type` flag value.
///
/// Accepts `"fast"`, `"final"`, or `"any"` (case-sensitive).
/// Returns `Err` for any other value.
///
/// # Errors
///
/// Returns an error string when `value` is not one of the accepted tokens.
fn validate_round_type(value: String) -> Result<String, String> {
    match value.as_str() {
        "fast" | "final" | "any" => Ok(value),
        _ => Err(format!("invalid --round-type value {value:?}: expected one of fast, final, any")),
    }
}

/// Parse the `--limit` flag value.
///
/// Accepts `"all"` (case-insensitive) → `u32::MAX`, a non-negative integer, or
/// returns `Err` for any other string.
///
/// # Errors
///
/// Returns an error string when `value` is neither `"all"` nor a valid `u32`.
fn parse_limit_arg(value: &str) -> Result<u32, String> {
    if value.eq_ignore_ascii_case("all") {
        return Ok(u32::MAX);
    }
    value.parse::<u32>().map_err(|_| {
        format!("invalid --limit value {value:?}: expected a non-negative integer or 'all'")
    })
}

/// Collect positional arguments from a word list, skipping only the known
/// flag tokens (`--track-id`, `--commit-hash`, `--section`, `--after`,
/// `--reason`, `--round-type`) and their immediately following values.
///
/// Tokens that do not match a known flag — even if they start with `-` — are
/// treated as positional arguments and returned.  This preserves free-form
/// values like descriptions or reasons that start with a dash.
///
/// A known flag only consumes the next token as its value when that next token
/// is not itself a known flag.  If the next token is another known flag, the
/// current flag is treated as having no value (the next flag is left in the
/// stream to be processed normally).  This prevents a malformed invocation
/// such as `--section --track-id abc T001` from silently treating `--track-id`
/// as the value of `--section`.
fn extract_positionals(words: &[String]) -> Vec<String> {
    const KNOWN_FLAGS: &[&str] =
        &["--track-id", "--commit-hash", "--section", "--after", "--reason", "--round-type"];
    let mut result = Vec::new();
    let mut iter = words.iter().peekable();
    while let Some(w) = iter.next() {
        if KNOWN_FLAGS.contains(&w.as_str()) {
            // Consume the next token as this flag's value only when it is not
            // itself a known flag (which would indicate a missing value).
            if iter.peek().is_some_and(|next| !KNOWN_FLAGS.contains(&next.as_str())) {
                iter.next();
            }
        } else {
            result.push(w.clone());
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::{extract_positionals, parse_limit_arg, require_flag_value, validate_round_type};

    /// `--limit all` (and case variants) must parse to `u32::MAX` — the sentinel
    /// that the underlying `bin/sotp review results` CLI accepts for "no limit".
    #[test]
    fn test_parse_limit_arg_with_all_returns_u32_max() {
        assert_eq!(parse_limit_arg("all").unwrap(), u32::MAX);
    }

    #[test]
    fn test_parse_limit_arg_with_all_uppercase_returns_u32_max() {
        assert_eq!(parse_limit_arg("ALL").unwrap(), u32::MAX);
    }

    /// A numeric string must parse to the corresponding `u32` value.
    #[test]
    fn test_parse_limit_arg_with_numeric_returns_value() {
        assert_eq!(parse_limit_arg("7").unwrap(), 7u32);
        assert_eq!(parse_limit_arg("0").unwrap(), 0u32);
        assert_eq!(parse_limit_arg("100").unwrap(), 100u32);
    }

    /// Any non-numeric, non-"all" string must return an error.
    #[test]
    fn test_parse_limit_arg_with_invalid_string_returns_error() {
        assert!(parse_limit_arg("foo").is_err());
        assert!(parse_limit_arg("-1").is_err());
        assert!(parse_limit_arg("").is_err());
    }

    /// A known flag and its value must be skipped; other tokens are positional.
    #[test]
    fn test_extract_positionals_skips_known_flags_and_their_values() {
        let words: Vec<String> =
            ["--track-id", "my-track", "T001", "done"].iter().map(|s| s.to_string()).collect();
        let positional = extract_positionals(&words);
        assert_eq!(positional, vec!["T001", "done"]);
    }

    /// A free-form positional that starts with `-` must NOT be dropped.
    #[test]
    fn test_extract_positionals_preserves_dash_prefixed_positionals() {
        let words: Vec<String> =
            ["--track-id", "my-track", "-important-note"].iter().map(|s| s.to_string()).collect();
        let positional = extract_positionals(&words);
        assert_eq!(positional, vec!["-important-note"]);
    }

    /// Unknown flags (not in KNOWN_FLAGS) that start with `--` are returned as positional.
    #[test]
    fn test_extract_positionals_treats_unknown_flags_as_positional() {
        let words: Vec<String> =
            ["--unknown-flag", "value", "positional"].iter().map(|s| s.to_string()).collect();
        let positional = extract_positionals(&words);
        assert_eq!(positional, vec!["--unknown-flag", "value", "positional"]);
    }

    /// When a flag is absent, `require_flag_value` returns `Ok(None)`.
    #[test]
    fn test_require_flag_value_when_flag_absent_returns_ok_none() {
        let words: Vec<String> = ["--other", "val"].iter().map(|s| s.to_string()).collect();
        let result = require_flag_value(&words, "--scope").unwrap();
        assert!(result.is_none());
    }

    /// When a flag is present with a following value, `require_flag_value` returns `Ok(Some(value))`.
    #[test]
    fn test_require_flag_value_when_flag_present_with_value_returns_ok_some() {
        let words: Vec<String> =
            ["--scope", "cli_composition"].iter().map(|s| s.to_string()).collect();
        let result = require_flag_value(&words, "--scope").unwrap();
        assert_eq!(result, Some("cli_composition".to_owned()));
    }

    /// When a flag is present but is the last token (no following value), `require_flag_value` returns `Err`.
    #[test]
    fn test_require_flag_value_when_flag_is_last_token_returns_error() {
        let words: Vec<String> = ["--round-type"].iter().map(|s| s.to_string()).collect();
        let result = require_flag_value(&words, "--round-type");
        assert!(result.is_err(), "flag at end of list must return Err, got: {result:?}");
    }

    /// When `--limit` is the last token, `require_flag_value` must fail, preventing
    /// the silent `--limit 0` fallback that would suppress history.
    #[test]
    fn test_require_flag_value_prevents_silent_limit_fallback() {
        let words: Vec<String> =
            ["--track-id", "my-track", "--limit"].iter().map(|s| s.to_string()).collect();
        let result = require_flag_value(&words, "--limit");
        assert!(result.is_err(), "--limit without value must return Err, got: {result:?}");
    }

    /// When a flag is followed immediately by another flag, `require_flag_value` must
    /// return `Err` rather than mis-parsing the next flag as the value.
    #[test]
    fn test_require_flag_value_when_next_token_is_flag_returns_error() {
        let words: Vec<String> =
            ["--scope", "--limit", "1"].iter().map(|s| s.to_string()).collect();
        let result = require_flag_value(&words, "--scope");
        assert!(
            result.is_err(),
            "--scope followed by another flag must return Err, got: {result:?}"
        );
    }

    /// Valid `--round-type` values must pass through unchanged.
    #[test]
    fn test_validate_round_type_with_valid_values_returns_ok() {
        assert_eq!(validate_round_type("fast".to_owned()).unwrap(), "fast");
        assert_eq!(validate_round_type("final".to_owned()).unwrap(), "final");
        assert_eq!(validate_round_type("any".to_owned()).unwrap(), "any");
    }

    /// A known flag followed immediately by another known flag must NOT consume
    /// the second flag as the value of the first.  This prevents a malformed
    /// invocation (`--section --track-id abc T001`) from silently treating
    /// `--track-id` as the value of `--section` and exposing `abc` and `T001`
    /// as positionals while the track ID is lost.
    #[test]
    fn test_extract_positionals_does_not_consume_adjacent_known_flag_as_value() {
        // Malformed: --section has no value; --track-id is immediately after.
        let words: Vec<String> =
            ["--section", "--track-id", "abc", "T001"].iter().map(|s| s.to_string()).collect();
        let positional = extract_positionals(&words);
        // --section's value is missing; --track-id should NOT be eaten.
        // --track-id then consumes "abc", leaving "T001" as the sole positional.
        assert_eq!(positional, vec!["T001"]);
    }

    /// An unrecognized `--round-type` value must return `Err` (fail-fast),
    /// not silently degrade to "any".
    #[test]
    fn test_validate_round_type_with_unrecognized_value_returns_error() {
        assert!(
            validate_round_type("all".to_owned()).is_err(),
            "\"all\" is not a valid round-type; expected Err"
        );
        assert!(
            validate_round_type("foo".to_owned()).is_err(),
            "\"foo\" is not a valid round-type; expected Err"
        );
        assert!(
            validate_round_type("FAST".to_owned()).is_err(),
            "case variants are not accepted; expected Err"
        );
        assert!(
            validate_round_type(String::new()).is_err(),
            "empty string is not a valid round-type; expected Err"
        );
    }
}
