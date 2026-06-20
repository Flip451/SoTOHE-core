//! Verify that spec.md contains a ## Domain States section with at least one table data row.
//!
//! When a sibling `spec.json` exists, delegates to the JSON-based path which
//! reads the sibling `domain-types.json` and verifies its entries.
//! Otherwise falls back to the markdown table scan (legacy path).

use std::path::{Path, PathBuf};

use domain::spec::{SpecDocument, check_spec_doc_signals};
use domain::verify::{VerifyFinding, VerifyOutcome};
use domain::{SignalCounts, check_type_signals};

use crate::tddd::type_signals_codec;
use crate::track::symlink_guard;

use super::frontmatter::parse_yaml_frontmatter;
use super::path_safety::check_signals_file;
use super::tddd_layers::{TdddLayerBinding, parse_tddd_layers};

/// Verifies spec.json Stage 1 signals (chain ① `check-spec-adr`).
///
/// Reads `spec.json`, rejects symlinks (D4.3), decodes JSON, runs a self-consistency
/// freshness check against `evaluate_signals()` (IN-09), and delegates the actual
/// signal-gate rules to `check_spec_doc_signals`. Stage 2 (chain ③) is performed by
/// `verify_type_signals_from_spec_json`.
///
/// `strict=true` promotes Yellow → error (merge gate); `false` → warning (D8.6).
/// `trusted_root` anchors the symlink guard to the repo root (pass the git root).
///
/// # Errors
///
/// Returns findings on symlinks, read/decode errors, stale signals, or gate violations.
pub fn verify_from_spec_json(
    spec_json_path: PathBuf,
    strict: bool,
    trusted_root: PathBuf,
) -> VerifyOutcome {
    let spec_json_path = spec_json_path.as_path();
    let trusted_root = trusted_root.as_path();

    let spec_doc = match load_spec_doc(spec_json_path, trusted_root) {
        Ok(doc) => doc,
        Err(finding) => return VerifyOutcome::from_findings(vec![finding]),
    };

    if let Some(finding) = spec_signal_freshness(&spec_doc).to_finding(spec_json_path) {
        return VerifyOutcome::from_findings(vec![finding]);
    }

    // Stage 1: delegate to the shared domain-layer pure function.
    check_spec_doc_signals(&spec_doc, strict)
}

enum SpecSignalFreshness {
    Fresh,
    Stale { stored: SignalCounts, computed: SignalCounts },
}

impl SpecSignalFreshness {
    fn to_finding(&self, spec_json_path: &Path) -> Option<VerifyFinding> {
        let SpecSignalFreshness::Stale { stored, computed } = self else {
            return None;
        };

        Some(VerifyFinding::error(format!(
            "{}: spec signals are stale (stored={stored:?}, computed={computed:?}) — \
             re-run `sotp signal calc-spec-adr` to refresh",
            spec_json_path.display(),
        )))
    }
}

fn spec_signal_freshness(spec_doc: &SpecDocument) -> SpecSignalFreshness {
    let Some(stored) = spec_doc.signals() else {
        return SpecSignalFreshness::Fresh;
    };
    let computed = spec_doc.evaluate_signals();
    // An empty / truncated spec must reject any stored non-zero counts — a
    // previously-blue `signals` block carried over from a prior revision
    // would otherwise let `check_spec_doc_signals` pass an unevaluated spec.
    // Only the all-zero stored case is treated as fresh when computed is
    // also zero; `check_spec_doc_signals` still rejects all-zero as
    // unevaluated downstream.
    if computed == *stored {
        return SpecSignalFreshness::Fresh;
    }
    SpecSignalFreshness::Stale { stored: *stored, computed }
}

/// Verifies Stage 2 type signals (chain ③ `check-impl-catalog`) for the spec
/// track identified by `spec_json_path`.
///
/// Reads `architecture-rules.json` from `trusted_root`, enumerates every
/// `tddd.enabled` layer, and runs the signal gate. Layers without a catalogue
/// are skipped (opt-in, ADR §D2.1). All findings are AND-aggregated.
///
/// `strict=true` promotes Yellow → error (merge gate); `false` → warning (D8.6).
///
/// # Errors
///
/// Returns findings on symlinks, read/decode errors, or gate violations.
pub fn verify_type_signals_from_spec_json(
    spec_json_path: PathBuf,
    strict: bool,
    trusted_root: PathBuf,
) -> VerifyOutcome {
    let spec_json_path = spec_json_path.as_path();
    let trusted_root = trusted_root.as_path();

    if let Err(finding) = load_spec_doc(spec_json_path, trusted_root) {
        return VerifyOutcome::from_findings(vec![finding]);
    }

    // Stage 2 multi-layer loop: read architecture-rules.json from the trusted
    // root, enumerate every `tddd.enabled` layer, and run the signal gate
    // against each layer's catalogue file. All findings are AND-aggregated.
    let bindings = match load_tddd_layers(trusted_root) {
        Ok(bindings) => bindings,
        Err(finding) => {
            return VerifyOutcome::from_findings(vec![finding]);
        }
    };

    // Locate the track directory (sibling of spec.json) to resolve each
    // layer's catalogue_file against.
    let dir = match spec_json_path.parent() {
        Some(d) if !d.as_os_str().is_empty() => d,
        _ => Path::new("."),
    };

    let mut outcome = VerifyOutcome::pass();
    for binding in &bindings {
        outcome.merge(evaluate_layer_catalogue(binding, dir, trusted_root, strict));
    }
    outcome
}

