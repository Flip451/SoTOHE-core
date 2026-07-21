//! `sotp catalog check` (D7 / IN-06 / AC-11): re-validate catalogue completion.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use domain::plan_ref::SpecElementId;
use domain::tddd::catalog_gen::DraftHole;
use domain::{ConfidenceSignal, evaluate_catalogue_entry_signal};
use serde_json::Value;
use usecase::catalog_gen::{
    CatalogCheckQuery, CatalogCheckReport, CatalogCheckVerdict, CatalogError,
};
use usecase::catalogue_traversal::iter_catalogue_entries;

use crate::tddd::catalogue_document_codec::derive_filename_stem;

use super::fs_access::{
    catalogue_path, catalogue_present, load_bindings, read_catalogue, track_dir,
};
use super::validate::load_spec_anchors_for_check;
use super::{scan_todo_holes, try_complete, validate_hole_free_schema};

/// Per-layer check outcome, aggregated across all checked layers.
struct LayerOutcome {
    verdict: CatalogCheckVerdict,
    findings: Vec<String>,
    holes: Vec<DraftHole>,
}

/// Check catalogue completion.
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

    let all_catalogue_files: Vec<(String, PathBuf)> = bindings
        .iter()
        .map(|binding| (binding.layer_id().to_owned(), dir.join(binding.catalogue_file())))
        .collect();
    let any_catalogue_exists = has_any_catalogue_file(&all_catalogue_files, items_dir)?;

    let targets: Vec<(String, PathBuf)> = match &query.layer {
        Some(layer) => vec![(layer.as_ref().to_owned(), catalogue_path(&dir, &bindings, layer)?)],
        None => all_catalogue_files,
    };

    let mut outcomes = Vec::new();
    for (layer_name, path) in &targets {
        outcomes.push(check_layer(
            layer_name,
            path,
            items_dir,
            &spec_anchors,
            any_catalogue_exists,
        )?);
    }
    Ok(aggregate(outcomes))
}

/// Return whether any expected catalogue file is present, using the same guarded
/// real-file boundary as catalogue reads.
fn has_any_catalogue_file(
    catalogue_files: &[(String, PathBuf)],
    trusted_root: &Path,
) -> Result<bool, CatalogError> {
    let mut any_present = false;
    for (_, path) in catalogue_files {
        if catalogue_present(path, trusted_root)? {
            any_present = true;
        }
    }
    Ok(any_present)
}

/// Check a single layer's catalogue file.
fn check_layer(
    layer_name: &str,
    path: &Path,
    trusted_root: &Path,
    spec_anchors: &BTreeSet<SpecElementId>,
    any_catalogue_exists: bool,
) -> Result<LayerOutcome, CatalogError> {
    let value: Value = match read_catalogue(path, trusted_root) {
        Ok(value) => value,
        Err(CatalogError::FileMissing { .. }) => {
            return Ok(missing_outcome(layer_name, path, any_catalogue_exists));
        }
        Err(err @ CatalogError::SchemaInvalid { .. }) => {
            return Ok(blocked(format!("{layer_name}: schema error: {err}")));
        }
        Err(err) => return Err(err),
    };

    let holes = scan_todo_holes(&value);
    let expected_stem = derive_filename_stem(path);
    if !holes.is_empty() {
        // Residual `$todo` holes block every gate once a catalogue file exists.
        // Validate the hole-free portion first so a real schema violation in a
        // completed entry (invalid role, unknown field, crate-name mismatch)
        // is still reported as a schema error instead of being hidden by the
        // residual-hole finding.
        if let Err(err) = validate_hole_free_schema(&value, &expected_stem) {
            return Ok(blocked(format!("{layer_name}: schema error: {err}")));
        }
        return Ok(hole_outcome(layer_name, holes));
    }

    let mut anchor_strings = Vec::new();
    collect_anchor_strings(&value, &mut anchor_strings);
    let catalogue = match try_complete(value, &expected_stem) {
        Ok(catalogue) => catalogue,
        Err(err) => return Ok(blocked(format!("{layer_name}: schema error: {err}"))),
    };

    let dangling = dangling_anchors(&anchor_strings, spec_anchors);
    if !dangling.is_empty() {
        return Ok(dangling_outcome(layer_name, &dangling));
    }

    let ungrounded = ungrounded_entries(&catalogue);
    if !ungrounded.is_empty() {
        return Ok(grounding_outcome(layer_name, &ungrounded));
    }

    Ok(LayerOutcome { verdict: CatalogCheckVerdict::Pass, findings: vec![], holes: vec![] })
}

