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
use std::path::PathBuf;
use std::sync::Arc;

use domain::TypeSignalsDocument;
use domain::tddd::catalogue_linter::FreeText;
use domain::tddd::catalogue_v2::TdddLayerBindingsPort;
// Re-export domain task_contract types accessible to the cli_driver primary adapter
// via usecase module path (architecture-rules.json: cli_driver may_depend_on [usecase] only).
pub use domain::task_contract::{
    CoverageVerifyOutcome, CoverageViolation, PreReviewGateOutcome, PreReviewGateViolation,
};
use thiserror::Error;

use crate::catalogue_document_loader::AttestedCatalogueDocumentLoaderPort;

// Pure-helper free functions extracted to a sibling module to keep this file
// under the workspace `verify-module-size` cap (700 non-test lines, see ADR
// `2026-06-06-1609-enforce-module-size-limit-splitting`). The glob import
// keeps call sites unchanged.
mod helpers;
use helpers::{
    blocked_coverage_outcome, blocked_outcome, build_scope_entries, check_signal_document,
    collect_non_canonical_layer_violations, collect_per_layer_violations,
    collect_task_key_ri_violations, load_catalogue,
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
///   `message` is opaque diagnostic [`FreeText`] (R9: opaque infrastructure error message).
/// - `CatalogueReadFailed`: I/O or decode error reading a layer catalogue;
///   `layer` identifies the affected TDDD layer.
/// - `CatalogueFreshnessMismatch`: the loaded catalogue's declaration hash
///   differs from the validated type-signals document's attested hash.
/// - `SignalReadFailed`: I/O or decode error reading the per-layer type-signals
///   document; `layer` is typed as `domain::tddd::LayerId` (the port takes
///   `&LayerId` so the error always originates from a valid `LayerId`), `message`
///   is an opaque diagnostic string.
/// - `ImplPlanReadFailed`: I/O or decode error reading `impl-plan.json` (D7:
///   added for [`ImplPlanReaderPort`]); `message` is opaque diagnostic [`FreeText`].
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
        message: FreeText,
    },

    /// I/O or decode error reading the catalogue for a TDDD layer.
    #[error("failed to read catalogue for layer '{layer}': {message}")]
    CatalogueReadFailed {
        /// The TDDD layer whose catalogue could not be read.
        layer: domain::tddd::LayerId,
        /// Opaque diagnostic message from the infrastructure adapter.
        message: FreeText,
    },

    /// The loaded catalogue does not match the declaration hash attested by
    /// the already-validated type-signals document.
    #[error("catalogue freshness mismatch for layer '{layer}': {message}")]
    CatalogueFreshnessMismatch {
        /// The TDDD layer whose catalogue is stale relative to its signals.
        layer: domain::tddd::LayerId,
        /// Diagnostic describing the failed freshness comparison.
        message: FreeText,
    },

    /// I/O or decode error reading the per-layer `<layer>-type-signals.json`.
    #[error("failed to read type-signals for layer '{layer}': {message}")]
    SignalReadFailed {
        /// The TDDD layer whose signal document could not be read.
        layer: domain::tddd::LayerId,
        /// Opaque diagnostic message from the infrastructure adapter.
        message: FreeText,
    },

    /// I/O or decode error reading `impl-plan.json` (D7).
    #[error("failed to read impl-plan.json: {message}")]
    ImplPlanReadFailed {
        /// Opaque diagnostic message from the infrastructure adapter.
        message: FreeText,
    },
}

/// Failure produced while reading a task-contract document.
#[derive(Debug, Error)]
pub enum TaskContractReadError {
    /// The task-contract document does not exist for the requested track.
    #[error("task-contract.json not found for track")]
    NotFound,

    /// I/O or decoding failed while reading the task-contract document.
    #[error("failed to read task-contract.json: {message}")]
    ReadFailed { message: FreeText },
}

/// Failure produced while reading implementation-catalogue signals.
#[derive(Debug, Error)]
pub enum ImplCatalogSignalReadError {
    /// I/O or decoding failed while reading a layer's signal document.
    #[error("failed to read type-signals for layer '{layer}': {message}")]
    ReadFailed { layer: domain::tddd::LayerId, message: FreeText },
}

/// Failure produced while reading implementation-plan task statuses.
#[derive(Debug, Error)]
pub enum ImplPlanReadError {
    /// I/O or decoding failed while reading the implementation plan.
    #[error("failed to read impl-plan.json: {message}")]
    ReadFailed { message: FreeText },
}

impl From<TaskContractReadError> for PreReviewGateError {
    fn from(error: TaskContractReadError) -> Self {
        match error {
            TaskContractReadError::NotFound => Self::TaskContractNotFound,
            TaskContractReadError::ReadFailed { message } => {
                Self::TaskContractReadFailed { message }
            }
        }
    }
}

impl From<ImplCatalogSignalReadError> for PreReviewGateError {
    fn from(error: ImplCatalogSignalReadError) -> Self {
        match error {
            ImplCatalogSignalReadError::ReadFailed { layer, message } => {
                Self::SignalReadFailed { layer, message }
            }
        }
    }
}

impl From<ImplPlanReadError> for PreReviewGateError {
    fn from(error: ImplPlanReadError) -> Self {
        match error {
            ImplPlanReadError::ReadFailed { message } => Self::ImplPlanReadFailed { message },
        }
    }
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
    /// Returns [`TaskContractReadError::NotFound`] when the file does not exist;
    /// [`TaskContractReadError::ReadFailed`] on I/O or decode errors.
    fn read(
        &self,
        track_id: &domain::TrackId,
    ) -> Result<domain::task_contract::TaskContractDocument, TaskContractReadError>;
}

/// Secondary port for reading a per-layer `<layer>-type-signals.json` document.
///
/// Implemented by
/// `infrastructure::impl_catalog_signal_reader::FsImplCatalogSignalReader`.
pub trait ImplCatalogSignalReaderPort: Send + Sync {
    /// Read the per-layer `impl_catalog` type-signals document for the given track
    /// and layer.
    ///
    /// Returns [`ImplCatalogSignalReadError::ReadFailed`] on I/O or decode errors.
    fn read_signals(
        &self,
        track_id: &domain::TrackId,
        layer: &domain::tddd::LayerId,
    ) -> Result<TypeSignalsDocument, ImplCatalogSignalReadError>;

