//! `ref_verify` command family — primary adapter driver.
//!
//! `RefVerifyDriver` holds a single injected `RefVerifyAggregateService` and
//! exposes `handle(input) -> CommandOutcome`. One injected interactor — no
//! per-service fields (D3/D4 cli_driver policy).

use std::path::PathBuf;
use std::sync::Arc;

use usecase::LayerId;
use usecase::ref_verify::{
    RefVerifyAggregateService, RefVerifyChainFilter, RefVerifyCheckApprovedOutcome,
    RefVerifyDriverError, RefVerifyLayerFilter, RefVerifyResultsOutput, RefVerifyRunOutcome,
    RefVerifyVerdictFilter, SemanticVerdict, VerifyOriginRef,
};

use crate::render::CommandOutcome;

// ---------------------------------------------------------------------------
// Input types
// ---------------------------------------------------------------------------

/// Input DTO for `sotp ref-verify run`.
#[derive(Debug, Clone)]
pub struct RefVerifyRunInput {
    /// Track ID whose semantic references should be verified.
    pub track_id: String,
    /// Path to the track items directory (e.g. `track/items`).
    pub items_dir: PathBuf,
}

/// Input DTO for `sotp ref-verify check-approved`.
#[derive(Debug, Clone)]
pub struct RefVerifyCheckApprovedInput {
    /// Track ID whose semantic references should be checked.
    pub track_id: String,
    /// Path to the track items directory (e.g. `track/items`).
    pub items_dir: PathBuf,
}

/// Chain-filter selection for the `ref-verify results` command.
///
/// Defined in the cli_driver layer so the cli layer can construct
/// [`RefVerifyResultsInput`] without depending on the usecase crate directly.
/// [`RefVerifyDriver::handle`] converts this to
/// [`usecase::ref_verify::RefVerifyChainFilter`] before calling the service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefVerifyChainSelect {
    /// Include only Chain-1 (spec↔ADR) pairs.
    Chain1,
    /// Include only Chain-2 (catalogue↔spec) pairs.
    Chain2,
    /// Include both Chain-1 and Chain-2 pairs.
    All,
}

/// Verdict-filter selection for the `ref-verify results` command.
///
/// `FailPending` represents omitted CLI `--filter` and preserves the ADR
/// default record block; explicit CLI values map to
/// `Pass`/`Fail`/`Pending`/`All` for `--filter {pass|fail|pending|all}`.
/// Defined in the cli_driver layer so the cli layer can construct
/// [`RefVerifyResultsInput`] without depending on the usecase crate directly.
/// [`RefVerifyDriver::handle`] converts this to
/// [`usecase::ref_verify::RefVerifyVerdictFilter`] before calling the service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefVerifyVerdictSelect {
    /// Include fail and pending records (default when `--filter` is omitted).
    FailPending,
    /// Include only pass records.
    Pass,
    /// Include only fail records.
    Fail,
    /// Include only pending records.
    Pending,
    /// Include all records regardless of verdict.
    All,
}

/// Input DTO for `sotp ref-verify results`.
///
/// `track_id` is already resolved from the branch when omitted on the CLI.
/// `chain` and `verdict` are typed cli_driver-level selects (no usecase import
/// required at the cli layer). `layer` is a plain `String` (`'all'` or a layer
/// name) because valid layer names are dynamic (from `architecture-rules.json`)
/// and cannot be captured in a closed enum at this layer; the driver ignores
/// this raw layer string when `chain=Chain1` because Chain-1 has no layer
/// dimension.
#[derive(Debug, Clone)]
pub struct RefVerifyResultsInput {
    /// Track ID whose results should be displayed.
    pub track_id: String,
    /// Path to the track items directory (e.g. `track/items`).
    pub items_dir: PathBuf,
    /// Which chain(s) to include.
    pub chain: RefVerifyChainSelect,
    /// Raw layer string: `'all'` or a valid layer name.
    pub layer: String,
    /// Which verdict class to include in the record block.
    pub verdict: RefVerifyVerdictSelect,
}

