//! Verify that spec.md has valid YAML frontmatter with required fields.

use super::frontmatter::parse_yaml_frontmatter;
use domain::verify::{Finding, VerifyOutcome};
use std::path::Path;

/// Serde-compatible mirror of `domain::SignalCounts` for YAML deserialization.
///
/// The domain type is serde-free (no serde dependency in domain layer).
/// This struct validates the frontmatter `signals` field via deserialization.
#[derive(serde::Deserialize)]
#[allow(dead_code)] // Fields are validated by deserialization, not read directly.
struct SignalCountsDto {
    blue: u32,
    yellow: u32,
    red: u32,
}

/// Frontmatter structure for YAML deserialization.
///
/// Required fields (`status`, `version`) are non-optional.
/// Optional fields (`signals`) use `Option` and are validated when present.
/// Unknown fields are silently ignored via `#[serde(flatten)]`.
#[derive(serde::Deserialize)]
#[allow(dead_code)] // Fields are validated by deserialization, not read directly.
struct SpecFrontmatterDto {
    status: String,
    version: serde_yaml::Value, // Accept both string "1.0" and number 1.0
    #[serde(default)]
    signals: Option<SignalCountsDto>,
    #[serde(flatten)]
    _extra: serde_yaml::Mapping, // Ignore unknown fields
}

/// Verifies spec.md has YAML frontmatter with `status` and `version`.
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

    // Parse the entire frontmatter as a typed DTO.
    // This validates required fields (status, version) and optional fields (signals)
    // in one pass via serde deserialization.
    match serde_yaml::from_str::<SpecFrontmatterDto>(&fm.frontmatter) {
        Ok(dto) => {
            // Post-validation: if signals key is present but null/empty, reject.
            if dto.signals.is_none() && fm.frontmatter.lines().any(|l| l.starts_with("signals:")) {
                return VerifyOutcome::from_findings(vec![Finding::error(format!(
                    "{}: 'signals' field is present but empty/null; \
                     provide a mapping like `signals: {{ blue: 0, yellow: 0, red: 0 }}`",
                    spec_path.display()
                ))]);
            }
            VerifyOutcome::pass()
        }
        Err(e) => {
            // Provide user-friendly error messages for common cases.
            let err_str = e.to_string();
            let mut findings = Vec::new();

            if err_str.contains("missing field `status`")
                || err_str.contains("missing field `version`")
            {
                // Check which required fields are missing for granular reporting.
                // Fall back to line-based check for individual field errors.
                let has_status = fm.frontmatter.lines().any(|l| l.starts_with("status:"));
                let has_version = fm.frontmatter.lines().any(|l| l.starts_with("version:"));
                if !has_status {
                    findings.push(Finding::error(format!(
                        "{}: YAML frontmatter missing required field 'status'",
                        spec_path.display()
                    )));
                }
                if !has_version {
                    findings.push(Finding::error(format!(
                        "{}: YAML frontmatter missing required field 'version'",
                        spec_path.display()
                    )));
                }
            } else {
                findings.push(Finding::error(format!(
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
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(&spec, "---\nstatus: draft\nversion: \"1.0\"\n---\n# Content\n").unwrap();
        let outcome = verify(&spec);
        assert!(!outcome.has_errors(), "spec with status+version and no signals must pass");
    }

    #[test]
    fn test_spec_frontmatter_passes_with_valid_signals_inline() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(
            &spec,
            "---\nstatus: draft\nversion: \"1.0\"\nsignals: { blue: 0, yellow: 0, red: 0 }\n---\n# Content\n",
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
            "---\nstatus: draft\nversion: \"1.0\"\nsignals:\n  blue: 12\n  yellow: 1\n  red: 0\n---\n# Content\n",
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
            "---\nstatus: approved\nversion: \"1.2\"\nsignals: { blue: 12, yellow: 1, red: 0 }\n---\n# Content\n",
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
        assert_eq!(outcome.error_count(), 2, "must report one error per missing required field");
    }

    #[test]
    fn test_spec_frontmatter_fails_with_malformed_signals_scalar_value() {
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
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(&spec, "---\nstatus: draft\nversion: \"1.0\"\nsignals:\n---\n# Content\n")
            .unwrap();
        let outcome = verify(&spec);
        assert!(outcome.has_errors(), "signals with empty value must produce an error");
    }

    #[test]
    fn test_spec_frontmatter_fails_with_signals_mismatched_braces() {
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
    fn test_spec_frontmatter_fails_with_negative_signal_count() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(
            &spec,
            "---\nstatus: draft\nversion: \"1.0\"\nsignals: { blue: -1, yellow: 0, red: 0 }\n---\n# Content\n",
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
            "---\nstatus: draft\nversion: \"1.0\"\nsignals: { blue: 0, yellow: 0 }\n---\n# Content\n",
        )
        .unwrap();
        let outcome = verify(&spec);
        assert!(outcome.has_errors(), "signals missing 'red' field must be rejected");
    }
}
