//! Error type for the Catalogue → ExtendedCrate (TypeGraph A) codec.
//!
//! `NewTypeGraphCodecError` is the domain-layer error returned when the
//! `CatalogueToExtendedCratePort` fails to convert a `CatalogueDocument` into
//! an `ExtendedCrate`.
//!
//! ## Variants (ADR 2 D9)
//!
//! * `InvalidTypeRef` — a `TypeRef` string could not be parsed by `syn` into a
//!   valid Rust type expression.
//! * `AmbiguousTypeName` — a short type name (used as a `BTreeMap` key in the
//!   catalogue) collides with another entry within the same catalogue, making
//!   `Id` assignment ambiguous.
//!
//! Crate-prefixed `TypeRef` values (e.g. `"domain_core::UserId"`) are **never**
//! rejected as codec errors; they are auto-collected into `external_crates`
//! (ADR 2 D5 / D11).

use thiserror::Error;

/// Error returned by [`crate::tddd::CatalogueToExtendedCratePort::encode`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum NewTypeGraphCodecError {
    /// A `TypeRef` string failed to parse as a valid Rust type expression.
    ///
    /// Contains a human-readable description of the parse failure and the
    /// offending `TypeRef` string.
    #[error("invalid TypeRef: {0}")]
    InvalidTypeRef(String),

    /// Two catalogue entries share the same short type name, causing an
    /// ambiguous `Id` assignment.
    ///
    /// Contains the conflicting short name.
    #[error("ambiguous type name: {0}")]
    AmbiguousTypeName(String),
}
