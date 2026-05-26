//! Top-level catalogue document and its validation error for the catalogue v2 schema.
//!
//! Implements:
//! - [`CatalogueDocument`]: the top-level document schema (`schema_version` / `crate_name` /
//!   `layer` / `types` / `traits` / `functions` BTreeMaps). ADR 1 D1 / D6.
//! - [`CatalogueDocumentError`]: validation error for `CatalogueDocument` construction.
//!
//! ## Validation
//!
//! `CatalogueDocument` exposes a `validate_filename` method that verifies the
//! `crate_name` field matches the expected filename pattern `<crate_name>-types.json`.
//! Full validation (including cross-field consistency) is performed at document
//! construction time by the infrastructure codec (T003). Domain-layer validation
//! is limited to the `crate_name vs filename` invariant because domain types must not
//! perform I/O (ADR 1 D6).
//!
//! ## Language × Entry-type constraint
//!
//! The three-BTreeMap structure enforces the Language axis at schema level (ADR 1 D1 / CN-01):
//! - `types: BTreeMap<TypeName, TypeEntry>` — Language = DataType
//! - `traits: BTreeMap<TraitName, TraitEntry>` — Language = Contract
//! - `functions: BTreeMap<FunctionPath, FunctionEntry>` — Language = Function
//!
//! No serde derives — per ADR `knowledge/adr/2026-04-14-1531-domain-serde-ripout.md`,
//! the domain layer is serialization-free. The infrastructure codec (T003) handles JSON.

use std::collections::BTreeMap;

use crate::tddd::catalogue_v2::entries::{
    FunctionEntry, InherentImplDeclV2, TraitEntry, TypeEntry,
};
use crate::tddd::catalogue_v2::identifiers::{CrateName, FunctionPath, TraitName, TypeName};
use crate::tddd::catalogue_v2::traits::TraitImplDeclV2;
use crate::tddd::layer_id::LayerId;

// ---------------------------------------------------------------------------
// CatalogueDocumentError — validation error
// ---------------------------------------------------------------------------

/// Error for `CatalogueDocument` validation (ADR 1 D6).
///
/// Returned by validation methods on `CatalogueDocument`. The infrastructure codec
/// (T003) maps these to JSON deserialization errors.
///
/// Variants:
/// - `CrateNameMismatch`: the `crate_name` field does not match the file's stem.
/// - `DuplicateTypeName`: duplicate key in the `types` BTreeMap.
/// - `DuplicateTraitName`: duplicate key in the `traits` BTreeMap.
/// - `DuplicateFunctionPath`: duplicate key in the `functions` BTreeMap.
/// - `InvalidIdentifier`: one or more identifier values failed validation.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CatalogueDocumentError {
    /// The `crate_name` field does not match the filename stem `<crate_name>-types.json`.
    #[error(
        "crate_name field does not match the filename: \
         expected '<crate_name>-types.json' pattern"
    )]
    CrateNameMismatch,

    /// Duplicate key detected in the `types` BTreeMap.
    ///
    /// Note: `BTreeMap` prevents duplicate keys structurally, so this error surfaces
    /// only when the infrastructure codec encounters duplicate keys during JSON parsing.
    #[error("duplicate type name in CatalogueDocument::types")]
    DuplicateTypeName,

    /// Duplicate key detected in the `traits` BTreeMap.
    #[error("duplicate trait name in CatalogueDocument::traits")]
    DuplicateTraitName,

    /// Duplicate key detected in the `functions` BTreeMap.
    #[error("duplicate function path in CatalogueDocument::functions")]
    DuplicateFunctionPath,

    /// An identifier value failed validation (e.g. empty or invalid characters).
    #[error("invalid identifier value in CatalogueDocument")]
    InvalidIdentifier,
}

// ---------------------------------------------------------------------------
// CatalogueDocument — top-level catalogue document
// ---------------------------------------------------------------------------

