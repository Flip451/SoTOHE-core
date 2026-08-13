//! Domain model for `task-contract.json`.
//!
//! `task-contract.json` maps each `TaskId` in the active track to the catalogue
//! entries (layer + entry_key pairs) that the task is responsible for implementing.
//! The pre-review gate (usecase) reads this document to verify that all contracted
//! catalogue entries have blue impl_catalog signals before allowing review to proceed.

use std::collections::BTreeMap;

use crate::ids::{TaskId, TrackId};
use crate::tddd::catalogue_v2::NonEmptyVec;
use crate::tddd::layer_id::LayerId;
use crate::tddd::semantic_verify::CatalogueEntryKey;
use crate::{FreeText, ValidationError};

// ---------------------------------------------------------------------------
// ContractedEntryRef
// ---------------------------------------------------------------------------

/// A `(layer, entry_key)` pair identifying one catalogue entry contracted to a task.
///
/// Distinct from `domain::tddd::semantic_verify::CatalogueEntryRef` (which holds
/// `file_path + section_key + entry_key` for spec-adr verification). This type
/// carries only the layer identity and catalogue entry key needed for the
/// pre-review gate check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractedEntryRef {
    /// The TDDD layer this entry belongs to (e.g. `"domain"`, `"usecase"`).
    pub(crate) layer: LayerId,
    /// The catalogue entry key (type name, trait name, or function path).
    pub(crate) entry_key: CatalogueEntryKey,
}

impl ContractedEntryRef {
    /// Construct a `ContractedEntryRef` from a layer id and catalogue entry key.
    #[must_use]
    pub fn new(layer: LayerId, entry_key: CatalogueEntryKey) -> Self {
        Self { layer, entry_key }
    }

    /// Returns a reference to the layer id.
    #[must_use]
    pub fn layer(&self) -> &LayerId {
        &self.layer
    }

    /// Returns a reference to the catalogue entry key.
    #[must_use]
    pub fn entry_key(&self) -> &CatalogueEntryKey {
        &self.entry_key
    }
}

// ---------------------------------------------------------------------------
// TaskContractDocument
// ---------------------------------------------------------------------------

/// Domain model for `task-contract.json`.
///
/// Maps each `TaskId` in the active track to the catalogue entries
/// (`layer + entry_key` pairs) that the task is responsible for implementing.
/// An empty `entries` map is accepted; the resulting contract has nothing to
/// verify and behaves equivalently to a missing contract file (short-circuit Passed).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskContractDocument {
    track_id: TrackId,
    entries: BTreeMap<TaskId, Vec<ContractedEntryRef>>,
}

impl TaskContractDocument {
    /// Construct a `TaskContractDocument`.
    ///
    /// An empty `entries` map or tasks with empty contracted-entry lists are
    /// accepted. Such a contract has nothing to verify and is treated the same
    /// as a missing contract file by the pre-review gate (short-circuit Passed).
    ///
    /// # Errors
    ///
    /// None — no validation error is produced from the `entries` argument.
    /// The `Result` return type is retained so future constraints can be added
    /// without a signature change.
    pub fn new(
        track_id: TrackId,
        entries: BTreeMap<TaskId, Vec<ContractedEntryRef>>,
    ) -> Result<Self, ValidationError> {
        Ok(Self { track_id, entries })
    }

    /// Returns the schema version for `task-contract.json` serialization.
    #[must_use]
    pub fn schema_version(&self) -> u32 {
        1
    }

    /// Returns a reference to the track ID this contract belongs to.
    #[must_use]
    pub fn track_id(&self) -> &TrackId {
        &self.track_id
    }

    /// Returns the task-to-entries mapping.
    #[must_use]
    pub fn entries(&self) -> &BTreeMap<TaskId, Vec<ContractedEntryRef>> {
        &self.entries
    }
}

// ---------------------------------------------------------------------------
// PreReviewGateViolation
// ---------------------------------------------------------------------------

/// A single liveness-gate violation found during `bin/sotp task-contract check`.
///
/// Narrowed to check-specific violations after D5 split:
/// - `MissingTaskContract`: `task-contract.json` is absent, gate cannot proceed.
/// - `NonBlueSignal`: an attributed entry for a current/done task has a
///   non-blue `impl_catalog` signal.
///
/// Attribution violations (`OrphanEntry`, `InvalidEntryRef`) moved to
/// [`CoverageViolation`] used by the `coverage` subcommand.
/// Modelled as a finding record (`ValueObject`), not an error type: it is data
/// carried inside [`PreReviewGateOutcome::Blocked`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreReviewGateViolation {
    /// `task-contract.json` is absent for the given track.
    MissingTaskContract,

    /// A contracted entry exists in the `TypeSignalsDocument` but its
    /// `impl_catalog` signal is not `Blue` for a current/done task.
    NonBlueSignal(ContractedEntryRef, crate::ConfidenceSignal),
}

// ---------------------------------------------------------------------------
// CoverageViolation
// ---------------------------------------------------------------------------

