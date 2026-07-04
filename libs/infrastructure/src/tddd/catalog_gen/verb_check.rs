//! `sotp catalog check` (D7 / IN-06 / AC-11): re-validate catalogue completion
//! against a gate context, resolving strictness per phase2 / commit / merge.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use domain::plan_ref::SpecElementId;
use domain::tddd::catalog_gen::DraftHole;
use serde_json::Value;
use usecase::catalog_gen::{
    CatalogCheckQuery, CatalogCheckReport, CatalogCheckVerdict, CatalogError, CatalogGateContext,
};

use crate::tddd::catalogue_document_codec::derive_filename_stem;

use super::fs_access::{catalogue_path, load_bindings, read_catalogue, track_dir};
use super::validate::load_spec_anchors_for_check;
use super::{scan_todo_holes, try_complete, validate_hole_free_schema};

/// Per-layer check outcome, aggregated across all checked layers.
struct LayerOutcome {
    verdict: CatalogCheckVerdict,
    findings: Vec<String>,
    holes: Vec<DraftHole>,
}

/// Check catalogue completion for the resolved gate.
///
/// # Errors
///
/// Returns [`CatalogError::Port`] on a filesystem read failure.
pub(super) fn run(
    track_id: &str,
    items_dir: &Path,
    query: CatalogCheckQuery,
) -> Result<CatalogCheckReport, CatalogError> {
    let bindings = load_bindings(items_dir)?;
    let dir = track_dir(items_dir, track_id)?;
    // Anchor validation is best-effort: a missing spec (Phase 0) yields an empty
    // set rather than failing the read-only check. Malformed/unreadable specs
    // still fail closed.
    let spec_anchors = load_spec_anchors_for_check(&dir, items_dir)?;

    let targets: Vec<(String, PathBuf)> = match &query.layer {
        Some(layer) => vec![(layer.as_ref().to_owned(), catalogue_path(&dir, &bindings, layer)?)],
        None => bindings
            .iter()
            .map(|binding| (binding.layer_id().to_owned(), dir.join(binding.catalogue_file())))
            .collect(),
    };

    let mut outcomes = Vec::new();
    for (layer_name, path) in &targets {
        outcomes.push(check_layer(query.gate, layer_name, path, items_dir, &spec_anchors)?);
    }
    Ok(aggregate(outcomes))
}

/// Check a single layer's catalogue file against the gate.
fn check_layer(
    gate: CatalogGateContext,
    layer_name: &str,
    path: &Path,
    trusted_root: &Path,
    spec_anchors: &BTreeSet<SpecElementId>,
) -> Result<LayerOutcome, CatalogError> {
    let value: Value = match read_catalogue(path, trusted_root) {
        Ok(value) => value,
        Err(CatalogError::FileMissing { .. }) => {
            return Ok(missing_outcome(gate, layer_name, path));
        }
        Err(err @ CatalogError::SchemaInvalid { .. }) => {
            return Ok(blocked(format!("{layer_name}: schema error: {err}")));
        }
        Err(err) => return Err(err),
    };

    let holes = scan_todo_holes(&value);
    let expected_stem = derive_filename_stem(path);
    if !holes.is_empty() {
        // A residual `$todo` hole is tolerated as an interim result at the commit
        // gate, but it must not mask a schema violation in a *hole-free* entry
        // (invalid role, unknown field, crate-name mismatch) — those would
        // otherwise slip through as interim / exit 0. Validate the hole-free
        // portion first and block on a real schema error; keep the hole interim
        // when only holes remain.
        if let Err(err) = validate_hole_free_schema(&value, &expected_stem) {
            return Ok(blocked(format!("{layer_name}: schema error: {err}")));
        }
        return Ok(hole_outcome(gate, layer_name, holes));
    }

    let mut anchor_strings = Vec::new();
    collect_anchor_strings(&value, &mut anchor_strings);
    if let Err(err) = try_complete(value, &expected_stem) {
        return Ok(blocked(format!("{layer_name}: schema error: {err}")));
    }

    let dangling = dangling_anchors(&anchor_strings, spec_anchors);
    if dangling.is_empty() {
        return Ok(LayerOutcome {
            verdict: CatalogCheckVerdict::Pass,
            findings: vec![],
            holes: vec![],
        });
    }
    Ok(dangling_outcome(gate, layer_name, &dangling))
}

