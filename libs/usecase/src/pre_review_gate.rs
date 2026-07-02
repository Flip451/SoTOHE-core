//! Pre-review contract conformance gate use case (D5 split).
//!
//! Provides two complementary use cases:
//!
//! ## Liveness gate (`PreReviewGateService` / `bin/sotp task-contract check`)
//!
//! Verifies that all contracted catalogue entries for tasks that are
//! `in_progress` or `done` have Blue `impl_catalog` signals (D7 status filter).
//! Attributed entries with no `in_progress` or `done` owner tolerate Yellow; Red
//! always blocks regardless of task status. Operates per-layer or across all 6
//! canonical TDDD layers.
//! Non-Blue entries produce [`PreReviewGateViolation::NonBlueSignal`].
//!
//! When `cmd.layer` is `None`, all 6 canonical TDDD layers are iterated and the
//! outcomes are combined into a single result. Layers reported missing by the
//! signal reader are skipped silently — that is "no entries to verify", not an
//! error — while other signal read or validation failures still propagate.
//!
//! ## Attribution-completeness gate (`CoverageVerifyService` / `bin/sotp task-contract coverage`)
//!
//! Verifies attribution completeness across all 6 canonical TDDD layers:
//!
//! 1. **Orphan detection**: every scope-relevant signal entry must be attributed
//!    to at least one task. Uncovered entries produce [`CoverageViolation::OrphanEntry`].
//!
//! 2. **Referential integrity**: every attributed entry must exist in the signal
//!    document. Missing entries produce [`CoverageViolation::InvalidEntryRef`].
//!
//! ADR: `knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md`.

use std::collections::HashMap;
use std::sync::Arc;

use domain::ConfidenceSignal;
use domain::TypeSignalsDocument;
// Re-export domain task_contract types accessible to the cli_driver primary adapter
// via usecase module path (architecture-rules.json: cli_driver may_depend_on [usecase] only).
pub use domain::task_contract::{
    CoverageVerifyOutcome, CoverageViolation, PreReviewGateOutcome, PreReviewGateViolation,
};
use thiserror::Error;

// Pure-helper free functions extracted to a sibling module to keep this file
// under the workspace `verify-module-size` cap (700 non-test lines, see ADR
// `2026-06-06-1609-enforce-module-size-limit-splitting`). The glob import
// keeps call sites unchanged.
mod helpers;
use helpers::{
    blocked_coverage_outcome, blocked_outcome, build_scope_entries,
    collect_non_canonical_layer_violations, collect_per_layer_violations,
    collect_task_key_ri_violations, entry_key_to_contracted_ref,
};

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

/// Error type returned by [`PreReviewGateService::check`] and
/// [`CoverageVerifyService::verify_coverage`].
///
/// Both services share this error type because they share the same secondary ports.
///
/// - `TaskContractNotFound`: the `task-contract.json` for the given `track_id`
///   does not exist. D9 (knowledge/adr/2026-06-26-0503-...) tolerance: both
///   `PreReviewGateInteractor::check` and `CoverageVerifyInteractor::verify_coverage`
///   short-circuit to `Passed` (no contract → nothing to verify). The
///   `MissingTaskContract` enum variant is retained for future refinement
///   (e.g. enforcing the gate when `impl-plan.json` exists).
/// - `TaskContractReadFailed`: I/O or decode error reading the contract;
///   `message` is an opaque diagnostic string (R9: opaque infrastructure error message).
/// - `SignalReadFailed`: I/O or decode error reading the per-layer type-signals
///   document; `layer` is typed as `domain::tddd::LayerId` (the port takes
///   `&LayerId` so the error always originates from a valid `LayerId`), `message`
///   is an opaque diagnostic string.
/// - `ImplPlanReadFailed`: I/O or decode error reading `impl-plan.json` (D7:
///   added for [`ImplPlanReaderPort`]); `message` is an opaque diagnostic string.
///
/// Gate violations (`NonBlueSignal`, `OrphanEntry` etc.) are NOT errors — they
/// are data inside [`PreReviewGateOutcome::Blocked`] or
/// [`CoverageVerifyOutcome::Blocked`].
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

    /// I/O or decode error reading `impl-plan.json` (D7).
    #[error("failed to read impl-plan.json: {message}")]
    ImplPlanReadFailed {
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

    /// Read the per-layer signal document when absence is expected state.
    /// `Ok(None)` only on positively-classified absent docs; default fail-closed.
    fn read_optional_signals(
        &self,
        track_id: &domain::TrackId,
        layer: &domain::tddd::LayerId,
    ) -> Result<Option<TypeSignalsDocument>, PreReviewGateError> {
        self.read_signals(track_id, layer).map(Some)
    }
}

/// Secondary port for reading task statuses from `impl-plan.json` (D7).
///
/// Implemented by `infrastructure::impl_plan_reader::FsImplPlanReader`.
/// Injected into [`PreReviewGateInteractor`] to supply the task status filter
/// for the liveness gate: `in_progress` and `done` entries require Blue signal;
/// entries with no `in_progress` or `done` owner are skipped unless Red.
pub trait ImplPlanReaderPort: Send + Sync {
    /// Read `impl-plan.json` and return `TaskId → TaskStatusKind`.
    fn read_task_statuses(
        &self,
        track_id: &domain::TrackId,
    ) -> Result<HashMap<domain::TaskId, domain::TaskStatusKind>, PreReviewGateError>;
}

// ---------------------------------------------------------------------------
// CoverageVerifyCommand
// ---------------------------------------------------------------------------

/// CQRS command for the attribution-completeness coverage check use case
/// (`bin/sotp task-contract coverage`).
///
/// `track_id` identifies the active track whose `task-contract.json` is
/// evaluated for attribution completeness across all 6 canonical TDDD layers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverageVerifyCommand {
    /// The active track whose task-contract.json is evaluated.
    pub track_id: domain::TrackId,
}

// ---------------------------------------------------------------------------
// CoverageVerifyService (primary application service port)
// ---------------------------------------------------------------------------

/// Primary application service port for the attribution-completeness coverage
/// check use case (`bin/sotp task-contract coverage`).
///
/// Called by `cli_driver::task_contract::TaskContractDriver` when handling
/// `TaskContractInput::Coverage`. Shares [`PreReviewGateError`] as the I/O
/// error type with [`PreReviewGateService`] because both services use the same
/// secondary ports.
pub trait CoverageVerifyService: Send + Sync {
    /// Run the attribution-completeness coverage check for the active track.
    ///
    /// Returns [`CoverageVerifyOutcome::Passed`] when all catalogue entries are
    /// attributed to at least one task and all attributed entries exist in the
    /// catalogue. Returns [`CoverageVerifyOutcome::Blocked`] with a list of
    /// attribution violations on failure.
    ///
    /// # Errors
    ///
    /// Returns [`PreReviewGateError`] on infrastructure read failures.
    fn verify_coverage(
        &self,
        cmd: CoverageVerifyCommand,
    ) -> Result<CoverageVerifyOutcome, PreReviewGateError>;
}

