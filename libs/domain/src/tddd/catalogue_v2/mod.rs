//! Catalogue v2 schema types for TDDD v2 framework rewrite.
//!
//! This module implements the new catalogue schema types as specified in
//! ADR `knowledge/adr/2026-05-08-0248-tddd-catalogue-layer-schema-axis-separation.md`.
//!
//! ## Modules
//!
//! - [`identifiers`]: 12 newtype wrappers with `Display` / `FromStr` / validation.
//! - [`roles`]: 6 enums (`DataRole`, `ContractRole`, `FunctionRole`, `ItemAction`,
//!   `SelfReceiver`, `Layer`) with `Display` / `FromStr` via strum.
//! - [`composite`]: `TypeKindV2` and `CompositePattern` (pattern-encoded struct kinds).
//! - [`variants`]: `FieldDecl`, `VariantPayload`, `VariantDecl`.
//! - [`methods`]: `ParamDecl`, `MethodDecl` (V2 typed-newtype method/param declarations).
//! - [`traits`]: `TraitImplDeclV2` (identity-only trait impl record).
//! - [`entries`]: `TypeEntry`, `TraitEntry`, `FunctionEntry` (BTreeMap values).
//! - [`document`]: `CatalogueDocument`, `CatalogueDocumentError` (top-level document + validation).
//!
//! ## Design notes
//!
//! **No serde derives** — per ADR `knowledge/adr/2026-04-14-1531-domain-serde-ripout.md`,
//! the domain layer is serialization-free. The infrastructure codec layer handles all
//! JSON serialization and deserialization.
//!
//! **Additive only (T001 / T002)** — existing legacy types (`TypeDefinitionKind`, etc.)
//! in `super::catalogue` are left untouched until T008. The V2 types live exclusively
//! in this `catalogue_v2` module hierarchy.

pub mod composite;
pub mod document;
pub mod entries;
pub mod identifiers;
pub mod methods;
pub mod roles;
pub mod traits;
pub mod variants;

// ---------------------------------------------------------------------------
// Re-exports — all public types accessible via the module root
// ---------------------------------------------------------------------------

pub use composite::{CompositePattern, TypeKindV2};

pub use document::{CatalogueDocument, CatalogueDocumentError};

pub use entries::{FunctionEntry, TraitEntry, TypeEntry};

pub use identifiers::{
    CrateName, FieldName, FunctionName, FunctionPath, Identifier, IdentifierError, MethodName,
    ModulePath, ParamName, TraitName, TypeName, TypeRef, VariantName,
};

pub use methods::{MethodDeclaration, ParamDeclaration};

pub use roles::{ContractRole, DataRole, FunctionRole, ItemAction, Layer, SelfReceiver};

pub use traits::TraitImplDeclV2;

pub use variants::{FieldDecl, VariantDecl, VariantPayload};
