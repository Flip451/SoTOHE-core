//! `sotp track three-way-signals` — run Phase 1 + Phase 2 3-way signal evaluation.
//!
//! Loads `<layer>-types.json` (CatalogueDocument / TypeGraph A), reads
//! `<layer>-types-baseline.json` as TypeGraph B (`rustdoc_types::Crate`), captures
//! TypeGraph C live via `cargo +nightly rustdoc`, and evaluates the 3-way
//! `SignalEvaluatorV2` producing a `ThreeWayEvaluationReport`.
//!
//! Exits with code 1 when any Red signals are present.

use std::fmt::Write as FmtWrite;
use std::path::PathBuf;
use std::process::ExitCode;

use infrastructure::schema_export::RustdocSchemaExporter;
use infrastructure::tddd::baseline_rustdoc_codec::BaselineRustdocCodec;
use infrastructure::tddd::catalogue_document_codec::CatalogueDocumentCodec;
use infrastructure::tddd::catalogue_to_extended_crate_codec::CatalogueToExtendedCrateCodec;
use infrastructure::tddd::signal_evaluator_v2::SignalEvaluatorV2;
use infrastructure::tddd::{
    CatalogueToExtendedCratePort, SignalEvaluatorPort, ThreeWaySignal, ThreeWaySignalKind,
};
use infrastructure::track::atomic_write::atomic_write_file;
use infrastructure::track::fs_store::read_track_status_str;
use infrastructure::track::symlink_guard::reject_symlinks_below;

use crate::CliError;
use crate::commands::track::tddd::signals::{ensure_active_track, resolve_layers};

/// Normalize a bare filename so that its parent directory component is non-empty.
///
/// `atomic_write_file` opens the parent directory for fsync; if the path is a
/// bare filename like `"report.md"`, `Path::parent()` returns `""`, which
/// causes `open("")` to fail with `NotFound`.  Prefixing with `"."` produces
/// `"./report.md"` whose parent is `"."`, fixing the issue.
///
/// Paths that already have a non-empty parent (absolute or relative with a
/// directory component) are returned unchanged.
fn normalize_output_path(out_path: &std::path::Path) -> std::path::PathBuf {
    if out_path.parent().map(|p| p == std::path::Path::new("")).unwrap_or(false) {
        std::path::Path::new(".").join(out_path)
    } else {
        out_path.to_path_buf()
    }
}

/// Write accumulated markdown output to a file.
///
/// Uses `normalize_output_path` to ensure the parent directory component is
/// non-empty, then delegates to `atomic_write_file` to avoid truncating an
/// existing report on an interrupted write.
///
/// # Errors
///
/// Returns `CliError::Message` when the atomic write fails.
fn write_output_file(out_path: &std::path::Path, content: &str) -> Result<(), CliError> {
    let canonical_out = normalize_output_path(out_path);
    atomic_write_file(&canonical_out, content.as_bytes()).map_err(|e| {
        CliError::Message(format!("failed to write output to '{}': {e}", out_path.display()))
    })
}

