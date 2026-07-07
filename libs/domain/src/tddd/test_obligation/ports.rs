//! Domain ports for the test-obligation gate.
//!
//! Holds the driven port the derivation / check use cases depend on to obtain
//! the decision table. The concrete loader (JSON codec over
//! `.harness/config/test-obligation-rules.json`) lives in the infrastructure
//! layer (IN-02 / IN-04 / AC-02).

use crate::tddd::test_obligation::errors::TestObligationRulesLoadError;
use crate::tddd::test_obligation::rules::TestObligationRulesDocument;

/// Loads and validates the test-obligation decision-table config.
///
/// Synchronous to match the existing domain port baseline. The implementation
/// performs the fail-closed load-time totality validation and returns the parsed
/// [`TestObligationRulesDocument`], or a [`TestObligationRulesLoadError`]
/// describing the first failure.
pub trait TestObligationRulesLoaderPort {
    /// Loads the decision table from the configured source.
    ///
    /// # Errors
    ///
    /// Returns a [`TestObligationRulesLoadError`] when the config cannot be read,
    /// is malformed, omits a role, leaves `obligations` implicit, or names an
    /// unknown role.
    fn load(&self) -> Result<TestObligationRulesDocument, TestObligationRulesLoadError>;
}
