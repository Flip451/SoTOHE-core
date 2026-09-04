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
//!   the domain-level `*Outcome::blocked` invariant failures as gate errors.
//!   Co-located because both gates need them and the wrapping pattern is
//!   identical.
//! - `build_scope_entries`: signal-document → entry-key map projection used by
//!   the coverage and liveness checks. No outcome-producing logic.
//! - `collect_per_layer_violations` / `collect_non_canonical_layer_violations`
//!   / `collect_task_key_ri_violations`: the three coverage-violation
//!   collectors, kept together because they share the same contract /
//!   signal-entry inspection idiom.
//! - `resolve_scope_entry_key`: shared namespace-aware join used by the
//!   coverage and liveness checks.

use std::collections::{HashMap, HashSet};
use std::path::{Component, Path};

use domain::ConfidenceSignal;
use domain::TrackId;
use domain::TypeSignalsDocument;
use domain::task_contract::ContractedEntryRef;
use domain::tddd::LayerId;
use domain::tddd::catalogue_linter::FreeText;
use domain::tddd::catalogue_v2::CatalogueDocument;
use domain::tddd::catalogue_v2::catalogue_impl_signals_ports::{
    AttestedCatalogueDocument, TdddLayerBindingsPort,
};
use domain::tddd::catalogue_v2::identifiers::CatalogueItemNamespace;
use domain::tddd::catalogue_v2::identity_resolution::resolve_contract_entry_namespace;

use super::{
    CANONICAL_LAYERS, CoverageVerifyOutcome, PreReviewGateError, PreReviewGateOutcome,
    PreReviewGateViolation,
};
use crate::catalogue_document_loader::AttestedCatalogueDocumentLoaderPort;

pub(super) fn blocked_coverage_outcome(
    violations: Vec<domain::task_contract::CoverageViolation>,
) -> Result<CoverageVerifyOutcome, PreReviewGateError> {
    CoverageVerifyOutcome::blocked(violations).map_err(|_| {
        PreReviewGateError::TaskContractReadFailed {
            message: FreeText::new("coverage verify blocked outcome invariant failed"),
        }
    })
}

pub(super) fn blocked_outcome(
    violations: Vec<PreReviewGateViolation>,
) -> Result<PreReviewGateOutcome, PreReviewGateError> {
    PreReviewGateOutcome::blocked(violations).map_err(|_| {
        PreReviewGateError::TaskContractReadFailed {
            message: FreeText::new("pre-review gate blocked outcome invariant failed"),
        }
    })
}

/// Identity key for a persisted signal row.
///
/// Namespace-less rows represent report labels (functions / trait-impls),
/// while catalogue type and trait rows retain their namespace even when their
/// display names are equal.
pub(super) type ScopeEntryKey = (String, Option<CatalogueItemNamespace>);

/// Result of resolving one task-contract entry against signal identities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ScopeEntryResolution {
    /// The contract entry identifies exactly one persisted signal row.
    Matched(ScopeEntryKey),
    /// No persisted signal row has the requested key and namespace.
    Missing,
    /// The catalogue cannot assign one namespace to the contract key.
    Invalid(FreeText),
}

/// Ensure that the two filesystem roots supplied to the gate identify one
/// repository context before any catalogue binding is resolved.
///
/// The artifact readers discover the repository from their injected
/// `items_dir`, while the layer-binding port receives `workspace_root` from
/// the composition root. An absolute items directory outside that root would
/// therefore let one request combine artifacts and architecture rules from
/// different repositories. Relative paths are interpreted by the composition
/// root's repository context; parent traversal is rejected because it would
/// escape that context.
fn validate_items_dir_scope(workspace_root: &Path, items_dir: &Path) -> Result<(), String> {
    if items_dir.is_absolute() {
        if !workspace_root.is_absolute() || !items_dir.starts_with(workspace_root) {
            return Err(format!(
                "items directory '{}' is outside workspace root '{}', so repository inputs cannot be mixed",
                items_dir.display(),
                workspace_root.display()
            ));
        }
        return Ok(());
    }

    if items_dir.components().any(|component| matches!(component, Component::ParentDir)) {
        return Err(format!(
            "items directory '{}' escapes workspace root '{}', so repository inputs cannot be mixed",
            items_dir.display(),
            workspace_root.display()
        ));
    }

    Ok(())
}

/// Resolve one task-contract entry against namespace-bearing signal keys.
///
/// Task-contract schema v1 stores the exact catalogue entry key but has no
/// namespace field. Resolve that key through the catalogue's type/trait/
/// function sections before joining it to a persisted signal identity.
pub(super) fn resolve_scope_entry_key(
    entry: &ContractedEntryRef,
    catalogue: &CatalogueDocument,
    scope_keys: &HashSet<ScopeEntryKey>,
) -> ScopeEntryResolution {
    let raw_key = entry.entry_key().as_str();
    let namespace = match resolve_contract_entry_namespace(catalogue, entry.entry_key()) {
        Ok(namespace) => namespace,
        Err(error) => return ScopeEntryResolution::Invalid(FreeText::new(error.to_string())),
    };
    let key = (raw_key.to_owned(), namespace);
    if scope_keys.contains(&key) {
        ScopeEntryResolution::Matched(key)
    } else {
        ScopeEntryResolution::Missing
    }
}

