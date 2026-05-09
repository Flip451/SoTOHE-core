//! `CatalogueToExtendedCratePort` — secondary port for Catalogue → ExtendedCrate.
//!
//! This port is declared in the domain layer and implemented in the
//! infrastructure layer by `CatalogueToExtendedCrateCodec` (T005).
//!
//! ## Contract (ADR 2 D8)
//!
//! Converts a `CatalogueDocument` into an `ExtendedCrate` (TypeGraph A).
//! The codec performs:
//! 1. inline → id-reference conversion (`FieldDecl` / `VariantDecl` → separate
//!    `index` items, parent references via `Vec<Id>`).
//! 2. 1 type = 1 Inherent Impl block: all inherent methods are collected into a
//!    single `Impl` block per type.
//! 3. `TypeRef` generics parse via `syn` crate, mapping each identifier to a
//!    `rustdoc_types::Type` variant.
//! 4. `external_crates` auto-build from `TraitImplDeclV2::origin_crate`,
//!    `TypeRef` crate prefixes, and the std prelude allowlist (ADR 2 D5).

use crate::tddd::catalogue_v2::CatalogueDocument;
use crate::tddd::extended_crate::ExtendedCrate;
use crate::tddd::new_typegraph_codec_error::NewTypeGraphCodecError;

/// Secondary port: converts a `CatalogueDocument` into an `ExtendedCrate`.
///
/// Implementors live in the infrastructure layer (see `CatalogueToExtendedCrateCodec`).
/// The domain layer declares only this trait; it does not know about `serde`,
/// file I/O, or `syn` parsing details.
///
/// # Errors
///
/// Returns `NewTypeGraphCodecError` if the `CatalogueDocument` contains a
/// `TypeRef` that cannot be parsed as a valid Rust type, or if two catalogue
/// entries share the same short type name within the same document.
pub trait CatalogueToExtendedCratePort: Send + Sync {
    /// Encodes a `CatalogueDocument` into an `ExtendedCrate` (TypeGraph A).
    ///
    /// # Errors
    ///
    /// Returns `Err(NewTypeGraphCodecError::InvalidTypeRef)` when a `TypeRef`
    /// string in `doc` fails `syn` parsing.
    ///
    /// Returns `Err(NewTypeGraphCodecError::AmbiguousTypeName)` when two types
    /// in `doc.types` share the same short name (would cause colliding
    /// `rustdoc_types::Id` keys).
    fn encode(&self, doc: CatalogueDocument) -> Result<ExtendedCrate, NewTypeGraphCodecError>;
}