/// Outcome for an absent catalogue file.
fn missing_outcome(gate: CatalogGateContext, layer_name: &str, path: &Path) -> LayerOutcome {
    match gate {
        CatalogGateContext::Commit => LayerOutcome {
            verdict: CatalogCheckVerdict::Skipped,
            findings: vec![format!("{layer_name}: catalogue file absent — skipped")],
            holes: vec![],
        },
        _ => LayerOutcome {
            verdict: CatalogCheckVerdict::Blocked,
            findings: vec![format!(
                "{layer_name}: TDDD catalogue file missing: {}",
                path.display()
            )],
            holes: vec![],
        },
    }
}

/// Outcome for a catalogue with residual `$todo` holes.
fn hole_outcome(gate: CatalogGateContext, layer_name: &str, holes: Vec<DraftHole>) -> LayerOutcome {
    let count = holes.len();
    match gate {
        CatalogGateContext::Commit => LayerOutcome {
            verdict: CatalogCheckVerdict::Interim,
            findings: vec![format!("{layer_name}: {count} unfilled $todo hole(s) (interim)")],
            holes,
        },
        _ => LayerOutcome {
            verdict: CatalogCheckVerdict::Blocked,
            findings: vec![format!("{layer_name}: {count} unfilled $todo hole(s)")],
            holes,
        },
    }
}

/// Outcome for a catalogue with dangling spec anchors.
fn dangling_outcome(
    gate: CatalogGateContext,
    layer_name: &str,
    dangling: &[String],
) -> LayerOutcome {
    match gate {
        CatalogGateContext::Commit => LayerOutcome {
            verdict: CatalogCheckVerdict::Pass,
            findings: vec![format!(
                "{layer_name}: {} dangling anchor(s) tolerated at commit",
                dangling.len()
            )],
            holes: vec![],
        },
        _ => LayerOutcome {
            verdict: CatalogCheckVerdict::Blocked,
            findings: dangling
                .iter()
                .map(|anchor| format!("{layer_name}: dangling spec anchor `{anchor}`"))
                .collect(),
            holes: vec![],
        },
    }
}

/// A blocked outcome with a single finding.
fn blocked(finding: String) -> LayerOutcome {
    LayerOutcome { verdict: CatalogCheckVerdict::Blocked, findings: vec![finding], holes: vec![] }
}

/// Aggregate per-layer outcomes; precedence Blocked > Interim > Pass > Skipped.
fn aggregate(outcomes: Vec<LayerOutcome>) -> CatalogCheckReport {
    let mut findings = Vec::new();
    let mut remaining_holes = Vec::new();
    let mut any_blocked = false;
    let mut any_interim = false;
    let mut any_pass = false;
    for outcome in outcomes {
        findings.extend(outcome.findings);
        remaining_holes.extend(outcome.holes);
        match outcome.verdict {
            CatalogCheckVerdict::Blocked => any_blocked = true,
            CatalogCheckVerdict::Interim => any_interim = true,
            CatalogCheckVerdict::Pass => any_pass = true,
            CatalogCheckVerdict::Skipped => {}
        }
    }
    let verdict = if any_blocked {
        CatalogCheckVerdict::Blocked
    } else if any_interim {
        CatalogCheckVerdict::Interim
    } else if any_pass {
        CatalogCheckVerdict::Pass
    } else {
        CatalogCheckVerdict::Skipped
    };
    CatalogCheckReport { verdict, findings, remaining_holes }
}

/// Collect every `anchor` string value in the JSON tree (spec_refs anchors).
fn collect_anchor_strings(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                if key == "anchor" {
                    if let Some(anchor) = child.as_str() {
                        out.push(anchor.to_owned());
                    }
                } else {
                    collect_anchor_strings(child, out);
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_anchor_strings(item, out);
            }
        }
        _ => {}
    }
}

