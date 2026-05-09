//! Error type for the Catalogue → ExtendedCrate (TypeGraph A) infrastructure codec.
//!
//! `CatalogueToExtendedCrateCodecError` is the infrastructure-layer error type
//! for `CatalogueToExtendedCrateCodec` (implemented in T005).
//!
//! The codec converts a domain `CatalogueDocument` to an `ExtendedCrate` via
//! the `CatalogueToExtendedCratePort` trait.  This module provides the
//! infrastructure-specific error wrapper that may carry additional context (e.g.
//! the offending `TypeRef` string, the conflicting name).
//!
//! ## Variants
//!
//! * `InvalidTypeRef` — a `TypeRef` string in the catalogue could not be parsed
//!   by `syn` into a valid Rust type expression.
//! * `AmbiguousIdentifier` — a short type name collides with another entry in
//!   the same catalogue, making `Id` assignment ambiguous.
//!
//! Note: crate-prefixed `TypeRef` values (e.g. `"domain_core::UserId"`) are
//! **never** rejected; they are auto-collected into `external_crates`
//! (ADR 2 D5 / D11 open-world semantics).  `ExternalCrateUnresolvable` is not
//! a variant of this error type.
//!
//! (infrastructure-types.json entry: `CatalogueToExtendedCrateCodecError`)

use domain::tddd::NewTypeGraphCodecError;
use thiserror::Error;

/// Infrastructure-level error for `CatalogueToExtendedCrateCodec`.
///
/// Mirrors the domain `NewTypeGraphCodecError` variants but carries additional
/// context strings for richer diagnostics at the infrastructure boundary.
#[derive(Debug, Error)]
pub enum CatalogueToExtendedCrateCodecError {
    /// A `TypeRef` string failed to parse as a valid Rust type expression.
    ///
    /// Contains the offending `TypeRef` string and a description of the parse
    /// failure.
    #[error("invalid TypeRef `{type_ref}`: {reason}")]
    InvalidTypeRef {
        /// The raw `TypeRef` string that failed parsing.
        type_ref: String,
        /// Human-readable description of the parse failure.
        reason: String,
    },

    /// Two catalogue entries share the same short type name within the same
    /// `CatalogueDocument`, causing an ambiguous `Id` assignment.
    ///
    /// Contains the conflicting short name.
    #[error("ambiguous identifier `{name}`: short name collision in catalogue")]
    AmbiguousIdentifier {
        /// The short name that collides.
        name: String,
    },
}

impl From<CatalogueToExtendedCrateCodecError> for NewTypeGraphCodecError {
    fn from(err: CatalogueToExtendedCrateCodecError) -> Self {
        match err {
            CatalogueToExtendedCrateCodecError::InvalidTypeRef { type_ref, reason } => {
                NewTypeGraphCodecError::InvalidTypeRef(format!("{type_ref}: {reason}"))
            }
            CatalogueToExtendedCrateCodecError::AmbiguousIdentifier { name } => {
                NewTypeGraphCodecError::AmbiguousTypeName(name)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_invalid_type_ref_display() {
        let err = CatalogueToExtendedCrateCodecError::InvalidTypeRef {
            type_ref: "Result<Option<User>, DomainError>".to_string(),
            reason: "unexpected token".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("Result<Option<User>, DomainError>"), "message: {msg}");
        assert!(msg.contains("unexpected token"), "message: {msg}");
    }

    #[test]
    fn test_ambiguous_identifier_display() {
        let err =
            CatalogueToExtendedCrateCodecError::AmbiguousIdentifier { name: "UserId".to_string() };
        let msg = err.to_string();
        assert!(msg.contains("UserId"), "message: {msg}");
        assert!(msg.contains("collision"), "message: {msg}");
    }

    #[test]
    fn test_from_invalid_type_ref_into_domain_error() {
        let codec_err = CatalogueToExtendedCrateCodecError::InvalidTypeRef {
            type_ref: "BadType<>".to_string(),
            reason: "empty generic args".to_string(),
        };
        let domain_err: NewTypeGraphCodecError = codec_err.into();
        assert!(
            matches!(domain_err, NewTypeGraphCodecError::InvalidTypeRef(_)),
            "expected InvalidTypeRef, got: {domain_err:?}"
        );
    }

    #[test]
    fn test_from_ambiguous_identifier_into_domain_error() {
        let codec_err =
            CatalogueToExtendedCrateCodecError::AmbiguousIdentifier { name: "Draft".to_string() };
        let domain_err: NewTypeGraphCodecError = codec_err.into();
        assert!(
            matches!(domain_err, NewTypeGraphCodecError::AmbiguousTypeName(_)),
            "expected AmbiguousTypeName, got: {domain_err:?}"
        );
    }
}
