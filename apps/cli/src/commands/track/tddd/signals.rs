//! `sotp track domain-type-signals` — evaluate domain type signals via rustdoc schema export.
//!
//! Reads `domain-types.json` from the track directory, exports the domain crate's
//! public API via rustdoc JSON, evaluates signals for each declared type, and writes
//! the updated document back to `domain-types.json`.

use std::path::PathBuf;
use std::process::ExitCode;

use domain::schema::{SchemaExportError, SchemaExporter};
use infrastructure::code_profile_builder::build_type_graph;
use infrastructure::schema_export::RustdocSchemaExporter;
use infrastructure::tddd::catalogue_codec;
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
    // If not found, instruct the user to run /track:design first (TDDD requirement).
    let json = std::fs::read_to_string(&domain_types_path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            CliError::Message(format!(
                "domain-types.json not found for track '{track_id}'. \
                 Run /track:design first to create it (TDDD: type definitions must be written before implementation)."
            ))
        } else {
            CliError::Message(format!("cannot read {}: {e}", domain_types_path.display()))
        }
    })?;

    let mut doc = catalogue_codec::decode(&json)
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

    // Collect typestate names for outgoing transition filtering in build_type_graph.
    let typestate_names: std::collections::HashSet<String> = doc
        .entries()
        .iter()
        .filter(|e| matches!(e.kind(), domain::DomainTypeKind::Typestate { .. }))
        .map(|e| e.name().to_string())
        .collect();

    // Build a pre-indexed TypeGraph from the flat schema export.
    let profile = build_type_graph(&schema, &typestate_names);

    // Load baseline for 4-group evaluation.
    // Match directly on read_to_string so permissions errors and broken symlinks are
    // surfaced instead of being silently misreported as "file not found".
    let baseline_path = track_dir.join("domain-types-baseline.json");
    let baseline = match std::fs::read_to_string(&baseline_path) {
        Ok(bl_json) => infrastructure::tddd::baseline_codec::decode(&bl_json)
            .map_err(|e| CliError::Message(format!("baseline decode error: {e}")))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(CliError::Message(format!(
                "domain-types-baseline.json not found for track '{track_id}'. \
                 Run `sotp track baseline-capture {track_id}` first."
            )));
        }
        Err(e) => {
            return Err(CliError::Message(format!("cannot read {}: {e}", baseline_path.display())));
        }
    };

    // Bidirectional consistency check: forward (spec → code) + reverse (code → spec).
    let report = domain::check_consistency(doc.entries(), &profile, &baseline);

    // Convert undeclared types/traits (group 4) to Red DomainTypeSignals.
    // Capture the count before appending group-3 baseline reds so the summary
    // WARN line only fires for truly new undeclared items (not baseline changes/deletions).
    let undeclared_signals =
        domain::undeclared_to_signals(report.undeclared_types(), report.undeclared_traits());
    let undeclared_count = undeclared_signals.len();
    let mut reverse_signals = undeclared_signals;

    // Baseline Red: structural changes or deletions (group 3).
    // `found_type` is true when the name still exists in the code (structural change),
    // false when the name is absent from the type graph (deletion).
    for name in report.baseline_red_types() {
        let found_type = profile.get_type(name).is_some();
        reverse_signals.push(domain::DomainTypeSignal::new(
            name.clone(),
            "baseline_changed_type",
            domain::ConfidenceSignal::Red,
            found_type,
            vec![],
            vec![],
            vec![],
        ));
    }
    for name in report.baseline_red_traits() {
        let found_type = profile.get_trait(name).is_some();
        reverse_signals.push(domain::DomainTypeSignal::new(
            name.clone(),
            "baseline_changed_trait",
            domain::ConfidenceSignal::Red,
            found_type,
            vec![],
            vec![],
            vec![],
        ));
    }

    let mut all_signals: Vec<_> = report.forward_signals().to_vec();
    all_signals.extend(reverse_signals);

    doc.set_signals(all_signals.clone());

    // Encode and write back.
    let encoded = catalogue_codec::encode(&doc)
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

    let skipped = report.skipped_count();
    print_signal_summary(&all_signals, undeclared_count, skipped);

    Ok(ExitCode::SUCCESS)
}

/// Format the signal summary line with baseline-aware counts.
///
/// Returns a `String` containing the full output (newline-terminated lines) so the
/// formatting logic is testable without requiring the nightly toolchain that the full
/// `execute_domain_type_signals` path needs.
///
/// `total` equals `signals.len()` — it counts every emitted signal (forward + reverse),
/// not just the entries in `domain-types.json`, to keep `blue + yellow + red == total`.
fn format_signal_summary(
    signals: &[domain::DomainTypeSignal],
    undeclared_count: usize,
    skipped_count: usize,
) -> String {
    let blue = signals.iter().filter(|s| s.signal() == domain::ConfidenceSignal::Blue).count();
    let yellow = signals.iter().filter(|s| s.signal() == domain::ConfidenceSignal::Yellow).count();
    let red = signals.iter().filter(|s| s.signal() == domain::ConfidenceSignal::Red).count();
    let total = signals.len();
    let mut out = format!(
        "[OK] domain-type-signals: blue={blue} yellow={yellow} red={red} (total={total}, undeclared={undeclared_count}, skipped={skipped_count})\n",
    );

    if undeclared_count > 0 {
        out.push_str(&format!(
            "[WARN] {undeclared_count} undeclared type(s)/trait(s) found. Run /track:design to update domain-types.json.\n"
        ));
    }

    out
}

