//! Telemetry wiring for the `cli-composition` composition root.
//!
//! Provides:
//! - `init_tracing_subscriber`: one-shot tracing-subscriber initialisation
//!   (composition root only, per IN-01 / CN-04 / AC-01).
//! - `resolve_telemetry_writer`: branch-bound `TelemetryWriter` construction
//!   (returns `None` on non-`track/*` branches — IN-04 / OS-07 / AC-11).
//! - `emit_track_subcommand`: fire-and-forget `TelemetryEvent::TrackSubcommand` emit.
//! - `emit_non_zero_exit`: fire-and-forget `TelemetryEvent::NonZeroExit` emit.
//! - `emit_gate_eval`: fire-and-forget `TelemetryEvent::GateEval` emit (T005).
//! - `emit_hook_block`: fire-and-forget `TelemetryEvent::HookBlock` emit (T005).
//! - `emit_advisory_hook_fired`: fire-and-forget `TelemetryEvent::AdvisoryHookFired` emit (T005).
//! - `emit_review_round`: fire-and-forget `TelemetryEvent::ReviewRound` emit (T006).
//! - `emit_external_subprocess`: fire-and-forget `TelemetryEvent::ExternalSubprocess` emit (T006).
//! - `now_timestamp`: ISO-8601 UTC timestamp helper.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Instant;

use infrastructure::telemetry::{TelemetryConfig, TelemetryEvent, TelemetryWriter};

// ---------------------------------------------------------------------------
// Tracing subscriber init (once-guard)
// ---------------------------------------------------------------------------

/// Initialises the tracing subscriber exactly once per process.
///
/// Uses `tracing_subscriber::fmt` with `EnvFilter::from_env("RUST_LOG")` so
/// that callers can control the log level via the `RUST_LOG` env variable.
/// The default filter is `"warn"` when `RUST_LOG` is not set, which suppresses
/// INFO-level output from dependency crates (e.g. ort/lance/onnxruntime) that
/// would otherwise contaminate command output (AC-01).
///
/// Safe to call more than once — the `OnceLock` ensures at most one attempt
/// through this function. Additionally uses `try_init()` instead of `init()`
/// so that an already-installed subscriber from another source in the process
/// does not cause a panic; the already-set case is silently ignored.
pub fn init_tracing_subscriber() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        use tracing_subscriber::EnvFilter;
        let filter = EnvFilter::try_from_env("RUST_LOG").unwrap_or_else(|_| EnvFilter::new("warn"));
        // try_init returns Err when a subscriber is already installed; ignore it.
        let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
    });
}

// ---------------------------------------------------------------------------
// Branch-bound TelemetryWriter construction (AC-11 / IN-04 / OS-07)
// ---------------------------------------------------------------------------

/// Resolves the current git branch and constructs a `TelemetryWriter` only
/// when the branch matches `track/<id>`.
///
/// Returns `None` (no file I/O, no writer construction) when:
/// - the current branch is not a `track/*` branch (e.g. `main`), or
/// - git branch resolution fails (git absent, detached HEAD, etc.).
///
/// This satisfies:
/// - AC-11: telemetry never records on non-`track/*` branches.
/// - AC-06: no file open when no event will be emitted (lazy init inside
///   `TelemetryWriter` + None short-circuit here).
///
/// `items_dir` is the `track/items` path used to derive the telemetry output
/// file when `SOTP_TELEMETRY_DIR` is not set (CN-03).
///
/// Returns both the `TelemetryWriter` and the resolved `track_id` so the
/// caller does not need to perform a second branch lookup.
pub fn resolve_telemetry_writer(items_dir: &Path) -> Option<(TelemetryWriter, String)> {
    let (track_id, anchored_items_dir) = resolve_telemetry_context_from_branch(items_dir)?;
    resolve_telemetry_writer_inner(Some(track_id), &anchored_items_dir)
}

/// Constructs a telemetry writer from an already-resolved command track context.
///
/// This avoids re-reading the current branch after a long command finishes. The
/// caller is responsible for resolving / validating the track id at command
/// start.
pub(crate) fn resolve_telemetry_writer_for_track(
    items_dir: &Path,
    track_id: &str,
) -> Option<(TelemetryWriter, String)> {
    let anchored_items_dir = resolve_anchored_items_dir(items_dir)?;
    resolve_telemetry_writer_inner(Some(track_id.to_owned()), &anchored_items_dir)
}