/// Load the catalogue for one canonical layer through the existing domain port.
///
/// The loader returns the document with the declaration hash of the bytes it
/// decoded. Compare that request-local attestation with the hash carried by the
/// signal document before namespace resolution can continue.
pub(super) fn load_catalogue(
    loader: &dyn AttestedCatalogueDocumentLoaderPort,
    layer_bindings: &dyn TdddLayerBindingsPort,
    workspace_root: &Path,
    items_dir: &Path,
    track_id: &TrackId,
    layer: &LayerId,
    signal_doc: &TypeSignalsDocument,
) -> Result<AttestedCatalogueDocument, PreReviewGateError> {
    validate_items_dir_scope(workspace_root, items_dir).map_err(|message| {
        PreReviewGateError::CatalogueReadFailed {
            layer: layer.clone(),
            message: FreeText::new(message),
        }
    })?;

    let catalogue_file = layer_bindings
        .load(workspace_root, Some(layer.as_ref()))
        .map_err(|error| PreReviewGateError::CatalogueReadFailed {
            layer: layer.clone(),
            message: FreeText::new(format!("failed to resolve layer binding: {error}")),
        })?
        .into_iter()
        .next()
        .map(|binding| binding.catalogue_file)
        .ok_or_else(|| PreReviewGateError::CatalogueReadFailed {
            layer: layer.clone(),
            message: FreeText::new("layer binding did not provide a catalogue filename"),
        })?;
    let path = items_dir.join(track_id.as_ref()).join(catalogue_file);
    let attested = loader.load(&path).map_err(|error| PreReviewGateError::CatalogueReadFailed {
        layer: layer.clone(),
        message: FreeText::new(error.to_string()),
    })?;
    if attested.declaration_hash() != signal_doc.cache_key().declaration_hash() {
        return Err(PreReviewGateError::CatalogueFreshnessMismatch {
            layer: layer.clone(),
            message: FreeText::new(
                "catalogue changed between signal validation and namespace resolution",
            ),
        });
    }
    Ok(attested)
}

/// Evaluates one loaded signal document against the task contract's namespace-
/// aware identities and task-status rules.
pub(super) fn check_signal_document(
    layer: &LayerId,
    contract_doc: &domain::task_contract::TaskContractDocument,
    catalogue: &CatalogueDocument,
    signal_doc: &TypeSignalsDocument,
    task_statuses: &HashMap<domain::TaskId, domain::TaskStatusKind>,
) -> Result<Vec<PreReviewGateViolation>, PreReviewGateError> {
    // A namespace-less key is reserved for function / trait-impl report labels.
    let mut scope_signals: HashMap<ScopeEntryKey, ConfidenceSignal> = HashMap::new();
    for signal in signal_doc.signals() {
        let entry_key = domain::tddd::semantic_verify::CatalogueEntryKey::try_new(
            signal.type_name().to_owned(),
        )
        .map_err(|_| PreReviewGateError::SignalReadFailed {
            layer: layer.clone(),
            message: FreeText::new(format!(
                "invalid entry key '{}' in {}-type-signals.json",
                signal.type_name(),
                layer.as_ref()
            )),
        })?;
        let identity_key = (entry_key.as_str().to_owned(), signal.identity().namespace());
        if scope_signals.insert(identity_key, signal.signal()).is_some() {
            return Err(PreReviewGateError::SignalReadFailed {
                layer: layer.clone(),
                message: FreeText::new(format!(
                    "duplicate signal identity '{}' in {}-type-signals.json",
                    signal.type_name(),
                    layer.as_ref()
                )),
            });
        }
    }

    // The catalogue determines whether each bare contract key is a type, trait,
    // or namespace-less function/report label.
    let scope_keys: HashSet<ScopeEntryKey> = scope_signals.keys().cloned().collect();
    let mut entry_task_statuses: HashMap<
        ScopeEntryKey,
        (Vec<domain::TaskStatusKind>, domain::task_contract::ContractedEntryRef),
    > = HashMap::new();
    for (task_id, refs) in contract_doc.entries() {
        let status = task_statuses.get(task_id).copied().unwrap_or(domain::TaskStatusKind::Done);
        for entry_ref in refs {
            if entry_ref.layer() != layer {
                continue;
            }
            match resolve_scope_entry_key(entry_ref, catalogue, &scope_keys) {
                ScopeEntryResolution::Matched(key) => {
                    let entry_statuses = entry_task_statuses
                        .entry(key)
                        .or_insert_with(|| (Vec::new(), entry_ref.clone()));
                    entry_statuses.0.push(status);
                }
                ScopeEntryResolution::Missing => {}
                ScopeEntryResolution::Invalid(reason) => {
                    return Err(PreReviewGateError::TaskContractReadFailed {
                        message: FreeText::new(format!(
                            "entry_key '{}' has no unique catalogue namespace in {}-types.json: {}",
                            entry_ref.entry_key().as_str(),
                            layer.as_ref(),
                            reason.as_str()
                        )),
                    });
                }
            }
        }
    }

    let mut violations = Vec::new();
    for (key, signal) in &scope_signals {
        let Some((statuses, entry_ref)) = entry_task_statuses.get(key) else {
            continue;
        };
        let is_red = *signal == ConfidenceSignal::Red;
        let requires_blue = statuses.iter().any(|status| {
            matches!(status, domain::TaskStatusKind::InProgress | domain::TaskStatusKind::Done)
        });
        if is_red || (requires_blue && *signal != ConfidenceSignal::Blue) {
            violations.push(PreReviewGateViolation::NonBlueSignal(entry_ref.clone(), *signal));
        }
    }
    Ok(violations)
}

