//! Pre-review contract conformance gate use case.
//!
//! Implements the three-phase gate that verifies all contracted catalogue entries
//! for a given TDDD layer have blue `impl_catalog` signals before allowing review
//! to proceed (ADR `knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md`).
//!
//! ## Gate logic (three phases, per layer)
//!
//! 1. **Scope discovery + orphan detection**: reads the `TypeSignalsDocument` for
//!    the layer derived from `cmd.layer`, derives the set of scope-relevant catalogue
//!    entry keys from that document, then checks that every scope-relevant signal
//!    entry is covered by at least one task attribution in `task-contract.json`.
//!    Uncovered entries produce [`PreReviewGateViolation::OrphanEntry`].
//!
//! 2. **Referential integrity**: verifies that every contracted entry for
//!    the layer exists in the `TypeSignalsDocument`. Non-existent entries
//!    produce [`PreReviewGateViolation::InvalidEntryRef`].
//!
//! 3. **Signal check**: verifies that every contracted entry that exists in the
//!    `TypeSignalsDocument` has an `impl_catalog` signal of `Blue`. Non-blue
//!    entries produce [`PreReviewGateViolation::NonBlueSignal`].
//!
//! When `cmd.layer` is `None`, all 6 canonical TDDD layers are iterated and the
//! outcomes are combined into a single result. Layers reported missing by the
//! signal reader are skipped silently — that is "no entries to verify", not an
//! error — while other signal read or validation failures still propagate.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use domain::ConfidenceSignal;
use domain::TypeSignalsDocument;
use domain::task_contract::ContractedEntryRef;
// Re-export domain task_contract types accessible to the cli_driver primary adapter
// via usecase module path (architecture-rules.json: cli_driver may_depend_on [usecase] only).
pub use domain::task_contract::{PreReviewGateOutcome, PreReviewGateViolation};
use thiserror::Error;

// ---------------------------------------------------------------------------
// PreReviewGateCommand
// ---------------------------------------------------------------------------

/// CQRS command for the pre-review gate check use case.
///
/// `track_id` identifies the active track whose `task-contract.json` is
/// evaluated. `layer` is the optional TDDD layer scope:
/// - `Some(layer_id)` → check only the given layer (per-layer mode).
/// - `None` → iterate all 6 canonical TDDD layers and combine their outcomes
///   (all-layers mode).
///
/// Both fields are domain value objects: `TrackId` enforces non-empty
/// identity; `LayerId` constrains the gate to layer scopes that have
/// `<layer>-type-signals.json` documents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreReviewGateCommand {
    /// The active track whose task-contract.json is evaluated.
    pub track_id: domain::TrackId,
    /// The TDDD layer to check, or `None` to iterate all 6 canonical layers.
    pub layer: Option<domain::tddd::LayerId>,
}

// ---------------------------------------------------------------------------
// PreReviewGateError
// ---------------------------------------------------------------------------

/// Error type returned by [`PreReviewGateService::check`].
///
/// - `TaskContractNotFound`: the `task-contract.json` for the given `track_id`
///   does not exist (gate short-circuits to `MissingTaskContract` violation).
/// - `TaskContractReadFailed`: I/O or decode error reading the contract;
///   `message` is an opaque diagnostic string (R9: opaque infrastructure error message).
/// - `SignalReadFailed`: I/O or decode error reading the per-layer type-signals
///   document; `layer` is typed as `domain::tddd::LayerId` (the port takes
///   `&LayerId` so the error always originates from a valid `LayerId`), `message`
///   is an opaque diagnostic string.
///
/// Gate violations (`NonBlueSignal` etc.) are NOT errors — they are data inside
/// [`PreReviewGateOutcome::Blocked`].
#[derive(Debug, Error)]
pub enum PreReviewGateError {
    /// The `task-contract.json` for the given track does not exist.
    #[error("task-contract.json not found for track")]
    TaskContractNotFound,

    /// I/O or decode error reading the `task-contract.json`.
    #[error("failed to read task-contract.json: {message}")]
    TaskContractReadFailed {
        /// Opaque diagnostic message from the infrastructure adapter.
        message: String,
    },

