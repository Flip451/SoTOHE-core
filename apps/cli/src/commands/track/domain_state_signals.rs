//! `sotp track domain-type-signals` — evaluate domain type signals via rustdoc schema export.
//!
//! Reads `domain-types.json` from the track directory, exports the domain crate's
//! public API via rustdoc JSON, evaluates signals for each declared type, and writes
//! the updated document back to `domain-types.json`.

use std::path::PathBuf;
use std::process::ExitCode;

use domain::schema::{SchemaExportError, SchemaExporter};
use infrastructure::code_profile_builder::build_code_profile;
use infrastructure::domain_types_codec;
use infrastructure::schema_export::RustdocSchemaExporter;
use infrastructure::track::atomic_write::atomic_write_file;

use crate::CliError;

/// Evaluate domain type signals via rustdoc schema export and write back to `domain-types.json`.
///
/// Steps:
/// 1. Read `domain-types.json` from `<items_dir>/<track_id>/`.
/// 2. Export the `domain` crate's public API using `RustdocSchemaExporter`.
/// 3. Call `domain::evaluate_domain_type_signals()` with the entries and schema.
/// 4. Set the signals on the document and write back to `domain-types.json`.
/// 5. Print a signal summary to stdout.
///
/// # Errors
///
/// Returns `CliError` when the track ID is invalid, the file cannot be read or
/// decoded, rustdoc export fails (e.g., nightly not installed), or the write fails.
pub fn execute_domain_type_signals(
    items_dir: PathBuf,
    track_id: String,
    workspace_root: PathBuf,
) -> Result<ExitCode, CliError> {
    // Validate track_id to prevent path traversal.
    let _valid_id = domain::TrackId::try_new(&track_id)
        .map_err(|e| CliError::Message(format!("invalid track ID: {e}")))?;

    let track_dir = items_dir.join(&track_id);
    let domain_types_path = track_dir.join("domain-types.json");

    // Read and decode domain-types.json.
    let json = std::fs::read_to_string(&domain_types_path).map_err(|e| {
        CliError::Message(format!("cannot read {}: {e}", domain_types_path.display()))
    })?;

    let mut doc = domain_types_codec::decode(&json)
        .map_err(|e| CliError::Message(format!("domain-types.json decode error: {e}")))?;

    // Export the domain crate's public API via rustdoc JSON.
    let exporter = RustdocSchemaExporter::new(workspace_root);
    let schema = exporter.export("domain").map_err(|e| {
        let hint = if matches!(e, SchemaExportError::NightlyNotFound) {
            " (install with: rustup toolchain install nightly)".to_owned()
        } else {
            String::new()
        };
        CliError::Message(format!("failed to export schema: {e}{hint}"))
    })?;

    // Build a pre-indexed CodeProfile from the flat schema export.
    let profile = build_code_profile(&schema);

    // Evaluate signals.
    let signals = domain::evaluate_domain_type_signals(doc.entries(), &profile);

    // Count by signal level for the summary.
    let blue = signals.iter().filter(|s| s.signal() == domain::ConfidenceSignal::Blue).count();
    let red = signals.iter().filter(|s| s.signal() == domain::ConfidenceSignal::Red).count();

    doc.set_signals(signals);

    // Encode and write back.
    let encoded = domain_types_codec::encode(&doc)
        .map_err(|e| CliError::Message(format!("domain-types.json encode error: {e}")))?;

    atomic_write_file(&domain_types_path, format!("{encoded}\n").as_bytes()).map_err(|e| {
        CliError::Message(format!("cannot write {}: {e}", domain_types_path.display()))
    })?;

    // Re-render domain-types.md so the view stays in sync.
    let domain_types_md_path = track_dir.join("domain-types.md");
    let rendered = infrastructure::domain_types_render::render_domain_types(&doc);
    atomic_write_file(&domain_types_md_path, rendered.as_bytes()).map_err(|e| {
        CliError::Message(format!("cannot write {}: {e}", domain_types_md_path.display()))
    })?;

    let total = doc.entries().len();
    println!("[OK] domain-type-signals: blue={blue} red={red} (total={total})",);

    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    /// Sets up a minimal track directory with the given `domain-types.json` content.
    fn setup_track(dir: &std::path::Path, domain_types: &str) -> (PathBuf, String) {
        let items_dir = dir.join("track/items");
        let track_id = "test-track";
        let track_dir = items_dir.join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("domain-types.json"), domain_types).unwrap();
        (items_dir, track_id.to_owned())
    }

    #[test]
    fn test_execute_domain_type_signals_with_invalid_track_id_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        std::fs::create_dir_all(&items_dir).unwrap();
        let workspace_root = dir.path().to_path_buf();

        let result = execute_domain_type_signals(items_dir, "../evil".to_owned(), workspace_root);
        assert!(result.is_err(), "path traversal track_id must be rejected");
    }

    #[test]
    fn test_execute_domain_type_signals_with_missing_domain_types_json_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        std::fs::create_dir_all(items_dir.join("test-track")).unwrap();
        let workspace_root = dir.path().to_path_buf();

        let result =
            execute_domain_type_signals(items_dir, "test-track".to_owned(), workspace_root);
        assert!(result.is_err(), "missing domain-types.json must return error");
    }

    #[test]
    fn test_execute_domain_type_signals_with_malformed_domain_types_json_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let (items_dir, track_id) = setup_track(dir.path(), "{not valid json}");
        let workspace_root = dir.path().to_path_buf();

        let result = execute_domain_type_signals(items_dir, track_id, workspace_root);
        assert!(result.is_err(), "malformed domain-types.json must return error");
    }
}
