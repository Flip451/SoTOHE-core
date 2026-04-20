//! Domain aggregate root for `task-coverage.json` (Phase 3 SSoT).
//!
//! `TaskCoverageDocument` maps spec element IDs (from the four requirement
//! sections: in_scope / out_of_scope / constraints / acceptance_criteria) to
//! the implementing task IDs. Introduced by ADR 2026-04-19-1242 §D1.4.

use std::collections::BTreeMap;

use crate::{DomainError, SpecElementId, TaskId, ValidationError};

/// The current schema version for `task-coverage.json`.
pub const TASK_COVERAGE_SCHEMA_VERSION: u32 = 1;

/// Aggregate root for `track/items/<id>/task-coverage.json`.
///
/// Holds per-section mappings from `SpecElementId` to `Vec<TaskId>` for the
/// four spec requirement sections. `BTreeMap` is used so that JSON output is
/// deterministic (keys are sorted).
///
/// Invariants enforced on construction:
/// - No duplicate `SpecElementId` across the four sections (each spec element
///   appears in at most one section).
///
/// Empty `Vec<TaskId>` values are permitted (some spec elements may have no
/// covering tasks yet at the time of document construction).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskCoverageDocument {
    schema_version: u32,
    in_scope: BTreeMap<SpecElementId, Vec<TaskId>>,
    out_of_scope: BTreeMap<SpecElementId, Vec<TaskId>>,
    constraints: BTreeMap<SpecElementId, Vec<TaskId>>,
    acceptance_criteria: BTreeMap<SpecElementId, Vec<TaskId>>,
}

impl TaskCoverageDocument {
    /// Creates a new `TaskCoverageDocument`, validating that no `SpecElementId`
    /// appears in more than one section.
    ///
    /// # Errors
    ///
    /// Returns `DomainError::Validation(ValidationError::DuplicateElementId(_))` if
    /// the same `SpecElementId` appears in more than one of the four sections.
    pub fn new(
        in_scope: BTreeMap<SpecElementId, Vec<TaskId>>,
        out_of_scope: BTreeMap<SpecElementId, Vec<TaskId>>,
        constraints: BTreeMap<SpecElementId, Vec<TaskId>>,
        acceptance_criteria: BTreeMap<SpecElementId, Vec<TaskId>>,
    ) -> Result<Self, DomainError> {
        // Validate: no duplicate SpecElementId across sections.
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for section_map in [&in_scope, &out_of_scope, &constraints, &acceptance_criteria] {
            for id in section_map.keys() {
                let id_str = id.as_ref().to_owned();
                if !seen.insert(id_str.clone()) {
                    return Err(ValidationError::DuplicateElementId(id_str).into());
                }
            }
        }

        Ok(Self {
            schema_version: TASK_COVERAGE_SCHEMA_VERSION,
            in_scope,
            out_of_scope,
            constraints,
            acceptance_criteria,
        })
    }

    /// Returns the schema version.
    #[must_use]
    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// Returns the in-scope coverage map.
    #[must_use]
    pub fn in_scope(&self) -> &BTreeMap<SpecElementId, Vec<TaskId>> {
        &self.in_scope
    }

    /// Returns the out-of-scope coverage map.
    #[must_use]
    pub fn out_of_scope(&self) -> &BTreeMap<SpecElementId, Vec<TaskId>> {
        &self.out_of_scope
    }

    /// Returns the constraints coverage map.
    #[must_use]
    pub fn constraints(&self) -> &BTreeMap<SpecElementId, Vec<TaskId>> {
        &self.constraints
    }