fn load_spec_doc(
    spec_json_path: &Path,
    trusted_root: &Path,
) -> Result<SpecDocument, VerifyFinding> {
    // D4.3 CI path: reject symlinks at spec_json_path or any ancestor below
    // the trusted_root before reading. The caller is responsible for supplying
    // an absolute `trusted_root` anchored at the repo root so host-level
    // symlinks (e.g. `/var` on macOS) above the repo are NOT walked.
    match symlink_guard::reject_symlinks_below(spec_json_path, trusted_root) {
        Ok(true) => {}
        Ok(false) => {
            return Err(VerifyFinding::error(format!(
                "cannot read {}: file not found",
                spec_json_path.display()
            )));
        }
        Err(e) => {
            return Err(VerifyFinding::error(format!("{}: {e}", spec_json_path.display())));
        }
    }

    let spec_json = std::fs::read_to_string(spec_json_path).map_err(|e| {
        VerifyFinding::error(format!("cannot read {}: {e}", spec_json_path.display()))
    })?;
    match crate::spec::codec::decode(&spec_json) {
        Ok(d) => Ok(d),
        Err(e) => Err(VerifyFinding::error(format!(
            "{}: spec.json decode error: {e}",
            spec_json_path.display()
        ))),
    }
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
///
/// T022: Rewired to use `TypeSignalsDocument` directly. The catalogue file
/// is still read to compute the `declaration_hash` for freshness verification
/// (ADR 2026-04-18-1400 §D5). All catalogue decoding and `doc.set_signals`
/// calls are removed — `check_type_signals` now takes `&TypeSignalsDocument`
/// directly (pure function over the signals document).
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

    // Read declaration bytes for the freshness check (SHA-256 comparison).
    // The `declaration_hash` in the signal file must match the SHA-256 of
    // these bytes (ADR 2026-04-18-1400 §D5). No catalogue decoding needed
    // along this CI path.
    //
    // ## Codex r3 accepted deviation (PR #132)
    //
    // Codex round 3 flagged that the merge-gate adapter
    // (`merge_gate_adapter::read_type_catalogue`) decodes the catalogue with
    // `CatalogueDocumentCodec` while this CI path only computes a hash,
    // creating a structural-validation asymmetry for malformed catalogues
    // with a matching committed signal hash.  Adding the same decode step
    // here would break dozens of pre-T039 fixtures still authored at
    // `schema_version=2`, so symmetry is deferred to a follow-up task that
    // bulk-converts the v2 fixtures to v3 alongside the decode validation.
    // Until then the merge-gate adapter remains the authoritative structural
    // check before a branch can merge.
    let declaration_bytes = match std::fs::read(&catalogue_path) {
        Ok(b) => b,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "cannot read {}: {e}",
                catalogue_path.display()
            ))]);
        }
    };

    // ADR §D5 + §D7: evaluate the per-layer signal file
    // (`<layer>-type-signals.json`). Missing / stale / symlink / decode error
    // all produce fail-closed `VerifyFinding::error` symmetric across CI and
    // merge gate paths (the `strict` flag does NOT relax these cases).
    let signal_file_name = binding.signal_file();
    let signal_path = dir.join(&signal_file_name);
    let signals_doc = match symlink_guard::reject_symlinks_below(&signal_path, trusted_root) {
        Ok(true) => {
            // Signal file present and not a symlink — decode and compare hash.
            let signal_str = match std::fs::read_to_string(&signal_path) {
                Ok(s) => s,
                Err(e) => {
                    return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                        "cannot read {}: {e}",
                        signal_path.display()
                    ))]);
                }
            };
            let doc = match type_signals_codec::decode(&signal_str) {
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
            if doc.declaration_hash() != current_hash {
                return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                    "{}: declaration_hash mismatch (recorded={}, current={}) — \
                     re-run `sotp track type-signals` to refresh the evaluation result",
                    signal_path.display(),
                    doc.declaration_hash(),
                    current_hash
                ))]);
            }
            doc
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
    };

    check_type_signals(&signals_doc, strict)
}

/// Evaluate chain ③ (`check-impl-catalog`) gate for a single layer with explicit paths.
///
/// Called by `signal check-impl-catalog --signals-path P --catalog-hash H --gate commit|merge`.
/// Performs symlink guards, `declaration_hash` freshness, and the Red/Yellow/Blue domain gate.
///
/// # Errors
///
/// Returns a `VerifyOutcome` with error findings on I/O, decode, or gate failures.
pub fn check_impl_catalog_from_signals_file(
    signals_path: &Path,
    catalog_hash_hex: &str,
    strict: bool,
) -> VerifyOutcome {
    check_signals_file(
        signals_path,
        catalog_hash_hex,
        &format!(
            "{} not found — run `sotp track type-signals` to generate the evaluation result",
            signals_path.display()
        ),
        |text| type_signals_codec::decode(text).map_err(|e| e.to_string()),
        |doc| doc.declaration_hash().to_owned(),
        |recorded, current, path| {
            format!(
                "{}: declaration_hash mismatch (recorded={}, current={}) — \
                 re-run `sotp signal calc-impl-catalog` to refresh the evaluation result",
                path.display(),
                recorded,
                current
            )
        },
        |doc, _normalized_signals, _workspace_root| check_type_signals(&doc, strict),
    )
}

/// Verifies that `spec.md` contains a `## Domain States` section with a markdown table
/// that has at least one data row (beyond the header and separator rows).
///
/// When a sibling `spec.json` exists next to `spec_path`, delegates to
/// `verify_from_spec_json` (Stage 1, chain ①) and then
/// `verify_type_signals_from_spec_json` (Stage 2, chain ③), merging the
/// results. Both stages use the same `strict` parameter here because this
/// combined entrypoint does not yet receive per-chain strictness. Independent
/// per-chain strictness is wired in T009-T011 via `signal check-spec-adr` and
/// `signal check-impl-catalog`. Otherwise falls back to the markdown table scan.
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
    if let Some(outcome) = verify_sibling_json_stages(spec_path, strict, trusted_root) {
        return outcome;
    }

    // Shared legacy markdown prelude: read the file and guard against generated v2 content.
    let content = match read_legacy_spec_markdown_with_label(spec_path, "states") {
        Ok(c) => c,
        Err(outcome) => return outcome,
    };
    verify_domain_states_markdown(spec_path, &content)
}

