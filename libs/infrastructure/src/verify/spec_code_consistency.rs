//! Bidirectional spec ↔ code consistency check (spec-code-consistency verify subcommand).
//!
//! All domain type handling is internal to this module. The CLI layer calls
//! `execute_spec_code_consistency_str` passing string arguments and receives a
//! `VerifyOutcome` — no `domain::` imports needed in `apps/cli/src/`.
//!
//! The `evaluate_consistency_from_components` and `consistency_report_to_findings`
//! helpers are also re-exported so that the CLI test module can construct domain
//! objects via the re-exported types and call these helpers without importing
//! `domain` directly.

use std::path::{Path, PathBuf};

use domain::schema::{SchemaExportError, SchemaExporter};
use domain::verify::{VerifyFinding, VerifyOutcome};

use crate::code_profile_builder;
use crate::schema_export::RustdocSchemaExporter;
use crate::tddd::{baseline_codec, catalogue_codec};
use crate::track::symlink_guard::reject_symlinks_below;

// Re-export domain types needed by CLI test code so CLI tests import from `infrastructure`
// rather than `domain` (AC-03 compliance).
pub use domain::schema::TypeKind;
pub use domain::{
    ConfidenceSignal, ConsistencyReport, Timestamp, TypeAction, TypeBaseline, TypeBaselineEntry,
    TypeCatalogueDocument, TypeCatalogueEntry, TypeDefinitionKind, TypeGraph, check_consistency,
};

/// Execute bidirectional spec ↔ code consistency check from CLI arguments.
///
/// Returns a `VerifyOutcome` suitable for passing to `print_outcome`.
///
/// # Arguments
/// * `track_id_str` — track identifier (validated internally)
/// * `crate_name` — crate to export rustdoc schema from
/// * `project_root` — workspace root directory
///
/// # Errors
///
/// Returns a `VerifyOutcome` containing error findings on invalid track id,
/// missing files, decode errors, or schema export failures.
#[allow(clippy::too_many_lines)]
pub fn execute_spec_code_consistency_str(
    track_id_str: &str,
    crate_name: &str,
    project_root: &Path,
) -> VerifyOutcome {
    // Security: guard `project_root` itself before using it as the symlink-guard trusted root.
    // `reject_symlinks_below` only inspects descendants — a symlinked root would bypass it.
    match project_root.symlink_metadata() {
        Ok(meta) if meta.file_type().is_symlink() => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "symlink guard: refusing to use symlinked project_root: {}",
                project_root.display()
            ))]);
        }
        Ok(_) => {}
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "symlink guard: cannot stat project_root {}: {e}",
                project_root.display()
            ))]);
        }
    }

    // Canonicalize project_root to resolve `..` traversal bypasses before using it
    // as the trusted root for all downstream symlink guards.
    let project_root_canonical = match project_root.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "cannot canonicalize project_root {}: {e}",
                project_root.display()
            ))]);
        }
    };
    let project_root = project_root_canonical.as_path();

    // Validate track ID.
    let track_id = match domain::TrackId::try_new(track_id_str) {
        Ok(id) => id,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "invalid track ID: {e}"
            ))]);
        }
    };

    let track_dir: PathBuf = project_root.join("track/items").join(track_id.as_ref());
    let domain_types_path = track_dir.join("domain-types.json");

    // Symlink guard on the catalogue read path: reject symlinks at domain_types_path
    // or any ancestor below `project_root` before reading (fail-closed per ADR §D7).
    match reject_symlinks_below(&domain_types_path, project_root) {
        Ok(_) => {}
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "refusing to read {}: {e}",
                domain_types_path.display()
            ))]);
        }
    }

    // Read and decode domain-types.json.
    let json = match std::fs::read_to_string(&domain_types_path) {
        Ok(s) => s,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "cannot read {}: {e}",
                domain_types_path.display()
            ))]);
        }
    };

    let doc = match catalogue_codec::decode(&json) {
        Ok(d) => d,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "domain-types.json decode error: {e}"
            ))]);
        }
    };

    // Export schema via rustdoc JSON.
    let exporter = RustdocSchemaExporter::new(project_root.to_owned());
    let schema = match exporter.export(crate_name) {
        Ok(s) => s,
        Err(e) => {
            let hint = if matches!(e, SchemaExportError::NightlyNotFound) {
                " (install with: rustup toolchain install nightly)"
            } else {
                ""
            };
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "schema export failed: {e}{hint}"
            ))]);
        }
    };

    // Collect typestate names and build TypeGraph.
    let typestate_names = doc.typestate_names();
    let graph = code_profile_builder::build_type_graph(&schema, &typestate_names);

    // Load baseline for 4-group evaluation.
    let baseline_path = track_dir.join("domain-types-baseline.json");

    // Symlink guard on the baseline read path (fail-closed per ADR §D7).
    match reject_symlinks_below(&baseline_path, project_root) {
        Ok(_) => {}
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "refusing to read {}: {e}",
                baseline_path.display()
            ))]);
        }
    }

    let baseline = match std::fs::read_to_string(&baseline_path) {
        Ok(bl_json) => match baseline_codec::decode(&bl_json) {
            Ok(bl) => bl,
            Err(e) => {
                return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                    "baseline decode error: {e}"
                ))]);
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "domain-types-baseline.json not found — run `sotp track baseline-capture {track_id_str}`"
            ))]);
        }
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "cannot read {}: {e}",
                baseline_path.display()
            ))]);
        }
    };

    // Load workspace crate names from architecture-rules.json for IN-10 reverse checks.
    let arch_rules_path = project_root.join("architecture-rules.json");
    let workspace_crates = match crate::verify::tddd_layers::load_workspace_crate_names(
        &arch_rules_path,
        project_root,
    ) {
        Ok(names) => names,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "cannot read architecture-rules.json: {e}"
            ))]);
        }
    };

    // Run bidirectional consistency check with baseline-aware 4-group evaluation.
    evaluate_consistency_from_components(&doc, &graph, &baseline, &workspace_crates)
}

