//! Verify that spec.md has valid YAML frontmatter with required fields.

use super::frontmatter::parse_yaml_frontmatter;
use domain::verify::{Finding, VerifyOutcome};
use std::path::Path;

/// Required frontmatter fields.
const REQUIRED_FIELDS: &[&str] = &["status", "version"];

/// Verifies spec.md has YAML frontmatter with `status` and `version`.
///
/// # Errors
///
/// Returns findings when the file cannot be read, when YAML frontmatter
/// delimiters are missing, or when required fields are absent.
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

    // Check for required fields (simple line-based check)
    let mut findings = Vec::new();
    for field in REQUIRED_FIELDS {
        let pattern = format!("{field}:");
        if !fm.frontmatter.lines().any(|line| line.starts_with(&pattern)) {
            findings.push(Finding::error(format!(
                "{}: YAML frontmatter missing required field '{field}'",
                spec_path.display()
            )));
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
}