    /// I/O or decode error reading the per-layer `<layer>-type-signals.json`.
    #[error("failed to read type-signals for layer '{layer}': {message}")]
    SignalReadFailed {
        /// The TDDD layer whose signal document could not be read.
        layer: domain::tddd::LayerId,
        /// Opaque diagnostic message from the infrastructure adapter.
        message: String,
    },
}

// ---------------------------------------------------------------------------
// Secondary ports
// ---------------------------------------------------------------------------

/// Secondary port for reading a `task-contract.json` domain document.
///
/// Implemented by `infrastructure::task_contract_reader::FsTaskContractReader`.
pub trait TaskContractReaderPort: Send + Sync {
    /// Read the `task-contract.json` for the given track.
    ///
    /// Returns [`PreReviewGateError::TaskContractNotFound`] when the file does not
    /// exist; [`PreReviewGateError::TaskContractReadFailed`] on I/O or decode errors.
    fn read(
        &self,
        track_id: &domain::TrackId,
    ) -> Result<domain::task_contract::TaskContractDocument, PreReviewGateError>;
}

/// Secondary port for reading a per-layer `<layer>-type-signals.json` document.
///
/// Implemented by
/// `infrastructure::impl_catalog_signal_reader::FsImplCatalogSignalReader`.
pub trait ImplCatalogSignalReaderPort: Send + Sync {
    /// Read the per-layer `impl_catalog` type-signals document for the given track
    /// and layer.
    ///
    /// Returns [`PreReviewGateError::SignalReadFailed`] on I/O or decode errors.
    fn read_signals(
        &self,
        track_id: &domain::TrackId,
        layer: &domain::tddd::LayerId,
    ) -> Result<TypeSignalsDocument, PreReviewGateError>;

    /// Read the per-layer signal document when absence is an expected state.
    ///
    /// Returns `Ok(None)` only when the adapter can positively classify the
    /// document as absent. Other read/decode failures must return
    /// [`PreReviewGateError::SignalReadFailed`]. The default implementation is
    /// fail-closed for adapters that only implement [`Self::read_signals`].
    fn read_optional_signals(
        &self,
        track_id: &domain::TrackId,
        layer: &domain::tddd::LayerId,
    ) -> Result<Option<TypeSignalsDocument>, PreReviewGateError> {
        self.read_signals(track_id, layer).map(Some)
    }
}

// ---------------------------------------------------------------------------
// PreReviewGateService (primary application service port)
// ---------------------------------------------------------------------------

/// Primary application service port for the pre-review gate use case.
///
/// Called by `cli_driver::task_contract::TaskContractDriver` when handling
/// `TaskContractInput::Check`.
pub trait PreReviewGateService: Send + Sync {
    /// Run the pre-review gate check for the active track.
    ///
    /// Returns [`PreReviewGateOutcome::Passed`] with a conformance summary, or
    /// [`PreReviewGateOutcome::Blocked`] with a list of violations.
    ///
    /// # Errors
    ///
    /// Returns [`PreReviewGateError`] on infrastructure read failures.
    fn check(&self, cmd: PreReviewGateCommand) -> Result<PreReviewGateOutcome, PreReviewGateError>;
}

// ---------------------------------------------------------------------------
// PreReviewGateInteractor
// ---------------------------------------------------------------------------

/// Interactor implementing [`PreReviewGateService`].
///
/// Holds two injected secondary ports: `task_contract_reader` reads the
/// `task-contract.json` for the active track; `signal_reader` reads per-layer
/// `impl_catalog` type-signal documents. The interactor joins task attribution
/// from the contract document with blue/non-blue classification from the
/// type-signals document to produce a [`PreReviewGateOutcome`].
pub struct PreReviewGateInteractor {
    task_contract_reader: Arc<dyn TaskContractReaderPort>,
    signal_reader: Arc<dyn ImplCatalogSignalReaderPort>,
}

impl PreReviewGateInteractor {
    /// Construct a `PreReviewGateInteractor` by injecting the two secondary ports.
    #[must_use]
    pub fn new(
        task_contract_reader: Arc<dyn TaskContractReaderPort>,
        signal_reader: Arc<dyn ImplCatalogSignalReaderPort>,
    ) -> Self {
        Self { task_contract_reader, signal_reader }
    }
}