/// Inner implementation of `resolve_telemetry_writer`, accepting a pre-resolved
/// `track_id` so tests can inject `None` (no branch / non-track branch) without
/// requiring a live git repository.
///
/// Kept `#[cfg(test)]`-visible via `pub(crate)` so tests in this module can
/// exercise the kill-switch and non-track-branch code paths directly.
pub(crate) fn resolve_telemetry_writer_inner(
    track_id: Option<String>,
    items_dir: &Path,
) -> Option<(TelemetryWriter, String)> {
    // Non-`track/*` branch (or git error): no writer, no file I/O (AC-11).
    let track_id = track_id?;

    // Kill switch: SOTP_TELEMETRY=0 suppresses writer construction (AC-05 / AC-06).
    let config = TelemetryConfig::from_env();
    if !config.is_enabled() {
        return None;
    }

    // Construct the writer; no file is opened at this point (lazy init).
    let writer = TelemetryWriter::new(config, track_id.clone(), items_dir.to_path_buf());
    Some((writer, track_id))
}

/// Extracts `<id>` from the current git branch when it matches `track/<id>`.
///
/// Git discovery is anchored to the project root derived from `items_dir`
/// (stripping the trailing `track/items` segments) so that non-default
/// `--items-dir` invocations or CLI runs outside the target repository resolve
/// the branch from the correct repository (AC-11 / IN-04).
///
/// Returns `None` for non-`track/*` branches, detached HEAD, or git failures.
/// Git failure is intentionally silent (fire-and-forget: telemetry is disabled
/// if we cannot determine the branch — AC-11).
fn resolve_telemetry_context_from_branch(items_dir: &Path) -> Option<(String, PathBuf)> {
    use infrastructure::git_cli::GitRepository as _;
    use usecase::track_resolution::resolve_track_id_from_branch as resolve_fn;

    // Derive the project root from items_dir so discovery is anchored to the
    // correct repo regardless of the process CWD (P1 fix: was discover()).
    let repo = discover_telemetry_repo(items_dir)?;
    let branch = repo.current_branch().ok().flatten()?;
    let track_id = resolve_fn(Some(&branch)).ok()?;
    let anchored_items_dir = anchor_items_dir_to_repo(items_dir, repo.root());

    Some((track_id, anchored_items_dir))
}

fn resolve_anchored_items_dir(items_dir: &Path) -> Option<PathBuf> {
    use infrastructure::git_cli::GitRepository as _;

    let repo = discover_telemetry_repo(items_dir)?;
    Some(anchor_items_dir_to_repo(items_dir, repo.root()))
}

fn discover_telemetry_repo(items_dir: &Path) -> Option<infrastructure::git_cli::SystemGitRepo> {
    let project_root = crate::track::resolve_project_root(items_dir).ok()?;
    infrastructure::git_cli::SystemGitRepo::discover_from(&project_root).ok()
}

fn anchor_items_dir_to_repo(items_dir: &Path, repo_root: &Path) -> PathBuf {
    if items_dir.is_absolute() { items_dir.to_path_buf() } else { repo_root.join(items_dir) }
}

// ---------------------------------------------------------------------------
// Event emit helpers
// ---------------------------------------------------------------------------

/// Returns an ISO-8601 UTC timestamp string for the current moment.
pub fn now_timestamp() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// Emits a `TelemetryEvent::TrackSubcommand` event via fire-and-forget.
///
/// Suppresses any `TelemetryWriteError` (CN-01 / diagnostic-only).
///
/// # Arguments
/// - `writer`: the writer constructed at startup (skipped if `None`).
/// - `track_id`: the branch-bound track id recorded at startup.
/// - `command`: the subcommand name string (e.g. `"track transition"`).
/// - `exit_code`: the dispatch exit code.
/// - `start`: the `Instant` captured before dispatch (duration is computed here).
pub fn emit_track_subcommand(
    writer: &TelemetryWriter,
    track_id: &str,
    command: &str,
    exit_code: i32,
    start: Instant,
) {
    let duration_ms = start.elapsed().as_millis().try_into().unwrap_or(u64::MAX);
    let event = TelemetryEvent::TrackSubcommand {
        schema_version: 1,
        track_id: track_id.to_string(),
        command: command.to_string(),
        exit_code,
        duration_ms,
        timestamp: now_timestamp(),
    };
    // Fire-and-forget: suppress errors per CN-01.
    let _ = writer.write(event);
}