fn verify_sibling_json_stages(
    spec_path: &Path,
    strict: bool,
    trusted_root: &Path,
) -> Option<VerifyOutcome> {
    let spec_json_path = sibling_spec_json(spec_path)?;
    if !spec_json_path.is_file() {
        return None;
    }

    let mut outcome =
        verify_from_spec_json(spec_json_path.clone(), strict, trusted_root.to_path_buf());
    merge_unique_findings(
        &mut outcome,
        verify_type_signals_from_spec_json(spec_json_path, strict, trusted_root.to_path_buf()),
    );
    Some(outcome)
}

fn merge_unique_findings(outcome: &mut VerifyOutcome, other: VerifyOutcome) {
    for finding in other.findings() {
        let duplicate = outcome.findings().iter().any(|existing| {
            existing.severity() == finding.severity() && existing.message() == finding.message()
        });
        if !duplicate {
            outcome.add(finding.clone());
        }
    }
}

/// Read a `spec.md` file and guard against generated v2 content.
///
/// Shared prelude for legacy markdown verifiers (`spec_signals`, `spec_attribution`,
/// `spec_states`).
///
/// Returns `Ok(content)` when the file is a readable, non-empty legacy markdown spec.
///
/// Returns `Err(outcome)` early when:
/// - The file cannot be read.
/// - The file is empty or contains only whitespace — a blank spec.md is not a valid
///   legacy specification and likely indicates a generation or checkout error.
/// - The file starts with the generated-v2 header (`<!-- Generated from spec.json`),
///   indicating that `spec.json` is absent — verification cannot proceed without it.
///
/// `verifier_label` is embedded in the generated-header error message (e.g.
/// `"signal"`, `"attribution"`, `"states"`) so users know which check triggered the
/// error.
pub(crate) fn read_legacy_spec_markdown_with_label(
    spec_path: &Path,
    verifier_label: &str,
) -> Result<String, VerifyOutcome> {
    let content = std::fs::read_to_string(spec_path).map_err(|e| {
        VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "cannot read {}: {e}",
            spec_path.display()
        ))])
    })?;
    if content.trim().is_empty() {
        return Err(VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "{}: spec.md is empty — a blank spec cannot be verified for {}",
            spec_path.display(),
            verifier_label
        ))]));
    }
    if content.starts_with("<!-- Generated from spec.json") {
        return Err(VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "{}: generated v2 spec.md requires a sibling spec.json for {} verification \
             (spec.json is absent — restore it or re-generate spec.md from spec.json)",
            spec_path.display(),
            verifier_label
        ))]));
    }
    Ok(content)
}

enum DomainStatesMarkdownCheck {
    MissingSection,
    EmptyTable,
    MissingSeparator,
    SeparatorWithoutHeader,
    NoDataRows,
    Valid,
}

fn verify_domain_states_markdown(spec_path: &Path, content: &str) -> VerifyOutcome {
    match check_domain_states_markdown(content) {
        DomainStatesMarkdownCheck::Valid => VerifyOutcome::pass(),
        DomainStatesMarkdownCheck::MissingSection => {
            VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "{}: missing '## Domain States' section",
                spec_path.display()
            ))])
        }
        DomainStatesMarkdownCheck::EmptyTable => {
            VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "{}: '## Domain States' section has no markdown table",
                spec_path.display()
            ))])
        }
        DomainStatesMarkdownCheck::MissingSeparator => {
            VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "{}: '## Domain States' table has no separator row (header-only table)",
                spec_path.display()
            ))])
        }
        DomainStatesMarkdownCheck::SeparatorWithoutHeader => {
            VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "{}: '## Domain States' table has no header row before separator",
                spec_path.display()
            ))])
        }
        DomainStatesMarkdownCheck::NoDataRows => {
            VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                "{}: '## Domain States' table has no data rows (header + separator only)",
                spec_path.display()
            ))])
        }
    }
}

fn check_domain_states_markdown(content: &str) -> DomainStatesMarkdownCheck {
    let body_lines = visible_markdown_body_lines(content);
    let Some(section_idx) =
        body_lines.iter().position(|line| line.trim_end() == "## Domain States")
    else {
        return DomainStatesMarkdownCheck::MissingSection;
    };

    let table_lines: Vec<&str> = body_lines
        .iter()
        .skip(section_idx + 1)
        .take_while(|line| !is_markdown_section_boundary(line))
        .copied()
        .filter(|line| line.trim_start().starts_with('|'))
        .collect();

    if table_lines.is_empty() {
        return DomainStatesMarkdownCheck::EmptyTable;
    }

    let sep_idx = table_lines.iter().position(|l| is_table_separator(l));
    let Some(sep_pos) = sep_idx else {
        return DomainStatesMarkdownCheck::MissingSeparator;
    };
    if sep_pos == 0 {
        return DomainStatesMarkdownCheck::SeparatorWithoutHeader;
    }

    let has_data_row = table_lines.iter().skip(sep_pos + 1).any(|line| !is_table_separator(line));
    if has_data_row {
        DomainStatesMarkdownCheck::Valid
    } else {
        DomainStatesMarkdownCheck::NoDataRows
    }
}

pub(crate) fn visible_markdown_body_lines(content: &str) -> Vec<&str> {
    let body_start = parse_yaml_frontmatter(content).map_or(0, |fm| fm.body_start);
    let mut active_fence = None;
    content
        .lines()
        .skip(body_start)
        .filter(|line| markdown_line_is_visible(line, &mut active_fence))
        .collect()
}

#[derive(Clone, Copy)]
pub(crate) struct MarkdownFence {
    marker: char,
    len: usize,
}

