//! Verify that spec.md contains a ## Domain States section with at least one table data row.
//!
//! When a sibling `spec.json` exists, delegates to the JSON-based path which
//! reads the sibling `domain-types.json` and verifies its entries.
//! Otherwise falls back to the markdown table scan (legacy path).

use std::path::Path;

use domain::check_type_signals;
use domain::spec::check_spec_doc_signals;
use domain::verify::{VerifyFinding, VerifyOutcome};

use crate::tddd::{catalogue_codec, type_signals_codec};
use crate::track::symlink_guard;

use super::frontmatter::parse_yaml_frontmatter;
use super::tddd_layers::{TdddLayerBinding, parse_tddd_layers};

/// Verifies spec.json Stage 1 signals and (if present) Stage 2 domain type signals.
///
/// This is a thin wrapper around the shared domain-layer pure functions
/// `check_spec_doc_signals` and `check_type_signals`. It reads the
/// files from the filesystem, rejects symlinks via `reject_symlinks_below`
/// (D4.3), decodes the JSON, and delegates the actual rule evaluation to
/// the domain layer.
///
/// Stage 2 (`domain-types.json`) is **opt-in**: when the file is absent,
/// Stage 2 is skipped entirely (TDDD not active for this track, per ADR §D2.1).
/// The same opt-in semantics apply to both the CI path and the merge gate.
///
/// The `strict` parameter controls Yellow handling:
/// - `true`: declared Yellow → `VerifyFinding::error` (merge gate)
/// - `false`: declared Yellow → `VerifyFinding::warning` (CI interim mode — D8.6)
///
/// Red, None, all-zero, empty entries, and coverage-gap conditions always
/// return `VerifyFinding::error` regardless of `strict`.
///
/// The `trusted_root` parameter anchors the symlink guard (`reject_symlinks_below`):
/// the guard walks ancestors of `spec_json_path` only until it reaches
/// `trusted_root`, then stops. Callers must pass an absolute path to the
/// repository root (e.g. `SystemGitRepo::discover()?.root()`) so that
/// host-level symlinks above the repo (for example `/var` on macOS) are NOT
/// walked and the gate behavior is environment-independent. Tests may pass
/// a tempdir root.
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
pub fn verify_from_spec_json(
    spec_json_path: &Path,
    strict: bool,
    trusted_root: &Path,
) -> VerifyOutcome {
    // D4.3 CI path: reject symlinks at spec_json_path or any ancestor below
    // the trusted_root before reading. The caller is responsible for supplying
    // an absolute `trusted_root` anchored at the repo root so host-level
    // symlinks (e.g. `/var` on macOS) above the repo are NOT walked.
    match symlink_guard::reject_symlinks_below(spec_json_path, trusted_root) {
        Ok(true) => {}
        Ok(false) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "cannot read {}: file not found",
                spec_json_path.display()
            ))]);
        }
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "{}: {e}",
                spec_json_path.display()
            ))]);
        }
    }

    let spec_json = match std::fs::read_to_string(spec_json_path) {
        Ok(s) => s,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "cannot read {}: {e}",
                spec_json_path.display()
            ))]);
        }
    };
    let spec_doc = match crate::spec::codec::decode(&spec_json) {
        Ok(d) => d,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
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

    // Stage 2 multi-layer loop: read architecture-rules.json from the trusted
    // root, enumerate every `tddd.enabled` layer, and run the signal gate
    // against each layer's catalogue file. All findings are AND-aggregated.
    let bindings = match load_tddd_layers(trusted_root) {
        Ok(bindings) => bindings,
        Err(finding) => {
            let mut outcome = stage1;
            outcome.merge(VerifyOutcome::from_findings(vec![finding]));
            return outcome;
        }
    };

    // Locate the track directory (sibling of spec.json) to resolve each
    // layer's catalogue_file against.
    let dir = match spec_json_path.parent() {
        Some(d) if !d.as_os_str().is_empty() => d,
        _ => Path::new("."),
    };

    let mut outcome = stage1;
    for binding in &bindings {
        outcome.merge(evaluate_layer_catalogue(binding, dir, trusted_root, strict));
    }
    outcome
}

