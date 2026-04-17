//! Validated architecture layer identifier.
//!
//! Wraps a `layers[].crate` value from `architecture-rules.json` as a
//! validated newtype. Used across the TDDD contract-map pipeline to
//! eliminate raw `String` at port and adapter boundaries (ADR
//! 2026-04-17-1528 §D1; layer-agnostic invariant §4.5).
//!
//! Validation rules: non-empty, first character is ASCII alphabetic, remaining
//! characters are ASCII alphanumeric / `_` / `-`. This covers both Rust crate
//! names (`snake_case`) and hyphenated template identifiers (`my-gateway`).
//!
//! **Why a hand-written newtype instead of `nutype`?** The current
//! `schema_export` + rustdoc JSON pipeline does not resolve `pub use`
//! aliases that point into `#[doc(hidden)]` modules, which is how
//! `nutype` publishes its generated structs. As a result every
//! `nutype`-wrapped type silently disappears from the rustdoc schema and
//! the TDDD forward check classifies them as Yellow
//! (`found_type = false`) even when they exist in the codebase. Using a
//! plain `pub struct` here keeps `LayerId` visible to the current
//! schema-export path and lets the catalogue entry for the type reach
//! Blue. Properly teaching `schema_export` to follow `pub use` aliases is
//! tracked separately — see `knowledge/strategy/TODO.md`
//! "harness-hardening-nutype-rustdoc-support".

use std::fmt;

use crate::ValidationError;

/// A validated architecture layer identifier (e.g. `domain`, `usecase`,
/// `my-gateway`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LayerId(String);

impl LayerId {
    /// Validate and wrap `value` as a [`LayerId`].
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::InvalidLayerId`] when `value` is
    /// empty, starts with a non-ASCII-alphabetic character, or contains
    /// characters outside `[A-Za-z0-9_-]`.
    pub fn try_new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        if is_valid_layer_id(&value) {
            Ok(Self(value))
        } else {
            Err(ValidationError::InvalidLayerId(value))
        }
    }
}

impl AsRef<str> for LayerId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for LayerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
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