    /// Read the per-layer signal document when absence is expected state.
    /// `Ok(None)` only on positively-classified absent docs; default fail-closed.
    fn read_optional_signals(
        &self,
        track_id: &domain::TrackId,
        layer: &domain::tddd::LayerId,
    ) -> Result<Option<TypeSignalsDocument>, ImplCatalogSignalReadError> {
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
    ) -> Result<HashMap<domain::TaskId, domain::TaskStatusKind>, ImplPlanReadError>;
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
/// Holds the task-contract, signal, implementation-plan, and catalogue loader
/// dependencies. It also receives the repository workspace root explicitly so
/// layer bindings never depend on the shape of `items_dir`. It checks that every
/// signaled entry is attributed to at least one task (orphan detection) and
/// every attributed entry has a signal (referential integrity). Shares the
/// task-contract, signal, and plan ports with `PreReviewGateInteractor` (reuse
/// per spec `IN-07`).
pub struct CoverageVerifyInteractor {
    task_contract_reader: Arc<dyn TaskContractReaderPort>,
    signal_reader: Arc<dyn ImplCatalogSignalReaderPort>,
    impl_plan_reader: Arc<dyn ImplPlanReaderPort>,
    catalogue_loader: Arc<dyn AttestedCatalogueDocumentLoaderPort>,
    layer_bindings: Arc<dyn TdddLayerBindingsPort>,
    workspace_root: PathBuf,
    items_dir: PathBuf,
}

impl CoverageVerifyInteractor {
    /// Construct by injecting the task-contract, signal, plan, and catalogue
    /// dependencies; `impl_plan_reader` enables the D9 task-key referential
    /// integrity check, `workspace_root` anchors layer-binding resolution, and
    /// `items_dir` anchors per-layer catalogue loading.
    #[must_use]
    pub fn new(
        task_contract_reader: Arc<dyn TaskContractReaderPort>,
        signal_reader: Arc<dyn ImplCatalogSignalReaderPort>,
        impl_plan_reader: Arc<dyn ImplPlanReaderPort>,
        catalogue_loader: Arc<dyn AttestedCatalogueDocumentLoaderPort>,
        layer_bindings: Arc<dyn TdddLayerBindingsPort>,
        workspace_root: PathBuf,
        items_dir: PathBuf,
    ) -> Self {
        Self {
            task_contract_reader,
            signal_reader,
            impl_plan_reader,
            catalogue_loader,
            layer_bindings,
            workspace_root,
            items_dir,
        }
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
            Err(TaskContractReadError::NotFound) => {
                return Ok(CoverageVerifyOutcome::Passed);
            }
            Err(e) => return Err(e.into()),
        };

        let mut all_violations: Vec<domain::task_contract::CoverageViolation> = Vec::new();
        for &layer_str in CANONICAL_LAYERS {
            let Ok(layer) = domain::tddd::LayerId::try_new(layer_str.to_owned()) else { continue };
            let Some(signal_doc) = self
                .signal_reader
                .read_optional_signals(&cmd.track_id, &layer)
                .map_err(PreReviewGateError::from)?
            else {
                all_violations.push(
                    domain::task_contract::CoverageViolation::MissingSignalDocument(layer.clone()),
                );
                continue;
            };
            let attested = load_catalogue(
                self.catalogue_loader.as_ref(),
                self.layer_bindings.as_ref(),
                &self.workspace_root,
                &self.items_dir,
                &cmd.track_id,
                &layer,
                &signal_doc,
            )?;
            let catalogue = attested.document();
            let scope_entries = build_scope_entries(&signal_doc, &layer)?;
            all_violations.extend(collect_per_layer_violations(
                &contract_doc,
                &layer,
                catalogue,
                &scope_entries,
            ));
        }
        all_violations.extend(collect_non_canonical_layer_violations(&contract_doc));
        let plan_task_ids = self
            .impl_plan_reader
            .read_task_statuses(&cmd.track_id)
            .map_err(PreReviewGateError::from)?;
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
/// Holds seven injected dependencies:
/// - `task_contract_reader` reads `task-contract.json` for the active track.
/// - `signal_reader` reads per-layer `impl_catalog` type-signal documents.
/// - `impl_plan_reader` reads `impl-plan.json` for task-status filtering (D7).
/// - `catalogue_loader` reads the layer catalogue used to resolve each contract
///   entry's type/trait/function namespace.
/// - `workspace_root` anchors `architecture-rules.json` resolution.
/// - `items_dir` anchors the catalogue paths for the active track.
///
/// The interactor checks that all attributed entries for current/done tasks
/// have Blue `impl_catalog` signals.
pub struct PreReviewGateInteractor {
    task_contract_reader: Arc<dyn TaskContractReaderPort>,
    signal_reader: Arc<dyn ImplCatalogSignalReaderPort>,
    impl_plan_reader: Arc<dyn ImplPlanReaderPort>,
    catalogue_loader: Arc<dyn AttestedCatalogueDocumentLoaderPort>,
    layer_bindings: Arc<dyn TdddLayerBindingsPort>,
    workspace_root: PathBuf,
    items_dir: PathBuf,
}

impl PreReviewGateInteractor {
    /// Construct a `PreReviewGateInteractor` by injecting its readers, the
    /// workspace root used for layer-binding resolution, and the catalogue
    /// path root used for namespace resolution.
    #[must_use]
    pub fn new(
        task_contract_reader: Arc<dyn TaskContractReaderPort>,
        signal_reader: Arc<dyn ImplCatalogSignalReaderPort>,
        impl_plan_reader: Arc<dyn ImplPlanReaderPort>,
        catalogue_loader: Arc<dyn AttestedCatalogueDocumentLoaderPort>,
        layer_bindings: Arc<dyn TdddLayerBindingsPort>,
        workspace_root: PathBuf,
        items_dir: PathBuf,
    ) -> Self {
        Self {
            task_contract_reader,
            signal_reader,
            impl_plan_reader,
            catalogue_loader,
            layer_bindings,
            workspace_root,
            items_dir,
        }
    }
}

/// Canonical TDDD layer identifiers iterated in all-layers mode.
const CANONICAL_LAYERS: &[&str] =
    &["domain", "usecase", "infrastructure", "cli_driver", "cli", "cli_composition"];

impl PreReviewGateInteractor {
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
        let signal_doc =
            self.signal_reader.read_signals(track_id, layer).map_err(PreReviewGateError::from)?;
        let attested = load_catalogue(
            self.catalogue_loader.as_ref(),
            self.layer_bindings.as_ref(),
            &self.workspace_root,
            &self.items_dir,
            track_id,
            layer,
            &signal_doc,
        )?;
        check_signal_document(layer, contract_doc, attested.document(), &signal_doc, task_statuses)
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
            Err(TaskContractReadError::NotFound) => {
                return Ok(PreReviewGateOutcome::Passed);
            }
            Err(e) => return Err(e.into()),
        };

