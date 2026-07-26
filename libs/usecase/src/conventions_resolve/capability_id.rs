//! Capability identifier in its matching-term role, split from the
//! `agent-profiles.json` lookup key (spec `IN-06`, `AC-06`, `AC-07`, `AC-09`).
//!
//! Re-exported from the parent module, whose type contract this file is part
//! of; it is a separate file only so that neither module outgrows the
//! workspace module-size limit.

use crate::dry_write_driver::CapabilityName;

/// Capability identifier as convention matching compares it (`IN-06`,
/// `AC-06`, `AC-09`).
///
/// This is deliberately not [`CapabilityName`], which stays the lookup key it
/// already is: that type's constructor trims the supplied value, which is a
/// defensible tolerance for a key into `agent-profiles.json` but would make
/// matching a normalize-then-compare, under which a padded identifier matches
/// an unpadded one. Preserving the value verbatim is what makes the exact
/// match literally true, and both sides of the comparison — a document's
/// `required_for` element and the requested identifier — carry this type, so
/// the conflation cannot reappear mirrored on the query side.
///
/// [`ConventionCapabilityId::try_new`] is the only site establishing the
/// non-blank invariant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConventionCapabilityId(String);

impl ConventionCapabilityId {
    /// Validates and wraps a capability identifier, preserving it verbatim.
    ///
    /// The identifier space is open-ended (`AC-09`): registration in
    /// `.harness/capabilities/` or `agent-profiles.json` is not consulted, and
    /// surrounding whitespace is neither stripped nor rejected. A padded
    /// identifier is a legitimate value that simply matches nothing spelled
    /// differently — rejecting it would add a sixth condition to the
    /// fail-closed list `AC-07` presents as exhaustive.
    ///
    /// # Errors
    ///
    /// Returns [`ConventionCapabilityIdError::Blank`] when `value` is empty or
    /// whitespace-only, which are the two conditions that list does name.
    pub fn try_new(value: impl Into<String>) -> Result<Self, ConventionCapabilityIdError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(ConventionCapabilityIdError::Blank);
        }
        Ok(Self(value))
    }

    /// Borrows the identifier exactly as it was supplied.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ConventionCapabilityId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl From<&CapabilityName> for ConventionCapabilityId {
    /// Bridges the lookup key into a matching term.
    ///
    /// Total, because a [`CapabilityName`] already holds a trimmed non-empty
    /// string. There is deliberately no conversion the other way: a matching
    /// term must not be usable as a profile lookup key.
    fn from(name: &CapabilityName) -> Self {
        Self(name.as_str().to_owned())
    }
}

/// Construction failure of [`ConventionCapabilityId`].
///
/// Extracted as its own type so that every consumer of the constructor admits
/// exactly this rejection rather than the whole resolver error surface.
#[derive(Debug, thiserror::Error)]
pub enum ConventionCapabilityIdError {
    /// The candidate identifier is empty or whitespace-only.
    ///
    /// A unit variant because the rejected value has nothing informative to
    /// carry: it is empty or whitespace-only by definition.
    #[error("capability id is empty or whitespace-only")]
    Blank,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::{ConventionCapabilityId, ConventionCapabilityIdError};
    use crate::dry_write_driver::CapabilityName;

    #[test]
    fn test_convention_capability_id_accepts_a_non_blank_identifier_unchanged() {
        for accepted in ["implementer", "team::custom-lint", "SHOUTING_ID", "x"] {
            let id = ConventionCapabilityId::try_new(accepted)
                .expect("a non-blank capability id is accepted as-is");

            assert_eq!(id.as_str(), accepted);
        }
    }

    #[test]
    fn test_convention_capability_id_preserves_surrounding_whitespace_verbatim() {
        let padded = ConventionCapabilityId::try_new(" reviewer ")
            .expect("a padded identifier is not blank, so it is a legitimate value");

        assert_eq!(
            padded.as_str(),
            " reviewer ",
            "the value is preserved byte-for-byte, so the match stays a literal whole-value \
             comparison rather than a normalize-then-compare"
        );
        assert_ne!(
            padded,
            ConventionCapabilityId::try_new("reviewer").unwrap(),
            "a padded identifier is a different identifier, not the same one written loosely"
        );
    }

    #[test]
    fn test_convention_capability_id_rejects_empty_and_whitespace_only_identifiers() {
        for blank in ["", "   ", "\t", "\n", " \t\n "] {
            assert!(
                matches!(
                    ConventionCapabilityId::try_new(blank),
                    Err(ConventionCapabilityIdError::Blank)
                ),
                "'{blank:?}' carries no identifier, which is the rejection this value admits"
            );
        }
    }

    #[test]
    fn test_convention_capability_id_rejection_renders_the_blank_condition() {
        let Err(error) = ConventionCapabilityId::try_new("   ") else {
            panic!("expected a Blank rejection");
        };

        let rendered = error.to_string();
        assert!(rendered.contains("empty or whitespace-only"), "{rendered}");
    }

    #[test]
    fn test_convention_capability_id_from_capability_name_carries_the_trimmed_key_value() {
        let key = CapabilityName::try_new(" reviewer ")
            .expect("the lookup key trims its input and accepts this");

        let id = ConventionCapabilityId::from(&key);

        assert_eq!(
            id.as_str(),
            "reviewer",
            "the bridge carries the value the key already holds, which is the trimmed one"
        );
        assert_eq!(id, ConventionCapabilityId::try_new("reviewer").unwrap());
    }

    #[test]
    fn test_convention_capability_id_display_renders_the_identifier() {
        let id = ConventionCapabilityId::try_new("consumer-house-style").unwrap();

        assert_eq!(id.to_string(), "consumer-house-style");
    }

    #[test]
    fn test_convention_capability_id_accepts_an_identifier_registered_in_no_registry() {
        // `consumer-house-style` names no entry in `.harness/capabilities/`
        // and none in `agent-profiles.json`. The constructor consults neither,
        // so there is no state in which it fails for want of a registration.
        let id = ConventionCapabilityId::try_new("consumer-house-style")
            .expect("an identifier registered nowhere is still a valid matching term");

        assert_eq!(id.as_str(), "consumer-house-style");
    }
}