impl MarkdownFence {
    pub(crate) fn opening(trimmed: &str) -> Option<Self> {
        ['`', '~'].into_iter().find_map(|marker| {
            let len = leading_marker_count(trimmed, marker);
            (len >= 3).then_some(Self { marker, len })
        })
    }

    pub(crate) fn closes(self, trimmed: &str) -> bool {
        leading_marker_count(trimmed, self.marker) >= self.len
            && trimmed.chars().all(|c| c == self.marker)
    }
}

pub(crate) fn markdown_line_is_visible(
    line: &str,
    active_fence: &mut Option<MarkdownFence>,
) -> bool {
    let trimmed = line.trim();
    if let Some(fence) = *active_fence {
        if fence.closes(trimmed) {
            *active_fence = None;
        }
        return false;
    }
    if let Some(fence) = MarkdownFence::opening(trimmed) {
        *active_fence = Some(fence);
        return false;
    }
    true
}

fn leading_marker_count(trimmed: &str, marker: char) -> usize {
    trimmed.chars().take_while(|c| *c == marker).count()
}

fn is_markdown_section_boundary(line: &str) -> bool {
    let trimmed = line.trim_end();
    trimmed.starts_with("## ") || trimmed == "##" || trimmed.starts_with("# ") || trimmed == "#"
}

