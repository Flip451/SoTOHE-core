//! Core type-signal evaluation and write pipeline.
//!
//! Moved from the CLI layer to infrastructure so that the CLI composition root
//! never imports `domain::TypeCatalogueDocument`, `domain::TypeGraph`,
//! `domain::TypeBaseline`, `domain::ConsistencyReport`, or
//! `domain::ConfidenceSignal` directly (CN-01 / AC-03).

use std::collections::HashSet;
use std::path::Path;
use std::process::ExitCode;

use domain::schema::{SchemaExportError, SchemaExporter};

use crate::code_profile_builder::build_type_graph;
use crate::schema_export::RustdocSchemaExporter;
use crate::tddd::{baseline_codec, catalogue_codec, type_signals_codec};
use crate::timestamp_now;
use crate::track::atomic_write::atomic_write_file;
use crate::track::symlink_guard::reject_symlinks_below;
use crate::verify::tddd_layers::TdddLayerBinding;

/// Error type returned by [`evaluate_and_write_signals`] so the CLI can map it
/// to `CliError::Message` without importing domain types.
#[derive(Debug)]
pub struct EvaluateSignalsError(pub String);

impl std::fmt::Display for EvaluateSignalsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Evaluate type signals for a single TDDD layer binding and write back to the
/// configured catalogue file.
///
/// Moved from CLI so that the CLI layer never imports `domain::schema::SchemaExporter`,
/// `domain::schema::SchemaExportError`, `domain::TypeCatalogueDocument`, or
/// `domain::TypeGraph` directly (CN-01 / AC-03).
///
/// # Errors
///
/// Returns [`EvaluateSignalsError`] if the catalogue cannot be read/decoded,
/// schema export fails (e.g., nightly not installed), baseline cannot be read,
/// or the signal write fails.
pub fn execute_type_signals_for_layer(
    items_dir: &Path,
    track_id: &str,
    workspace_root: &Path,
    binding: &TdddLayerBinding,
) -> Result<ExitCode, EvaluateSignalsError> {
    // Security: guard root directories themselves before using them as trusted roots.
    // `reject_symlinks_below` only inspects descendants — a symlinked root would bypass it
    // (fail-closed per ADR §D7).
    for (path, label) in [(items_dir, "items_dir"), (workspace_root, "workspace_root")] {
        match path.symlink_metadata() {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(EvaluateSignalsError(format!(
                    "symlink guard: refusing to use symlinked {label}: {}",
                    path.display()
                )));
            }
            Ok(_) => {}
            Err(e) => {
                return Err(EvaluateSignalsError(format!(
                    "symlink guard: cannot stat {label} {}: {e}",
                    path.display()
                )));
            }
        }
    }

    // Containment: verify items_dir resolves under workspace_root.
    // This prevents directory-traversal (`..`) bypasses even when no symlinks are involved.
    let canonical_workspace = workspace_root.canonicalize().map_err(|e| {
        EvaluateSignalsError(format!(
            "cannot canonicalize workspace_root {}: {e}",
            workspace_root.display()
        ))
    })?;
    let canonical_items = items_dir.canonicalize().map_err(|e| {
        EvaluateSignalsError(format!("cannot canonicalize items_dir {}: {e}", items_dir.display()))
    })?;
    if !canonical_items.starts_with(&canonical_workspace) {
        return Err(EvaluateSignalsError(format!(
            "items_dir '{}' is outside workspace_root '{}'. Only paths under the workspace are allowed.",
            items_dir.display(),
            workspace_root.display()
        )));
    }

    let layer_id = binding.layer_id();

    // Security: validate track_id via domain::TrackId before joining onto items_dir.
    // `Path::join` resolves `..`, `/`, and multi-segment paths (`foo/bar`) at the OS
    // level. Using the domain type enforces the slug rules (single-segment, no `..`,
    // no path separators) and makes this function safe when called directly without
    // upstream CLI validation.
    let valid_track_id = domain::TrackId::try_new(track_id)
        .map_err(|e| EvaluateSignalsError(format!("invalid track_id: {e}")))?;

    let track_dir = items_dir.join(valid_track_id.as_ref());
    let catalogue_file = binding.catalogue_file();
    let catalogue_path = track_dir.join(catalogue_file);

    // Symlink guard on the read path: reject symlinks at the catalogue path or
    // any ancestor below `items_dir` before reading (fail-closed per ADR §D7).
    reject_symlinks_below(&catalogue_path, items_dir).map_err(|e| {
        EvaluateSignalsError(format!(
            "refusing to read catalogue {}: {e}",
            catalogue_path.display()
        ))
    })?;

    // Read and decode the configured catalogue file.
    let json = std::fs::read_to_string(&catalogue_path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            EvaluateSignalsError(format!(
                "{catalogue_file} not found for track '{track_id}'. \
                 Run /track:design first to create it (TDDD: type definitions must be written before implementation)."
            ))
        } else {
            EvaluateSignalsError(format!("cannot read {}: {e}", catalogue_path.display()))
        }
    })?;

    let mut doc = catalogue_codec::decode(&json)
        .map_err(|e| EvaluateSignalsError(format!("{catalogue_file} decode error: {e}")))?;

    // Resolve the target crate for schema export from the binding.
    let target_crate = match binding.targets() {
        [single] => single,
        [] => {
            return Err(EvaluateSignalsError(format!(
                "schema_export.targets is empty for layer '{layer_id}'; check architecture-rules.json"
            )));
        }
        multi => {
            return Err(EvaluateSignalsError(format!(
                "layer '{layer_id}' has {} schema_export.targets ({:?}), but multi-target export is not yet implemented. Use a single-target layer or wait for multi-target merge support.",
                multi.len(),
                multi
            )));
        }
    };

    // Export the target crate's public API via rustdoc JSON.
    let exporter = RustdocSchemaExporter::new(workspace_root.to_path_buf());
    let schema = exporter.export(target_crate).map_err(|e| {
        let hint = if matches!(e, SchemaExportError::NightlyNotFound) {
            " (install with: rustup toolchain install nightly)".to_owned()
        } else {
            String::new()
        };
        EvaluateSignalsError(format!("failed to export schema: {e}{hint}"))
    })?;

    // Collect typestate names for outgoing transition filtering in build_type_graph.
    let typestate_names = doc.typestate_names();

    // Build a pre-indexed TypeGraph from the flat schema export.
    let profile = build_type_graph(&schema, &typestate_names);

    // Load baseline for 4-group evaluation.
    let baseline_filename = binding.baseline_file();
    let baseline_path = track_dir.join(&baseline_filename);

    // Symlink guard on the baseline read path (fail-closed per ADR §D7).
    reject_symlinks_below(&baseline_path, items_dir).map_err(|e| {
        EvaluateSignalsError(format!("refusing to read baseline {}: {e}", baseline_path.display()))
    })?;

    let baseline = match std::fs::read_to_string(&baseline_path) {
        Ok(bl_json) => baseline_codec::decode(&bl_json)
            .map_err(|e| EvaluateSignalsError(format!("baseline decode error: {e}")))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(EvaluateSignalsError(format!(
                "{baseline_filename} not found for track '{track_id}'. \
                 Run `sotp track baseline-capture {track_id} --layer {layer_id}` first."
            )));
        }
        Err(e) => {
            return Err(EvaluateSignalsError(format!(
                "cannot read {}: {e}",
                baseline_path.display()
            )));
        }
    };

    // Load workspace crate names from architecture-rules.json for IN-10 reverse checks.
    let arch_rules_path = workspace_root.join("architecture-rules.json");
    let workspace_crates = match crate::verify::tddd_layers::load_workspace_crate_names(
        &arch_rules_path,
        workspace_root,
    ) {
        Ok(names) => names,
        Err(e) => {
            return Err(EvaluateSignalsError(format!(
                "cannot read architecture-rules.json for workspace_crates: {e}"
            )));
        }
    };

    // Delegate to the core evaluator.
    let signal_file_name = binding.signal_file();
    let (exit_code, summary) = evaluate_and_write_signals(
        &mut doc,
        &profile,
        &baseline,
        &catalogue_path,
        &track_dir,
        &signal_file_name,
        &workspace_crates,
    )?;

    // Print signal summary to stdout.
    print!("{}", summary.format());

    Ok(exit_code)
}

