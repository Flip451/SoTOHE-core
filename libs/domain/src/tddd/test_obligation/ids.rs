//! Validated string newtypes shared across the test-obligation domain model.
//!
//! Holds the foundational value objects that the rules / errors modules depend
//! on: [`RoleName`] (a role identifier used in load-time diagnostics) and
//! [`DiagnosticMessage`] (a non-empty human-readable diagnostic string). Both
//! reject empty / whitespace-only input at construction (IN-04 / CN-05).

use crate::ValidationError;

/// A validated role name used in test-obligation rules diagnostics.
///
/// Wraps the raw role identifier that appears as a key in
/// `.harness/config/test-obligation-rules.json`. Kept as a distinct newtype so
/// load-time errors ([`crate::tddd::test_obligation::errors::TestObligationRulesLoadError`])
/// carry a role name rather than a bare `String`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RoleName(String);

impl RoleName {
    /// Validate and wrap `value` as a [`RoleName`].
    ///
    /// The input is trimmed before the emptiness check, so whitespace-only
    /// strings are treated as empty.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::EmptyString`] when `value` is empty or
    /// whitespace-only.
    pub fn try_new(value: String) -> Result<Self, ValidationError> {
        if value.trim().is_empty() {
            return Err(ValidationError::EmptyString);
        }
        Ok(Self(value))
    }

    /// Borrow the inner role name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A validated, non-empty diagnostic message.
///
/// Used by test-obligation errors and verdicts to carry human-readable detail
/// without leaking a bare `String` across port boundaries. Empty /
/// whitespace-only input is rejected so a diagnostic can never be silent.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DiagnosticMessage(String);

impl DiagnosticMessage {
    /// Validate and wrap `value` as a [`DiagnosticMessage`].
    ///
    /// The input is trimmed before the emptiness check, so whitespace-only
    /// strings are treated as empty.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::EmptyString`] when `value` is empty or
    /// whitespace-only.
    pub fn try_new(value: String) -> Result<Self, ValidationError> {
        if value.trim().is_empty() {
            return Err(ValidationError::EmptyString);
        }
        Ok(Self(value))
    }

    /// Borrow the inner diagnostic string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_role_name_accepts_non_empty() {
        let name = RoleName::try_new("ValueObject".to_owned()).unwrap();
        assert_eq!(name.as_str(), "ValueObject");
    }

    #[test]
    fn test_role_name_rejects_empty() {
        assert_eq!(RoleName::try_new(String::new()), Err(ValidationError::EmptyString));
    }

    #[test]
    fn test_role_name_rejects_whitespace_only() {
        assert_eq!(RoleName::try_new("   ".to_owned()), Err(ValidationError::EmptyString));
    }

    #[test]
    fn test_diagnostic_message_accepts_non_empty() {
        let msg = DiagnosticMessage::try_new("boom".to_owned()).unwrap();
        assert_eq!(msg.as_str(), "boom");
    }

    #[test]
    fn test_diagnostic_message_rejects_empty() {
        assert_eq!(DiagnosticMessage::try_new(String::new()), Err(ValidationError::EmptyString));
    }

    #[test]
    fn test_diagnostic_message_rejects_whitespace_only() {
        assert_eq!(
            DiagnosticMessage::try_new(" \n\t ".to_owned()),
            Err(ValidationError::EmptyString)
        );
    }
}