        // ── Step 0b: load impl-plan.json task statuses (D7) ──────────────────
        //
        // Used to filter attributions by task status: done/in_progress entries
        // require Blue; entries without a done/in_progress owner tolerate Yellow;
        // Red always blocks.
        let task_statuses = self
            .impl_plan_reader
            .read_task_statuses(&cmd.track_id)
            .map_err(PreReviewGateError::from)?;

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
                    match self
                        .signal_reader
                        .read_optional_signals(&cmd.track_id, &layer)
                        .map_err(PreReviewGateError::from)?
                    {
                        Some(signal_doc) => {
                            let attested = load_catalogue(
                                self.catalogue_loader.as_ref(),
                                self.layer_bindings.as_ref(),
                                &self.workspace_root,
                                &self.items_dir,
                                &cmd.track_id,
                                &layer,
                                &signal_doc,
                            )?;
                            let violations = check_signal_document(
                                &layer,
                                &contract_doc,
                                attested.document(),
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
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    use domain::TaskStatusKind;
    use domain::task_contract::{
        ContractedEntryRef, CoverageVerifyOutcome, CoverageViolation, PreReviewGateOutcome,
        PreReviewGateViolation, TaskContractDocument,
    };
    use domain::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
    use domain::tddd::catalogue_v2::entries::{TraitEntry, TypeEntry};
    use domain::tddd::catalogue_v2::identifiers::{CatalogueItemNamespace, CrateName, ModulePath};
    use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole, ItemAction};
    use domain::tddd::catalogue_v2::{
        AttestedCatalogueDocument, CatalogueDocument, CatalogueDocumentLoaderError,
        TdddLayerBinding, TdddLayerBindingsError, TdddLayerBindingsPort,
    };
    use domain::tddd::semantic_verify::CatalogueEntryKey;
    use domain::tddd::signal_evaluator::ThreeWaySignalIdentity;
    use domain::tddd::{LayerId, type_signals_doc::TypeSignalsDocument};
    use domain::{ConfidenceSignal, FreeText, TaskId, Timestamp, TrackId, TypeSignal};

    use super::{
        CoverageVerifyCommand, CoverageVerifyInteractor, CoverageVerifyService,
        ImplCatalogSignalReadError, ImplCatalogSignalReaderPort, ImplPlanReadError,
        ImplPlanReaderPort, PreReviewGateCommand, PreReviewGateError, PreReviewGateInteractor,
        PreReviewGateService, TaskContractReadError, TaskContractReaderPort,
    };
    use crate::catalogue_document_loader::AttestedCatalogueDocumentLoaderPort;

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
        TypeSignal::new(
            ThreeWaySignalIdentity::CatalogueItem {
                item_name: FreeText::new(name),
                namespace: CatalogueItemNamespace::Type,
            },
            "struct".to_owned(),
            ConfidenceSignal::Blue,
            true,
            vec![],
            vec![],
            vec![],
        )
    }

    fn yellow_signal(name: &str) -> TypeSignal {
        TypeSignal::new(
            ThreeWaySignalIdentity::CatalogueItem {
                item_name: FreeText::new(name),
                namespace: CatalogueItemNamespace::Type,
            },
            "struct".to_owned(),
            ConfidenceSignal::Yellow,
            false,
            vec![],
            vec![],
            vec![],
        )
    }

    fn catalogue_signal(
        name: &str,
        namespace: CatalogueItemNamespace,
        signal: ConfidenceSignal,
    ) -> TypeSignal {
        let kind_tag = match namespace {
            CatalogueItemNamespace::Type => "struct",
            CatalogueItemNamespace::Trait => "secondary_port",
        };
        TypeSignal::new(
            ThreeWaySignalIdentity::CatalogueItem { item_name: FreeText::new(name), namespace },
            kind_tag.to_owned(),
            signal,
            true,
            vec![],
            vec![],
            vec![],
        )
    }

    fn unknown_signal(name: &str) -> TypeSignal {
        TypeSignal::new(
            ThreeWaySignalIdentity::Label { label: FreeText::new(name) },
            "unknown".to_owned(),
            ConfidenceSignal::Yellow,
            true,
            vec![],
            vec![],
            vec![],
        )
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

    fn test_type_entry() -> TypeEntry {
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
            vec![],
            vec![],
            vec![],
            Some(ModulePath::root()),
            None,
            vec![],
            vec![],
        )
    }

    fn test_trait_entry() -> TraitEntry {
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SecondaryPort,
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            Some(ModulePath::root()),
            None,
            vec![],
            vec![],
        )
    }

    fn type_only_catalogue() -> CatalogueDocument {
        let mut document =
            CatalogueDocument::new(5, CrateName::new("domain").unwrap(), layer("domain"));
        for name in [
            "Foo",
            "Bar",
            "UseFoo",
            "UseBar",
            "Missing",
            "domain::alpha::Shared",
            "domain::beta::Shared",
        ] {
            document.insert_type(entry_key(name), test_type_entry());
        }
        document.insert_type(entry_key("Shared"), test_type_entry());
        document
    }

    fn ambiguous_catalogue() -> CatalogueDocument {
        let mut document = type_only_catalogue();
        document.insert_trait(entry_key("Shared"), test_trait_entry());
        document
    }

    struct ConstCatalogueReader(AttestedCatalogueDocument);

    impl AttestedCatalogueDocumentLoaderPort for ConstCatalogueReader {
        fn load(
            &self,
            _path: &Path,
        ) -> Result<AttestedCatalogueDocument, CatalogueDocumentLoaderError> {
            Ok(self.0.clone())
        }
    }

    struct RecordingCatalogueReader {
        document: AttestedCatalogueDocument,
        paths: Arc<Mutex<Vec<PathBuf>>>,
    }

    impl AttestedCatalogueDocumentLoaderPort for RecordingCatalogueReader {
        fn load(
            &self,
            path: &Path,
        ) -> Result<AttestedCatalogueDocument, CatalogueDocumentLoaderError> {
            self.paths.lock().unwrap().push(path.to_path_buf());
            Ok(self.document.clone())
        }
    }

    fn catalogue_reader() -> Arc<dyn AttestedCatalogueDocumentLoaderPort> {
        Arc::new(ConstCatalogueReader(test_catalogue_with_hash('a')))
    }

    fn ambiguous_catalogue_reader() -> Arc<dyn AttestedCatalogueDocumentLoaderPort> {
        Arc::new(ConstCatalogueReader(ambiguous_catalogue_with_hash('a')))
    }

    struct FailingCatalogueReader;

    impl AttestedCatalogueDocumentLoaderPort for FailingCatalogueReader {
        fn load(
            &self,
            path: &Path,
        ) -> Result<AttestedCatalogueDocument, CatalogueDocumentLoaderError> {
            Err(CatalogueDocumentLoaderError::NotFound { path: path.to_path_buf() })
        }
    }

    fn failing_catalogue_reader() -> Arc<dyn AttestedCatalogueDocumentLoaderPort> {
        Arc::new(FailingCatalogueReader)
    }

    fn attest_catalogue(
        document: CatalogueDocument,
        source_tag: char,
    ) -> AttestedCatalogueDocument {
        let source = format!("T014 catalogue source {source_tag}").into_bytes();
        AttestedCatalogueDocument::attest(&source, |_| Ok::<_, std::convert::Infallible>(document))
            .unwrap()
    }

    fn test_catalogue_with_hash(hash_byte: char) -> AttestedCatalogueDocument {
        attest_catalogue(type_only_catalogue(), hash_byte)
    }

    fn ambiguous_catalogue_with_hash(hash_byte: char) -> AttestedCatalogueDocument {
        attest_catalogue(ambiguous_catalogue(), hash_byte)
    }

    fn items_dir() -> PathBuf {
        PathBuf::from("track/items")
    }

    fn custom_items_dir() -> PathBuf {
        PathBuf::from("custom/track/items")
    }

    fn workspace_root() -> PathBuf {
        PathBuf::from("workspace")
    }

    fn make_signals(signals: Vec<TypeSignal>) -> TypeSignalsDocument {
        make_signals_with_hash(signals, 'a')
    }

    fn test_cache_key(
        declaration_hash: domain::CatalogueDeclarationHash,
        head_commit: domain::CommitHash,
        baseline_hash: domain::BaselineHash,
    ) -> domain::TypeSignalsCacheKey {
        let target = domain::ResolvedCargoTargetDirectory::try_new(std::path::PathBuf::from(
            "/tmp/sotohe-usecase-test-target",
        ))
        .unwrap();
        let expected = domain::ExpectedRustdocJsonPath::try_new(
            target.as_path().join("doc/legacy.json"),
            &target,
        )
        .unwrap();
        let identity = domain::RustdocExecutionIdentity::new(
            target,
            domain::tddd::catalogue_v2::CrateName::new("legacy").unwrap(),
            vec![],
            domain::CargoProfileName::try_new("dev".to_owned()).unwrap(),
            expected,
        )
        .unwrap();
        let zero = domain::Sha256Digest::try_new("0".repeat(64)).unwrap();
        domain::TypeSignalsCacheKey::new(
            declaration_hash,
            head_commit,
            baseline_hash,
            domain::ImplementationFingerprint::new(zero.clone()),
            domain::ResolutionFingerprint::new(zero),
            identity,
        )
    }

