//! Validated architecture layer identifier.
//!
//! Wraps a `layers[].crate` value from `architecture-rules.json` as a
//! `nutype`-validated newtype. Used across the TDDD contract-map pipeline to
//! eliminate raw `String` at port and adapter boundaries (ADR
//! 2026-04-17-1528 §D1; layer-agnostic invariant §4.5).
//!
//! Validation rules: non-empty, first character is ASCII alphabetic, remaining
//! characters are ASCII alphanumeric / `_` / `-`. This covers both Rust crate
//! names (`snake_case`) and hyphenated template identifiers (`my-gateway`).

use nutype::nutype;

use crate::ValidationError;

fn validate_layer_id(value: &str) -> Result<(), ValidationError> {
    if is_valid_layer_id(value) {
        Ok(())
    } else {
        Err(ValidationError::InvalidLayerId(value.to_owned()))
    }
}

fn is_valid_layer_id(value: &str) -> bool {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() => {}
        _ => return false,
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

/// A validated architecture layer identifier (e.g. `domain`, `usecase`,
/// `my-gateway`).
#[nutype(
    validate(with = validate_layer_id, error = ValidationError),
    derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Display, AsRef)
)]
pub struct LayerId(String);

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_layer_id_accepts_snake_case() {
        let id = LayerId::try_new("domain").unwrap();
        assert_eq!(id.as_ref(), "domain");
    }

    #[test]
    fn test_layer_id_accepts_underscore_and_hyphen() {
        assert!(LayerId::try_new("app_core").is_ok());
        assert!(LayerId::try_new("my-gateway").is_ok());
        assert!(LayerId::try_new("layer_v2").is_ok());
    }

    #[test]
    fn test_layer_id_rejects_empty_string() {
        let err = LayerId::try_new("").unwrap_err();
        assert!(matches!(err, ValidationError::InvalidLayerId(s) if s.is_empty()));
    }

    #[test]
    fn test_layer_id_rejects_leading_digit() {
        assert!(matches!(
            LayerId::try_new("1layer").unwrap_err(),
            ValidationError::InvalidLayerId(_)
        ));
    }

    #[test]
    fn test_layer_id_rejects_leading_symbol() {
        assert!(LayerId::try_new("-layer").is_err());
        assert!(LayerId::try_new("_layer").is_err());
    }

    #[test]
    fn test_layer_id_rejects_internal_spaces_and_slashes() {
        assert!(LayerId::try_new("domain core").is_err());
        assert!(LayerId::try_new("apps/cli").is_err());
    }

    #[test]
    fn test_layer_id_ordering_is_alphabetic() {
        let a = LayerId::try_new("domain").unwrap();
        let b = LayerId::try_new("usecase").unwrap();
        assert!(a < b);
    }

    #[test]
    fn test_layer_id_display_matches_inner_string() {
        let id = LayerId::try_new("infrastructure").unwrap();
        assert_eq!(id.to_string(), "infrastructure");
    }
}
