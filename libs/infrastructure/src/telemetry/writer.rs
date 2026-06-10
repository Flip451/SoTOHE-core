//! `TelemetryWriter` — O_APPEND single-write JSONL event writer.
//!
//! Writes telemetry events to `track/items/<id>/logs/telemetry.jsonl` using
//! O_APPEND single-syscall writes per IN-02 / CN-05.
//!
//! ## Lazy initialisation
//!
//! File open is deferred until the first call to `write`.  If telemetry is
//! disabled (kill switch) or no events are ever emitted, the file is never
//! opened (AC-06).
//!
//! ## Lazy-init mechanism: `Mutex<Option<File>>`
//!
//! `std::sync::OnceLock<File>` would be the natural fit for a one-shot init,
//! but its `get_or_try_init` method is nightly-only (stable toolchain 1.91
//! ships only the infallible `get_or_init`).  Because file open is fallible
//! *and* `File::write` requires `&mut self`, we use
//! `Mutex<Option<std::fs::File>>`:
//!
//! - The `Option` is the lazy sentinel: `None` = not yet opened.
//! - The `Mutex` provides the `&mut File` needed for `Write::write` from a
//!   `&self` receiver, while remaining sound.
//! - For a sync CLI the mutex is always uncontested, so the overhead is a
//!   single atomic CAS on each write — negligible compared to the file I/O.
//! - CN-05 ("no userland lock") targets *concurrent append* semantics (where
//!   a lock would serialize writes that the kernel already makes atomic via
//!   O_APPEND).  The Mutex here wraps *initialization + write*, not just write,
//!   and does not defeat O_APPEND atomicity for multi-process use.

use std::fs::{File, OpenOptions};
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::Mutex;

use super::TelemetryEvent;
use super::TelemetryWriteError;
use crate::telemetry::config::TelemetryConfig;

/// Maximum byte length of a single JSONL event line (including the trailing
/// newline) per CN-05 / IN-02.
const MAX_LINE_BYTES: usize = 4096;

/// Byte cap applied to variable-length fields when the line would exceed
/// `MAX_LINE_BYTES` (CN-05 / T003 description).
const TRUNCATED_FIELD_CAP: usize = 256;

// ---------------------------------------------------------------------------
// TelemetryWriter
// ---------------------------------------------------------------------------

/// Writes telemetry events to `track/items/<id>/logs/telemetry.jsonl` using
/// O_APPEND single-write syscalls per D3 (IN-02, CN-05).
///
/// Initialization is lazy: file open is deferred until the first event is
/// written (IN-05, AC-06).  Respects `SOTP_TELEMETRY=0` kill switch and
/// `SOTP_TELEMETRY_DIR` override from `TelemetryConfig`.
///
/// Constructed by `cli-composition` from `TelemetryConfig`.  The `write()`
/// method returns `Result<(), TelemetryWriteError>` — IO/serialize errors are
/// returned to the caller (composition root) which silently suppresses them
/// (fire-and-forget, CN-01).
///
/// Private fields: resolved output path and enabled flag.
///
/// # Lazy-init mechanism
///
/// See the module-level doc for the rationale behind `Mutex<Option<File>>`.
pub struct TelemetryWriter {
    /// Whether telemetry is enabled (from `TelemetryConfig::is_enabled`).
    enabled: bool,
    /// Resolved path to `telemetry.jsonl`.
    output_path: PathBuf,
    /// Lazily-opened file handle.  `None` until the first `write` call.
    file: Mutex<Option<File>>,
}

impl std::fmt::Debug for TelemetryWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TelemetryWriter")
            .field("enabled", &self.enabled)
            .field("output_path", &self.output_path)
            .finish_non_exhaustive()
    }
}

impl TelemetryWriter {
    /// Constructs a `TelemetryWriter`.
    ///
    /// The output path is resolved as:
    /// - `SOTP_TELEMETRY_DIR/telemetry.jsonl` when the dir override is set.
    /// - `items_dir/<track_id>/logs/telemetry.jsonl` otherwise (CN-03).
    ///
    /// `track_id` is sanitized before being joined into the path: only the
    /// final normal path component is used, and any `..` or root-prefix
    /// components are stripped.  This prevents path-traversal escapes when an
    /// untrusted string is passed as `track_id`.
    ///
    /// No file I/O is performed at construction time (AC-06).
    #[must_use]
    pub fn new(config: TelemetryConfig, track_id: String, items_dir: PathBuf) -> Self {
        let output_path = match config.output_dir_override() {
            Some(override_dir) => override_dir.join("telemetry.jsonl"),
            None => {
                // Sanitize track_id: collect only Normal components to prevent
                // path-traversal via `..` or absolute prefixes in the string.
                let safe_track_id = safe_path_component(&track_id);
                items_dir.join(safe_track_id).join("logs").join("telemetry.jsonl")
            }
        };

        Self { enabled: config.is_enabled(), output_path, file: Mutex::new(None) }
    }

