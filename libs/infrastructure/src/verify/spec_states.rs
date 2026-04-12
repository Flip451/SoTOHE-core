//! Verify that spec.md contains a ## Domain States section with at least one table data row.
//!
//! When a sibling `spec.json` exists, delegates to the JSON-based path which
//! reads the sibling `domain-types.json` and verifies its entries.
//! Otherwise falls back to the markdown table scan (legacy path).

use std::path::Path;

use domain::spec::check_spec_doc_signals;
use domain::tddd::catalogue::check_domain_types_signals;
use domain::verify::{Finding, VerifyOutcome};

use crate::tddd::catalogue_codec;
use crate::track::symlink_guard;

use super::frontmatter::parse_yaml_frontmatter;

/// Verifies spec.json Stage 1 signals and (if present) Stage 2 domain type signals.
///
/// This is a thin wrapper around the shared domain-layer pure functions
/// `check_spec_doc_signals` and `check_domain_types_signals`. It reads the
/// files from the filesystem, rejects symlinks via `reject_symlinks_below`
/// (D4.3), decodes the JSON, and delegates the actual rule evaluation to
/// the domain layer.
///
/// Stage 2 (`domain-types.json`) is **opt-in**: when the file is absent,
/// Stage 2 is skipped entirely (TDDD not active for this track, per ADR §D2.1).
/// The same opt-in semantics apply to both the CI path and the merge gate.
///
/// The `strict` parameter controls Yellow handling:
/// - `true`: declared Yellow → `Finding::error` (merge gate)
/// - `false`: declared Yellow → `Finding::warning` (CI interim mode — D8.6)
///
/// Red, None, all-zero, empty entries, and coverage-gap conditions always
/// return `Finding::error` regardless of `strict`.
///
/// # Errors
///
/// Returns findings when:
/// - `spec.json` is a symlink or lives under a symlink'd directory (fail-closed).
/// - `spec.json` cannot be read or decoded.
/// - Stage 1 signal-gate rules are violated.
/// - `domain-types.json` exists but cannot be read or decoded.
/// - `domain-types.json` exists and Stage 2 signal-gate rules are violated.
///
/// Reference: ADR `knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md`
/// §D2, §D2.1, §D4.3, §D8.6.
pub fn verify_from_spec_json(spec_json_path: &Path, strict: bool) -> VerifyOutcome {
    // Use `.` as the trusted root so `reject_symlinks_below` walks every
    // ancestor of `spec_json_path` (stopping at the filesystem boundary
    // when the path is absolute and outside the current working directory).
    // This catches a symlinked immediate parent directory, which would
    // otherwise be the `parent()` short-circuit.
    let trusted_root = Path::new(".");

    // D4.3 CI path: reject symlinks at spec_json_path or any ancestor below
    // the trusted_root before reading.
    match symlink_guard::reject_symlinks_below(spec_json_path, trusted_root) {
        Ok(true) => {}
        Ok(false) => {
            return VerifyOutcome::from_findings(vec![Finding::error(format!(
                "cannot read {}: file not found",
                spec_json_path.display()
            ))]);
        }
        Err(e) => {
            return VerifyOutcome::from_findings(vec![Finding::error(format!(
                "{}: {e}",
                spec_json_path.display()
            ))]);
        }
    }

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

    // Stage 1: delegate to the shared domain-layer pure function.
    let stage1 = check_spec_doc_signals(&spec_doc, strict);
    if stage1.has_errors() {
        return stage1;
    }

    // Locate the sibling domain-types.json. Stage 2 is opt-in:
    // NotFound → skip entirely (TDDD not active).
    let dir = match spec_json_path.parent() {
        Some(d) if !d.as_os_str().is_empty() => d,
        _ => Path::new("."),
    };
    let domain_types_path = dir.join("domain-types.json");

    // D4.3 CI path: reject symlinks on domain-types.json as well.
    // Same trusted_root logic as spec_json: use `.` to walk every ancestor.
    match symlink_guard::reject_symlinks_below(&domain_types_path, trusted_root) {
        Ok(true) => {}
        Ok(false) => {
            // Stage 2 NotFound: TDDD not active for this track — merge stage 1 result.
            return stage1;
        }
        Err(e) => {
            return VerifyOutcome::from_findings(vec![Finding::error(format!(
                "{}: {e}",
                domain_types_path.display()
            ))]);
        }
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

    // Stage 2: delegate to the shared domain-layer pure function.
    // Merge with stage1 findings so Yellow warnings from Stage 1 are preserved
    // alongside Stage 2 results.
    let mut outcome = stage1;
    outcome.merge(check_domain_types_signals(&doc, strict));
    outcome
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
    fn test_verify_from_spec_json_with_missing_domain_types_passes_in_interim_mode() {
        // ADR §D2.1: domain-types.json absent = TDDD opt-out. Stage 2 is skipped.
        let dir = tempfile::tempdir().unwrap();
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        // No domain-types.json — TDDD not active
        let outcome = verify_from_spec_json(&spec_json_path, false);
        assert!(
            !outcome.has_errors(),
            "missing domain-types.json must pass (Stage 2 skip): {outcome:?}"
        );
    }

    #[test]
    fn test_verify_from_spec_json_with_missing_domain_types_passes_in_strict_mode() {
        // Same opt-out behavior in strict mode — NotFound is always skipped.
        let dir = tempfile::tempdir().unwrap();
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        let outcome = verify_from_spec_json(&spec_json_path, true);
        assert!(
            !outcome.has_errors(),
            "missing domain-types.json must pass even in strict mode: {outcome:?}"
        );
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

    // --- D4.3 symlink rejection (S1–S5) ---

    #[cfg(unix)]
    #[test]
    fn test_verify_from_spec_json_rejects_spec_json_symlink() {
        // S1: spec.json is a symlink — BLOCKED by reject_symlinks_below
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("real-spec.json");
        std::fs::write(&target, SPEC_JSON_MINIMAL).unwrap();
        let link = dir.path().join("spec.json");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let outcome = verify_from_spec_json(&link, false);
        assert!(outcome.has_errors(), "symlink spec.json must be rejected: {outcome:?}");
        assert!(
            outcome.findings().iter().any(|f| f.message().contains("symlink")),
            "finding must mention symlink: {outcome:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_verify_from_spec_json_rejects_parent_directory_symlink() {
        // S2: parent directory of spec.json is a symlink — BLOCKED
        let dir = tempfile::tempdir().unwrap();
        let real_sub = dir.path().join("real-sub");
        std::fs::create_dir(&real_sub).unwrap();
        std::fs::write(real_sub.join("spec.json"), SPEC_JSON_MINIMAL).unwrap();
        let link_sub = dir.path().join("link-sub");
        std::os::unix::fs::symlink(&real_sub, &link_sub).unwrap();

        // Compose a path that goes through the symlinked parent directory.
        let spec_via_link = link_sub.join("spec.json");
        let outcome = verify_from_spec_json(&spec_via_link, false);
        assert!(outcome.has_errors(), "parent symlink must be rejected: {outcome:?}");
    }

    #[cfg(unix)]
    #[test]
    fn test_verify_from_spec_json_rejects_domain_types_symlink() {
        // S3: spec.json is a regular file but domain-types.json is a symlink — BLOCKED
        let dir = tempfile::tempdir().unwrap();
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();

        let dt_target = dir.path().join("real-domain-types.json");
        std::fs::write(&dt_target, DOMAIN_TYPES_WITH_ALL_BLUE_SIGNALS).unwrap();
        let dt_link = dir.path().join("domain-types.json");
        std::os::unix::fs::symlink(&dt_target, &dt_link).unwrap();

        let outcome = verify_from_spec_json(&spec_json_path, false);
        assert!(outcome.has_errors(), "symlink domain-types.json must be rejected: {outcome:?}");
    }

    #[test]
    fn test_verify_from_spec_json_regular_files_pass() {
        // S5 (control): both files are regular, Stage 1 and Stage 2 both pass.
        let dir = tempfile::tempdir().unwrap();
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_ALL_BLUE_SIGNALS)
            .unwrap();
        let outcome = verify_from_spec_json(&spec_json_path, false);
        assert!(!outcome.has_errors(), "regular files must pass: {outcome:?}");
    }
}
