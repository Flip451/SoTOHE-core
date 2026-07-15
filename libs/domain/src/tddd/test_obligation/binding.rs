//! Binding-record value objects for the test-bindings artifact.
//!
//! [`TestLocation`] names where a bound test lives (layer + module path + test
//! function name). [`TestBindingRecord`] is the closed vocabulary of binding
//! outcomes an author records against derived obligations: a fulfilling binding,
//! a justified waiver, or a voluntary binding for an edge with no derived
//! obligation. [`TestBindingsDocument`] is the track-scoped collection persisted
//! as the test-bindings artifact (IN-06 / AC-04 / AC-12 / CN-02).

use crate::TrackId;
use crate::tddd::catalogue_v2::roles::ConstructionError;
use crate::tddd::layer_id::LayerId;
use crate::tddd::test_obligation::ids::{
    TestFunctionName, TestModulePath, TestObligationEdgeId, TestObligationId, WaivedReason,
};

/// Where a bound test lives: its layer, module path, and function name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestLocation {
    layer: LayerId,
    module_path: TestModulePath,
    test_name: TestFunctionName,
}

impl TestLocation {
    /// Builds a [`TestLocation`].
    #[must_use]
    pub fn new(layer: LayerId, module_path: TestModulePath, test_name: TestFunctionName) -> Self {
        Self { layer, module_path, test_name }
    }

    /// Returns the layer the test lives in.
    #[must_use]
    pub fn layer(&self) -> &LayerId {
        &self.layer
    }

    /// Returns the module path the test is declared in.
    #[must_use]
    pub fn module_path(&self) -> &TestModulePath {
        &self.module_path
    }

    /// Returns the test function name.
    #[must_use]
    pub fn test_name(&self) -> &TestFunctionName {
        &self.test_name
    }
}

/// Validated non-empty set of [`TestLocation`] entries.
///
/// Guards the invariant that a [`TestBindingRecord::Fulfillment`] or
/// [`TestBindingRecord::VoluntaryBinding`] record binds at least one test
/// (IN-06). Mirrors the [`NonEmptyVec`](crate::tddd::catalogue_v2::roles::NonEmptyVec)
/// pattern: [`try_new`](Self::try_new) rejects empty vectors via
/// [`ConstructionError::EmptyCollection`] so the codec boundary surfaces the
/// invariant fail-closed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonEmptyTestLocations((TestLocation, Vec<TestLocation>));

impl NonEmptyTestLocations {
    /// Builds a [`NonEmptyTestLocations`] from a required first element and the
    /// remaining (possibly empty) elements; the invariant holds by construction.
    #[must_use]
    pub fn new(first: TestLocation, rest: Vec<TestLocation>) -> Self {
        let mut values = Vec::with_capacity(rest.len() + 1);
        values.push(first.clone());
        values.extend(rest);
        Self((first, values))
    }

    /// Validates and wraps `values`, rejecting an empty vector.
    ///
    /// # Errors
    ///
    /// Returns [`ConstructionError::EmptyCollection`] when `values` is empty.
    pub fn try_new(values: Vec<TestLocation>) -> Result<Self, ConstructionError> {
        let Some(first) = values.first().cloned() else {
            return Err(ConstructionError::EmptyCollection);
        };

        Ok(Self((first, values)))
    }

    /// Returns the bound test locations as a slice, guaranteed non-empty.
    #[must_use]
    pub fn as_slice(&self) -> &[TestLocation] {
        self.0.1.as_slice()
    }

    /// Returns the first bound test location, always present by invariant.
    #[must_use]
    pub fn first(&self) -> &TestLocation {
        &self.0.0
    }

    /// Structural predicate backing the `non_empty` invariant declaration.
    #[must_use]
    pub fn is_non_empty(&self) -> bool {
        true
    }
}