/// Core spec-code consistency evaluation given pre-built domain components.
///
/// Separated from `execute_spec_code_consistency_str` so the wiring from
/// `check_consistency` → `consistency_report_to_findings` → `VerifyOutcome` can be
/// exercised in unit tests without requiring the nightly toolchain.
///
/// # Arguments
/// * `doc` — decoded `TypeCatalogueDocument` (entries read from `domain-types.json`)
/// * `graph` — `TypeGraph` built from the schema export
/// * `baseline` — decoded `TypeBaseline` from `domain-types-baseline.json`
/// * `workspace_crates` — crate names from `architecture-rules.json` layers; enables
///   IN-10 workspace-origin reverse checks.  Pass an empty set to suppress them.
pub fn evaluate_consistency_from_components(
    doc: &TypeCatalogueDocument,
    graph: &TypeGraph,
    baseline: &TypeBaseline,
    workspace_crates: &std::collections::HashSet<String>,
) -> VerifyOutcome {
    let report = check_consistency(doc.entries(), graph, baseline, workspace_crates);
    let findings = consistency_report_to_findings(&report);
    print_consistency_report_json(&report);
    if findings.is_empty() { VerifyOutcome::pass() } else { VerifyOutcome::from_findings(findings) }
}

/// Convert a `ConsistencyReport` into a flat list of `VerifyFinding`s for `VerifyOutcome`.
///
/// See module-level docs for the duplicate-suppression logic for delete errors.
pub fn consistency_report_to_findings(report: &ConsistencyReport) -> Vec<VerifyFinding> {
    let mut findings = Vec::new();

    let delete_error_names: std::collections::HashSet<&str> =
        report.delete_errors().iter().map(String::as_str).collect();

    // Forward: red signals become errors (excluding synthetic delete-error patches).
    for sig in report.forward_signals() {
        if sig.signal() == ConfidenceSignal::Red {
            let is_delete_error_signal = delete_error_names.contains(sig.type_name())
                && !sig.found_type()
                && sig.missing_items().is_empty()
                && sig.extra_items().is_empty();
            if !is_delete_error_signal {
                findings.push(VerifyFinding::error(format!(
                    "{} ({}): Red — missing={:?}, extra={:?}",
                    sig.type_name(),
                    sig.kind_tag(),
                    sig.missing_items(),
                    sig.extra_items(),
                )));
            }
        }
    }

    // Group 4: undeclared types/traits (not in baseline, not declared) → Red.
    for name in report.undeclared_types() {
        findings.push(VerifyFinding::error(format!(
            "undeclared new type in code: `{name}` — add to domain-types.json"
        )));
    }
    for name in report.undeclared_traits() {
        findings.push(VerifyFinding::error(format!(
            "undeclared new trait in code: `{name}` — add to domain-types.json"
        )));
    }
    for name in report.undeclared_functions() {
        findings.push(VerifyFinding::error(format!(
            "undeclared new free function in code: `{name}` — add a FreeFunction entry to domain-types.json"
        )));
    }

    // Group 3: baseline structural changes or deletions → Red.
    for name in report.baseline_red_types() {
        findings.push(VerifyFinding::error(format!(
            "undeclared structural change to baseline type: `{name}` — add to domain-types.json"
        )));
    }
    for name in report.baseline_red_traits() {
        findings.push(VerifyFinding::error(format!(
            "undeclared structural change to baseline trait: `{name}` — add to domain-types.json"
        )));
    }
    for name in report.baseline_red_functions() {
        findings.push(VerifyFinding::error(format!(
            "undeclared structural change or deletion of baseline free function: `{name}` — add a FreeFunction entry to domain-types.json"
        )));
    }

    // Action-baseline contradictions → warnings (advisory, not CI-blocking).
    for contradiction in report.contradictions() {
        findings.push(VerifyFinding::warning(format!(
            "{} (action={}): {:?}",
            contradiction.name(),
            contradiction.action().action_tag(),
            contradiction.kind(),
        )));
    }

    // Delete baseline validation errors → hard errors (CI-blocking, specific diagnostic).
    for name in report.delete_errors() {
        findings.push(VerifyFinding::error(format!(
            "action=delete for `{name}` but type not in baseline — cannot delete non-existent type"
        )));
    }

    findings
}

