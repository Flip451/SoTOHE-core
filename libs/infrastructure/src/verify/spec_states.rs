//! Verify that spec.md contains a ## Domain States section with at least one table data row.
//!
//! When a sibling `spec.json` exists, delegates to the JSON-based path which
//! checks `doc.domain_states()` is non-empty. Otherwise falls back to the
//! markdown table scan (legacy path).

use std::path::Path;

use domain::verify::{Finding, VerifyOutcome};

use super::frontmatter::parse_yaml_frontmatter;

/// Verifies domain states using a pre-decoded `spec.json`.
///
/// Checks that `doc.domain_states()` is non-empty (at least one entry).
///
/// # Errors
///
/// Returns findings when:
/// - The file cannot be read.
/// - The JSON cannot be decoded.
/// - `domain_states` is empty.
pub fn verify_from_spec_json(spec_json_path: &Path) -> VerifyOutcome {
    let json = match std::fs::read_to_string(spec_json_path) {
        Ok(s) => s,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![Finding::error(format!(
                "cannot read {}: {e}",
                spec_json_path.display()
            ))]);
        }
    };

    let doc = match crate::spec::codec::decode(&json) {
        Ok(d) => d,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![Finding::error(format!(
                "{}: spec.json decode error: {e}",
                spec_json_path.display()
            ))]);
        }
    };

    if doc.domain_states().is_empty() {
        return VerifyOutcome::from_findings(vec![Finding::error(format!(
            "{}: domain_states is empty — at least one entry required",
            spec_json_path.display()
        ))]);
    }

    VerifyOutcome::pass()
}

/// Verifies that `spec.md` contains a `## Domain States` section with a markdown table
/// that has at least one data row (beyond the header and separator rows).
///
/// When a sibling `spec.json` exists next to `spec_path`, delegates to
/// `verify_from_spec_json`. Otherwise falls back to the markdown table scan.
///
/// # Errors
///
/// Returns findings when:
/// - The file cannot be read.
/// - The `## Domain States` heading is absent from the body.
/// - The section exists but contains no markdown table.
/// - The table has no data rows (header + separator only).
pub fn verify(spec_path: &Path) -> VerifyOutcome {
    // Delegate to spec.json path when a sibling spec.json exists.
    if let Some(spec_json_path) = sibling_spec_json(spec_path) {
        if spec_json_path.is_file() {
            return verify_from_spec_json(&spec_json_path);
        }
    }

    // Legacy markdown-based flow.
    let content = match std::fs::read_to_string(spec_path) {
        Ok(c) => c,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![Finding::error(format!(
                "cannot read {}: {e}",
                spec_path.display()
            ))]);
        }
    };

    let lines: Vec<&str> = content.lines().collect();

    // Determine where the body starts (skip YAML frontmatter if present).
    let body_start = match parse_yaml_frontmatter(&content) {
        Some(fm) => fm.body_start,
        None => 0,
    };

    let body_lines = lines.get(body_start..).unwrap_or_default();

    // Locate the `## Domain States` heading, skipping fenced code blocks.
    let mut section_start: Option<usize> = None;
    let mut heading_fence: Option<(char, usize)> = None;
    for (i, line) in body_lines.iter().enumerate() {
        let trimmed = line.trim();
        // Track fenced code blocks
        if let Some((fc, fc_len)) = heading_fence {
            let run = trimmed.len() - trimmed.trim_start_matches(fc).len();
            if run >= fc_len && trimmed.chars().all(|c| c == fc) {
                heading_fence = None;
            }
            continue;
        }
        let backtick_count = trimmed.len() - trimmed.trim_start_matches('`').len();
        let tilde_count = trimmed.len() - trimmed.trim_start_matches('~').len();
        if backtick_count >= 3 {
            heading_fence = Some(('`', backtick_count));
            continue;
        }
        if tilde_count >= 3 {
            heading_fence = Some(('~', tilde_count));
            continue;
        }
        if line.trim_end() == "## Domain States" {
            section_start = Some(i);
            break;
        }
    }

    let Some(section_idx) = section_start else {
        return VerifyOutcome::from_findings(vec![Finding::error(format!(
            "{}: missing '## Domain States' section",
            spec_path.display()
        ))]);
    };

    // Collect table lines from the section body, skipping fenced code blocks
    // and stopping at the next ## or # heading (outside a fence).
    let mut table_lines: Vec<&str> = Vec::new();
    let mut body_fence: Option<(char, usize)> = None;
    for line in body_lines.iter().skip(section_idx + 1) {
        let trimmed = line.trim();

        // Track fenced code blocks
        if let Some((fc, fc_len)) = body_fence {
            let run = trimmed.len() - trimmed.trim_start_matches(fc).len();
            if run >= fc_len && trimmed.chars().all(|c| c == fc) {
                body_fence = None;
            }
            continue;
        }
        let backtick_count = trimmed.len() - trimmed.trim_start_matches('`').len();
        let tilde_count = trimmed.len() - trimmed.trim_start_matches('~').len();
        if backtick_count >= 3 {
            body_fence = Some(('`', backtick_count));
            continue;
        }
        if tilde_count >= 3 {
            body_fence = Some(('~', tilde_count));
            continue;
        }

        // Stop at the next same-or-higher-level heading (## or #) outside fences.
        let t = line.trim_end();
        if t.starts_with("## ") || t == "##" || t.starts_with("# ") || t == "#" {
            break;
        }

        if line.trim_start().starts_with('|') {
            table_lines.push(line);
        }
    }

    if table_lines.is_empty() {
        return VerifyOutcome::from_findings(vec![Finding::error(format!(
            "{}: '## Domain States' section has no markdown table",
            spec_path.display()
        ))]);
    }

    // A valid table needs header row, separator row (`|---|`), and at least one data row.
    // We detect the separator row as a `|`-prefixed line containing only `-`, `|`, ` `, `:`.
    let sep_idx = table_lines.iter().position(|l| is_table_separator(l));

    let Some(sep_pos) = sep_idx else {
        return VerifyOutcome::from_findings(vec![Finding::error(format!(
            "{}: '## Domain States' table has no separator row (header-only table)",
            spec_path.display()
        ))]);
    };

    // Separator must be preceded by at least one header row.
    if sep_pos == 0 {
        return VerifyOutcome::from_findings(vec![Finding::error(format!(
            "{}: '## Domain States' table has no header row before separator",
            spec_path.display()
        ))]);
    }

    // Data rows come after the separator (excluding additional separator rows).
    let data_rows: Vec<&str> =
        table_lines.iter().skip(sep_pos + 1).copied().filter(|l| !is_table_separator(l)).collect();

    if data_rows.is_empty() {
        return VerifyOutcome::from_findings(vec![Finding::error(format!(
            "{}: '## Domain States' table has no data rows (header + separator only)",
            spec_path.display()
        ))]);
    }

    VerifyOutcome::pass()
}