/// Execute the `track three-way-signals` command.
///
/// For each TDDD-enabled layer (or the single layer specified by `--layer`):
///
/// 1. Load `<layer>-types.json` via `CatalogueDocumentCodec` → `CatalogueDocument`.
/// 2. Convert to `ExtendedCrate` (TypeGraph A) via `CatalogueToExtendedCrateCodec`.
/// 3. Load `<layer>-types-baseline.json` via `BaselineRustdocCodec` → TypeGraph B.
/// 4. Capture TypeGraph C via `cargo +nightly rustdoc`.
/// 5. Run `SignalEvaluatorV2::evaluate(A, B, C)` → `ThreeWayEvaluationReport`.
/// 6. Print the report as a markdown table.
///
/// Exits with code 1 if any Red signals are found across all layers.
///
/// # Errors
///
/// Returns [`CliError`] when the track ID is invalid, any file is missing, or
/// the evaluation fails.
#[allow(clippy::too_many_lines)]
pub fn execute_three_way_signals(
    items_dir: PathBuf,
    track_id: String,
    workspace_root: PathBuf,
    layer: Option<String>,
    output: Option<PathBuf>,
) -> Result<ExitCode, CliError> {
    // Security: verify workspace_root is not a symlink before passing it to
    // resolve_layers (which reads architecture-rules.json from workspace_root) and to
    // RustdocSchemaExporter. A symlinked workspace root would redirect the rules load
    // and the rustdoc build to an unintended tree.  Must be checked before resolve_layers
    // so the symlink guard cannot be bypassed by loading a forged architecture-rules.json.
    // Mirrors the guard in `baseline_capture::capture_rustdoc_baseline_for_layer`
    // (infrastructure layer).
    match workspace_root.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(CliError::Message(format!(
                "symlink guard: refusing to follow symlink at workspace_root: {}",
                workspace_root.display()
            )));
        }
        Ok(_) => {}
        Err(e) => {
            return Err(CliError::Message(format!(
                "symlink guard: cannot stat workspace_root {}: {e}",
                workspace_root.display()
            )));
        }
    }

    let bindings = resolve_layers(&workspace_root, layer.as_deref())?;

    if bindings.is_empty() {
        return Err(CliError::Message(
            "no tddd.enabled layers found in architecture-rules.json".to_owned(),
        ));
    }

    // Validate track_id and guard against frozen tracks (done / archived) before any I/O.
    // Mirrors the same pattern used by `execute_type_signals` (signals.rs): read_track_status_str
    // validates the id and loads impl-plan fail-closed; ensure_active_track rejects completed tracks.
    let status_str = read_track_status_str(&items_dir, &track_id).map_err(|e| {
        CliError::Message(format!("cannot load track status for '{track_id}': {e}"))
    })?;
    ensure_active_track(&status_str, &track_id)?;

    // Security: verify the items_dir root itself is not a symlink before using it as the
    // trusted anchor for `reject_symlinks_below`. That helper only checks components
    // *below* the trusted_root, so a symlinked items_dir would bypass all path guards.
    // Mirrors `execute_baseline_capture` (baseline.rs).
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

    // Security: verify the track directory itself is not a symlink before joining
    // catalogue / baseline beneath it. A symlinked track directory would escape the
    // trusted tree before `reject_symlinks_below` (anchored at `items_dir`) can catch it.
    // Mirrors `execute_catalogue_spec_signals` (catalogue_spec_signals.rs).
    match track_dir.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(CliError::Message(format!(
                "symlink guard: refusing to follow symlink at track directory: {}",
                track_dir.display()
            )));
        }
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Track directory absent — the catalogue read below will produce a
            // clear error message. Don't short-circuit here.
        }
        Err(e) => {
            return Err(CliError::Message(format!(
                "symlink guard: cannot stat track directory {}: {e}",
                track_dir.display()
            )));
        }
    }

    let ext_crate_codec = CatalogueToExtendedCrateCodec::new();
    let evaluator = SignalEvaluatorV2::new();
    let exporter = RustdocSchemaExporter::new(workspace_root.clone());

    let mut any_red = false;
    // Accumulate markdown for optional file output.
    let mut file_buf = String::new();

    for binding in &bindings {
        let layer_id = binding.layer_id();
        let catalogue_filename = binding.catalogue_file();
        let baseline_filename = binding.baseline_file();

        // --- Step 1: Load catalogue document (TypeGraph A source) ---
        let catalogue_path = track_dir.join(catalogue_filename);
        // Security: reject symlinks at the catalogue path and every ancestor below items_dir.
        // Mirrors `execute_spec_element_hash` (spec_element_hash.rs).
        reject_symlinks_below(&catalogue_path, &items_dir).map_err(|e| {
            CliError::Message(format!(
                "symlink guard: refusing to read catalogue '{}': {e}",
                catalogue_path.display()
            ))
        })?;
        let doc = CatalogueDocumentCodec::load(&catalogue_path).map_err(|e| {
            CliError::Message(format!(
                "failed to load catalogue '{}': {e}",
                catalogue_path.display()
            ))
        })?;

        // --- Step 2: Convert CatalogueDocument → ExtendedCrate (A) ---
        let extended_a = ext_crate_codec.encode(doc).map_err(|e| {
            CliError::Message(format!(
                "CatalogueToExtendedCrateCodec error for layer '{layer_id}': {e}"
            ))
        })?;

        // --- Step 3: Load baseline (TypeGraph B) ---
        let baseline_path = track_dir.join(&baseline_filename);
        // Security: reject symlinks at the baseline path and every ancestor below items_dir.
        // Mirrors `execute_spec_element_hash` (spec_element_hash.rs).
        let baseline_exists = reject_symlinks_below(&baseline_path, &items_dir).map_err(|e| {
            CliError::Message(format!(
                "symlink guard: refusing to read baseline '{}': {e}",
                baseline_path.display()
            ))
        })?;
        if !baseline_exists {
            return Err(CliError::Message(format!(
                "baseline file not found: {} — run `sotp track baseline-capture {}` first \
                 (rustdoc format; delete old TypeBaseline JSON if present and re-capture)",
                baseline_path.display(),
                track_id,
            )));
        }
        let baseline_b = BaselineRustdocCodec::load(&baseline_path).map_err(|e| {
            CliError::Message(format!("failed to load baseline '{}': {e}", baseline_path.display()))
        })?;

        // --- Step 4: Capture current TypeGraph (C) via rustdoc ---
        let target_crate = match binding.targets() {
            [single] => single,
            [] => {
                return Err(CliError::Message(format!(
                    "schema_export.targets is empty for layer '{layer_id}'"
                )));
            }
            multi => {
                return Err(CliError::Message(format!(
                    "layer '{layer_id}' has {} schema_export.targets — multi-target not yet supported",
                    multi.len()
                )));
            }
        };

        let json_path = exporter.export_rustdoc_json_path(target_crate).map_err(|e| {
            CliError::Message(format!(
                "rustdoc export failed for crate '{target_crate}' (layer '{layer_id}'): {e}"
            ))
        })?;
        let json_content =
            std::fs::read_to_string(&json_path).map_err(|e| CliError::Message(e.to_string()))?;
        let current_c = BaselineRustdocCodec::from_json(&json_content).map_err(|e| {
            CliError::Message(format!(
                "failed to parse rustdoc JSON for crate '{target_crate}': {e}"
            ))
        })?;

        // --- Step 5: Evaluate ---
        let report = evaluator.evaluate(extended_a, baseline_b, current_c).map_err(|e| {
            CliError::Message(format!("signal evaluation error for layer '{layer_id}': {e:?}"))
        })?;

        // --- Step 6: Render ---
        // Build the layer section as a string so we can write to stdout and optionally to a file.
        let mut section = String::new();
        let _ = writeln!(section);
        let _ = writeln!(section, "## Layer: `{layer_id}`");
        let _ = writeln!(section);

        if report.is_empty() {
            let _ = writeln!(section, "All items maintained (no non-skip signals).");
        } else {
            let _ = writeln!(section, "| Item | Region | Signal |");
            let _ = writeln!(section, "|------|--------|--------|");
            for signal in report.iter() {
                let kind_str = match signal.signal() {
                    ThreeWaySignalKind::Blue => "🔵 Blue",
                    ThreeWaySignalKind::Yellow => "🟡 Yellow",
                    ThreeWaySignalKind::Red => "🔴 Red",
                    ThreeWaySignalKind::Skip => "Skip",
                };
                let region_str = format!("{:?}", signal.region());
                let _ =
                    writeln!(section, "| {} | {} | {} |", signal.item_name(), region_str, kind_str);
                if signal.signal().is_red() {
                    any_red = true;
                }
            }
            let _ = writeln!(section);
            let blue = report.iter().filter(|s: &&ThreeWaySignal| s.signal().is_blue()).count();
            let yellow = report.iter().filter(|s: &&ThreeWaySignal| s.signal().is_yellow()).count();
            let red = report.iter().filter(|s: &&ThreeWaySignal| s.signal().is_red()).count();
            let _ =
                writeln!(section, "Summary: 🔵 {blue} Blue | 🟡 {yellow} Yellow | 🔴 {red} Red");
        }

        print!("{section}");
        if output.is_some() {
            file_buf.push_str(&section);
        }
    }

    // Write to file if --output was specified.
    // Use atomic_write_file (via write_output_file) to avoid truncating an existing report
    // on interrupted write (mirrors the atomic write pattern used by other track artifact writers).
    if let Some(out_path) = &output {
        write_output_file(out_path, &file_buf)?;
    }

    if any_red { Ok(ExitCode::FAILURE) } else { Ok(ExitCode::SUCCESS) }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_three_way_signals_invalid_track_id_returns_error() {
        let tmp = tempfile::tempdir().unwrap();
        let items_dir = tmp.path().join("items");
        std::fs::create_dir_all(&items_dir).unwrap();
        let result = execute_three_way_signals(
            items_dir,
            "bad track id!!".to_owned(),
            tmp.path().into(),
            None,
            None,
        );
        assert!(result.is_err(), "invalid track ID must return Err");
        let msg = format!("{}", result.unwrap_err());
        // read_track_status_str validates the track id via domain::TrackId::try_new and
        // returns "cannot load track status for '...': ... invalid track id: ..."
        assert!(
            msg.contains("invalid track id") || msg.contains("invalid track ID"),
            "error must mention invalid track id: {msg}"
        );
    }

    #[test]
    fn test_execute_three_way_signals_missing_catalogue_file_returns_error() {
        // Track dir exists but contains no catalogue files.
        let tmp = tempfile::tempdir().unwrap();
        let items_dir = tmp.path().join("items");
        let track_id = "test-track-2026-01-01";
        let track_dir = items_dir.join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();

        // Write minimal metadata.json (in-progress, branch set) so the frozen-track guard passes.
        let metadata = serde_json::json!({
            "schema_version": 5,
            "id": track_id,
            "branch": format!("track/{track_id}"),
            "title": "Test",
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-01-01T00:00:00Z"
        });
        std::fs::write(
            track_dir.join("metadata.json"),
            serde_json::to_string_pretty(&metadata).unwrap(),
        )
        .unwrap();
        // Write a minimal impl-plan (no tasks → status = in_progress / no tasks done).
        let impl_plan = r#"{"schema_version":1,"tasks":[],"plan":{"summary":[],"sections":[]}}"#;
        std::fs::write(track_dir.join("impl-plan.json"), impl_plan).unwrap();

        // Create a minimal architecture-rules.json so resolve_layers can find a layer.
        let workspace_root = tmp.path().to_path_buf();
        // architecture-rules.json uses "crate" key (not "id") per TdddLayerBinding schema.
        let arch_rules = serde_json::json!({
            "schema_version": 1,
            "layers": [
                {
                    "crate": "domain",
                    "path": "libs/domain",
                    "may_depend_on": [],
                    "tddd": {
                        "enabled": true,
                        "catalogue_file": "domain-types.json",
                        "schema_export": { "targets": ["domain"] }
                    }
                }
            ]
        });
        std::fs::write(
            workspace_root.join("architecture-rules.json"),
            serde_json::to_string_pretty(&arch_rules).unwrap(),
        )
        .unwrap();

        let result = execute_three_way_signals(
            items_dir,
            track_id.to_owned(),
            workspace_root,
            Some("domain".to_owned()),
            None,
        );
        assert!(result.is_err(), "missing catalogue file must return Err");
    }

    /// Verify that `write_output_file` writes the exact content passed to it and
    /// that the written file is byte-for-byte identical to the in-memory string
    /// (stdout/file parity invariant for the `--output` path).
    #[test]
    fn test_write_output_file_writes_content_and_matches_stdout_buffer() {
        let tmp = tempfile::tempdir().unwrap();
        let out_path = tmp.path().join("report.md");

        // Simulate the accumulated `file_buf` that would be printed to stdout.
        let stdout_content =
            "\n## Layer: `domain`\n\nAll items maintained (no non-skip signals).\n";

        // Call the output-file helper directly (equivalent to the `--output` code path).
        write_output_file(&out_path, stdout_content).expect("write_output_file must succeed");

        // Verify the file was created and contains exactly the same bytes as the in-memory buffer.
        let written = std::fs::read_to_string(&out_path).expect("output file must be readable");
        assert_eq!(
            written, stdout_content,
            "file content must be byte-for-byte identical to the stdout buffer"
        );
    }

    /// Verify the bare-filename normalization logic of `normalize_output_path`.
    ///
    /// A bare filename like `"report.md"` has `parent() == ""`, which would cause
    /// `atomic_write_file` to fail with `NotFound` when it opens the parent directory
    /// for fsync. `normalize_output_path` must prefix `"."` to produce `"./report.md"`.
    ///
    /// This test is purely in-memory (no CWD mutation) and therefore safe under
    /// parallel test execution.
    #[test]
    fn test_normalize_output_path_prefixes_dot_for_bare_filename() {
        // Bare filename — parent is "".
        let bare = std::path::Path::new("report.md");
        let normalized = normalize_output_path(bare);
        assert_eq!(
            normalized,
            std::path::PathBuf::from("./report.md"),
            "bare filename must be prefixed with '.'"
        );

        // Path with an explicit directory component must pass through unchanged.
        let with_dir = std::path::Path::new("subdir/report.md");
        let normalized_with_dir = normalize_output_path(with_dir);
        assert_eq!(
            normalized_with_dir,
            std::path::PathBuf::from("subdir/report.md"),
            "path with directory component must not be modified"
        );

        // Absolute path must pass through unchanged.
        let absolute = std::path::Path::new("/tmp/report.md");
        let normalized_abs = normalize_output_path(absolute);
        assert_eq!(
            normalized_abs,
            std::path::PathBuf::from("/tmp/report.md"),
            "absolute path must not be modified"
        );
    }

    #[test]
    fn test_execute_three_way_signals_missing_baseline_file_returns_error_with_hint() {
        // Track dir has a valid catalogue JSON but no baseline file.
        let tmp = tempfile::tempdir().unwrap();
        let items_dir = tmp.path().join("items");
        let track_id = "test-track-2026-01-02";
        let track_dir = items_dir.join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();

        // Write minimal metadata.json (branch set, no tasks) so the frozen-track guard passes.
        let metadata = serde_json::json!({
            "schema_version": 5,
            "id": track_id,
            "branch": format!("track/{track_id}"),
            "title": "Test",
            "created_at": "2026-01-02T00:00:00Z",
            "updated_at": "2026-01-02T00:00:00Z"
        });
        std::fs::write(
            track_dir.join("metadata.json"),
            serde_json::to_string_pretty(&metadata).unwrap(),
        )
        .unwrap();
        let impl_plan = r#"{"schema_version":1,"tasks":[],"plan":{"summary":[],"sections":[]}}"#;
        std::fs::write(track_dir.join("impl-plan.json"), impl_plan).unwrap();

        // Write a minimal valid CatalogueDocument JSON.
        let catalogue_json = serde_json::json!({
            "schema_version": 3,
            "crate_name": "domain",
            "layer": "domain",
            "types": {},
            "traits": {},
            "functions": {}
        });
        std::fs::write(
            track_dir.join("domain-types.json"),
            serde_json::to_string_pretty(&catalogue_json).unwrap(),
        )
        .unwrap();

        let workspace_root = tmp.path().to_path_buf();
        // architecture-rules.json uses "crate" key (not "id") per TdddLayerBinding schema.
        let arch_rules = serde_json::json!({
            "schema_version": 1,
            "layers": [
                {
                    "crate": "domain",
                    "path": "libs/domain",
                    "may_depend_on": [],
                    "tddd": {
                        "enabled": true,
                        "catalogue_file": "domain-types.json",
                        "schema_export": { "targets": ["domain"] }
                    }
                }
            ]
        });
        std::fs::write(
            workspace_root.join("architecture-rules.json"),
            serde_json::to_string_pretty(&arch_rules).unwrap(),
        )
        .unwrap();

        let result = execute_three_way_signals(
            items_dir,
            track_id.to_owned(),
            workspace_root,
            Some("domain".to_owned()),
            None,
        );
        assert!(result.is_err(), "missing baseline must return Err");
        let msg = format!("{}", result.unwrap_err());
        assert!(
            msg.contains("baseline-capture"),
            "error must hint at baseline-capture command: {msg}"
        );
    }
}