/// Outcome for an absent catalogue file.
fn missing_outcome(layer_name: &str, path: &Path, any_catalogue_exists: bool) -> LayerOutcome {
    if any_catalogue_exists {
        LayerOutcome {
            verdict: CatalogCheckVerdict::Blocked,
            findings: vec![format!(
                "{layer_name}: TDDD catalogue file missing: {}",
                path.display()
            )],
            holes: vec![],
        }
    } else {
        LayerOutcome {
            verdict: CatalogCheckVerdict::Skipped,
            findings: vec![format!("{layer_name}: catalogue file absent — skipped")],
            holes: vec![],
        }
    }
}

/// Outcome for a catalogue with residual `$todo` holes.
fn hole_outcome(layer_name: &str, holes: Vec<DraftHole>) -> LayerOutcome {
    let count = holes.len();
    LayerOutcome {
        verdict: CatalogCheckVerdict::Blocked,
        findings: vec![format!("{layer_name}: {count} unfilled $todo hole(s)")],
        holes,
    }
}

/// Outcome for a catalogue with dangling spec anchors.
fn dangling_outcome(layer_name: &str, dangling: &[String]) -> LayerOutcome {
    LayerOutcome {
        verdict: CatalogCheckVerdict::Blocked,
        findings: dangling
            .iter()
            .map(|anchor| format!("{layer_name}: dangling spec anchor `{anchor}`"))
            .collect(),
        holes: vec![],
    }
}

/// Outcome for completed entries that still lack formal or informal grounding.
fn grounding_outcome(layer_name: &str, ungrounded: &[String]) -> LayerOutcome {
    LayerOutcome {
        verdict: CatalogCheckVerdict::Blocked,
        findings: ungrounded
            .iter()
            .map(|entry| {
                format!("{layer_name}: ungrounded catalogue entry `{entry}` has no spec_refs or informal_grounds")
            })
            .collect(),
        holes: vec![],
    }
}

fn ungrounded_entries(catalogue: &domain::tddd::catalogue_v2::CatalogueDocument) -> Vec<String> {
    iter_catalogue_entries(catalogue)
        .filter_map(|entry| {
            let signal = evaluate_catalogue_entry_signal(
                entry.action,
                entry.spec_refs,
                entry.informal_grounds,
            );
            (signal == ConfidenceSignal::Red).then_some(entry.key)
        })
        .collect()
}

/// A blocked outcome with a single finding.
fn blocked(finding: String) -> LayerOutcome {
    LayerOutcome { verdict: CatalogCheckVerdict::Blocked, findings: vec![finding], holes: vec![] }
}

