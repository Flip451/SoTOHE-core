//! Verify that spec.md requirement lines have [source: ...] attribution.

use super::frontmatter::parse_yaml_frontmatter;
use domain::verify::{Finding, VerifyOutcome};
use std::path::Path;

/// Checks if a line contains a valid `[source: <non-empty>]` tag.
/// Scans all `[source: ...]` occurrences so that an empty tag followed by a
/// valid one (e.g., `[source: ] [source: PRD]`) still passes.
fn has_valid_source_tag(line: &str) -> bool {
    let mut remaining = line;
    while let Some(start) = remaining.find("[source:") {
        let after = &remaining[start + "[source:".len()..];
        if let Some(end) = after.find(']') {
            if !after[..end].trim().is_empty() {
                return true;
            }
            // Empty tag — keep scanning after the closing bracket
            remaining = &after[end + 1..];
        } else {
            // No closing bracket — nothing more to scan
            break;
        }
    }
    false
}

/// Verifies spec.md requirement attribution.
///
/// Requirement lines are markdown headings starting with `### S-` or `### REQ-`.
/// Each must contain a `[source: ...]` tag somewhere on the same line.
/// Non-requirement headings and other content are exempt.
/// A spec.md with zero requirement lines passes.
///
/// # Errors
///
/// Returns findings when the file cannot be read, or when requirement lines
/// are missing `[source: ...]` tags.
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

    let mut findings = Vec::new();
    // (fence_char, min_count) — closing fence must use same char, at least as many
    let mut fence: Option<(char, usize)> = None;

    // Skip YAML frontmatter using shared parser.
    // If no valid frontmatter is found, scan all lines.
    let lines_vec: Vec<(usize, &str)> = content.lines().enumerate().collect();
    let body_start = parse_yaml_frontmatter(&content).map(|fm| fm.body_start).unwrap_or(0);

    for &(line_num, line) in lines_vec.get(body_start..).unwrap_or_default() {
        let trimmed = line.trim();

        // Track fenced code blocks (``` or ~~~, 3+ chars)
        if let Some((fc, fc_len)) = fence {
            // Inside a code block — check for closing fence (same char, >= length, nothing else)
            let run = trimmed.len() - trimmed.trim_start_matches(fc).len();
            if run >= fc_len && trimmed.chars().all(|c| c == fc) {
                fence = None;
            }
            continue;
        }
        // Check for opening fence
        let backtick_count = trimmed.len() - trimmed.trim_start_matches('`').len();
        let tilde_count = trimmed.len() - trimmed.trim_start_matches('~').len();
        if backtick_count >= 3 {
            fence = Some(('`', backtick_count));
            continue;
        }
        if tilde_count >= 3 {
            fence = Some(('~', tilde_count));
            continue;
        }

        let is_requirement = trimmed.starts_with("### S-") || trimmed.starts_with("### REQ-");
        if !is_requirement {
            continue;
        }
        if !has_valid_source_tag(line) {
            findings.push(Finding::error(format!(
                "{}:{}: requirement line missing [source: ...] tag: {}",
                spec_path.display(),
                line_num + 1,
                trimmed
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
    fn test_spec_attribution_fails_for_s_prefix_without_source() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(&spec, "---\nstatus: draft\nversion: 1.0\n---\n### S-AUTH-01\n").unwrap();
        let outcome = verify(&spec);
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_spec_attribution_fails_for_req_prefix_without_source() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(&spec, "---\nstatus: draft\n---\n### REQ-DATA-01\n").unwrap();
        let outcome = verify(&spec);
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_spec_attribution_passes_for_s_prefix_with_source() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(&spec, "### S-AUTH-01 [source: PRD]\n").unwrap();
        let outcome = verify(&spec);
        assert!(!outcome.has_errors());
    }

    #[test]
    fn test_spec_attribution_passes_for_req_prefix_with_source() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(&spec, "### REQ-DATA-01 [source: user-interview]\n").unwrap();
        let outcome = verify(&spec);
        assert!(!outcome.has_errors());
    }

    #[test]
    fn test_spec_attribution_passes_with_no_requirement_lines() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(&spec, "## Scope\n## Constraints\n- bullet item\n").unwrap();
        let outcome = verify(&spec);
        assert!(!outcome.has_errors());
    }

    #[test]
    fn test_spec_attribution_passes_for_non_requirement_lines_without_source() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(
            &spec,
            "## Scope\n## Constraints\n- bullet item without source\n### Non-S-heading\n",
        )
        .unwrap();
        let outcome = verify(&spec);
        assert!(!outcome.has_errors());
    }

    #[test]
    fn test_spec_attribution_fails_for_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("nonexistent.md");
        let outcome = verify(&spec);
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_spec_attribution_reports_multiple_violations() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(&spec, "### S-AUTH-01\n### REQ-DATA-01\n### S-AUTH-02 [source: PRD]\n")
            .unwrap();
        let outcome = verify(&spec);
        assert!(outcome.has_errors());
        assert_eq!(outcome.error_count(), 2);
    }

    #[test]
    fn test_spec_attribution_rejects_source_without_closing_bracket() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(&spec, "### S-AUTH-01 [source: PRD\n").unwrap();
        let outcome = verify(&spec);
        assert!(outcome.has_errors(), "[source: without closing ] must fail");
    }

    #[test]
    fn test_spec_attribution_rejects_empty_source_content() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(&spec, "### S-AUTH-01 [source: ]\n").unwrap();
        let outcome = verify(&spec);
        assert!(outcome.has_errors(), "[source: ] with empty content must fail");
    }

    #[test]
    fn test_spec_attribution_passes_empty_tag_followed_by_valid_tag() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(&spec, "### S-AUTH-01 [source: ] [source: PRD]\n").unwrap();
        let outcome = verify(&spec);
        assert!(
            !outcome.has_errors(),
            "empty [source: ] followed by valid [source: PRD] must pass"
        );
    }

    #[test]
    fn test_spec_attribution_skips_requirement_headings_inside_code_blocks() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(&spec, "## Examples\n```markdown\n### S-AUTH-01 Example heading\n```\n")
            .unwrap();
        let outcome = verify(&spec);
        assert!(
            !outcome.has_errors(),
            "requirement headings inside fenced code blocks must be exempt"
        );
    }

    #[test]
    fn test_spec_attribution_skips_yaml_frontmatter() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        // Frontmatter contains a line that looks like a requirement heading
        std::fs::write(
            &spec,
            "---\nstatus: draft\ntitle: |\n  ### S-EXAMPLE heading inside frontmatter\n---\n### S-AUTH-01 [source: PRD]\n",
        )
        .unwrap();
        let outcome = verify(&spec);
        assert!(
            !outcome.has_errors(),
            "requirement-like lines inside YAML frontmatter must be skipped"
        );
    }

    #[test]
    fn test_spec_attribution_unclosed_frontmatter_still_scans() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        // No closing --- → frontmatter is invalid → all lines are scanned
        std::fs::write(&spec, "---\nstatus: draft\n### S-AUTH-01\n").unwrap();
        let outcome = verify(&spec);
        assert!(outcome.has_errors(), "unclosed frontmatter must not swallow requirement lines");
    }

    #[test]
    fn test_spec_attribution_skips_headings_inside_tilde_fenced_code_blocks() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(&spec, "## Examples\n~~~\n### S-AUTH-01 Example\n~~~\n").unwrap();
        let outcome = verify(&spec);
        assert!(
            !outcome.has_errors(),
            "requirement headings inside ~~~ fenced code blocks must be exempt"
        );
    }
}