    /// Returns the acceptance-criteria coverage map.
    #[must_use]
    pub fn acceptance_criteria(&self) -> &BTreeMap<SpecElementId, Vec<TaskId>> {
        &self.acceptance_criteria
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::collections::BTreeMap;

    use crate::{
        DomainError, SpecElementId, TaskId, ValidationError, task_coverage::TaskCoverageDocument,
    };

    fn eid(s: &str) -> SpecElementId {
        SpecElementId::try_new(s).unwrap()
    }

    fn tid(s: &str) -> TaskId {
        TaskId::try_new(s).unwrap()
    }

    fn empty() -> BTreeMap<SpecElementId, Vec<TaskId>> {
        BTreeMap::new()
    }

    // --- happy path ---

    #[test]
    fn test_new_with_all_empty_sections_succeeds() {
        let doc = TaskCoverageDocument::new(empty(), empty(), empty(), empty()).unwrap();
        assert_eq!(doc.schema_version(), 1);
        assert!(doc.in_scope().is_empty());
        assert!(doc.out_of_scope().is_empty());
        assert!(doc.constraints().is_empty());
        assert!(doc.acceptance_criteria().is_empty());
    }

    #[test]
    fn test_new_with_non_overlapping_ids_succeeds() {
        let mut in_scope = empty();
        in_scope.insert(eid("IN-01"), vec![tid("T001")]);
        let mut ac = empty();
        ac.insert(eid("AC-01"), vec![tid("T002")]);

        let doc = TaskCoverageDocument::new(in_scope, empty(), empty(), ac).unwrap();
        assert_eq!(doc.in_scope().len(), 1);
        assert_eq!(doc.acceptance_criteria().len(), 1);
    }

    #[test]
    fn test_new_with_multiple_task_ids_per_element_succeeds() {
        let mut in_scope = empty();
        in_scope.insert(eid("IN-01"), vec![tid("T001"), tid("T002"), tid("T003")]);

        let doc = TaskCoverageDocument::new(in_scope, empty(), empty(), empty()).unwrap();
        assert_eq!(doc.in_scope()[&eid("IN-01")].len(), 3);
    }

    #[test]
    fn test_new_with_empty_task_list_per_element_succeeds() {
        let mut in_scope = empty();
        in_scope.insert(eid("IN-01"), vec![]);

        let doc = TaskCoverageDocument::new(in_scope, empty(), empty(), empty()).unwrap();
        assert!(doc.in_scope()[&eid("IN-01")].is_empty());
    }

    // --- validation: duplicate SpecElementId across sections ---

    #[test]
    fn test_new_with_duplicate_id_across_in_scope_and_out_of_scope_returns_error() {
        let mut in_scope = empty();
        in_scope.insert(eid("IN-01"), vec![]);
        let mut out_of_scope = empty();
        out_of_scope.insert(eid("IN-01"), vec![]); // duplicate

        let err = TaskCoverageDocument::new(in_scope, out_of_scope, empty(), empty()).unwrap_err();
        assert!(
            matches!(
                err,
                DomainError::Validation(ValidationError::DuplicateElementId(ref id)) if id == "IN-01"
            ),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn test_new_with_duplicate_id_across_in_scope_and_constraints_returns_error() {
        let mut in_scope = empty();
        in_scope.insert(eid("CO-01"), vec![]);
        let mut constraints = empty();
        constraints.insert(eid("CO-01"), vec![]);

        let err = TaskCoverageDocument::new(in_scope, empty(), constraints, empty()).unwrap_err();
        assert!(
            matches!(
                err,
                DomainError::Validation(ValidationError::DuplicateElementId(ref id)) if id == "CO-01"
            ),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn test_new_with_duplicate_id_across_constraints_and_ac_returns_error() {
        let mut constraints = empty();
        constraints.insert(eid("AC-01"), vec![]);
        let mut ac = empty();
        ac.insert(eid("AC-01"), vec![]);

        let err = TaskCoverageDocument::new(empty(), empty(), constraints, ac).unwrap_err();
        assert!(
            matches!(
                err,
                DomainError::Validation(ValidationError::DuplicateElementId(ref id)) if id == "AC-01"
            ),
            "unexpected error: {err:?}"
        );
    }

    // --- accessors ---

    #[test]
    fn test_schema_version_is_1() {
        let doc = TaskCoverageDocument::new(empty(), empty(), empty(), empty()).unwrap();
        assert_eq!(doc.schema_version(), 1);
    }

    #[test]
    fn test_in_scope_accessor_returns_correct_entries() {
        let mut in_scope = empty();
        in_scope.insert(eid("IN-01"), vec![tid("T001")]);
        in_scope.insert(eid("IN-02"), vec![tid("T002"), tid("T003")]);

        let doc = TaskCoverageDocument::new(in_scope, empty(), empty(), empty()).unwrap();
        assert_eq!(doc.in_scope().len(), 2);
        assert_eq!(doc.in_scope()[&eid("IN-02")].len(), 2);
    }

    #[test]
    fn test_btreemap_keys_are_sorted() {
        let mut in_scope = empty();
        // Insert in non-sorted order.
        in_scope.insert(eid("IN-03"), vec![]);
        in_scope.insert(eid("IN-01"), vec![]);
        in_scope.insert(eid("IN-02"), vec![]);

        let doc = TaskCoverageDocument::new(in_scope, empty(), empty(), empty()).unwrap();
        let keys: Vec<&str> = doc.in_scope().keys().map(|k| k.as_ref()).collect();
        // BTreeMap guarantees sorted order.
        assert_eq!(keys, vec!["IN-01", "IN-02", "IN-03"]);
    }
}
