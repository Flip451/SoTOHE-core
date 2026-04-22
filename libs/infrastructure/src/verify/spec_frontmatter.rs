//! Verify that spec.md has valid YAML frontmatter with required fields.
//!
//! When a sibling `spec.json` exists, the schema check is performed by
//! attempting to decode it via `crate::spec::codec::decode`. A successful
//! decode means the schema is valid. Otherwise, the markdown frontmatter
//! check (legacy path) is used.

use super::frontmatter::parse_yaml_frontmatter;
use domain::verify::{VerifyFinding, VerifyOutcome};
use std::path::Path;

/// Serde-compatible mirror of `domain::SignalCounts` for YAML deserialization.
///
/// The domain type is serde-free (no serde dependency in domain layer).
/// This struct validates the frontmatter `signals` field via deserialization.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)] // Fields are validated by deserialization, not read directly.
struct SignalCountsDto {
    blue: u32,
    yellow: u32,
    red: u32,
}

/// Frontmatter structure for YAML deserialization.
///
/// `status` is optional because schema-v2 `spec.md` no longer emits it.
/// `version` remains required for both v1 and v2 formats.
/// Optional fields (`signals`) use `Option` and are validated when present.
/// Unknown fields are silently ignored via `#[serde(flatten)]`.
///
/// NOTE: Enforcing `deny_unknown_fields` to reject legacy v1 fields (`status`,
/// `approved_at`, `content_hash`) requires updating the CLI test at
/// `apps/cli/src/commands/verify.rs` (out of infrastructure scope). Until that
/// update is in scope, unknown fields are accepted to prevent CI breakage.
/// See the full-model review finding for context.
#[derive(serde::Deserialize)]
#[allow(dead_code)] // Fields are validated by deserialization, not read directly.
struct SpecFrontmatterDto {
    #[serde(default)]
    status: Option<String>,
    version: serde_yaml::Value, // Accept both string "1.0" and number 1.0
    #[serde(default)]
    signals: Option<SignalCountsDto>,
    #[serde(flatten)]
    _extra: serde_yaml::Mapping, // Ignore unknown fields
}

/// Verifies that a `spec.json` file has a valid schema by attempting to decode it.
///
/// A successful decode proves the JSON is well-formed, passes schema-version
/// validation, and satisfies all domain constraints. Failure means the schema
/// is invalid.
///
/// # Errors
///
/// Returns findings when the file cannot be read or the decode fails.
pub fn verify_spec_schema(spec_json_path: &Path) -> VerifyOutcome {
    let json = match std::fs::read_to_string(spec_json_path) {
        Ok(s) => s,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "cannot read {}: {e}",
                spec_json_path.display()
            ))]);
        }
    };

    match crate::spec::codec::decode(&json) {
        Ok(_) => VerifyOutcome::pass(),
        Err(e) => VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "{}: spec.json schema invalid: {e}",
            spec_json_path.display()
        ))]),
    }
}

/// Derives the sibling `spec.json` path from a `spec.md` path.
///
/// Returns `None` when the path has no parent directory.
fn sibling_spec_json(spec_md_path: &Path) -> Option<std::path::PathBuf> {
    spec_md_path
        .parent()
        .map(|dir| if dir.as_os_str().is_empty() { Path::new(".") } else { dir })
        .map(|dir| dir.join("spec.json"))
}