/// Top-level catalogue document schema.
///
/// Represents one catalogue file `<crate_name>-types.json`, which corresponds to
/// exactly one crate in one architectural layer (ADR 1 D1 / D6 / CN-01).
///
/// ## Field descriptions
///
/// - `schema_version`: format version for forward-compatibility detection.
/// - `crate_name`: the Rust crate this catalogue describes.
/// - `layer`: the architectural layer identifier (e.g. `"domain"`, `"usecase"`,
///   `"infrastructure"`) as a validated [`LayerId`].
/// - `types`: BTreeMap of type name → `TypeEntry` (Language = DataType).
/// - `traits`: BTreeMap of trait name → `TraitEntry` (Language = Contract).
/// - `functions`: BTreeMap of function path → `FunctionEntry` (Language = Function).
/// - `inherent_impls`: Vec of `InherentImplDeclV2` (inherent impl block declarations).
///   Multiple entries with the same `type_name` represent multiple impl blocks for
///   one struct. Default empty Vec for backward compatibility.
/// - `trait_impls`: Vec of `TraitImplDeclV2` (trait impl block declarations, top-level).
///   ADR `2026-05-20-0048` D1: each entry is an independent `impl Trait for Type` block
///   with `trait_ref` + `for_type` fields. Both self-crate and external-crate types are
///   supported symmetrically. Default empty Vec.
///
/// ## Validation
///
/// Use [`CatalogueDocument::validate_filename`] to verify that the `crate_name`
/// field matches the filename stem (domain-layer concern only; I/O is handled by
/// the infrastructure codec in T003).
///
/// The infrastructure codec performs additional validation during JSON deserialization:
/// - duplicate key detection (maps reject duplicates in BTreeMap)
/// - identifier format validation (via `FromStr` / `TryFrom`)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogueDocument {
    /// Format version; currently `3` for the v2 schema.
    pub schema_version: u32,
    /// The crate this catalogue describes.
    pub crate_name: CrateName,
    /// The architectural layer identifier (e.g. `"domain"`, `"usecase"`, `"infrastructure"`).
    pub layer: LayerId,
    /// Type entries (struct / enum / type alias declarations).
    pub types: BTreeMap<TypeName, TypeEntry>,
    /// Trait entries (trait declarations).
    pub traits: BTreeMap<TraitName, TraitEntry>,
    /// Function entries (free function declarations).
    pub functions: BTreeMap<FunctionPath, FunctionEntry>,
    /// Inherent impl block declarations (one entry per impl block).
    ///
    /// Multiple entries sharing the same `type_name` represent multiple inherent
    /// `impl` blocks for one struct. Default empty Vec — catalogues predating
    /// this field decode as if they had no inherent impl blocks.
    pub inherent_impls: Vec<InherentImplDeclV2>,

    /// Trait impl block declarations (top-level, ADR `2026-05-20-0048` D1).
    ///
    /// Each entry represents one `impl Trait for Type` block. Unlike the old design
    /// where these were embedded in `TypeEntry::trait_impls`, top-level placement
    /// enables declaring impls for external-crate self types (Case B orphan impls).
    ///
    /// Default empty Vec — catalogues predating this field decode without trait impl blocks.
    pub trait_impls: Vec<TraitImplDeclV2>,
}

impl CatalogueDocument {
    /// Creates a new empty `CatalogueDocument`.
    #[must_use]
    pub fn new(schema_version: u32, crate_name: CrateName, layer: LayerId) -> Self {
        Self {
            schema_version,
            crate_name,
            layer,
            types: BTreeMap::new(),
            traits: BTreeMap::new(),
            functions: BTreeMap::new(),
            inherent_impls: Vec::new(),
            trait_impls: Vec::new(),
        }
    }

