//! Domain model for `task-contract.json`.
//!
//! `task-contract.json` maps each `TaskId` in the active track to the catalogue
//! entries (layer + entry_key pairs) that the task is responsible for implementing.
//! The pre-review gate (usecase) reads this document to verify that all contracted
//! catalogue entries have blue impl_catalog signals before allowing review to proceed.

use std::collections::BTreeMap;

use crate::ValidationError;
use crate::ids::{TaskId, TrackId};
use crate::tddd::layer_id::LayerId;
use crate::tddd::semantic_verify::CatalogueEntryKey;

/// Schema version constant for `task-contract.json` serialization.
pub const TASK_CONTRACT_SCHEMA_VERSION: u32 = 1;

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
/// Validated at construction: `track_id` must be non-empty (enforced by `TrackId`),
/// `entries` map must be non-empty, and every task must carry at least one
/// contracted entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskContractDocument {
    track_id: TrackId,
    entries: BTreeMap<TaskId, Vec<ContractedEntryRef>>,
}

impl TaskContractDocument {
    /// Construct a `TaskContractDocument`.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::EmptyString`] when `entries` is empty or any
    /// task has an empty contracted-entry list, since a contract with no
    /// attributed entries is not a meaningful gating document.
    pub fn new(
        track_id: TrackId,
        entries: BTreeMap<TaskId, Vec<ContractedEntryRef>>,
    ) -> Result<Self, ValidationError> {
        if entries.is_empty() || entries.values().any(Vec::is_empty) {
            return Err(ValidationError::EmptyString);
        }
        Ok(Self { track_id, entries })
    }

    /// Returns the schema version constant for `task-contract.json` serialization.
    #[must_use]
    pub fn schema_version(&self) -> u32 {
        TASK_CONTRACT_SCHEMA_VERSION
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
    NonBlueSignal {
        /// The contracted entry whose signal is not blue.
        entry: ContractedEntryRef,
        /// The actual confidence signal recorded in the type-signals document.
        signal: crate::ConfidenceSignal,
    },
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
///
/// These violations are data inside [`CoverageVerifyOutcome::Blocked`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoverageViolation {
    /// `task-contract.json` is absent for the given track.
    MissingTaskContract,

    /// A catalogue entry exists but has no task attribution in
    /// `task-contract.json`.
    OrphanEntry {
        /// The catalogue entry that has no corresponding task attribution.
        entry: ContractedEntryRef,
    },

    /// A contracted entry's `entry_key` does not exist in the
    /// `TypeSignalsDocument` for the reviewed layer.
    InvalidEntryRef {
        /// The contracted entry that cannot be found in the signal document.
        entry: ContractedEntryRef,
        /// Opaque diagnostic message explaining why the reference is invalid.
        reason: String,
    },
}

// ---------------------------------------------------------------------------
// PreReviewGateOutcome
// ---------------------------------------------------------------------------

/// Outcome of the `bin/sotp task-contract check` liveness gate.
///
/// `Passed` is a binary OK signal — all current/done attributed entries have
/// blue `impl_catalog` signals, no further data attached. `Blocked` carries
/// the list of liveness violations (`MissingTaskContract`, `NonBlueSignal`).
/// The `Blocked` variant is `#[non_exhaustive]`. Use
/// [`PreReviewGateOutcome::blocked`] to construct a `Blocked` outcome so the
/// non-empty invariant is checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreReviewGateOutcome {
    /// All current/done attributed entries have blue impl_catalog signals.
    Passed,
    /// One or more liveness gate violations were found.
    ///
    /// Use [`PreReviewGateOutcome::blocked`] to construct this variant so the
    /// non-empty invariant is checked at the crate boundary.
    #[non_exhaustive]
    Blocked {
        /// All liveness violations collected during the gate check.
        violations: Vec<PreReviewGateViolation>,
    },
}

impl PreReviewGateOutcome {
    /// Constructs a blocked outcome with at least one liveness violation.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::EmptyString`] when `violations` is empty.
    pub fn blocked(violations: Vec<PreReviewGateViolation>) -> Result<Self, ValidationError> {
        if violations.is_empty() {
            return Err(ValidationError::EmptyString);
        }
        Ok(Self::Blocked { violations })
    }
}

// ---------------------------------------------------------------------------
// CoverageVerifyOutcome
// ---------------------------------------------------------------------------

