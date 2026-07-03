//! Pure helper functions for the pre-review gate use case.
//!
//! Extracted from `super` (`pre_review_gate.rs`) to keep the parent module
//! under the workspace `verify-module-size` cap (700 non-test lines, see
//! `2026-06-06-1609-enforce-module-size-limit-splitting.md`). The helpers are
//! free functions with no shared mutable state — moving them into a sibling
//! file is a pure refactor (call sites use `use helpers::*;` in the parent so
//! identifiers resolve unchanged).
//!
//! All helpers are `pub(super)`: they are part of the gate's private API and
//! must not be reused outside `pre_review_gate.rs`.
//!
//! Grouping rationale:
//! - `blocked_coverage_outcome` / `blocked_outcome`: thin wrappers that surface
//!   the domain-level `*Outcome::blocked` invariant failures as
//!   `PreReviewGateError::TaskContractReadFailed`. Co-located because both
//!   gates need them and the wrapping pattern is identical.
//! - `build_scope_entries`: signal-document → entry-key map projection used by
//!   the coverage and liveness checks. No outcome-producing logic.
//! - `collect_per_layer_violations` / `collect_non_canonical_layer_violations`
//!   / `collect_task_key_ri_violations`: the three coverage-violation
//!   collectors, kept together because they share the same contract /
//!   signal-entry inspection idiom.
//! - `entry_key_to_contracted_ref`: used by the liveness check to recover a
//!   typed `ContractedEntryRef` from a raw entry key when populating
//!   `NonBlueSignal` / `InvalidEntryRef` violation data.

use std::collections::{HashMap, HashSet};

use domain::TypeSignalsDocument;
use domain::task_contract::ContractedEntryRef;

use super::{
    CANONICAL_LAYERS, CoverageVerifyOutcome, PreReviewGateError, PreReviewGateOutcome,
    PreReviewGateViolation,
};

pub(super) fn blocked_coverage_outcome(
    violations: Vec<domain::task_contract::CoverageViolation>,
) -> Result<CoverageVerifyOutcome, PreReviewGateError> {
    CoverageVerifyOutcome::blocked(violations).map_err(|_| {
        PreReviewGateError::TaskContractReadFailed {
            message: "coverage verify blocked outcome invariant failed".to_owned(),
        }
    })
}

pub(super) fn blocked_outcome(
    violations: Vec<PreReviewGateViolation>,
) -> Result<PreReviewGateOutcome, PreReviewGateError> {
    PreReviewGateOutcome::blocked(violations).map_err(|_| {
        PreReviewGateError::TaskContractReadFailed {
            message: "pre-review gate blocked outcome invariant failed".to_owned(),
        }
    })
}

/// Build `entry_key -> ContractedEntryRef` per layer.
///
/// ADR `2026-06-27-0852-pre-review-task-contract-conformance-gate.md` D1/D3/D4/D9
/// requires that "型カタログの全エントリが漏れなくタスクに帰属" — every catalogue entry
/// must be counted, without silent exclusions. `kind: "unknown"` rows (newly added
/// types not yet registered in the catalogue) are included so that coverage's
/// `OrphanEntry` detection surfaces them at pre-review time; silently skipping
/// them was the pattern explicitly rejected as Alternative AB in the same ADR
/// ("silently 無視すると stale entry が catalogue 全集合の attribution カバレッジ判定に
/// 含まれず、catalogue にあるはずの entry が attribution されていない bug を覆い隠す。
/// fail-closed が安全").
pub(super) fn build_scope_entries(
    signal_doc: &TypeSignalsDocument,
    layer: &domain::tddd::LayerId,
) -> Result<HashMap<String, ContractedEntryRef>, PreReviewGateError> {
    let mut entries: HashMap<String, ContractedEntryRef> = HashMap::new();
    for signal in signal_doc.signals() {
        let entry_key = domain::tddd::semantic_verify::CatalogueEntryKey::try_new(
            signal.type_name().to_owned(),
        )
        .map_err(|_| PreReviewGateError::SignalReadFailed {
            layer: layer.clone(),
            message: format!(
                "invalid entry key '{}' in {}-type-signals.json",
                signal.type_name(),
                layer.as_ref()
            ),
        })?;
        let key = entry_key.as_str().to_owned();
        entries.entry(key).or_insert_with(|| ContractedEntryRef::new(layer.clone(), entry_key));
    }
    Ok(entries)
}

