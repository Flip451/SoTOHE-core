//! Named claim/evidence payload value objects for semantic obligation
//! evaluation (IN-09).
//!
//! Each pair bundles the claim, evidence, and entry-local responsibility inputs
//! a semantic verifier reasons over into a named value object, replacing an
//! anonymous `(String, String, String)` triple so the components can no longer
//! be positionally swapped:
//!
//! - [`ObligationFulfillmentPair`]: bound test source vs. the catalogue entry
//!   declaration and structured specification element (the fulfillment lane).
//! - [`WaiverPair`]: the waiver reason vs. the same declaration and structured
//!   specification element (the waiver lane).
//!
//! Every textual claim, reason, and declaration component is a validated
//! non-empty newtype ([`TestsSource`], [`EntryDeclaration`], and
//! [`WaivedReason`]); the existing structured reference ([`SpecElementRef`])
//! preserves the specification element's identity, section, and verbatim text.

use crate::ValidationError;
use crate::tddd::semantic_verify::SpecElementRef;
use crate::tddd::test_obligation::ids::{TestObligationBrief, TestObligationId, WaivedReason};

/// Validated non-empty bound-test source text (the fulfillment claim side).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestsSource {
    value: String,
}

impl TestsSource {
    /// Validates and wraps `text` as a [`TestsSource`].
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::EmptyString`] when `text` is empty or
    /// whitespace-only.
    pub fn try_new(text: String) -> Result<Self, ValidationError> {
        if text.trim().is_empty() {
            return Err(ValidationError::EmptyString);
        }
        Ok(Self { value: text })
    }

    /// Borrows the inner test source text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.value
    }
}

/// Validated non-empty catalogue entry declaration text (the evidence side).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryDeclaration {
    value: String,
}

impl EntryDeclaration {
    /// Validates and wraps `text` as an [`EntryDeclaration`].
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::EmptyString`] when `text` is empty or
    /// whitespace-only.
    pub fn try_new(text: String) -> Result<Self, ValidationError> {
        if text.trim().is_empty() {
            return Err(ValidationError::EmptyString);
        }
        Ok(Self { value: text })
    }

    /// Borrows the inner declaration text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.value
    }
}

/// Claim/evidence payload for the obligation-fulfillment lane (IN-09).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObligationFulfillmentPair {
    tests_source: TestsSource,
    entry_declaration: EntryDeclaration,
    spec_element: SpecElementRef,
    obligation_id: TestObligationId,
    obligation_brief: TestObligationBrief,
}

impl ObligationFulfillmentPair {
    /// Builds an [`ObligationFulfillmentPair`] from its validated claim,
    /// evidence, and entry-local responsibility components.
    #[must_use]
    pub fn new(
        tests_source: TestsSource,
        entry_declaration: EntryDeclaration,
        spec_element: SpecElementRef,
        obligation_id: TestObligationId,
        obligation_brief: TestObligationBrief,
    ) -> Self {
        Self { tests_source, entry_declaration, spec_element, obligation_id, obligation_brief }
    }

    /// Borrows the concatenated bound test source (the claim side).
    #[must_use]
    pub fn tests_source(&self) -> &TestsSource {
        &self.tests_source
    }

    /// Borrows the catalogue entry declaration (the evidence side).
    #[must_use]
    pub fn entry_declaration(&self) -> &EntryDeclaration {
        &self.entry_declaration
    }

    /// Borrows the structured specification element the obligation binds to.
    #[must_use]
    pub fn spec_element(&self) -> &SpecElementRef {
        &self.spec_element
    }

    /// Borrows the stable identity of the obligation being judged.
    #[must_use]
    pub fn obligation_id(&self) -> &TestObligationId {
        &self.obligation_id
    }

    /// Borrows the responsibility brief for the obligation being judged.
    #[must_use]
    pub fn obligation_brief(&self) -> &TestObligationBrief {
        &self.obligation_brief
    }
}

/// Claim/evidence payload for the waiver lane (IN-09).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaiverPair {
    waived_reason: WaivedReason,
    entry_declaration: EntryDeclaration,
    spec_element: SpecElementRef,
    obligation_id: TestObligationId,
    obligation_brief: TestObligationBrief,
}

impl WaiverPair {
    /// Builds a [`WaiverPair`] from its validated claim, evidence, and
    /// entry-local responsibility components.
    #[must_use]
    pub fn new(
        waived_reason: WaivedReason,
        entry_declaration: EntryDeclaration,
        spec_element: SpecElementRef,
        obligation_id: TestObligationId,
        obligation_brief: TestObligationBrief,
    ) -> Self {
        Self { waived_reason, entry_declaration, spec_element, obligation_id, obligation_brief }
    }

