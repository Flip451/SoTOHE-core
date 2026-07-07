//! Load-time error vocabulary for the test-obligation rules config.
//!
//! [`TestObligationRulesLoadError`] is produced when
//! `.harness/config/test-obligation-rules.json` fails the fail-closed load-time
//! totality validation (IN-04 / CN-05 / AC-02): a role missing its rule entry,
//! an implicit zero-obligation declaration, an unknown role key, an invalid
//! rule value, or an I/O / JSON failure.

use crate::tddd::test_obligation::ids::{DiagnosticMessage, RoleName};

/// Error raised while loading and validating the test-obligation rules config.
///
/// Each role-scoped variant carries the [`RoleName`] it concerns so the CLI can
/// report exactly which config entry failed (IN-04 / CN-05 / AC-02).
#[derive(Debug)]
pub enum TestObligationRulesLoadError {
    /// A role enum variant has no entry in the config (totality violation).
    RoleNotCovered {
        /// The role that the config failed to declare.
        role_name: RoleName,
    },
    /// A role entry omitted the `obligations` field instead of declaring `[]`.
    ObligationsFieldOmitted {
        /// The role whose entry omitted `obligations`.
        role_name: RoleName,
    },
    /// The config declared a key that does not resolve to a known role.
    UnknownRoleName {
        /// The unrecognised role key found in the config.
        role_name: RoleName,
    },
    /// A rule value was syntactically present but semantically invalid.
    InvalidRuleValue {
        /// The role whose rule failed validation.
        role_name: RoleName,
        /// Human-readable detail describing the invalid value.
        message: DiagnosticMessage,
    },
    /// The config file could not be read.
    IoError(DiagnosticMessage),
    /// The config file was not valid JSON for the expected schema.
    MalformedJson(DiagnosticMessage),
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_role_not_covered_carries_role_name() {
        let err = TestObligationRulesLoadError::RoleNotCovered {
            role_name: RoleName::try_new("ValueObject".to_owned()).unwrap(),
        };
        match err {
            TestObligationRulesLoadError::RoleNotCovered { role_name } => {
                assert_eq!(role_name.as_str(), "ValueObject");
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn test_invalid_rule_value_carries_message() {
        let err = TestObligationRulesLoadError::InvalidRuleValue {
            role_name: RoleName::try_new("UseCase".to_owned()).unwrap(),
            message: DiagnosticMessage::try_new("unknown kind 'frobnicate'".to_owned()).unwrap(),
        };
        match err {
            TestObligationRulesLoadError::InvalidRuleValue { role_name, message } => {
                assert_eq!(role_name.as_str(), "UseCase");
                assert_eq!(message.as_str(), "unknown kind 'frobnicate'");
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn test_malformed_json_is_debuggable() {
        let err = TestObligationRulesLoadError::MalformedJson(
            DiagnosticMessage::try_new("expected object".to_owned()).unwrap(),
        );
        assert!(format!("{err:?}").contains("MalformedJson"));
    }
}
