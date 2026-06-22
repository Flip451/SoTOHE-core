//! Filesystem adapter implementing `ArchivedTrackTelemetryPort`.
//!
//! Mirrors the I/O pattern from `apps/cli/src/main.rs:247-294`
//! (`emit_archived_track_subcommand`), which is the source being extracted
//! in T009. This adapter:
//!
//! 1. Constructs the telemetry JSON object with `subcommand` and a RFC-3339
//!    timestamp (produced via `chrono::Utc::now()`).
//! 2. Serializes to JSONL bytes (`serde_json::to_vec`).
//! 3. Creates the telemetry directory with `std::fs::create_dir_all`.
//! 4. Appends a newline-terminated event to the JSONL file using
//!    `std::fs::OpenOptions` with `append(true).create(true)`.
//!
//! Timestamp capture intentionally lives here (infrastructure), keeping the
//! usecase layer free of `chrono` (hexagonal purity).

use std::fs::{File, OpenOptions};
use std::io::Write as _;
use std::path::{Path, PathBuf};

use usecase::telemetry::{ArchivedTrackTelemetryError, ArchivedTrackTelemetryPort};

// ── Adapter ───────────────────────────────────────────────────────────────────

/// Infrastructure adapter implementing [`ArchivedTrackTelemetryPort`].
///
/// Persists a single JSONL telemetry event to `<telemetry_dir>/telemetry.jsonl`
/// on each `emit` call. The telemetry directory is injected at construction time
/// by the composition root.
///
/// Timestamp capture stays inside this adapter; the usecase layer receives no
/// `chrono` dependency.
pub struct FsArchivedTrackTelemetryAdapter {
    telemetry_dir: PathBuf,
}

impl FsArchivedTrackTelemetryAdapter {
    /// Constructs a new adapter that writes to `<telemetry_dir>/telemetry.jsonl`.
    ///
    /// No I/O is performed at construction time.
    #[must_use]
    pub fn new(telemetry_dir: PathBuf) -> Self {
        Self { telemetry_dir }
    }
}

impl ArchivedTrackTelemetryPort for FsArchivedTrackTelemetryAdapter {
    /// Emit a telemetry event for `subcommand` by appending a JSONL line to
    /// `<telemetry_dir>/telemetry.jsonl`.
    ///
    /// # Errors
    ///
    /// Returns [`ArchivedTrackTelemetryError::Io`] on directory creation failure,
    /// file open failure, or a short write.
    /// Returns [`ArchivedTrackTelemetryError::Serialize`] when `serde_json`
    /// fails to serialize the event object.
    fn emit(
        &self,
        track_id: String,
        subcommand: String,
    ) -> Result<(), ArchivedTrackTelemetryError> {
        let timestamp = chrono::Utc::now().to_rfc3339();
        // Emit the canonical TelemetryEvent::TrackSubcommand JSONL schema so the
        // downstream `sotp telemetry report` pipeline can deserialize archived
        // events alongside active-track events. `exit_code` and `duration_ms`
        // are not tracked at archive time; default to 0.
        let event = serde_json::json!({
            "event_type": "TrackSubcommand",
            "schema_version": 1,
            "track_id": track_id,
            "command": subcommand,
            "exit_code": 0,
            "duration_ms": 0,
            "timestamp": timestamp,
        });

        let mut bytes = serde_json::to_vec(&event)
            .map_err(|e| ArchivedTrackTelemetryError::Serialize(e.to_string()))?;
        bytes.push(b'\n');

        let path = self.telemetry_dir.join("telemetry.jsonl");
        reject_symlinks_from_root(&self.telemetry_dir)
            .map_err(|e| ArchivedTrackTelemetryError::Io(e.to_string()))?;
        std::fs::create_dir_all(&self.telemetry_dir)
            .map_err(|e| ArchivedTrackTelemetryError::Io(e.to_string()))?;
        reject_symlinks_from_root(&path)
            .map_err(|e| ArchivedTrackTelemetryError::Io(e.to_string()))?;

        let mut file = open_append_no_follow(&path)
            .map_err(|e| ArchivedTrackTelemetryError::Io(e.to_string()))?;

        let written =
            file.write(&bytes).map_err(|e| ArchivedTrackTelemetryError::Io(e.to_string()))?;
        if written != bytes.len() {
            return Err(ArchivedTrackTelemetryError::Io(format!(
                "short write for telemetry file {}: wrote {written} of {} bytes",
                path.display(),
                bytes.len()
            )));
        }

        Ok(())
    }
}

fn reject_symlinks_from_root(path: &Path) -> Result<(), std::io::Error> {
    let absolute_path =
        if path.is_absolute() { path.to_path_buf() } else { std::env::current_dir()?.join(path) };

    let mut components: Vec<&Path> = absolute_path.ancestors().collect();
    components.reverse();

    for component in components {
        if component.as_os_str().is_empty() {
            continue;
        }
        match component.symlink_metadata() {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("refusing to follow symlink: {}", component.display()),
                ));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(std::io::Error::new(
                    e.kind(),
                    format!("failed to stat {}: {e}", component.display()),
                ));
            }
        }
    }

    Ok(())
}