/// Aggregate per-layer outcomes; precedence Blocked > Pass > Skipped.
fn aggregate(outcomes: Vec<LayerOutcome>) -> CatalogCheckReport {
    let mut findings = Vec::new();
    let mut remaining_holes = Vec::new();
    let mut any_blocked = false;
    let mut any_pass = false;
    for outcome in outcomes {
        findings.extend(outcome.findings);
        remaining_holes.extend(outcome.holes);
        match outcome.verdict {
            CatalogCheckVerdict::Blocked => any_blocked = true,
            CatalogCheckVerdict::Pass => any_pass = true,
            CatalogCheckVerdict::Skipped => {}
        }
    }
    let verdict = if any_blocked {
        CatalogCheckVerdict::Blocked
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

    fn spec_set(ids: &[&str]) -> BTreeSet<SpecElementId> {
        ids.iter().map(|id| SpecElementId::try_new(*id).unwrap()).collect()
    }

    #[cfg(unix)]
    #[test]
    fn test_has_any_catalogue_file_rejects_symlinked_catalogue_path() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("real.json");
        let link = temp.path().join("domain-types.json");
        std::fs::write(&target, "{}").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let files = vec![("domain".to_owned(), link)];

        let err = has_any_catalogue_file(&files, temp.path()).unwrap_err();

        assert!(err.to_string().contains("symlink"), "{err}");
    }

    #[test]
    fn test_missing_file_skipped_before_catalogue_init_blocked_after_partial_init() {
        let missing = Path::new("/nonexistent/domain-types.json");
        let before_init =
            check_layer("domain", missing, Path::new("/"), &empty_set(), false).unwrap();
        assert_eq!(before_init.verdict, CatalogCheckVerdict::Skipped);
        let after_partial_init =
            check_layer("domain", missing, Path::new("/"), &empty_set(), true).unwrap();
        assert_eq!(after_partial_init.verdict, CatalogCheckVerdict::Blocked);
    }

    #[test]
    fn test_holes_block_after_catalogue_init() {
        let body = r#"{"schema_version":5,"crate_name":"domain","layer":"domain","types":{"Foo":{"role":{"$todo":"pick"}}},"traits":{},"functions":{}}"#;
        let (temp, path) = write_file("domain-types.json", body);
        let outcome = check_layer("domain", &path, temp.path(), &empty_set(), true).unwrap();
        assert_eq!(outcome.verdict, CatalogCheckVerdict::Blocked);
        assert!(!outcome.holes.is_empty());
    }

    #[test]
    fn test_clean_empty_catalogue_passes() {
        let body = r#"{"schema_version":5,"crate_name":"domain","layer":"domain","types":{},"traits":{},"functions":{}}"#;
        let (temp, path) = write_file("domain-types.json", body);
        let outcome = check_layer("domain", &path, temp.path(), &empty_set(), true).unwrap();
        assert_eq!(outcome.verdict, CatalogCheckVerdict::Pass);
    }

    #[test]
    fn test_grounded_delete_only_catalogue_passes() {
        // A delete tombstone has no role/docs holes, but it still carries
        // grounding so deletion cannot happen without a spec reason.
        let body = r#"{"schema_version":5,"crate_name":"domain","layer":"domain","types":{"OldType":{"action":"delete","module_path":"tddd","spec_refs":[{"file":"spec.json","anchor":"IN-01"}],"informal_grounds":[]}},"traits":{},"functions":{}}"#;
        let (temp, path) = write_file("domain-types.json", body);
        let outcome =
            check_layer("domain", &path, temp.path(), &spec_set(&["IN-01"]), true).unwrap();
        assert_eq!(outcome.verdict, CatalogCheckVerdict::Pass);
    }

    #[test]
    fn test_ungrounded_delete_only_catalogue_blocks() {
        let body = r#"{"schema_version":5,"crate_name":"domain","layer":"domain","types":{"OldType":{"action":"delete","module_path":"tddd","spec_refs":[],"informal_grounds":[]}},"traits":{},"functions":{}}"#;
        let (temp, path) = write_file("domain-types.json", body);
        let outcome = check_layer("domain", &path, temp.path(), &empty_set(), true).unwrap();
        assert_eq!(outcome.verdict, CatalogCheckVerdict::Blocked);
        assert!(outcome.findings[0].contains("ungrounded catalogue entry"));
    }

    #[test]
    fn test_invalid_schema_blocks() {
        let body = r#"{"schema_version":99,"crate_name":"domain","layer":"domain","types":{},"traits":{},"functions":{}}"#;
        let (temp, path) = write_file("domain-types.json", body);
        let outcome = check_layer("domain", &path, temp.path(), &empty_set(), true).unwrap();
        assert_eq!(outcome.verdict, CatalogCheckVerdict::Blocked);
    }

    #[test]
    fn test_invalid_schema_with_holes_blocks_before_hole_finding() {
        let body = r#"{"schema_version":99,"crate_name":"domain","layer":"domain","types":{"Foo":{"role":{"$todo":"pick"}}},"traits":{},"functions":{}}"#;
        let (temp, path) = write_file("domain-types.json", body);
        let outcome = check_layer("domain", &path, temp.path(), &empty_set(), true).unwrap();
        assert_eq!(outcome.verdict, CatalogCheckVerdict::Blocked);
        assert!(outcome.holes.is_empty());
    }

    #[test]
    fn test_schema_error_in_hole_free_entry_blocks_despite_unrelated_hole() {
        // `Foo` carries a legitimate `$todo` hole (still being annotated). `Bar` is
        // hole-free but has an unknown field that `read_catalogue`'s schema_version
        // probe does not catch — only a full decode does. The unrelated hole must
        // not mask `Bar`'s schema violation.
        let body = r#"{"schema_version":5,"crate_name":"domain","layer":"domain","types":{"Foo":{"role":{"$todo":"pick"}},"Bar":{"action":"add","role":{"ValueObject":{}},"kind":{"kind":"struct","shape":{"kind":"unit"}},"bogus_field":true}},"traits":{},"functions":{}}"#;
        let (temp, path) = write_file("domain-types.json", body);
        let outcome = check_layer("domain", &path, temp.path(), &empty_set(), true).unwrap();
        assert_eq!(outcome.verdict, CatalogCheckVerdict::Blocked);
    }

    #[test]
    fn test_crate_name_mismatch_with_hole_blocks() {
        // A `crate_name` that disagrees with the filename stem is a decode-time
        // schema error the schema_version probe never catches. It must be blocked even when an unrelated `$todo` hole is present.
        let body = r#"{"schema_version":5,"crate_name":"wrong","layer":"domain","types":{"Foo":{"role":{"$todo":"pick"}}},"traits":{},"functions":{}}"#;
        let (temp, path) = write_file("domain-types.json", body);
        let outcome = check_layer("domain", &path, temp.path(), &empty_set(), true).unwrap();
        assert_eq!(outcome.verdict, CatalogCheckVerdict::Blocked);
    }

    #[test]
    fn test_hole_free_valid_entry_alongside_hole_blocks() {
        // `Foo` carries a hole; `Bar` is hole-free and schema-valid. The hole-free
        // schema validation must not mask the residual hole: the check still blocks and reports the hole.
        let body = r#"{"schema_version":5,"crate_name":"domain","layer":"domain","types":{"Foo":{"role":{"$todo":"pick"}},"Bar":{"action":"add","role":{"ValueObject":{}},"kind":{"kind":"struct","shape":{"kind":"unit"}}}},"traits":{},"functions":{}}"#;
        let (temp, path) = write_file("domain-types.json", body);
        let outcome = check_layer("domain", &path, temp.path(), &empty_set(), true).unwrap();
        assert_eq!(outcome.verdict, CatalogCheckVerdict::Blocked);
        assert!(!outcome.holes.is_empty());
    }

    #[test]
    fn test_top_level_hole_does_not_mask_hole_free_schema_error() {
        // A top-level scalar hole used to survive pruning and make `try_complete`
        // return `Incomplete` before the codec could decode `Bar`. The
        // hole-free `Bar` schema error must still block instead of becoming non-blocking.
        let body = r#"{"schema_version":5,"crate_name":"domain","layer":{"$todo":"pick"},"types":{"Bar":{"action":"add","role":{"ValueObject":{}},"kind":{"kind":"struct","shape":{"kind":"unit"}},"bogus_field":true}},"traits":{},"functions":{}}"#;
        let (temp, path) = write_file("domain-types.json", body);
        let outcome = check_layer("domain", &path, temp.path(), &empty_set(), true).unwrap();
        assert_eq!(outcome.verdict, CatalogCheckVerdict::Blocked);
    }

    #[test]
    fn test_top_level_hole_with_valid_remainder_blocks() {
        // Top-level scalar holes still block even when the hole-free remainder
        // decodes cleanly.
        let body = r#"{"schema_version":5,"crate_name":"domain","layer":{"$todo":"pick"},"types":{},"traits":{},"functions":{}}"#;
        let (temp, path) = write_file("domain-types.json", body);
        let outcome = check_layer("domain", &path, temp.path(), &empty_set(), true).unwrap();
        assert_eq!(outcome.verdict, CatalogCheckVerdict::Blocked);
        assert!(!outcome.holes.is_empty());
    }

    #[test]
    fn test_root_hole_does_not_mask_hole_free_schema_error() {
        // A root-level marker is valid draft syntax for "the document is still
        // incomplete", but when other fields are present the hole-free remainder
        // must still be decoded so schema errors cannot become non-blocking.
        let body = r#"{"$todo":"finish draft","schema_version":5,"crate_name":"domain","layer":"domain","types":{"Bar":{"action":"add","role":{"ValueObject":{}},"kind":{"kind":"struct","shape":{"kind":"unit"}},"bogus_field":true}},"traits":{},"functions":{}}"#;
        let (temp, path) = write_file("domain-types.json", body);
        let outcome = check_layer("domain", &path, temp.path(), &empty_set(), true).unwrap();
        assert_eq!(outcome.verdict, CatalogCheckVerdict::Blocked);
    }

    #[test]
    fn test_root_hole_with_valid_remainder_blocks() {
        let body = r#"{"$todo":"finish draft","schema_version":5,"crate_name":"domain","layer":"domain","types":{},"traits":{},"functions":{}}"#;
        let (temp, path) = write_file("domain-types.json", body);
        let outcome = check_layer("domain", &path, temp.path(), &empty_set(), true).unwrap();
        assert_eq!(outcome.verdict, CatalogCheckVerdict::Blocked);
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
        let outcome = check_layer("domain", &path, temp.path(), &empty_set(), true).unwrap();
        assert_eq!(outcome.verdict, CatalogCheckVerdict::Blocked);
    }

    #[test]
    fn test_duplicate_entry_key_blocks_check() {
        // Two `Foo` entries under `types`: parsing through `serde_json::Value`
        // would collapse them to last-wins and bypass the codec's StrictMap
        // duplicate rejection. The check must fail closed on the raw duplicate.
        let body = r#"{"schema_version":5,"crate_name":"domain","layer":"domain","types":{"Foo":{"role":{"ValueObject":{}}},"Foo":{"role":{"ValueObject":{}}}},"traits":{},"functions":{}}"#;
        let (temp, path) = write_file("domain-types.json", body);
        let outcome = check_layer("domain", &path, temp.path(), &empty_set(), true).unwrap();
        assert_eq!(outcome.verdict, CatalogCheckVerdict::Blocked);
    }

    #[test]
    fn test_dangling_anchor_blocks() {
        let body = r#"{"schema_version":5,"crate_name":"domain","layer":"domain","types":{"Foo":{"role":{"ValueObject":{}},"kind":{"kind":"struct","shape":{"kind":"unit"}},"spec_refs":[{"file":"spec.json","anchor":"ZZ-99"}]}},"traits":{},"functions":{}}"#;
        let (temp, path) = write_file("domain-types.json", body);
        let outcome = check_layer("domain", &path, temp.path(), &empty_set(), true).unwrap();
        assert_eq!(outcome.verdict, CatalogCheckVerdict::Blocked);
    }

    #[test]
    fn test_aggregate_precedence() {
        let outcomes = vec![
            LayerOutcome { verdict: CatalogCheckVerdict::Pass, findings: vec![], holes: vec![] },
            LayerOutcome { verdict: CatalogCheckVerdict::Blocked, findings: vec![], holes: vec![] },
        ];
        assert_eq!(aggregate(outcomes).verdict, CatalogCheckVerdict::Blocked);

        let outcomes = vec![
            LayerOutcome { verdict: CatalogCheckVerdict::Skipped, findings: vec![], holes: vec![] },
            LayerOutcome { verdict: CatalogCheckVerdict::Pass, findings: vec![], holes: vec![] },
        ];
        assert_eq!(aggregate(outcomes).verdict, CatalogCheckVerdict::Pass);

        let outcomes = vec![LayerOutcome {
            verdict: CatalogCheckVerdict::Skipped,
            findings: vec![],
            holes: vec![],
        }];
        assert_eq!(aggregate(outcomes).verdict, CatalogCheckVerdict::Skipped);
    }
}