    fn make_signals_with_hash(signals: Vec<TypeSignal>, hash_byte: char) -> TypeSignalsDocument {
        let declaration_hash = test_catalogue_with_hash(hash_byte).declaration_hash().clone();
        let baseline_digest =
            domain::Sha256Digest::try_new(hash_byte.to_string().repeat(64)).unwrap();
        TypeSignalsDocument::new(
            ts("2026-06-27T00:00:00Z"),
            test_cache_key(
                declaration_hash,
                domain::CommitHash::try_new("a".repeat(40)).unwrap(),
                domain::BaselineHash::new(baseline_digest),
            ),
            signals,
        )
    }

    fn assert_liveness_violations(
        outcome: PreReviewGateOutcome,
        expected: Vec<PreReviewGateViolation>,
    ) {
        match outcome {
            PreReviewGateOutcome::Blocked(violations) => {
                assert_eq!(violations.as_slice(), expected.as_slice());
            }
            PreReviewGateOutcome::Passed => panic!("expected liveness gate to be blocked"),
        }
    }

    fn assert_coverage_violations(
        outcome: CoverageVerifyOutcome,
        expected: Vec<CoverageViolation>,
    ) {
        match outcome {
            CoverageVerifyOutcome::Blocked(violations) => {
                assert_eq!(violations.as_slice(), expected.as_slice());
            }
            CoverageVerifyOutcome::Passed => panic!("expected coverage gate to be blocked"),
        }
    }

    fn missing_signal_documents(layers: &[&str]) -> Vec<CoverageViolation> {
        layers.iter().map(|name| CoverageViolation::MissingSignalDocument(layer(name))).collect()
    }

    // ── Mock implementations ──────────────────────────────────────────────────

    struct ConstContractReader(Result<TaskContractDocument, PreReviewGateError>);

    impl TaskContractReaderPort for ConstContractReader {
        fn read(
            &self,
            _track_id: &TrackId,
        ) -> Result<domain::task_contract::TaskContractDocument, TaskContractReadError> {
            match &self.0 {
                Ok(doc) => Ok(doc.clone()),
                Err(PreReviewGateError::TaskContractNotFound) => {
                    Err(TaskContractReadError::NotFound)
                }
                Err(PreReviewGateError::TaskContractReadFailed { message }) => {
                    Err(TaskContractReadError::ReadFailed { message: message.clone() })
                }
                Err(error) => Err(TaskContractReadError::ReadFailed {
                    message: domain::FreeText::new(error.to_string()),
                }),
            }
        }
    }

    struct ConstSignalReader(Result<TypeSignalsDocument, PreReviewGateError>);

    impl ImplCatalogSignalReaderPort for ConstSignalReader {
        fn read_signals(
            &self,
            _track_id: &TrackId,
            _layer: &LayerId,
        ) -> Result<TypeSignalsDocument, ImplCatalogSignalReadError> {
            match &self.0 {
                Ok(doc) => Ok(doc.clone()),
                Err(PreReviewGateError::SignalReadFailed { layer, message }) => {
                    Err(ImplCatalogSignalReadError::ReadFailed {
                        layer: layer.clone(),
                        message: message.clone(),
                    })
                }
                Err(error) => Err(ImplCatalogSignalReadError::ReadFailed {
                    layer: LayerId::try_new("domain".to_owned()).expect("valid test layer"),
                    message: domain::FreeText::new(error.to_string()),
                }),
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
        ) -> Result<TypeSignalsDocument, ImplCatalogSignalReadError> {
            match self.0.get(layer.as_ref()) {
                Some(doc) => Ok(doc.clone()),
                None => Err(ImplCatalogSignalReadError::ReadFailed {
                    layer: layer.clone(),
                    message: domain::FreeText::new(format!(
                        "no signal document for layer '{}'",
                        layer.as_ref()
                    )),
                }),
            }
        }

        fn read_optional_signals(
            &self,
            _track_id: &TrackId,
            layer: &LayerId,
        ) -> Result<Option<TypeSignalsDocument>, ImplCatalogSignalReadError> {
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
        ) -> Result<TypeSignalsDocument, ImplCatalogSignalReadError> {
            Err(ImplCatalogSignalReadError::ReadFailed {
                layer: layer.clone(),
                message: domain::FreeText::new(self.message),
            })
        }
    }

    struct ConstLayerBindings {
        catalogue_file: String,
    }

    impl TdddLayerBindingsPort for ConstLayerBindings {
        fn load(
            &self,
            _workspace_root: &Path,
            layer_filter: Option<&str>,
        ) -> Result<Vec<TdddLayerBinding>, TdddLayerBindingsError> {
            Ok(vec![TdddLayerBinding {
                layer_id: layer_filter.unwrap_or("domain").to_owned(),
                catalogue_file: self.catalogue_file.clone(),
                baseline_file: "domain-types-baseline.json".to_owned(),
                targets: vec!["domain".to_owned()],
            }])
        }
    }

    struct RecordingLayerBindings {
        catalogue_file: String,
        roots: Arc<Mutex<Vec<PathBuf>>>,
    }

    impl TdddLayerBindingsPort for RecordingLayerBindings {
        fn load(
            &self,
            workspace_root: &Path,
            layer_filter: Option<&str>,
        ) -> Result<Vec<TdddLayerBinding>, TdddLayerBindingsError> {
            self.roots.lock().unwrap().push(workspace_root.to_path_buf());
            Ok(vec![TdddLayerBinding {
                layer_id: layer_filter.unwrap_or("domain").to_owned(),
                catalogue_file: self.catalogue_file.clone(),
                baseline_file: "domain-types-baseline.json".to_owned(),
                targets: vec!["domain".to_owned()],
            }])
        }
    }

    fn layer_bindings() -> Arc<dyn TdddLayerBindingsPort> {
        layer_bindings_with_catalogue_file("domain-types.json")
    }

    fn layer_bindings_with_catalogue_file(file: &str) -> Arc<dyn TdddLayerBindingsPort> {
        Arc::new(ConstLayerBindings { catalogue_file: file.to_owned() })
    }

    /// Const impl-plan reader that always returns an empty task-status map.
    struct EmptyImplPlanReader;

    impl ImplPlanReaderPort for EmptyImplPlanReader {
        fn read_task_statuses(
            &self,
            _track_id: &TrackId,
        ) -> Result<std::collections::HashMap<TaskId, TaskStatusKind>, ImplPlanReadError> {
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
            catalogue_reader(),
            layer_bindings(),
            workspace_root(),
            items_dir(),
        )
    }