/// Emits a `TelemetryEvent::NonZeroExit` event via fire-and-forget.
///
/// Called when `exit_code != 0`.  Suppresses any `TelemetryWriteError`
/// (CN-01 / diagnostic-only).
pub fn emit_non_zero_exit(
    writer: &TelemetryWriter,
    track_id: &str,
    command: &str,
    exit_code: i32,
    error_chain: &str,
) {
    let event = TelemetryEvent::NonZeroExit {
        schema_version: 1,
        track_id: track_id.to_string(),
        command: command.to_string(),
        exit_code,
        error_chain: error_chain.to_string(),
        timestamp: now_timestamp(),
    };
    // Fire-and-forget: suppress errors per CN-01.
    let _ = writer.write(event);
}

/// Emits a `TelemetryEvent::GateEval` event via fire-and-forget (T005 / AC-03 / GO-01).
///
/// # Arguments
/// - `writer`: the writer constructed at startup.
/// - `track_id`: the branch-bound track id.
/// - `gate_name`: the verify subcommand name, e.g. `"verify-adr-signals"`.
/// - `verdict`: `"ok"` when exit_code == 0, `"error"` otherwise.
/// - `reason_summary`: short summary of findings (leading findings, ≤ 4096 bytes).
/// - `start`: the `Instant` captured before the gate evaluation.
///
/// Suppresses any `TelemetryWriteError` (CN-01 / diagnostic-only).
pub fn emit_gate_eval(
    writer: &TelemetryWriter,
    track_id: &str,
    gate_name: &str,
    verdict: &str,
    reason_summary: &str,
    start: Instant,
) {
    let duration_ms = start.elapsed().as_millis().try_into().unwrap_or(u64::MAX);
    let event = TelemetryEvent::GateEval {
        schema_version: 1,
        track_id: track_id.to_string(),
        gate_name: gate_name.to_string(),
        verdict: verdict.to_string(),
        reason_summary: reason_summary.to_string(),
        duration_ms,
        timestamp: now_timestamp(),
    };
    // Fire-and-forget: suppress errors per CN-01.
    let _ = writer.write(event);
}

/// Emits a `TelemetryEvent::HookBlock` event via fire-and-forget (T005 / AC-04).
///
/// Called ONLY when the hook blocks (exit code 2).  The allow path must NOT
/// call this function (OS-03: hook allow path has no file IO).
///
/// Suppresses any `TelemetryWriteError` (CN-01 / diagnostic-only).
pub fn emit_hook_block(writer: &TelemetryWriter, track_id: &str, hook_name: &str) {
    let event = TelemetryEvent::HookBlock {
        schema_version: 1,
        track_id: track_id.to_string(),
        hook_name: hook_name.to_string(),
        timestamp: now_timestamp(),
    };
    // Fire-and-forget: suppress errors per CN-01.
    let _ = writer.write(event);
}

/// Emits a `TelemetryEvent::ReviewRound` event via fire-and-forget (T006 / AC-03).
///
/// Emitted after a review or dry round completes with the round result known.
/// For dry rounds `round_type` should be `"dry"`.
///
/// # Arguments
/// - `writer`: the writer constructed at startup.
/// - `track_id`: the branch-bound track id.
/// - `provider`: the provider name, e.g. `"codex"` or `"claude"`.
/// - `model`: the model name used.
/// - `round_type`: `"fast"`, `"final"`, or `"dry"`.
/// - `findings_count`: number of findings / violations in the completed round.
/// - `start`: the `Instant` captured before the round started.
///
/// Suppresses any `TelemetryWriteError` (CN-01 / diagnostic-only).
pub fn emit_review_round(
    writer: &TelemetryWriter,
    track_id: &str,
    provider: &str,
    model: &str,
    round_type: &str,
    findings_count: u32,
    start: Instant,
) {
    let duration_ms = start.elapsed().as_millis().try_into().unwrap_or(u64::MAX);
    let event = TelemetryEvent::ReviewRound {
        schema_version: 1,
        track_id: track_id.to_string(),
        provider: provider.to_string(),
        model: model.to_string(),
        round_type: round_type.to_string(),
        duration_ms,
        findings_count,
        timestamp: now_timestamp(),
    };
    // Fire-and-forget: suppress errors per CN-01.
    let _ = writer.write(event);
}

