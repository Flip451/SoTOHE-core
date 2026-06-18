//! Catalogue v2 schema types for TDDD v2 framework rewrite.
//!
//! This module implements the new catalogue schema types as specified in
//! ADR `knowledge/adr/2026-05-08-0248-tddd-catalogue-layer-schema-axis-separation.md`.
//!
//! ## Modules
//!
//! - [`identifiers`]: 13 newtype wrappers with `Display` / `FromStr` / validation.
//! - [`roles`]: role enums (`DataRole`, `ContractRole`, `FunctionRole`, `ItemAction`,
//!   `SelfReceiver`) and role payload value objects (`InvariantPredicate`,
//!   `InvariantDecl`, `IdentityAccessor`, `NonEmptyVec`).
//!   The Layer axis is represented by [`crate::tddd::LayerId`] (ADR `2026-05-08-0248` D1).
//! - [`composite`]: `TypeKindV2`, `StructKind`, `StructShape`, `TypestateMarker`, and `TypestateTransitions`.
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

pub mod catalogue_impl_signals_ports;
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

pub use composite::{StructKind, StructShape, TypeKindV2, TypestateMarker, TypestateTransitions};

pub use document::{CatalogueDocument, CatalogueDocumentError};

pub use entries::{
    AssocConstDecl, AssocTypeDecl, FunctionEntry, InherentImplDeclV2, TraitEntry, TypeEntry,
};

pub use identifiers::{
    AssocConstName, CrateName, FieldName, FunctionName, FunctionPath, Identifier, IdentifierError,
    InvariantName, MethodName, ModulePath, ParamName, TraitName, TypeName, TypeRef, VariantName,
};

pub use methods::{
    BoundOp, MethodDeclaration, MethodGenericParam, ParamDeclaration, WherePredicateDecl,
};

pub use roles::{
    ConstructionError, ContractRole, DataRole, FunctionRole, IdentityAccessor, InvariantDecl,
    InvariantPredicate, ItemAction, NonEmptyVec, SelfReceiver,
};

pub use traits::TraitImplDeclV2;

pub use variants::{FieldDecl, VariantDecl, VariantPayload};

pub use catalogue_impl_signals_ports::{
    BaselineCaptureIoError, CatalogueDocumentLoaderError, CatalogueDocumentLoaderPort,
    RustdocBaselineCapturePort, RustdocCratePort, RustdocCratePortError, TdddLayerBinding,
    TdddLayerBindingsError, TdddLayerBindingsPort, TrackStatusReadError, TrackStatusReaderPort,
    TypeSignalsExecutionError, TypeSignalsExecutorPort,
};