    #[test]
    fn reader_ports_are_interactor_dependencies_and_are_invoked() {
        let source = include_str!("pre_review_gate.rs");
        let production_source = source.split("#[cfg(test)]").next().unwrap();

        for required_fragment in [
            "pub trait ImplCatalogSignalReaderPort: Send + Sync",
            "fn read_optional_signals(",
            "pub trait ImplPlanReaderPort: Send + Sync",
            "fn read_task_statuses(",
            "signal_reader: Arc<dyn ImplCatalogSignalReaderPort>",
            "impl_plan_reader: Arc<dyn ImplPlanReaderPort>",
            "read_optional_signals(&cmd.track_id, &layer)",
            "read_task_statuses(&cmd.track_id)",
        ] {
            assert!(
                production_source.contains(required_fragment),
                "pre-review interactor must declare, receive, and invoke {required_fragment}"
            );
        }
        for forbidden_runtime_path in
            ["ServiceImpl", "CompatibilityShim", "CompatService", "CompositionRoot"]
        {
            assert!(
                !production_source.contains(forbidden_runtime_path),
                "signal-reader execution must not reference or reverse-delegate through {forbidden_runtime_path}"
            );
        }

        let mut statuses = std::collections::HashMap::new();
        statuses.insert(task_id("T001"), TaskStatusKind::InProgress);
        let service = PreReviewGateInteractor::new(
            Arc::new(ConstContractReader(Ok(make_contract(
                "my-track",
                vec![(
                    task_id("T001"),
                    vec![ContractedEntryRef::new(layer("domain"), entry_key("Foo"))],
                )],
            )))),
            Arc::new(LayerAwareSignalReader(std::collections::HashMap::from([(
                "domain".to_owned(),
                make_signals(vec![blue_signal("Foo")]),
            )]))),
            Arc::new(FixedImplPlanReader(statuses)),
            catalogue_reader(),
            layer_bindings(),
            workspace_root(),
            items_dir(),
        );

        assert!(
            matches!(
                service.check(cmd("my-track", "domain")).unwrap(),
                PreReviewGateOutcome::Passed
            ),
            "the injected reader ports must drive the pre-review result"
        );
    }

    fn cmd(track: &str, group: &str) -> PreReviewGateCommand {
        PreReviewGateCommand { track_id: track_id(track), layer: Some(layer(group)) }
    }

    #[test]
    fn catalogue_hash_mismatch_fails_closed_before_namespace_resolution() {
        let service = PreReviewGateInteractor::new(
            Arc::new(ConstContractReader(Ok(make_contract(
                "my-track",
                vec![(
                    task_id("T001"),
                    vec![ContractedEntryRef::new(layer("domain"), entry_key("Foo"))],
                )],
            )))),
            Arc::new(ConstSignalReader(Ok(make_signals_with_hash(vec![blue_signal("Foo")], 'a')))),
            Arc::new(EmptyImplPlanReader),
            Arc::new(ConstCatalogueReader(test_catalogue_with_hash('b'))),
            layer_bindings(),
            workspace_root(),
            items_dir(),
        );

        let error = service.check(cmd("my-track", "domain")).unwrap_err();

        assert!(matches!(
            error,
            PreReviewGateError::CatalogueFreshnessMismatch { layer, message }
                if layer.as_ref() == "domain"
                    && message.as_str()
                        == "catalogue changed between signal validation and namespace resolution"
        ));
    }

    #[test]
    fn catalogue_read_failure_is_reported_as_catalogue_error() {
        let service = PreReviewGateInteractor::new(
            Arc::new(ConstContractReader(Ok(make_contract(
                "my-track",
                vec![(
                    task_id("T001"),
                    vec![ContractedEntryRef::new(layer("domain"), entry_key("Foo"))],
                )],
            )))),
            Arc::new(ConstSignalReader(Ok(make_signals(vec![blue_signal("Foo")])))),
            Arc::new(EmptyImplPlanReader),
            failing_catalogue_reader(),
            layer_bindings(),
            workspace_root(),
            items_dir(),
        );

        let error = service.check(cmd("my-track", "domain")).unwrap_err();
        assert!(matches!(
            error,
            PreReviewGateError::CatalogueReadFailed { layer, message }
                if layer.as_ref() == "domain" && message.as_str().contains("catalogue file not found")
        ));
    }

    #[test]
    fn configured_catalogue_filename_is_used_for_namespace_resolution() {
        let paths = Arc::new(Mutex::new(Vec::new()));
        let service = PreReviewGateInteractor::new(
            Arc::new(ConstContractReader(Ok(make_contract(
                "my-track",
                vec![(
                    task_id("T001"),
                    vec![ContractedEntryRef::new(layer("domain"), entry_key("Foo"))],
                )],
            )))),
            Arc::new(ConstSignalReader(Ok(make_signals(vec![blue_signal("Foo")])))),
            Arc::new(EmptyImplPlanReader),
            Arc::new(RecordingCatalogueReader {
                document: test_catalogue_with_hash('a'),
                paths: Arc::clone(&paths),
            }),
            layer_bindings_with_catalogue_file("custom-domain-types.json"),
            workspace_root(),
            items_dir(),
        );

        assert!(matches!(
            service.check(cmd("my-track", "domain")),
            Ok(PreReviewGateOutcome::Passed)
        ));
        assert_eq!(
            paths.lock().unwrap().as_slice(),
            [PathBuf::from("track/items/my-track/custom-domain-types.json")].as_slice()
        );
    }

    #[test]
    fn custom_items_dir_uses_explicit_workspace_root_for_liveness_and_coverage() {
        let contract_document = make_contract(
            "my-track",
            vec![(
                task_id("T001"),
                vec![ContractedEntryRef::new(layer("domain"), entry_key("Foo"))],
            )],
        );
        let liveness_contract = Ok(contract_document.clone());
        let coverage_contract = Ok(contract_document);
        let liveness_roots = Arc::new(Mutex::new(Vec::new()));
        let liveness = PreReviewGateInteractor::new(
            Arc::new(ConstContractReader(liveness_contract)),
            Arc::new(ConstSignalReader(Ok(make_signals(vec![blue_signal("Foo")])))),
            Arc::new(EmptyImplPlanReader),
            Arc::new(RecordingCatalogueReader {
                document: test_catalogue_with_hash('a'),
                paths: Arc::new(Mutex::new(Vec::new())),
            }),
            Arc::new(RecordingLayerBindings {
                catalogue_file: "domain-types.json".to_owned(),
                roots: Arc::clone(&liveness_roots),
            }),
            workspace_root(),
            custom_items_dir(),
        );
        assert!(matches!(
            liveness.check(cmd("my-track", "domain")),
            Ok(PreReviewGateOutcome::Passed)
        ));
        assert_eq!(liveness_roots.lock().unwrap().as_slice(), [workspace_root()].as_slice());

        let coverage_roots = Arc::new(Mutex::new(Vec::new()));
        let coverage = CoverageVerifyInteractor::new(
            Arc::new(ConstContractReader(coverage_contract)),
            Arc::new(LayerAwareSignalReader(std::collections::HashMap::from([(
                "domain".to_owned(),
                make_signals(vec![blue_signal("Foo")]),
            )]))),
            plan_reader_matching_contract(&Ok(make_contract(
                "my-track",
                vec![(
                    task_id("T001"),
                    vec![ContractedEntryRef::new(layer("domain"), entry_key("Foo"))],
                )],
            ))),
            Arc::new(RecordingCatalogueReader {
                document: test_catalogue_with_hash('a'),
                paths: Arc::new(Mutex::new(Vec::new())),
            }),
            Arc::new(RecordingLayerBindings {
                catalogue_file: "domain-types.json".to_owned(),
                roots: Arc::clone(&coverage_roots),
            }),
            workspace_root(),
            custom_items_dir(),
        );
        let coverage_outcome = coverage.verify_coverage(coverage_cmd("my-track")).unwrap();
        assert!(matches!(coverage_outcome, CoverageVerifyOutcome::Blocked(_)));
        assert_eq!(coverage_roots.lock().unwrap().as_slice(), [workspace_root()].as_slice());
    }