// ---------------------------------------------------------------------------
// CoverageVerifyInteractor
// ---------------------------------------------------------------------------

/// Interactor implementing [`CoverageVerifyService`] (attribution-completeness check).
///
/// Holds two injected secondary ports: `task_contract_reader` reads
/// `task-contract.json`; `signal_reader` reads per-layer type-signals documents
/// to enumerate all catalogue entry keys.  Checks that every signaled entry is
/// attributed to at least one task (orphan detection) and every attributed
/// entry has a signal (referential integrity). Shares `TaskContractReaderPort`
/// and `ImplCatalogSignalReaderPort` with `PreReviewGateInteractor` (reuse per
/// spec `IN-07`).
pub struct CoverageVerifyInteractor {
    task_contract_reader: Arc<dyn TaskContractReaderPort>,
    signal_reader: Arc<dyn ImplCatalogSignalReaderPort>,
    impl_plan_reader: Arc<dyn ImplPlanReaderPort>,
}

impl CoverageVerifyInteractor {
    /// Construct by injecting three secondary ports; `impl_plan_reader` enables
    /// the D9 task-key referential integrity check.
    #[must_use]
    pub fn new(
        task_contract_reader: Arc<dyn TaskContractReaderPort>,
        signal_reader: Arc<dyn ImplCatalogSignalReaderPort>,
        impl_plan_reader: Arc<dyn ImplPlanReaderPort>,
    ) -> Self {
        Self { task_contract_reader, signal_reader, impl_plan_reader }
    }
}

