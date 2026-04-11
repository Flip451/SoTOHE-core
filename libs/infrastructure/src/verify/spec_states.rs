//! Verify that spec.md contains a ## Domain States section with at least one table data row.
//!
//! When a sibling `spec.json` exists, delegates to the JSON-based path which
//! reads the sibling `domain-types.json` and verifies its entries.
//! Otherwise falls back to the markdown table scan (legacy path).

use std::path::Path;

use domain::ConfidenceSignal;
use domain::verify::{Finding, VerifyOutcome};

use crate::tddd::catalogue_codec;

use super::frontmatter::parse_yaml_frontmatter;

/// Verifies domain types using a sibling `domain-types.json` file.
///
/// The `domain-types.json` file is expected to reside in the same directory as
/// `spec.json`. Verification passes when:
/// - `domain-types.json` exists and can be decoded.
/// - The document has at least one entry.
/// - If signals are present, no signal is Red (red count = 0).
///
/// # Errors
///
/// Returns findings when:
/// - `domain-types.json` does not exist.
/// - The file cannot be read or decoded.
/// - The entries list is empty.
/// - Any signal is Red.
pub fn verify_from_spec_json(spec_json_path: &Path, strict: bool) -> VerifyOutcome {
    // Validate spec.json itself is readable and parseable
    let spec_json = match std::fs::read_to_string(spec_json_path) {
        Ok(s) => s,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![Finding::error(format!(
                "cannot read {}: {e}",
                spec_json_path.display()
            ))]);
        }
    };
    let spec_doc = match crate::spec::codec::decode(&spec_json) {
        Ok(d) => d,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![Finding::error(format!(
                "{}: spec.json decode error: {e}",
                spec_json_path.display()
            ))]);
        }
    };

    // Stage 1 prerequisite: spec signals must exist and satisfy the mode gate.
    // - Default: red == 0 (Yellow WIP allowed)
    // - Strict:  red == 0 AND yellow == 0 (all Blue required for merge)
    match spec_doc.signals() {
        None => {
            return VerifyOutcome::from_findings(vec![Finding::error(format!(
                "{}: Stage 1 prerequisite not met: spec signals not yet evaluated. Run `sotp track signals` first.",
                spec_json_path.display()
            ))]);
        }
        Some(counts) if counts.has_red() => {
            return VerifyOutcome::from_findings(vec![Finding::error(format!(
                "{}: Stage 1 prerequisite not met: spec signals have red={} (must be 0)",
                spec_json_path.display(),
                counts.red()
            ))]);
        }
        Some(counts) if strict && counts.yellow() > 0 => {
            return VerifyOutcome::from_findings(vec![Finding::error(format!(
                "{}: Stage 1 prerequisite not met in strict mode: spec signals have yellow={} (all must be Blue for merge — run /track:design)",
                spec_json_path.display(),
                counts.yellow()
            ))]);
        }
        _ => {}
    }

    let dir = match spec_json_path.parent() {
        Some(d) if !d.as_os_str().is_empty() => d,
        _ => Path::new("."),
    };
    let domain_types_path = dir.join("domain-types.json");

    if !domain_types_path.is_file() {
        return VerifyOutcome::from_findings(vec![Finding::error(format!(
            "{}: domain-types.json is missing; declare domain types to enable type verification",
            dir.display()
        ))]);
    }

    let json = match std::fs::read_to_string(&domain_types_path) {
        Ok(s) => s,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![Finding::error(format!(
                "cannot read {}: {e}",
                domain_types_path.display()
            ))]);
        }
    };

    let doc = match catalogue_codec::decode(&json) {
        Ok(d) => d,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![Finding::error(format!(
                "{}: invalid domain-types.json: {e}",
                domain_types_path.display()
            ))]);
        }
    };

    if doc.entries().is_empty() {
        return VerifyOutcome::from_findings(vec![Finding::error(format!(
            "{}: domain-types.json has no entries; add at least one domain type declaration",
            domain_types_path.display()
        ))]);
    }

    let Some(signals) = doc.signals() else {
        return VerifyOutcome::from_findings(vec![Finding::error(format!(
            "{}: domain type signals not yet evaluated; run `sotp track domain-type-signals` first",
            domain_types_path.display()
        ))]);
    };

    // Check signal coverage by name + kind: every entry must have a matching signal
    let signal_keys: std::collections::HashSet<(&str, &str)> =
        signals.iter().map(|s| (s.type_name(), s.kind_tag())).collect();
    let uncovered: Vec<&str> = doc
        .entries()
        .iter()
        .filter(|e| !signal_keys.contains(&(e.name(), e.kind().kind_tag())))
        .map(|e| e.name())
        .collect();
    if !uncovered.is_empty() {
        return VerifyOutcome::from_findings(vec![Finding::error(format!(
            "{}: {} domain type(s) have no signal evaluation: {}; re-run `sotp track domain-type-signals`",
            domain_types_path.display(),
            uncovered.len(),
            uncovered.join(", ")
        ))]);
    }

    // Two-stage gate (TDDD):
    // - Default (interim commit): Red → fail, Yellow → pass (WIP allowed)
    // - Strict (merge gate): any non-Blue → fail (Yellow also blocked)
    // Red check: ALL signals (forward + reverse undeclared) — single gate per ADR §Decision.4
    let all_red: Vec<&str> = signals
        .iter()
        .filter(|s| s.signal() == ConfidenceSignal::Red)
        .map(|s| s.type_name())
        .collect();
    if !all_red.is_empty() {
        return VerifyOutcome::from_findings(vec![Finding::error(format!(
            "{}: {} type(s) have Red signal (TDDD violation — run /track:design): {}",
            domain_types_path.display(),
            all_red.len(),
            all_red.join(", ")
        ))]);
    }

    // Yellow check (strict only): declared entries only (undeclared signals are never Yellow)
    let entry_keys: std::collections::HashSet<(&str, &str)> =
        doc.entries().iter().map(|e| (e.name(), e.kind().kind_tag())).collect();

    if strict {
        let yellow_entries: Vec<&str> = signals
            .iter()
            .filter(|s| entry_keys.contains(&(s.type_name(), s.kind_tag())))
            .filter(|s| s.signal() == ConfidenceSignal::Yellow)
            .map(|s| s.type_name())
            .collect();
        if !yellow_entries.is_empty() {
            return VerifyOutcome::from_findings(vec![Finding::error(format!(
                "{}: {} domain type(s) have Yellow signal (not yet implemented — all must be Blue for merge): {}",
                domain_types_path.display(),
                yellow_entries.len(),
                yellow_entries.join(", ")
            ))]);
        }
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
pub fn verify(spec_path: &Path, strict: bool) -> VerifyOutcome {
    // Delegate to spec.json path when a sibling spec.json exists.
    if let Some(spec_json_path) = sibling_spec_json(spec_path) {
        if spec_json_path.is_file() {
            return verify_from_spec_json(&spec_json_path, strict);
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
        let outcome = verify(&path, false);
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
        let outcome = verify(&path, false);
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
        let outcome = verify(&path, false);
        assert!(!outcome.has_errors(), "table with multiple data rows must pass");
    }

    // --- 3. Header only (no separator) ---

    #[test]
    fn test_spec_states_with_header_only_no_separator_returns_error() {
        let (_dir, path) = make_spec(
            "## Domain States\n\n\
             | State | Description |\n",
        );
        let outcome = verify(&path, false);
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
        let outcome = verify(&path, false);
        assert!(outcome.has_errors(), "header + separator with no data rows must be an error");
    }

    // --- 5. Section exists with empty body ---

    #[test]
    fn test_spec_states_with_empty_section_body_returns_error() {
        let (_dir, path) = make_spec("## Domain States\n");
        let outcome = verify(&path, false);
        assert!(outcome.has_errors(), "empty section body must be an error");
    }

    // --- 6. Section exists with non-table content ---

    #[test]
    fn test_spec_states_with_non_table_content_returns_error() {
        let (_dir, path) = make_spec(
            "## Domain States\n\n\
             This section describes domain states but has no table.\n",
        );
        let outcome = verify(&path, false);
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
        let outcome = verify(&path, false);
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
        let outcome = verify(&path, false);
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
        let outcome = verify(&path, false);
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
        let outcome = verify(&path, false);
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
        let outcome = verify(&path, false);
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
        let outcome = verify(&path, false);
        assert!(
            !outcome.has_errors(),
            "## Domain States after other sections with valid table must pass"
        );
    }

    // --- verify_from_spec_json() tests ---

    const SPEC_JSON_MINIMAL: &str = r#"{
  "schema_version": 1,
  "status": "draft",
  "version": "1.0",
  "title": "Feature",
  "scope": { "in_scope": [], "out_of_scope": [] },
  "signals": { "blue": 1, "yellow": 0, "red": 0 }
}"#;

    const SPEC_JSON_WITH_YELLOW_SIGNALS: &str = r#"{
  "schema_version": 1,
  "status": "draft",
  "version": "1.0",
  "title": "Feature",
  "scope": { "in_scope": [], "out_of_scope": [] },
  "signals": { "blue": 0, "yellow": 1, "red": 0 }
}"#;

    const DOMAIN_TYPES_WITH_ONE_ENTRY: &str = r#"{
  "schema_version": 1,
  "domain_types": [
    { "name": "TrackId", "kind": "value_object", "description": "Track identifier", "approved": true }
  ]
}"#;

    const DOMAIN_TYPES_EMPTY_ENTRIES: &str = r#"{
  "schema_version": 1,
  "domain_types": []
}"#;

    const DOMAIN_TYPES_WITH_ALL_BLUE_SIGNALS: &str = r#"{
  "schema_version": 1,
  "domain_types": [
    { "name": "TrackId", "kind": "value_object", "description": "Track identifier", "approved": true }
  ],
  "signals": [
    { "type_name": "TrackId", "kind_tag": "value_object", "signal": "blue", "found_type": true }
  ]
}"#;

    const DOMAIN_TYPES_WITH_RED_SIGNAL: &str = r#"{
  "schema_version": 1,
  "domain_types": [
    { "name": "TrackId", "kind": "value_object", "description": "Track identifier", "approved": true }
  ],
  "signals": [
    { "type_name": "TrackId", "kind_tag": "value_object", "signal": "red", "found_type": false }
  ]
}"#;

    #[test]
    fn test_verify_from_spec_json_with_valid_domain_types_and_blue_signals_passes() {
        let dir = tempfile::tempdir().unwrap();
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_ALL_BLUE_SIGNALS)
            .unwrap();
        let outcome = verify_from_spec_json(&spec_json_path, false);
        assert!(
            !outcome.has_errors(),
            "domain-types.json with blue signals should pass: {outcome:?}"
        );
    }

    #[test]
    fn test_verify_from_spec_json_with_no_signals_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_ONE_ENTRY).unwrap();
        let outcome = verify_from_spec_json(&spec_json_path, false);
        assert!(outcome.has_errors(), "missing signals must be an error: {outcome:?}");
    }

    #[test]
    fn test_verify_from_spec_json_with_missing_domain_types_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        // No domain-types.json
        let outcome = verify_from_spec_json(&spec_json_path, false);
        assert!(outcome.has_errors(), "missing domain-types.json should be an error: {outcome:?}");
    }

    #[test]
    fn test_verify_from_spec_json_with_empty_entries_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_EMPTY_ENTRIES).unwrap();
        let outcome = verify_from_spec_json(&spec_json_path, false);
        assert!(outcome.has_errors(), "empty entries must be an error: {outcome:?}");
    }

    #[test]
    fn test_verify_from_spec_json_with_all_blue_signals_passes() {
        let dir = tempfile::tempdir().unwrap();
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_ALL_BLUE_SIGNALS)
            .unwrap();
        let outcome = verify_from_spec_json(&spec_json_path, false);
        assert!(!outcome.has_errors(), "all-blue signals should pass: {outcome:?}");
    }

    const DOMAIN_TYPES_WITH_YELLOW_SIGNAL: &str = r#"{
  "schema_version": 1,
  "domain_types": [
    { "name": "TrackId", "kind": "value_object", "description": "Track identifier", "approved": true }
  ],
  "signals": [
    { "type_name": "TrackId", "kind_tag": "value_object", "signal": "yellow", "found_type": false }
  ]
}"#;

    #[test]
    fn test_verify_from_spec_json_with_yellow_signal_passes_in_default_mode() {
        let dir = tempfile::tempdir().unwrap();
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_YELLOW_SIGNAL)
            .unwrap();
        let outcome = verify_from_spec_json(&spec_json_path, false);
        assert!(
            !outcome.has_errors(),
            "yellow signal must pass in default (interim) mode: {outcome:?}"
        );
    }

    #[test]
    fn test_verify_from_spec_json_with_yellow_signal_fails_in_strict_mode() {
        let dir = tempfile::tempdir().unwrap();
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_YELLOW_SIGNAL)
            .unwrap();
        let outcome = verify_from_spec_json(&spec_json_path, true);
        assert!(
            outcome.has_errors(),
            "yellow signal must fail in strict (merge) mode: {outcome:?}"
        );
    }

    // --- Stage 1 strict gate: spec.json yellow signals ---

    #[test]
    fn test_verify_from_spec_json_with_spec_yellow_signals_passes_in_default_mode() {
        let dir = tempfile::tempdir().unwrap();
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_WITH_YELLOW_SIGNALS).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_ALL_BLUE_SIGNALS)
            .unwrap();
        let outcome = verify_from_spec_json(&spec_json_path, false);
        assert!(
            !outcome.has_errors(),
            "spec.json with yellow signals must pass Stage 1 in default (interim) mode: {outcome:?}"
        );
    }

    #[test]
    fn test_verify_from_spec_json_with_spec_yellow_signals_fails_in_strict_mode() {
        let dir = tempfile::tempdir().unwrap();
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_WITH_YELLOW_SIGNALS).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_ALL_BLUE_SIGNALS)
            .unwrap();
        let outcome = verify_from_spec_json(&spec_json_path, true);
        assert!(
            outcome.has_errors(),
            "spec.json with yellow signals must fail Stage 1 in strict (merge) mode: {outcome:?}"
        );
    }

    const DOMAIN_TYPES_WITH_UNDECLARED_RED_SIGNAL: &str = r#"{
  "schema_version": 1,
  "domain_types": [
    { "name": "TrackId", "kind": "value_object", "description": "Track identifier", "approved": true }
  ],
  "signals": [
    { "type_name": "TrackId", "kind_tag": "value_object", "signal": "blue", "found_type": true },
    { "type_name": "SomeUndeclared", "kind_tag": "undeclared_type", "signal": "red", "found_type": true }
  ]
}"#;

    #[test]
    fn test_verify_from_spec_json_with_undeclared_red_signal_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        std::fs::write(
            dir.path().join("domain-types.json"),
            DOMAIN_TYPES_WITH_UNDECLARED_RED_SIGNAL,
        )
        .unwrap();
        let outcome = verify_from_spec_json(&spec_json_path, false);
        assert!(
            outcome.has_errors(),
            "undeclared reverse Red signal must block spec-states (single gate per ADR): {outcome:?}"
        );
    }

    #[test]
    fn test_verify_from_spec_json_with_red_signal_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_RED_SIGNAL).unwrap();
        let outcome = verify_from_spec_json(&spec_json_path, false);
        assert!(outcome.has_errors(), "red signal must be an error: {outcome:?}");
    }

    #[test]
    fn test_verify_from_spec_json_with_invalid_json_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), "not valid json").unwrap();
        let outcome = verify_from_spec_json(&spec_json_path, false);
        assert!(outcome.has_errors(), "invalid JSON must be an error: {outcome:?}");
    }

    // --- verify() delegation tests ---

    #[test]
    fn test_verify_delegates_to_spec_json_when_sibling_exists() {
        let dir = tempfile::tempdir().unwrap();
        // Write a minimal spec.json and a valid domain-types.json
        std::fs::write(dir.path().join("spec.json"), SPEC_JSON_MINIMAL).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_ALL_BLUE_SIGNALS)
            .unwrap();
        // Write spec.md without ## Domain States (would fail under legacy path)
        std::fs::write(
            dir.path().join("spec.md"),
            "---\nstatus: draft\nversion: \"1.0\"\n---\n# Overview\n\nNo domain states here.\n",
        )
        .unwrap();
        let outcome = verify(&dir.path().join("spec.md"), false);
        assert!(
            !outcome.has_errors(),
            "spec.json delegation with valid domain-types.json should pass: {outcome:?}"
        );
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
        let outcome = verify(&dir.path().join("spec.md"), false);
        assert!(
            !outcome.has_errors(),
            "legacy markdown path with valid table must pass: {outcome:?}"
        );
    }
}