/// Core signal evaluation and catalogue write given pre-built domain components.
///
/// Separated from the CLI `execute_type_signals` so the abort-before-write
/// ordering and signal assembly can be exercised in unit tests without requiring
/// the nightly toolchain that `RustdocSchemaExporter` depends on.
///
/// # Errors
///
/// Returns [`EvaluateSignalsError`] if action diagnostics fail (delete errors),
/// encoding fails, either declaration-path / signal-path is a symlink
/// (fail-closed per ADR §D7), or any atomic write fails.
pub fn evaluate_and_write_signals(
    doc: &mut domain::TypeCatalogueDocument,
    profile: &domain::TypeGraph,
    baseline: &domain::TypeBaseline,
    domain_types_path: &Path,
    track_dir: &Path,
    signal_file_name: &str,
    workspace_crates: &HashSet<String>,
) -> Result<(ExitCode, SignalSummary), EvaluateSignalsError> {
    // Bidirectional consistency check: forward (spec → code) + reverse (code → spec).
    // workspace_crates enables IN-10 reverse checks for Interactor/SecondaryAdapter.
    let report = domain::check_consistency(doc.entries(), profile, baseline, workspace_crates);

    // Convert undeclared types/traits/functions (group 4) to Red TypeSignals.
    // Capture the count before appending group-3 baseline reds so the summary
    // WARN line only fires for truly new undeclared items (not baseline changes/deletions).
    let undeclared_type_trait_signals =
        domain::undeclared_to_signals(report.undeclared_types(), report.undeclared_traits());
    let undeclared_function_signals =
        domain::undeclared_functions_to_signals(report.undeclared_functions());
    let undeclared_count = undeclared_type_trait_signals.len() + undeclared_function_signals.len();
    let mut reverse_signals = undeclared_type_trait_signals;
    reverse_signals.extend(undeclared_function_signals);

    // Baseline Red: structural changes or deletions (group 3).
    // `found_type` is true when the name still exists in the code (structural change),
    // false when the name is absent from the type graph (deletion).
    for name in report.baseline_red_types() {
        let found_type = profile.get_type(name).is_some();
        reverse_signals.push(domain::TypeSignal::new(
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
        reverse_signals.push(domain::TypeSignal::new(
            name.clone(),
            "baseline_changed_trait",
            domain::ConfidenceSignal::Red,
            found_type,
            vec![],
            vec![],
            vec![],
        ));
    }
    for fq_name in report.baseline_red_functions() {
        // Parse fq_name back to (short_name, module_path) for graph lookup.
        // fq_name is "module_path::short_name" when module_path is Some, or just
        // "short_name" otherwise. Use rfind so nested module paths split correctly.
        let (short_name, module_path) = match fq_name.rfind("::") {
            Some(idx) => (&fq_name[idx + 2..], Some(&fq_name[..idx])),
            None => (fq_name.as_str(), None),
        };
        let found_type = profile.get_function(short_name, module_path).is_some();
        reverse_signals.push(domain::TypeSignal::new(
            fq_name.clone(),
            "baseline_changed_function",
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

    // Encode the catalogue to bytes before any writes. The encoded bytes are needed
    // to compute `declaration_hash` for the signal file (ADR §D5).
    let declaration_bytes = prepare_catalogue_bytes(&report, doc, domain_types_path, track_dir)?;

    // Write the signal file BEFORE the declaration file to ensure the declaration is
    // only updated when both writes can proceed. If the signal write fails, no files
    // are mutated. If the catalogue write fails after the signal write, the signal is
    // stale but recoverable on the next evaluation run (stale detection compares
    // `declaration_hash` against the on-disk declaration bytes).
    write_signal_file(&all_signals, &declaration_bytes, track_dir, signal_file_name)?;

    // Write the declaration file after the signal file is confirmed.
    write_catalogue_file(domain_types_path, track_dir, &declaration_bytes)?;

    let skipped = report.skipped_count();
    // Extract the catalogue filename from the path so the WARN message names the
    // correct file when running against a non-domain layer (e.g. usecase-types.json).
    let catalogue_file =
        domain_types_path.file_name().and_then(|n| n.to_str()).unwrap_or("types.json");

    let summary = build_signal_summary(&all_signals, undeclared_count, skipped, catalogue_file);

    Ok((ExitCode::SUCCESS, summary))
}

/// Signal summary returned by [`evaluate_and_write_signals`].
///
/// Contains string-typed counts so the CLI can print signal summaries without
/// importing `domain::ConfidenceSignal` directly.
#[derive(Debug)]
pub struct SignalSummary {
    pub blue: usize,
    pub yellow: usize,
    pub red: usize,
    pub total: usize,
    pub undeclared_count: usize,
    pub skipped_count: usize,
    pub catalogue_file: String,
}

impl SignalSummary {
    /// Formats the signal summary line(s) for stdout.
    #[must_use]
    pub fn format(&self) -> String {
        let SignalSummary {
            blue,
            yellow,
            red,
            total,
            undeclared_count,
            skipped_count,
            catalogue_file,
        } = self;
        let mut out = format!(
            "[OK] type-signals: blue={blue} yellow={yellow} red={red} (total={total}, undeclared={undeclared_count}, skipped={skipped_count})\n",
        );
        if *undeclared_count > 0 {
            out.push_str(&format!(
                "[WARN] {undeclared_count} undeclared type(s)/trait(s) found. Run /track:design to update {catalogue_file}.\n"
            ));
        }
        out
    }
}

fn build_signal_summary(
    signals: &[domain::TypeSignal],
    undeclared_count: usize,
    skipped_count: usize,
    catalogue_file: &str,
) -> SignalSummary {
    let blue = signals.iter().filter(|s| s.signal_as_str() == "blue").count();
    let yellow = signals.iter().filter(|s| s.signal_as_str() == "yellow").count();
    let red = signals.iter().filter(|s| s.signal_as_str() == "red").count();
    SignalSummary {
        blue,
        yellow,
        red,
        total: signals.len(),
        undeclared_count,
        skipped_count,
        catalogue_file: catalogue_file.to_owned(),
    }
}

/// Validate and encode the catalogue to bytes without writing to disk.
///
/// Calls [`validate_action_diagnostics`] first. If that returns `Err`
/// (delete-error abort), no bytes are produced. On success the catalogue JSON is
/// encoded to bytes and the declaration write-path symlink guard is verified.
///
/// Returns the exact byte vector that would be written to the declaration file
/// (including the trailing newline). Callers MUST pass these bytes to
/// [`write_catalogue_file`] (or to `write_signal_file` for hash computation) so
/// the `declaration_hash` in the signal file is pinned to the post-encode disk
/// bytes (ADR §D5).
///
/// # Errors
///
/// Returns [`EvaluateSignalsError`] if action diagnostics fail, `track_dir` is a
/// symlink, encoding fails, or the declaration file path is a symlink (ADR §D7
/// fail-closed write-path guard).
fn prepare_catalogue_bytes(
    report: &domain::ConsistencyReport,
    doc: &domain::TypeCatalogueDocument,
    domain_types_path: &Path,
    track_dir: &Path,
) -> Result<Vec<u8>, EvaluateSignalsError> {
    // Security: guard track_dir itself before using it as the trusted root for
    // `reject_symlinks_below`. A symlinked track_dir would otherwise pass through
    // the guard unchecked (fail-closed per ADR §D7).
    match track_dir.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            return Err(EvaluateSignalsError(format!(
                "symlink guard: refusing to use symlinked track_dir: {}",
                track_dir.display()
            )));
        }
        Ok(_) => {}
        Err(e) => {
            return Err(EvaluateSignalsError(format!(
                "symlink guard: cannot stat track_dir {}: {e}",
                track_dir.display()
            )));
        }
    }

    // Validate first: abort before any writes if delete errors are present.
    validate_action_diagnostics(report)?;

    // Encode and return bytes without writing.
    let encoded = catalogue_codec::encode(doc)
        .map_err(|e| EvaluateSignalsError(format!("catalogue encode error: {e}")))?;

    let declaration_bytes = format!("{encoded}\n").into_bytes();

    // ADR §D7 symlink guard on the write path: reject symlinks at the leaf or
    // at any ancestor below `track_dir`. `reject_symlinks_below` returns
    // `Ok(true)` when the target is a regular file, `Ok(false)` when the
    // target is absent, and `Err(_)` for any symlink or I/O error — both
    // `Ok` variants are safe to write to.
    reject_symlinks_below(domain_types_path, track_dir).map_err(|e| {
        EvaluateSignalsError(format!(
            "refusing to write declaration file {}: {e}",
            domain_types_path.display()
        ))
    })?;

    Ok(declaration_bytes)
}

/// Write the pre-encoded declaration bytes to disk atomically.
///
/// `declaration_bytes` MUST be the exact byte sequence returned by
/// [`prepare_catalogue_bytes`].
///
/// # Errors
///
/// Returns [`EvaluateSignalsError`] if the atomic write fails.
fn write_catalogue_file(
    domain_types_path: &Path,
    _track_dir: &Path,
    declaration_bytes: &[u8],
) -> Result<(), EvaluateSignalsError> {
    atomic_write_file(domain_types_path, declaration_bytes).map_err(|e| {
        EvaluateSignalsError(format!("cannot write {}: {e}", domain_types_path.display()))
    })?;

    // NOTE: Markdown view rendering is intentionally NOT performed here. The
    // `<layer>-types.md` view depends on both type-signals (this step) AND
    // catalogue-spec-signals (pre-commit step 3). Rendering from this step
    // would require a stale/fresh check against the catalogue-spec-signals
    // JSON that has not yet been refreshed, creating a deadlock in the
    // pre-commit chain. Instead, rendering is centralised in
    // `sync_rendered_views` (pre-commit step 3.5 and track-transition /
    // track-sync-views entrypoints), which runs once both signals are
    // persisted. See ADR `2026-04-23-0344-catalogue-spec-signal-activation.md`
    // §D2.5 (view content) / §D3.4 (pre-commit ordering).
    Ok(())
}

/// Validate action-baseline contradiction warnings and delete validation errors,
/// then encode and write back the catalogue.
///
/// Calls [`validate_action_diagnostics`] first.  If that returns `Err`
/// (delete-error abort), no files are written.  On success the catalogue JSON is
/// atomically written to disk.
///
/// Returns the exact byte vector written to the declaration file (including the
/// trailing newline). Callers computing `declaration_hash` for the signal file
/// MUST use this return value so the hash is pinned to post-encode disk bytes
/// (ADR §D5).
///
/// # Errors
///
/// Returns [`EvaluateSignalsError`] if action diagnostics fail, encoding fails,
/// the declaration file path is a symlink (ADR §D7 fail-closed write-path guard),
/// or any atomic write fails.
pub fn validate_and_write_catalogue(
    report: &domain::ConsistencyReport,
    doc: &domain::TypeCatalogueDocument,
    domain_types_path: &Path,
    track_dir: &Path,
) -> Result<Vec<u8>, EvaluateSignalsError> {
    let declaration_bytes = prepare_catalogue_bytes(report, doc, domain_types_path, track_dir)?;
    write_catalogue_file(domain_types_path, track_dir, &declaration_bytes)?;
    Ok(declaration_bytes)
}

/// Print action-baseline contradiction warnings and delete validation errors.
///
/// Contradictions are printed as `[WARN]` to stderr. Delete errors cause an
/// early return with [`EvaluateSignalsError`].
///
/// # Errors
///
/// Returns `EvaluateSignalsError` when delete errors are present.
pub fn validate_action_diagnostics(
    report: &domain::ConsistencyReport,
) -> Result<(), EvaluateSignalsError> {
    for contradiction in report.contradictions() {
        eprintln!(
            "[WARN] {} (action={}): {:?}",
            contradiction.name(),
            contradiction.action().action_tag(),
            contradiction.kind(),
        );
    }

    if !report.delete_errors().is_empty() {
        for name in report.delete_errors() {
            eprintln!(
                "[ERROR] action=delete for `{name}` but type not in baseline — \
                 cannot delete non-existent type"
            );
        }
        return Err(EvaluateSignalsError(
            "delete action validation failed: one or more entries reference non-existent \
             baseline types"
                .to_owned(),
        ));
    }

    Ok(())
}

/// Encode and write the per-layer evaluation-result file
/// (`<layer>-type-signals.json`, schema_version 1) alongside the declaration
/// file.
///
/// `declaration_bytes` MUST be the exact byte sequence just written to the
/// declaration file. The SHA-256 hash of these bytes is persisted as
/// `declaration_hash` so that stale detection can compare against the current
/// on-disk declaration file (ADR §D5).
///
/// # Errors
///
/// Returns [`EvaluateSignalsError`] when the system timestamp cannot be
/// derived, the signal file path is a symlink (ADR §D7 fail-closed write-path
/// guard), signal encoding fails, or the atomic write fails.
fn write_signal_file(
    signals: &[domain::TypeSignal],
    declaration_bytes: &[u8],
    track_dir: &Path,
    signal_file_name: &str,
) -> Result<(), EvaluateSignalsError> {
    let generated_at = timestamp_now()
        .map_err(|e| EvaluateSignalsError(format!("cannot derive generation timestamp: {e}")))?;
    let declaration_hash = type_signals_codec::declaration_hash(declaration_bytes);
    let signals_doc =
        domain::TypeSignalsDocument::new(generated_at, declaration_hash, signals.to_vec());

    let encoded = type_signals_codec::encode(&signals_doc)
        .map_err(|e| EvaluateSignalsError(format!("signal file encode error: {e}")))?;

    let signal_path = track_dir.join(signal_file_name);

    // ADR §D7 symlink guard on the write path: same policy as the declaration
    // file. `reject_symlinks_below` returns Ok(true | false) for regular files
    // and absent paths; any symlink or unexpected I/O error is Err.
    reject_symlinks_below(&signal_path, track_dir).map_err(|e| {
        EvaluateSignalsError(format!(
            "refusing to write signal file {}: {e}",
            signal_path.display()
        ))
    })?;

    atomic_write_file(&signal_path, format!("{encoded}\n").as_bytes()).map_err(|e| {
        EvaluateSignalsError(format!("cannot write {}: {e}", signal_path.display()))
    })?;

    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used)]
mod tests {
    use std::process::ExitCode;

    use super::*;

    fn make_signal(signal: domain::ConfidenceSignal) -> domain::TypeSignal {
        domain::TypeSignal::new("Foo", "value_object", signal, true, vec![], vec![], vec![])
    }

    fn make_entry_d(name: &str, action: domain::TypeAction) -> domain::TypeCatalogueEntry {
        domain::TypeCatalogueEntry::new(
            name,
            "desc",
            domain::TypeDefinitionKind::ValueObject {
                expected_members: Vec::new(),
                expected_methods: Vec::new(),
            },
            action,
            true,
        )
        .unwrap()
    }

    fn empty_graph_d() -> domain::TypeGraph {
        domain::TypeGraph::new(std::collections::HashMap::new(), std::collections::HashMap::new())
    }

    fn empty_baseline_d() -> domain::TypeBaseline {
        domain::TypeBaseline::new(
            1,
            domain::Timestamp::new("2026-01-01T00:00:00Z").unwrap(),
            std::collections::HashMap::new(),
            std::collections::HashMap::new(),
        )
    }

    // --- SignalSummary::format tests ---

    #[test]
    fn test_signal_summary_format_with_no_signals_prints_zero_counts() {
        let summary = build_signal_summary(&[], 0, 0, "domain-types.json");
        let out = summary.format();
        assert!(
            out.contains("blue=0 yellow=0 red=0 (total=0, undeclared=0, skipped=0)"),
            "unexpected summary: {out}"
        );
        assert!(!out.contains("[WARN]"), "no WARN expected when undeclared=0");
    }

    #[test]
    fn test_signal_summary_format_with_mixed_signals_counts_correctly() {
        let signals = vec![
            make_signal(domain::ConfidenceSignal::Blue),
            make_signal(domain::ConfidenceSignal::Blue),
            make_signal(domain::ConfidenceSignal::Yellow),
            make_signal(domain::ConfidenceSignal::Red),
        ];
        let summary = build_signal_summary(&signals, 0, 0, "domain-types.json");
        let out = summary.format();
        assert!(
            out.contains("blue=2 yellow=1 red=1 (total=4, undeclared=0, skipped=0)"),
            "unexpected summary: {out}"
        );
        assert!(!out.contains("[WARN]"), "no WARN expected when undeclared=0");
    }

    #[test]
    fn test_signal_summary_format_with_undeclared_shows_warn_and_track_design() {
        let signals = vec![
            make_signal(domain::ConfidenceSignal::Blue),
            make_signal(domain::ConfidenceSignal::Red),
            make_signal(domain::ConfidenceSignal::Red),
        ];
        let summary = build_signal_summary(&signals, 2, 0, "usecase-types.json");
        let out = summary.format();
        assert!(
            out.contains("blue=1 yellow=0 red=2 (total=3, undeclared=2, skipped=0)"),
            "unexpected summary: {out}"
        );
        assert!(out.contains("[WARN]"), "WARN line expected when undeclared>0");
        assert!(out.contains("/track:design"), "WARN must mention /track:design, got: {out}");
        assert!(
            out.contains("usecase-types.json"),
            "WARN must mention the specific catalogue file, got: {out}"
        );
    }

    #[test]
    fn test_signal_summary_format_blue_plus_yellow_plus_red_equals_total() {
        let signals = vec![
            make_signal(domain::ConfidenceSignal::Blue),
            make_signal(domain::ConfidenceSignal::Yellow),
            make_signal(domain::ConfidenceSignal::Yellow),
            make_signal(domain::ConfidenceSignal::Red),
        ];
        let summary = build_signal_summary(&signals, 1, 0, "domain-types.json");
        let out = summary.format();
        assert!(out.contains("blue=1 yellow=2 red=1 (total=4,"), "totals must sum: {out}");
    }

    // --- validate_action_diagnostics tests ---

    #[test]
    fn test_validate_action_diagnostics_with_clean_report_returns_ok() {
        let report = domain::check_consistency(
            &[],
            &empty_graph_d(),
            &empty_baseline_d(),
            &std::collections::HashSet::new(),
        );
        let result = validate_action_diagnostics(&report);
        assert!(result.is_ok(), "clean report must return Ok: {result:?}");
    }

    #[test]
    fn test_validate_action_diagnostics_with_delete_error_returns_error() {
        let entry = make_entry_d("Ghost", domain::TypeAction::Delete);
        let report = domain::check_consistency(
            &[entry],
            &empty_graph_d(),
            &empty_baseline_d(),
            &std::collections::HashSet::new(),
        );
        assert!(!report.delete_errors().is_empty(), "delete_errors must be non-empty");
        let result = validate_action_diagnostics(&report);
        assert!(result.is_err(), "delete error must return Err: {result:?}");
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("delete action validation failed"),
            "error must mention delete validation, got: {err_msg}"
        );
    }

    #[test]
    fn test_validate_action_diagnostics_does_not_write_files_on_failure() {
        let entry = make_entry_d("Phantom", domain::TypeAction::Delete);
        let report = domain::check_consistency(
            &[entry],
            &empty_graph_d(),
            &empty_baseline_d(),
            &std::collections::HashSet::new(),
        );
        let result = validate_action_diagnostics(&report);
        assert!(result.is_err(), "must return Err for delete_errors");
    }

    // --- validate_and_write_catalogue tests ---

    #[test]
    fn test_validate_and_write_catalogue_does_not_write_on_delete_error() {
        let dir = tempfile::tempdir().unwrap();
        let domain_types_path = dir.path().join("domain-types.json");
        let track_dir = dir.path().to_path_buf();

        let entry = make_entry_d("Ghost", domain::TypeAction::Delete);
        let doc = domain::TypeCatalogueDocument::new(1, vec![entry.clone()]);
        let report = domain::check_consistency(
            &[entry],
            &empty_graph_d(),
            &empty_baseline_d(),
            &std::collections::HashSet::new(),
        );

        assert!(
            !report.delete_errors().is_empty(),
            "precondition: delete_errors must be non-empty"
        );

        let result = validate_and_write_catalogue(&report, &doc, &domain_types_path, &track_dir);

        assert!(result.is_err(), "delete errors must cause validate_and_write_catalogue to fail");
        assert!(
            !domain_types_path.exists(),
            "domain-types.json must NOT be written on delete-error abort"
        );
        assert!(
            !dir.path().join("domain-types.md").exists(),
            "domain-types.md must NOT be written on delete-error abort (md render is centralised \
             in sync_rendered_views and is never invoked by this function)"
        );
    }

    #[test]
    fn test_validate_and_write_catalogue_writes_json_only_when_no_errors() {
        let dir = tempfile::tempdir().unwrap();
        let domain_types_path = dir.path().join("domain-types.json");
        let track_dir = dir.path().to_path_buf();

        let doc = domain::TypeCatalogueDocument::new(1, vec![]);
        let report = domain::check_consistency(
            &[],
            &empty_graph_d(),
            &empty_baseline_d(),
            &std::collections::HashSet::new(),
        );

        assert!(report.delete_errors().is_empty(), "precondition: no delete errors");

        let result = validate_and_write_catalogue(&report, &doc, &domain_types_path, &track_dir);

        assert!(result.is_ok(), "no-error report must succeed: {result:?}");
        assert!(domain_types_path.exists(), "domain-types.json must be written on success");
        assert!(
            !dir.path().join("domain-types.md").exists(),
            "domain-types.md must NOT be written by this function (render is owned by \
             sync_rendered_views)"
        );
    }

    // --- evaluate_and_write_signals tests ---

    #[test]
    fn test_evaluate_and_write_signals_with_delete_error_returns_err_and_leaves_files_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let domain_types_path = dir.path().join("domain-types.json");
        let track_dir = dir.path().to_path_buf();

        let entry = make_entry_d("Ghost", domain::TypeAction::Delete);
        let mut doc = domain::TypeCatalogueDocument::new(1, vec![entry.clone()]);

        let result = evaluate_and_write_signals(
            &mut doc,
            &empty_graph_d(),
            &empty_baseline_d(),
            &domain_types_path,
            &track_dir,
            "domain-type-signals.json",
            &std::collections::HashSet::new(),
        );

        assert!(result.is_err(), "delete error must cause evaluate_and_write_signals to fail");
        assert!(
            !domain_types_path.exists(),
            "domain-types.json must NOT be written on delete-error abort"
        );
        assert!(
            !dir.path().join("domain-types.md").exists(),
            "domain-types.md must NOT be written on delete-error abort"
        );
        assert!(
            !dir.path().join("domain-type-signals.json").exists(),
            "domain-type-signals.json must NOT be written on delete-error abort"
        );
    }

    #[test]
    fn test_evaluate_and_write_signals_with_clean_report_returns_success_and_writes_files() {
        let dir = tempfile::tempdir().unwrap();
        let domain_types_path = dir.path().join("domain-types.json");
        let track_dir = dir.path().to_path_buf();

        let mut doc = domain::TypeCatalogueDocument::new(1, vec![]);

        let result = evaluate_and_write_signals(
            &mut doc,
            &empty_graph_d(),
            &empty_baseline_d(),
            &domain_types_path,
            &track_dir,
            "domain-type-signals.json",
            &std::collections::HashSet::new(),
        );

        assert!(result.is_ok(), "clean report must succeed: {result:?}");
        let (code, _summary) = result.unwrap();
        assert_eq!(code, ExitCode::SUCCESS, "must return EXIT_SUCCESS");
        assert!(domain_types_path.exists(), "domain-types.json must be written on success");
        assert!(
            !dir.path().join("domain-types.md").exists(),
            "domain-types.md must NOT be written by evaluate_and_write_signals — \
             view rendering is centralised in sync_rendered_views"
        );
        let signal_path = dir.path().join("domain-type-signals.json");
        assert!(signal_path.exists(), "domain-type-signals.json must be written on success");

        let declaration_bytes = std::fs::read(&domain_types_path).unwrap();
        let expected_hash = crate::tddd::type_signals_codec::declaration_hash(&declaration_bytes);
        let signal_json = std::fs::read_to_string(&signal_path).unwrap();
        let signal_doc = crate::tddd::type_signals_codec::decode(&signal_json).unwrap();
        assert_eq!(
            signal_doc.declaration_hash(),
            expected_hash,
            "declaration_hash in signal file must match SHA-256 of on-disk declaration bytes"
        );
        assert_eq!(signal_doc.schema_version(), 1, "signal file schema_version must be 1");
        assert!(
            signal_doc.signals().is_empty(),
            "empty report must yield empty signals in the signal file"
        );
    }

    #[test]
    fn test_evaluate_and_write_signals_signal_file_includes_forward_signals() {
        use std::collections::HashMap;

        use domain::schema::{TypeKind, TypeNode};

        let dir = tempfile::tempdir().unwrap();
        let domain_types_path = dir.path().join("domain-types.json");
        let track_dir = dir.path().to_path_buf();

        let entry = make_entry_d("TrackId", domain::TypeAction::Add);
        let mut doc = domain::TypeCatalogueDocument::new(1, vec![entry]);

        let mut types = HashMap::new();
        types.insert(
            "TrackId".to_owned(),
            TypeNode::new(TypeKind::Struct, vec![], vec![], std::collections::HashSet::new()),
        );
        let profile = domain::TypeGraph::new(types, HashMap::new());

        let result = evaluate_and_write_signals(
            &mut doc,
            &profile,
            &empty_baseline_d(),
            &domain_types_path,
            &track_dir,
            "domain-type-signals.json",
            &std::collections::HashSet::new(),
        );

        assert!(result.is_ok(), "clean report must succeed: {result:?}");

        let signal_path = dir.path().join("domain-type-signals.json");
        let signal_json = std::fs::read_to_string(&signal_path).unwrap();
        let signal_doc = crate::tddd::type_signals_codec::decode(&signal_json).unwrap();
        let signals = signal_doc.signals();
        assert!(!signals.is_empty(), "signal file must carry forward signals, got empty");
        let blue_track_id =
            signals.iter().find(|s| s.type_name() == "TrackId" && s.signal_as_str() == "blue");
        assert!(blue_track_id.is_some(), "TrackId must appear as Blue in the signal file");
    }

    #[cfg(unix)]
    #[test]
    fn test_evaluate_and_write_signals_rejects_signal_file_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let domain_types_path = dir.path().join("domain-types.json");
        let signal_path = dir.path().join("domain-type-signals.json");
        let track_dir = dir.path().to_path_buf();

        std::os::unix::fs::symlink(dir.path().join("nowhere.json"), &signal_path).unwrap();

        let mut doc = domain::TypeCatalogueDocument::new(1, vec![]);

        let result = evaluate_and_write_signals(
            &mut doc,
            &empty_graph_d(),
            &empty_baseline_d(),
            &domain_types_path,
            &track_dir,
            "domain-type-signals.json",
            &std::collections::HashSet::new(),
        );

        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("refusing to write signal file"),
            "error must be from the signal-file symlink guard, got: {msg}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_evaluate_and_write_signals_rejects_declaration_file_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let domain_types_path = dir.path().join("domain-types.json");
        let track_dir = dir.path().to_path_buf();

        std::os::unix::fs::symlink(dir.path().join("nowhere.json"), &domain_types_path).unwrap();

        let mut doc = domain::TypeCatalogueDocument::new(1, vec![]);

        let result = evaluate_and_write_signals(
            &mut doc,
            &empty_graph_d(),
            &empty_baseline_d(),
            &domain_types_path,
            &track_dir,
            "domain-type-signals.json",
            &std::collections::HashSet::new(),
        );

        let err = result.unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("refusing to write declaration file"),
            "error must be from the declaration-file symlink guard, got: {msg}"
        );
        assert!(
            !dir.path().join("domain-type-signals.json").exists(),
            "signal file must NOT be written when declaration-file guard aborts"
        );
    }
}
