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
//!
//! ## Design notes
//!
//! **No serde derives** — per ADR `knowledge/adr/2026-04-14-1531-domain-serde-ripout.md`,
//! the domain layer is serialization-free. The infrastructure codec layer handles all
//! JSON serialization and deserialization.
//!
//! **Additive only (T001)** — existing legacy types (`TypeDefinitionKind`, etc.)
//! in `super::catalogue` are left untouched until T008.

pub mod identifiers;
pub mod roles;

// Re-export all public types for convenient access via the module root.

pub use identifiers::{
    CrateName, FieldName, FunctionName, FunctionPath, Identifier, IdentifierError, MethodName,
    ModulePath, ParamName, TraitName, TypeName, TypeRef, VariantName,
};

pub use roles::{ContractRole, DataRole, FunctionRole, ItemAction, Layer, SelfReceiver};