/// Loads `architecture-rules.json` from `trusted_root` and returns the list
/// of enabled TDDD layer bindings.
///
/// Fails closed when the rules file is absent or contains no `tddd.enabled`
/// layers. Both conditions match the strict merge-gate behavior in
/// `check_strict_merge_gate`, so a dev who sees CI pass can be sure the
/// merge gate will also pass (and vice-versa).
///
/// The symlink guard (`reject_symlinks_below`, D4.3) is applied before reading
/// the file, consistent with how `spec.json` and per-layer catalogues are read.
fn load_tddd_layers(trusted_root: &Path) -> Result<Vec<TdddLayerBinding>, VerifyFinding> {
    let path = trusted_root.join("architecture-rules.json");
    // D4.3 CI path: reject symlinks before reading, consistent with spec.json
    // and catalogue guards.
    match symlink_guard::reject_symlinks_below(&path, trusted_root) {
        Ok(true) => {}
        Ok(false) => {
            // Fail-closed: match the merge-gate contract. A repo / test dir
            // without architecture-rules.json cannot run Stage 2 at all.
            return Err(VerifyFinding::error(format!(
                "{}: architecture-rules.json not found — \
                 the type-signal gate requires the file to enumerate TDDD layers",
                path.display()
            )));
        }
        Err(e) => {
            return Err(VerifyFinding::error(format!("{}: {e}", path.display())));
        }
    }
    let content = std::fs::read_to_string(&path)
        .map_err(|e| VerifyFinding::error(format!("cannot read {}: {e}", path.display())))?;
    let bindings = parse_tddd_layers(&content)
        .map_err(|e| VerifyFinding::error(format!("{}: {e}", path.display())))?;
    // Fail-closed: a parsed-but-empty binding list (every layer
    // `tddd.enabled = false`, or a rules file without any `tddd` blocks)
    // is treated as a configuration error. Silently skipping Stage 2 would
    // let CI pass on a rules file that disabled every layer, which breaks
    // the strict enforcement contract shared with `check_strict_merge_gate`.
    if bindings.is_empty() {
        return Err(VerifyFinding::error(format!(
            "{}: architecture-rules.json declares no tddd.enabled layers — \
             the type-signal gate cannot verify an empty layer set",
            path.display()
        )));
    }
    Ok(bindings)
}

/// Runs Stage 2 signal evaluation for a single enabled TDDD layer.
///
/// `dir` is the track directory that contains the catalogue file.
/// NotFound on the catalogue file is treated as "TDDD not active for this
/// layer" and returns a clean outcome (no findings). Other errors return
/// fail-closed [`VerifyFinding::error`] entries.
fn evaluate_layer_catalogue(
    binding: &TdddLayerBinding,
    dir: &Path,
    trusted_root: &Path,
    strict: bool,
) -> VerifyOutcome {
    let catalogue_path = dir.join(binding.catalogue_file());

    // D4.3 CI path: reject symlinks per layer.
    match symlink_guard::reject_symlinks_below(&catalogue_path, trusted_root) {
        Ok(true) => {}
        Ok(false) => {
            // NotFound for this layer → TDDD not active for the layer.
            return VerifyOutcome::pass();
        }
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "{}: {e}",
                catalogue_path.display()
            ))]);
        }
    }

    // Read declaration bytes once. `declaration_hash` (below) is pinned to the
    // post-encode on-disk bytes per ADR 2026-04-18-1400 §D5, so the `[u8]` view
    // feeding the SHA-256 digest must be exactly what went to disk.
    let declaration_bytes = match std::fs::read(&catalogue_path) {
        Ok(b) => b,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "cannot read {}: {e}",
                catalogue_path.display()
            ))]);
        }
    };
    let catalogue_str = match std::str::from_utf8(&declaration_bytes) {
        Ok(s) => s,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "{}: invalid UTF-8: {e}",
                catalogue_path.display()
            ))]);
        }
    };
    let mut doc = match catalogue_codec::decode(catalogue_str) {
        Ok(d) => d,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "{}: invalid {}: {e}",
                catalogue_path.display(),
                binding.catalogue_file()
            ))]);
        }
    };

    // ADR §D5 + §D7: evaluate the per-layer signal file
    // (`<layer>-type-signals.json`). Missing / stale / symlink / decode error
    // all produce fail-closed `VerifyFinding::error` symmetric across CI and
    // merge gate paths (the `strict` flag does NOT relax these cases).
    let signal_file_name = binding.signal_file();
    let signal_path = dir.join(&signal_file_name);
    match symlink_guard::reject_symlinks_below(&signal_path, trusted_root) {
        Ok(true) => {
            // Signal file present and not a symlink — decode, compare hash,
            // and plumb the signals into the declaration document so that
            // `check_type_signals` evaluates against the externally-stored
            // evaluation result rather than any legacy inline signals.
            let signal_str = match std::fs::read_to_string(&signal_path) {
                Ok(s) => s,
                Err(e) => {
                    return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                        "cannot read {}: {e}",
                        signal_path.display()
                    ))]);
                }
            };
            let signals_doc = match type_signals_codec::decode(&signal_str) {
                Ok(d) => d,
                Err(e) => {
                    return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                        "{}: invalid {}: {e}",
                        signal_path.display(),
                        signal_file_name
                    ))]);
                }
            };
            let current_hash = type_signals_codec::declaration_hash(&declaration_bytes);
            if signals_doc.declaration_hash() != current_hash {
                return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                    "{}: declaration_hash mismatch (recorded={}, current={}) — \
                     re-run `sotp track type-signals` to refresh the evaluation result",
                    signal_path.display(),
                    signals_doc.declaration_hash(),
                    current_hash
                ))]);
            }
            doc.set_signals(signals_doc.signals().to_vec());
        }
        Ok(false) => {
            // Signal file is genuinely absent. ADR §D5: fail-closed symmetric
            // across CI (strict=false) and merge gate (strict=true) — the
            // `strict` flag does NOT relax this case. The catalogue has
            // declared types but no evaluation has been persisted.
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "{} not found — run `sotp track type-signals` to generate the evaluation result",
                signal_path.display()
            ))]);
        }
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "{}: {e}",
                signal_path.display()
            ))]);
        }
    }

    check_type_signals(&doc, strict, binding.catalogue_file())
}