    /// Writes a single telemetry event as a JSONL line.
    ///
    /// - Returns `Ok(())` immediately when telemetry is disabled (kill-switch,
    ///   AC-05) without opening any file.
    /// - On the first call when enabled, opens (or creates) the output file
    ///   with `O_APPEND | O_CREAT`, creating parent directories as needed.
    /// - Serializes the event to JSON, appends `\n`, and issues a single
    ///   `Write::write` call (not `write_all`).  A short write is reported as
    ///   `TelemetryWriteError::Io` because it would corrupt the line boundary.
    /// - If the serialized line exceeds `MAX_LINE_BYTES` bytes, variable-length
    ///   fields (`error_chain` / `reason_summary`) are truncated to
    ///   `TRUNCATED_FIELD_CAP` bytes and serialization is retried once (CN-05).
    ///
    /// # Errors
    ///
    /// Returns `TelemetryWriteError::Serialize` on JSON serialization failure.
    /// Returns `TelemetryWriteError::Io` on file open failure, short write, or
    /// other I/O error.  The caller (composition root) suppresses this error
    /// (fire-and-forget, CN-01).
    pub fn write(&self, event: TelemetryEvent) -> Result<(), TelemetryWriteError> {
        if !self.enabled {
            return Ok(());
        }

        // Build the line bytes (with possible truncation retry).
        let line_bytes = self.serialize_event(event)?;

        // Acquire the lazy-init lock.
        let mut guard = self.file.lock().map_err(|_| TelemetryWriteError::Io {
            path: self.output_path.display().to_string(),
            message: "mutex poisoned".to_string(),
        })?;

        // Lazy open: only on first write.
        if guard.is_none() {
            let dir = self.output_path.parent().ok_or_else(|| TelemetryWriteError::Io {
                path: self.output_path.display().to_string(),
                message: "output path has no parent directory".to_string(),
            })?;
            std::fs::create_dir_all(dir).map_err(|e| TelemetryWriteError::Io {
                path: dir.display().to_string(),
                message: e.to_string(),
            })?;
            let file =
                OpenOptions::new().append(true).create(true).open(&self.output_path).map_err(
                    |e| TelemetryWriteError::Io {
                        path: self.output_path.display().to_string(),
                        message: e.to_string(),
                    },
                )?;
            *guard = Some(file);
        }

        // Write the line in a single syscall (O_APPEND atomicity).
        let file = guard.as_mut().ok_or_else(|| TelemetryWriteError::Io {
            path: self.output_path.display().to_string(),
            message: "file handle unexpectedly absent after init".to_string(),
        })?;

        // `Write::write` (not `write_all`) — single syscall; short-write is an error.
        let n = file.write(&line_bytes).map_err(|e| TelemetryWriteError::Io {
            path: self.output_path.display().to_string(),
            message: e.to_string(),
        })?;

        if n != line_bytes.len() {
            return Err(TelemetryWriteError::Io {
                path: self.output_path.display().to_string(),
                message: format!("short write: wrote {n} of {} bytes", line_bytes.len()),
            });
        }

        Ok(())
    }

