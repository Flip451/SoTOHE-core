//! Verify that spec.md has valid YAML frontmatter with required fields.

use super::frontmatter::parse_yaml_frontmatter;
use domain::verify::{Finding, VerifyOutcome};
use std::path::Path;

/// Required frontmatter fields.
const REQUIRED_FIELDS: &[&str] = &["status", "version"];

/// Checks that `value` is a valid inline YAML mapping with string keys and
/// integer values, as required for the `signals` field.
///
/// Uses `serde_yaml` to parse the value.  The result must be a YAML mapping
/// where every key is a string and every value is an integer.  This rejects:
/// - plain scalars (`not_a_mapping`)
/// - sequences (`[1, 2, 3]`)
/// - malformed YAML (`{ blue: [1, 2 }`)
/// - entries without a value separator (`{ blue 1 }` — no colon means the
///   value is null, not an integer)
///
/// # Examples
///
/// ```text
/// // accepted
/// { blue: 0, yellow: 0, red: 0 }
/// { blue: 12, yellow: 1, red: 0 }
///
/// // rejected
/// not_a_mapping
/// { blue: [1, 2 }
/// { blue 1 }
/// ```
fn is_valid_inline_mapping(value: &str) -> bool {
    // Require inline flow mapping syntax (must start with `{`).
    // Block mappings like `blue: 0` are valid YAML but not the required format.
    if !value.starts_with('{') {
        return false;
    }
    match serde_yaml::from_str::<serde_yaml::Value>(value) {
        Ok(serde_yaml::Value::Mapping(map)) => map.iter().all(|(k, v)| {
            k.is_string() && matches!(v, serde_yaml::Value::Number(n) if n.as_u64().is_some())
        }),
        _ => false,
    }
}

/// Optional frontmatter fields that are validated when present.
///
/// Each optional field listed here is accepted when absent.  When present the
/// field is subject to structural validation (e.g. `signals` must be an inline
/// YAML mapping value starting with `{`).
const OPTIONAL_FIELDS: &[&str] = &["signals"];