/// A single attribution-completeness violation found during
/// `bin/sotp task-contract coverage`.
///
/// - `MissingTaskContract`: `task-contract.json` is absent, coverage check
///   cannot proceed (fail-closed).
/// - `OrphanEntry`: a catalogue entry exists but is not attributed to any task
///   in `task-contract.json` (attribution completeness failure).
/// - `InvalidEntryRef`: an entry attributed in `task-contract.json` does not
///   exist in the current catalogue (referential integrity failure).
///   `reason` is an opaque diagnostic string (R9 exception: error message).
/// - `MissingSignalDocument`: the per-layer `<layer>-type-signals.json` document
///   is absent for a canonical TDDD layer. Emitted regardless of whether any
///   entries are attributed to that layer in `task-contract.json`, so that
///   coverage fails closed when a signal document cannot be found.
/// - `InvalidTaskRef`: a task key in `task-contract.json` does not exist in
///   `impl-plan.json` (referential integrity failure on the attribution map's
///   task dimension). Emitted when `task-contract.json` has been rebased over
///   an `impl-plan.json` that removed or renamed the task without updating
///   the contract attributions, so coverage fails closed instead of letting
///   stale entries silently pass.
///
/// These violations are data inside [`CoverageVerifyOutcome::Blocked`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoverageViolation {
    /// `task-contract.json` is absent for the given track.
    MissingTaskContract,

    /// A catalogue entry exists but has no task attribution in
    /// `task-contract.json`.
    OrphanEntry(ContractedEntryRef),

    /// A contracted entry's `entry_key` does not exist in the
    /// `TypeSignalsDocument` for the reviewed layer.
    InvalidEntryRef(ContractedEntryRef, FreeText),

    /// The per-layer `<layer>-type-signals.json` document is absent for this
    /// canonical TDDD layer. Emitted by `CoverageVerifyInteractor` whenever
    /// `read_optional_signals` returns `None`, regardless of whether any
    /// entries are attributed to that layer, so that the coverage gate fails
    /// closed when a signal document cannot be located.
    MissingSignalDocument(LayerId),

    /// A task key in `task-contract.json` does not exist in the current
    /// `impl-plan.json` task list. Emitted when the contract attributes
    /// catalogue entries to a task that has been renamed or removed from
    /// `impl-plan.json` (referential integrity failure on the task dimension).
    InvalidTaskRef(TaskId, Vec<ContractedEntryRef>),
}

// ---------------------------------------------------------------------------
// PreReviewGateOutcome
// ---------------------------------------------------------------------------

/// Outcome of the `bin/sotp task-contract check` liveness gate.
///
/// `Passed` is a binary OK signal — all current/done attributed entries have
/// blue `impl_catalog` signals, no further data attached. `Blocked` carries
/// a non-empty list of liveness violations (`MissingTaskContract`,
/// `NonBlueSignal`). The public tuple payload can be destructured by adapters
/// for read-only diagnostic access.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreReviewGateOutcome {
    /// All current/done attributed entries have blue impl_catalog signals.
    Passed,
    /// One or more liveness gate violations were found.
    Blocked(NonEmptyVec<PreReviewGateViolation>),
}

impl PreReviewGateOutcome {
    /// Constructs a blocked outcome with at least one liveness violation.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::EmptyString`] when `violations` is empty.
    pub fn blocked(violations: Vec<PreReviewGateViolation>) -> Result<Self, ValidationError> {
        NonEmptyVec::try_new(violations)
            .map(Self::Blocked)
            .map_err(|_| ValidationError::EmptyString)
    }
}

// ---------------------------------------------------------------------------
// CoverageVerifyOutcome
// ---------------------------------------------------------------------------

/// Outcome of the `bin/sotp task-contract coverage` attribution-completeness check.
///
/// `Passed` means all catalogue entries are attributed to at least one task,
/// and all attributed entries exist in the catalogue. `Blocked` carries the
/// non-empty list of attribution violations (`MissingTaskContract`,
/// `OrphanEntry`, `InvalidEntryRef`). The public tuple payload can be
/// destructured by adapters for read-only diagnostic access.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoverageVerifyOutcome {
    /// All catalogue entries are attributed and referentially consistent.
    Passed,
    /// One or more attribution-completeness violations were found.
    Blocked(NonEmptyVec<CoverageViolation>),
}

