//! ADR front-matter typed deserialization and anchor existence verification.
//!
//! Provides `check_adr_anchor_exists`, which verifies that an `AdrRef.anchor`
//! value (e.g. `"D1"`) is present in the `decisions[].id` list of the ADR's
//! YAML front-matter.
//!
//! Implements D3 of ADR 2026-05-27-1601: ADR canonical hash is not computed,
//! but anchor existence in `decisions[].id` is verified as a structural gate
//! (IN-03 / AC-05).
//!
//! Uses the infrastructure ADR front-matter codec per the typed-deserialization
//! convention. That codec owns the strict DTOs and schema validation; this
//! module only checks membership in the validated `decisions[].id` values.
//!
//! Fail-closed contract:
//! - ADR file missing front-matter → error finding.
//! - Front-matter present but schema parsing fails → error finding.
//! - `decisions` array wrong type or malformed entries → error finding.
//! - `decisions` array empty or anchor not found → error finding.

use std::path::Path;

use crate::adr_decision::parse_adr_frontmatter;
use domain::AdrDecisionEntry;
use domain::verify::VerifyFinding;

// ---------------------------------------------------------------------------
// Decision helpers
// ---------------------------------------------------------------------------

fn decision_id(decision: &AdrDecisionEntry) -> &str {
    match decision {
        AdrDecisionEntry::ProposedDecision(decision) => decision.common.id(),
        AdrDecisionEntry::AcceptedDecision(decision) => decision.common.id(),
        AdrDecisionEntry::ImplementedDecision(decision) => decision.common.id(),
        AdrDecisionEntry::SupersededDecision(decision) => decision.common.id(),
        AdrDecisionEntry::DeprecatedDecision(decision) => decision.common.id(),
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Verify that `anchor` appears in the ADR front-matter's `decisions[].id` list.
///
/// Steps:
/// 1. Read the ADR file at `adr_path` (already existence-checked by the caller).
/// 2. Parse and validate the YAML front-matter using the strict ADR codec.
/// 3. Check that `anchor` matches one of the validated `decisions[].id` values.
///
/// On any failure (missing front-matter, parse error, anchor not found), an
/// `error`-level `VerifyFinding` is pushed to `findings` — the function is
/// fail-closed throughout.
///
/// # Parameters
/// - `adr_path`: absolute, resolved path to the ADR file (already existence-checked).
/// - `anchor`: the `AdrRef.anchor` value to look up (e.g. `"D1"`).
/// - `context`: human-readable prefix for the finding message.
/// - `findings`: mutable accumulator for `VerifyFinding` entries.
pub(crate) fn check_adr_anchor_exists(
    adr_path: &Path,
    anchor: &str,
    context: &str,
    findings: &mut Vec<VerifyFinding>,
) {
    // Read the ADR file.
    let content = match std::fs::read_to_string(adr_path) {
        Ok(c) => c,
        Err(e) => {
            findings.push(VerifyFinding::error(format!(
                "{context}: cannot read ADR file '{}': {e}",
                adr_path.display()
            )));
            return;
        }
    };

    let frontmatter = match parse_adr_frontmatter(&content) {
        Ok(frontmatter) => frontmatter,
        Err(e) => {
            findings.push(VerifyFinding::error(format!(
                "{context}: ADR '{}' front-matter parse error \
                 (typed-deserialization failure): {e}",
                adr_path.display()
            )));
            return;
        }
    };

    // Check that the anchor is present in `decisions[].id`.
    let decisions = frontmatter.decisions();
    let found = decisions.iter().any(|decision| decision_id(decision) == anchor);
    if !found {
        findings.push(VerifyFinding::error(format!(
            "{context}: anchor '{}' not found in decisions[].id of ADR '{}' \
             (available: [{}])",
            anchor,
            adr_path.display(),
            decisions.iter().map(decision_id).collect::<Vec<_>>().join(", ")
        )));
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn write_file(dir: &std::path::Path, name: &str, content: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, content).unwrap();
        path
    }

    // ADR front-matter with decisions D1, D2, D3.
    const ADR_WITH_DECISIONS: &str = r#"---
adr_id: test-adr
decisions:
  - id: D1
    status: proposed
  - id: D2
    status: proposed
  - id: D3
    status: proposed
---
# Test ADR

## D1 Section
Decision content.
"#;

    // ADR with no front-matter at all.
    const ADR_NO_FRONTMATTER: &str = r#"# Test ADR

## D1 Section
Decision content.
"#;

    // ADR with front-matter but no `decisions` field.
    const ADR_FRONTMATTER_NO_DECISIONS: &str = r#"---
adr_id: test-adr
---
# Test ADR
"#;

    // ADR with front-matter and empty `decisions` array.
    const ADR_FRONTMATTER_EMPTY_DECISIONS: &str = r#"---
adr_id: test-adr
decisions: []
---
# Test ADR
"#;

    // ADR with invalid YAML (decision entries are not maps).
    const ADR_FRONTMATTER_BAD_YAML: &str = r#"---
adr_id: test-adr
decisions:
  - "not a map"
  - "also not a map"
---
# Test ADR
"#;

    // ADR with a decision missing the required lifecycle status.
    const ADR_FRONTMATTER_DECISION_MISSING_STATUS: &str = r#"---
adr_id: test-adr
decisions:
  - id: D1
---
# Test ADR
"#;

    // ADR with an unrecognized top-level schema field.
    const ADR_FRONTMATTER_UNKNOWN_FIELD: &str = r#"---
adr_id: test-adr
title: Some Title
decisions:
  - id: D1
    status: proposed
---
# Test ADR
"#;

    #[test]
    fn test_anchors_exist_in_decisions_pass() {
        let tmp = TempDir::new().unwrap();
        let adr_path = write_file(tmp.path(), "test.md", ADR_WITH_DECISIONS);

        for anchor in ["D1", "D2"] {
            let mut findings = Vec::new();
            check_adr_anchor_exists(&adr_path, anchor, "test context", &mut findings);
            assert!(
                findings.is_empty(),
                "anchor '{anchor}' exists in decisions, expected no findings, got: {findings:?}"
            );
        }
    }

    #[test]
    fn test_anchor_not_in_decisions_reports_error() {
        let tmp = TempDir::new().unwrap();
        let adr_path = write_file(tmp.path(), "test.md", ADR_WITH_DECISIONS);

        let mut findings = Vec::new();
        check_adr_anchor_exists(&adr_path, "D99", "test context", &mut findings);
        assert_eq!(findings.len(), 1, "anchor not found must produce exactly one error finding");
        assert!(
            findings[0].message().contains("D99"),
            "finding must mention the missing anchor 'D99': {}",
            findings[0].message()
        );
    }

    #[test]
    fn test_no_frontmatter_reports_error() {
        let tmp = TempDir::new().unwrap();
        let adr_path = write_file(tmp.path(), "test.md", ADR_NO_FRONTMATTER);

        let mut findings = Vec::new();
        check_adr_anchor_exists(&adr_path, "D1", "test context", &mut findings);
        assert_eq!(
            findings.len(),
            1,
            "missing front-matter must produce exactly one error finding"
        );
        assert!(
            findings[0].message().contains("no YAML front-matter")
                || findings[0].message().contains("front-matter"),
            "finding must mention missing front-matter: {}",
            findings[0].message()
        );
    }

    #[test]
    fn test_frontmatter_without_decisions_field_reports_anchor_not_found_error() {
        let tmp = TempDir::new().unwrap();
        let adr_path = write_file(tmp.path(), "test.md", ADR_FRONTMATTER_NO_DECISIONS);

        let mut findings = Vec::new();
        check_adr_anchor_exists(&adr_path, "D1", "test context", &mut findings);
        assert_eq!(
            findings.len(),
            1,
            "missing decisions field must produce exactly one error finding"
        );
        assert!(
            findings[0].message().contains("D1"),
            "finding must mention the sought anchor 'D1': {}",
            findings[0].message()
        );
    }

    #[test]
    fn test_empty_decisions_array_reports_anchor_not_found_error() {
        let tmp = TempDir::new().unwrap();
        let adr_path = write_file(tmp.path(), "test.md", ADR_FRONTMATTER_EMPTY_DECISIONS);

        let mut findings = Vec::new();
        check_adr_anchor_exists(&adr_path, "D1", "test context", &mut findings);
        assert_eq!(
            findings.len(),
            1,
            "empty decisions array must produce exactly one error finding"
        );
        assert!(
            findings[0].message().contains("D1"),
            "finding must mention the sought anchor 'D1': {}",
            findings[0].message()
        );
    }

    #[test]
    fn test_bad_yaml_decision_entries_reports_typed_deserialization_error() {
        let tmp = TempDir::new().unwrap();
        let adr_path = write_file(tmp.path(), "test.md", ADR_FRONTMATTER_BAD_YAML);

        let mut findings = Vec::new();
        check_adr_anchor_exists(&adr_path, "D1", "test context", &mut findings);
        assert_eq!(
            findings.len(),
            1,
            "type-mismatch in decisions entries must produce exactly one error finding"
        );
        assert!(
            findings[0].message().contains("parse error")
                || findings[0].message().contains("typed-deserialization"),
            "finding must mention parse/typed-deserialization error: {}",
            findings[0].message()
        );
    }

    #[test]
    fn test_decision_missing_status_reports_typed_deserialization_error() {
        let tmp = TempDir::new().unwrap();
        let adr_path = write_file(tmp.path(), "test.md", ADR_FRONTMATTER_DECISION_MISSING_STATUS);

        let mut findings = Vec::new();
        check_adr_anchor_exists(&adr_path, "D1", "test context", &mut findings);
        assert_eq!(
            findings.len(),
            1,
            "missing status in decisions entries must produce exactly one error finding"
        );
        assert!(
            findings[0].message().contains("parse error")
                || findings[0].message().contains("typed-deserialization"),
            "finding must mention parse/typed-deserialization error: {}",
            findings[0].message()
        );
    }

    #[test]
    fn test_unknown_frontmatter_field_reports_typed_deserialization_error() {
        let tmp = TempDir::new().unwrap();
        let adr_path = write_file(tmp.path(), "test.md", ADR_FRONTMATTER_UNKNOWN_FIELD);

        let mut findings = Vec::new();
        check_adr_anchor_exists(&adr_path, "D1", "test context", &mut findings);
        assert_eq!(
            findings.len(),
            1,
            "unknown front-matter fields must produce exactly one error finding"
        );
        assert!(
            findings[0].message().contains("parse error")
                || findings[0].message().contains("typed-deserialization"),
            "finding must mention parse/typed-deserialization error: {}",
            findings[0].message()
        );
    }

    #[test]
    fn test_context_prefix_appears_in_finding_message() {
        let tmp = TempDir::new().unwrap();
        let adr_path = write_file(tmp.path(), "test.md", ADR_NO_FRONTMATTER);

        let mut findings = Vec::new();
        check_adr_anchor_exists(&adr_path, "D1", "spec.json adr_ref", &mut findings);
        assert!(
            findings[0].message().contains("spec.json adr_ref"),
            "finding must contain the context prefix: {}",
            findings[0].message()
        );
    }
}