/// Verifies that `spec.md` contains a `## Domain States` section with a markdown table
/// that has at least one data row (beyond the header and separator rows).
///
/// When a sibling `spec.json` exists next to `spec_path`, delegates to
/// `verify_from_spec_json` (passing through `trusted_root`). Otherwise falls
/// back to the markdown table scan.
///
/// See [`verify_from_spec_json`] for the `trusted_root` contract.
///
/// # Errors
///
/// Returns findings when:
/// - The file cannot be read.
/// - The `## Domain States` heading is absent from the body.
/// - The section exists but contains no markdown table.
/// - The table has no data rows (header + separator only).
pub fn verify(spec_path: &Path, strict: bool, trusted_root: &Path) -> VerifyOutcome {
    // Delegate to spec.json path when a sibling spec.json exists.
    if let Some(spec_json_path) = sibling_spec_json(spec_path) {
        if spec_json_path.is_file() {
            return verify_from_spec_json(&spec_json_path, strict, trusted_root);
        }
    }

    // Legacy markdown-based flow.
    let content = match std::fs::read_to_string(spec_path) {
        Ok(c) => c,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
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
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
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
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "{}: '## Domain States' section has no markdown table",
            spec_path.display()
        ))]);
    }

    // A valid table needs header row, separator row (`|---|`), and at least one data row.
    // We detect the separator row as a `|`-prefixed line containing only `-`, `|`, ` `, `:`.
    let sep_idx = table_lines.iter().position(|l| is_table_separator(l));

    let Some(sep_pos) = sep_idx else {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "{}: '## Domain States' table has no separator row (header-only table)",
            spec_path.display()
        ))]);
    };

    // Separator must be preceded by at least one header row.
    if sep_pos == 0 {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "{}: '## Domain States' table has no header row before separator",
            spec_path.display()
        ))]);
    }

    // Data rows come after the separator (excluding additional separator rows).
    let data_rows: Vec<&str> =
        table_lines.iter().skip(sep_pos + 1).copied().filter(|l| !is_table_separator(l)).collect();

    if data_rows.is_empty() {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
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

    // Helper: write a `<layer>-type-signals.json` (schema_version 1) whose
    // `declaration_hash` matches the on-disk bytes of the companion
    // `<layer>-types.json` file. The `signals` field is copied verbatim from
    // the declaration file's legacy `signals` array (raw JSON) — this is
    // independent of `catalogue_codec::decode`, which silently drops legacy
    // inline signals. Tests that write fixture declaration files with inline
    // signals still exercise the intended Blue/Yellow/Red paths in
    // `check_type_signals` via the signal file.
    fn write_matching_signal_file(track_dir: &Path, catalogue_name: &str, signal_name: &str) {
        let decl_bytes = std::fs::read(track_dir.join(catalogue_name)).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&decl_bytes).unwrap();
        let signals_array =
            value.get("signals").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        let hash = crate::tddd::type_signals_codec::declaration_hash(&decl_bytes);
        let signal_file = serde_json::json!({
            "schema_version": 1,
            "generated_at": "2026-04-18T12:00:00Z",
            "declaration_hash": hash,
            "signals": signals_array,
        });
        let encoded = serde_json::to_string_pretty(&signal_file).unwrap();
        std::fs::write(track_dir.join(signal_name), encoded).unwrap();
    }

    // --- 1. No Domain States section ---

    #[test]
    fn test_spec_states_with_no_domain_states_section_returns_error() {
        let (_dir, path) =
            make_spec("---\nstatus: draft\nversion: \"1.0\"\n---\n# Overview\n\nSome content.\n");
        let outcome = verify(&path, false, _dir.path());
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
        let outcome = verify(&path, false, _dir.path());
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
        let outcome = verify(&path, false, _dir.path());
        assert!(!outcome.has_errors(), "table with multiple data rows must pass");
    }

    // --- 3. Header only (no separator) ---

    #[test]
    fn test_spec_states_with_header_only_no_separator_returns_error() {
        let (_dir, path) = make_spec(
            "## Domain States\n\n\
             | State | Description |\n",
        );
        let outcome = verify(&path, false, _dir.path());
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
        let outcome = verify(&path, false, _dir.path());
        assert!(outcome.has_errors(), "header + separator with no data rows must be an error");
    }

    // --- 5. Section exists with empty body ---

    #[test]
    fn test_spec_states_with_empty_section_body_returns_error() {
        let (_dir, path) = make_spec("## Domain States\n");
        let outcome = verify(&path, false, _dir.path());
        assert!(outcome.has_errors(), "empty section body must be an error");
    }

    // --- 6. Section exists with non-table content ---

    #[test]
    fn test_spec_states_with_non_table_content_returns_error() {
        let (_dir, path) = make_spec(
            "## Domain States\n\n\
             This section describes domain states but has no table.\n",
        );
        let outcome = verify(&path, false, _dir.path());
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
        let outcome = verify(&path, false, _dir.path());
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
        let outcome = verify(&path, false, _dir.path());
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
        let outcome = verify(&path, false, dir.path());
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
        let outcome = verify(&path, false, _dir.path());
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
        let outcome = verify(&path, false, _dir.path());
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
        let outcome = verify(&path, false, _dir.path());
        assert!(
            !outcome.has_errors(),
            "## Domain States after other sections with valid table must pass"
        );
    }

    // --- verify_from_spec_json() tests ---

    /// Writes a minimal `architecture-rules.json` with only `domain` TDDD-enabled
    /// into the given tmp dir. All `verify_from_spec_json` tests that expect
    /// Stage 2 evaluation must call this helper so that the multi-layer loop
    /// finds exactly one enabled layer pointing at `domain-types.json`.
    fn write_minimal_arch_rules(dir: &Path) {
        let content = r#"{
  "version": 2,
  "layers": [
    {
      "crate": "domain",
      "path": "libs/domain",
      "may_depend_on": [],
      "deny_reason": "",
      "tddd": {
        "enabled": true,
        "catalogue_file": "domain-types.json"
      }
    }
  ]
}"#;
        std::fs::write(dir.join("architecture-rules.json"), content).unwrap();
    }

    const SPEC_JSON_MINIMAL: &str = r#"{
  "schema_version": 2,
  "version": "1.0",
  "title": "Feature",
  "scope": { "in_scope": [], "out_of_scope": [] },
  "signals": { "blue": 1, "yellow": 0, "red": 0 }
}"#;

    const SPEC_JSON_WITH_YELLOW_SIGNALS: &str = r#"{
  "schema_version": 2,
  "version": "1.0",
  "title": "Feature",
  "scope": { "in_scope": [], "out_of_scope": [] },
  "signals": { "blue": 0, "yellow": 1, "red": 0 }
}"#;

    const DOMAIN_TYPES_WITH_ONE_ENTRY: &str = r#"{
  "schema_version": 2,
  "type_definitions": [
    { "name": "TrackId", "kind": "value_object", "description": "Track identifier", "approved": true, "expected_members": [], "expected_methods": [] }
  ]
}"#;

    const DOMAIN_TYPES_EMPTY_ENTRIES: &str = r#"{
  "schema_version": 2,
  "type_definitions": []
}"#;

    const DOMAIN_TYPES_WITH_ALL_BLUE_SIGNALS: &str = r#"{
  "schema_version": 2,
  "type_definitions": [
    { "name": "TrackId", "kind": "value_object", "description": "Track identifier", "approved": true, "expected_members": [], "expected_methods": [] }
  ],
  "signals": [
    { "type_name": "TrackId", "kind_tag": "value_object", "signal": "blue", "found_type": true }
  ]
}"#;

    const DOMAIN_TYPES_WITH_RED_SIGNAL: &str = r#"{
  "schema_version": 2,
  "type_definitions": [
    { "name": "TrackId", "kind": "value_object", "description": "Track identifier", "approved": true, "expected_members": [], "expected_methods": [] }
  ],
  "signals": [
    { "type_name": "TrackId", "kind_tag": "value_object", "signal": "red", "found_type": false }
  ]
}"#;

    #[test]
    fn test_verify_from_spec_json_with_valid_domain_types_and_blue_signals_passes() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_arch_rules(dir.path());
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_ALL_BLUE_SIGNALS)
            .unwrap();
        write_matching_signal_file(dir.path(), "domain-types.json", "domain-type-signals.json");
        let outcome = verify_from_spec_json(&spec_json_path, false, dir.path());
        assert!(
            !outcome.has_errors(),
            "domain-types.json with blue signals should pass: {outcome:?}"
        );
    }

    #[test]
    fn test_verify_from_spec_json_with_no_signals_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_arch_rules(dir.path());
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_ONE_ENTRY).unwrap();
        let outcome = verify_from_spec_json(&spec_json_path, false, dir.path());
        assert!(outcome.has_errors(), "missing signals must be an error: {outcome:?}");
    }

    #[test]
    fn test_verify_from_spec_json_with_missing_domain_types_passes_in_interim_mode() {
        // ADR §D2.1: domain-types.json absent = TDDD opt-out. Stage 2 is skipped.
        let dir = tempfile::tempdir().unwrap();
        write_minimal_arch_rules(dir.path());
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        // No domain-types.json — TDDD not active
        let outcome = verify_from_spec_json(&spec_json_path, false, dir.path());
        assert!(
            !outcome.has_errors(),
            "missing domain-types.json must pass (Stage 2 skip): {outcome:?}"
        );
    }

    #[test]
    fn test_verify_from_spec_json_with_missing_domain_types_passes_in_strict_mode() {
        // Same opt-out behavior in strict mode — NotFound is always skipped.
        let dir = tempfile::tempdir().unwrap();
        write_minimal_arch_rules(dir.path());
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        let outcome = verify_from_spec_json(&spec_json_path, true, dir.path());
        assert!(
            !outcome.has_errors(),
            "missing domain-types.json must pass even in strict mode: {outcome:?}"
        );
    }

    #[test]
    fn test_verify_from_spec_json_with_empty_entries_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_arch_rules(dir.path());
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_EMPTY_ENTRIES).unwrap();
        let outcome = verify_from_spec_json(&spec_json_path, false, dir.path());
        assert!(outcome.has_errors(), "empty entries must be an error: {outcome:?}");
    }

    #[test]
    fn test_verify_from_spec_json_with_all_blue_signals_passes() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_arch_rules(dir.path());
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_ALL_BLUE_SIGNALS)
            .unwrap();
        write_matching_signal_file(dir.path(), "domain-types.json", "domain-type-signals.json");
        let outcome = verify_from_spec_json(&spec_json_path, false, dir.path());
        assert!(!outcome.has_errors(), "all-blue signals should pass: {outcome:?}");
    }

    const DOMAIN_TYPES_WITH_YELLOW_SIGNAL: &str = r#"{
  "schema_version": 2,
  "type_definitions": [
    { "name": "TrackId", "kind": "value_object", "description": "Track identifier", "approved": true, "expected_members": [], "expected_methods": [] }
  ],
  "signals": [
    { "type_name": "TrackId", "kind_tag": "value_object", "signal": "yellow", "found_type": false }
  ]
}"#;

    #[test]
    fn test_verify_from_spec_json_with_yellow_signal_passes_in_default_mode() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_arch_rules(dir.path());
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_YELLOW_SIGNAL)
            .unwrap();
        write_matching_signal_file(dir.path(), "domain-types.json", "domain-type-signals.json");
        let outcome = verify_from_spec_json(&spec_json_path, false, dir.path());
        assert!(
            !outcome.has_errors(),
            "yellow signal must pass in default (interim) mode: {outcome:?}"
        );
    }

    #[test]
    fn test_verify_from_spec_json_with_yellow_signal_fails_in_strict_mode() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_arch_rules(dir.path());
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_YELLOW_SIGNAL)
            .unwrap();
        write_matching_signal_file(dir.path(), "domain-types.json", "domain-type-signals.json");
        let outcome = verify_from_spec_json(&spec_json_path, true, dir.path());
        assert!(
            outcome.has_errors(),
            "yellow signal must fail in strict (merge) mode: {outcome:?}"
        );
    }

    // --- Stage 1 strict gate: spec.json yellow signals ---

    #[test]
    fn test_verify_from_spec_json_with_spec_yellow_signals_passes_in_default_mode() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_arch_rules(dir.path());
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_WITH_YELLOW_SIGNALS).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_ALL_BLUE_SIGNALS)
            .unwrap();
        write_matching_signal_file(dir.path(), "domain-types.json", "domain-type-signals.json");
        let outcome = verify_from_spec_json(&spec_json_path, false, dir.path());
        assert!(
            !outcome.has_errors(),
            "spec.json with yellow signals must pass Stage 1 in default (interim) mode: {outcome:?}"
        );
    }

    #[test]
    fn test_verify_from_spec_json_with_spec_yellow_signals_fails_in_strict_mode() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_arch_rules(dir.path());
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_WITH_YELLOW_SIGNALS).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_ALL_BLUE_SIGNALS)
            .unwrap();
        write_matching_signal_file(dir.path(), "domain-types.json", "domain-type-signals.json");
        let outcome = verify_from_spec_json(&spec_json_path, true, dir.path());
        assert!(
            outcome.has_errors(),
            "spec.json with yellow signals must fail Stage 1 in strict (merge) mode: {outcome:?}"
        );
    }

    const DOMAIN_TYPES_WITH_UNDECLARED_RED_SIGNAL: &str = r#"{
  "schema_version": 2,
  "type_definitions": [
    { "name": "TrackId", "kind": "value_object", "description": "Track identifier", "approved": true, "expected_members": [], "expected_methods": [] }
  ],
  "signals": [
    { "type_name": "TrackId", "kind_tag": "value_object", "signal": "blue", "found_type": true },
    { "type_name": "SomeUndeclared", "kind_tag": "undeclared_type", "signal": "red", "found_type": true }
  ]
}"#;

    #[test]
    fn test_verify_from_spec_json_with_undeclared_red_signal_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_arch_rules(dir.path());
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        std::fs::write(
            dir.path().join("domain-types.json"),
            DOMAIN_TYPES_WITH_UNDECLARED_RED_SIGNAL,
        )
        .unwrap();
        write_matching_signal_file(dir.path(), "domain-types.json", "domain-type-signals.json");
        let outcome = verify_from_spec_json(&spec_json_path, false, dir.path());
        assert!(
            outcome.has_errors(),
            "undeclared reverse Red signal must block spec-states (single gate per ADR): {outcome:?}"
        );
    }

    #[test]
    fn test_verify_from_spec_json_with_red_signal_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_arch_rules(dir.path());
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_RED_SIGNAL).unwrap();
        write_matching_signal_file(dir.path(), "domain-types.json", "domain-type-signals.json");
        let outcome = verify_from_spec_json(&spec_json_path, false, dir.path());
        assert!(outcome.has_errors(), "red signal must be an error: {outcome:?}");
    }

    #[test]
    fn test_verify_from_spec_json_with_invalid_json_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_arch_rules(dir.path());
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), "not valid json").unwrap();
        let outcome = verify_from_spec_json(&spec_json_path, false, dir.path());
        assert!(outcome.has_errors(), "invalid JSON must be an error: {outcome:?}");
    }

    #[test]
    fn test_verify_from_spec_json_with_custom_catalogue_override_mentions_override_in_findings() {
        // Verify that when `architecture-rules.json` uses a non-default
        // `tddd.catalogue_file` override (here: "custom-types.json"), the
        // error message produced by Stage 2 mentions that overridden filename
        // rather than the layer-id derived default ("domain-types.json").
        //
        // A regression in this code path would silently forward
        // `binding.catalogue_file()` as the wrong name, producing diagnostics
        // that point at a file the developer never sees.
        let dir = tempfile::tempdir().unwrap();
        // Write arch rules with a custom catalogue_file override.
        let arch_rules_custom = r#"{
  "version": 2,
  "layers": [
    {
      "crate": "domain",
      "path": "libs/domain",
      "may_depend_on": [],
      "deny_reason": "",
      "tddd": {
        "enabled": true,
        "catalogue_file": "custom-types.json"
      }
    }
  ]
}"#;
        std::fs::write(dir.path().join("architecture-rules.json"), arch_rules_custom).unwrap();
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        // Write the catalogue under the override name with a Red signal so Stage 2 fails.
        std::fs::write(dir.path().join("custom-types.json"), DOMAIN_TYPES_WITH_RED_SIGNAL).unwrap();
        write_matching_signal_file(dir.path(), "custom-types.json", "custom-type-signals.json");
        let outcome = verify_from_spec_json(&spec_json_path, false, dir.path());
        assert!(outcome.has_errors(), "red signal must be an error: {outcome:?}");
        assert!(
            outcome.findings().iter().any(|f| f.message().contains("custom-types.json")),
            "finding must mention the override filename 'custom-types.json': {outcome:?}"
        );
    }

    // --- verify() delegation tests ---

    #[test]
    fn test_verify_delegates_to_spec_json_when_sibling_exists() {
        let dir = tempfile::tempdir().unwrap();
        write_minimal_arch_rules(dir.path());
        // Write a minimal spec.json and a valid domain-types.json
        std::fs::write(dir.path().join("spec.json"), SPEC_JSON_MINIMAL).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_ALL_BLUE_SIGNALS)
            .unwrap();
        write_matching_signal_file(dir.path(), "domain-types.json", "domain-type-signals.json");
        // Write spec.md without ## Domain States (would fail under legacy path)
        std::fs::write(
            dir.path().join("spec.md"),
            "---\nstatus: draft\nversion: \"1.0\"\n---\n# Overview\n\nNo domain states here.\n",
        )
        .unwrap();
        let outcome = verify(&dir.path().join("spec.md"), false, dir.path());
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
        let outcome = verify(&dir.path().join("spec.md"), false, dir.path());
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

        let outcome = verify_from_spec_json(&link, false, dir.path());
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
        let outcome = verify_from_spec_json(&spec_via_link, false, dir.path());
        assert!(outcome.has_errors(), "parent symlink must be rejected: {outcome:?}");
    }

    #[cfg(unix)]
    #[test]
    fn test_verify_from_spec_json_rejects_domain_types_symlink() {
        // S3: spec.json is a regular file but domain-types.json is a symlink — BLOCKED
        let dir = tempfile::tempdir().unwrap();
        write_minimal_arch_rules(dir.path());
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();

        let dt_target = dir.path().join("real-domain-types.json");
        std::fs::write(&dt_target, DOMAIN_TYPES_WITH_ALL_BLUE_SIGNALS).unwrap();
        let dt_link = dir.path().join("domain-types.json");
        std::os::unix::fs::symlink(&dt_target, &dt_link).unwrap();
        // Write a matching signal file so the symlink rejection on the declaration
        // file is the only reason the test fails (isolates the S3 guard under test).
        write_matching_signal_file(
            dir.path(),
            "real-domain-types.json",
            "domain-type-signals.json",
        );

        let outcome = verify_from_spec_json(&spec_json_path, false, dir.path());
        assert!(outcome.has_errors(), "symlink domain-types.json must be rejected: {outcome:?}");
    }

    #[test]
    fn test_verify_from_spec_json_regular_files_pass() {
        // S5 (control): both files are regular, Stage 1 and Stage 2 both pass.
        let dir = tempfile::tempdir().unwrap();
        write_minimal_arch_rules(dir.path());
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_ALL_BLUE_SIGNALS)
            .unwrap();
        write_matching_signal_file(dir.path(), "domain-types.json", "domain-type-signals.json");
        let outcome = verify_from_spec_json(&spec_json_path, false, dir.path());
        assert!(!outcome.has_errors(), "regular files must pass: {outcome:?}");
    }

    // --- ADR 2026-04-18-1400 §D5 signal-file evaluation ---

    #[test]
    fn test_signal_file_missing_returns_error_in_interim_mode() {
        // Missing signal file is fail-closed in the CI interim path per ADR §D5.
        let dir = tempfile::tempdir().unwrap();
        write_minimal_arch_rules(dir.path());
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        // Declaration file present, signal file intentionally absent.
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_ALL_BLUE_SIGNALS)
            .unwrap();
        let outcome = verify_from_spec_json(&spec_json_path, false, dir.path());
        assert!(outcome.has_errors(), "missing signal file must be fail-closed: {outcome:?}");
        assert!(
            outcome.findings().iter().any(|f| f.message().contains("domain-type-signals.json")
                && f.message().contains("not found")),
            "finding must name the missing signal file: {outcome:?}"
        );
    }

    #[test]
    fn test_signal_file_missing_returns_error_in_strict_mode() {
        // Symmetric with the interim case: missing signal file is error in both
        // CI and merge gate (strict=true) paths per ADR §D5.
        let dir = tempfile::tempdir().unwrap();
        write_minimal_arch_rules(dir.path());
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_ALL_BLUE_SIGNALS)
            .unwrap();
        let outcome = verify_from_spec_json(&spec_json_path, true, dir.path());
        assert!(outcome.has_errors(), "missing signal file must block merge gate: {outcome:?}");
    }

    const DOMAIN_TYPE_SIGNALS_STALE_HASH: &str = r#"{
  "schema_version": 1,
  "generated_at": "2026-04-18T12:00:00Z",
  "declaration_hash": "0000000000000000000000000000000000000000000000000000000000000000",
  "signals": [
    { "type_name": "TrackId", "kind_tag": "value_object", "signal": "blue", "found_type": true }
  ]
}"#;

    #[test]
    fn test_signal_file_stale_hash_returns_error_in_interim_mode() {
        // Stale (declaration_hash mismatch) is fail-closed in CI per ADR §D5,
        // symmetric with the merge gate. The `strict` flag does NOT relax it.
        let dir = tempfile::tempdir().unwrap();
        write_minimal_arch_rules(dir.path());
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_ALL_BLUE_SIGNALS)
            .unwrap();
        std::fs::write(dir.path().join("domain-type-signals.json"), DOMAIN_TYPE_SIGNALS_STALE_HASH)
            .unwrap();
        let outcome = verify_from_spec_json(&spec_json_path, false, dir.path());
        assert!(outcome.has_errors(), "stale signal file must be fail-closed: {outcome:?}");
        assert!(
            outcome.findings().iter().any(|f| f.message().contains("declaration_hash mismatch")),
            "finding must mention declaration_hash mismatch: {outcome:?}"
        );
    }

    #[test]
    fn test_signal_file_stale_hash_returns_error_in_strict_mode() {
        // Symmetric: stale is error in both CI (strict=false) and merge gate
        // (strict=true) per ADR §D5.
        let dir = tempfile::tempdir().unwrap();
        write_minimal_arch_rules(dir.path());
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_ALL_BLUE_SIGNALS)
            .unwrap();
        std::fs::write(dir.path().join("domain-type-signals.json"), DOMAIN_TYPE_SIGNALS_STALE_HASH)
            .unwrap();
        let outcome = verify_from_spec_json(&spec_json_path, true, dir.path());
        assert!(outcome.has_errors(), "stale signal file must block merge gate: {outcome:?}");
    }

    #[test]
    fn test_signal_file_decode_error_returns_error() {
        // Malformed signal file JSON is fail-closed per ADR §D7 decode-error row.
        let dir = tempfile::tempdir().unwrap();
        write_minimal_arch_rules(dir.path());
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_ALL_BLUE_SIGNALS)
            .unwrap();
        std::fs::write(dir.path().join("domain-type-signals.json"), "{not valid json").unwrap();
        let outcome = verify_from_spec_json(&spec_json_path, false, dir.path());
        assert!(outcome.has_errors(), "decode error must be fail-closed: {outcome:?}");
        assert!(
            outcome
                .findings()
                .iter()
                .any(|f| f.message().contains("invalid domain-type-signals.json")),
            "finding must mention invalid signal file: {outcome:?}"
        );
    }

    #[test]
    fn test_signal_file_wrong_schema_version_returns_error() {
        // schema_version != 1 hits the codec's UnsupportedSchemaVersion branch,
        // which bubbles up as a decode error from evaluate_layer_catalogue.
        let dir = tempfile::tempdir().unwrap();
        write_minimal_arch_rules(dir.path());
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_ALL_BLUE_SIGNALS)
            .unwrap();
        let future_schema = r#"{
          "schema_version": 2,
          "generated_at": "2026-04-18T12:00:00Z",
          "declaration_hash": "0000",
          "signals": []
        }"#;
        std::fs::write(dir.path().join("domain-type-signals.json"), future_schema).unwrap();
        let outcome = verify_from_spec_json(&spec_json_path, false, dir.path());
        assert!(outcome.has_errors(), "unknown schema_version must be fail-closed: {outcome:?}");
    }

    #[cfg(unix)]
    #[test]
    fn test_signal_file_symlink_returns_error() {
        // Symlink at the signal-file path is fail-closed per ADR §D7. Uses the
        // existing `reject_symlinks_below` guard (no new symlink detection code).
        let dir = tempfile::tempdir().unwrap();
        write_minimal_arch_rules(dir.path());
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_ALL_BLUE_SIGNALS)
            .unwrap();

        // Signal file content exists at a real path but `domain-type-signals.json`
        // is a symlink to it — the guard rejects the symlink regardless of
        // target validity.
        let real = dir.path().join("real-signals.json");
        write_matching_signal_file(dir.path(), "domain-types.json", "real-signals.json");
        // `write_matching_signal_file` wrote to `real` via the above call; now
        // replace the plain path with a symlink pointing at it.
        std::fs::remove_file(dir.path().join("domain-type-signals.json")).ok();
        std::os::unix::fs::symlink(&real, dir.path().join("domain-type-signals.json")).unwrap();

        let outcome = verify_from_spec_json(&spec_json_path, false, dir.path());
        assert!(outcome.has_errors(), "symlink signal file must be rejected: {outcome:?}");
    }

    #[test]
    fn test_signal_file_matching_hash_and_all_blue_passes_in_both_modes() {
        // Control: a fresh signal file with matching hash + all Blue signals
        // passes both CI (strict=false) and merge gate (strict=true) paths.
        for strict in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            write_minimal_arch_rules(dir.path());
            let spec_json_path = dir.path().join("spec.json");
            std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
            std::fs::write(
                dir.path().join("domain-types.json"),
                DOMAIN_TYPES_WITH_ALL_BLUE_SIGNALS,
            )
            .unwrap();
            write_matching_signal_file(dir.path(), "domain-types.json", "domain-type-signals.json");
            let outcome = verify_from_spec_json(&spec_json_path, strict, dir.path());
            assert!(
                !outcome.has_errors(),
                "matching signal file with Blue entries must pass (strict={strict}): {outcome:?}"
            );
        }
    }

    #[test]
    fn test_signal_file_overrides_inline_declaration_signals() {
        // When the signal file carries Blue signals but the declaration file
        // has stale Red inline signals, the evaluation result reflects the
        // signal file (Blue → pass). This proves the signal file is the
        // authoritative evaluation source (ADR §D1: authored declaration vs
        // generated evaluation result).
        let dir = tempfile::tempdir().unwrap();
        write_minimal_arch_rules(dir.path());
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();

        // Declaration file has inline Red signals for legacy compatibility.
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_RED_SIGNAL).unwrap();

        // Signal file overrides with a Blue signal for the same entry at the
        // current declaration hash.
        let decl_bytes = std::fs::read(dir.path().join("domain-types.json")).unwrap();
        let hash = crate::tddd::type_signals_codec::declaration_hash(&decl_bytes);
        let blue_signal_file = format!(
            r#"{{
              "schema_version": 1,
              "generated_at": "2026-04-18T12:00:00Z",
              "declaration_hash": "{hash}",
              "signals": [
                {{
                  "type_name": "TrackId",
                  "kind_tag": "value_object",
                  "signal": "blue",
                  "found_type": true
                }}
              ]
            }}"#
        );
        std::fs::write(dir.path().join("domain-type-signals.json"), blue_signal_file).unwrap();

        let outcome = verify_from_spec_json(&spec_json_path, false, dir.path());
        assert!(
            !outcome.has_errors(),
            "signal file (Blue) must override inline declaration signals (Red): {outcome:?}"
        );
    }
}