    /// Validates that `crate_name` matches the given `filename_stem`.
    ///
    /// The expected filename pattern is `<crate_name>-types.json`; the `filename_stem`
    /// argument should be the portion before `-types.json` (e.g. `"domain"` for the
    /// file `domain-types.json`).
    ///
    /// # Errors
    ///
    /// Returns `CatalogueDocumentError::CrateNameMismatch` when the `crate_name` field
    /// does not equal `filename_stem`.
    pub fn validate_filename(&self, filename_stem: &str) -> Result<(), CatalogueDocumentError> {
        if self.crate_name.as_str() == filename_stem {
            Ok(())
        } else {
            Err(CatalogueDocumentError::CrateNameMismatch)
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
    use crate::tddd::catalogue_v2::entries::TypeEntry;
    use crate::tddd::catalogue_v2::identifiers::{FunctionName, ModulePath, TypeRef};
    use crate::tddd::catalogue_v2::roles::{DataRole, FunctionRole, ItemAction};
    use crate::tddd::layer_id::LayerId;

    fn layer_domain() -> LayerId {
        LayerId::try_new("domain").unwrap()
    }

    fn layer_usecase() -> LayerId {
        LayerId::try_new("usecase").unwrap()
    }

    fn layer_infrastructure() -> LayerId {
        LayerId::try_new("infrastructure").unwrap()
    }

    fn make_simple_type_entry() -> TypeEntry {
        TypeEntry {
            action: ItemAction::Add,
            role: DataRole::ValueObject,
            kind: TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
            methods: vec![],
            module_path: ModulePath::root(),
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        }
    }

    fn make_simple_function_entry() -> FunctionEntry {
        FunctionEntry {
            action: ItemAction::Add,
            role: FunctionRole::FreeFunction,
            params: vec![],
            returns: TypeRef::new("()").unwrap(),
            is_async: false,
            generics: vec![],
            where_predicates: vec![],
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        }
    }

    // -----------------------------------------------------------------------
    // CatalogueDocument construction
    // -----------------------------------------------------------------------

    #[test]
    fn test_catalogue_document_new_creates_empty_document() {
        let crate_name = CrateName::new("domain").unwrap();
        let doc = CatalogueDocument::new(2, crate_name.clone(), layer_domain());
        assert_eq!(doc.schema_version, 2);
        assert_eq!(doc.crate_name, crate_name);
        assert_eq!(doc.layer, layer_domain());
        assert!(doc.types.is_empty());
        assert!(doc.traits.is_empty());
        assert!(doc.functions.is_empty());
    }

    #[test]
    fn test_catalogue_document_with_usecase_layer() {
        let crate_name = CrateName::new("usecase").unwrap();
        let doc = CatalogueDocument::new(2, crate_name.clone(), layer_usecase());
        assert_eq!(doc.layer, layer_usecase());
    }

    #[test]
    fn test_catalogue_document_with_infrastructure_layer() {
        let crate_name = CrateName::new("infrastructure").unwrap();
        let doc = CatalogueDocument::new(2, crate_name.clone(), layer_infrastructure());
        assert_eq!(doc.layer, layer_infrastructure());
    }

    // -----------------------------------------------------------------------
    // BTreeMap insertion and access
    // -----------------------------------------------------------------------

    #[test]
    fn test_catalogue_document_types_btreemap_stores_type_entry() {
        let crate_name = CrateName::new("domain").unwrap();
        let mut doc = CatalogueDocument::new(2, crate_name, layer_domain());
        let type_name = TypeName::new("UserId").unwrap();
        doc.types.insert(type_name.clone(), make_simple_type_entry());
        assert_eq!(doc.types.len(), 1);
        assert!(doc.types.contains_key(&type_name));
    }

    #[test]
    fn test_catalogue_document_functions_btreemap_stores_function_entry() {
        let crate_name = CrateName::new("domain").unwrap();
        let mut doc = CatalogueDocument::new(2, crate_name.clone(), layer_domain());
        let fn_path =
            FunctionPath::at_root(crate_name.clone(), FunctionName::new("register_user").unwrap());
        doc.functions.insert(fn_path.clone(), make_simple_function_entry());
        assert_eq!(doc.functions.len(), 1);
        assert!(doc.functions.contains_key(&fn_path));
    }

    #[test]
    fn test_catalogue_document_btreemaps_use_deterministic_order() {
        // BTreeMap ensures sorted key iteration — verify type key order.
        let crate_name = CrateName::new("domain").unwrap();
        let mut doc = CatalogueDocument::new(2, crate_name, layer_domain());
        doc.types.insert(TypeName::new("ZOrder").unwrap(), make_simple_type_entry());
        doc.types.insert(TypeName::new("AUser").unwrap(), make_simple_type_entry());
        doc.types.insert(TypeName::new("MItem").unwrap(), make_simple_type_entry());
        let keys: Vec<_> = doc.types.keys().map(|k| k.as_str()).collect();
        assert_eq!(keys, vec!["AUser", "MItem", "ZOrder"]);
    }

    // -----------------------------------------------------------------------
    // validate_filename
    // -----------------------------------------------------------------------

    #[test]
    fn test_validate_filename_with_matching_stem_returns_ok() {
        let crate_name = CrateName::new("domain").unwrap();
        let doc = CatalogueDocument::new(2, crate_name, layer_domain());
        // "domain-types.json" → stem = "domain"
        assert!(doc.validate_filename("domain").is_ok());
    }

    #[test]
    fn test_validate_filename_with_mismatched_stem_returns_crate_name_mismatch_error() {
        let crate_name = CrateName::new("domain").unwrap();
        let doc = CatalogueDocument::new(2, crate_name, layer_domain());
        // "usecase-types.json" stem "usecase" does not match crate_name "domain"
        let err = doc.validate_filename("usecase").unwrap_err();
        assert_eq!(err, CatalogueDocumentError::CrateNameMismatch);
    }

    #[test]
    fn test_validate_filename_with_empty_stem_returns_crate_name_mismatch_error() {
        let crate_name = CrateName::new("domain").unwrap();
        let doc = CatalogueDocument::new(2, crate_name, layer_domain());
        let err = doc.validate_filename("").unwrap_err();
        assert_eq!(err, CatalogueDocumentError::CrateNameMismatch);
    }

    #[test]
    fn test_validate_filename_with_underscore_crate_name_succeeds() {
        let crate_name = CrateName::new("domain_core").unwrap();
        let doc = CatalogueDocument::new(2, crate_name, layer_domain());
        // "domain_core-types.json" → stem = "domain_core"
        assert!(doc.validate_filename("domain_core").is_ok());
    }

    #[test]
    fn test_validate_filename_with_full_filename_returns_mismatch() {
        // The caller should pass only the stem, not the full filename.
        let crate_name = CrateName::new("domain").unwrap();
        let doc = CatalogueDocument::new(2, crate_name, layer_domain());
        // Passing the full filename "domain-types.json" should fail (not equal to "domain").
        let err = doc.validate_filename("domain-types.json").unwrap_err();
        assert_eq!(err, CatalogueDocumentError::CrateNameMismatch);
    }

    // -----------------------------------------------------------------------
    // CatalogueDocumentError Display
    // -----------------------------------------------------------------------

    #[test]
    fn test_catalogue_document_error_crate_name_mismatch_display() {
        let err = CatalogueDocumentError::CrateNameMismatch;
        let msg = err.to_string();
        assert!(!msg.is_empty());
    }

    #[test]
    fn test_catalogue_document_error_duplicate_type_name_display() {
        let err = CatalogueDocumentError::DuplicateTypeName;
        let msg = err.to_string();
        assert!(msg.contains("types"));
    }

    #[test]
    fn test_catalogue_document_error_duplicate_trait_name_display() {
        let err = CatalogueDocumentError::DuplicateTraitName;
        let msg = err.to_string();
        assert!(msg.contains("traits"));
    }

    #[test]
    fn test_catalogue_document_error_duplicate_function_path_display() {
        let err = CatalogueDocumentError::DuplicateFunctionPath;
        let msg = err.to_string();
        assert!(msg.contains("functions"));
    }

    #[test]
    fn test_catalogue_document_error_invalid_identifier_display() {
        let err = CatalogueDocumentError::InvalidIdentifier;
        let msg = err.to_string();
        assert!(!msg.is_empty());
    }

    // -----------------------------------------------------------------------
    // 1-crate-per-file invariant documentation
    // -----------------------------------------------------------------------

    // -----------------------------------------------------------------------
    // CatalogueDocument.trait_impls (ADR 0048 D1)
    // -----------------------------------------------------------------------

    #[test]
    fn test_catalogue_document_trait_impls_defaults_to_empty() {
        // AC-13: CatalogueDocument must have top-level trait_impls defaulting to empty.
        use crate::tddd::catalogue_v2::identifiers::CrateName;
        let crate_name = CrateName::new("domain").unwrap();
        let layer = LayerId::try_new("domain").unwrap();
        let doc = CatalogueDocument::new(3, crate_name, layer);
        assert!(doc.trait_impls.is_empty());
    }

    #[test]
    fn test_catalogue_document_trait_impls_stores_entries() {
        use crate::tddd::catalogue_v2::identifiers::{CrateName, TypeRef};
        use crate::tddd::catalogue_v2::traits::TraitImplDeclV2;

        let crate_name = CrateName::new("domain").unwrap();
        let layer = LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer);

        // Case A: external trait + self-crate type
        let impl_a = TraitImplDeclV2::new(
            TypeRef::new("std::fmt::Display").unwrap(),
            TypeRef::new("SelfType").unwrap(),
        );
        // Case B: self-crate trait + external type
        let impl_b = TraitImplDeclV2::new(
            TypeRef::new("MyTrait").unwrap(),
            TypeRef::new("std::vec::Vec<i32>").unwrap(),
        );

        doc.trait_impls.push(impl_a.clone());
        doc.trait_impls.push(impl_b.clone());

        assert_eq!(doc.trait_impls.len(), 2);
        assert_eq!(doc.trait_impls[0], impl_a);
        assert_eq!(doc.trait_impls[1], impl_b);
    }

    #[test]
    fn test_catalogue_document_one_crate_per_layer_structural_invariant() {
        // A CatalogueDocument always has exactly one crate_name and one layer.
        // This is a structural property — there is no way to have multiple crates
        // in one document at the type level (ADR 1 D6).
        let crate_name = CrateName::new("domain").unwrap();
        let doc = CatalogueDocument::new(2, crate_name.clone(), layer_domain());
        assert_eq!(doc.crate_name, crate_name);
        // No "other crate name" field — the invariant is structural.
        let _ = &doc.layer;
    }
}