/// Verifies spec.md has YAML frontmatter with `version` (required).
/// `status` is accepted when present (v1 format) but not required (v2 omits it).
///
/// When a sibling `spec.json` exists next to `spec_path`, delegates to
/// `verify_spec_schema` (decode-based check). Otherwise falls back to the
/// YAML frontmatter check (legacy path).
///
/// The optional `signals` field, when present, must be a valid mapping with
/// `blue`, `yellow`, `red` as non-negative integer fields. Both inline
/// (`{ blue: 0, yellow: 0, red: 0 }`) and block-style YAML are accepted.
///
/// # Errors
///
/// Returns findings when the file cannot be read, when YAML frontmatter
/// delimiters are missing, or when the frontmatter content is invalid.
pub fn verify(spec_path: &Path) -> VerifyOutcome {
    // Delegate to spec.json schema check when sibling exists.
    if let Some(spec_json_path) = sibling_spec_json(spec_path) {
        if spec_json_path.is_file() {
            return verify_spec_schema(&spec_json_path);
        }
    }

    // Legacy markdown frontmatter check.
    let content = match std::fs::read_to_string(spec_path) {
        Ok(c) => c,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "cannot read {}: {e}",
                spec_path.display()
            ))]);
        }
    };

    let Some(fm) = parse_yaml_frontmatter(&content) else {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "{}: missing or invalid YAML frontmatter (expected '---' delimiters)",
            spec_path.display()
        ))]);
    };

    // Parse the entire frontmatter as a typed DTO.
    // This validates required fields (status, version) and optional fields (signals)
    // in one pass via serde deserialization.
    match serde_yaml::from_str::<SpecFrontmatterDto>(&fm.frontmatter) {
        Ok(dto) => {
            // Post-validation: if signals key is present but null/empty, reject.
            if dto.signals.is_none() && fm.frontmatter.lines().any(|l| l.starts_with("signals:")) {
                return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                    "{}: 'signals' field is present but empty/null; \
                     provide a mapping like `signals: {{ blue: 0, yellow: 0, red: 0 }}`",
                    spec_path.display()
                ))]);
            }
            VerifyOutcome::pass()
        }
        Err(e) => {
            // Derive granular errors from serde error message.
            // serde_yaml reports missing fields as "missing field `<name>`".
            let err_str = e.to_string();
            let mut findings = Vec::new();

            // `status` is optional (schema v2 no longer emits it); only `version` is required.
            let missing_version = err_str.contains("missing field `version`");

            if missing_version {
                findings.push(VerifyFinding::error(format!(
                    "{}: YAML frontmatter missing required field 'version'",
                    spec_path.display()
                )));
            } else {
                findings.push(VerifyFinding::error(format!(
                    "{}: invalid YAML frontmatter: {e}",
                    spec_path.display()
                )));
            }

            VerifyOutcome::from_findings(findings)
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_spec_frontmatter_fails_without_frontmatter() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(&spec, "# No frontmatter\n").unwrap();
        let outcome = verify(&spec);
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_spec_frontmatter_fails_missing_version() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(&spec, "---\nstatus: draft\n---\n# Content\n").unwrap();
        let outcome = verify(&spec);
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_spec_frontmatter_passes_without_status() {
        // Schema v2 spec.md no longer emits `status`; `status` is now optional.
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(&spec, "---\nversion: \"1.0\"\n---\n# Content\n").unwrap();
        let outcome = verify(&spec);
        assert!(!outcome.has_errors(), "v2 format without status must pass");
    }

    #[test]
    fn test_spec_frontmatter_passes_with_valid_frontmatter() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(&spec, "---\nstatus: draft\nversion: \"1.0\"\n---\n# Content\n").unwrap();
        let outcome = verify(&spec);
        assert!(!outcome.has_errors());
    }

    #[test]
    fn test_spec_frontmatter_fails_for_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("nonexistent.md");
        let outcome = verify(&spec);
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_spec_frontmatter_fails_missing_both_fields() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(&spec, "---\ntitle: something\n---\n# Content\n").unwrap();
        let outcome = verify(&spec);
        assert!(outcome.has_errors());
        // serde_yaml reports the first missing field it encounters; at least 1 error.
        assert!(outcome.error_count() >= 1);
    }

    #[test]
    fn test_spec_frontmatter_fails_without_closing_delimiter() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(&spec, "---\nversion: \"1.0\"\n").unwrap();
        let outcome = verify(&spec);
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_spec_frontmatter_rejects_malformed_opening_delimiter() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(&spec, "---yaml\nversion: \"1.0\"\n---\n").unwrap();
        let outcome = verify(&spec);
        assert!(outcome.has_errors(), "---yaml is not a valid frontmatter delimiter");
    }

    #[test]
    fn test_spec_frontmatter_rejects_four_dash_opening() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(&spec, "----\nstatus: draft\nversion: \"1.0\"\n---\n").unwrap();
        let outcome = verify(&spec);
        assert!(outcome.has_errors(), "---- is not a valid frontmatter delimiter");
    }

    #[test]
    fn test_spec_frontmatter_passes_with_extra_fields() {
        // Extra fields (like `author`) are silently ignored — `status` was preserved
        // for backward compatibility with v1 spec.md files. Full v2 enforcement
        // (deny_unknown_fields) requires also updating apps/cli/src/commands/verify.rs
        // (out of infrastructure scope).
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(
            &spec,
            "---\nstatus: active\nversion: \"2.1\"\nauthor: Alice\n---\n# Content\n",
        )
        .unwrap();
        let outcome = verify(&spec);
        assert!(!outcome.has_errors());
    }

    #[test]
    fn test_spec_frontmatter_rejects_indented_closing_delimiter() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(&spec, "---\nstatus: draft\nversion: \"1.0\"\n  ---\n# Content\n").unwrap();
        let outcome = verify(&spec);
        assert!(outcome.has_errors(), "indented closing --- must not be accepted");
    }

    #[test]
    fn test_spec_frontmatter_rejects_leading_whitespace_before_opening() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(&spec, "\n---\nstatus: draft\nversion: \"1.0\"\n---\n").unwrap();
        let outcome = verify(&spec);
        assert!(outcome.has_errors(), "leading newline before --- must not be accepted");
    }

    #[test]
    fn test_spec_frontmatter_rejects_leading_spaces_before_opening() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(&spec, "  ---\nstatus: draft\nversion: \"1.0\"\n---\n").unwrap();
        let outcome = verify(&spec);
        assert!(outcome.has_errors(), "leading spaces before --- must not be accepted");
    }

    #[test]
    fn test_spec_frontmatter_rejects_indented_fields_in_block_scalar() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        // status and version appear only as indented content inside a block scalar
        std::fs::write(&spec, "---\nnotes: |\n  status: draft\n  version: 1.0\n---\n# Content\n")
            .unwrap();
        let outcome = verify(&spec);
        assert!(
            outcome.has_errors(),
            "indented fields inside block scalars must not satisfy required field check"
        );
    }

    // --- signals field tests (Phase 2 preparation) ---

    #[test]
    fn test_spec_frontmatter_passes_without_signals() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        // v2 format: only `version`, no `status`
        std::fs::write(&spec, "---\nversion: \"1.0\"\n---\n# Content\n").unwrap();
        let outcome = verify(&spec);
        assert!(!outcome.has_errors(), "spec with version and no signals must pass");
    }

    #[test]
    fn test_spec_frontmatter_passes_with_valid_signals_inline() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(
            &spec,
            "---\nversion: \"1.0\"\nsignals: { blue: 0, yellow: 0, red: 0 }\n---\n# Content\n",
        )
        .unwrap();
        let outcome = verify(&spec);
        assert!(!outcome.has_errors(), "spec with valid inline signals must pass");
    }

    #[test]
    fn test_spec_frontmatter_passes_with_valid_signals_block_style() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(
            &spec,
            "---\nversion: \"1.0\"\nsignals:\n  blue: 12\n  yellow: 1\n  red: 0\n---\n# Content\n",
        )
        .unwrap();
        let outcome = verify(&spec);
        assert!(!outcome.has_errors(), "spec with valid block-style signals must pass");
    }

    #[test]
    fn test_spec_frontmatter_passes_with_nonzero_signals() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(
            &spec,
            "---\nversion: \"1.2\"\nsignals: { blue: 12, yellow: 1, red: 0 }\n---\n# Content\n",
        )
        .unwrap();
        let outcome = verify(&spec);
        assert!(!outcome.has_errors(), "spec with non-zero signals must pass");
    }

    #[test]
    fn test_spec_frontmatter_fails_with_only_signals_missing_required_fields() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(&spec, "---\nsignals: { blue: 0, yellow: 0, red: 0 }\n---\n# Content\n")
            .unwrap();
        let outcome = verify(&spec);
        assert!(
            outcome.has_errors(),
            "spec missing required fields must fail even if signals present"
        );
        assert!(
            outcome.error_count() >= 1,
            "must report at least one missing required field error"
        );
    }

    #[test]
    fn test_spec_frontmatter_fails_with_malformed_signals_scalar_value() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(&spec, "---\nversion: \"1.0\"\nsignals: not_a_mapping\n---\n# Content\n")
            .unwrap();
        let outcome = verify(&spec);
        assert!(outcome.has_errors(), "malformed signals value must produce an error");
    }

    #[test]
    fn test_spec_frontmatter_fails_with_signals_empty_value() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(&spec, "---\nversion: \"1.0\"\nsignals:\n---\n# Content\n").unwrap();
        let outcome = verify(&spec);
        assert!(outcome.has_errors(), "signals with empty value must produce an error");
    }

    #[test]
    fn test_spec_frontmatter_fails_with_signals_mismatched_braces() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(&spec, "---\nversion: \"1.0\"\nsignals: { blue: [1, 2 }\n---\n# Content\n")
            .unwrap();
        let outcome = verify(&spec);
        assert!(
            outcome.has_errors(),
            "malformed YAML with mismatched braces/brackets must be rejected"
        );
    }

    #[test]
    fn test_spec_frontmatter_fails_with_negative_signal_count() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(
            &spec,
            "---\nversion: \"1.0\"\nsignals: { blue: -1, yellow: 0, red: 0 }\n---\n# Content\n",
        )
        .unwrap();
        let outcome = verify(&spec);
        assert!(outcome.has_errors(), "negative signal count must be rejected");
    }

    #[test]
    fn test_spec_frontmatter_fails_with_signals_missing_field() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(
            &spec,
            "---\nversion: \"1.0\"\nsignals: { blue: 0, yellow: 0 }\n---\n# Content\n",
        )
        .unwrap();
        let outcome = verify(&spec);
        assert!(outcome.has_errors(), "signals missing 'red' field must be rejected");
    }

    // --- verify_spec_schema() tests ---

    const VALID_SPEC_JSON: &str = r#"{
  "schema_version": 2,
  "version": "1.0",
  "title": "Feature",
  "scope": { "in_scope": [], "out_of_scope": [] }
}"#;

    #[test]
    fn test_verify_spec_schema_with_valid_json_passes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("spec.json");
        std::fs::write(&path, VALID_SPEC_JSON).unwrap();
        let outcome = verify_spec_schema(&path);
        assert!(!outcome.has_errors(), "valid spec.json should pass: {outcome:?}");
    }

    #[test]
    fn test_verify_spec_schema_with_missing_file_fails() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.json");
        let outcome = verify_spec_schema(&path);
        assert!(outcome.has_errors(), "missing file should fail");
    }

    #[test]
    fn test_verify_spec_schema_with_invalid_json_fails() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("spec.json");
        std::fs::write(&path, "{not valid json}").unwrap();
        let outcome = verify_spec_schema(&path);
        assert!(outcome.has_errors(), "invalid JSON should fail");
    }

    #[test]
    fn test_verify_spec_schema_with_wrong_schema_version_fails() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("spec.json");
        // schema_version 99 is unsupported (valid is 2); codec must reject it.
        std::fs::write(
            &path,
            r#"{"schema_version":99,"version":"1","title":"T","scope":{"in_scope":[],"out_of_scope":[]}}"#,
        )
        .unwrap();
        let outcome = verify_spec_schema(&path);
        assert!(outcome.has_errors(), "unsupported schema version should fail");
    }

    #[test]
    fn test_verify_spec_schema_with_empty_title_fails() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("spec.json");
        // schema v2 with empty title must fail domain validation.
        std::fs::write(
            &path,
            r#"{"schema_version":2,"version":"1","title":"","scope":{"in_scope":[],"out_of_scope":[]}}"#,
        )
        .unwrap();
        let outcome = verify_spec_schema(&path);
        assert!(outcome.has_errors(), "empty title should fail domain validation");
    }

    // --- verify() delegation tests ---

    #[test]
    fn test_verify_delegates_to_spec_json_when_sibling_exists() {
        let dir = tempfile::tempdir().unwrap();
        // Write a valid spec.json
        std::fs::write(dir.path().join("spec.json"), VALID_SPEC_JSON).unwrap();
        // Write a spec.md that lacks required frontmatter (would fail under legacy path)
        std::fs::write(dir.path().join("spec.md"), "# No frontmatter\n").unwrap();
        let outcome = verify(&dir.path().join("spec.md"));
        // spec.json takes priority and is valid
        assert!(
            !outcome.has_errors(),
            "spec.json delegation should override markdown failure: {outcome:?}"
        );
    }

    #[test]
    fn test_verify_spec_json_invalid_propagates_failure_through_verify() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("spec.json"), "{not json}").unwrap();
        std::fs::write(dir.path().join("spec.md"), "---\nversion: \"1.0\"\n---\n# Content\n")
            .unwrap();
        let outcome = verify(&dir.path().join("spec.md"));
        assert!(outcome.has_errors(), "invalid spec.json should propagate failure");
    }

    #[test]
    fn test_verify_falls_back_to_markdown_when_no_spec_json() {
        let dir = tempfile::tempdir().unwrap();
        // No spec.json — legacy path uses v2-only frontmatter
        std::fs::write(dir.path().join("spec.md"), "---\nversion: \"1.0\"\n---\n# Content\n")
            .unwrap();
        let outcome = verify(&dir.path().join("spec.md"));
        assert!(!outcome.has_errors(), "v2 markdown path should pass: {outcome:?}");
    }
}
