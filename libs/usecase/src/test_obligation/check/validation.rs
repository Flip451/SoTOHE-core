//! Binding invariants enforced before check-edge resolution.

use domain::tddd::test_obligation::binding::{TestBindingRecord, TestBindingsDocument};
use domain::tddd::test_obligation::errors::TestBindingConsistencyError;
use domain::tddd::test_obligation::obligations::ObligationsDocument;

/// Applies the domain-owned voluntary-binding ownership invariant to every record.
pub(super) fn validate_voluntary_bindings(
    obligations: &ObligationsDocument,
    bindings: &TestBindingsDocument,
) -> Result<(), TestBindingConsistencyError> {
    for record in bindings.records() {
        if let TestBindingRecord::VoluntaryBinding { edge_id, .. } = record {
            obligations.validate_voluntary_binding(edge_id)?;
        }
    }
    Ok(())
}