impl CoverageVerifyService for CoverageVerifyInteractor {
    fn verify_coverage(
        &self,
        cmd: CoverageVerifyCommand,
    ) -> Result<CoverageVerifyOutcome, PreReviewGateError> {
        // D9 (knowledge/adr/2026-06-26-0503-adr2pr-back-and-forth-skill-definition.md):
        // when `task-contract.json` is absent, return Passed — no contract to
        // verify, gate has nothing to evaluate. Same precedent pattern as
        // 2026-06-03-1241-spec-states-gate-tolerate-missing-spec-artifact and
        // 2026-06-01-0406-review-gate-tolerate-missing-catalogue. When the file
        // exists, every coverage check (orphan / referential integrity / task-ref
        // RI) runs as before; no fail-open is introduced for malformed contracts.
        let contract_doc = match self.task_contract_reader.read(&cmd.track_id) {
            Ok(doc) => doc,
            Err(PreReviewGateError::TaskContractNotFound) => {
                return Ok(CoverageVerifyOutcome::Passed);
            }
            Err(e) => return Err(e),
        };

        let mut all_violations: Vec<domain::task_contract::CoverageViolation> = Vec::new();
        for &layer_str in CANONICAL_LAYERS {
            let Ok(layer) = domain::tddd::LayerId::try_new(layer_str.to_owned()) else { continue };
            let Some(signal_doc) =
                self.signal_reader.read_optional_signals(&cmd.track_id, &layer)?
            else {
                all_violations.push(
                    domain::task_contract::CoverageViolation::MissingSignalDocument {
                        layer: layer.clone(),
                    },
                );
                continue;
            };
            let scope_entries = build_scope_entries(&signal_doc, &layer)?;
            all_violations.extend(collect_per_layer_violations(
                &contract_doc,
                &layer,
                &scope_entries,
            ));
        }
        all_violations.extend(collect_non_canonical_layer_violations(&contract_doc));
        let plan_task_ids = self.impl_plan_reader.read_task_statuses(&cmd.track_id)?;
        all_violations.extend(collect_task_key_ri_violations(&contract_doc, &plan_task_ids));

        if all_violations.is_empty() {
            Ok(CoverageVerifyOutcome::Passed)
        } else {
            blocked_coverage_outcome(all_violations)
        }
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
    /// Returns [`PreReviewGateOutcome::Passed`] (binary OK signal) or
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

/// Interactor implementing [`PreReviewGateService`] (liveness check).
///
/// Holds three injected secondary ports:
/// - `task_contract_reader` reads `task-contract.json` for the active track.
/// - `signal_reader` reads per-layer `impl_catalog` type-signal documents.
/// - `impl_plan_reader` reads `impl-plan.json` for task-status filtering (D7).
///
/// The interactor checks that all attributed entries for current/done tasks
/// have Blue `impl_catalog` signals.
pub struct PreReviewGateInteractor {
    task_contract_reader: Arc<dyn TaskContractReaderPort>,
    signal_reader: Arc<dyn ImplCatalogSignalReaderPort>,
    impl_plan_reader: Arc<dyn ImplPlanReaderPort>,
}

impl PreReviewGateInteractor {
    /// Construct a `PreReviewGateInteractor` by injecting three secondary ports:
    /// task-contract reader, signal reader, and impl-plan reader (D7 addition
    /// for task status filtering).
    #[must_use]
    pub fn new(
        task_contract_reader: Arc<dyn TaskContractReaderPort>,
        signal_reader: Arc<dyn ImplCatalogSignalReaderPort>,
        impl_plan_reader: Arc<dyn ImplPlanReaderPort>,
    ) -> Self {
        Self { task_contract_reader, signal_reader, impl_plan_reader }
    }
}

/// Canonical TDDD layer identifiers iterated in all-layers mode.
const CANONICAL_LAYERS: &[&str] =
    &["domain", "usecase", "infrastructure", "cli_driver", "cli", "cli_composition"];

impl PreReviewGateInteractor {
    /// Evaluate one already-loaded signal document against the task contract.
    ///
    /// Performs liveness check only (Phase 3, D7 variant): verifies that every
    /// contracted entry that exists in the signal document has a Blue
    /// `impl_catalog` signal, with the following task-status filtering rules
    /// (D7):
    ///
    /// - Entries whose owning tasks include no `done` or `in_progress` task in
    ///   `impl-plan.json` are **skipped** from the Blue requirement (Yellow
    ///   tolerated).
    /// - If ANY owning task is `done` or `in_progress`, the entry **requires**
    ///   Blue.
    /// - Red signal is a blocker **regardless** of task status.
    /// - Entries absent from `impl-plan.json` are treated conservatively
    ///   (required to be Blue).
    ///
    /// Attribution checks (orphan detection, referential integrity) are
    /// handled by [`CoverageVerifyInteractor`].
    ///
    /// Returns the list of violations found for this layer (empty = passed).
    /// The caller is responsible for combining per-layer results into a final
    /// [`PreReviewGateOutcome`].
    fn check_signal_document(
        &self,
        layer: &domain::tddd::LayerId,
        contract_doc: &domain::task_contract::TaskContractDocument,
        signal_doc: &TypeSignalsDocument,
        task_statuses: &HashMap<domain::TaskId, domain::TaskStatusKind>,
    ) -> Result<Vec<PreReviewGateViolation>, PreReviewGateError> {
        // ── Build signal lookup ───────────────────────────────────────────────
        //
        // scope_signals: type_name -> ConfidenceSignal for entries in the signal doc.
        // Validate entry-key shape up front so malformed signal documents fail closed.
        let mut scope_signals: HashMap<String, ConfidenceSignal> = HashMap::new();
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
            scope_signals.insert(entry_key.as_str().to_owned(), signal.signal());
        }

        // Build a map: entry_key → set of task statuses that attribute it for this layer.
        // We need this to determine if any owning task requires Blue.
        let entries = contract_doc.entries();
        let mut entry_task_statuses: HashMap<&str, Vec<domain::TaskStatusKind>> = HashMap::new();
        for (task_id, refs) in entries {
            let status =
                task_statuses.get(task_id).copied().unwrap_or(domain::TaskStatusKind::Done); // conservative default
            for entry_ref in refs {
                if entry_ref.layer() == layer {
                    entry_task_statuses
                        .entry(entry_ref.entry_key().as_str())
                        .or_default()
                        .push(status);
                }
            }
        }

        // ── Phase 3 (D7): Signal check with task-status filtering ─────────────
        //
        // For each attributed entry present in the signal document:
        // - If the signal is Red → always block (status-independent).
        // - If any owning task is done or in_progress → require Blue.
        // - Otherwise, skip Blue check (Yellow tolerated).
        // Entries not present in the signal document are skipped (coverage concern).
        let mut violations: Vec<PreReviewGateViolation> = Vec::new();
        for (key, statuses) in &entry_task_statuses {
            let Some(&signal) = scope_signals.get(*key) else {
                continue; // Not in signal doc → coverage concern, skip here.
            };

            let is_red = signal == ConfidenceSignal::Red;
            let requires_blue = statuses.iter().any(|&status| {
                matches!(status, domain::TaskStatusKind::InProgress | domain::TaskStatusKind::Done)
            });

            if is_red {
                // Red is always a blocker regardless of task status.
                let entry_ref = entry_key_to_contracted_ref(contract_doc, layer, key)?;
                violations.push(PreReviewGateViolation::NonBlueSignal { entry: entry_ref, signal });
            } else if requires_blue && signal != ConfidenceSignal::Blue {
                // Done/in_progress task with non-blue, non-red signal → block.
                let entry_ref = entry_key_to_contracted_ref(contract_doc, layer, key)?;
                violations.push(PreReviewGateViolation::NonBlueSignal { entry: entry_ref, signal });
            }
            // No done/in_progress owner + Yellow → skip (no violation)
        }

        Ok(violations)
    }

    /// Run the liveness gate for a single TDDD layer.
    ///
    /// Returns the list of violations found for this layer (empty = passed).
    /// The caller is responsible for combining per-layer results into a final
    /// [`PreReviewGateOutcome`].
    fn check_layer(
        &self,
        track_id: &domain::TrackId,
        layer: &domain::tddd::LayerId,
        contract_doc: &domain::task_contract::TaskContractDocument,
        task_statuses: &HashMap<domain::TaskId, domain::TaskStatusKind>,
    ) -> Result<Vec<PreReviewGateViolation>, PreReviewGateError> {
        // ── Step 1: read type-signals for layer ───────────────────────────────
        let signal_doc = self.signal_reader.read_signals(track_id, layer)?;
        self.check_signal_document(layer, contract_doc, &signal_doc, task_statuses)
    }
}

impl PreReviewGateService for PreReviewGateInteractor {
    fn check(&self, cmd: PreReviewGateCommand) -> Result<PreReviewGateOutcome, PreReviewGateError> {
        // ── Step 0: read task-contract.json ──────────────────────────────────
        //
        // D9 (knowledge/adr/2026-06-26-0503-adr2pr-back-and-forth-skill-definition.md):
        // TaskContractNotFound returns Passed (no contract → no entries to
        // verify). Same precedent pattern as the sibling tolerance ADRs
        // (2026-06-03-1241-spec-states / 2026-06-01-0406-review-gate). When
        // `task-contract.json` exists, the liveness check still runs in full.
        let contract_doc = match self.task_contract_reader.read(&cmd.track_id) {
            Ok(doc) => doc,
            Err(PreReviewGateError::TaskContractNotFound) => {
                return Ok(PreReviewGateOutcome::Passed);
            }
            Err(e) => return Err(e),
        };

        // ── Step 0b: load impl-plan.json task statuses (D7) ──────────────────
        //
        // Used to filter attributions by task status: done/in_progress entries
        // require Blue; entries without a done/in_progress owner tolerate Yellow;
        // Red always blocks.
        let task_statuses = self.impl_plan_reader.read_task_statuses(&cmd.track_id)?;

        match cmd.layer {
            Some(layer) => {
                // ── Per-layer mode ────────────────────────────────────────────
                let violations =
                    self.check_layer(&cmd.track_id, &layer, &contract_doc, &task_statuses)?;
                if violations.is_empty() {
                    Ok(PreReviewGateOutcome::Passed)
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
                            let violations = self.check_signal_document(
                                &layer,
                                &contract_doc,
                                &signal_doc,
                                &task_statuses,
                            )?;
                            all_violations.extend(violations);
                        }
                        None => {
                            // No signal document for this layer — skip silently.
                            //
                            // Attribution completeness (orphan detection,
                            // referential integrity) is handled by
                            // CoverageVerifyInteractor. The liveness check only
                            // verifies Blue signals for entries that are present in
                            // the signal document; absent layers have nothing to
                            // verify here.
                        }
                    }
                }
                if all_violations.is_empty() {
                    Ok(PreReviewGateOutcome::Passed)
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

    use domain::TaskStatusKind;
    use domain::task_contract::{
        ContractedEntryRef, CoverageVerifyOutcome, CoverageViolation, PreReviewGateOutcome,
        PreReviewGateViolation, TaskContractDocument,
    };
    use domain::tddd::semantic_verify::CatalogueEntryKey;
    use domain::tddd::{LayerId, type_signals_doc::TypeSignalsDocument};
    use domain::{ConfidenceSignal, TaskId, Timestamp, TrackId, TypeSignal};

    use super::{
        CoverageVerifyCommand, CoverageVerifyInteractor, CoverageVerifyService,
        ImplCatalogSignalReaderPort, ImplPlanReaderPort, PreReviewGateCommand, PreReviewGateError,
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

    fn unknown_signal(name: &str) -> TypeSignal {
        TypeSignal::new(name, "unknown", ConfidenceSignal::Yellow, true, vec![], vec![], vec![])
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
                Err(PreReviewGateError::ImplPlanReadFailed { message }) => {
                    Err(PreReviewGateError::ImplPlanReadFailed { message: message.clone() })
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

    /// Const impl-plan reader that always returns an empty task-status map.
    struct EmptyImplPlanReader;

    impl ImplPlanReaderPort for EmptyImplPlanReader {
        fn read_task_statuses(
            &self,
            _track_id: &TrackId,
        ) -> Result<std::collections::HashMap<TaskId, TaskStatusKind>, PreReviewGateError> {
            Ok(std::collections::HashMap::new())
        }
    }

    fn interactor(
        contract: Result<TaskContractDocument, PreReviewGateError>,
        signals: Result<TypeSignalsDocument, PreReviewGateError>,
    ) -> PreReviewGateInteractor {
        PreReviewGateInteractor::new(
            Arc::new(ConstContractReader(contract)),
            Arc::new(ConstSignalReader(signals)),
            Arc::new(EmptyImplPlanReader),
        )
    }

    fn cmd(track: &str, group: &str) -> PreReviewGateCommand {
        PreReviewGateCommand { track_id: track_id(track), layer: Some(layer(group)) }
    }

    // ── D9 tolerance (knowledge/adr/2026-06-26-0503-...): TaskContractNotFound → Passed ──
    //
    // When `task-contract.json` is absent, the liveness check returns Passed —
    // no contract means no entries to verify. Same precedent pattern as the
    // sibling tolerance ADRs (2026-06-03-spec-states / 2026-06-01-review-gate).

    #[test]
    fn missing_task_contract_yields_passed_via_d9_tolerance() {
        let svc = interactor(
            Err(PreReviewGateError::TaskContractNotFound),
            Ok(make_signals(vec![blue_signal("Foo")])),
        );
        let outcome = svc.check(cmd("my-track", "domain")).unwrap();
        assert!(
            matches!(outcome, PreReviewGateOutcome::Passed),
            "expected Passed (D9 tolerance), got {outcome:?}"
        );
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

    // ── AC-07 (e): all blue + complete attribution → Passed (binary) ──────────

    #[test]
    fn all_blue_and_complete_attribution_yields_passed() {
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
        assert!(
            matches!(outcome, PreReviewGateOutcome::Passed),
            "expected Passed, got {outcome:?}"
        );
    }

    // ── Narrowed check: contracted key absent from signal doc is skipped ───────
    //
    // After D5 split, the check interactor no longer emits InvalidEntryRef for
    // entries absent from the signal doc. That is now a coverage concern.
    // Attributed entries without a signal document entry are simply skipped.

    #[test]
    fn contracted_key_absent_from_signal_doc_is_skipped_by_check() {
        // task-contract.json attributes "Missing" in domain, but signal doc only has "Foo".
        // After D5 narrowing: check must pass for "Foo" (blue) and skip "Missing".
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
        // Check no longer emits InvalidEntryRef; "Missing" is skipped.
        assert!(
            matches!(outcome, PreReviewGateOutcome::Passed),
            "expected Passed, got {outcome:?}"
        );
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
            Arc::new(EmptyImplPlanReader),
        );

        let outcome = svc
            .check(PreReviewGateCommand { track_id: track_id("my-track"), layer: None })
            .unwrap();

        assert!(
            matches!(outcome, PreReviewGateOutcome::Passed),
            "expected Passed, got {outcome:?}"
        );
    }

    // ── All-layers mode: missing signal doc for contracted layer is now skipped ─
    //
    // After D5 narrowing: the check interactor silently skips layers with absent
    // signal documents. Attribution concerns (InvalidEntryRef for missing layers)
    // are now handled by CoverageVerifyInteractor.

    #[test]
    fn all_layer_iterate_missing_signal_doc_for_contracted_layer_passes_check() {
        // task-contract attributes 2 entries to "domain" and 1 to "usecase".
        // signal_docs only registers "usecase" → "domain" returns Ok(None).
        // After D5: check passes because "domain" is silently skipped (no check violation).
        let mut signal_docs = std::collections::HashMap::new();
        signal_docs.insert("usecase".to_owned(), make_signals(vec![blue_signal("UseFoo")]));
        let svc = PreReviewGateInteractor::new(
            Arc::new(ConstContractReader(Ok(make_contract(
                "my-track",
                vec![
                    (
                        task_id("T001"),
                        vec![
                            ContractedEntryRef::new(layer("domain"), entry_key("DomFoo")),
                            ContractedEntryRef::new(layer("domain"), entry_key("DomBar")),
                        ],
                    ),
                    (
                        task_id("T002"),
                        vec![ContractedEntryRef::new(layer("usecase"), entry_key("UseFoo"))],
                    ),
                ],
            )))),
            Arc::new(LayerAwareSignalReader(signal_docs)),
            Arc::new(EmptyImplPlanReader),
        );

        let outcome = svc
            .check(PreReviewGateCommand { track_id: track_id("my-track"), layer: None })
            .unwrap();

        assert!(
            matches!(outcome, PreReviewGateOutcome::Passed),
            "expected Passed after D5 narrowing (missing domain layer skipped), got {outcome:?}"
        );
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
            Arc::new(EmptyImplPlanReader),
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
            Arc::new(EmptyImplPlanReader),
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
            Arc::new(EmptyImplPlanReader),
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

    // ── CoverageVerifyInteractor tests (AC-07 cases b, c, e-coverage) ─────────

    /// Build a plan reader whose task-id set matches the contract's task keys,
    /// so existing coverage tests that don't care about D9 RI keep passing.
    fn plan_reader_matching_contract(
        contract: &Result<TaskContractDocument, PreReviewGateError>,
    ) -> Arc<dyn ImplPlanReaderPort> {
        match contract {
            Ok(doc) => {
                let map: std::collections::HashMap<TaskId, TaskStatusKind> =
                    doc.entries().keys().map(|id| (id.clone(), TaskStatusKind::Todo)).collect();
                Arc::new(FixedImplPlanReader(map))
            }
            Err(_) => Arc::new(EmptyImplPlanReader),
        }
    }

    fn coverage_interactor(
        contract: Result<TaskContractDocument, PreReviewGateError>,
        signal_docs: std::collections::HashMap<String, TypeSignalsDocument>,
    ) -> CoverageVerifyInteractor {
        let plan_reader = plan_reader_matching_contract(&contract);
        CoverageVerifyInteractor::new(
            Arc::new(ConstContractReader(contract)),
            Arc::new(LayerAwareSignalReader(signal_docs)),
            plan_reader,
        )
    }

    fn coverage_interactor_with_plan(
        contract: Result<TaskContractDocument, PreReviewGateError>,
        signal_docs: std::collections::HashMap<String, TypeSignalsDocument>,
        plan_reader: Arc<dyn ImplPlanReaderPort>,
    ) -> CoverageVerifyInteractor {
        CoverageVerifyInteractor::new(
            Arc::new(ConstContractReader(contract)),
            Arc::new(LayerAwareSignalReader(signal_docs)),
            plan_reader,
        )
    }

    fn coverage_cmd(track: &str) -> CoverageVerifyCommand {
        CoverageVerifyCommand { track_id: track_id(track) }
    }

    // ── Coverage (D9 tolerance): TaskContractNotFound → Passed ────────────────
    //
    // Same precondition as the liveness check (D9, ADR
    // knowledge/adr/2026-06-26-0503-...). Empty contract → nothing to verify.

    #[test]
    fn coverage_missing_task_contract_yields_passed_via_d9_tolerance() {
        let svc = CoverageVerifyInteractor::new(
            Arc::new(ConstContractReader(Err(PreReviewGateError::TaskContractNotFound))),
            Arc::new(LayerAwareSignalReader(std::collections::HashMap::new())),
            Arc::new(EmptyImplPlanReader),
        );
        let outcome = svc.verify_coverage(coverage_cmd("my-track")).unwrap();
        assert!(
            matches!(outcome, CoverageVerifyOutcome::Passed),
            "expected Passed (D9 tolerance), got {outcome:?}"
        );
    }

    // ── Coverage (b): signal entry absent from task attribution → OrphanEntry ──

    #[test]
    fn coverage_signal_entry_not_attributed_yields_orphan_entry_violation() {
        // Signal doc has "Foo" in domain layer.
        // task-contract.json only attributes "Bar" to domain (not "Foo").
        let mut signal_docs = std::collections::HashMap::new();
        signal_docs.insert(
            "domain".to_owned(),
            make_signals(vec![blue_signal("Foo"), blue_signal("Bar")]),
        );
        let svc = coverage_interactor(
            Ok(make_contract(
                "my-track",
                vec![(
                    task_id("T001"),
                    vec![ContractedEntryRef::new(layer("domain"), entry_key("Bar"))],
                )],
            )),
            signal_docs,
        );
        let outcome = svc.verify_coverage(coverage_cmd("my-track")).unwrap();
        match outcome {
            CoverageVerifyOutcome::Blocked { violations, .. } => {
                let orphan_keys: Vec<&str> = violations
                    .iter()
                    .filter_map(|v| {
                        if let CoverageViolation::OrphanEntry { entry } = v {
                            Some(entry.entry_key().as_str())
                        } else {
                            None
                        }
                    })
                    .collect();
                assert!(
                    orphan_keys.contains(&"Foo"),
                    "expected OrphanEntry for Foo, got: {orphan_keys:?}"
                );
            }
            other => panic!("expected Blocked with OrphanEntry, got {other:?}"),
        }
    }

    // ── Coverage (c): attributed key absent from signal doc → InvalidEntryRef ──

    #[test]
    fn coverage_contracted_key_absent_from_signal_doc_yields_invalid_entry_ref() {
        // task-contract attributes "Missing" to domain, but signal doc only has "Foo".
        let mut signal_docs = std::collections::HashMap::new();
        signal_docs.insert("domain".to_owned(), make_signals(vec![blue_signal("Foo")]));
        let svc = coverage_interactor(
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
            signal_docs,
        );
        let outcome = svc.verify_coverage(coverage_cmd("my-track")).unwrap();
        match outcome {
            CoverageVerifyOutcome::Blocked { violations, .. } => {
                let invalid_keys: Vec<&str> = violations
                    .iter()
                    .filter_map(|v| {
                        if let CoverageViolation::InvalidEntryRef { entry, .. } = v {
                            Some(entry.entry_key().as_str())
                        } else {
                            None
                        }
                    })
                    .collect();
                assert!(
                    invalid_keys.contains(&"Missing"),
                    "expected InvalidEntryRef for Missing, got: {invalid_keys:?}"
                );
            }
            other => panic!("expected Blocked with InvalidEntryRef, got {other:?}"),
        }
    }

    #[test]
    fn coverage_absent_signal_doc_for_contracted_layer_yields_missing_signal_document() {
        // Attributed entries exist for "domain", but the signal document is absent.
        // Under the new fail-closed rule, MissingSignalDocument is emitted for the
        // absent layer regardless of attribution — one violation per absent layer.
        let svc = coverage_interactor(
            Ok(make_contract(
                "my-track",
                vec![(
                    task_id("T001"),
                    vec![ContractedEntryRef::new(layer("domain"), entry_key("Missing"))],
                )],
            )),
            std::collections::HashMap::new(), // no signal docs for any layer
        );

        let outcome = svc.verify_coverage(coverage_cmd("my-track")).unwrap();
        match outcome {
            CoverageVerifyOutcome::Blocked { violations, .. } => {
                // All 6 canonical TDDD layers are absent → 6 MissingSignalDocument violations.
                let missing_layers: Vec<&str> = violations
                    .iter()
                    .filter_map(|v| {
                        if let CoverageViolation::MissingSignalDocument { layer } = v {
                            Some(layer.as_ref())
                        } else {
                            None
                        }
                    })
                    .collect();
                assert!(
                    missing_layers.contains(&"domain"),
                    "expected MissingSignalDocument for domain, got violations: {violations:?}"
                );
                // Ensure no InvalidEntryRef is produced when the signal doc is absent.
                let has_invalid_ref = violations
                    .iter()
                    .any(|v| matches!(v, CoverageViolation::InvalidEntryRef { .. }));
                assert!(
                    !has_invalid_ref,
                    "InvalidEntryRef should not be emitted when signal document is absent"
                );
            }
            other => panic!("expected Blocked with MissingSignalDocument, got {other:?}"),
        }
    }

    // ── Coverage: no attribution AND no signal doc → MissingSignalDocument ────────
    //
    // F1 fix: MissingSignalDocument must be emitted regardless of whether entries are
    // attributed to the absent layer. The previous behavior was to silently `continue`
    // when no entries were attributed to the absent layer, leaving the gap invisible.

    #[test]
    fn coverage_absent_signal_doc_with_no_attribution_yields_missing_signal_document() {
        // Contract only attributes to "usecase"; "domain" has neither a signal doc
        // nor any attribution. Under the new rule, "domain" must still emit
        // MissingSignalDocument so the absent signal document is surfaced.
        let mut signal_docs = std::collections::HashMap::new();
        signal_docs.insert("usecase".to_owned(), make_signals(vec![blue_signal("UseBar")]));
        // "infrastructure", "cli_driver", "cli", "cli_composition" absent too but
        // we focus on "domain" which has zero attribution as the key test case.

        let svc = coverage_interactor(
            Ok(make_contract(
                "my-track",
                vec![(
                    task_id("T001"),
                    vec![ContractedEntryRef::new(layer("usecase"), entry_key("UseBar"))],
                )],
            )),
            signal_docs,
        );

        let outcome = svc.verify_coverage(coverage_cmd("my-track")).unwrap();
        match outcome {
            CoverageVerifyOutcome::Blocked { violations, .. } => {
                let missing_layers: Vec<&str> = violations
                    .iter()
                    .filter_map(|v| {
                        if let CoverageViolation::MissingSignalDocument { layer } = v {
                            Some(layer.as_ref())
                        } else {
                            None
                        }
                    })
                    .collect();
                assert!(
                    missing_layers.contains(&"domain"),
                    "expected MissingSignalDocument for domain (no attribution, no signal doc), got: {missing_layers:?}"
                );
            }
            other => panic!("expected Blocked with MissingSignalDocument, got {other:?}"),
        }
    }

    // ── Coverage (f): non-canonical layer attribution → InvalidEntryRef ──────────
    //
    // When task-contract.json attributes an entry to a layer that is not one of
    // the 6 canonical TDDD layers (e.g. "doman" as a typo for "domain"), the
    // per-layer CANONICAL_LAYERS iteration never visits it. Without Phase 3, the
    // entry would silently bypass both orphan detection and referential integrity
    // checks, producing a false-pass result.
    // Phase 3 detects these entries and emits `InvalidEntryRef` for each one.

    #[test]
    fn coverage_non_canonical_layer_attribution_yields_invalid_entry_ref() {
        // task-contract attributes "Foo" to "doman" (typo for "domain").
        // All 6 canonical layers have present-but-empty signal docs so that
        // MissingSignalDocument violations do not obscure the assertion.
        let mut signal_docs = std::collections::HashMap::new();
        for layer_name in
            &["domain", "usecase", "infrastructure", "cli_driver", "cli", "cli_composition"]
        {
            signal_docs.insert((*layer_name).to_owned(), make_signals(vec![]));
        }
        let svc = coverage_interactor(
            Ok(make_contract(
                "my-track",
                vec![(
                    task_id("T001"),
                    // "doman" is a non-canonical layer (typo for "domain").
                    vec![ContractedEntryRef::new(layer("doman"), entry_key("Foo"))],
                )],
            )),
            signal_docs,
        );
        let outcome = svc.verify_coverage(coverage_cmd("my-track")).unwrap();
        match outcome {
            CoverageVerifyOutcome::Blocked { violations, .. } => {
                let invalid_refs: Vec<_> = violations
                    .iter()
                    .filter_map(|v| {
                        if let CoverageViolation::InvalidEntryRef { entry, reason } = v {
                            Some((entry, reason))
                        } else {
                            None
                        }
                    })
                    .collect();
                assert!(
                    !invalid_refs.is_empty(),
                    "expected at least one InvalidEntryRef for non-canonical layer 'doman', got: {violations:?}"
                );
                let (entry, reason) = invalid_refs[0];
                assert_eq!(entry.layer().as_ref(), "doman", "expected layer 'doman' in violation");
                assert_eq!(entry.entry_key().as_str(), "Foo");
                assert!(
                    reason.contains("not a canonical TDDD layer"),
                    "expected reason to mention canonical TDDD layer, got: {reason}"
                );
            }
            other => panic!("expected Blocked with InvalidEntryRef, got {other:?}"),
        }
    }

    // ── Coverage (e): all entries attributed and consistent → Passed ───────────
    //
    // All 6 canonical TDDD layers must have signal documents present to avoid
    // MissingSignalDocument violations. Layers with no entries in the signal doc
    // and no attribution in task-contract.json produce zero violations (no orphans,
    // no invalid refs) when their signal documents are present (even if empty).

    #[test]
    fn coverage_all_entries_attributed_and_consistent_yields_passed() {
        // domain has Foo and Bar (both attributed T001) — all consistent.
        // Remaining 5 canonical layers have empty signal docs (no entries) to
        // satisfy the MissingSignalDocument gate without adding noise.
        let mut signal_docs = std::collections::HashMap::new();
        signal_docs.insert(
            "domain".to_owned(),
            make_signals(vec![blue_signal("Foo"), blue_signal("Bar")]),
        );
        // Provide present (empty) signal docs for the other 5 canonical layers
        // so that CoverageVerifyInteractor finds a document for each and does
        // not emit MissingSignalDocument violations.
        for layer_name in &["usecase", "infrastructure", "cli_driver", "cli", "cli_composition"] {
            signal_docs.insert((*layer_name).to_owned(), make_signals(vec![]));
        }
        let svc = coverage_interactor(
            Ok(make_contract(
                "my-track",
                vec![(
                    task_id("T001"),
                    vec![
                        ContractedEntryRef::new(layer("domain"), entry_key("Foo")),
                        ContractedEntryRef::new(layer("domain"), entry_key("Bar")),
                    ],
                )],
            )),
            signal_docs,
        );
        let outcome = svc.verify_coverage(coverage_cmd("my-track")).unwrap();
        assert!(
            matches!(outcome, CoverageVerifyOutcome::Passed),
            "expected Passed, got {outcome:?}"
        );
    }

    // ── Coverage: unknown-kind rows must trigger OrphanEntry (ADR fail-closed) ─
    //
    // ADR `2026-06-27-0852-pre-review-task-contract-conformance-gate.md` D1/D3/D4/D9
    // require attribution completeness across **every** catalogue entry, and
    // Rejected Alternative AB explicitly forbids silently ignoring rows. A
    // `kind: "unknown"` signal typically means a newly-added type that is not yet
    // registered in the catalogue — precisely the case that must fail-closed at
    // pre-review time so `/track:diagnose` can route to `type-design`.
    //
    // The prior behavior (round 15 P1) silently excluded unknown-kind rows from
    // orphan detection, which allowed such types to slip past the pre-review gate
    // and only surface at commit time. That was a bug — the row now correctly
    // triggers `OrphanEntry`.

    #[test]
    fn coverage_unknown_kind_signal_row_yields_orphan_entry() {
        // Signal doc has Foo (blue, attributed) AND ImplOnlyType (kind: unknown,
        // NOT attributed). Under fail-closed semantics, ImplOnlyType must trigger
        // OrphanEntry so the planner is forced to either add it to the catalogue
        // (registering a proper kind) or remove the impl.
        let mut signal_docs = std::collections::HashMap::new();
        signal_docs.insert(
            "domain".to_owned(),
            make_signals(vec![blue_signal("Foo"), unknown_signal("ImplOnlyType")]),
        );
        for layer_name in &["usecase", "infrastructure", "cli_driver", "cli", "cli_composition"] {
            signal_docs.insert((*layer_name).to_owned(), make_signals(vec![]));
        }
        let svc = coverage_interactor(
            Ok(make_contract(
                "my-track",
                vec![(
                    task_id("T001"),
                    vec![ContractedEntryRef::new(layer("domain"), entry_key("Foo"))],
                )],
            )),
            signal_docs,
        );
        let outcome = svc.verify_coverage(coverage_cmd("my-track")).unwrap();
        match outcome {
            CoverageVerifyOutcome::Blocked { violations, .. } => {
                let orphan_keys: Vec<&str> = violations
                    .iter()
                    .filter_map(|v| {
                        if let CoverageViolation::OrphanEntry { entry } = v {
                            Some(entry.entry_key().as_str())
                        } else {
                            None
                        }
                    })
                    .collect();
                assert!(
                    orphan_keys.contains(&"ImplOnlyType"),
                    "expected OrphanEntry for ImplOnlyType (kind: unknown), got: {orphan_keys:?}"
                );
            }
            other => {
                panic!("expected Blocked with OrphanEntry for unknown-kind row, got {other:?}")
            }
        }
    }

    // ── D9 task-key referential integrity tests ───────────────────────────────

    /// Build a signal-docs map populated with empty docs for every canonical
    /// TDDD layer plus the given `domain_entries`, so D9 tests can focus on the
    /// task-key RI check without `MissingSignalDocument` / `OrphanEntry` noise.
    fn d9_signal_docs(
        domain_entries: Vec<&'static str>,
    ) -> std::collections::HashMap<String, TypeSignalsDocument> {
        let mut signal_docs = std::collections::HashMap::new();
        signal_docs.insert(
            "domain".to_owned(),
            make_signals(domain_entries.into_iter().map(blue_signal).collect()),
        );
        for layer_name in &["usecase", "infrastructure", "cli_driver", "cli", "cli_composition"] {
            signal_docs.insert((*layer_name).to_owned(), make_signals(vec![]));
        }
        signal_docs
    }

    // ── Coverage (h): stale task key absent from impl-plan → InvalidTaskRef ───

    #[test]
    fn coverage_stale_task_key_absent_from_impl_plan_yields_invalid_task_ref() {
        let contract = Ok(make_contract(
            "my-track",
            vec![(
                task_id("T999"),
                vec![ContractedEntryRef::new(layer("domain"), entry_key("Foo"))],
            )],
        ));
        // impl-plan only knows about T001, so T999 in the contract is stale.
        let plan_reader = Arc::new(FixedImplPlanReader(std::collections::HashMap::from([(
            task_id("T001"),
            TaskStatusKind::Done,
        )])));
        let svc = coverage_interactor_with_plan(contract, d9_signal_docs(vec!["Foo"]), plan_reader);
        let outcome = svc.verify_coverage(coverage_cmd("my-track")).unwrap();
        match outcome {
            CoverageVerifyOutcome::Blocked { violations, .. } => {
                let invalid_task_refs: Vec<_> = violations
                    .iter()
                    .filter_map(|v| match v {
                        CoverageViolation::InvalidTaskRef { task_id: tid, entry_keys } => {
                            Some((tid.clone(), entry_keys.clone()))
                        }
                        _ => None,
                    })
                    .collect();
                assert_eq!(invalid_task_refs.len(), 1, "expected exactly 1 InvalidTaskRef");
                assert_eq!(invalid_task_refs[0].0.as_ref(), "T999");
                assert_eq!(invalid_task_refs[0].1.len(), 1);
                assert_eq!(invalid_task_refs[0].1[0].entry_key().as_str(), "Foo");
            }
            other => panic!("expected Blocked with InvalidTaskRef, got {other:?}"),
        }
    }

    // ── Coverage (i): every task key present in impl-plan → no InvalidTaskRef ─

    #[test]
    fn coverage_all_task_keys_present_in_impl_plan_emits_no_invalid_task_ref() {
        let contract = Ok(make_contract(
            "my-track",
            vec![(
                task_id("T001"),
                vec![ContractedEntryRef::new(layer("domain"), entry_key("Foo"))],
            )],
        ));
        let plan_reader = Arc::new(FixedImplPlanReader(std::collections::HashMap::from([
            (task_id("T001"), TaskStatusKind::Done),
            (task_id("T002"), TaskStatusKind::Todo),
        ])));
        let svc = coverage_interactor_with_plan(contract, d9_signal_docs(vec!["Foo"]), plan_reader);
        let outcome = svc.verify_coverage(coverage_cmd("my-track")).unwrap();
        assert!(
            matches!(outcome, CoverageVerifyOutcome::Passed),
            "expected Passed when every task key resolves in impl-plan, got {outcome:?}"
        );
    }

    // ── Coverage (j): multiple stale task keys → one InvalidTaskRef each ──────

    #[test]
    fn coverage_multiple_stale_task_keys_yield_one_invalid_task_ref_each() {
        let contract = Ok(make_contract(
            "my-track",
            vec![
                (task_id("T100"), vec![ContractedEntryRef::new(layer("domain"), entry_key("Foo"))]),
                (task_id("T200"), vec![ContractedEntryRef::new(layer("domain"), entry_key("Bar"))]),
            ],
        ));
        // impl-plan has neither T100 nor T200, so both are stale.
        let plan_reader = Arc::new(FixedImplPlanReader(std::collections::HashMap::from([(
            task_id("T001"),
            TaskStatusKind::Done,
        )])));
        let svc = coverage_interactor_with_plan(
            contract,
            d9_signal_docs(vec!["Foo", "Bar"]),
            plan_reader,
        );
        let outcome = svc.verify_coverage(coverage_cmd("my-track")).unwrap();
        match outcome {
            CoverageVerifyOutcome::Blocked { violations, .. } => {
                let stale_ids: std::collections::BTreeSet<String> = violations
                    .iter()
                    .filter_map(|v| match v {
                        CoverageViolation::InvalidTaskRef { task_id: tid, .. } => {
                            Some(tid.as_ref().to_owned())
                        }
                        _ => None,
                    })
                    .collect();
                assert_eq!(stale_ids.len(), 2);
                assert!(stale_ids.contains("T100"));
                assert!(stale_ids.contains("T200"));
            }
            other => panic!("expected Blocked with InvalidTaskRefs, got {other:?}"),
        }
    }

    // ── D7 task-status filtering tests ───────────────────────────────────────────

    /// Impl-plan reader that returns a fixed, caller-supplied task status map.
    struct FixedImplPlanReader(std::collections::HashMap<TaskId, TaskStatusKind>);

    impl ImplPlanReaderPort for FixedImplPlanReader {
        fn read_task_statuses(
            &self,
            _track_id: &TrackId,
        ) -> Result<std::collections::HashMap<TaskId, TaskStatusKind>, PreReviewGateError> {
            Ok(self.0.clone())
        }
    }

    /// Impl-plan reader that always fails with `ImplPlanReadFailed`.
    struct FailingImplPlanReader;

    impl ImplPlanReaderPort for FailingImplPlanReader {
        fn read_task_statuses(
            &self,
            _track_id: &TrackId,
        ) -> Result<std::collections::HashMap<TaskId, TaskStatusKind>, PreReviewGateError> {
            Err(PreReviewGateError::ImplPlanReadFailed {
                message: "test: impl-plan.json read failed".to_owned(),
            })
        }
    }

    fn red_signal(name: &str) -> TypeSignal {
        TypeSignal::new(name, "struct", ConfidenceSignal::Red, false, vec![], vec![], vec![])
    }

    // ── D7 case (d): Red signal on todo task → always blocked ────────────────────

    #[test]
    fn d7_red_signal_on_todo_task_is_always_blocked() {
        // Even when the owning task is `todo`, Red must block.
        let mut statuses = std::collections::HashMap::new();
        statuses.insert(task_id("T001"), TaskStatusKind::Todo);

        let svc = PreReviewGateInteractor::new(
            Arc::new(ConstContractReader(Ok(make_contract(
                "my-track",
                vec![(
                    task_id("T001"),
                    vec![ContractedEntryRef::new(layer("domain"), entry_key("Foo"))],
                )],
            )))),
            Arc::new(ConstSignalReader(Ok(make_signals(vec![red_signal("Foo")])))),
            Arc::new(FixedImplPlanReader(statuses)),
        );

        let outcome = svc.check(cmd("my-track", "domain")).unwrap();
        match outcome {
            PreReviewGateOutcome::Blocked { violations, .. } => {
                assert_eq!(violations.len(), 1);
                match &violations[0] {
                    PreReviewGateViolation::NonBlueSignal { entry, signal } => {
                        assert_eq!(entry.entry_key().as_str(), "Foo");
                        assert_eq!(*signal, ConfidenceSignal::Red);
                    }
                    other => panic!("expected NonBlueSignal(Red), got {other:?}"),
                }
            }
            other => panic!("expected Blocked, got {other:?}"),
        }
    }

    // ── D7 case (e): in_progress + done tasks with Blue signals → Passed ─────────

    #[test]
    fn d7_in_progress_and_done_tasks_with_blue_signals_yields_passed() {
        let mut statuses = std::collections::HashMap::new();
        statuses.insert(task_id("T001"), TaskStatusKind::InProgress);
        statuses.insert(task_id("T002"), TaskStatusKind::Done);

        let svc = PreReviewGateInteractor::new(
            Arc::new(ConstContractReader(Ok(make_contract(
                "my-track",
                vec![
                    (
                        task_id("T001"),
                        vec![ContractedEntryRef::new(layer("domain"), entry_key("Foo"))],
                    ),
                    (
                        task_id("T002"),
                        vec![ContractedEntryRef::new(layer("domain"), entry_key("Bar"))],
                    ),
                ],
            )))),
            Arc::new(ConstSignalReader(Ok(make_signals(vec![
                blue_signal("Foo"),
                blue_signal("Bar"),
            ])))),
            Arc::new(FixedImplPlanReader(statuses)),
        );

        let outcome = svc.check(cmd("my-track", "domain")).unwrap();
        assert!(
            matches!(outcome, PreReviewGateOutcome::Passed),
            "expected Passed for in_progress/done tasks with Blue signals, got {outcome:?}"
        );
    }

    // ── D7 case (f): todo-only task with Yellow signal → Passed (skipped) ────────

    #[test]
    fn d7_todo_task_with_yellow_signal_is_tolerated() {
        // Yellow is tolerated when the owning task is still `todo`.
        let mut statuses = std::collections::HashMap::new();
        statuses.insert(task_id("T001"), TaskStatusKind::Todo);

        let svc = PreReviewGateInteractor::new(
            Arc::new(ConstContractReader(Ok(make_contract(
                "my-track",
                vec![(
                    task_id("T001"),
                    vec![ContractedEntryRef::new(layer("domain"), entry_key("Foo"))],
                )],
            )))),
            Arc::new(ConstSignalReader(Ok(make_signals(vec![yellow_signal("Foo")])))),
            Arc::new(FixedImplPlanReader(statuses)),
        );

        let outcome = svc.check(cmd("my-track", "domain")).unwrap();
        assert!(
            matches!(outcome, PreReviewGateOutcome::Passed),
            "expected Passed (todo + Yellow tolerated by D7), got {outcome:?}"
        );
    }

    #[test]
    fn d7_skipped_task_with_yellow_signal_is_tolerated() {
        let mut statuses = std::collections::HashMap::new();
        statuses.insert(task_id("T001"), TaskStatusKind::Skipped);

        let svc = PreReviewGateInteractor::new(
            Arc::new(ConstContractReader(Ok(make_contract(
                "my-track",
                vec![(
                    task_id("T001"),
                    vec![ContractedEntryRef::new(layer("domain"), entry_key("Foo"))],
                )],
            )))),
            Arc::new(ConstSignalReader(Ok(make_signals(vec![yellow_signal("Foo")])))),
            Arc::new(FixedImplPlanReader(statuses)),
        );

        let outcome = svc.check(cmd("my-track", "domain")).unwrap();
        assert!(
            matches!(outcome, PreReviewGateOutcome::Passed),
            "expected Passed (skipped + Yellow tolerated by D7), got {outcome:?}"
        );
    }

    // ── D7: ImplPlanReadFailed propagates as an error ────────────────────────────

    #[test]
    fn d7_impl_plan_read_failed_propagates_as_error() {
        let svc = PreReviewGateInteractor::new(
            Arc::new(ConstContractReader(Ok(make_contract(
                "my-track",
                vec![(
                    task_id("T001"),
                    vec![ContractedEntryRef::new(layer("domain"), entry_key("Foo"))],
                )],
            )))),
            Arc::new(ConstSignalReader(Ok(make_signals(vec![blue_signal("Foo")])))),
            Arc::new(FailingImplPlanReader),
        );

        let err = svc.check(cmd("my-track", "domain")).unwrap_err();
        match err {
            PreReviewGateError::ImplPlanReadFailed { message } => {
                assert!(
                    message.contains("read failed"),
                    "expected read-failed diagnostic, got: {message}"
                );
            }
            other => panic!("expected ImplPlanReadFailed, got {other}"),
        }
    }
}