/// Derives the sibling `spec.json` path from a `spec.md` path by replacing
/// the filename component.
///
/// Returns `None` when the path has no parent directory.
pub(crate) fn sibling_spec_json(spec_md_path: &Path) -> Option<std::path::PathBuf> {
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
    // the declaration file's legacy `signals` array (raw JSON) so that fixture
    // declaration files with inline signals still exercise the intended
    // Blue/Yellow/Red paths in `check_type_signals` via the signal file.
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

    /// Writes the smallest `architecture-rules.json` fixture needed by
    /// `parse_tddd_layers`: one enabled `domain` layer pointing at
    /// `domain-types.json`.
    fn write_domain_tddd_rules(dir: &Path) {
        let content = r#"{
  "layers": [
    {
      "crate": "domain",
      "tddd": { "enabled": true, "catalogue_file": "domain-types.json" }
    }
  ]
}"#;
        std::fs::write(dir.join("architecture-rules.json"), content).unwrap();
    }

    fn verify_stage2_fixture<F>(strict: bool, populate: F) -> VerifyOutcome
    where
        F: FnOnce(&Path),
    {
        let dir = tempfile::tempdir().unwrap();
        write_domain_tddd_rules(dir.path());
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        populate(dir.path());
        verify_type_signals_from_spec_json(spec_json_path, strict, dir.path().to_path_buf())
    }

    fn verify_stage2_domain_types(
        domain_types_json: &str,
        strict: bool,
        write_signal_file: bool,
    ) -> VerifyOutcome {
        verify_stage2_fixture(strict, |dir| {
            std::fs::write(dir.join("domain-types.json"), domain_types_json).unwrap();
            if write_signal_file {
                write_matching_signal_file(dir, "domain-types.json", "domain-type-signals.json");
            }
        })
    }

    fn verify_stage2_stale_hash(strict: bool) -> VerifyOutcome {
        verify_stage2_fixture(strict, |dir| {
            std::fs::write(dir.join("domain-types.json"), DOMAIN_TYPES_WITH_ALL_BLUE_SIGNALS)
                .unwrap();
            std::fs::write(dir.join("domain-type-signals.json"), DOMAIN_TYPE_SIGNALS_STALE_HASH)
                .unwrap();
        })
    }

    const SPEC_JSON_MINIMAL: &str = r#"{
  "schema_version": 2,
  "version": "1.0",
  "title": "Feature",
  "scope": {
    "in_scope": [
      {
        "id": "IS-01",
        "text": "minimal blue requirement",
        "adr_refs": [
          { "file": "knowledge/adr/some.md", "anchor": "D1" }
        ]
      }
    ],
    "out_of_scope": []
  },
  "signals": { "blue": 1, "yellow": 0, "red": 0 }
}"#;

    const SPEC_JSON_WITH_YELLOW_SIGNALS: &str = r#"{
  "schema_version": 2,
  "version": "1.0",
  "title": "Feature",
  "scope": {
    "in_scope": [
      {
        "id": "IS-01",
        "text": "yellow requirement",
        "informal_grounds": [{ "kind": "user_directive", "summary": "historical convention" }]
      }
    ],
    "out_of_scope": []
  },
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
    fn test_verify_from_spec_json_with_invalid_domain_type_shapes_return_error() {
        for (fixture, message) in [
            (DOMAIN_TYPES_WITH_ONE_ENTRY, "missing signals must be an error"),
            (DOMAIN_TYPES_EMPTY_ENTRIES, "empty entries must be an error"),
        ] {
            let outcome = verify_stage2_domain_types(fixture, false, false);
            assert!(outcome.has_errors(), "{message}: {outcome:?}");
        }
    }

    #[test]
    fn test_verify_from_spec_json_with_missing_domain_types_passes_in_interim_mode() {
        // Stage 2 (chain ③): ADR §D2.1: domain-types.json absent = TDDD opt-out.
        // Stage 2 is skipped entirely.
        let dir = tempfile::tempdir().unwrap();
        write_domain_tddd_rules(dir.path());
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        // No domain-types.json — TDDD not active
        let outcome = verify_type_signals_from_spec_json(
            spec_json_path.clone(),
            false,
            dir.path().to_path_buf(),
        );
        assert!(
            !outcome.has_errors(),
            "missing domain-types.json must pass (Stage 2 skip): {outcome:?}"
        );
    }

    #[test]
    fn test_verify_from_spec_json_with_missing_domain_types_passes_in_strict_mode() {
        // Stage 2 (chain ③): same opt-out behavior in strict mode.
        let dir = tempfile::tempdir().unwrap();
        write_domain_tddd_rules(dir.path());
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        let outcome = verify_type_signals_from_spec_json(
            spec_json_path.clone(),
            true,
            dir.path().to_path_buf(),
        );
        assert!(
            !outcome.has_errors(),
            "missing domain-types.json must pass even in strict mode: {outcome:?}"
        );
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
        // Stage 2 (chain ③): Yellow type signal passes in interim mode.
        let dir = tempfile::tempdir().unwrap();
        write_domain_tddd_rules(dir.path());
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_YELLOW_SIGNAL)
            .unwrap();
        write_matching_signal_file(dir.path(), "domain-types.json", "domain-type-signals.json");
        let outcome = verify_type_signals_from_spec_json(
            spec_json_path.clone(),
            false,
            dir.path().to_path_buf(),
        );
        assert!(
            !outcome.has_errors(),
            "yellow signal must pass in default (interim) mode: {outcome:?}"
        );
    }

    #[test]
    fn test_verify_from_spec_json_with_yellow_signal_fails_in_strict_mode() {
        // Stage 2 (chain ③): Yellow type signal fails in strict mode.
        let dir = tempfile::tempdir().unwrap();
        write_domain_tddd_rules(dir.path());
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_YELLOW_SIGNAL)
            .unwrap();
        write_matching_signal_file(dir.path(), "domain-types.json", "domain-type-signals.json");
        let outcome = verify_type_signals_from_spec_json(
            spec_json_path.clone(),
            true,
            dir.path().to_path_buf(),
        );
        assert!(
            outcome.has_errors(),
            "yellow signal must fail in strict (merge) mode: {outcome:?}"
        );
    }

    // --- Stage 1 strict gate: spec.json yellow signals ---

    #[test]
    fn test_verify_from_spec_json_with_spec_yellow_signals_passes_in_default_mode() {
        let dir = tempfile::tempdir().unwrap();
        write_domain_tddd_rules(dir.path());
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_WITH_YELLOW_SIGNALS).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_ALL_BLUE_SIGNALS)
            .unwrap();
        write_matching_signal_file(dir.path(), "domain-types.json", "domain-type-signals.json");
        let outcome =
            verify_from_spec_json(spec_json_path.clone(), false, dir.path().to_path_buf());
        assert!(
            !outcome.has_errors(),
            "spec.json with yellow signals must pass Stage 1 in default (interim) mode: {outcome:?}"
        );
    }

    #[test]
    fn test_verify_from_spec_json_with_spec_yellow_signals_fails_in_strict_mode() {
        let dir = tempfile::tempdir().unwrap();
        write_domain_tddd_rules(dir.path());
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_WITH_YELLOW_SIGNALS).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_ALL_BLUE_SIGNALS)
            .unwrap();
        write_matching_signal_file(dir.path(), "domain-types.json", "domain-type-signals.json");
        let outcome = verify_from_spec_json(spec_json_path.clone(), true, dir.path().to_path_buf());
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
        // Stage 2 (chain ③): undeclared Red type signal must block.
        let dir = tempfile::tempdir().unwrap();
        write_domain_tddd_rules(dir.path());
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        std::fs::write(
            dir.path().join("domain-types.json"),
            DOMAIN_TYPES_WITH_UNDECLARED_RED_SIGNAL,
        )
        .unwrap();
        write_matching_signal_file(dir.path(), "domain-types.json", "domain-type-signals.json");
        let outcome = verify_type_signals_from_spec_json(
            spec_json_path.clone(),
            false,
            dir.path().to_path_buf(),
        );
        assert!(
            outcome.has_errors(),
            "undeclared reverse Red signal must block spec-states (single gate per ADR): {outcome:?}"
        );
    }

    #[test]
    fn test_verify_from_spec_json_with_red_signal_returns_error() {
        // Stage 2 (chain ③): Red type signal must be an error.
        let dir = tempfile::tempdir().unwrap();
        write_domain_tddd_rules(dir.path());
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_RED_SIGNAL).unwrap();
        write_matching_signal_file(dir.path(), "domain-types.json", "domain-type-signals.json");
        let outcome = verify_type_signals_from_spec_json(
            spec_json_path.clone(),
            false,
            dir.path().to_path_buf(),
        );
        assert!(outcome.has_errors(), "red signal must be an error: {outcome:?}");
    }

    #[test]
    fn test_verify_type_signals_from_spec_json_with_invalid_spec_json_returns_error() {
        // Stage 2 (chain ③): spec.json shares the same fail-closed read/decode
        // preamble as Stage 1, even though type-signal evaluation only needs the
        // track directory after that validation succeeds.
        let dir = tempfile::tempdir().unwrap();
        write_domain_tddd_rules(dir.path());
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, "not valid json").unwrap();
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_ALL_BLUE_SIGNALS)
            .unwrap();
        write_matching_signal_file(dir.path(), "domain-types.json", "domain-type-signals.json");
        let outcome = verify_type_signals_from_spec_json(
            spec_json_path.clone(),
            false,
            dir.path().to_path_buf(),
        );
        assert!(outcome.has_errors(), "invalid spec.json must be an error: {outcome:?}");
        assert!(
            outcome.findings().iter().any(|f| f.message().contains("spec.json decode error")),
            "finding must mention spec.json decode error: {outcome:?}"
        );
    }

    #[test]
    fn test_verify_from_spec_json_with_invalid_json_returns_error() {
        // Stage 2 (chain ③): invalid domain-types.json is fail-closed.
        let dir = tempfile::tempdir().unwrap();
        write_domain_tddd_rules(dir.path());
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), "not valid json").unwrap();
        let outcome = verify_type_signals_from_spec_json(
            spec_json_path.clone(),
            false,
            dir.path().to_path_buf(),
        );
        assert!(outcome.has_errors(), "invalid JSON must be an error: {outcome:?}");
    }

    #[test]
    fn test_verify_from_spec_json_with_custom_catalogue_override_red_signal_blocks() {
        // Stage 2 (chain ③): when `architecture-rules.json` uses a non-default
        // `tddd.catalogue_file` override (here: "custom-types.json"), Stage 2
        // still correctly evaluates the overridden file and blocks on a Red signal.
        //
        // T022: check_type_signals no longer receives a filename argument, so the
        // error message contains the type name ("TrackId") instead of the catalogue
        // filename. The test asserts that (a) the gate blocks, and (b) the Red
        // signal type name is present in the finding.
        let dir = tempfile::tempdir().unwrap();
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
        let outcome = verify_type_signals_from_spec_json(
            spec_json_path.clone(),
            false,
            dir.path().to_path_buf(),
        );
        assert!(outcome.has_errors(), "red signal must be an error: {outcome:?}");
        // The error mentions the type name (TrackId) from the Red signal.
        assert!(
            outcome.findings().iter().any(|f| f.message().contains("Red")),
            "finding must mention Red signal: {outcome:?}"
        );
        // T022: check_type_signals no longer receives a filename argument; the
        // diagnostic now identifies the offending type by name. Verify that
        // "TrackId" (the Red-signal type in DOMAIN_TYPES_WITH_RED_SIGNAL) is
        // present in at least one finding.
        assert!(
            outcome.findings().iter().any(|f| f.message().contains("TrackId")),
            "finding must mention the Red-signal type name 'TrackId': {outcome:?}"
        );
    }

    // --- verify() delegation tests ---

    #[test]
    fn test_verify_delegates_to_spec_json_when_sibling_exists() {
        let dir = tempfile::tempdir().unwrap();
        write_domain_tddd_rules(dir.path());
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
    fn test_verify_with_stage1_error_and_stage2_error_merges_findings() {
        let dir = tempfile::tempdir().unwrap();
        write_domain_tddd_rules(dir.path());
        let stale_spec = r#"{
  "schema_version": 2,
  "version": "1.0",
  "title": "Feature",
  "scope": {
    "in_scope": [
      {
        "id": "IS-01",
        "text": "some requirement",
        "adr_refs": [
          { "file": "knowledge/adr/some.md", "anchor": "D1" }
        ]
      }
    ],
    "out_of_scope": []
  },
  "signals": { "blue": 0, "yellow": 0, "red": 1 }
}"#;
        std::fs::write(dir.path().join("spec.json"), stale_spec).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_ALL_BLUE_SIGNALS)
            .unwrap();
        std::fs::write(dir.path().join("spec.md"), "# Overview\n").unwrap();

        let outcome = verify(&dir.path().join("spec.md"), false, dir.path());

        assert_eq!(
            outcome.error_count(),
            2,
            "combined spec.json verification must report both stage errors: {outcome:?}"
        );
        assert!(
            outcome.findings().iter().any(|f| f.message().contains("stale")),
            "Stage 1 stale finding must be retained: {outcome:?}"
        );
        assert!(
            outcome.findings().iter().any(|f| f.message().contains("domain-type-signals.json")
                && f.message().contains("not found")),
            "Stage 2 missing signal-file finding must be retained: {outcome:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_verify_with_spec_json_symlink_reports_shared_preamble_once() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("real-spec.json");
        std::fs::write(&target, SPEC_JSON_MINIMAL).unwrap();
        std::os::unix::fs::symlink(&target, dir.path().join("spec.json")).unwrap();
        std::fs::write(dir.path().join("spec.md"), "# Overview\n").unwrap();

        let outcome = verify(&dir.path().join("spec.md"), false, dir.path());

        assert_eq!(
            outcome.error_count(),
            1,
            "shared spec.json symlink guard failure must not be double-counted: {outcome:?}"
        );
        assert!(
            outcome.findings().iter().any(|f| f.message().contains("symlink")),
            "finding must mention symlink: {outcome:?}"
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

        let outcome = verify_from_spec_json(link.clone(), false, dir.path().to_path_buf());
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
        let outcome = verify_from_spec_json(spec_via_link.clone(), false, dir.path().to_path_buf());
        assert!(outcome.has_errors(), "parent symlink must be rejected: {outcome:?}");
    }

    #[cfg(unix)]
    #[test]
    fn test_verify_from_spec_json_rejects_domain_types_symlink() {
        // S3: spec.json is a regular file but domain-types.json is a symlink — BLOCKED
        // (Stage 2 / chain ③ rejects the symlink catalogue via verify_type_signals_from_spec_json).
        let dir = tempfile::tempdir().unwrap();
        write_domain_tddd_rules(dir.path());
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

        let outcome = verify_type_signals_from_spec_json(
            spec_json_path.clone(),
            false,
            dir.path().to_path_buf(),
        );
        assert!(outcome.has_errors(), "symlink domain-types.json must be rejected: {outcome:?}");
    }

    #[test]
    fn test_verify_from_spec_json_regular_files_pass() {
        // S5 (control): both files are regular, Stage 2 (chain ③) passes.
        let dir = tempfile::tempdir().unwrap();
        write_domain_tddd_rules(dir.path());
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_ALL_BLUE_SIGNALS)
            .unwrap();
        write_matching_signal_file(dir.path(), "domain-types.json", "domain-type-signals.json");
        let outcome = verify_type_signals_from_spec_json(
            spec_json_path.clone(),
            false,
            dir.path().to_path_buf(),
        );
        assert!(!outcome.has_errors(), "regular files must pass: {outcome:?}");
    }

    // --- ADR 2026-04-18-1400 §D5 signal-file evaluation ---

    fn verify_missing_signal_file(strict: bool) -> VerifyOutcome {
        let dir = tempfile::tempdir().unwrap();
        write_domain_tddd_rules(dir.path());
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        // Declaration file present, signal file intentionally absent.
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_ALL_BLUE_SIGNALS)
            .unwrap();

        verify_type_signals_from_spec_json(spec_json_path, strict, dir.path().to_path_buf())
    }

    #[test]
    fn test_signal_file_missing_returns_error_in_interim_mode() {
        // Stage 2 (chain ③): missing signal file is fail-closed per ADR §D5.
        let outcome = verify_missing_signal_file(false);
        assert!(outcome.has_errors(), "missing signal file must be fail-closed: {outcome:?}");
        assert!(
            outcome.findings().iter().any(|f| f.message().contains("domain-type-signals.json")
                && f.message().contains("not found")),
            "finding must name the missing signal file: {outcome:?}"
        );
    }

    #[test]
    fn test_signal_file_missing_returns_error_in_strict_mode() {
        // Stage 2 (chain ③): missing signal file is error in strict mode per ADR §D5.
        let outcome = verify_missing_signal_file(true);
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
    fn test_signal_file_stale_hash_returns_error_in_both_modes() {
        // Stage 2 (chain ③): stale declaration_hash is fail-closed per ADR §D5.
        for strict in [false, true] {
            let outcome = verify_stage2_stale_hash(strict);
            assert!(
                outcome.has_errors(),
                "stale signal file must be an error (strict={strict}): {outcome:?}"
            );
            assert!(
                outcome
                    .findings()
                    .iter()
                    .any(|f| f.message().contains("declaration_hash mismatch")),
                "finding must mention declaration_hash mismatch (strict={strict}): {outcome:?}"
            );
        }
    }

    #[test]
    fn test_signal_file_decode_error_returns_error() {
        // Stage 2 (chain ③): malformed signal file JSON is fail-closed per ADR §D7.
        let dir = tempfile::tempdir().unwrap();
        write_domain_tddd_rules(dir.path());
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
        std::fs::write(dir.path().join("domain-types.json"), DOMAIN_TYPES_WITH_ALL_BLUE_SIGNALS)
            .unwrap();
        std::fs::write(dir.path().join("domain-type-signals.json"), "{not valid json").unwrap();
        let outcome = verify_type_signals_from_spec_json(
            spec_json_path.clone(),
            false,
            dir.path().to_path_buf(),
        );
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
        // Stage 2 (chain ③): schema_version != 1 is a decode error per ADR §D7.
        let dir = tempfile::tempdir().unwrap();
        write_domain_tddd_rules(dir.path());
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
        let outcome = verify_type_signals_from_spec_json(
            spec_json_path.clone(),
            false,
            dir.path().to_path_buf(),
        );
        assert!(outcome.has_errors(), "unknown schema_version must be fail-closed: {outcome:?}");
    }

    #[cfg(unix)]
    #[test]
    fn test_signal_file_symlink_returns_error() {
        // Stage 2 (chain ③): symlink signal file is fail-closed per ADR §D7.
        let dir = tempfile::tempdir().unwrap();
        write_domain_tddd_rules(dir.path());
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

        let outcome = verify_type_signals_from_spec_json(
            spec_json_path.clone(),
            false,
            dir.path().to_path_buf(),
        );
        assert!(outcome.has_errors(), "symlink signal file must be rejected: {outcome:?}");
    }

    #[test]
    fn test_signal_file_matching_hash_and_all_blue_passes_in_both_modes() {
        // Stage 2 (chain ③): fresh signal file with matching hash + all Blue signals
        // passes both CI (strict=false) and merge gate (strict=true) paths.
        for strict in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            write_domain_tddd_rules(dir.path());
            let spec_json_path = dir.path().join("spec.json");
            std::fs::write(&spec_json_path, SPEC_JSON_MINIMAL).unwrap();
            std::fs::write(
                dir.path().join("domain-types.json"),
                DOMAIN_TYPES_WITH_ALL_BLUE_SIGNALS,
            )
            .unwrap();
            write_matching_signal_file(dir.path(), "domain-types.json", "domain-type-signals.json");
            let outcome = verify_type_signals_from_spec_json(
                spec_json_path.clone(),
                strict,
                dir.path().to_path_buf(),
            );
            assert!(
                !outcome.has_errors(),
                "matching signal file with Blue entries must pass (strict={strict}): {outcome:?}"
            );
        }
    }

    #[test]
    fn test_signal_file_overrides_inline_declaration_signals() {
        // Stage 2 (chain ③): when the signal file carries Blue signals but the
        // declaration file has stale Red inline signals, the evaluation result
        // reflects the signal file (Blue → pass).
        let dir = tempfile::tempdir().unwrap();
        write_domain_tddd_rules(dir.path());
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

        let outcome = verify_type_signals_from_spec_json(
            spec_json_path.clone(),
            false,
            dir.path().to_path_buf(),
        );
        assert!(
            !outcome.has_errors(),
            "signal file (Blue) must override inline declaration signals (Red): {outcome:?}"
        );
    }

    // --- Stage 1 self-consistency freshness check (IN-09) ---

    #[test]
    fn test_verify_from_spec_json_stage1_freshness_stale_returns_error() {
        // Stage 1 (chain ①): stored signals differ from computed → stale error (CN-04).
        // Use a spec.json with actual requirements so evaluate_signals() is non-zero.
        let dir = tempfile::tempdir().unwrap();
        // spec.json with one in_scope entry (signal: blue) but stored as red=1.
        let spec_stale = r#"{
  "schema_version": 2,
  "version": "1.0",
  "title": "Feature",
  "scope": {
    "in_scope": [
      {
        "id": "IS-01",
        "text": "some requirement",
        "adr_refs": [
          { "file": "knowledge/adr/some.md", "anchor": "D1" }
        ]
      }
    ],
    "out_of_scope": []
  },
  "signals": { "blue": 0, "yellow": 0, "red": 1 }
}"#;
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, spec_stale).unwrap();
        let outcome =
            verify_from_spec_json(spec_json_path.clone(), false, dir.path().to_path_buf());
        assert!(outcome.has_errors(), "stale Stage 1 signals must be an error: {outcome:?}");
        assert!(
            outcome.findings().iter().any(|f| f.message().contains("stale")),
            "finding must mention stale: {outcome:?}"
        );
        assert!(
            outcome.findings().iter().any(|f| f.message().contains("calc-spec-adr")),
            "finding must suggest calc-spec-adr: {outcome:?}"
        );
    }

    #[test]
    fn test_verify_from_spec_json_stage1_freshness_consistent_passes() {
        // Stage 1 (chain ①): stored signals match computed → no stale error.
        // Use a spec.json where evaluate_signals() matches the stored counts.
        let dir = tempfile::tempdir().unwrap();
        // One blue in_scope entry → evaluate_signals() = blue:1, yellow:0, red:0.
        // Stored signals also blue:1 → consistent.
        let spec_consistent = r#"{
  "schema_version": 2,
  "version": "1.0",
  "title": "Feature",
  "scope": {
    "in_scope": [
      {
        "id": "IS-01",
        "text": "some requirement",
        "adr_refs": [
          { "file": "knowledge/adr/some.md", "anchor": "D1" }
        ]
      }
    ],
    "out_of_scope": []
  },
  "signals": { "blue": 1, "yellow": 0, "red": 0 }
}"#;
        let spec_json_path = dir.path().join("spec.json");
        std::fs::write(&spec_json_path, spec_consistent).unwrap();
        let outcome =
            verify_from_spec_json(spec_json_path.clone(), false, dir.path().to_path_buf());
        assert!(
            !outcome.has_errors(),
            "consistent Stage 1 signals must pass in interim mode: {outcome:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Tests for check_impl_catalog_from_signals_file (T011 explicit-path gate)
    // -----------------------------------------------------------------------

    use crate::verify::test_support::git_init;

    /// Minimal catalogue bytes for a type-signals fixture.
    const MINIMAL_CATALOGUE_JSON: &str = r#"{"schema_version":5,"crate_name":"domain","layer":"domain","types":{"Foo":{"action":"add","role":{"ValueObject":{}},"kind":{"kind":"struct","shape":{"kind":"plain"}},"docs":"A value object."}},"traits":{},"functions":{}}"#;

    /// Build a fresh `<layer>-type-signals.json` JSON string whose
    /// `declaration_hash` matches the given catalogue bytes.
    fn build_fresh_type_signals(catalogue_bytes: &[u8], signal: &str) -> String {
        let hash = crate::tddd::type_signals_codec::declaration_hash(catalogue_bytes);
        // `TypeSignalDto` requires `kind_tag`, `signal`, and `found_type` fields
        // (deny_unknown_fields; missing fields fail decoding).
        format!(
            r#"{{"schema_version":1,"generated_at":"2026-01-01T00:00:00Z","declaration_hash":"{hash}","signals":[{{"type_name":"Foo","kind_tag":"value_object","signal":"{signal}","found_type":true}}]}}"#,
        )
    }

    /// Set up a minimal git repo containing catalogue + type-signals files.
    ///
    /// Returns `(TempDir, signals_path)`.
    fn setup_type_signals_git_repo(signal: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        git_init(dir.path());
        let catalogue_bytes = MINIMAL_CATALOGUE_JSON.as_bytes();
        std::fs::write(dir.path().join("domain-types.json"), catalogue_bytes).unwrap();
        let signals_json = build_fresh_type_signals(catalogue_bytes, signal);
        let signals_path = dir.path().join("domain-type-signals.json");
        std::fs::write(&signals_path, signals_json).unwrap();
        (dir, signals_path)
    }

    #[test]
    fn test_check_impl_catalog_blue_signal_non_strict_passes() {
        let (_dir, signals_path) = setup_type_signals_git_repo("blue");
        let catalog_hash =
            crate::tddd::type_signals_codec::declaration_hash(MINIMAL_CATALOGUE_JSON.as_bytes());

        let outcome = check_impl_catalog_from_signals_file(&signals_path, &catalog_hash, false);

        assert!(
            !outcome.has_errors(),
            "blue signal with correct hash must pass (non-strict): {outcome:?}"
        );
    }

    #[test]
    fn test_check_impl_catalog_stale_hash_returns_error() {
        let (_dir, signals_path) = setup_type_signals_git_repo("blue");
        let stale_hash = "0000000000000000000000000000000000000000000000000000000000000000";

        let outcome = check_impl_catalog_from_signals_file(&signals_path, stale_hash, false);

        let has_mismatch =
            outcome.findings().iter().any(|f| f.message().contains("declaration_hash mismatch"));
        assert!(
            has_mismatch,
            "stale catalog_hash must produce a declaration_hash mismatch error: {outcome:?}"
        );
    }

    #[test]
    fn test_check_impl_catalog_signals_file_not_found_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        git_init(dir.path());
        let missing_path = dir.path().join("domain-type-signals.json");
        let any_hash = "0000000000000000000000000000000000000000000000000000000000000000";

        let outcome = check_impl_catalog_from_signals_file(&missing_path, any_hash, false);

        assert!(outcome.has_errors(), "missing signals file must return an error: {outcome:?}");
    }

    #[test]
    fn test_check_impl_catalog_yellow_strict_returns_error() {
        let (_dir, signals_path) = setup_type_signals_git_repo("yellow");
        let catalog_hash =
            crate::tddd::type_signals_codec::declaration_hash(MINIMAL_CATALOGUE_JSON.as_bytes());

        let outcome = check_impl_catalog_from_signals_file(&signals_path, &catalog_hash, true);

        let has_error =
            outcome.findings().iter().any(|f| f.severity() == domain::verify::Severity::Error);
        assert!(
            has_error,
            "yellow signal with strict=true must produce an error finding: {outcome:?}"
        );
    }
}