/// Emits a `TelemetryEvent::ExternalSubprocess` event via fire-and-forget
/// (T006 / AC-03).
///
/// Emitted after an external subprocess (codex/claude/gemini) completes.
/// The `start` instant should be captured before the subprocess was launched.
///
/// Placement rationale: emitted at the composition layer (`apps/cli-composition`)
/// after `run_codex_review_str` / `run_claude_review_str` / `dry_write` return.
/// This avoids threading the writer into `libs/infrastructure`; the composition
/// layer has all required data (command name, timing, retry_count=0, parse failure
/// flag). `retry_count` is 0 because the current implementation has no retry loop;
/// `verdict_parse_failed` is derived from whether the round returned an error.
///
/// # Arguments
/// - `writer`: the writer constructed at startup.
/// - `track_id`: the branch-bound track id.
/// - `command`: the subprocess binary name (e.g. `"codex"`, `"claude"`).
/// - `retry_count`: number of retries attempted (currently always 0).
/// - `verdict_parse_failed`: `true` when the round returned a parse / verdict error.
/// - `start`: the `Instant` captured before the subprocess was launched.
///
/// Suppresses any `TelemetryWriteError` (CN-01 / diagnostic-only).
pub fn emit_external_subprocess(
    writer: &TelemetryWriter,
    track_id: &str,
    command: &str,
    retry_count: u32,
    verdict_parse_failed: bool,
    start: Instant,
) {
    let duration_ms = start.elapsed().as_millis().try_into().unwrap_or(u64::MAX);
    let event = TelemetryEvent::ExternalSubprocess {
        schema_version: 1,
        track_id: track_id.to_string(),
        command: command.to_string(),
        duration_ms,
        retry_count,
        verdict_parse_failed,
        timestamp: now_timestamp(),
    };
    // Fire-and-forget: suppress errors per CN-01.
    let _ = writer.write(event);
}