/// Derives the sibling `spec.json` path from a `spec.md` path by replacing
/// the filename component.
///
/// Returns `None` when the path has no parent directory.
fn sibling_spec_json(spec_md_path: &Path) -> Option<std::path::PathBuf> {
    spec_md_path
        .parent()
        .map(|dir| if dir.as_os_str().is_empty() { Path::new(".") } else { dir })
        .map(|dir| dir.join("spec.json"))
}

/// Returns `true` when `line` is a markdown table separator row.
///
/// A separator row consists solely of `|`, `-`, `:`, and space characters
/// and starts with `|`.
fn is_table_separator(line: &str) -> bool {
    let trimmed = line.trim();
    if !trimmed.starts_with('|') {
        return false;
    }
    // Must contain at least one `-` to distinguish from a header row.
    if !trimmed.contains('-') {
        return false;
    }
    // All characters must be `|`, `-`, `:`, or space.
    trimmed.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // Helper: write content to a temp spec.md and return its path.
    fn make_spec(content: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("spec.md");
        std::fs::write(&path, content).unwrap();
        (dir, path)
    }

    // --- 1. No Domain States section ---

    #[test]
    fn test_spec_states_with_no_domain_states_section_returns_error() {
        let (_dir, path) =
            make_spec("---\nstatus: draft\nversion: \"1.0\"\n---\n# Overview\n\nSome content.\n");
        let outcome = verify(&path);
        assert!(outcome.has_errors(), "missing ## Domain States must be an error");
    }

    // --- 2. Valid table (header + separator + at least one data row) ---

    #[test]
    fn test_spec_states_with_valid_table_passes() {
        let (_dir, path) = make_spec(
            "---\nstatus: draft\nversion: \"1.0\"\n---\n# Overview\n\n## Domain States\n\n\
             | State | Description |\n\
             |-------|-------------|\n\
             | Draft | Initial state |\n",
        );
        let outcome = verify(&path);
        assert!(!outcome.has_errors(), "valid table must pass");
    }

    #[test]
    fn test_spec_states_with_multiple_data_rows_passes() {
        let (_dir, path) = make_spec(
            "## Domain States\n\n\
             | State | Description |\n\
             |-------|-------------|\n\
             | Draft | Initial state |\n\
             | Active | Active state |\n\
             | Done | Terminal state |\n",
        );
        let outcome = verify(&path);
        assert!(!outcome.has_errors(), "table with multiple data rows must pass");
    }

    // --- 3. Header only (no separator) ---

    #[test]
    fn test_spec_states_with_header_only_no_separator_returns_error() {
        let (_dir, path) = make_spec(
            "## Domain States\n\n\
             | State | Description |\n",
        );
        let outcome = verify(&path);
        assert!(outcome.has_errors(), "header-only table (no separator) must be an error");
    }

    // --- 4. Header + separator only (no data rows) ---

    #[test]
    fn test_spec_states_with_header_and_separator_only_returns_error() {
        let (_dir, path) = make_spec(
            "## Domain States\n\n\
             | State | Description |\n\
             |-------|-------------|\n",
        );
        let outcome = verify(&path);
        assert!(outcome.has_errors(), "header + separator with no data rows must be an error");
    }

    // --- 5. Section exists with empty body ---

    #[test]
    fn test_spec_states_with_empty_section_body_returns_error() {
        let (_dir, path) = make_spec("## Domain States\n");
        let outcome = verify(&path);
        assert!(outcome.has_errors(), "empty section body must be an error");
    }

    // --- 6. Section exists with non-table content ---

    #[test]
    fn test_spec_states_with_non_table_content_returns_error() {
        let (_dir, path) = make_spec(
            "## Domain States\n\n\
             This section describes domain states but has no table.\n",
        );
        let outcome = verify(&path);
        assert!(outcome.has_errors(), "section with non-table content must be an error");
    }

    // --- 7. Heading level disambiguation: ### does not match ---

    #[test]
    fn test_spec_states_with_only_h3_heading_does_not_match() {
        let (_dir, path) = make_spec(
            "### Domain States\n\n\
             | State | Description |\n\
             |-------|-------------|\n\
             | Draft | Initial state |\n",
        );
        let outcome = verify(&path);
        assert!(
            outcome.has_errors(),
            "### Domain States must not satisfy the ## Domain States requirement"
        );
    }

    #[test]
    fn test_spec_states_with_h1_heading_does_not_match() {
        let (_dir, path) = make_spec(
            "# Domain States\n\n\
             | State | Description |\n\
             |-------|-------------|\n\
             | Draft | Initial state |\n",
        );
        let outcome = verify(&path);
        assert!(
            outcome.has_errors(),
            "# Domain States must not satisfy the ## Domain States requirement"
        );
    }

    // --- 8. File read error ---

    #[test]
    fn test_spec_states_with_nonexistent_file_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.md");
        let outcome = verify(&path);
        assert!(outcome.has_errors(), "unreadable file must return an error");
    }

    // --- Additional edge cases ---

    #[test]
    fn test_spec_states_section_without_frontmatter_passes() {
        // No frontmatter — body starts at line 0.
        let (_dir, path) = make_spec(
            "# Title\n\n\
             ## Domain States\n\n\
             | State | Desc |\n\
             |-------|------|\n\
             | Ready | ok   |\n",
        );
        let outcome = verify(&path);
        assert!(!outcome.has_errors(), "spec without frontmatter but valid section must pass");
    }

    #[test]
    fn test_spec_states_section_with_frontmatter_passes() {
        let (_dir, path) = make_spec(
            "---\nstatus: active\nversion: \"2.0\"\n---\n\
             # Title\n\n\
             ## Domain States\n\n\
             | State | Desc |\n\
             |-------|------|\n\
             | Ready | ok   |\n",
        );
        let outcome = verify(&path);
        assert!(!outcome.has_errors(), "spec with frontmatter and valid section must pass");
    }

    #[test]
    fn test_spec_states_section_after_other_sections_passes() {
        let (_dir, path) = make_spec(
            "## Overview\n\nSome text.\n\n\
             ## Domain States\n\n\
             | State | Desc |\n\
             |-------|------|\n\
             | Ready | ok   |\n\n\
             ## Other Section\n\nMore text.\n",
        );
        let outcome = verify(&path);
        assert!(
            !outcome.has_errors(),
            "## Domain States after other sections with valid table must pass"
        );
    }

    // --- verify_from_spec_json() tests ---

    const SPEC_JSON_WITH_DOMAIN_STATES: &str = r#"{
  "schema_version": 1,
  "status": "draft",
  "version": "1.0",
  "title": "Feature",
  "scope": { "in_scope": [], "out_of_scope": [] },
  "domain_states": [{ "name": "Draft", "description": "Initial state" }]
}"#;

    const SPEC_JSON_WITHOUT_DOMAIN_STATES: &str = r#"{
  "schema_version": 1,
  "status": "draft",
  "version": "1.0",
  "title": "Feature",
  "scope": { "in_scope": [], "out_of_scope": [] }
}"#;

    #[test]
    fn test_verify_from_spec_json_with_domain_states_passes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("spec.json");
        std::fs::write(&path, SPEC_JSON_WITH_DOMAIN_STATES).unwrap();
        let outcome = verify_from_spec_json(&path);
        assert!(!outcome.has_errors(), "non-empty domain_states should pass: {outcome:?}");
    }

    #[test]
    fn test_verify_from_spec_json_with_empty_domain_states_fails() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("spec.json");
        std::fs::write(&path, SPEC_JSON_WITHOUT_DOMAIN_STATES).unwrap();
        let outcome = verify_from_spec_json(&path);
        assert!(outcome.has_errors(), "empty domain_states should fail");
    }

    #[test]
    fn test_verify_from_spec_json_with_missing_file_fails() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nonexistent.json");
        let outcome = verify_from_spec_json(&path);
        assert!(outcome.has_errors(), "missing file should fail");
    }

    #[test]
    fn test_verify_from_spec_json_with_invalid_json_fails() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("spec.json");
        std::fs::write(&path, "not valid json").unwrap();
        let outcome = verify_from_spec_json(&path);
        assert!(outcome.has_errors(), "invalid JSON should fail");
    }

    // --- verify() delegation tests ---

    #[test]
    fn test_verify_delegates_to_spec_json_when_sibling_exists() {
        let dir = tempfile::tempdir().unwrap();
        // Write spec.json with domain states (passes)
        std::fs::write(dir.path().join("spec.json"), SPEC_JSON_WITH_DOMAIN_STATES).unwrap();
        // Write spec.md without ## Domain States (would fail under legacy path)
        std::fs::write(
            dir.path().join("spec.md"),
            "---\nstatus: draft\nversion: \"1.0\"\n---\n# Overview\n\nNo domain states here.\n",
        )
        .unwrap();
        let outcome = verify(&dir.path().join("spec.md"));
        assert!(
            !outcome.has_errors(),
            "spec.json delegation should override markdown findings: {outcome:?}"
        );
    }

    #[test]
    fn test_verify_spec_json_empty_domain_states_propagates_failure() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("spec.json"), SPEC_JSON_WITHOUT_DOMAIN_STATES).unwrap();
        std::fs::write(
            dir.path().join("spec.md"),
            "---\nstatus: draft\nversion: \"1.0\"\n---\n## Domain States\n\n| State | Desc |\n|---|---|\n| Draft | ok |\n",
        )
        .unwrap();
        let outcome = verify(&dir.path().join("spec.md"));
        assert!(outcome.has_errors(), "empty domain_states in spec.json should fail");
    }

    #[test]
    fn test_verify_falls_back_to_markdown_when_no_spec_json() {
        let dir = tempfile::tempdir().unwrap();
        // No spec.json — use legacy markdown path
        std::fs::write(
            dir.path().join("spec.md"),
            "## Domain States\n\n| State | Desc |\n|-------|------|\n| Ready | ok |\n",
        )
        .unwrap();
        let outcome = verify(&dir.path().join("spec.md"));
        assert!(
            !outcome.has_errors(),
            "legacy markdown path with valid table must pass: {outcome:?}"
        );
    }
}