/// Print the signal summary produced by [`format_signal_summary`].
fn print_signal_summary(
    signals: &[domain::DomainTypeSignal],
    undeclared_count: usize,
    skipped_count: usize,
) {
    print!("{}", format_signal_summary(signals, undeclared_count, skipped_count));
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used)]
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
        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("/track:design"), "error must suggest /track:design, got: {msg}");
    }

    #[test]
    fn test_execute_domain_type_signals_with_malformed_domain_types_json_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let (items_dir, track_id) = setup_track(dir.path(), "{not valid json}");
        let workspace_root = dir.path().to_path_buf();

        let result = execute_domain_type_signals(items_dir, track_id, workspace_root);
        assert!(result.is_err(), "malformed domain-types.json must return error");
    }

    // --- format_signal_summary tests (pure, no nightly needed) ---

    fn make_signal(signal: domain::ConfidenceSignal) -> domain::DomainTypeSignal {
        domain::DomainTypeSignal::new("Foo", "value_object", signal, true, vec![], vec![], vec![])
    }

    #[test]
    fn test_format_signal_summary_with_no_signals_prints_zero_counts() {
        let out = format_signal_summary(&[], 0, 0);
        assert!(
            out.contains("blue=0 yellow=0 red=0 (total=0, undeclared=0, skipped=0)"),
            "unexpected summary: {out}"
        );
        assert!(!out.contains("[WARN]"), "no WARN expected when undeclared=0");
    }

    #[test]
    fn test_format_signal_summary_with_mixed_signals_counts_correctly() {
        let signals = vec![
            make_signal(domain::ConfidenceSignal::Blue),
            make_signal(domain::ConfidenceSignal::Blue),
            make_signal(domain::ConfidenceSignal::Yellow),
            make_signal(domain::ConfidenceSignal::Red),
        ];
        let out = format_signal_summary(&signals, 0, 0);
        assert!(
            out.contains("blue=2 yellow=1 red=1 (total=4, undeclared=0, skipped=0)"),
            "unexpected summary: {out}"
        );
        assert!(!out.contains("[WARN]"), "no WARN expected when undeclared=0");
    }

    #[test]
    fn test_format_signal_summary_with_undeclared_shows_warn_and_track_design() {
        let signals = vec![
            make_signal(domain::ConfidenceSignal::Blue),
            make_signal(domain::ConfidenceSignal::Red),
            make_signal(domain::ConfidenceSignal::Red),
        ];
        // 2 undeclared signals are represented in the red count above; undeclared_count is passed
        // separately to distinguish reverse-Red from forward-Red in the WARN line.
        let out = format_signal_summary(&signals, 2, 0);
        assert!(
            out.contains("blue=1 yellow=0 red=2 (total=3, undeclared=2, skipped=0)"),
            "unexpected summary: {out}"
        );
        assert!(out.contains("[WARN]"), "WARN line expected when undeclared>0");
        assert!(out.contains("/track:design"), "WARN must mention /track:design, got: {out}");
    }

    #[test]
    fn test_format_signal_summary_blue_plus_yellow_plus_red_equals_total() {
        let signals = vec![
            make_signal(domain::ConfidenceSignal::Blue),
            make_signal(domain::ConfidenceSignal::Yellow),
            make_signal(domain::ConfidenceSignal::Yellow),
            make_signal(domain::ConfidenceSignal::Red),
        ];
        let out = format_signal_summary(&signals, 1, 0);
        // invariant: blue + yellow + red == total
        assert!(out.contains("blue=1 yellow=2 red=1 (total=4,"), "totals must sum: {out}");
    }

    /// Success-path integration test.  Requires nightly toolchain for `cargo +nightly rustdoc`.
    /// Run with: `cargo test --package cli -- --ignored`
    #[test]
    #[ignore]
    fn test_execute_domain_type_signals_success_path_writes_signals() {
        let dir = tempfile::tempdir().unwrap();
        let domain_types_json = r#"{
  "schema_version": 1,
  "domain_types": [
    { "name": "TrackId", "kind": "value_object", "description": "Track identifier", "approved": true }
  ]
}"#;
        let (items_dir, track_id) = setup_track(dir.path(), domain_types_json);
        // Write an empty baseline so the baseline-required code path succeeds.
        let baseline_json = r#"{
  "schema_version": 1,
  "captured_at": "2026-01-01T00:00:00Z",
  "types": {},
  "traits": {}
}"#;
        std::fs::write(items_dir.join(&track_id).join("domain-types-baseline.json"), baseline_json)
            .unwrap();
        // workspace_root must point to the real workspace so rustdoc can find the domain crate.
        let workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|p| p.parent())
            .expect("workspace root")
            .to_path_buf();

        let result =
            execute_domain_type_signals(items_dir.clone(), track_id.clone(), workspace_root);
        assert!(result.is_ok(), "success path must return Ok: {result:?}");

        // Verify signals were written back
        let updated =
            std::fs::read_to_string(items_dir.join(&track_id).join("domain-types.json")).unwrap();
        assert!(updated.contains("\"signals\""), "signals must be written to domain-types.json");

        // Verify domain-types.md was generated
        let md_path = items_dir.join(&track_id).join("domain-types.md");
        assert!(md_path.exists(), "domain-types.md must be generated");
    }
}