fn open_append_no_follow(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.append(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    options.open(path)
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use std::io::BufRead as _;

    use tempfile::TempDir;

    use super::FsArchivedTrackTelemetryAdapter;
    use usecase::telemetry::ArchivedTrackTelemetryPort;

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn adapter_in_tempdir(tmp: &TempDir) -> FsArchivedTrackTelemetryAdapter {
        FsArchivedTrackTelemetryAdapter::new(tmp.path().to_path_buf())
    }

    // ── Tests ─────────────────────────────────────────────────────────────────

    #[test]
    fn emit_writes_one_jsonl_line_to_telemetry_file() {
        let tmp = TempDir::new().unwrap();
        let adapter = adapter_in_tempdir(&tmp);

        adapter.emit("archived-2026-06-22".to_string(), "track spec-design".to_string()).unwrap();

        let path = tmp.path().join("telemetry.jsonl");
        assert!(path.exists(), "telemetry.jsonl must be created after emit");

        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 1, "exactly one JSONL line must be written");

        let parsed: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(
            parsed["event_type"].as_str().unwrap(),
            "TrackSubcommand",
            "event_type must be the canonical TelemetryEvent::TrackSubcommand tag"
        );
        assert_eq!(parsed["schema_version"].as_u64().unwrap(), 1);
        assert_eq!(parsed["track_id"].as_str().unwrap(), "archived-2026-06-22");
        assert_eq!(
            parsed["command"].as_str().unwrap(),
            "track spec-design",
            "command must match the emitted subcommand"
        );
        assert_eq!(parsed["exit_code"].as_i64().unwrap(), 0);
        assert_eq!(parsed["duration_ms"].as_u64().unwrap(), 0);
        assert!(
            parsed.get("timestamp").is_some(),
            "timestamp field must be present in the JSONL line"
        );
    }

    #[test]
    fn emit_appends_multiple_lines_on_repeated_calls() {
        let tmp = TempDir::new().unwrap();
        let adapter = adapter_in_tempdir(&tmp);

        adapter.emit("t1".to_string(), "track init".to_string()).unwrap();
        adapter.emit("t1".to_string(), "track review".to_string()).unwrap();
        adapter.emit("t1".to_string(), "track commit".to_string()).unwrap();

        let path = tmp.path().join("telemetry.jsonl");
        let file = std::fs::File::open(&path).unwrap();
        let lines: Vec<String> =
            std::io::BufReader::new(file).lines().collect::<Result<_, _>>().unwrap();

        assert_eq!(lines.len(), 3, "three JSONL lines must be written after three emits");

        let cmds: Vec<String> = lines
            .iter()
            .map(|l| {
                serde_json::from_str::<serde_json::Value>(l).unwrap()["command"]
                    .as_str()
                    .unwrap()
                    .to_owned()
            })
            .collect();
        assert_eq!(
            cmds,
            &["track init", "track review", "track commit"],
            "command field must match the emitted subcommand in order"
        );
    }

    #[test]
    fn emit_returns_io_error_when_directory_is_unwritable() {
        // Use a path inside an existing file as the "directory" — create_dir_all
        // will fail because a regular file blocks the mkdir.
        let tmp = TempDir::new().unwrap();
        let blocker = tmp.path().join("blocker");
        std::fs::write(&blocker, b"").unwrap();

        // telemetry_dir points to a path whose parent is a regular file — creates
        // a hierarchy that create_dir_all cannot satisfy.
        let bad_dir = blocker.join("sub");
        let adapter = FsArchivedTrackTelemetryAdapter::new(bad_dir);

        let result = adapter.emit("test-track".to_string(), "track init".to_string());
        assert!(
            result.is_err(),
            "emit must return an error when the telemetry directory cannot be created"
        );
        assert!(
            matches!(result, Err(usecase::telemetry::ArchivedTrackTelemetryError::Io(_))),
            "error must be the Io variant"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_emit_with_symlinked_telemetry_dir_returns_io_error() {
        let tmp = TempDir::new().unwrap();
        let real_dir = tmp.path().join("real");
        let link_dir = tmp.path().join("link");
        std::fs::create_dir_all(&real_dir).unwrap();
        std::os::unix::fs::symlink(&real_dir, &link_dir).unwrap();

        let adapter = FsArchivedTrackTelemetryAdapter::new(link_dir);
        let result = adapter.emit("test-track".to_string(), "track init".to_string());

        assert!(
            matches!(result, Err(usecase::telemetry::ArchivedTrackTelemetryError::Io(_))),
            "symlinked telemetry directory must be rejected as an Io error"
        );
        assert!(
            !real_dir.join("telemetry.jsonl").exists(),
            "emit must not follow the symlinked telemetry directory"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_emit_with_symlinked_telemetry_file_returns_io_error() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("redirect-target.jsonl");
        let link = tmp.path().join("telemetry.jsonl");
        std::fs::write(&target, b"existing\n").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let adapter = adapter_in_tempdir(&tmp);
        let result = adapter.emit("test-track".to_string(), "track init".to_string());

        assert!(
            matches!(result, Err(usecase::telemetry::ArchivedTrackTelemetryError::Io(_))),
            "symlinked telemetry file must be rejected as an Io error"
        );
        assert_eq!(
            std::fs::read_to_string(&target).unwrap(),
            "existing\n",
            "emit must not append through a symlinked telemetry file"
        );
    }

    #[test]
    fn emitted_line_contains_rfc3339_timestamp() {
        let tmp = TempDir::new().unwrap();
        let adapter = adapter_in_tempdir(&tmp);

        adapter.emit("t-impl".to_string(), "track impl".to_string()).unwrap();

        let content = std::fs::read_to_string(tmp.path().join("telemetry.jsonl")).unwrap();
        let line = content.lines().next().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(line).unwrap();
        let ts = parsed["timestamp"].as_str().unwrap();

        // RFC-3339 timestamps must contain at least one 'T' separator.
        assert!(ts.contains('T'), "timestamp must be in RFC-3339 format (contains 'T'); got: {ts}");
    }
}
