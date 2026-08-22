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
//! * `AmbiguousIdentifier` — a short identifier resolves to multiple catalogue
//!   entries, so the codec reports every fully qualified candidate.
//! * `UnresolvedIdentifier` — a catalogue reference has no matching declaration.
//!
//! Crate-prefixed `TypeRef` values (e.g. `"domain_core::UserId"`) are **never**
//! rejected as codec errors; they are auto-collected into `external_crates`
//! (ADR 2 D5 / D11).

use thiserror::Error;

use crate::tddd::catalogue_v2::identifiers::{FullyQualifiedItemPath, Identifier, TypeRef};
use crate::tddd::catalogue_v2::roles::NonEmptyVec;
use crate::tddd::test_obligation::ids::DiagnosticMessage;

/// Error returned by [`crate::tddd::CatalogueToExtendedCratePort::encode`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum NewTypeGraphCodecError {
    /// A `TypeRef` string failed to parse as a valid Rust type expression.
    ///
    /// Contains a human-readable description of the parse failure and the
    /// offending `TypeRef` string.
    #[error("invalid TypeRef `{type_ref}`: {diagnostic}", type_ref = .0.as_str(), diagnostic = .1.as_str())]
    InvalidTypeRef(TypeRef, DiagnosticMessage),

    /// Two catalogue entries share the same short type name, causing an
    /// ambiguous `Id` assignment.
    ///
    /// Contains the conflicting short name.
    #[error("ambiguous identifier `{identifier}`; candidates: {candidates:?}", identifier = .0.as_str(), candidates = .1.as_slice())]
    AmbiguousIdentifier(Identifier, NonEmptyVec<FullyQualifiedItemPath>),

    /// A local identifier could not be resolved to a catalogue declaration.
    #[error("unresolved identifier `{identifier}`", identifier = .0.as_str())]
    UnresolvedIdentifier(TypeRef),
}
