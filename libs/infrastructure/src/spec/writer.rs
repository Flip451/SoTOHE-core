//! Filesystem adapter implementing `SpecFileWriterPort`.
//!
//! Provides an infrastructure adapter that reads and atomically writes
//! `spec.json` files. Decodes the JSON content via `infrastructure::spec::codec`
//! on read and encodes back via the same codec on write, then uses the shared
//! `atomic_write_file` helper for crash-safe persistence.
//!
//! This keeps the usecase layer (`SpecAdrSignalInteractor`) free of direct
//! filesystem I/O and JSON codec details (hexagonal purity).
//!
//! Extracted from `apps/cli-composition/src/signal.rs:167-194`
//! (`signal_calc_spec_adr`) per ADR 2026-06-21-1328 D4.

use std::path::{Path, PathBuf};

use domain::SpecDocument;
use usecase::spec_adr_signal::{SpecAdrSignalError, SpecFileWriterPort};

use crate::spec::codec as spec_codec;
use crate::track::atomic_write::atomic_write_file;
use crate::track::symlink_guard::reject_symlinks_below;

// ── Adapter ───────────────────────────────────────────────────────────────────

/// Infrastructure adapter implementing [`SpecFileWriterPort`].
///
/// Stateless unit struct; all file paths are passed per-call. Uses
/// `std::fs::read_to_string` + `spec_codec::decode` for reads and
/// `spec_codec::encode` + `atomic_write_file` (tmp-in-same-dir + fsync +
/// rename) for crash-safe atomic writes.
pub struct FsSpecFileWriterAdapter;

impl FsSpecFileWriterAdapter {
    /// Constructs a new stateless adapter.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for FsSpecFileWriterAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl SpecFileWriterPort for FsSpecFileWriterAdapter {
    /// Read and decode the `spec.json` at `path`, returning a [`SpecDocument`].
    ///
    /// # Errors
    ///
    /// Returns [`SpecAdrSignalError::Read`] if the file cannot be read.
    /// Returns [`SpecAdrSignalError::Decode`] if the JSON or domain decode fails.
    fn read_spec_json(&self, path: PathBuf) -> Result<SpecDocument, SpecAdrSignalError> {
        reject_spec_path_symlinks(&path)
            .map_err(|e| map_io_error(&path, e, SpecAdrSignalError::Read))?;
        let content = std::fs::read_to_string(&path)
            .map_err(|e| SpecAdrSignalError::Read(format!("{}: {e}", path.display())))?;
        spec_codec::decode(&content)
            .map_err(|e| SpecAdrSignalError::Decode(format!("{}: {e}", path.display())))
    }

    /// Encode `doc` to JSON and atomically write it to the `spec.json` at `path`.
    ///
    /// The written content is newline-terminated, matching the convention
    /// established by the original `signal_calc_spec_adr` implementation.
    ///
    /// Uses `atomic_write_file` (tmp-in-same-dir + fsync + rename) for
    /// crash-safe writes.
    ///
    /// # Errors
    ///
    /// Returns [`SpecAdrSignalError::Encode`] if JSON encoding fails.
    /// Returns [`SpecAdrSignalError::Write`] if the file cannot be written.
    fn write_spec_json(&self, path: PathBuf, doc: &SpecDocument) -> Result<(), SpecAdrSignalError> {
        let encoded = spec_codec::encode(doc)
            .map_err(|e| SpecAdrSignalError::Encode(format!("{}: {e}", path.display())))?;
        let content = format!("{encoded}\n");
        reject_spec_path_symlinks(&path)
            .map_err(|e| map_io_error(&path, e, SpecAdrSignalError::Write))?;
        atomic_write_file(&path, content.as_bytes())
            .map_err(|e| SpecAdrSignalError::Write(format!("{}: {e}", path.display())))
    }
}

fn reject_spec_path_symlinks(path: &Path) -> std::io::Result<bool> {
    let trusted_root = path.ancestors().last().unwrap_or_else(|| Path::new(""));
    reject_symlinks_below(path, trusted_root)
}