/// Conformance summary for per-layer (single-layer) gate passes.
const SINGLE_LAYER_SUMMARY: &str = "宣言した API surface が型契約と shape 一致（body は未検証 — stub / liveness は reviewer が確認）";

/// Conformance summary for all-layer gate passes.
const ALL_LAYERS_SUMMARY: &str = "全 6 TDDD レイヤー（domain / usecase / infrastructure / cli_driver / cli / cli_composition）\
の API surface が型契約と shape 一致（body は未検証 — stub / liveness は reviewer が確認）";

/// Canonical TDDD layer identifiers iterated in all-layers mode.
const CANONICAL_LAYERS: &[&str] =
    &["domain", "usecase", "infrastructure", "cli_driver", "cli", "cli_composition"];

fn blocked_outcome(
    violations: Vec<PreReviewGateViolation>,
) -> Result<PreReviewGateOutcome, PreReviewGateError> {
    PreReviewGateOutcome::blocked(violations).map_err(|_| {
        PreReviewGateError::TaskContractReadFailed {
            message: "pre-review gate blocked outcome invariant failed".to_owned(),
        }
    })
}

impl PreReviewGateInteractor {
    /// Evaluate one already-loaded signal document against the task contract.
    ///
    /// Returns the list of violations found for this layer (empty = passed).
    /// The caller is responsible for combining per-layer results into a final
    /// [`PreReviewGateOutcome`].
    fn check_signal_document(
        &self,
        layer: &domain::tddd::LayerId,
        contract_doc: &domain::task_contract::TaskContractDocument,
        signal_doc: &TypeSignalsDocument,
    ) -> Result<Vec<PreReviewGateViolation>, PreReviewGateError> {
        // ── Build lookup structures ───────────────────────────────────────────
        //
        // scope_signals: type_name -> ConfidenceSignal for entries in the signal doc.
        // Validate entry-key shape up front so malformed signal documents fail closed.
        let mut scope_signals: HashMap<String, ConfidenceSignal> = HashMap::new();
        let mut scope_entries: HashMap<String, ContractedEntryRef> = HashMap::new();
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
            scope_entries
                .entry(key.clone())
                .or_insert_with(|| ContractedEntryRef::new(layer.clone(), entry_key));
            scope_signals.insert(key, signal.signal());
        }

        // attributed_keys: set of entry_key strings contracted to layer
        let attributed_entries: Vec<&ContractedEntryRef> =
            contract_doc.entries().values().flatten().filter(|e| e.layer() == layer).collect();

        let attributed_keys: HashSet<&str> =
            attributed_entries.iter().map(|e| e.entry_key().as_str()).collect();

        // ── Phase 1: Scope discovery + orphan detection ───────────────────────
        //
        // Every scope-relevant signal entry must have at least one task attribution.
        let mut violations: Vec<PreReviewGateViolation> = Vec::new();

        for (type_name, entry) in &scope_entries {
            if !attributed_keys.contains(type_name.as_str()) {
                violations.push(PreReviewGateViolation::OrphanEntry { entry: entry.clone() });
            }
        }

        // ── Phase 2: Referential integrity ────────────────────────────────────
        //
        // Every contracted entry for layer must exist in the signal document.
        for entry in &attributed_entries {
            let key = entry.entry_key().as_str();
            if !scope_signals.contains_key(key) {
                violations.push(PreReviewGateViolation::InvalidEntryRef {
                    entry: (*entry).clone(),
                    reason: format!(
                        "entry_key '{}' not found in {}-type-signals.json",
                        key,
                        layer.as_ref()
                    ),
                });
            }
        }

        // ── Phase 3: Signal check ─────────────────────────────────────────────
        //
        // Every contracted entry that exists in the signal document must have Blue signal.
        for entry in &attributed_entries {
            let key = entry.entry_key().as_str();
            if let Some(&signal) = scope_signals.get(key) {
                if signal != ConfidenceSignal::Blue {
                    violations.push(PreReviewGateViolation::NonBlueSignal {
                        entry: (*entry).clone(),
                        signal,
                    });
                }
            }
        }

