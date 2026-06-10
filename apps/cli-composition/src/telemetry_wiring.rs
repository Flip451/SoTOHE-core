//! Telemetry wiring for the `cli-composition` composition root.
//!
//! Provides:
//! - `init_tracing_subscriber`: one-shot tracing-subscriber initialisation
//!   (composition root only, per IN-01 / CN-04 / AC-01).
//! - `resolve_telemetry_writer`: branch-bound `TelemetryWriter` construction
//!   (returns `None` on non-`track/*` branches — IN-04 / OS-07 / AC-11).
//! - `emit_track_subcommand`: fire-and-forget `TelemetryEvent::TrackSubcommand` emit.
//! - `emit_non_zero_exit`: fire-and-forget `TelemetryEvent::NonZeroExit` emit.
//! - `now_timestamp`: ISO-8601 UTC timestamp helper.

use std::path::Path;
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
/// The default filter is `"info"` (AC-01).
///
/// Safe to call more than once — the `OnceLock` ensures at most one attempt
/// through this function. Additionally uses `try_init()` instead of `init()`
/// so that an already-installed subscriber from another source in the process
/// does not cause a panic; the already-set case is silently ignored.
pub fn init_tracing_subscriber() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        use tracing_subscriber::EnvFilter;
        let filter = EnvFilter::try_from_env("RUST_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
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
    resolve_telemetry_writer_inner(resolve_track_id_from_branch(items_dir), items_dir)
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
fn resolve_track_id_from_branch(items_dir: &Path) -> Option<String> {
    use infrastructure::git_cli::{GitRepository as _, SystemGitRepo};
    use usecase::track_resolution::resolve_track_id_from_branch as resolve_fn;

    // Derive the project root from items_dir so discovery is anchored to the
    // correct repo regardless of the process CWD (P1 fix: was discover()).
    let project_root = crate::track::resolve_project_root(items_dir).ok()?;
    let repo = SystemGitRepo::discover_from(&project_root).ok()?;
    let branch = repo.current_branch().ok().flatten()?;

    resolve_fn(Some(&branch)).ok()
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

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::time::Instant;

    use infrastructure::telemetry::{TelemetryConfig, TelemetryWriter};
    use tempfile::TempDir;

    use super::{emit_non_zero_exit, emit_track_subcommand};

    // -----------------------------------------------------------------------
    // resolve_track_id_from_branch: branch-parse coverage lives in
    // libs/usecase/src/track_resolution.rs; tests here cover only the
    // telemetry wiring composition path.
    // -----------------------------------------------------------------------

    #[test]
    fn test_detached_head_yields_no_track_id() {
        use usecase::track_resolution::{TrackResolutionError, resolve_track_id_from_branch};
        // None branch = detached HEAD or NoBranch
        let result = resolve_track_id_from_branch(None);
        match result {
            Err(TrackResolutionError::NoBranch) => {}
            Err(TrackResolutionError::DetachedHead) => {}
            Err(_) => {} // Any error is acceptable
            Ok(id) => panic!("expected no track id for None branch, got {id:?}"),
        }
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
}