/// Anchors that are malformed or absent from the spec set.
fn dangling_anchors(anchors: &[String], spec_anchors: &BTreeSet<SpecElementId>) -> Vec<String> {
    anchors
        .iter()
        .filter(|anchor| match SpecElementId::try_new(anchor.as_str()) {
            Ok(element) => !spec_anchors.contains(&element),
            Err(_) => true,
        })
        .cloned()
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;

    fn write_file(name: &str, body: &str) -> (tempfile::TempDir, PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join(name);
        std::fs::write(&path, body).unwrap();
        (temp, path)
    }

    fn empty_set() -> BTreeSet<SpecElementId> {
        BTreeSet::new()
    }

    #[test]
    fn test_missing_file_skipped_at_commit_blocked_at_phase2() {
        let missing = Path::new("/nonexistent/domain-types.json");
        let commit = check_layer(
            CatalogGateContext::Commit,
            "domain",
            missing,
            Path::new("/"),
            &empty_set(),
        )
        .unwrap();
        assert_eq!(commit.verdict, CatalogCheckVerdict::Skipped);
        let phase2 = check_layer(
            CatalogGateContext::Phase2,
            "domain",
            missing,
            Path::new("/"),
            &empty_set(),
        )
        .unwrap();
        assert_eq!(phase2.verdict, CatalogCheckVerdict::Blocked);
    }

    #[test]
    fn test_holes_interim_at_commit_blocked_at_merge() {
        let body = r#"{"schema_version":5,"crate_name":"domain","layer":"domain","types":{"Foo":{"role":{"$todo":"pick"}}},"traits":{},"functions":{}}"#;
        let (temp, path) = write_file("domain-types.json", body);
        let commit =
            check_layer(CatalogGateContext::Commit, "domain", &path, temp.path(), &empty_set())
                .unwrap();
        assert_eq!(commit.verdict, CatalogCheckVerdict::Interim);
        assert!(!commit.holes.is_empty());
        let merge =
            check_layer(CatalogGateContext::Merge, "domain", &path, temp.path(), &empty_set())
                .unwrap();
        assert_eq!(merge.verdict, CatalogCheckVerdict::Blocked);
    }

    #[test]
    fn test_clean_empty_catalogue_passes() {
        let body = r#"{"schema_version":5,"crate_name":"domain","layer":"domain","types":{},"traits":{},"functions":{}}"#;
        let (temp, path) = write_file("domain-types.json", body);
        let outcome =
            check_layer(CatalogGateContext::Phase2, "domain", &path, temp.path(), &empty_set())
                .unwrap();
        assert_eq!(outcome.verdict, CatalogCheckVerdict::Pass);
    }

    #[test]
    fn test_invalid_schema_blocks_all_gates() {
        let body = r#"{"schema_version":99,"crate_name":"domain","layer":"domain","types":{},"traits":{},"functions":{}}"#;
        let (temp, path) = write_file("domain-types.json", body);
        let outcome =
            check_layer(CatalogGateContext::Commit, "domain", &path, temp.path(), &empty_set())
                .unwrap();
        assert_eq!(outcome.verdict, CatalogCheckVerdict::Blocked);
    }

    #[test]
    fn test_invalid_schema_with_holes_blocks_before_interim() {
        let body = r#"{"schema_version":99,"crate_name":"domain","layer":"domain","types":{"Foo":{"role":{"$todo":"pick"}}},"traits":{},"functions":{}}"#;
        let (temp, path) = write_file("domain-types.json", body);
        let outcome =
            check_layer(CatalogGateContext::Commit, "domain", &path, temp.path(), &empty_set())
                .unwrap();
        assert_eq!(outcome.verdict, CatalogCheckVerdict::Blocked);
        assert!(outcome.holes.is_empty());
    }

    #[test]
    fn test_schema_error_in_hole_free_entry_blocks_despite_unrelated_hole_at_commit() {
        // `Foo` carries a legitimate `$todo` hole (still being annotated). `Bar` is
        // hole-free but has an unknown field that `read_catalogue`'s schema_version
        // probe does not catch — only a full decode does. At the commit gate the
        // unrelated hole must not mask `Bar`'s schema violation as an interim result.
        let body = r#"{"schema_version":5,"crate_name":"domain","layer":"domain","types":{"Foo":{"role":{"$todo":"pick"}},"Bar":{"action":"add","role":{"ValueObject":{}},"kind":{"kind":"struct","shape":{"kind":"unit"}},"bogus_field":true}},"traits":{},"functions":{}}"#;
        let (temp, path) = write_file("domain-types.json", body);
        let outcome =
            check_layer(CatalogGateContext::Commit, "domain", &path, temp.path(), &empty_set())
                .unwrap();
        assert_eq!(outcome.verdict, CatalogCheckVerdict::Blocked);
    }

    #[test]
    fn test_crate_name_mismatch_with_hole_blocks_at_commit() {
        // A `crate_name` that disagrees with the filename stem is a decode-time
        // schema error the schema_version probe never catches. It must be blocked
        // at the commit gate even when an unrelated `$todo` hole is present.
        let body = r#"{"schema_version":5,"crate_name":"wrong","layer":"domain","types":{"Foo":{"role":{"$todo":"pick"}}},"traits":{},"functions":{}}"#;
        let (temp, path) = write_file("domain-types.json", body);
        let outcome =
            check_layer(CatalogGateContext::Commit, "domain", &path, temp.path(), &empty_set())
                .unwrap();
        assert_eq!(outcome.verdict, CatalogCheckVerdict::Blocked);
    }

    #[test]
    fn test_hole_free_valid_entry_alongside_hole_stays_interim_at_commit() {
        // `Foo` carries a hole; `Bar` is hole-free and schema-valid. The hole-free
        // schema validation must not block a legitimately-interim catalogue: the
        // outcome stays interim (and still reports the residual hole).
        let body = r#"{"schema_version":5,"crate_name":"domain","layer":"domain","types":{"Foo":{"role":{"$todo":"pick"}},"Bar":{"action":"add","role":{"ValueObject":{}},"kind":{"kind":"struct","shape":{"kind":"unit"}}}},"traits":{},"functions":{}}"#;
        let (temp, path) = write_file("domain-types.json", body);
        let outcome =
            check_layer(CatalogGateContext::Commit, "domain", &path, temp.path(), &empty_set())
                .unwrap();
        assert_eq!(outcome.verdict, CatalogCheckVerdict::Interim);
        assert!(!outcome.holes.is_empty());
    }

    #[test]
    fn test_top_level_hole_does_not_mask_hole_free_schema_error_at_commit() {
        // A top-level scalar hole used to survive pruning and make `try_complete`
        // return `Incomplete` before the strict codec could decode `Bar`. The
        // hole-free `Bar` schema error must still block instead of becoming Interim.
        let body = r#"{"schema_version":5,"crate_name":"domain","layer":{"$todo":"pick"},"types":{"Bar":{"action":"add","role":{"ValueObject":{}},"kind":{"kind":"struct","shape":{"kind":"unit"}},"bogus_field":true}},"traits":{},"functions":{}}"#;
        let (temp, path) = write_file("domain-types.json", body);
        let outcome =
            check_layer(CatalogGateContext::Commit, "domain", &path, temp.path(), &empty_set())
                .unwrap();
        assert_eq!(outcome.verdict, CatalogCheckVerdict::Blocked);
    }

    #[test]
    fn test_top_level_hole_with_valid_remainder_stays_interim_at_commit() {
        // Top-level scalar holes are still tolerated by the commit gate when the
        // hole-free remainder decodes cleanly.
        let body = r#"{"schema_version":5,"crate_name":"domain","layer":{"$todo":"pick"},"types":{},"traits":{},"functions":{}}"#;
        let (temp, path) = write_file("domain-types.json", body);
        let outcome =
            check_layer(CatalogGateContext::Commit, "domain", &path, temp.path(), &empty_set())
                .unwrap();
        assert_eq!(outcome.verdict, CatalogCheckVerdict::Interim);
        assert!(!outcome.holes.is_empty());
    }

    #[test]
    fn test_root_hole_does_not_mask_hole_free_schema_error_at_commit() {
        // A root-level marker is valid draft syntax for "the document is still
        // incomplete", but when other fields are present the hole-free remainder
        // must still be decoded so schema errors cannot become Interim.
        let body = r#"{"$todo":"finish draft","schema_version":5,"crate_name":"domain","layer":"domain","types":{"Bar":{"action":"add","role":{"ValueObject":{}},"kind":{"kind":"struct","shape":{"kind":"unit"}},"bogus_field":true}},"traits":{},"functions":{}}"#;
        let (temp, path) = write_file("domain-types.json", body);
        let outcome =
            check_layer(CatalogGateContext::Commit, "domain", &path, temp.path(), &empty_set())
                .unwrap();
        assert_eq!(outcome.verdict, CatalogCheckVerdict::Blocked);
    }

    #[test]
    fn test_root_hole_with_valid_remainder_stays_interim_at_commit() {
        let body = r#"{"$todo":"finish draft","schema_version":5,"crate_name":"domain","layer":"domain","types":{},"traits":{},"functions":{}}"#;
        let (temp, path) = write_file("domain-types.json", body);
        let outcome =
            check_layer(CatalogGateContext::Commit, "domain", &path, temp.path(), &empty_set())
                .unwrap();
        assert_eq!(outcome.verdict, CatalogCheckVerdict::Interim);
        assert!(!outcome.holes.is_empty());
    }

    #[test]
    fn test_mismatched_crate_name_blocks_via_filename_stem() {
        // Hole-free, otherwise-valid catalogue whose `crate_name` disagrees with
        // the filename stem (`domain-types.json` → `domain`). The check must
        // derive the expected crate from the path, not trust the JSON field, so
        // this tampered file is blocked instead of passing.
        let body = r#"{"schema_version":5,"crate_name":"wrong","layer":"domain","types":{},"traits":{},"functions":{}}"#;
        let (temp, path) = write_file("domain-types.json", body);
        let outcome =
            check_layer(CatalogGateContext::Phase2, "domain", &path, temp.path(), &empty_set())
                .unwrap();
        assert_eq!(outcome.verdict, CatalogCheckVerdict::Blocked);
    }

    #[test]
    fn test_duplicate_entry_key_blocks_check() {
        // Two `Foo` entries under `types`: parsing through `serde_json::Value`
        // would collapse them to last-wins and bypass the codec's StrictMap
        // duplicate rejection. The check must fail closed on the raw duplicate.
        let body = r#"{"schema_version":5,"crate_name":"domain","layer":"domain","types":{"Foo":{"role":{"ValueObject":{}}},"Foo":{"role":{"ValueObject":{}}}},"traits":{},"functions":{}}"#;
        let (temp, path) = write_file("domain-types.json", body);
        let outcome =
            check_layer(CatalogGateContext::Phase2, "domain", &path, temp.path(), &empty_set())
                .unwrap();
        assert_eq!(outcome.verdict, CatalogCheckVerdict::Blocked);
    }

    #[test]
    fn test_dangling_anchor_blocks_phase2_passes_commit() {
        let body = r#"{"schema_version":5,"crate_name":"domain","layer":"domain","types":{"Foo":{"role":{"ValueObject":{}},"kind":{"kind":"struct","shape":{"kind":"unit"}},"spec_refs":[{"file":"spec.json","anchor":"ZZ-99"}]}},"traits":{},"functions":{}}"#;
        let (temp, path) = write_file("domain-types.json", body);
        let phase2 =
            check_layer(CatalogGateContext::Phase2, "domain", &path, temp.path(), &empty_set())
                .unwrap();
        assert_eq!(phase2.verdict, CatalogCheckVerdict::Blocked);
        let commit =
            check_layer(CatalogGateContext::Commit, "domain", &path, temp.path(), &empty_set())
                .unwrap();
        assert_eq!(commit.verdict, CatalogCheckVerdict::Pass);
    }

    #[test]
    fn test_aggregate_precedence() {
        let outcomes = vec![
            LayerOutcome { verdict: CatalogCheckVerdict::Pass, findings: vec![], holes: vec![] },
            LayerOutcome { verdict: CatalogCheckVerdict::Interim, findings: vec![], holes: vec![] },
        ];
        assert_eq!(aggregate(outcomes).verdict, CatalogCheckVerdict::Interim);

        let outcomes = vec![
            LayerOutcome { verdict: CatalogCheckVerdict::Skipped, findings: vec![], holes: vec![] },
            LayerOutcome { verdict: CatalogCheckVerdict::Blocked, findings: vec![], holes: vec![] },
        ];
        assert_eq!(aggregate(outcomes).verdict, CatalogCheckVerdict::Blocked);
    }
}