        Ok(violations)
    }

    /// Run the three-phase gate for a single TDDD layer.
    ///
    /// Returns the list of violations found for this layer (empty = passed).
    /// The caller is responsible for combining per-layer results into a final
    /// [`PreReviewGateOutcome`].
    fn check_layer(
        &self,
        track_id: &domain::TrackId,
        layer: &domain::tddd::LayerId,
        contract_doc: &domain::task_contract::TaskContractDocument,
    ) -> Result<Vec<PreReviewGateViolation>, PreReviewGateError> {
        // ── Step 1: read type-signals for layer ───────────────────────────────
        let signal_doc = self.signal_reader.read_signals(track_id, layer)?;
        self.check_signal_document(layer, contract_doc, &signal_doc)
    }
}

impl PreReviewGateService for PreReviewGateInteractor {
    fn check(&self, cmd: PreReviewGateCommand) -> Result<PreReviewGateOutcome, PreReviewGateError> {
        // ── Step 0: read task-contract.json ──────────────────────────────────
        //
        // TaskContractNotFound short-circuits to MissingTaskContract violation.
        let contract_doc = match self.task_contract_reader.read(&cmd.track_id) {
            Ok(doc) => doc,
            Err(PreReviewGateError::TaskContractNotFound) => {
                return blocked_outcome(vec![PreReviewGateViolation::MissingTaskContract]);
            }
            Err(e) => return Err(e),
        };

        match cmd.layer {
            Some(layer) => {
                // ── Per-layer mode ────────────────────────────────────────────
                let violations = self.check_layer(&cmd.track_id, &layer, &contract_doc)?;
                if violations.is_empty() {
                    Ok(PreReviewGateOutcome::Passed {
                        conformance_summary: SINGLE_LAYER_SUMMARY.to_owned(),
                    })
                } else {
                    blocked_outcome(violations)
                }
            }
            None => {
                // ── All-layers mode ───────────────────────────────────────────
                //
                // Iterate all 6 canonical TDDD layers and combine violations.
                // Layers reported missing by the signal reader are skipped
                // silently — that is "no entries to verify", not an error.
                // Other signal read or validation failures still fail closed.
                let mut all_violations: Vec<PreReviewGateViolation> = Vec::new();
                for &layer_str in CANONICAL_LAYERS {
                    let Ok(layer) = domain::tddd::LayerId::try_new(layer_str.to_owned()) else {
                        // Unreachable: CANONICAL_LAYERS contains only valid identifiers.
                        continue;
                    };
                    match self.signal_reader.read_optional_signals(&cmd.track_id, &layer)? {
                        Some(signal_doc) => {
                            let violations =
                                self.check_signal_document(&layer, &contract_doc, &signal_doc)?;
                            all_violations.extend(violations);
                        }
                        None => {
                            // No signal document for this layer — skip silently.
                        }
                    }
                }
                if all_violations.is_empty() {
                    Ok(PreReviewGateOutcome::Passed {
                        conformance_summary: ALL_LAYERS_SUMMARY.to_owned(),
                    })
                } else {
                    blocked_outcome(all_violations)
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests (AC-07)
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::Arc;

    use domain::task_contract::{
        ContractedEntryRef, PreReviewGateOutcome, PreReviewGateViolation, TaskContractDocument,
    };
    use domain::tddd::semantic_verify::CatalogueEntryKey;
    use domain::tddd::{LayerId, type_signals_doc::TypeSignalsDocument};
    use domain::{ConfidenceSignal, TaskId, Timestamp, TrackId, TypeSignal};

    use super::{
        ImplCatalogSignalReaderPort, PreReviewGateCommand, PreReviewGateError,
        PreReviewGateInteractor, PreReviewGateService, TaskContractReaderPort,
    };

    // ── Mock helpers ──────────────────────────────────────────────────────────

    fn layer(s: &str) -> LayerId {
        LayerId::try_new(s.to_owned()).unwrap()
    }

    fn entry_key(s: &str) -> CatalogueEntryKey {
        CatalogueEntryKey::try_new(s.to_owned()).unwrap()
    }

    fn task_id(s: &str) -> TaskId {
        TaskId::try_new(s).unwrap()
    }

    fn track_id(s: &str) -> TrackId {
        TrackId::try_new(s).unwrap()
    }

    fn ts(s: &str) -> Timestamp {
        Timestamp::new(s).unwrap()
    }

    fn blue_signal(name: &str) -> TypeSignal {
        TypeSignal::new(name, "struct", ConfidenceSignal::Blue, true, vec![], vec![], vec![])
    }

    fn yellow_signal(name: &str) -> TypeSignal {
        TypeSignal::new(name, "struct", ConfidenceSignal::Yellow, false, vec![], vec![], vec![])
    }

    fn make_contract(
        track: &str,
        entries: Vec<(TaskId, Vec<ContractedEntryRef>)>,
    ) -> TaskContractDocument {
        let mut map = BTreeMap::new();
        for (tid, refs) in entries {
            map.insert(tid, refs);
        }
        TaskContractDocument::new(track_id(track), map).unwrap()
    }

    fn make_signals(signals: Vec<TypeSignal>) -> TypeSignalsDocument {
        TypeSignalsDocument::new(ts("2026-06-27T00:00:00Z"), "hash", signals)
    }

    // ── Mock implementations ──────────────────────────────────────────────────

    struct ConstContractReader(Result<TaskContractDocument, PreReviewGateError>);

    impl TaskContractReaderPort for ConstContractReader {
        fn read(
            &self,
            _track_id: &TrackId,
        ) -> Result<domain::task_contract::TaskContractDocument, PreReviewGateError> {
            match &self.0 {
                Ok(doc) => Ok(doc.clone()),
                Err(PreReviewGateError::TaskContractNotFound) => {
                    Err(PreReviewGateError::TaskContractNotFound)
                }
                Err(PreReviewGateError::TaskContractReadFailed { message }) => {
                    Err(PreReviewGateError::TaskContractReadFailed { message: message.clone() })
                }
                Err(PreReviewGateError::SignalReadFailed { layer, message }) => {
                    Err(PreReviewGateError::SignalReadFailed {
                        layer: layer.clone(),
                        message: message.clone(),
                    })
                }
            }
        }
    }

    struct ConstSignalReader(Result<TypeSignalsDocument, PreReviewGateError>);

    impl ImplCatalogSignalReaderPort for ConstSignalReader {
        fn read_signals(
            &self,
            _track_id: &TrackId,
            _layer: &LayerId,
        ) -> Result<TypeSignalsDocument, PreReviewGateError> {
            match &self.0 {
                Ok(doc) => Ok(doc.clone()),
                Err(PreReviewGateError::SignalReadFailed { layer, message }) => {
                    Err(PreReviewGateError::SignalReadFailed {
                        layer: layer.clone(),
                        message: message.clone(),
                    })
                }
                Err(e) => {
                    Err(PreReviewGateError::TaskContractReadFailed { message: e.to_string() })
                }
            }
        }
    }

    /// Layer-aware signal reader: returns the document registered for the requested
    /// layer, or typed absence if no document is registered for that layer.
    struct LayerAwareSignalReader(std::collections::HashMap<String, TypeSignalsDocument>);

    impl ImplCatalogSignalReaderPort for LayerAwareSignalReader {
        fn read_signals(
            &self,
            _track_id: &TrackId,
            layer: &LayerId,
        ) -> Result<TypeSignalsDocument, PreReviewGateError> {
            match self.0.get(layer.as_ref()) {
                Some(doc) => Ok(doc.clone()),
                None => Err(PreReviewGateError::SignalReadFailed {
                    layer: layer.clone(),
                    message: format!("no signal document for layer '{}'", layer.as_ref()),
                }),
            }
        }

        fn read_optional_signals(
            &self,
            _track_id: &TrackId,
            layer: &LayerId,
        ) -> Result<Option<TypeSignalsDocument>, PreReviewGateError> {
            Ok(self.0.get(layer.as_ref()).cloned())
        }
    }

    struct FailingSignalReader {
        message: &'static str,
    }

    impl ImplCatalogSignalReaderPort for FailingSignalReader {
        fn read_signals(
            &self,
            _track_id: &TrackId,
            layer: &LayerId,
        ) -> Result<TypeSignalsDocument, PreReviewGateError> {
            Err(PreReviewGateError::SignalReadFailed {
                layer: layer.clone(),
                message: self.message.to_owned(),
            })
        }
    }

    fn interactor(
        contract: Result<TaskContractDocument, PreReviewGateError>,
        signals: Result<TypeSignalsDocument, PreReviewGateError>,
    ) -> PreReviewGateInteractor {
        PreReviewGateInteractor::new(
            Arc::new(ConstContractReader(contract)),
            Arc::new(ConstSignalReader(signals)),
        )
    }

    fn cmd(track: &str, group: &str) -> PreReviewGateCommand {
        PreReviewGateCommand { track_id: track_id(track), layer: Some(layer(group)) }
    }

    // ── AC-07 (a): TaskContractNotFound → Blocked/MissingTaskContract ─────────

    #[test]
    fn missing_task_contract_yields_blocked_with_missing_task_contract() {
        let svc = interactor(
            Err(PreReviewGateError::TaskContractNotFound),
            Ok(make_signals(vec![blue_signal("Foo")])),
        );
        let outcome = svc.check(cmd("my-track", "domain")).unwrap();
        match outcome {
            PreReviewGateOutcome::Blocked { violations, .. } => {
                assert_eq!(violations.len(), 1);
                assert!(matches!(violations[0], PreReviewGateViolation::MissingTaskContract));
            }
            other => panic!("expected Blocked, got {other:?}"),
        }
    }

    // ── AC-07 (b): signal doc entry absent from task attribution → OrphanEntry ─

    #[test]
    fn signal_entry_not_attributed_yields_orphan_entry_violation() {
        // Signal doc has "Foo" in domain layer.
        // task-contract.json has no attribution for "Foo" in domain.
        let svc = interactor(
            Ok(make_contract(
                "my-track",
                vec![(
                    task_id("T001"),
                    vec![ContractedEntryRef::new(layer("domain"), entry_key("Bar"))],
                )],
            )),
            Ok(make_signals(vec![blue_signal("Foo"), blue_signal("Bar")])),
        );
        let outcome = svc.check(cmd("my-track", "domain")).unwrap();
        match outcome {
            PreReviewGateOutcome::Blocked { violations, .. } => {
                let orphans: Vec<_> = violations
                    .iter()
                    .filter(|v| matches!(v, PreReviewGateViolation::OrphanEntry { .. }))
                    .collect();
                assert!(
                    !orphans.is_empty(),
                    "expected at least one OrphanEntry violation, got: {violations:?}"
                );
                let orphan_keys: Vec<&str> = orphans
                    .iter()
                    .filter_map(|v| {
                        if let PreReviewGateViolation::OrphanEntry { entry } = v {
                            Some(entry.entry_key().as_str())
                        } else {
                            None
                        }
                    })
                    .collect();
                assert!(
                    orphan_keys.contains(&"Foo"),
                    "Foo must be the orphan, got: {orphan_keys:?}"
                );
            }
            other => panic!("expected Blocked, got {other:?}"),
        }
    }

    #[test]
    fn test_check_invalid_signal_entry_key_returns_signal_read_failed() {
        let invalid_signal =
            TypeSignal::new("   ", "struct", ConfidenceSignal::Blue, true, vec![], vec![], vec![]);
        let svc = interactor(
            Ok(make_contract(
                "my-track",
                vec![(
                    task_id("T001"),
                    vec![ContractedEntryRef::new(layer("domain"), entry_key("Foo"))],
                )],
            )),
            Ok(make_signals(vec![blue_signal("Foo"), invalid_signal])),
        );
        let err = svc.check(cmd("my-track", "domain")).unwrap_err();
        match err {
            PreReviewGateError::SignalReadFailed { layer, message } => {
                assert_eq!(layer.as_ref(), "domain");
                assert!(
                    message.contains("invalid entry key"),
                    "expected invalid entry key diagnostic, got: {message}"
                );
            }
            other => panic!("expected SignalReadFailed, got {other}"),
        }
    }

    // ── AC-07 (c): contracted key not in signal doc → InvalidEntryRef ─────────

    #[test]
    fn contracted_key_absent_from_signal_doc_yields_invalid_entry_ref() {
        // task-contract.json attributes "Missing" in domain, but signal doc only has "Foo".
        let svc = interactor(
            Ok(make_contract(
                "my-track",
                vec![(
                    task_id("T001"),
                    vec![
                        ContractedEntryRef::new(layer("domain"), entry_key("Foo")),
                        ContractedEntryRef::new(layer("domain"), entry_key("Missing")),
                    ],
                )],
            )),
            Ok(make_signals(vec![blue_signal("Foo")])),
        );
        let outcome = svc.check(cmd("my-track", "domain")).unwrap();
        match outcome {
            PreReviewGateOutcome::Blocked { violations, .. } => {
                let invalid: Vec<_> = violations
                    .iter()
                    .filter(|v| matches!(v, PreReviewGateViolation::InvalidEntryRef { .. }))
                    .collect();
                assert!(!invalid.is_empty(), "expected InvalidEntryRef violation");
                let invalid_keys: Vec<&str> = invalid
                    .iter()
                    .filter_map(|v| {
                        if let PreReviewGateViolation::InvalidEntryRef { entry, .. } = v {
                            Some(entry.entry_key().as_str())
                        } else {
                            None
                        }
                    })
                    .collect();
                assert!(
                    invalid_keys.contains(&"Missing"),
                    "Missing must be the invalid key, got: {invalid_keys:?}"
                );
            }
            other => panic!("expected Blocked, got {other:?}"),
        }
    }

    // ── AC-07 (d): contracted key has Yellow/Red signal → NonBlueSignal ───────

    #[test]
    fn non_blue_signal_yields_non_blue_signal_violation() {
        let svc = interactor(
            Ok(make_contract(
                "my-track",
                vec![(
                    task_id("T001"),
                    vec![ContractedEntryRef::new(layer("domain"), entry_key("Foo"))],
                )],
            )),
            Ok(make_signals(vec![yellow_signal("Foo")])),
        );
        let outcome = svc.check(cmd("my-track", "domain")).unwrap();
        match outcome {
            PreReviewGateOutcome::Blocked { violations, .. } => {
                assert_eq!(violations.len(), 1);
                match &violations[0] {
                    PreReviewGateViolation::NonBlueSignal { entry, signal } => {
                        assert_eq!(entry.entry_key().as_str(), "Foo");
                        assert_eq!(*signal, ConfidenceSignal::Yellow);
                    }
                    other => panic!("expected NonBlueSignal, got {other:?}"),
                }
            }
            other => panic!("expected Blocked, got {other:?}"),
        }
    }

    // ── AC-07 (e): all blue + complete attribution → Passed with D5 summary ───

    #[test]
    fn all_blue_and_complete_attribution_yields_passed_with_d5_summary() {
        let svc = interactor(
            Ok(make_contract(
                "my-track",
                vec![(
                    task_id("T001"),
                    vec![ContractedEntryRef::new(layer("domain"), entry_key("Foo"))],
                )],
            )),
            Ok(make_signals(vec![blue_signal("Foo")])),
        );
        let outcome = svc.check(cmd("my-track", "domain")).unwrap();
        match outcome {
            PreReviewGateOutcome::Passed { conformance_summary } => {
                assert!(
                    conformance_summary.contains("型契約と shape 一致"),
                    "expected D5 summary to contain '型契約と shape 一致', got: {conformance_summary}"
                );
                assert!(
                    conformance_summary.contains("未検証"),
                    "expected D5 summary to contain '未検証', got: {conformance_summary}"
                );
            }
            other => panic!("expected Passed, got {other:?}"),
        }
    }

    // ── All-layers mode: None layer → iterate all 6 TDDD layers ─────────────

    #[test]
    fn all_layer_iterate_passes_when_two_layers_are_blue_and_complete() {
        // Domain has Foo (blue, attributed T001); usecase has Bar (blue, attributed T001).
        // The other 4 canonical layers are reported as missing → skipped silently.
        let mut signal_docs = std::collections::HashMap::new();
        signal_docs.insert("domain".to_owned(), make_signals(vec![blue_signal("Foo")]));
        signal_docs.insert("usecase".to_owned(), make_signals(vec![blue_signal("Bar")]));

        let svc = PreReviewGateInteractor::new(
            Arc::new(ConstContractReader(Ok(make_contract(
                "my-track",
                vec![(
                    task_id("T001"),
                    vec![
                        ContractedEntryRef::new(layer("domain"), entry_key("Foo")),
                        ContractedEntryRef::new(layer("usecase"), entry_key("Bar")),
                    ],
                )],
            )))),
            Arc::new(LayerAwareSignalReader(signal_docs)),
        );

        let outcome = svc
            .check(PreReviewGateCommand { track_id: track_id("my-track"), layer: None })
            .unwrap();

        match outcome {
            PreReviewGateOutcome::Passed { conformance_summary } => {
                assert!(
                    conformance_summary.contains("全 6"),
                    "all-layer summary must mention all 6 layers, got: {conformance_summary}"
                );
                assert!(
                    conformance_summary.contains("shape 一致"),
                    "all-layer summary must contain D5 phrasing 'shape 一致', got: {conformance_summary}"
                );
                assert!(
                    conformance_summary.contains("未検証"),
                    "all-layer summary must contain D5 phrasing '未検証', got: {conformance_summary}"
                );
            }
            other => panic!("expected Passed, got {other:?}"),
        }
    }

    #[test]
    fn test_all_layer_iterate_malformed_signal_document_returns_signal_read_failed() {
        let invalid_signal =
            TypeSignal::new("   ", "struct", ConfidenceSignal::Blue, true, vec![], vec![], vec![]);
        let mut signal_docs = std::collections::HashMap::new();
        signal_docs.insert("domain".to_owned(), make_signals(vec![invalid_signal]));

        let svc = PreReviewGateInteractor::new(
            Arc::new(ConstContractReader(Ok(make_contract(
                "my-track",
                vec![(
                    task_id("T001"),
                    vec![ContractedEntryRef::new(layer("domain"), entry_key("Foo"))],
                )],
            )))),
            Arc::new(LayerAwareSignalReader(signal_docs)),
        );

        let err = svc
            .check(PreReviewGateCommand { track_id: track_id("my-track"), layer: None })
            .unwrap_err();

        match err {
            PreReviewGateError::SignalReadFailed { layer, message } => {
                assert_eq!(layer.as_ref(), "domain");
                assert!(
                    message.contains("invalid entry key"),
                    "expected malformed signal document to propagate, got: {message}"
                );
            }
            other => panic!("expected SignalReadFailed, got {other}"),
        }
    }

    #[test]
    fn test_all_layer_iterate_non_missing_signal_read_failed_returns_error() {
        let svc = PreReviewGateInteractor::new(
            Arc::new(ConstContractReader(Ok(make_contract(
                "my-track",
                vec![(
                    task_id("T001"),
                    vec![ContractedEntryRef::new(layer("domain"), entry_key("Foo"))],
                )],
            )))),
            Arc::new(FailingSignalReader {
                message: "codec error reading domain-type-signals.json",
            }),
        );

        let err = svc
            .check(PreReviewGateCommand { track_id: track_id("my-track"), layer: None })
            .unwrap_err();

        match err {
            PreReviewGateError::SignalReadFailed { layer, message } => {
                assert_eq!(layer.as_ref(), "domain");
                assert!(
                    message.contains("codec error"),
                    "expected non-missing signal read failure to propagate, got: {message}"
                );
            }
            other => panic!("expected SignalReadFailed, got {other}"),
        }
    }

    #[test]
    fn test_all_layer_iterate_missing_like_signal_read_failed_returns_error() {
        let svc = PreReviewGateInteractor::new(
            Arc::new(ConstContractReader(Ok(make_contract(
                "my-track",
                vec![(
                    task_id("T001"),
                    vec![ContractedEntryRef::new(layer("domain"), entry_key("Foo"))],
                )],
            )))),
            Arc::new(FailingSignalReader {
                message: "signal file not found: codec emitted misleading diagnostic",
            }),
        );

        let err = svc
            .check(PreReviewGateCommand { track_id: track_id("my-track"), layer: None })
            .unwrap_err();

        match err {
            PreReviewGateError::SignalReadFailed { layer, message } => {
                assert_eq!(layer.as_ref(), "domain");
                assert!(
                    message.contains("signal file not found"),
                    "expected original diagnostic to propagate, got: {message}"
                );
            }
            other => panic!("expected SignalReadFailed, got {other}"),
        }
    }
}