/// Serialize a `ConsistencyReport` as a JSON line to stdout via serde_json.
fn print_consistency_report_json(report: &ConsistencyReport) {
    let signal_str = |s: ConfidenceSignal| match s {
        ConfidenceSignal::Blue => "blue",
        ConfidenceSignal::Yellow => "yellow",
        ConfidenceSignal::Red => "red",
        _ => "unknown",
    };
    let forward: Vec<serde_json::Value> = report
        .forward_signals()
        .iter()
        .map(|s| {
            serde_json::json!({
                "type_name": s.type_name(),
                "kind_tag": s.kind_tag(),
                "signal": signal_str(s.signal()),
                "found_type": s.found_type(),
                "found_items": s.found_items(),
                "missing_items": s.missing_items(),
                "extra_items": s.extra_items(),
            })
        })
        .collect();
    let contradictions: Vec<serde_json::Value> = report
        .contradictions()
        .iter()
        .map(|c| {
            serde_json::json!({
                "name": c.name(),
                "action": c.action().action_tag(),
                "kind": format!("{:?}", c.kind()),
            })
        })
        .collect();
    let output = serde_json::json!({
        "forward_signals": forward,
        "undeclared_types": report.undeclared_types(),
        "undeclared_traits": report.undeclared_traits(),
        "undeclared_functions": report.undeclared_functions(),
        "skipped_count": report.skipped_count(),
        "baseline_red_types": report.baseline_red_types(),
        "baseline_red_traits": report.baseline_red_traits(),
        "baseline_red_functions": report.baseline_red_functions(),
        "contradictions": contradictions,
        "delete_errors": report.delete_errors(),
    });
    println!("{output}");
}
