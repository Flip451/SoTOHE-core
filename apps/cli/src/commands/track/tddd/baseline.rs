//! `sotp track baseline-capture` — capture TypeGraph snapshot as baseline.
//!
//! Generates `domain-types-baseline.json` from the current TypeGraph.
//! Skips if baseline already exists (idempotent). Use `--force` to regenerate.

use std::path::PathBuf;
use std::process::ExitCode;

use domain::schema::{SchemaExportError, SchemaExporter};
use infrastructure::code_profile_builder::build_type_graph;
use infrastructure::schema_export::RustdocSchemaExporter;
use infrastructure::tddd::{baseline_builder, baseline_codec};
use infrastructure::track::atomic_write::atomic_write_file;
use infrastructure::track::symlink_guard::reject_symlinks_below;

use crate::CliError;

/// Capture the current TypeGraph as a baseline snapshot for TDDD reverse signal filtering.
///
/// Steps:
/// 1. Check if `domain-types-baseline.json` already exists (skip unless `--force`).
/// 2. Export domain crate schema via rustdoc JSON.
/// 3. Build TypeGraph and convert to TypeBaseline.
/// 4. Encode and write to `domain-types-baseline.json`.
///
/// # Errors
///
/// Returns `CliError` when the track ID is invalid, rustdoc export fails,
/// or the file write fails.
pub fn execute_baseline_capture(
    items_dir: PathBuf,
    track_id: String,
    workspace_root: PathBuf,
    force: bool,
) -> Result<ExitCode, CliError> {
    let _valid_id = domain::TrackId::try_new(&track_id)
        .map_err(|e| CliError::Message(format!("invalid track ID: {e}")))?;

    // Security: verify the items_dir root itself is not a symlink before using it as the
    // trusted anchor. reject_symlinks_below only checks components *below* the trusted_root,
    // so a symlinked items_dir would pass through undetected.
    match items_dir.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(CliError::Message(format!(
                "symlink guard: refusing to follow symlink at items_dir: {}",
                items_dir.display()
            )));
        }
        Ok(_) => {}
        Err(e) => {
            return Err(CliError::Message(format!(
                "symlink guard: cannot stat items_dir {}: {e}",
                items_dir.display()
            )));
        }
    }

    let track_dir = items_dir.join(&track_id);
    let baseline_path = track_dir.join("domain-types-baseline.json");

    // Security: reject symlinks in path components below items_dir.
    reject_symlinks_below(&baseline_path, &items_dir)
        .map_err(|e| CliError::Message(format!("symlink guard: {e}")))?;

    // Idempotent: skip if baseline already exists as a regular file (unless --force).
    // Use is_file() rather than exists() so that a directory or other non-file node at
    // that path does not silently produce a spurious success — it falls through and will
    // fail at the write step with a meaningful error instead.
    if baseline_path.is_file() && !force {
        println!(
            "[OK] baseline-capture: domain-types-baseline.json already exists for '{track_id}' (use --force to regenerate)"
        );
        return Ok(ExitCode::SUCCESS);
    }

    // Fail fast if the track directory does not exist.
    if !track_dir.is_dir() {
        return Err(CliError::Message(format!(
            "track directory not found: {} (did you mean an existing track ID?)",
            track_dir.display()
        )));
    }

    // Read domain-types.json to get typestate names for build_type_graph.
    // Security: guard the leaf path too — a symlinked domain-types.json inside the
    // track directory could redirect reads outside the trusted tree.
    let domain_types_path = track_dir.join("domain-types.json");
    reject_symlinks_below(&domain_types_path, &items_dir)
        .map_err(|e| CliError::Message(format!("symlink guard: {e}")))?;
    let typestate_names: std::collections::HashSet<String> =
        if let Ok(json) = std::fs::read_to_string(&domain_types_path) {
            if let Ok(doc) = infrastructure::tddd::catalogue_codec::decode(&json) {
                doc.typestate_names()
            } else {
                std::collections::HashSet::new()
            }
        } else {
            std::collections::HashSet::new()
        };

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

    // Build TypeGraph and convert to TypeBaseline.
    let graph = build_type_graph(&schema, &typestate_names);
    let captured_at = infrastructure::timestamp_now()
        .map_err(|e| CliError::Message(format!("timestamp error: {e}")))?;
    let baseline = baseline_builder::build_baseline(&graph, captured_at);

    // Encode and write.
    let encoded = baseline_codec::encode(&baseline)
        .map_err(|e| CliError::Message(format!("baseline encode error: {e}")))?;

    atomic_write_file(&baseline_path, format!("{encoded}\n").as_bytes())
        .map_err(|e| CliError::Message(format!("cannot write {}: {e}", baseline_path.display())))?;

    let type_count = baseline.types().len();
    let trait_count = baseline.traits().len();
    println!(
        "[OK] baseline-capture: wrote domain-types-baseline.json ({type_count} types, {trait_count} traits)"
    );

    Ok(ExitCode::SUCCESS)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_baseline_capture_with_invalid_track_id_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        std::fs::create_dir_all(&items_dir).unwrap();

        let result =
            execute_baseline_capture(items_dir, "../evil".to_owned(), dir.path().into(), false);
        assert!(result.is_err(), "path traversal track_id must be rejected");
    }

    #[test]
    fn test_baseline_capture_skips_when_baseline_exists() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("test-track");
        std::fs::create_dir_all(&track_dir).unwrap();

        // Write a dummy baseline file.
        std::fs::write(track_dir.join("domain-types-baseline.json"), "{}").unwrap();

        let result =
            execute_baseline_capture(items_dir, "test-track".to_owned(), dir.path().into(), false);
        assert!(result.is_ok(), "should skip existing baseline without error");
    }

    /// `--force` bypasses the skip check and proceeds to rustdoc export.
    ///
    /// In a test environment nightly rustdoc is not available, so the call fails
    /// with a schema export error rather than returning `Ok(SUCCESS)`.
    #[test]
    fn test_baseline_capture_force_flag_bypasses_skip() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("test-track");
        std::fs::create_dir_all(&track_dir).unwrap();

        // Pre-existing baseline — would trigger skip without --force.
        std::fs::write(track_dir.join("domain-types-baseline.json"), "{}").unwrap();

        let result =
            execute_baseline_capture(items_dir, "test-track".to_owned(), dir.path().into(), true);
        assert!(result.is_err(), "--force must bypass skip and attempt export");
    }
}