/// Verifies spec.md has YAML frontmatter with `status` and `version`.
///
/// The optional `signals` field, when present, must be an inline YAML mapping
/// (value starts with `{`).  A plain scalar value for `signals` is reported as
/// an error.
///
/// # Errors
///
/// Returns findings when the file cannot be read, when YAML frontmatter
/// delimiters are missing, required fields are absent, or an optional field is
/// present but malformed.
pub fn verify(spec_path: &Path) -> VerifyOutcome {
    let content = match std::fs::read_to_string(spec_path) {
        Ok(c) => c,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![Finding::error(format!(
                "cannot read {}: {e}",
                spec_path.display()
            ))]);
        }
    };

    let Some(fm) = parse_yaml_frontmatter(&content) else {
        return VerifyOutcome::from_findings(vec![Finding::error(format!(
            "{}: missing or invalid YAML frontmatter (expected '---' delimiters)",
            spec_path.display()
        ))]);
    };

    let mut findings = Vec::new();

    // Check for required fields (line-based: field must appear at column 0).
    for field in REQUIRED_FIELDS {
        let pattern = format!("{field}:");
        if !fm.frontmatter.lines().any(|line| line.starts_with(&pattern)) {
            findings.push(Finding::error(format!(
                "{}: YAML frontmatter missing required field '{field}'",
                spec_path.display()
            )));
        }
    }

    // Validate optional fields when present.
    //
    // `signals` must be an inline mapping: the characters after `signals:` and
    // any surrounding whitespace must start with `{`.  Block mappings (next
    // line indented) and plain scalars are rejected so that Phase 2 tooling can
    // rely on a consistent format.
    for field in OPTIONAL_FIELDS {
        let pattern = format!("{field}:");
        if let Some(line) = fm.frontmatter.lines().find(|l| l.starts_with(&pattern)) {
            let value = line[pattern.len()..].trim();
            if !is_valid_inline_mapping(value) {
                findings.push(Finding::error(format!(
                    "{}: optional field '{field}' must be a valid inline YAML mapping \
                     (e.g. `{field}: {{ blue: 0, yellow: 0, red: 0 }}`), got: {value:?}",
                    spec_path.display()
                )));
            }
        }
    }

    if findings.is_empty() { VerifyOutcome::pass() } else { VerifyOutcome::from_findings(findings) }
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
    fn test_spec_frontmatter_fails_missing_status() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(&spec, "---\nversion: \"1.0\"\n---\n# Content\n").unwrap();
        let outcome = verify(&spec);
        assert!(outcome.has_errors());
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
        assert_eq!(outcome.error_count(), 2);
    }

    #[test]
    fn test_spec_frontmatter_fails_without_closing_delimiter() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(&spec, "---\nstatus: draft\nversion: \"1.0\"\n").unwrap();
        let outcome = verify(&spec);
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_spec_frontmatter_rejects_malformed_opening_delimiter() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(&spec, "---yaml\nstatus: draft\nversion: \"1.0\"\n---\n").unwrap();
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
        // Existing spec files without signals must continue to pass.
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(&spec, "---\nstatus: draft\nversion: \"1.0\"\n---\n# Content\n").unwrap();
        let outcome = verify(&spec);
        assert!(!outcome.has_errors(), "spec with status+version and no signals must pass");
    }

    #[test]
    fn test_spec_frontmatter_passes_with_valid_signals_mapping() {
        // signals with an inline mapping value must pass.
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(
            &spec,
            "---\nstatus: draft\nversion: \"1.0\"\nsignals: { blue: 0, yellow: 0, red: 0 }\n---\n# Content\n",
        )
        .unwrap();
        let outcome = verify(&spec);
        assert!(!outcome.has_errors(), "spec with valid signals mapping must pass");
    }

    #[test]
    fn test_spec_frontmatter_passes_with_nonzero_signals() {
        // signals with non-zero counts (the realistic Phase 2 scenario) must pass.
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(
            &spec,
            "---\nstatus: approved\nversion: \"1.2\"\nsignals: { blue: 12, yellow: 1, red: 0 }\n---\n# Content\n",
        )
        .unwrap();
        let outcome = verify(&spec);
        assert!(!outcome.has_errors(), "spec with non-zero signals mapping must pass");
    }

    #[test]
    fn test_spec_frontmatter_fails_with_only_signals_missing_required_fields() {
        // Only signals present — required fields are missing so it must fail.
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(&spec, "---\nsignals: { blue: 0, yellow: 0, red: 0 }\n---\n# Content\n")
            .unwrap();
        let outcome = verify(&spec);
        assert!(
            outcome.has_errors(),
            "spec missing required fields must fail even if signals present"
        );
        assert_eq!(outcome.error_count(), 2, "must report one error per missing required field");
    }

    #[test]
    fn test_spec_frontmatter_fails_with_malformed_signals_scalar_value() {
        // signals present but value is a plain scalar, not a mapping.
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(
            &spec,
            "---\nstatus: draft\nversion: \"1.0\"\nsignals: not_a_mapping\n---\n# Content\n",
        )
        .unwrap();
        let outcome = verify(&spec);
        assert!(outcome.has_errors(), "malformed signals value must produce an error");
    }

    #[test]
    fn test_spec_frontmatter_fails_with_signals_empty_value() {
        // signals: (empty value after colon) — empty string is not a mapping.
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(&spec, "---\nstatus: draft\nversion: \"1.0\"\nsignals:\n---\n# Content\n")
            .unwrap();
        let outcome = verify(&spec);
        assert!(outcome.has_errors(), "signals with empty value must produce an error");
    }

    #[test]
    fn test_spec_frontmatter_fails_with_signals_mismatched_braces() {
        // Balanced-brace heuristic would accept this, but proper YAML parsing must reject it.
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(
            &spec,
            "---\nstatus: draft\nversion: \"1.0\"\nsignals: { blue: [1, 2 }\n---\n# Content\n",
        )
        .unwrap();
        let outcome = verify(&spec);
        assert!(
            outcome.has_errors(),
            "malformed YAML with mismatched braces/brackets must be rejected"
        );
    }

    #[test]
    fn test_spec_frontmatter_fails_with_signals_missing_colon_in_mapping() {
        // `{ blue 1 }` is not valid YAML mapping syntax.
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(
            &spec,
            "---\nstatus: draft\nversion: \"1.0\"\nsignals: { blue 1 }\n---\n# Content\n",
        )
        .unwrap();
        let outcome = verify(&spec);
        assert!(
            outcome.has_errors(),
            "invalid YAML mapping syntax (missing colon) must be rejected"
        );
    }
}