/// A single record in the test-bindings artifact.
///
/// The three variants are mutually exclusive binding outcomes: a
/// [`TestBindingRecord::Fulfillment`] binds one or more tests to a derived
/// obligation, a [`TestBindingRecord::Waiver`] justifies leaving an obligation
/// edge unbound, and a [`TestBindingRecord::VoluntaryBinding`] binds tests to an
/// edge that carried no derived obligation. The non-empty test-evidence
/// invariant for the bound variants is carried by [`NonEmptyTestLocations`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestBindingRecord {
    /// Tests that fulfill a derived obligation.
    Fulfillment {
        /// The obligation being fulfilled.
        obligation_id: TestObligationId,
        /// The tests bound to the obligation.
        tests: NonEmptyTestLocations,
    },
    /// A justified waiver for an obligation edge.
    Waiver {
        /// The obligation edge being waived.
        edge_id: TestObligationEdgeId,
        /// The reason the edge is waived.
        reason: WaivedReason,
    },
    /// Tests voluntarily bound to an edge with no derived obligation.
    VoluntaryBinding {
        /// The edge the tests are voluntarily bound to.
        edge_id: TestObligationEdgeId,
        /// The voluntarily bound tests.
        tests: NonEmptyTestLocations,
    },
}

/// Track-scoped collection of binding records (the test-bindings artifact).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestBindingsDocument {
    track_id: TrackId,
    records: Vec<TestBindingRecord>,
}

impl TestBindingsDocument {
    /// Builds a [`TestBindingsDocument`] for `track_id`.
    #[must_use]
    pub fn new(track_id: TrackId, records: Vec<TestBindingRecord>) -> Self {
        Self { track_id, records }
    }

    /// Returns the track this document was authored for.
    #[must_use]
    pub fn track_id(&self) -> &TrackId {
        &self.track_id
    }

    /// Returns the binding records.
    #[must_use]
    pub fn records(&self) -> &[TestBindingRecord] {
        &self.records
    }

    /// Returns the edges carrying a waiver record.
    ///
    /// Waiver records win per edge: a waived edge is resolved through its
    /// waiver, so fulfillment and voluntary bindings must not compete with it.
    #[must_use]
    pub fn waived_edge_ids(&self) -> Vec<&TestObligationEdgeId> {
        self.records
            .iter()
            .filter_map(|record| match record {
                TestBindingRecord::Waiver { edge_id, .. } => Some(edge_id),
                _ => None,
            })
            .collect()
    }