/// Outcome of the `bin/sotp task-contract coverage` attribution-completeness check.
///
/// `Passed` means all catalogue entries are attributed to at least one task,
/// and all attributed entries exist in the catalogue. `Blocked` carries the
/// list of attribution violations (`MissingTaskContract`, `OrphanEntry`,
/// `InvalidEntryRef`). The `Blocked` variant is `#[non_exhaustive]`. Use
/// [`CoverageVerifyOutcome::blocked`] to construct a `Blocked` outcome so the
/// non-empty invariant is checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoverageVerifyOutcome {
    /// All catalogue entries are attributed and referentially consistent.
    Passed,
    /// One or more attribution-completeness violations were found.
    ///
    /// Use [`CoverageVerifyOutcome::blocked`] to construct this variant so the
    /// non-empty invariant is checked at the crate boundary.
    #[non_exhaustive]
    Blocked {
        /// All attribution violations collected during the coverage check.
        violations: Vec<CoverageViolation>,
    },
}

impl CoverageVerifyOutcome {
    /// Constructs a blocked outcome with at least one attribution-completeness violation.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::EmptyString`] when `violations` is empty.
    pub fn blocked(violations: Vec<CoverageViolation>) -> Result<Self, ValidationError> {
        if violations.is_empty() {
            return Err(ValidationError::EmptyString);
        }
        Ok(Self::Blocked { violations })
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
    fn contracted_entry_ref_new_stores_fields() {
        let e = ContractedEntryRef::new(layer("domain"), entry_key("Foo"));
        assert_eq!(e.layer().as_ref(), "domain");
        assert_eq!(e.entry_key().as_str(), "Foo");
    }

    #[test]
    fn contracted_entry_ref_clone_eq() {
        let a = sample_entry();
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn task_contract_document_rejects_empty_entries() {
        let result = TaskContractDocument::new(track_id("my-track"), BTreeMap::new());
        assert!(result.is_err());
    }

    #[test]
    fn task_contract_document_rejects_empty_task_entry_list() {
        let mut entries = BTreeMap::new();
        entries.insert(task_id("T001"), Vec::new());
        let result = TaskContractDocument::new(track_id("my-track"), entries);
        assert!(result.is_err());
    }

    #[test]
    fn task_contract_document_accepts_non_empty_entries() {
        let mut entries = BTreeMap::new();
        entries.insert(task_id("T001"), vec![sample_entry()]);
        let doc = TaskContractDocument::new(track_id("my-track"), entries).unwrap();
        assert_eq!(doc.track_id().as_ref(), "my-track");
        assert_eq!(doc.schema_version(), TASK_CONTRACT_SCHEMA_VERSION);
        assert_eq!(doc.entries().len(), 1);
    }

    #[test]
    fn task_contract_document_clone_eq() {
        let mut entries = BTreeMap::new();
        entries.insert(task_id("T001"), vec![sample_entry()]);
        let a = TaskContractDocument::new(track_id("my-track"), entries).unwrap();
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn pre_review_gate_violation_debug_and_clone() {
        let v = PreReviewGateViolation::MissingTaskContract;
        let c = v.clone();
        assert_eq!(v, c);

        let v2 = PreReviewGateViolation::NonBlueSignal {
            entry: sample_entry(),
            signal: crate::ConfidenceSignal::Yellow,
        };
        let c2 = v2.clone();
        assert_eq!(v2, c2);
    }

    #[test]
    fn coverage_violation_debug_and_clone() {
        let v = CoverageViolation::MissingTaskContract;
        assert_eq!(v.clone(), v);

        let v2 = CoverageViolation::OrphanEntry { entry: sample_entry() };
        assert_eq!(v2.clone(), v2);

        let v3 = CoverageViolation::InvalidEntryRef {
            entry: sample_entry(),
            reason: "not found".to_owned(),
        };
        assert_eq!(v3.clone(), v3);
    }

    #[test]
    fn coverage_verify_outcome_passed() {
        let outcome = CoverageVerifyOutcome::Passed;
        assert!(matches!(outcome, CoverageVerifyOutcome::Passed));
    }

    #[test]
    fn coverage_verify_outcome_blocked() {
        let outcome =
            CoverageVerifyOutcome::blocked(vec![CoverageViolation::MissingTaskContract]).unwrap();
        assert!(matches!(outcome, CoverageVerifyOutcome::Blocked { .. }));
    }

    #[test]
    fn coverage_verify_outcome_rejects_empty_blocked_violations() {
        let result = CoverageVerifyOutcome::blocked(Vec::new());
        assert!(result.is_err());
    }

    #[test]
    fn pre_review_gate_outcome_passed() {
        let outcome = PreReviewGateOutcome::Passed;
        assert!(matches!(outcome, PreReviewGateOutcome::Passed));
    }

    #[test]
    fn pre_review_gate_outcome_blocked() {
        let outcome =
            PreReviewGateOutcome::blocked(vec![PreReviewGateViolation::MissingTaskContract])
                .unwrap();
        assert!(matches!(outcome, PreReviewGateOutcome::Blocked { .. }));
    }

    #[test]
    fn pre_review_gate_outcome_rejects_empty_blocked_violations() {
        let result = PreReviewGateOutcome::blocked(Vec::new());
        assert!(result.is_err());
    }
}