    #[test]
    fn items_dir_outside_workspace_root_fails_closed_before_binding_resolution() {
        let roots = Arc::new(Mutex::new(Vec::new()));
        let service = PreReviewGateInteractor::new(
            Arc::new(ConstContractReader(Ok(make_contract(
                "my-track",
                vec![(
                    task_id("T001"),
                    vec![ContractedEntryRef::new(layer("domain"), entry_key("Foo"))],
                )],
            )))),
            Arc::new(ConstSignalReader(Ok(make_signals(vec![blue_signal("Foo")])))),
            Arc::new(EmptyImplPlanReader),
            catalogue_reader(),
            Arc::new(RecordingLayerBindings {
                catalogue_file: "domain-types.json".to_owned(),
                roots: Arc::clone(&roots),
            }),
            PathBuf::from("/repository-a"),
            PathBuf::from("/repository-b/custom/track/items"),
        );

        let error = service.check(cmd("my-track", "domain")).unwrap_err();

        assert!(matches!(
            error,
            PreReviewGateError::CatalogueReadFailed { layer, message }
                if layer.as_ref() == "domain"
                    && message.as_str().contains("outside workspace root")
        ));
        assert!(
            roots.lock().unwrap().is_empty(),
            "layer bindings must not be resolved after repository context validation fails"
        );
    }