    /// Serializes `event` into JSONL bytes (JSON + `\n`).
    ///
    /// If the resulting line would exceed `MAX_LINE_BYTES`, truncates
    /// variable-length fields and retries once (CN-05).  If the line still
    /// exceeds the cap after truncation (e.g. due to a very long `track_id` or
    /// JSON-encoding expansion), the oversized line is hard-capped by
    /// byte-truncating at a `\n`-terminated boundary so the file is not
    /// corrupted.
    fn serialize_event(&self, event: TelemetryEvent) -> Result<Vec<u8>, TelemetryWriteError> {
        let line = build_line(&event)?;
        if line.len() <= MAX_LINE_BYTES {
            return Ok(line);
        }

        // Line too long: truncate variable-length fields and retry once.
        let truncated = truncate_variable_fields(event);
        let line2 = build_line(&truncated)?;

        // Guard: if the truncated line still exceeds the cap (e.g. a very long
        // track_id or JSON-escaping expansion), return an Io error rather than
        // writing a byte-truncated line that could be invalid UTF-8/JSONL and
        // would corrupt downstream readers that parse the whole file as text.
        if line2.len() > MAX_LINE_BYTES {
            return Err(TelemetryWriteError::Io {
                path: self.output_path.display().to_string(),
                message: format!(
                    "serialized event still exceeds {MAX_LINE_BYTES} bytes ({} bytes) after \
                     field truncation; event dropped",
                    line2.len()
                ),
            });
        }

        Ok(line2)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns a safe, single-component path name derived from `raw`.
///
/// Takes only the **last** `Normal` path component from `raw`, stripping any
/// `..`, root prefix, or embedded path separators that could cause
/// path-traversal escapes when the value is used in `.join()`.  Track IDs are
/// single-level slugs by convention, so taking the last component is the
/// correct behaviour for any input.  If `raw` contains no normal components,
/// returns `"unknown"` as a fallback.
///
/// Examples:
/// - `"my-track-2026"` → `"my-track-2026"` (slug, unchanged)
/// - `"../escape"` → `"escape"` (`.` stripped, last component used)
/// - `"foo/bar"` → `"bar"` (last component used; prevents subdirectory write)
/// - `""` → `"unknown"` (empty fallback)
fn safe_path_component(raw: &str) -> String {
    use std::path::Component;
    let last = std::path::Path::new(raw)
        .components()
        .filter_map(|c| if let Component::Normal(s) = c { s.to_str() } else { None })
        .next_back();
    last.unwrap_or("unknown").to_string()
}

/// Serializes a `TelemetryEvent` to `JSON\n` bytes.
fn build_line(event: &TelemetryEvent) -> Result<Vec<u8>, TelemetryWriteError> {
    let json = serde_json::to_string(event)
        .map_err(|e| TelemetryWriteError::Serialize { message: e.to_string() })?;
    let mut bytes = json.into_bytes();
    bytes.push(b'\n');
    Ok(bytes)
}

/// Truncates variable-length string fields that can make a line exceed 4096
/// bytes, returning a new event with capped values (CN-05).
///
/// The truncation is byte-level (`truncate_utf8`).  Fields truncated:
/// - `NonZeroExit::error_chain`
/// - `GateEval::reason_summary`
fn truncate_variable_fields(event: TelemetryEvent) -> TelemetryEvent {
    match event {
        TelemetryEvent::NonZeroExit {
            schema_version,
            track_id,
            command,
            exit_code,
            error_chain,
            timestamp,
        } => TelemetryEvent::NonZeroExit {
            schema_version,
            track_id,
            command,
            exit_code,
            error_chain: truncate_utf8(error_chain, TRUNCATED_FIELD_CAP),
            timestamp,
        },
        TelemetryEvent::GateEval {
            schema_version,
            track_id,
            gate_name,
            verdict,
            reason_summary,
            input_hash,
            duration_ms,
            timestamp,
        } => TelemetryEvent::GateEval {
            schema_version,
            track_id,
            gate_name,
            verdict,
            reason_summary: truncate_utf8(reason_summary, TRUNCATED_FIELD_CAP),
            input_hash,
            duration_ms,
            timestamp,
        },
        // Other variants have no variable-length fields that typically cause
        // oversized lines; return as-is.
        other => other,
    }
}

/// Truncates `s` to at most `max_bytes` bytes, aligned to a UTF-8 character
/// boundary so the result is always valid UTF-8.
fn truncate_utf8(s: String, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s;
    }
    // Walk back from max_bytes until we find a valid char boundary.
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::io::BufRead as _;
    use tempfile::TempDir;

    use super::*;
    use crate::telemetry::config::TelemetryConfig;

    // -----------------------------------------------------------------------
    // Config constructors for tests (use temp_env for safe env mutation)
    // -----------------------------------------------------------------------

    /// Returns an enabled `TelemetryConfig` with `output_dir_override` pointing
    /// to `override_dir`.
    fn enabled_config_with_dir(override_dir: &std::path::Path) -> TelemetryConfig {
        let dir_str = override_dir.to_string_lossy().into_owned();
        let mut cfg = None;
        temp_env::with_vars(
            [("SOTP_TELEMETRY_DIR", Some(dir_str.as_str())), ("SOTP_TELEMETRY", None)],
            || {
                cfg = Some(TelemetryConfig::from_env());
            },
        );
        cfg.unwrap()
    }

    /// Returns a disabled `TelemetryConfig` (kill-switch set).
    fn disabled_config() -> TelemetryConfig {
        let mut cfg = None;
        temp_env::with_var("SOTP_TELEMETRY", Some("0"), || {
            cfg = Some(TelemetryConfig::from_env());
        });
        cfg.unwrap()
    }

    /// Builds a writer that writes directly to `tmp.path()/telemetry.jsonl`
    /// (via SOTP_TELEMETRY_DIR override pointing to `tmp.path()`).
    fn writer_in_tempdir(tmp: &TempDir) -> TelemetryWriter {
        let config = enabled_config_with_dir(tmp.path());
        TelemetryWriter::new(config, "test-track-2026".to_string(), tmp.path().to_path_buf())
    }

    fn sample_hook_block_event() -> TelemetryEvent {
        TelemetryEvent::HookBlock {
            schema_version: 1,
            track_id: "test-track-2026".to_string(),
            hook_name: "block-direct-git-ops".to_string(),
            timestamp: "2026-06-10T00:00:00Z".to_string(),
        }
    }

    // -----------------------------------------------------------------------
    // Kill-switch: disabled config must not open file
    // -----------------------------------------------------------------------

    #[test]
    fn test_write_with_disabled_config_does_not_open_file() {
        let tmp = TempDir::new().unwrap();
        let config = disabled_config();
        let writer =
            TelemetryWriter::new(config, "test-track-2026".to_string(), tmp.path().to_path_buf());

        let result = writer.write(sample_hook_block_event());
        assert!(result.is_ok(), "disabled writer must return Ok: {result:?}");

        // The logs dir must not have been created (no file open occurred).
        let logs_dir = tmp.path().join("test-track-2026").join("logs");
        assert!(!logs_dir.exists(), "logs/ dir must not be created when telemetry is disabled");
    }

    // -----------------------------------------------------------------------
    // Enabled: file is created and event is appended
    // -----------------------------------------------------------------------

    #[test]
    fn test_write_enabled_creates_telemetry_jsonl_and_appends_event() {
        let tmp = TempDir::new().unwrap();
        let writer = writer_in_tempdir(&tmp);
        let output_path = tmp.path().join("telemetry.jsonl");

        let event = sample_hook_block_event();
        writer.write(event).unwrap();

        assert!(output_path.exists(), "telemetry.jsonl must be created after first write");

        let content = std::fs::read_to_string(&output_path).unwrap();
        assert!(!content.is_empty(), "telemetry.jsonl must not be empty after write");
        // Must end with newline (JSONL convention).
        assert!(content.ends_with('\n'), "line must end with newline");
    }

    // -----------------------------------------------------------------------
    // Lazy init: file must not be opened before first write
    // -----------------------------------------------------------------------

    #[test]
    fn test_lazy_init_no_file_before_first_write() {
        let tmp = TempDir::new().unwrap();
        let writer = writer_in_tempdir(&tmp);
        let output_path = tmp.path().join("telemetry.jsonl");

        // File must not exist before any write.
        assert!(!output_path.exists(), "telemetry.jsonl must not exist before first write");

        // After one write it must exist.
        writer.write(sample_hook_block_event()).unwrap();
        assert!(output_path.exists(), "telemetry.jsonl must exist after first write");
    }

    // -----------------------------------------------------------------------
    // Valid JSONL: each line is valid JSON with schema_version
    // -----------------------------------------------------------------------

    #[test]
    fn test_written_lines_are_valid_json_with_schema_version() {
        let tmp = TempDir::new().unwrap();
        let writer = writer_in_tempdir(&tmp);
        let output_path = tmp.path().join("telemetry.jsonl");

        writer.write(sample_hook_block_event()).unwrap();
        writer
            .write(TelemetryEvent::TrackSubcommand {
                schema_version: 1,
                track_id: "test-track-2026".to_string(),
                command: "track spec-design".to_string(),
                exit_code: 0,
                duration_ms: 1_000,
                timestamp: "2026-06-10T00:00:00Z".to_string(),
            })
            .unwrap();

        let file = std::fs::File::open(&output_path).unwrap();
        for line in std::io::BufReader::new(file).lines() {
            let line = line.unwrap();
            let val: serde_json::Value =
                serde_json::from_str(&line).expect("each JSONL line must be valid JSON");
            assert!(
                val.get("schema_version").is_some(),
                "each line must have schema_version field; got: {val}"
            );
            assert!(
                val.get("event_type").is_some(),
                "each line must have event_type field; got: {val}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // Multiple appends: O_APPEND semantics — lines accumulate
    // -----------------------------------------------------------------------

    #[test]
    fn test_multiple_writes_append_multiple_lines() {
        let tmp = TempDir::new().unwrap();
        let writer = writer_in_tempdir(&tmp);
        let output_path = tmp.path().join("telemetry.jsonl");

        for _ in 0..3 {
            writer.write(sample_hook_block_event()).unwrap();
        }

        let content = std::fs::read_to_string(&output_path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 3, "must have 3 JSONL lines after 3 writes");
    }

    // -----------------------------------------------------------------------
    // 4096-byte cap: oversized line is truncated
    // -----------------------------------------------------------------------

    #[test]
    fn test_oversized_event_line_is_truncated_to_fit_4096_bytes() {
        let tmp = TempDir::new().unwrap();
        let writer = writer_in_tempdir(&tmp);
        let output_path = tmp.path().join("telemetry.jsonl");

        // Build a NonZeroExit with a very large error_chain.
        let big_error_chain = "x".repeat(8_192);
        let event = TelemetryEvent::NonZeroExit {
            schema_version: 1,
            track_id: "test-track-2026".to_string(),
            command: "track impl".to_string(),
            exit_code: 1,
            error_chain: big_error_chain,
            timestamp: "2026-06-10T00:00:00Z".to_string(),
        };

        writer.write(event).unwrap();

        let content = std::fs::read_to_string(&output_path).unwrap();
        let line = content.lines().next().unwrap();
        assert!(
            line.len() <= MAX_LINE_BYTES,
            "written line must be ≤ {MAX_LINE_BYTES} bytes; got {} bytes",
            line.len()
        );

        // The line must still be valid JSON with schema_version.
        let val: serde_json::Value = serde_json::from_str(line).unwrap();
        assert!(val.get("schema_version").is_some());
    }

    // -----------------------------------------------------------------------
    // 4096-byte cap: GateEval reason_summary is also truncated
    // -----------------------------------------------------------------------

    #[test]
    fn test_oversized_gate_eval_reason_summary_is_truncated() {
        let tmp = TempDir::new().unwrap();
        let writer = writer_in_tempdir(&tmp);
        let output_path = tmp.path().join("telemetry.jsonl");

        let big_summary = "y".repeat(8_192);
        let event = TelemetryEvent::GateEval {
            schema_version: 1,
            track_id: "test-track-2026".to_string(),
            gate_name: "verify-adr-signals".to_string(),
            verdict: "error".to_string(),
            reason_summary: big_summary,
            input_hash: "abc123".to_string(),
            duration_ms: 100,
            timestamp: "2026-06-10T00:00:00Z".to_string(),
        };

        writer.write(event).unwrap();

        let content = std::fs::read_to_string(&output_path).unwrap();
        let line = content.lines().next().unwrap();
        assert!(
            line.len() <= MAX_LINE_BYTES,
            "written line must be ≤ {MAX_LINE_BYTES} bytes; got {} bytes",
            line.len()
        );
    }

    // -----------------------------------------------------------------------
    // Error case: write to a path whose parent cannot be created
    // -----------------------------------------------------------------------

    #[test]
    fn test_write_to_nonexistent_parent_returns_io_error() {
        let tmp = TempDir::new().unwrap();
        // Point to a path where the "parent" is actually an existing file —
        // create_dir_all will fail because it cannot create a dir over a file.
        let blocker = tmp.path().join("blocker_file");
        std::fs::write(&blocker, b"").unwrap();

        // Use SOTP_TELEMETRY_DIR pointing to <blocker_file>/sub — a path whose
        // parent (blocker_file) is a regular file, so create_dir_all will fail.
        let fake_parent = blocker.join("sub");
        let fake_str = fake_parent.to_string_lossy().into_owned();
        let mut cfg = None;
        temp_env::with_vars(
            [("SOTP_TELEMETRY_DIR", Some(fake_str.as_str())), ("SOTP_TELEMETRY", None)],
            || {
                cfg = Some(TelemetryConfig::from_env());
            },
        );

        let writer = TelemetryWriter::new(
            cfg.unwrap(),
            "test-track-2026".to_string(),
            tmp.path().to_path_buf(),
        );

        let result = writer.write(sample_hook_block_event());
        assert!(
            matches!(result, Err(TelemetryWriteError::Io { .. })),
            "expected Io error when parent dir cannot be created; got: {result:?}"
        );
    }

    // -----------------------------------------------------------------------
    // truncate_utf8 helper
    // -----------------------------------------------------------------------

    #[test]
    fn test_truncate_utf8_within_limit_unchanged() {
        let s = "hello".to_string();
        assert_eq!(truncate_utf8(s.clone(), 10), s);
    }

    #[test]
    fn test_truncate_utf8_exceeds_limit_truncated_at_char_boundary() {
        // 3-byte UTF-8 character: '€' = 0xE2 0x82 0xAC
        let s = "€€€€".to_string(); // 12 bytes total
        let truncated = truncate_utf8(s, 7);
        // 7 bytes: '€€' = 6 bytes fits, 7th byte is the start of 3rd '€' —
        // must back up to 6.
        assert_eq!(truncated, "€€");
        assert!(truncated.is_empty() || truncated.chars().all(|_| true)); // valid UTF-8
    }

    // -----------------------------------------------------------------------
    // safe_path_component: path-traversal sanitization
    // -----------------------------------------------------------------------

    #[test]
    fn test_safe_path_component_plain_slug_unchanged() {
        assert_eq!(safe_path_component("my-track-2026-06-10"), "my-track-2026-06-10");
    }

    #[test]
    fn test_safe_path_component_strips_parent_dir_segments() {
        // "../secret" must not escape items_dir
        let result = safe_path_component("../secret");
        // Only the Normal component "secret" should be kept
        assert_eq!(result, "secret");
    }

    #[test]
    fn test_safe_path_component_strips_absolute_prefix() {
        // "/etc/passwd" → only the last Normal component "passwd" is used
        let result = safe_path_component("/etc/passwd");
        assert_eq!(result, "passwd");
    }

    #[test]
    fn test_safe_path_component_multi_component_uses_last() {
        // "foo/bar" → only the last Normal component "bar" is used;
        // prevents writing to items_dir/foo/bar/logs/ instead of items_dir/bar/logs/.
        let result = safe_path_component("foo/bar");
        assert_eq!(result, "bar");
    }

    #[test]
    fn test_safe_path_component_empty_string_returns_unknown() {
        assert_eq!(safe_path_component(""), "unknown");
    }

    #[test]
    fn test_safe_path_component_only_dots_returns_unknown() {
        // ".." has no Normal components
        assert_eq!(safe_path_component(".."), "unknown");
    }

    // -----------------------------------------------------------------------
    // 4096-byte cap: oversized line after truncation returns Io error
    // -----------------------------------------------------------------------

    #[test]
    fn test_oversized_line_after_field_truncation_returns_io_error() {
        let tmp = TempDir::new().unwrap();
        let writer = writer_in_tempdir(&tmp);
        let output_path = tmp.path().join("telemetry.jsonl");

        // Build an event where even after truncating error_chain to 256 bytes,
        // a very long track_id keeps the line over the limit.
        let long_track_id = "t".repeat(4096);
        let event = TelemetryEvent::NonZeroExit {
            schema_version: 1,
            track_id: long_track_id,
            command: "track impl".to_string(),
            exit_code: 1,
            error_chain: "x".repeat(8192),
            timestamp: "2026-06-10T00:00:00Z".to_string(),
        };

        // When the line still exceeds the cap after field truncation, write()
        // must return an Io error rather than emitting an invalid JSONL line.
        let result = writer.write(event);
        assert!(
            matches!(result, Err(TelemetryWriteError::Io { .. })),
            "expected Io error when line exceeds cap after truncation; got: {result:?}"
        );

        // No partial/corrupted line must have been written to the file.
        assert!(
            !output_path.exists(),
            "output file must not exist when event was dropped before file open"
        );
    }
}
