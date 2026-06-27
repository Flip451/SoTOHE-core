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

/// A single gate violation found during pre-review conformance checking.
///
/// Modelled as a finding record (`ValueObject`), not an error type: it is data
/// carried inside [`PreReviewGateOutcome::Blocked`].
/// `InvalidEntryRef.reason` is an opaque diagnostic string with no domain
/// invariant (R9 exception: error message string).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreReviewGateViolation {
    /// `task-contract.json` is absent for the given track.
    MissingTaskContract,

    /// A scope-relevant catalogue entry has no task attribution in
    /// `task-contract.json` for the reviewed layer.
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

    /// A contracted entry exists in the `TypeSignalsDocument` but its
    /// `impl_catalog` signal is not `Blue`.
    NonBlueSignal {
        /// The contracted entry whose signal is not blue.
        entry: ContractedEntryRef,
        /// The actual confidence signal recorded in the type-signals document.
        signal: crate::ConfidenceSignal,
    },
}

// ---------------------------------------------------------------------------
// PreReviewGateOutcome
// ---------------------------------------------------------------------------

/// Outcome of the pre-review gate check.
///
/// `Passed` carries a human-readable conformance summary to be appended to the
/// reviewer briefing (`IN-06`, `GO-04`). `Blocked` carries all violations found.
/// `conformance_summary` is an opaque prose string with no cross-domain identity
/// (R9: content of the summary is not a domain concept).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreReviewGateOutcome {
    /// All contracted entries have blue impl_catalog signals.
    Passed {
        /// Human-readable conformance summary to be prepended to the reviewer briefing.
        conformance_summary: String,
    },
    /// One or more gate violations were found.
    ///
    /// Use [`PreReviewGateOutcome::blocked`] to construct this variant so the
    /// non-empty invariant is checked at the crate boundary.
    #[non_exhaustive]
    Blocked {
        /// All violations collected during the gate check.
        violations: Vec<PreReviewGateViolation>,
    },
}

impl PreReviewGateOutcome {
    /// Constructs a blocked outcome with at least one violation.
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

        let v2 = PreReviewGateViolation::OrphanEntry { entry: sample_entry() };
        let c2 = v2.clone();
        assert_eq!(v2, c2);
    }

    #[test]
    fn pre_review_gate_outcome_passed() {
        let outcome = PreReviewGateOutcome::Passed { conformance_summary: "all blue".to_owned() };
        assert!(matches!(outcome, PreReviewGateOutcome::Passed { .. }));
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