    #[test]
    fn catalogue_hash_validation_is_request_scoped_for_same_path() {
        let contract = make_contract(
            "my-track",
            vec![(
                task_id("T001"),
                vec![ContractedEntryRef::new(layer("domain"), entry_key("Foo"))],
            )],
        );
        let first_request = PreReviewGateInteractor::new(
            Arc::new(ConstContractReader(Ok(contract.clone()))),
            Arc::new(ConstSignalReader(Ok(make_signals_with_hash(vec![blue_signal("Foo")], 'a')))),
            Arc::new(EmptyImplPlanReader),
            Arc::new(ConstCatalogueReader(test_catalogue_with_hash('a'))),
            layer_bindings(),
            workspace_root(),
            items_dir(),
        );
        let second_request = PreReviewGateInteractor::new(
            Arc::new(ConstContractReader(Ok(contract))),
            Arc::new(ConstSignalReader(Ok(make_signals_with_hash(vec![blue_signal("Foo")], 'b')))),
            Arc::new(EmptyImplPlanReader),
            Arc::new(ConstCatalogueReader(test_catalogue_with_hash('b'))),
            layer_bindings(),
            workspace_root(),
            items_dir(),
        );

        // Each call carries its own decoded signal hash and loaded catalogue
        // attestation. There is no process-global path cache for concurrent
        // calls to cross-consume.
        std::thread::scope(|scope| {
            let first = scope.spawn(|| first_request.check(cmd("my-track", "domain")));
            let second = scope.spawn(|| second_request.check(cmd("my-track", "domain")));
            assert!(matches!(first.join().unwrap(), Ok(PreReviewGateOutcome::Passed)));
            assert!(matches!(second.join().unwrap(), Ok(PreReviewGateOutcome::Passed)));
        });
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
        let invalid_signal = TypeSignal::new(
            ThreeWaySignalIdentity::Label { label: FreeText::new("   ") },
            "free_function".to_owned(),
            ConfidenceSignal::Blue,
            true,
            vec![],
            vec![],
            vec![],
        );
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
                    message.as_str().contains("invalid entry key"),
                    "expected invalid entry key diagnostic, got: {message}"
                );
            }
            other => panic!("expected SignalReadFailed, got {other}"),
        }
    }

    #[test]
    fn test_check_duplicate_signal_identity_returns_signal_read_failed() {
        for (first, second) in [
            (ConfidenceSignal::Blue, ConfidenceSignal::Yellow),
            (ConfidenceSignal::Yellow, ConfidenceSignal::Blue),
        ] {
            let svc = interactor(
                Ok(make_contract(
                    "my-track",
                    vec![(
                        task_id("T001"),
                        vec![ContractedEntryRef::new(layer("domain"), entry_key("Foo"))],
                    )],
                )),
                Ok(make_signals(vec![
                    catalogue_signal("Foo", CatalogueItemNamespace::Type, first),
                    catalogue_signal("Foo", CatalogueItemNamespace::Type, second),
                ])),
            );

            let err = svc.check(cmd("my-track", "domain")).unwrap_err();
            match err {
                PreReviewGateError::SignalReadFailed { layer, message } => {
                    assert_eq!(layer.as_ref(), "domain");
                    assert!(message.as_str().contains("duplicate signal identity"));
                }
                other => panic!("expected duplicate identity to fail closed, got {other}"),
            }
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
        assert_liveness_violations(
            outcome,
            vec![PreReviewGateViolation::NonBlueSignal(
                ContractedEntryRef::new(layer("domain"), entry_key("Foo")),
                ConfidenceSignal::Yellow,
            )],
        );
    }

    /// Uses the type-only fixture; see `same_named_type_and_trait_contract_key_fails_closed_for_liveness_and_coverage` and `coverage_ambiguous_contract_key_fails_closed_for_same_named_rows`.
    #[test]
    fn test_pre_review_gate_attributes_duplicate_qualified_entry_keys_independently() {
        let alpha = entry_key("domain::alpha::Shared");
        let beta = entry_key("domain::beta::Shared");
        let statuses =
            std::collections::HashMap::from([(task_id("T001"), TaskStatusKind::InProgress)]);
        let service = PreReviewGateInteractor::new(
            Arc::new(ConstContractReader(Ok(make_contract(
                "my-track",
                vec![(
                    task_id("T001"),
                    vec![
                        ContractedEntryRef::new(layer("domain"), alpha.clone()),
                        ContractedEntryRef::new(layer("domain"), beta.clone()),
                    ],
                )],
            )))),
            Arc::new(ConstSignalReader(Ok(make_signals(vec![
                yellow_signal(alpha.as_str()),
                blue_signal(beta.as_str()),
            ])))),
            Arc::new(FixedImplPlanReader(statuses)),
            catalogue_reader(),
            layer_bindings(),
            workspace_root(),
            items_dir(),
        );

        let outcome = service.check(cmd("my-track", "domain")).unwrap();
        assert_liveness_violations(
            outcome,
            vec![PreReviewGateViolation::NonBlueSignal(
                ContractedEntryRef::new(layer("domain"), alpha),
                ConfidenceSignal::Yellow,
            )],
        );
    }

    #[test]
    fn same_named_type_and_trait_contract_key_fails_closed_for_liveness_and_coverage() {
        let shared = entry_key("Shared");
        let catalogue = ambiguous_catalogue();
        assert!(
            catalogue.types().contains_key(&shared),
            "the ambiguity fixture must register Shared as a Type"
        );
        assert!(
            catalogue.traits().contains_key(&shared),
            "the ambiguity fixture must register Shared as a Trait"
        );
        let contract = make_contract(
            "my-track",
            vec![(task_id("T001"), vec![ContractedEntryRef::new(layer("domain"), shared.clone())])],
        );
        let signal_doc = make_signals(vec![
            catalogue_signal("Shared", CatalogueItemNamespace::Type, ConfidenceSignal::Yellow),
            catalogue_signal("Shared", CatalogueItemNamespace::Trait, ConfidenceSignal::Blue),
        ]);

        let statuses =
            std::collections::HashMap::from([(task_id("T001"), TaskStatusKind::InProgress)]);
        let liveness = PreReviewGateInteractor::new(
            Arc::new(ConstContractReader(Ok(contract.clone()))),
            Arc::new(ConstSignalReader(Ok(signal_doc.clone()))),
            Arc::new(FixedImplPlanReader(statuses)),
            ambiguous_catalogue_reader(),
            layer_bindings(),
            workspace_root(),
            items_dir(),
        );
        let liveness_error = liveness.check(cmd("my-track", "domain")).unwrap_err();
        match liveness_error {
            PreReviewGateError::TaskContractReadFailed { message } => {
                assert!(message.as_str().contains("entry_key 'Shared'"));
                assert!(message.as_str().contains("no unique catalogue namespace"));
            }
            other => panic!("expected liveness ambiguity to fail closed, got {other}"),
        }

        let mut signal_docs = std::collections::HashMap::new();
        signal_docs.insert("domain".to_owned(), signal_doc);
        for layer_name in &["usecase", "infrastructure", "cli_driver", "cli", "cli_composition"] {
            signal_docs.insert((*layer_name).to_owned(), make_signals(vec![]));
        }
        let coverage = coverage_interactor_with_catalogue(
            Ok(contract),
            signal_docs,
            ambiguous_catalogue_reader(),
        );
        let coverage_outcome = coverage.verify_coverage(coverage_cmd("my-track")).unwrap();
        assert_coverage_violations(
            coverage_outcome,
            vec![
                CoverageViolation::OrphanEntry(ContractedEntryRef::new(
                    layer("domain"),
                    shared.clone(),
                )),
                CoverageViolation::OrphanEntry(ContractedEntryRef::new(
                    layer("domain"),
                    shared.clone(),
                )),
                CoverageViolation::InvalidEntryRef(
                    ContractedEntryRef::new(layer("domain"), shared),
                    domain::FreeText::new(
                        "entry_key 'Shared' has no unique catalogue namespace in domain: \
                         could not classify TypeRef path at `Shared`",
                    ),
                ),
            ],
        );
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
            catalogue_reader(),
            layer_bindings(),
            workspace_root(),
            items_dir(),
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
            catalogue_reader(),
            layer_bindings(),
            workspace_root(),
            items_dir(),
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
        let invalid_signal = TypeSignal::new(
            ThreeWaySignalIdentity::Label { label: FreeText::new("   ") },
            "free_function".to_owned(),
            ConfidenceSignal::Blue,
            true,
            vec![],
            vec![],
            vec![],
        );
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
            catalogue_reader(),
            layer_bindings(),
            workspace_root(),
            items_dir(),
        );

        let err = svc
            .check(PreReviewGateCommand { track_id: track_id("my-track"), layer: None })
            .unwrap_err();

        match err {
            PreReviewGateError::SignalReadFailed { layer, message } => {
                assert_eq!(layer.as_ref(), "domain");
                assert!(
                    message.as_str().contains("invalid entry key"),
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
            catalogue_reader(),
            layer_bindings(),
            workspace_root(),
            items_dir(),
        );

        let err = svc
            .check(PreReviewGateCommand { track_id: track_id("my-track"), layer: None })
            .unwrap_err();

        match err {
            PreReviewGateError::SignalReadFailed { layer, message } => {
                assert_eq!(layer.as_ref(), "domain");
                assert!(
                    message.as_str().contains("codec error"),
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
            catalogue_reader(),
            layer_bindings(),
            workspace_root(),
            items_dir(),
        );

        let err = svc
            .check(PreReviewGateCommand { track_id: track_id("my-track"), layer: None })
            .unwrap_err();

        match err {
            PreReviewGateError::SignalReadFailed { layer, message } => {
                assert_eq!(layer.as_ref(), "domain");
                assert!(
                    message.as_str().contains("signal file not found"),
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
        coverage_interactor_with_catalogue(contract, signal_docs, catalogue_reader())
    }

    fn coverage_interactor_with_catalogue(
        contract: Result<TaskContractDocument, PreReviewGateError>,
        signal_docs: std::collections::HashMap<String, TypeSignalsDocument>,
        catalogue_loader: Arc<dyn AttestedCatalogueDocumentLoaderPort>,
    ) -> CoverageVerifyInteractor {
        let plan_reader = plan_reader_matching_contract(&contract);
        CoverageVerifyInteractor::new(
            Arc::new(ConstContractReader(contract)),
            Arc::new(LayerAwareSignalReader(signal_docs)),
            plan_reader,
            catalogue_loader,
            layer_bindings(),
            workspace_root(),
            items_dir(),
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
            catalogue_reader(),
            layer_bindings(),
            workspace_root(),
            items_dir(),
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
            catalogue_reader(),
            layer_bindings(),
            workspace_root(),
            items_dir(),
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
        let mut expected = vec![CoverageViolation::OrphanEntry(ContractedEntryRef::new(
            layer("domain"),
            entry_key("Foo"),
        ))];
        expected.extend(missing_signal_documents(&[
            "usecase",
            "infrastructure",
            "cli_driver",
            "cli",
            "cli_composition",
        ]));
        assert_coverage_violations(outcome, expected);
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
        let mut expected = vec![CoverageViolation::InvalidEntryRef(
            ContractedEntryRef::new(layer("domain"), entry_key("Missing")),
            domain::FreeText::new("entry_key 'Missing' not found in domain-type-signals.json"),
        )];
        expected.extend(missing_signal_documents(&[
            "usecase",
            "infrastructure",
            "cli_driver",
            "cli",
            "cli_composition",
        ]));
        assert_coverage_violations(outcome, expected);
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
        assert_coverage_violations(
            outcome,
            missing_signal_documents(&[
                "domain",
                "usecase",
                "infrastructure",
                "cli_driver",
                "cli",
                "cli_composition",
            ]),
        );
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
        assert_coverage_violations(
            outcome,
            missing_signal_documents(&[
                "domain",
                "infrastructure",
                "cli_driver",
                "cli",
                "cli_composition",
            ]),
        );
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
        assert_coverage_violations(
            outcome,
            vec![CoverageViolation::InvalidEntryRef(
                ContractedEntryRef::new(layer("doman"), entry_key("Foo")),
                domain::FreeText::new("layer 'doman' is not a canonical TDDD layer"),
            )],
        );
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
        assert_coverage_violations(
            outcome,
            vec![CoverageViolation::OrphanEntry(ContractedEntryRef::new(
                layer("domain"),
                entry_key("ImplOnlyType"),
            ))],
        );
    }

    #[test]
    fn coverage_ambiguous_contract_key_fails_closed_for_same_named_rows() {
        let mut signal_docs = std::collections::HashMap::new();
        signal_docs.insert(
            "domain".to_owned(),
            make_signals(vec![
                catalogue_signal("Shared", CatalogueItemNamespace::Type, ConfidenceSignal::Blue),
                catalogue_signal("Shared", CatalogueItemNamespace::Trait, ConfidenceSignal::Blue),
            ]),
        );
        for layer_name in &["usecase", "infrastructure", "cli_driver", "cli", "cli_composition"] {
            signal_docs.insert((*layer_name).to_owned(), make_signals(vec![]));
        }

        let svc = coverage_interactor_with_catalogue(
            Ok(make_contract(
                "my-track",
                vec![(
                    task_id("T001"),
                    vec![ContractedEntryRef::new(layer("domain"), entry_key("Shared"))],
                )],
            )),
            signal_docs,
            ambiguous_catalogue_reader(),
        );

        let outcome = svc.verify_coverage(coverage_cmd("my-track")).unwrap();
        assert_coverage_violations(
            outcome,
            vec![
                CoverageViolation::OrphanEntry(ContractedEntryRef::new(
                    layer("domain"),
                    entry_key("Shared"),
                )),
                CoverageViolation::OrphanEntry(ContractedEntryRef::new(
                    layer("domain"),
                    entry_key("Shared"),
                )),
                CoverageViolation::InvalidEntryRef(
                    ContractedEntryRef::new(layer("domain"), entry_key("Shared")),
                    domain::FreeText::new(
                        "entry_key 'Shared' has no unique catalogue namespace in domain: \
                         could not classify TypeRef path at `Shared`",
                    ),
                ),
            ],
        );
    }

    #[test]
    fn coverage_duplicate_signal_identity_fails_closed() {
        for (first, second) in [
            (ConfidenceSignal::Blue, ConfidenceSignal::Yellow),
            (ConfidenceSignal::Yellow, ConfidenceSignal::Blue),
        ] {
            let mut signal_docs = std::collections::HashMap::new();
            signal_docs.insert(
                "domain".to_owned(),
                make_signals(vec![
                    catalogue_signal("Foo", CatalogueItemNamespace::Type, first),
                    catalogue_signal("Foo", CatalogueItemNamespace::Type, second),
                ]),
            );
            for layer_name in &["usecase", "infrastructure", "cli_driver", "cli", "cli_composition"]
            {
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

            let err = svc.verify_coverage(coverage_cmd("my-track")).unwrap_err();
            match err {
                PreReviewGateError::SignalReadFailed { layer, message } => {
                    assert_eq!(layer.as_ref(), "domain");
                    assert!(message.as_str().contains("duplicate signal identity"));
                }
                other => panic!("expected duplicate identity to fail closed, got {other}"),
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
        assert_coverage_violations(
            outcome,
            vec![CoverageViolation::InvalidTaskRef(
                task_id("T999"),
                vec![ContractedEntryRef::new(layer("domain"), entry_key("Foo"))],
            )],
        );
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
        assert_coverage_violations(
            outcome,
            vec![
                CoverageViolation::InvalidTaskRef(
                    task_id("T100"),
                    vec![ContractedEntryRef::new(layer("domain"), entry_key("Foo"))],
                ),
                CoverageViolation::InvalidTaskRef(
                    task_id("T200"),
                    vec![ContractedEntryRef::new(layer("domain"), entry_key("Bar"))],
                ),
            ],
        );
    }

    // ── D7 task-status filtering tests ───────────────────────────────────────────

    /// Impl-plan reader that returns a fixed, caller-supplied task status map.
    struct FixedImplPlanReader(std::collections::HashMap<TaskId, TaskStatusKind>);

    impl ImplPlanReaderPort for FixedImplPlanReader {
        fn read_task_statuses(
            &self,
            _track_id: &TrackId,
        ) -> Result<std::collections::HashMap<TaskId, TaskStatusKind>, ImplPlanReadError> {
            Ok(self.0.clone())
        }
    }

    /// Impl-plan reader that always fails with `ImplPlanReadFailed`.
    struct FailingImplPlanReader;

    impl ImplPlanReaderPort for FailingImplPlanReader {
        fn read_task_statuses(
            &self,
            _track_id: &TrackId,
        ) -> Result<std::collections::HashMap<TaskId, TaskStatusKind>, ImplPlanReadError> {
            Err(ImplPlanReadError::ReadFailed {
                message: domain::FreeText::new("test: impl-plan.json read failed"),
            })
        }
    }

    fn red_signal(name: &str) -> TypeSignal {
        TypeSignal::new(
            ThreeWaySignalIdentity::CatalogueItem {
                item_name: FreeText::new(name),
                namespace: CatalogueItemNamespace::Type,
            },
            "struct".to_owned(),
            ConfidenceSignal::Red,
            false,
            vec![],
            vec![],
            vec![],
        )
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
            catalogue_reader(),
            layer_bindings(),
            workspace_root(),
            items_dir(),
        );

        let outcome = svc.check(cmd("my-track", "domain")).unwrap();
        assert_liveness_violations(
            outcome,
            vec![PreReviewGateViolation::NonBlueSignal(
                ContractedEntryRef::new(layer("domain"), entry_key("Foo")),
                ConfidenceSignal::Red,
            )],
        );
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
            catalogue_reader(),
            layer_bindings(),
            workspace_root(),
            items_dir(),
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
            catalogue_reader(),
            layer_bindings(),
            workspace_root(),
            items_dir(),
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
            catalogue_reader(),
            layer_bindings(),
            workspace_root(),
            items_dir(),
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
            catalogue_reader(),
            layer_bindings(),
            workspace_root(),
            items_dir(),
        );

        let err = svc.check(cmd("my-track", "domain")).unwrap_err();
        match err {
            PreReviewGateError::ImplPlanReadFailed { message } => {
                assert!(
                    message.as_str().contains("read failed"),
                    "expected read-failed diagnostic, got: {message}"
                );
            }
            other => panic!("expected ImplPlanReadFailed, got {other}"),
        }
    }

    #[test]
    fn test_pre_review_gate_error_conversions_preserve_free_text_and_rendering() {
        let task_contract_error = PreReviewGateError::from(TaskContractReadError::ReadFailed {
            message: domain::FreeText::new("contract read failure"),
        });
        assert_eq!(
            task_contract_error.to_string(),
            "failed to read task-contract.json: contract read failure"
        );
        assert!(matches!(
            task_contract_error,
            PreReviewGateError::TaskContractReadFailed { message }
                if message.as_str() == "contract read failure"
        ));

        let catalogue_error = PreReviewGateError::CatalogueReadFailed {
            layer: LayerId::try_new("domain".to_owned()).expect("valid test layer"),
            message: domain::FreeText::new("catalogue read failure"),
        };
        assert_eq!(
            catalogue_error.to_string(),
            "failed to read catalogue for layer 'domain': catalogue read failure"
        );

        let freshness_error = PreReviewGateError::CatalogueFreshnessMismatch {
            layer: LayerId::try_new("domain".to_owned()).expect("valid test layer"),
            message: domain::FreeText::new("catalogue freshness failure"),
        };
        assert_eq!(
            freshness_error.to_string(),
            "catalogue freshness mismatch for layer 'domain': catalogue freshness failure"
        );

        let signal_error = PreReviewGateError::from(ImplCatalogSignalReadError::ReadFailed {
            layer: LayerId::try_new("domain".to_owned()).expect("valid test layer"),
            message: domain::FreeText::new("signal read failure"),
        });
        assert_eq!(
            signal_error.to_string(),
            "failed to read type-signals for layer 'domain': signal read failure"
        );
        assert!(matches!(
            signal_error,
            PreReviewGateError::SignalReadFailed { layer, message }
                if layer.as_ref() == "domain" && message.as_str() == "signal read failure"
        ));

        let impl_plan_error = PreReviewGateError::from(ImplPlanReadError::ReadFailed {
            message: domain::FreeText::new("impl-plan read failure"),
        });
        assert_eq!(
            impl_plan_error.to_string(),
            "failed to read impl-plan.json: impl-plan read failure"
        );
        assert!(matches!(
            impl_plan_error,
            PreReviewGateError::ImplPlanReadFailed { message }
                if message.as_str() == "impl-plan read failure"
        ));
    }
}
