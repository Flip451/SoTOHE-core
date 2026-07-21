//! Named claim/evidence payload value objects for semantic obligation
//! evaluation (IN-09).
//!
//! Each pair bundles the three text components a semantic verifier reasons over
//! into a named value object, replacing an anonymous `(String, String, String)`
//! triple so the components can no longer be positionally swapped:
//!
//! - [`ObligationFulfillmentPair`]: bound test source vs. the catalogue entry
//!   declaration and the anchor text (the fulfillment lane).
//! - [`WaiverPair`]: the waiver reason vs. the same declaration and anchor text
//!   (the waiver lane).
//!
//! Every component is a validated non-empty newtype ([`TestsSource`],
//! [`EntryDeclaration`], [`AnchorText`], and [`WaivedReason`]) so a pair can
//! never be constructed with an empty claim or evidence side.

use crate::ValidationError;
use crate::tddd::test_obligation::ids::WaivedReason;

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

/// Validated non-empty anchor text an obligation binds to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorText {
    value: String,
}

impl AnchorText {
    /// Validates and wraps `text` as an [`AnchorText`].
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

    /// Borrows the inner anchor text.
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
    anchor_text: AnchorText,
}

impl ObligationFulfillmentPair {
    /// Builds an [`ObligationFulfillmentPair`] from its three validated components.
    #[must_use]
    pub fn new(
        tests_source: TestsSource,
        entry_declaration: EntryDeclaration,
        anchor_text: AnchorText,
    ) -> Self {
        Self { tests_source, entry_declaration, anchor_text }
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

    /// Borrows the anchor text the obligation binds to.
    #[must_use]
    pub fn anchor_text(&self) -> &AnchorText {
        &self.anchor_text
    }
}

/// Claim/evidence payload for the waiver lane (IN-09).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaiverPair {
    waived_reason: WaivedReason,
    entry_declaration: EntryDeclaration,
    anchor_text: AnchorText,
}

impl WaiverPair {
    /// Builds a [`WaiverPair`] from its three validated components.
    #[must_use]
    pub fn new(
        waived_reason: WaivedReason,
        entry_declaration: EntryDeclaration,
        anchor_text: AnchorText,
    ) -> Self {
        Self { waived_reason, entry_declaration, anchor_text }
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

    /// Borrows the anchor text the obligation binds to.
    #[must_use]
    pub fn anchor_text(&self) -> &AnchorText {
        &self.anchor_text
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn newtypes_reject_blank_input() {
        assert_eq!(TestsSource::try_new(String::new()), Err(ValidationError::EmptyString));
        assert_eq!(EntryDeclaration::try_new("   ".to_owned()), Err(ValidationError::EmptyString));
        assert_eq!(AnchorText::try_new(" \n\t ".to_owned()), Err(ValidationError::EmptyString));
    }

    #[test]
    fn newtypes_expose_non_empty_input() {
        assert_eq!(TestsSource::try_new("tests".to_owned()).unwrap().as_str(), "tests");
        assert_eq!(EntryDeclaration::try_new("decl".to_owned()).unwrap().as_str(), "decl");
        assert_eq!(AnchorText::try_new("anchor".to_owned()).unwrap().as_str(), "anchor");
    }

    #[test]
    fn fulfillment_pair_exposes_components_in_order() {
        let pair = ObligationFulfillmentPair::new(
            TestsSource::try_new("tests".to_owned()).unwrap(),
            EntryDeclaration::try_new("decl".to_owned()).unwrap(),
            AnchorText::try_new("anchor".to_owned()).unwrap(),
        );
        assert_eq!(pair.tests_source().as_str(), "tests");
        assert_eq!(pair.entry_declaration().as_str(), "decl");
        assert_eq!(pair.anchor_text().as_str(), "anchor");
    }

    #[test]
    fn waiver_pair_exposes_components_in_order() {
        let pair = WaiverPair::new(
            WaivedReason::try_new("reason".to_owned()).unwrap(),
            EntryDeclaration::try_new("decl".to_owned()).unwrap(),
            AnchorText::try_new("anchor".to_owned()).unwrap(),
        );
        assert_eq!(pair.waived_reason().as_str(), "reason");
        assert_eq!(pair.entry_declaration().as_str(), "decl");
        assert_eq!(pair.anchor_text().as_str(), "anchor");
    }

    #[test]
    fn pairs_participate_in_equality() {
        let build = |tests: &str| {
            ObligationFulfillmentPair::new(
                TestsSource::try_new(tests.to_owned()).unwrap(),
                EntryDeclaration::try_new("d".to_owned()).unwrap(),
                AnchorText::try_new("a".to_owned()).unwrap(),
            )
        };
        assert_eq!(build("t"), build("t"));
        assert_ne!(build("t"), build("x"));
    }
}