impl CoverageVerifyOutcome {
    /// Constructs a blocked outcome with at least one attribution-completeness violation.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::EmptyString`] when `violations` is empty.
    pub fn blocked(violations: Vec<CoverageViolation>) -> Result<Self, ValidationError> {
        NonEmptyVec::try_new(violations)
            .map(Self::Blocked)
            .map_err(|_| ValidationError::EmptyString)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

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

    fn sample_entry() -> ContractedEntryRef {
        ContractedEntryRef::new(layer("domain"), entry_key("MyType"))
    }

    #[test]
    fn test_contracted_entry_ref_new_stores_fields() {
        let e = ContractedEntryRef::new(layer("domain"), entry_key("Foo"));
        assert_eq!(e.layer().as_ref(), "domain");
        assert_eq!(e.entry_key().as_str(), "Foo");
    }

    #[test]
    fn test_contracted_entry_ref_clone_preserves_value() {
        let a = sample_entry();
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn test_task_contract_document_empty_entries_is_accepted() {
        let result = TaskContractDocument::new(track_id("my-track"), BTreeMap::new());
        assert!(result.is_ok());
    }

    #[test]
    fn test_task_contract_document_empty_task_entry_list_is_accepted() {
        let mut entries = BTreeMap::new();
        entries.insert(task_id("T001"), Vec::new());
        let result = TaskContractDocument::new(track_id("my-track"), entries);
        assert!(result.is_ok());
    }

    #[test]
    fn test_task_contract_document_valid_entries_are_preserved() {
        let mut entries = BTreeMap::new();
        entries.insert(task_id("T001"), vec![sample_entry()]);
        let doc = TaskContractDocument::new(track_id("my-track"), entries).unwrap();
        assert_eq!(doc.track_id().as_ref(), "my-track");
        assert_eq!(doc.schema_version(), 1);
        assert_eq!(doc.entries().len(), 1);
    }

    #[test]
    fn test_task_contract_document_clone_preserves_value() {
        let mut entries = BTreeMap::new();
        entries.insert(task_id("T001"), vec![sample_entry()]);
        let a = TaskContractDocument::new(track_id("my-track"), entries).unwrap();
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn test_pre_review_gate_violation_non_blue_signal_preserves_arguments() {
        let v = PreReviewGateViolation::MissingTaskContract;
        let c = v.clone();
        assert_eq!(v, c);

        let v2 =
            PreReviewGateViolation::NonBlueSignal(sample_entry(), crate::ConfidenceSignal::Yellow);
        let c2 = v2.clone();
        assert_eq!(v2, c2);

        match v2 {
            PreReviewGateViolation::NonBlueSignal(entry, signal) => {
                assert_eq!(entry.entry_key().as_str(), "MyType");
                assert_eq!(signal, crate::ConfidenceSignal::Yellow);
            }
            PreReviewGateViolation::MissingTaskContract => panic!("expected NonBlueSignal"),
        }
    }

    #[test]
    fn test_coverage_violation_tuple_variants_preserve_arguments() {
        let v = CoverageViolation::MissingTaskContract;
        assert_eq!(v.clone(), v);

        let v2 = CoverageViolation::OrphanEntry(sample_entry());
        assert_eq!(v2.clone(), v2);

        let v3 = CoverageViolation::InvalidEntryRef(sample_entry(), FreeText::new("not found"));
        assert_eq!(v3.clone(), v3);

        let v4 = CoverageViolation::MissingSignalDocument(layer("domain"));
        assert_eq!(v4.clone(), v4);

        let v5 = CoverageViolation::InvalidTaskRef(task_id("T001"), vec![sample_entry()]);
        assert_eq!(v5.clone(), v5);

        match v5 {
            CoverageViolation::InvalidTaskRef(task_id, entries) => {
                assert_eq!(task_id.as_ref(), "T001");
                assert_eq!(entries.len(), 1);
            }
            _ => panic!("expected InvalidTaskRef"),
        }
    }

    #[test]
    fn test_coverage_verify_outcome_passed_is_preserved() {
        let outcome = CoverageVerifyOutcome::Passed;
        assert!(matches!(outcome, CoverageVerifyOutcome::Passed));
    }

    #[test]
    fn test_coverage_verify_outcome_non_empty_violations_is_blocked() {
        let outcome =
            CoverageVerifyOutcome::blocked(vec![CoverageViolation::MissingTaskContract]).unwrap();
        assert!(
            matches!(outcome, CoverageVerifyOutcome::Blocked(violations) if violations.as_slice().len() == 1)
        );
    }

    #[test]
    fn test_coverage_verify_outcome_empty_violations_is_rejected() {
        let result = CoverageVerifyOutcome::blocked(Vec::new());
        assert!(result.is_err());
    }

    #[test]
    fn test_pre_review_gate_outcome_passed_is_preserved() {
        let outcome = PreReviewGateOutcome::Passed;
        assert!(matches!(outcome, PreReviewGateOutcome::Passed));
    }

    #[test]
    fn test_pre_review_gate_outcome_non_empty_violations_is_blocked() {
        let outcome =
            PreReviewGateOutcome::blocked(vec![PreReviewGateViolation::MissingTaskContract])
                .unwrap();
        assert!(
            matches!(outcome, PreReviewGateOutcome::Blocked(violations) if violations.as_slice().len() == 1)
        );
    }

    #[test]
    fn test_pre_review_gate_outcome_empty_violations_is_rejected() {
        let result = PreReviewGateOutcome::blocked(Vec::new());
        assert!(result.is_err());
    }
}