/// Phase 1+2: orphan detection + entry-key RI for one canonical layer.
pub(super) fn collect_per_layer_violations(
    contract_doc: &domain::task_contract::TaskContractDocument,
    layer: &domain::tddd::LayerId,
    scope_entries: &HashMap<String, ContractedEntryRef>,
) -> Vec<domain::task_contract::CoverageViolation> {
    let attributed: Vec<&ContractedEntryRef> =
        contract_doc.entries().values().flatten().filter(|e| e.layer() == layer).collect();
    let attributed_keys: HashSet<&str> =
        attributed.iter().map(|e| e.entry_key().as_str()).collect();
    let mut out = Vec::new();
    for (key, entry) in scope_entries {
        if !attributed_keys.contains(key.as_str()) {
            out.push(domain::task_contract::CoverageViolation::OrphanEntry {
                entry: entry.clone(),
            });
        }
    }
    for entry in &attributed {
        let key = entry.entry_key().as_str();
        if !scope_entries.contains_key(key) {
            out.push(domain::task_contract::CoverageViolation::InvalidEntryRef {
                entry: (*entry).clone(),
                reason: format!(
                    "entry_key '{}' not found in {}-type-signals.json",
                    key,
                    layer.as_ref()
                ),
            });
        }
    }
    out
}

/// Phase 3: any contract entry whose layer is outside the 6 canonical TDDD set.
pub(super) fn collect_non_canonical_layer_violations(
    contract_doc: &domain::task_contract::TaskContractDocument,
) -> Vec<domain::task_contract::CoverageViolation> {
    let canonical: HashSet<&str> = CANONICAL_LAYERS.iter().copied().collect();
    let mut out = Vec::new();
    for refs in contract_doc.entries().values() {
        for entry in refs {
            if !canonical.contains(entry.layer().as_ref()) {
                out.push(domain::task_contract::CoverageViolation::InvalidEntryRef {
                    entry: entry.clone(),
                    reason: format!(
                        "layer '{}' is not a canonical TDDD layer",
                        entry.layer().as_ref()
                    ),
                });
            }
        }
    }
    out
}

/// Phase 4 (D9): task keys present in `task-contract.json` but absent from
/// `impl-plan.json` — emit one `InvalidTaskRef` per stale task so the gate
/// fails closed instead of silently passing stale attributions.
pub(super) fn collect_task_key_ri_violations(
    contract_doc: &domain::task_contract::TaskContractDocument,
    plan_task_ids: &HashMap<domain::TaskId, domain::TaskStatusKind>,
) -> Vec<domain::task_contract::CoverageViolation> {
    contract_doc
        .entries()
        .iter()
        .filter(|(task_id, _)| !plan_task_ids.contains_key(task_id))
        .map(|(task_id, refs)| domain::task_contract::CoverageViolation::InvalidTaskRef {
            task_id: task_id.clone(),
            entry_keys: refs.clone(),
        })
        .collect()
}

/// Extract a `ContractedEntryRef` for an `entry_key` from the contract document
/// for the given layer. Used by `check_signal_document` to produce violation data.
///
/// Returns the first matching `ContractedEntryRef` found in the document.
/// Fails with `SignalReadFailed` if the key is not found (should be unreachable
/// since we iterate from the contract's own entries, but guards against logic errors).
pub(super) fn entry_key_to_contracted_ref(
    contract_doc: &domain::task_contract::TaskContractDocument,
    layer: &domain::tddd::LayerId,
    key: &str,
) -> Result<ContractedEntryRef, PreReviewGateError> {
    contract_doc
        .entries()
        .values()
        .flatten()
        .find(|e| e.layer() == layer && e.entry_key().as_str() == key)
        .cloned()
        .ok_or_else(|| PreReviewGateError::TaskContractReadFailed {
            message: format!(
                "internal error: entry_key '{key}' not found in contract for layer '{}'",
                layer.as_ref()
            ),
        })
}
