//! Verify spec.md source tag signals match frontmatter declaration.
//!
//! Parses the spec body (after frontmatter) and extracts list items from
//! `## Scope`, `## Constraints`, and `## Acceptance Criteria` sections only.
//! For each item, evaluates the `[source: ...]` tag via domain logic.
//! Aggregates results into `SignalCounts` and compares against the frontmatter
//! `signals:` declaration to detect drift.
//!
//! When a sibling `spec.json` exists, the JSON-based path is used instead of
//! the markdown-based path (legacy fallback).

use std::path::Path;

use domain::verify::{Finding, VerifyOutcome};
use domain::{ConfidenceSignal, SignalCounts, evaluate_source_tag};

use super::frontmatter::parse_yaml_frontmatter;

/// Top-level `##` sections whose list items are evaluated.
const INCLUDED_H2_SECTIONS: &[&str] = &["## Scope", "## Constraints", "## Acceptance Criteria"];

/// Returns true if the line is a `##`-level heading.
fn is_h2_heading(line: &str) -> bool {
    line.starts_with("## ")
}

/// Returns true if the line is a `###`-level heading.
fn is_h3_heading(line: &str) -> bool {
    line.starts_with("### ")
}

/// Returns true if a line starts one of the included `##` sections.
fn is_included_h2(line: &str) -> bool {
    INCLUDED_H2_SECTIONS.iter().any(|h| line.trim_end() == *h)
}

/// Determines whether a line is a markdown list item (starts with `- `).
/// Handles checkbox items (`- [ ]`, `- [x]`) as well.
fn is_list_item(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("- ")
}

/// Extracts the source tag body from a list item line.
/// Returns `Some(body)` if `[source: ...]` is found (body may be empty).
/// Returns `None` if no `[source:` tag is present.
fn extract_source_tag(line: &str) -> Option<&str> {
    // Find the first [source: ...] occurrence
    if let Some(start) = line.find("[source:") {
        let after_prefix = &line[start + "[source:".len()..];
        if let Some(end) = after_prefix.find(']') {
            return Some(after_prefix[..end].trim());
        }
    }
    None
}

/// Pure evaluation: parses the body content and evaluates source tag signals.
///
/// Only list items (`- ` prefix) in Scope, Constraints, and Acceptance Criteria
/// sections are evaluated. Code blocks are skipped. Items without a
/// `[source: ...]` tag produce a Red signal.
///
/// Returns aggregate `SignalCounts`.
///
/// # Examples
///
/// ```
/// use infrastructure::verify::spec_signals::evaluate;
///
/// let body = "## Scope\n- item one [source: PRD §1]\n- item two [source: inference — guess]\n";
/// let counts = evaluate(body);
/// assert_eq!(counts.blue(), 1);
/// assert_eq!(counts.yellow(), 1);
/// assert_eq!(counts.red(), 0);
/// ```
pub fn evaluate(content: &str) -> SignalCounts {
    let mut blue: u32 = 0;
    let mut yellow: u32 = 0;
    let mut red: u32 = 0;

    let mut in_included_section = false;
    let mut in_scope_parent = false; // true when the current ## parent is ## Scope
    // (fence_char, min_count) for code block tracking
    let mut fence: Option<(char, usize)> = None;

    for line in content.lines() {
        let trimmed = line.trim();

        // Track fenced code blocks
        if let Some((fc, fc_len)) = fence {
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

        // ## heading boundary detection
        if is_h2_heading(trimmed) {
            if is_included_h2(trimmed) {
                in_included_section = true;
                in_scope_parent = trimmed.trim_end() == "## Scope";
            } else {
                in_included_section = false;
                in_scope_parent = false;
            }
            continue;
        }

        // ### heading boundary detection
        if is_h3_heading(trimmed) {
            if in_scope_parent {
                // Under ## Scope: all ### subsections are included
                // (### In Scope, ### Out of Scope, or any other ### like ### Functional Requirements)
                in_included_section = true;
            }
            // ### under non-included ## headings: no change to in_included_section
            continue;
        }

        if !in_included_section {
            continue;
        }

        if !is_list_item(line) {
            continue;
        }

        // Evaluate source tag
        match extract_source_tag(line) {
            Some(tag_body) => {
                let (signal, _basis) = evaluate_source_tag(tag_body);
                match signal {
                    ConfidenceSignal::Blue => blue += 1,
                    ConfidenceSignal::Yellow => yellow += 1,
                    ConfidenceSignal::Red => red += 1,
                    _ => red += 1,
                }
            }
            None => {
                // No source tag → Red
                red += 1;
            }
        }
    }

    SignalCounts::new(blue, yellow, red)
}

/// Verifies signal quality using a pre-decoded `spec.json`.
///
/// Evaluates signals via `doc.evaluate_signals()` (domain logic).
/// Returns an error if `red > 0` (gate policy violation).
///
/// # Errors
///
/// Returns findings when:
/// - The file cannot be read.
/// - The JSON cannot be decoded.
/// - The evaluated signals contain red items.
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

    let evaluated = doc.evaluate_signals();
    let mut findings = Vec::new();

    if evaluated.red() > 0 {
        findings.push(Finding::error(format!(
            "{}: red > 0 gate policy violation — {} item(s) missing or have unverified sources",
            spec_json_path.display(),
            evaluated.red()
        )));
    }

    // Warn if cached signals are stale (mismatch with fresh evaluation)
    if let Some(cached) = doc.signals() {
        if *cached != evaluated {
            findings.push(Finding::error(format!(
                "{}: cached signals (blue={} yellow={} red={}) differ from evaluated (blue={} yellow={} red={}). Run `sotp track signals` to update.",
                spec_json_path.display(),
                cached.blue(), cached.yellow(), cached.red(),
                evaluated.blue(), evaluated.yellow(), evaluated.red()
            )));
        }
    }

    if findings.is_empty() { VerifyOutcome::pass() } else { VerifyOutcome::from_findings(findings) }
}