    /// Returns whether `edge_id` is resolved by a waiver record.
    #[must_use]
    pub fn is_edge_waived(&self, edge_id: &TestObligationEdgeId) -> bool {
        self.records.iter().any(|record| {
            matches!(record, TestBindingRecord::Waiver { edge_id: waived, .. } if waived == edge_id)
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::tddd::semantic_verify::CatalogueEntryKey;
    use crate::tddd::test_obligation::ids::{TestObligationAnchorId, TestObligationItemIdentifier};
    use crate::tddd::test_obligation::vocab::TestObligationKind;

    fn sample_location() -> TestLocation {
        TestLocation::new(
            LayerId::try_new("domain").unwrap(),
            TestModulePath::try_new("domain::user::tests".to_owned()).unwrap(),
            TestFunctionName::try_new("test_rejects_empty".to_owned()).unwrap(),
        )
    }

    fn sample_obligation_id() -> TestObligationId {
        TestObligationId::new(
            CatalogueEntryKey::try_new("domain::User".to_owned()).unwrap(),
            TestObligationKind::Boundary,
            TestObligationItemIdentifier::try_new("invariant:non_empty".to_owned()).unwrap(),
        )
    }

    fn sample_edge_id() -> TestObligationEdgeId {
        TestObligationEdgeId::new(
            CatalogueEntryKey::try_new("domain::User".to_owned()).unwrap(),
            TestObligationAnchorId::try_new("spec.json".to_owned(), "IN-06".to_owned()).unwrap(),
        )
    }

    #[test]
    fn test_location_exposes_components() {
        let location = sample_location();
        assert_eq!(location.layer().as_ref(), "domain");
        assert_eq!(location.module_path().as_str(), "domain::user::tests");
        assert_eq!(location.test_name().as_str(), "test_rejects_empty");
    }

    #[test]
    fn test_non_empty_test_locations_new_exposes_all_entries() {
        let locations = NonEmptyTestLocations::new(sample_location(), vec![sample_location()]);
        assert!(locations.is_non_empty());
        assert_eq!(locations.as_slice().len(), 2);
        assert_eq!(locations.first().test_name().as_str(), "test_rejects_empty");
    }

    #[test]
    fn test_non_empty_test_locations_try_new_accepts_non_empty() {
        let locations = NonEmptyTestLocations::try_new(vec![sample_location()]).unwrap();
        assert!(locations.is_non_empty());
        assert_eq!(locations.as_slice().len(), 1);
    }

    #[test]
    fn test_non_empty_test_locations_try_new_rejects_empty() {
        assert_eq!(NonEmptyTestLocations::try_new(vec![]), Err(ConstructionError::EmptyCollection));
    }

    #[test]
    fn test_fulfillment_record_holds_obligation_and_tests() {
        let record = TestBindingRecord::Fulfillment {
            obligation_id: sample_obligation_id(),
            tests: NonEmptyTestLocations::try_new(vec![sample_location()]).unwrap(),
        };
        match record {
            TestBindingRecord::Fulfillment { obligation_id, tests } => {
                assert_eq!(obligation_id.entry_key().as_str(), "domain::User");
                assert_eq!(tests.as_slice().len(), 1);
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn test_waiver_record_holds_edge_and_reason() {
        let record = TestBindingRecord::Waiver {
            edge_id: sample_edge_id(),
            reason: WaivedReason::try_new("covered elsewhere".to_owned()).unwrap(),
        };
        match record {
            TestBindingRecord::Waiver { edge_id, reason } => {
                assert_eq!(edge_id.entry_key().as_str(), "domain::User");
                assert_eq!(reason.as_str(), "covered elsewhere");
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn test_voluntary_binding_record_holds_edge_and_tests() {
        let record = TestBindingRecord::VoluntaryBinding {
            edge_id: sample_edge_id(),
            tests: NonEmptyTestLocations::try_new(vec![sample_location()]).unwrap(),
        };
        match record {
            TestBindingRecord::VoluntaryBinding { edge_id, tests } => {
                assert_eq!(edge_id.anchor_id().element_id(), "IN-06");
                assert_eq!(tests.as_slice().len(), 1);
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn test_bindings_document_round_trips() {
        let track_id = TrackId::try_new("my-track").unwrap();
        let doc = TestBindingsDocument::new(
            track_id.clone(),
            vec![TestBindingRecord::Fulfillment {
                obligation_id: sample_obligation_id(),
                tests: NonEmptyTestLocations::try_new(vec![sample_location()]).unwrap(),
            }],
        );
        assert_eq!(doc.track_id(), &track_id);
        assert_eq!(doc.records().len(), 1);
    }

    #[test]
    fn test_waived_edge_ids_reports_exactly_the_waiver_records() {
        let waived = sample_edge_id();
        let doc = TestBindingsDocument::new(
            TrackId::try_new("my-track").unwrap(),
            vec![
                TestBindingRecord::Fulfillment {
                    obligation_id: sample_obligation_id(),
                    tests: NonEmptyTestLocations::try_new(vec![sample_location()]).unwrap(),
                },
                TestBindingRecord::VoluntaryBinding {
                    edge_id: other_edge_id(),
                    tests: NonEmptyTestLocations::try_new(vec![sample_location()]).unwrap(),
                },
                TestBindingRecord::Waiver {
                    edge_id: waived.clone(),
                    reason: WaivedReason::try_new("covered elsewhere".to_owned()).unwrap(),
                },
            ],
        );

        assert_eq!(doc.waived_edge_ids(), vec![&waived]);
        assert!(doc.is_edge_waived(&waived), "the waiver record owns its edge");
        assert!(
            !doc.is_edge_waived(&other_edge_id()),
            "fulfillment and voluntary records never mark an edge waived"
        );
    }

    fn other_edge_id() -> TestObligationEdgeId {
        TestObligationEdgeId::new(
            CatalogueEntryKey::try_new("domain::User".to_owned()).unwrap(),
            TestObligationAnchorId::try_new("spec.json".to_owned(), "IN-07".to_owned()).unwrap(),
        )
    }
}