fn map_io_error<F>(path: &Path, error: std::io::Error, wrap: F) -> SpecAdrSignalError
where
    F: FnOnce(String) -> SpecAdrSignalError,
{
    wrap(format!("{}: {error}", path.display()))
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use std::path::PathBuf;

    use tempfile::TempDir;
    use usecase::spec_adr_signal::{SpecAdrSignalError, SpecFileWriterPort};

    use super::FsSpecFileWriterAdapter;

    fn adapter() -> FsSpecFileWriterAdapter {
        FsSpecFileWriterAdapter::new()
    }

    /// Returns a minimal valid spec.json string (schema_version 2, no requirements).
    ///
    /// Uses only the fields the codec accepts. Optional fields with defaults
    /// (`goal`, `constraints`, `acceptance_criteria`, `hearing_history`, etc.)
    /// are omitted so the codec can apply `#[serde(default)]`.
    fn minimal_spec_json() -> String {
        serde_json::json!({
            "schema_version": 2,
            "version": "1.0.0",
            "title": "Test spec",
            "scope": {
                "in_scope": [],
                "out_of_scope": []
            }
        })
        .to_string()
    }

    // ── read_spec_json happy path ─────────────────────────────────────────────

    #[test]
    fn read_spec_json_returns_spec_document_for_valid_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("spec.json");
        std::fs::write(&path, minimal_spec_json()).unwrap();

        let doc = adapter().read_spec_json(path).unwrap();
        assert_eq!(doc.title(), "Test spec", "title must match the fixture spec");
    }

    // ── read_spec_json error paths ────────────────────────────────────────────

    #[test]
    fn read_spec_json_returns_read_error_for_missing_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("nonexistent.json");

        let result = adapter().read_spec_json(path);
        assert!(result.is_err(), "read_spec_json must return an error for a missing file");
        assert!(
            matches!(result, Err(SpecAdrSignalError::Read(_))),
            "error must be the Read variant"
        );
    }

    #[test]
    fn read_spec_json_returns_decode_error_for_invalid_json() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("bad.json");
        std::fs::write(&path, b"not valid json").unwrap();

        let result = adapter().read_spec_json(path);
        assert!(result.is_err(), "read_spec_json must return an error for invalid JSON");
        assert!(
            matches!(result, Err(SpecAdrSignalError::Decode(_))),
            "error must be the Decode variant"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_read_spec_json_rejects_symlink_file() {
        let tmp = TempDir::new().unwrap();
        let real_path = tmp.path().join("real.json");
        let link_path = tmp.path().join("spec.json");
        std::fs::write(&real_path, minimal_spec_json()).unwrap();
        std::os::unix::fs::symlink(&real_path, &link_path).unwrap();

        let result = adapter().read_spec_json(link_path);
        assert!(
            matches!(result, Err(SpecAdrSignalError::Read(_))),
            "symlink file must be rejected as a Read error"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_read_spec_json_rejects_symlink_parent() {
        let tmp = TempDir::new().unwrap();
        let real_dir = tmp.path().join("real");
        let link_dir = tmp.path().join("linked");
        std::fs::create_dir_all(&real_dir).unwrap();
        std::fs::write(real_dir.join("spec.json"), minimal_spec_json()).unwrap();
        std::os::unix::fs::symlink(&real_dir, &link_dir).unwrap();

        let result = adapter().read_spec_json(link_dir.join("spec.json"));
        assert!(
            matches!(result, Err(SpecAdrSignalError::Read(_))),
            "symlink parent must be rejected as a Read error"
        );
    }

    // ── write_spec_json happy path ────────────────────────────────────────────

    #[test]
    fn write_spec_json_creates_file_with_newline_terminated_content() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("spec.json");
        // First write the spec so we can read it back.
        std::fs::write(&path, minimal_spec_json()).unwrap();

        let doc = adapter().read_spec_json(path.clone()).unwrap();
        adapter().write_spec_json(path.clone(), &doc).unwrap();

        let written = std::fs::read_to_string(&path).unwrap();
        assert!(written.ends_with('\n'), "written spec.json must be newline-terminated");
    }

    #[test]
    fn write_spec_json_overwrites_existing_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("spec.json");
        std::fs::write(&path, minimal_spec_json()).unwrap();

        let doc = adapter().read_spec_json(path.clone()).unwrap();
        adapter().write_spec_json(path.clone(), &doc).unwrap();

        // File must still be parseable after overwrite.
        let written = std::fs::read_to_string(&path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&written).unwrap();
        assert_eq!(
            parsed["schema_version"].as_u64().unwrap(),
            2,
            "overwritten spec.json must preserve schema_version 2"
        );
    }

    // ── write_spec_json error paths ───────────────────────────────────────────

    #[test]
    fn write_spec_json_returns_write_error_for_nonexistent_directory() {
        let tmp = TempDir::new().unwrap();
        let path: PathBuf =
            [tmp.path(), std::path::Path::new("nonexistent"), std::path::Path::new("spec.json")]
                .iter()
                .collect();

        // Read from a valid location first to get a SpecDocument.
        let valid_path = tmp.path().join("valid.json");
        std::fs::write(&valid_path, minimal_spec_json()).unwrap();
        let doc = adapter().read_spec_json(valid_path).unwrap();

        let result = adapter().write_spec_json(path, &doc);
        assert!(result.is_err(), "write_spec_json must return an error for a bad path");
        assert!(
            matches!(result, Err(SpecAdrSignalError::Write(_))),
            "error must be the Write variant"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_write_spec_json_rejects_symlink_parent() {
        let tmp = TempDir::new().unwrap();
        let valid_path = tmp.path().join("valid.json");
        std::fs::write(&valid_path, minimal_spec_json()).unwrap();
        let doc = adapter().read_spec_json(valid_path).unwrap();

        let real_dir = tmp.path().join("real");
        let link_dir = tmp.path().join("linked");
        std::fs::create_dir_all(&real_dir).unwrap();
        std::os::unix::fs::symlink(&real_dir, &link_dir).unwrap();

        let result = adapter().write_spec_json(link_dir.join("spec.json"), &doc);
        assert!(
            matches!(result, Err(SpecAdrSignalError::Write(_))),
            "symlink parent must be rejected as a Write error"
        );
    }

    // ── round-trip ────────────────────────────────────────────────────────────

    #[test]
    fn write_then_read_round_trips_spec_document() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("spec.json");
        std::fs::write(&path, minimal_spec_json()).unwrap();

        let a = adapter();
        let original = a.read_spec_json(path.clone()).unwrap();
        a.write_spec_json(path.clone(), &original).unwrap();
        let round_tripped = a.read_spec_json(path).unwrap();

        assert_eq!(original.title(), round_tripped.title(), "title must survive round-trip");
        assert_eq!(original.version(), round_tripped.version(), "version must survive round-trip");
    }
}