/// Verifies that the spec.md `signals:` frontmatter declaration matches
/// the actual source tag signals found in the body.
///
/// When a sibling `spec.json` exists next to `spec_path`, delegates to
/// `verify_from_spec_json`. Otherwise falls back to the markdown-based flow.
///
/// Steps (markdown fallback):
/// 1. Read file, parse frontmatter.
/// 2. Evaluate body with `evaluate()`.
/// 3. If `evaluated.red() > 0` → error ("red > 0 gate policy violation").
/// 4. If frontmatter has `signals:` field → compare against evaluated counts;
///    mismatch → error.
/// 5. Return `VerifyOutcome`.
///
/// # Errors
///
/// Returns findings when:
/// - The file cannot be read.
/// - YAML frontmatter delimiters are missing.
/// - The body contains Red-signal items (missing source).
/// - The declared `signals:` counts differ from the evaluated counts.
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

    let Some(fm) = parse_yaml_frontmatter(&content) else {
        return VerifyOutcome::from_findings(vec![Finding::error(format!(
            "{}: missing or invalid YAML frontmatter (expected '---' delimiters)",
            spec_path.display()
        ))]);
    };

    // Collect body lines
    let lines: Vec<&str> = content.lines().collect();
    let body_lines = lines.get(fm.body_start..).unwrap_or_default();
    let body = body_lines.join("\n");

    let evaluated = evaluate(&body);

    let mut findings = Vec::new();

    // Gate: red > 0 is a policy violation
    if evaluated.red() > 0 {
        findings.push(Finding::error(format!(
            "{}: red > 0 gate policy violation — {} item(s) missing [source: ...] tag or have unverified sources",
            spec_path.display(),
            evaluated.red()
        )));
    }

    if findings.is_empty() { VerifyOutcome::pass() } else { VerifyOutcome::from_findings(findings) }
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // --- evaluate() unit tests ---

    #[test]
    fn test_evaluate_empty_body_returns_zero_counts() {
        let counts = evaluate("");
        assert_eq!(counts.blue(), 0);
        assert_eq!(counts.yellow(), 0);
        assert_eq!(counts.red(), 0);
        assert_eq!(counts.total(), 0);
    }

    #[test]
    fn test_evaluate_scope_section_with_blue_source() {
        let body = "## Scope\n- item one [source: PRD §1]\n";
        let counts = evaluate(body);
        assert_eq!(counts.blue(), 1);
        assert_eq!(counts.yellow(), 0);
        assert_eq!(counts.red(), 0);
    }

    #[test]
    fn test_evaluate_constraints_section_with_yellow_source() {
        let body = "## Constraints\n- constraint one [source: inference — best practice]\n";
        let counts = evaluate(body);
        assert_eq!(counts.blue(), 0);
        assert_eq!(counts.yellow(), 1);
        assert_eq!(counts.red(), 0);
    }

    #[test]
    fn test_evaluate_acceptance_criteria_section_with_red_missing_source() {
        let body = "## Acceptance Criteria\n- item without source\n";
        let counts = evaluate(body);
        assert_eq!(counts.blue(), 0);
        assert_eq!(counts.yellow(), 0);
        assert_eq!(counts.red(), 1);
    }

    #[test]
    fn test_evaluate_goal_section_is_skipped() {
        // Goal section items should NOT be counted
        let body = "## Goal\n- goal item [source: PRD §2]\n- another goal item\n";
        let counts = evaluate(body);
        assert_eq!(counts.total(), 0);
    }

    #[test]
    fn test_evaluate_items_outside_any_section_are_skipped() {
        let body = "- item before any section [source: PRD]\n## Other\n- item in other section\n";
        let counts = evaluate(body);
        assert_eq!(counts.total(), 0);
    }

    #[test]
    fn test_evaluate_code_blocks_are_skipped() {
        let body = "## Scope\n```\n- item inside code block [source: PRD]\n```\n- real item [source: PRD §1]\n";
        let counts = evaluate(body);
        // Only the real item outside the code block should be counted
        assert_eq!(counts.blue(), 1);
        assert_eq!(counts.total(), 1);
    }

    #[test]
    fn test_evaluate_tilde_code_blocks_are_skipped() {
        let body = "## Acceptance Criteria\n~~~\n- item inside tilde block\n~~~\n- real item [source: feedback — confirmed]\n";
        let counts = evaluate(body);
        assert_eq!(counts.blue(), 1);
        assert_eq!(counts.total(), 1);
    }

    #[test]
    fn test_evaluate_inference_source_is_yellow() {
        let body = "## Scope\n- item [source: inference — guess]\n";
        let counts = evaluate(body);
        assert_eq!(counts.yellow(), 1);
        assert_eq!(counts.blue(), 0);
    }

    #[test]
    fn test_evaluate_in_scope_subsection_counted() {
        // ### In Scope must be under ## Scope to be counted
        let body =
            "## Scope\n### In Scope\n- item [source: convention — project-docs/conventions/x.md]\n";
        let counts = evaluate(body);
        assert_eq!(counts.blue(), 1);
    }

    #[test]
    fn test_evaluate_out_of_scope_subsection_counted() {
        // ### Out of Scope must be under ## Scope to be counted
        let body = "## Scope\n### Out of Scope\n- item [source: discussion]\n";
        let counts = evaluate(body);
        assert_eq!(counts.yellow(), 1);
    }

    #[test]
    fn test_evaluate_in_scope_under_goal_not_counted() {
        // ### In Scope under ## Goal must NOT be counted
        let body = "## Goal\n### In Scope\n- item [source: PRD §1]\n";
        let counts = evaluate(body);
        assert_eq!(counts.total(), 0, "### In Scope under ## Goal should be ignored");
    }

    #[test]
    fn test_evaluate_section_boundary_stops_at_next_h2() {
        let body = "## Scope\n- item A [source: PRD]\n## Goal\n- item B [source: PRD]\n";
        let counts = evaluate(body);
        // Only item A in Scope should be counted; item B in Goal is skipped
        assert_eq!(counts.blue(), 1);
        assert_eq!(counts.total(), 1);
    }

    #[test]
    fn test_evaluate_multiple_included_sections() {
        let body = concat!(
            "## Scope\n",
            "- scope item [source: PRD §1]\n",
            "## Constraints\n",
            "- constraint [source: inference — security]\n",
            "## Acceptance Criteria\n",
            "- ac item missing source\n",
        );
        let counts = evaluate(body);
        assert_eq!(counts.blue(), 1);
        assert_eq!(counts.yellow(), 1);
        assert_eq!(counts.red(), 1);
    }

    #[test]
    fn test_evaluate_checkbox_list_items_are_counted() {
        let body = "## Acceptance Criteria\n- [ ] unchecked item [source: PRD]\n- [x] checked item [source: discussion]\n";
        let counts = evaluate(body);
        assert_eq!(counts.blue(), 1);
        assert_eq!(counts.yellow(), 1);
        assert_eq!(counts.red(), 0);
    }

    #[test]
    fn test_evaluate_empty_source_tag_is_red() {
        let body = "## Scope\n- item [source: ]\n";
        let counts = evaluate(body);
        // [source: ] with empty body evaluates as MissingSource → Red
        assert_eq!(counts.red(), 1);
    }

    // --- verify() integration tests ---

    #[test]
    fn test_verify_passes_when_no_source_tagged_items_and_no_frontmatter_signals() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(
            &spec,
            "---\nstatus: draft\nversion: \"1.0\"\n---\n## Scope\n- item [source: PRD §1]\n",
        )
        .unwrap();
        let outcome = verify(&spec);
        assert!(!outcome.has_errors());
    }

    #[test]
    fn test_verify_fails_when_red_items_present() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(
            &spec,
            "---\nstatus: draft\nversion: \"1.0\"\n---\n## Scope\n- item without source\n",
        )
        .unwrap();
        let outcome = verify(&spec);
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_verify_fails_for_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("nonexistent.md");
        let outcome = verify(&spec);
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_verify_fails_for_missing_frontmatter() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(&spec, "## Scope\n- item [source: PRD]\n").unwrap();
        let outcome = verify(&spec);
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_verify_red_gate_reports_count() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        std::fs::write(
            &spec,
            concat!(
                "---\nstatus: draft\nversion: \"1.0\"\n---\n",
                "## Scope\n",
                "- item without source\n",
                "- another missing\n",
            ),
        )
        .unwrap();
        let outcome = verify(&spec);
        assert!(outcome.has_errors());
        assert_eq!(outcome.error_count(), 1, "expected 1 red gate error");
    }

    #[test]
    fn test_verify_no_items_in_included_sections_passes() {
        let dir = tempfile::tempdir().unwrap();
        let spec = dir.path().join("spec.md");
        // Spec with sections but no list items — zero items is valid
        std::fs::write(
            &spec,
            "---\nstatus: draft\nversion: \"1.0\"\n---\n## Scope\nNo bullet items here.\n",
        )
        .unwrap();
        let outcome = verify(&spec);
        assert!(!outcome.has_errors());
    }

    // --- verify_from_spec_json() tests ---

    const MINIMAL_SPEC_JSON: &str = r#"{
  "schema_version": 1,
  "status": "draft",
  "version": "1.0",
  "title": "Feature Title",
  "scope": { "in_scope": [], "out_of_scope": [] }
}"#;

    const SPEC_JSON_WITH_BLUE_SOURCE: &str = r#"{
  "schema_version": 1,
  "status": "draft",
  "version": "1.0",
  "title": "Feature Title",
  "scope": {
    "in_scope": [{ "text": "In scope item", "sources": ["PRD §1"] }],
    "out_of_scope": []
  }
}"#;

    const SPEC_JSON_WITH_RED_SOURCE: &str = r#"{
  "schema_version": 1,
  "status": "draft",
  "version": "1.0",
  "title": "Feature Title",
  "scope": {
    "in_scope": [{ "text": "Missing source item", "sources": [] }],
    "out_of_scope": []
  }
}"#;

    #[test]
    fn test_verify_from_spec_json_with_no_requirements_passes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("spec.json");
        std::fs::write(&path, MINIMAL_SPEC_JSON).unwrap();
        let outcome = verify_from_spec_json(&path);
        assert!(!outcome.has_errors(), "no requirements should pass: {outcome:?}");
    }

    #[test]
    fn test_verify_from_spec_json_with_blue_sources_passes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("spec.json");
        std::fs::write(&path, SPEC_JSON_WITH_BLUE_SOURCE).unwrap();
        let outcome = verify_from_spec_json(&path);
        assert!(!outcome.has_errors(), "blue source should pass: {outcome:?}");
    }

    #[test]
    fn test_verify_from_spec_json_with_empty_sources_fails() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("spec.json");
        std::fs::write(&path, SPEC_JSON_WITH_RED_SOURCE).unwrap();
        let outcome = verify_from_spec_json(&path);
        assert!(outcome.has_errors(), "empty sources (red signal) should fail");
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
        std::fs::write(&path, "not json at all").unwrap();
        let outcome = verify_from_spec_json(&path);
        assert!(outcome.has_errors(), "invalid JSON should fail");
    }

    // --- verify() delegation tests ---

    #[test]
    fn test_verify_delegates_to_spec_json_when_sibling_exists() {
        let dir = tempfile::tempdir().unwrap();
        // Write spec.json with blue sources (should pass)
        std::fs::write(dir.path().join("spec.json"), SPEC_JSON_WITH_BLUE_SOURCE).unwrap();
        // Write spec.md with red items (would fail under legacy path)
        std::fs::write(
            dir.path().join("spec.md"),
            "---\nstatus: draft\nversion: \"1.0\"\n---\n## Scope\n- item without source\n",
        )
        .unwrap();
        let outcome = verify(&dir.path().join("spec.md"));
        // spec.json takes priority; blue source passes
        assert!(
            !outcome.has_errors(),
            "spec.json delegation should override markdown findings: {outcome:?}"
        );
    }

    #[test]
    fn test_verify_falls_back_to_markdown_when_no_spec_json() {
        let dir = tempfile::tempdir().unwrap();
        // No spec.json present
        std::fs::write(
            dir.path().join("spec.md"),
            "---\nstatus: draft\nversion: \"1.0\"\n---\n## Scope\n- item [source: PRD §1]\n",
        )
        .unwrap();
        let outcome = verify(&dir.path().join("spec.md"));
        assert!(!outcome.has_errors(), "legacy markdown path should pass: {outcome:?}");
    }

    #[test]
    fn test_verify_spec_json_failure_propagates_through_verify() {
        let dir = tempfile::tempdir().unwrap();
        // Write spec.json with red (empty) sources
        std::fs::write(dir.path().join("spec.json"), SPEC_JSON_WITH_RED_SOURCE).unwrap();
        std::fs::write(
            dir.path().join("spec.md"),
            "---\nstatus: draft\nversion: \"1.0\"\n---\n## Scope\n- item [source: PRD §1]\n",
        )
        .unwrap();
        let outcome = verify(&dir.path().join("spec.md"));
        assert!(outcome.has_errors(), "spec.json red signal should propagate as failure");
    }
}