/// Build `(entry_key, namespace) -> ContractedEntryRef` per layer.
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
) -> Result<HashMap<ScopeEntryKey, ContractedEntryRef>, PreReviewGateError> {
    let mut entries: HashMap<ScopeEntryKey, ContractedEntryRef> = HashMap::new();
    for signal in signal_doc.signals() {
        let entry_key = domain::tddd::semantic_verify::CatalogueEntryKey::try_new(
            signal.type_name().to_owned(),
        )
        .map_err(|_| PreReviewGateError::SignalReadFailed {
            layer: layer.clone(),
            message: FreeText::new(format!(
                "invalid entry key '{}' in {}-type-signals.json",
                signal.type_name(),
                layer.as_ref()
            )),
        })?;
        let key = (entry_key.as_str().to_owned(), signal.identity().namespace());
        if entries.contains_key(&key) {
            return Err(PreReviewGateError::SignalReadFailed {
                layer: layer.clone(),
                message: FreeText::new(format!(
                    "duplicate signal identity '{}' in {}-type-signals.json",
                    signal.type_name(),
                    layer.as_ref()
                )),
            });
        }
        entries.insert(key, ContractedEntryRef::new(layer.clone(), entry_key));
    }
    Ok(entries)
}

/// Phase 1+2: orphan detection + entry-key RI for one canonical layer.
pub(super) fn collect_per_layer_violations(
    contract_doc: &domain::task_contract::TaskContractDocument,
    layer: &domain::tddd::LayerId,
    catalogue: &CatalogueDocument,
    scope_entries: &HashMap<ScopeEntryKey, ContractedEntryRef>,
) -> Vec<domain::task_contract::CoverageViolation> {
    // `ContractedEntryRef` v1 carries only the entry key, while signal rows
    // may carry a type/trait namespace. A key shared by multiple namespaces
    // therefore cannot be joined safely: treating the key as a wildcard would
    // make one contract attribution cover every same-named row. Keep unique
    // keys compatible with the v1 contract, but fail closed for ambiguous
    // keys until the contract can identify a namespace explicitly.
    let attributed: Vec<&ContractedEntryRef> =
        contract_doc.entries().values().flatten().filter(|e| e.layer() == layer).collect();
    let scope_keys: HashSet<ScopeEntryKey> = scope_entries.keys().cloned().collect();
    let mut attributed_keys: HashSet<ScopeEntryKey> = HashSet::new();
    let mut unresolved = Vec::new();
    for entry in &attributed {
        match resolve_scope_entry_key(entry, catalogue, &scope_keys) {
            ScopeEntryResolution::Matched(key) => {
                attributed_keys.insert(key);
            }
            ScopeEntryResolution::Missing => {
                unresolved.push(domain::task_contract::CoverageViolation::InvalidEntryRef(
                    (*entry).clone(),
                    FreeText::new(format!(
                        "entry_key '{}' not found in {}-type-signals.json",
                        entry.entry_key().as_str(),
                        layer.as_ref()
                    )),
                ));
            }
            ScopeEntryResolution::Invalid(reason) => {
                unresolved.push(domain::task_contract::CoverageViolation::InvalidEntryRef(
                    (*entry).clone(),
                    FreeText::new(format!(
                        "entry_key '{}' has no unique catalogue namespace in {}: {}",
                        entry.entry_key().as_str(),
                        layer.as_ref(),
                        reason.as_str()
                    )),
                ));
            }
        }
    }

    let mut out = Vec::new();
    for (key, entry) in scope_entries {
        if !attributed_keys.contains(key) {
            out.push(domain::task_contract::CoverageViolation::OrphanEntry(entry.clone()));
        }
    }
    out.extend(unresolved);
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
                out.push(domain::task_contract::CoverageViolation::InvalidEntryRef(
                    entry.clone(),
                    FreeText::new(format!(
                        "layer '{}' is not a canonical TDDD layer",
                        entry.layer().as_ref()
                    )),
                ));
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
        .map(|(task_id, refs)| {
            domain::task_contract::CoverageViolation::InvalidTaskRef(task_id.clone(), refs.clone())
        })
        .collect()
}