/// Typed input for the `ref_verify` command family.
///
/// Extended with a `Results` variant to carry the structured input for
/// `sotp ref-verify results`.
pub enum RefVerifyInput {
    /// Run semantic reference verification.
    Run(RefVerifyRunInput),
    /// Check whether all production reference pairs have verified Pass cache entries.
    CheckApproved(RefVerifyCheckApprovedInput),
    /// Display cached verify results filtered by chain, layer, and verdict.
    Results(RefVerifyResultsInput),
}

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

/// Primary adapter driver for the `ref_verify` command family.
///
/// Holds a single injected `RefVerifyAggregateService`; exposes
/// `handle(input) -> CommandOutcome`. One injected interactor — no per-service
/// fields (D3/D4 cli_driver policy). The match body gains a `Results` arm in
/// this track; the method signature is unchanged.
pub struct RefVerifyDriver {
    service: Arc<dyn RefVerifyAggregateService>,
}

impl RefVerifyDriver {
    /// Create a new `RefVerifyDriver` with a single injected aggregate service.
    pub fn new(service: Arc<dyn RefVerifyAggregateService>) -> Self {
        Self { service }
    }

    /// Handle a ref_verify command by dispatching to the corresponding service method.
    pub fn handle(&self, input: RefVerifyInput) -> CommandOutcome {
        match input {
            RefVerifyInput::Run(input) => self.ref_verify_run(input),
            RefVerifyInput::CheckApproved(input) => self.ref_verify_check_approved(input),
            RefVerifyInput::Results(input) => self.ref_verify_results(input),
        }
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn ref_verify_run(&self, input: RefVerifyRunInput) -> CommandOutcome {
        match self.service.run(&input.track_id, &input.items_dir) {
            Ok(RefVerifyRunOutcome::Passed) => CommandOutcome::success(Some(
                "[OK] Semantic reference verification passed — all pairs verified.".to_owned(),
            )),
            Ok(RefVerifyRunOutcome::SemanticFailuresConfirmed { pair_count }) => CommandOutcome {
                stdout: None,
                stderr: Some(format!(
                    "[BLOCKED] Semantic review confirmed {pair_count} production failure(s). \
                     Resolve the failures before committing."
                )),
                exit_code: 1,
            },
            Ok(RefVerifyRunOutcome::HumanEscalationRequired { pair_count }) => CommandOutcome {
                stdout: None,
                stderr: Some(format!(
                    "[ESCALATE] Human review required for {pair_count} unresolved pair(s) \
                     or known-bad detection failure."
                )),
                exit_code: 1,
            },
            Err(RefVerifyDriverError::Wiring(msg)) => {
                CommandOutcome::failure(Some(format!("ref-verify run failed (wiring): {msg}")))
            }
            Err(e) => CommandOutcome::failure(Some(format!("ref-verify run failed: {e}"))),
        }
    }

    fn ref_verify_check_approved(&self, input: RefVerifyCheckApprovedInput) -> CommandOutcome {
        match self.service.check_approved(&input.track_id, &input.items_dir) {
            Ok(RefVerifyCheckApprovedOutcome::NoPairs) => CommandOutcome::success(Some(
                "[OK] No production reference pairs found — check-approved gate passes.".to_owned(),
            )),
            Ok(RefVerifyCheckApprovedOutcome::AllApproved) => CommandOutcome::success(Some(
                "[OK] All production reference pairs have verified Pass cache entries.".to_owned(),
            )),
            Ok(RefVerifyCheckApprovedOutcome::NotApproved { missing_or_non_pass }) => {
                CommandOutcome {
                    stdout: None,
                    stderr: Some(format!(
                        "[BLOCKED] ref-verify check-approved failed: {} pair(s) without Pass cache:\n{}",
                        missing_or_non_pass.len(),
                        missing_or_non_pass.join("\n")
                    )),
                    exit_code: 1,
                }
            }
            Err(RefVerifyDriverError::Wiring(msg)) => CommandOutcome::failure(Some(format!(
                "ref-verify check-approved failed (wiring): {msg}"
            ))),
            Err(e) => {
                CommandOutcome::failure(Some(format!("ref-verify check-approved failed: {e}")))
            }
        }
    }

    fn ref_verify_results(&self, input: RefVerifyResultsInput) -> CommandOutcome {
        let chain_filter = chain_select_to_filter(input.chain.clone());

        let layer_filter = match layer_string_to_filter(&input.chain, &input.layer) {
            Ok(f) => f,
            Err(msg) => {
                return CommandOutcome::failure(Some(format!("ref-verify results: {msg}")));
            }
        };

        let verdict_filter = verdict_select_to_filter(input.verdict);

        match self.service.results(
            &input.track_id,
            &input.items_dir,
            chain_filter,
            layer_filter,
            verdict_filter,
        ) {
            Ok(output) => CommandOutcome::success(Some(render_results_text(&output))),
            Err(RefVerifyDriverError::Wiring(msg)) => {
                CommandOutcome::failure(Some(format!("ref-verify results failed (wiring): {msg}")))
            }
            Err(e) => CommandOutcome::failure(Some(format!("ref-verify results failed: {e}"))),
        }
    }
}

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

fn chain_select_to_filter(chain: RefVerifyChainSelect) -> RefVerifyChainFilter {
    match chain {
        RefVerifyChainSelect::Chain1 => RefVerifyChainFilter::Chain1,
        RefVerifyChainSelect::Chain2 => RefVerifyChainFilter::Chain2,
        RefVerifyChainSelect::All => RefVerifyChainFilter::All,
    }
}

fn verdict_select_to_filter(verdict: RefVerifyVerdictSelect) -> RefVerifyVerdictFilter {
    match verdict {
        RefVerifyVerdictSelect::FailPending => RefVerifyVerdictFilter::FailPending,
        RefVerifyVerdictSelect::Pass => RefVerifyVerdictFilter::Pass,
        RefVerifyVerdictSelect::Fail => RefVerifyVerdictFilter::Fail,
        RefVerifyVerdictSelect::Pending => RefVerifyVerdictFilter::Pending,
        RefVerifyVerdictSelect::All => RefVerifyVerdictFilter::All,
    }
}

/// Convert a raw layer string to [`RefVerifyLayerFilter`].
///
/// - `Chain1`: always returns `All` regardless of `layer` (no-op, AC-08).
/// - `Chain2` / `All`: `"all"` → `All`; any other string is validated as a
///   [`LayerId`] and wrapped in `Specific`.
///
/// Returns `Err` when the layer string fails [`LayerId`] validation for
/// Chain-2 / All chains.
fn layer_string_to_filter(
    chain: &RefVerifyChainSelect,
    layer: &str,
) -> Result<RefVerifyLayerFilter, String> {
    match chain {
        RefVerifyChainSelect::Chain1 => Ok(RefVerifyLayerFilter::All),
        RefVerifyChainSelect::Chain2 | RefVerifyChainSelect::All => {
            if layer == "all" {
                Ok(RefVerifyLayerFilter::All)
            } else {
                LayerId::try_new(layer.to_owned())
                    .map(RefVerifyLayerFilter::Specific)
                    .map_err(|e| format!("invalid --layer value '{layer}': {e}"))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Rendering helpers
// ---------------------------------------------------------------------------

fn render_results_text(output: &RefVerifyResultsOutput) -> String {
    let mut text = String::new();

    // Header block: one line per lane summary.
    for lane in &output.lane_summaries {
        text.push_str(&format!(
            "  {}: pass={} fail={} pending={}\n",
            lane.label, lane.pass_count, lane.fail_count, lane.pending_count
        ));
    }

    // Blank line separating header block from record block.
    text.push('\n');

    // Record block: one entry per pair record.
    for record in &output.pair_records {
        let verdict_label = match &record.verdict {
            SemanticVerdict::Pass { .. } => "pass",
            SemanticVerdict::Fail { .. } => "fail",
            SemanticVerdict::Pending => "pending",
        };
        text.push_str(&format!(
            "claim: {}\nevidence: {}\nverdict: {}\nreason: {}\nchain+layer: {}\nclaim_origin: {}\nevidence_origin: {}\n",
            record.claim_hash.to_hex(),
            record.evidence_hash.to_hex(),
            verdict_label,
            record.reason,
            record.chain_layer,
            format_origin_display(&record.claim_origin),
            format_origin_display(&record.evidence_origin),
        ));
    }

    // Summary line (always present, exit code is always 0 per CN-02).
    let total = output.total_pass + output.total_fail + output.total_pending;
    text.push_str(&format!(
        "Summary: {} pass, {} fail, {} pending, {} total",
        output.total_pass, output.total_fail, output.total_pending, total
    ));

    text
}

/// Format a [`VerifyOriginRef`] as a human-readable display string.
///
/// - `SpecElement` → `spec:{section_kind}:{element_id}:{text_label_truncated}`
/// - `AdrDecision` → `adr:{file_path}#{decision_id}`
/// - `CatalogueEntry` → `catalogue:{file_path}:{section_key}:{entry_key}`
///
/// Text labels longer than 40 characters are truncated to 40 characters.
fn format_origin_display(origin: &VerifyOriginRef) -> String {
    match origin {
        VerifyOriginRef::SpecElement(r) => {
            let section = format!("{:?}", r.section);
            let element_id = r.element_id.as_ref();
            let text: String = r.text_label.chars().take(40).collect();
            format!("spec:{section}:{element_id}:{text}")
        }
        VerifyOriginRef::AdrDecision(r) => {
            format!("adr:{}#{}", r.file_path, r.decision_id)
        }
        VerifyOriginRef::CatalogueEntry(r) => {
            let section = format!("{:?}", r.section_key);
            format!("catalogue:{}:{}:{}", r.file_path, section, r.entry_key.as_str())
        }
    }
}

// ---------------------------------------------------------------------------
// Legacy helpers (kept for existing callers)
// ---------------------------------------------------------------------------

/// Format a missing-or-non-pass pair entry as a bracketed status string.
///
/// Used when iterating over production_pairs to build the missing_or_non_pass vec.
/// Format: `"pair ({claim_hex}, {evidence_hex}) {reason}"`
pub fn format_pair_status(claim_hex: &str, evidence_hex: &str, reason: &str) -> String {
    format!("pair ({claim_hex}, {evidence_hex}) {reason}")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use usecase::ref_verify::{
        AdrDecisionRef, CatalogueEntryKey, CatalogueEntryRef, CatalogueSectionKey,
        RefVerifyLaneSummary, RefVerifyResultsOutput, SpecElementId, SpecElementRef,
        SpecSectionKind,
    };

    // ── format_origin_display ─────────────────────────────────────────────────

    #[test]
    fn test_format_origin_display_adr_decision() {
        let origin = VerifyOriginRef::AdrDecision(AdrDecisionRef::new(
            "knowledge/adr/my-adr.md".to_owned(),
            "D1".to_owned(),
        ));
        assert_eq!(format_origin_display(&origin), "adr:knowledge/adr/my-adr.md#D1");
    }

    #[test]
    fn test_format_origin_display_spec_element() {
        let element_id = SpecElementId::try_new("IN-01".to_owned()).unwrap();
        let origin = VerifyOriginRef::SpecElement(SpecElementRef::new(
            SpecSectionKind::Goal,
            element_id,
            "some spec text".to_owned(),
        ));
        assert_eq!(format_origin_display(&origin), "spec:Goal:IN-01:some spec text");
    }

    #[test]
    fn test_format_origin_display_spec_element_truncates_long_text() {
        let element_id = SpecElementId::try_new("AC-01".to_owned()).unwrap();
        let long_text = "a".repeat(60);
        let origin = VerifyOriginRef::SpecElement(SpecElementRef::new(
            SpecSectionKind::AcceptanceCriteria,
            element_id,
            long_text,
        ));
        let result = format_origin_display(&origin);
        let prefix = "spec:AcceptanceCriteria:AC-01:";
        assert!(result.starts_with(prefix), "expected prefix '{prefix}' in '{result}'");
        let text_part = &result[prefix.len()..];
        assert_eq!(text_part.chars().count(), 40, "text should be truncated to 40 chars");
    }

    #[test]
    fn test_format_origin_display_catalogue_entry() {
        let entry_key = CatalogueEntryKey::try_new("MyType".to_owned()).unwrap();
        let origin = VerifyOriginRef::CatalogueEntry(CatalogueEntryRef::new(
            "track/items/foo/domain-types.json".to_owned(),
            CatalogueSectionKey::Types,
            entry_key,
        ));
        assert_eq!(
            format_origin_display(&origin),
            "catalogue:track/items/foo/domain-types.json:Types:MyType"
        );
    }

    // ── chain / verdict / layer conversion ───────────────────────────────────

    #[test]
    fn test_chain1_with_named_layer_string_ignored_returns_all() {
        // AC-08: Chain1 always produces RefVerifyLayerFilter::All, ignoring raw layer.
        let result = layer_string_to_filter(&RefVerifyChainSelect::Chain1, "domain");
        assert_eq!(result, Ok(RefVerifyLayerFilter::All));
    }

    #[test]
    fn test_chain1_with_invalid_layer_string_still_returns_all() {
        // Chain1 does NOT validate the layer string even if it would fail LayerId.
        let result = layer_string_to_filter(&RefVerifyChainSelect::Chain1, "123-invalid!");
        assert_eq!(result, Ok(RefVerifyLayerFilter::All));
    }

    #[test]
    fn test_chain2_layer_all_produces_layer_filter_all() {
        let result = layer_string_to_filter(&RefVerifyChainSelect::Chain2, "all");
        assert_eq!(result, Ok(RefVerifyLayerFilter::All));
    }

    #[test]
    fn test_chain2_valid_layer_produces_specific_filter() {
        let result = layer_string_to_filter(&RefVerifyChainSelect::Chain2, "domain");
        let expected =
            RefVerifyLayerFilter::Specific(LayerId::try_new("domain".to_owned()).unwrap());
        assert_eq!(result, Ok(expected));
    }

    #[test]
    fn test_chain_all_invalid_layer_returns_error() {
        let result = layer_string_to_filter(&RefVerifyChainSelect::All, "123invalid!");
        assert!(result.is_err(), "invalid layer name should return Err");
    }

    #[test]
    fn test_verdict_fail_pending_converts_to_filter_fail_pending() {
        let filter = verdict_select_to_filter(RefVerifyVerdictSelect::FailPending);
        assert_eq!(filter, RefVerifyVerdictFilter::FailPending);
    }

    #[test]
    fn test_verdict_all_converts_to_filter_all() {
        let filter = verdict_select_to_filter(RefVerifyVerdictSelect::All);
        assert_eq!(filter, RefVerifyVerdictFilter::All);
    }

    // ── rendering ─────────────────────────────────────────────────────────────

    #[test]
    fn test_render_empty_lane_summaries_and_empty_pair_records() {
        let output = RefVerifyResultsOutput {
            lane_summaries: vec![],
            pair_records: vec![],
            total_pass: 0,
            total_fail: 0,
            total_pending: 0,
        };
        let text = render_results_text(&output);
        assert!(text.starts_with('\n'), "empty header block must produce a leading blank line");
        assert!(
            text.contains("Summary: 0 pass, 0 fail, 0 pending, 0 total"),
            "expected Summary line in: {text}"
        );
    }

    #[test]
    fn test_summary_line_format_with_lane_summary() {
        let output = RefVerifyResultsOutput {
            lane_summaries: vec![RefVerifyLaneSummary {
                label: "Chain1 (spec\u{2194}ADR)".to_owned(),
                pass_count: 2,
                fail_count: 1,
                pending_count: 3,
            }],
            pair_records: vec![],
            total_pass: 2,
            total_fail: 1,
            total_pending: 3,
        };
        let text = render_results_text(&output);
        assert!(
            text.contains("Summary: 2 pass, 1 fail, 3 pending, 6 total"),
            "expected Summary line in: {text}"
        );
        assert!(
            text.contains("  Chain1 (spec\u{2194}ADR): pass=2 fail=1 pending=3"),
            "expected lane summary line in: {text}"
        );
    }
}