/// Emits a `TelemetryEvent::AdvisoryHookFired` event via fire-and-forget
/// (T005 / AC-04).
///
/// Called ONLY when an advisory (UserPromptSubmit / injection-type) hook fires
/// with a non-empty context injection (i.e., a non-None stdout from the hook
/// outcome).  The allow path that produces no injection must NOT call this
/// function (OS-03).
///
/// Suppresses any `TelemetryWriteError` (CN-01 / diagnostic-only).
pub fn emit_advisory_hook_fired(writer: &TelemetryWriter, track_id: &str, hook_name: &str) {
    let event = TelemetryEvent::AdvisoryHookFired {
        schema_version: 1,
        track_id: track_id.to_string(),
        hook_name: hook_name.to_string(),
        timestamp: now_timestamp(),
    };
    // Fire-and-forget: suppress errors per CN-01.
    let _ = writer.write(event);
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::path::Path;
    use std::time::Instant;

    use infrastructure::telemetry::{TelemetryConfig, TelemetryWriter};
    use tempfile::TempDir;

    use crate::review_v2::process_guards::{CwdGuard, run_git};

    use super::{
        emit_external_subprocess, emit_non_zero_exit, emit_review_round, emit_track_subcommand,
    };

    fn temp_repo_on_track_branch(track_id: &str) -> TempDir {
        let repo = TempDir::new().unwrap();
        run_git(repo.path(), &["init", "-q"]);
        set_git_head_ref(repo.path(), &format!("track/{track_id}"));
        repo
    }

    fn set_git_head_ref(repo_root: &Path, branch: &str) {
        let branch_ref_path = repo_root.join(".git/refs/heads").join(branch);
        std::fs::create_dir_all(branch_ref_path.parent().unwrap()).unwrap();
        std::fs::write(&branch_ref_path, "0000000000000000000000000000000000000001\n").unwrap();
        std::fs::write(repo_root.join(".git/HEAD"), format!("ref: refs/heads/{branch}\n")).unwrap();
    }

    fn emit_test_track_subcommand<F>(resolve_writer: F) -> String
    where
        F: FnOnce() -> Option<(TelemetryWriter, String)>,
    {
        temp_env::with_vars(
            [("SOTP_TELEMETRY", Some("1")), ("SOTP_TELEMETRY_DIR", None::<&str>)],
            || {
                let (writer, resolved_track_id) = resolve_writer().unwrap();
                emit_track_subcommand(
                    &writer,
                    &resolved_track_id,
                    "track transition",
                    0,
                    Instant::now(),
                );
                resolved_track_id
            },
        )
    }

    // -----------------------------------------------------------------------
    // resolve_telemetry_writer: kill-switch and non-track-branch gating
    // -----------------------------------------------------------------------

    /// When `SOTP_TELEMETRY=0` the kill switch fires and the composition path
    /// must return `None` even when a valid track id is supplied.
    /// Uses `resolve_telemetry_writer_inner` with an injected track_id so the
    /// test is independent of the current git branch.
    #[test]
    fn test_resolve_telemetry_writer_returns_none_when_kill_switch_set() {
        // Safety: mutates process environment — must hold lock for test isolation.
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let tmp = TempDir::new().unwrap();
        temp_env::with_vars([("SOTP_TELEMETRY", Some("0"))], || {
            let result = super::resolve_telemetry_writer_inner(
                Some("track-test-2026-06-11".to_string()),
                tmp.path(),
            );
            assert!(
                result.is_none(),
                "SOTP_TELEMETRY=0 kill switch must suppress writer construction"
            );
        });
    }

    /// When there is no track id (non-`track/*` branch or git failure) the
    /// composition path must return `None` regardless of env.
    /// Uses `resolve_telemetry_writer_inner` with `None` track_id so the test
    /// is independent of the current git branch.
    #[test]
    fn test_resolve_telemetry_writer_returns_none_when_no_track_id() {
        // Safety: mutates process environment — must hold lock for test isolation.
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let tmp = TempDir::new().unwrap();
        temp_env::with_vars([("SOTP_TELEMETRY", Some("1"))], || {
            let result = super::resolve_telemetry_writer_inner(None, tmp.path());
            assert!(result.is_none(), "None track_id (non-track branch) must yield None writer");
        });
    }

    #[test]
    fn test_resolve_telemetry_writer_relative_items_dir_anchors_at_repo_root() {
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let track_id = "telemetry-anchor-2026-06-11";
        let repo = temp_repo_on_track_branch(track_id);
        std::fs::create_dir_all(repo.path().join("track/items")).unwrap();

        let subdir = repo.path().join("nested");
        std::fs::create_dir_all(&subdir).unwrap();
        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(&subdir).unwrap();

        let resolved_track_id = emit_test_track_subcommand(|| {
            super::resolve_telemetry_writer(Path::new("track/items"))
        });
        assert_eq!(resolved_track_id, track_id);

        let repo_telemetry_path =
            repo.path().join("track/items").join(track_id).join("logs/telemetry.jsonl");
        let subdir_telemetry_path =
            subdir.join("track/items").join(track_id).join("logs/telemetry.jsonl");
        assert!(repo_telemetry_path.exists(), "telemetry must be written under repo track/items");
        assert!(
            !subdir_telemetry_path.exists(),
            "relative items_dir must not write telemetry under the process cwd"
        );
    }

    #[test]
    fn test_resolve_telemetry_writer_preserves_custom_items_dir() {
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let track_id = "telemetry-custom-items-2026-06-11";
        let repo = temp_repo_on_track_branch(track_id);

        let custom_items_dir = repo.path().join("custom/track/items");
        std::fs::create_dir_all(&custom_items_dir).unwrap();

        let resolved_track_id =
            emit_test_track_subcommand(|| super::resolve_telemetry_writer(&custom_items_dir));
        assert_eq!(resolved_track_id, track_id);

        let custom_telemetry_path = custom_items_dir.join(track_id).join("logs/telemetry.jsonl");
        let default_telemetry_path =
            repo.path().join("track/items").join(track_id).join("logs/telemetry.jsonl");
        assert!(custom_telemetry_path.exists(), "telemetry must use the supplied items_dir");
        assert!(
            !default_telemetry_path.exists(),
            "custom items_dir must not be rewritten to the default track/items path"
        );
    }

    #[test]
    fn test_resolve_telemetry_writer_for_track_does_not_reread_branch() {
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let track_id = "telemetry-captured-track-2026-06-11";
        let repo = temp_repo_on_track_branch(track_id);
        std::fs::create_dir_all(repo.path().join("track/items")).unwrap();
        set_git_head_ref(repo.path(), "main");
        let _cwd_guard = CwdGuard::save_current();
        std::env::set_current_dir(repo.path()).unwrap();

        let resolved_track_id = emit_test_track_subcommand(|| {
            super::resolve_telemetry_writer_for_track(Path::new("track/items"), track_id)
        });
        assert_eq!(resolved_track_id, track_id);

        let telemetry_path =
            repo.path().join("track/items").join(track_id).join("logs/telemetry.jsonl");
        assert!(
            telemetry_path.exists(),
            "pre-resolved telemetry must use the captured track id even after a branch switch"
        );
    }

    // -----------------------------------------------------------------------
    // init_tracing_subscriber: once-guard
    // -----------------------------------------------------------------------

    #[test]
    fn test_init_tracing_subscriber_twice_does_not_panic() {
        // If the once-guard is broken, calling twice would panic with
        // "a subscriber has already been set".
        super::init_tracing_subscriber();
        super::init_tracing_subscriber(); // must not panic
    }

    // -----------------------------------------------------------------------
    // emit helpers: write to tmpdir and verify event is present
    // -----------------------------------------------------------------------

    fn writer_in_tempdir(tmp: &TempDir) -> (TelemetryWriter, String) {
        let track_id = "test-track-2026-06-10".to_string();
        let dir_str = tmp.path().to_string_lossy().into_owned();
        let mut cfg = None;
        temp_env::with_vars(
            [("SOTP_TELEMETRY_DIR", Some(dir_str.as_str())), ("SOTP_TELEMETRY", Some("1"))],
            || {
                cfg = Some(TelemetryConfig::from_env());
            },
        );
        let writer = TelemetryWriter::new(cfg.unwrap(), track_id.clone(), tmp.path().to_path_buf());
        (writer, track_id)
    }

    #[test]
    fn test_emit_track_subcommand_writes_event_line_to_jsonl() {
        // Safety: writer_in_tempdir mutates process environment via temp_env.
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let tmp = TempDir::new().unwrap();
        let (writer, track_id) = writer_in_tempdir(&tmp);
        let start = Instant::now();

        emit_track_subcommand(&writer, &track_id, "track transition", 0, start);

        let output_path = tmp.path().join("telemetry.jsonl");
        assert!(output_path.exists(), "telemetry.jsonl must exist after emit");
        let content = std::fs::read_to_string(&output_path).unwrap();
        assert!(content.contains("TrackSubcommand"), "event_type must be TrackSubcommand");
        assert!(content.contains("track transition"), "command must be present");
        assert!(content.contains(&track_id), "track_id must be present");
        assert!(content.contains("\"exit_code\":0"), "exit_code 0 must be present");
    }

    #[test]
    fn test_emit_non_zero_exit_writes_event_line_to_jsonl() {
        // Safety: writer_in_tempdir mutates process environment via temp_env.
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let tmp = TempDir::new().unwrap();
        let (writer, track_id) = writer_in_tempdir(&tmp);

        emit_non_zero_exit(&writer, &track_id, "track transition", 1, "something failed");

        let output_path = tmp.path().join("telemetry.jsonl");
        assert!(output_path.exists(), "telemetry.jsonl must exist after emit");
        let content = std::fs::read_to_string(&output_path).unwrap();
        assert!(content.contains("NonZeroExit"), "event_type must be NonZeroExit");
        assert!(content.contains("something failed"), "error_chain must be present");
        assert!(content.contains("\"exit_code\":1"), "exit_code 1 must be present");
    }

    #[test]
    fn test_emit_track_subcommand_with_nonzero_exit_is_recorded() {
        // Safety: writer_in_tempdir mutates process environment via temp_env.
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let tmp = TempDir::new().unwrap();
        let (writer, track_id) = writer_in_tempdir(&tmp);
        let start = Instant::now();

        emit_track_subcommand(&writer, &track_id, "track transition", 1, start);

        let output_path = tmp.path().join("telemetry.jsonl");
        let content = std::fs::read_to_string(&output_path).unwrap();
        assert!(content.contains("TrackSubcommand"));
        assert!(content.contains("\"exit_code\":1"));
    }

    // -----------------------------------------------------------------------
    // emit_gate_eval: GateEval event with required fields (AC-03 / GO-01)
    // -----------------------------------------------------------------------

    #[test]
    fn test_emit_gate_eval_writes_gate_eval_event_with_required_fields() {
        // Safety: writer_in_tempdir mutates process environment via temp_env.
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let tmp = TempDir::new().unwrap();
        let (writer, track_id) = writer_in_tempdir(&tmp);
        let start = Instant::now();

        super::emit_gate_eval(&writer, &track_id, "verify-adr-signals", "ok", "", start);

        let output_path = tmp.path().join("telemetry.jsonl");
        assert!(output_path.exists(), "telemetry.jsonl must exist after emit_gate_eval");
        let content = std::fs::read_to_string(&output_path).unwrap();
        // Required fields per AC-03 / infrastructure-types.json
        assert!(content.contains("GateEval"), "event_type must be GateEval; got: {content}");
        assert!(
            content.contains("verify-adr-signals"),
            "gate_name must be present; got: {content}"
        );
        assert!(content.contains("\"verdict\":\"ok\""), "verdict must be present; got: {content}");
        assert!(content.contains("\"duration_ms\""), "duration_ms must be present (GO-01)");
        assert!(content.contains("\"schema_version\":1"), "schema_version must be present (AC-09)");
        assert!(content.contains(&track_id), "track_id must be present");
    }

    // -----------------------------------------------------------------------
    // emit_hook_block: HookBlock emitted on blocking verdict (AC-04)
    // -----------------------------------------------------------------------

    #[test]
    fn test_emit_hook_block_writes_hook_block_event() {
        // Safety: writer_in_tempdir mutates process environment via temp_env.
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let tmp = TempDir::new().unwrap();
        let (writer, track_id) = writer_in_tempdir(&tmp);

        super::emit_hook_block(&writer, &track_id, "block-direct-git-ops");

        let output_path = tmp.path().join("telemetry.jsonl");
        assert!(output_path.exists(), "telemetry.jsonl must exist after emit_hook_block");
        let content = std::fs::read_to_string(&output_path).unwrap();
        assert!(content.contains("HookBlock"), "event_type must be HookBlock; got: {content}");
        assert!(
            content.contains("block-direct-git-ops"),
            "hook_name must be present; got: {content}"
        );
        assert!(content.contains(&track_id), "track_id must be present");
        assert!(content.contains("\"schema_version\":1"), "schema_version must be present");
    }

    // -----------------------------------------------------------------------
    // Nothing emitted on allow path (OS-03 / AC-04 / AC-06)
    // -----------------------------------------------------------------------

    #[test]
    fn test_no_emit_on_allow_path_leaves_no_file() {
        // Safety: tests env-var mutation.
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let tmp = TempDir::new().unwrap();
        // Construct writer but never call emit_hook_block or emit_advisory_hook_fired.
        let (_writer, _track_id) = writer_in_tempdir(&tmp);

        // No emit means no file open (lazy init — AC-06).
        let output_path = tmp.path().join("telemetry.jsonl");
        assert!(!output_path.exists(), "no file must be created when nothing is emitted (OS-03)");
    }

    // -----------------------------------------------------------------------
    // emit_advisory_hook_fired: AdvisoryHookFired emitted for advisory hooks (AC-04)
    // -----------------------------------------------------------------------

    // -----------------------------------------------------------------------
    // emit_review_round: ReviewRound event with required fields (AC-03 / T006)
    // -----------------------------------------------------------------------

    #[test]
    fn test_emit_review_round_writes_review_round_event_with_required_fields() {
        // Safety: writer_in_tempdir mutates process environment via temp_env.
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let tmp = TempDir::new().unwrap();
        let (writer, track_id) = writer_in_tempdir(&tmp);
        let start = Instant::now();

        emit_review_round(&writer, &track_id, "codex", "gpt-5.4-mini", "fast", 3, start);

        let output_path = tmp.path().join("telemetry.jsonl");
        assert!(output_path.exists(), "telemetry.jsonl must exist after emit_review_round");
        let content = std::fs::read_to_string(&output_path).unwrap();
        // Required fields per AC-03 / infrastructure-types.json
        assert!(content.contains("ReviewRound"), "event_type must be ReviewRound; got: {content}");
        assert!(
            content.contains("\"provider\":\"codex\""),
            "provider must be present; got: {content}"
        );
        assert!(
            content.contains("\"model\":\"gpt-5.4-mini\""),
            "model must be present; got: {content}"
        );
        assert!(
            content.contains("\"round_type\":\"fast\""),
            "round_type must be present; got: {content}"
        );
        assert!(
            content.contains("\"findings_count\":3"),
            "findings_count must be present; got: {content}"
        );
        assert!(content.contains("\"duration_ms\""), "duration_ms must be present; got: {content}");
        assert!(content.contains("\"schema_version\":1"), "schema_version must be present (AC-09)");
        assert!(content.contains(&track_id), "track_id must be present");
        // Findings body must NOT be present (OS-04).
        assert!(!content.contains("message"), "findings body must not be recorded (OS-04)");
    }

    #[test]
    fn test_emit_review_round_dry_round_type_is_recorded() {
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let tmp = TempDir::new().unwrap();
        let (writer, track_id) = writer_in_tempdir(&tmp);
        let start = Instant::now();

        emit_review_round(&writer, &track_id, "codex", "gpt-5.4-mini", "dry", 0, start);

        let content = std::fs::read_to_string(tmp.path().join("telemetry.jsonl")).unwrap();
        assert!(
            content.contains("\"round_type\":\"dry\""),
            "dry round_type must be recorded; got: {content}"
        );
        assert!(
            content.contains("\"findings_count\":0"),
            "zero findings must be recorded; got: {content}"
        );
    }

    // -----------------------------------------------------------------------
    // emit_external_subprocess: ExternalSubprocess event (AC-03 / T006)
    // -----------------------------------------------------------------------

    #[test]
    fn test_emit_external_subprocess_success_path_writes_event() {
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let tmp = TempDir::new().unwrap();
        let (writer, track_id) = writer_in_tempdir(&tmp);
        let start = Instant::now();

        // verdict_parse_failed = false: normal completion
        emit_external_subprocess(&writer, &track_id, "codex", 0, false, start);

        let output_path = tmp.path().join("telemetry.jsonl");
        assert!(output_path.exists(), "telemetry.jsonl must exist after emit_external_subprocess");
        let content = std::fs::read_to_string(&output_path).unwrap();
        assert!(
            content.contains("ExternalSubprocess"),
            "event_type must be ExternalSubprocess; got: {content}"
        );
        assert!(
            content.contains("\"command\":\"codex\""),
            "command must be present; got: {content}"
        );
        assert!(
            content.contains("\"retry_count\":0"),
            "retry_count must be present; got: {content}"
        );
        assert!(
            content.contains("\"verdict_parse_failed\":false"),
            "verdict_parse_failed false must be present; got: {content}"
        );
        assert!(content.contains("\"duration_ms\""), "duration_ms must be present; got: {content}");
        assert!(content.contains("\"schema_version\":1"), "schema_version must be present (AC-09)");
        assert!(content.contains(&track_id), "track_id must be present");
    }

    #[test]
    fn test_emit_external_subprocess_failure_path_records_verdict_parse_failed_true() {
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let tmp = TempDir::new().unwrap();
        let (writer, track_id) = writer_in_tempdir(&tmp);
        let start = Instant::now();

        // verdict_parse_failed = true: subprocess failed / verdict could not be parsed
        emit_external_subprocess(&writer, &track_id, "claude", 0, true, start);

        let content = std::fs::read_to_string(tmp.path().join("telemetry.jsonl")).unwrap();
        assert!(
            content.contains("ExternalSubprocess"),
            "event_type must be ExternalSubprocess; got: {content}"
        );
        assert!(
            content.contains("\"command\":\"claude\""),
            "command must be claude; got: {content}"
        );
        assert!(
            content.contains("\"verdict_parse_failed\":true"),
            "verdict_parse_failed true must be present; got: {content}"
        );
    }

    #[test]
    fn test_emit_advisory_hook_fired_writes_advisory_hook_fired_event() {
        // Safety: writer_in_tempdir mutates process environment via temp_env.
        let _lock = crate::test_support::process_env_lock().lock().unwrap();
        let tmp = TempDir::new().unwrap();
        let (writer, track_id) = writer_in_tempdir(&tmp);

        super::emit_advisory_hook_fired(&writer, &track_id, "skill-compliance");

        let output_path = tmp.path().join("telemetry.jsonl");
        assert!(output_path.exists(), "telemetry.jsonl must exist after emit_advisory_hook_fired");
        let content = std::fs::read_to_string(&output_path).unwrap();
        assert!(
            content.contains("AdvisoryHookFired"),
            "event_type must be AdvisoryHookFired; got: {content}"
        );
        assert!(content.contains("skill-compliance"), "hook_name must be present; got: {content}");
        assert!(content.contains(&track_id), "track_id must be present");
        assert!(content.contains("\"schema_version\":1"), "schema_version must be present");
    }
}