    /// Borrows the waiver reason (the claim side).
    #[must_use]
    pub fn waived_reason(&self) -> &WaivedReason {
        &self.waived_reason
    }

    /// Borrows the catalogue entry declaration (the evidence side).
    #[must_use]
    pub fn entry_declaration(&self) -> &EntryDeclaration {
        &self.entry_declaration
    }

    /// Borrows the structured specification element the obligation binds to.
    #[must_use]
    pub fn spec_element(&self) -> &SpecElementRef {
        &self.spec_element
    }

    /// Borrows the stable identity of the obligation being judged.
    #[must_use]
    pub fn obligation_id(&self) -> &TestObligationId {
        &self.obligation_id
    }

    /// Borrows the responsibility brief for the obligation being judged.
    #[must_use]
    pub fn obligation_brief(&self) -> &TestObligationBrief {
        &self.obligation_brief
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::plan_ref::SpecElementId;
    use crate::tddd::catalogue_v2::CatalogueEntryKey;
    use crate::tddd::semantic_verify::SpecSectionKind;
    use crate::tddd::test_obligation::ids::TestObligationItemIdentifier;
    use crate::tddd::test_obligation::vocab::TestObligationKind;

    fn obligation_id() -> TestObligationId {
        TestObligationId::new(
            CatalogueEntryKey::try_new("Entry".to_owned()).unwrap(),
            TestObligationKind::Contract,
            TestObligationItemIdentifier::try_new("trait_method:verify".to_owned()).unwrap(),
        )
    }

    fn obligation_brief() -> TestObligationBrief {
        TestObligationBrief::try_new("verify the entry-local contract".to_owned()).unwrap()
    }

    fn spec_element() -> SpecElementRef {
        SpecElementRef::new(
            SpecSectionKind::InScope,
            SpecElementId::try_new("IN-01".to_owned()).unwrap(),
            "the entry-local contract".to_owned(),
        )
    }

    #[test]
    fn newtypes_reject_blank_input() {
        assert_eq!(TestsSource::try_new(String::new()), Err(ValidationError::EmptyString));
        assert_eq!(EntryDeclaration::try_new("   ".to_owned()), Err(ValidationError::EmptyString));
    }

    #[test]
    fn newtypes_expose_non_empty_input() {
        assert_eq!(TestsSource::try_new("tests".to_owned()).unwrap().as_str(), "tests");
        assert_eq!(EntryDeclaration::try_new("decl".to_owned()).unwrap().as_str(), "decl");
    }

    #[test]
    fn fulfillment_pair_exposes_components_in_order() {
        let pair = ObligationFulfillmentPair::new(
            TestsSource::try_new("tests".to_owned()).unwrap(),
            EntryDeclaration::try_new("decl".to_owned()).unwrap(),
            spec_element(),
            obligation_id(),
            obligation_brief(),
        );
        assert_eq!(pair.tests_source().as_str(), "tests");
        assert_eq!(pair.entry_declaration().as_str(), "decl");
        assert_eq!(pair.spec_element(), &spec_element());
        assert_eq!(pair.obligation_id(), &obligation_id());
        assert_eq!(pair.obligation_brief(), &obligation_brief());
    }

    #[test]
    fn waiver_pair_exposes_components_in_order() {
        let pair = WaiverPair::new(
            WaivedReason::try_new("reason".to_owned()).unwrap(),
            EntryDeclaration::try_new("decl".to_owned()).unwrap(),
            spec_element(),
            obligation_id(),
            obligation_brief(),
        );
        assert_eq!(pair.waived_reason().as_str(), "reason");
        assert_eq!(pair.entry_declaration().as_str(), "decl");
        assert_eq!(pair.spec_element(), &spec_element());
        assert_eq!(pair.obligation_id(), &obligation_id());
        assert_eq!(pair.obligation_brief(), &obligation_brief());
    }

    #[test]
    fn pairs_participate_in_equality() {
        let build = |tests: &str| {
            ObligationFulfillmentPair::new(
                TestsSource::try_new(tests.to_owned()).unwrap(),
                EntryDeclaration::try_new("d".to_owned()).unwrap(),
                spec_element(),
                obligation_id(),
                obligation_brief(),
            )
        };
        assert_eq!(build("t"), build("t"));
        assert_ne!(build("t"), build("x"));
    }
}
