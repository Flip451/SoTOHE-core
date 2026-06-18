//! JSON codec for [`CatalogueDocument`] (schema_version = 5).
//!
//! `CatalogueDocumentCodec` converts between `CatalogueDocument` domain types
//! and JSON using serde DTO structs defined here in the infrastructure layer.
//! The domain layer is serialization-free (ADR `2026-04-14-1531-domain-serde-ripout.md`).
//!
//! ## Wire format (schema_version = 5)
//!
//! ```json
//! {
//!   "schema_version": 5,
//!   "crate_name": "domain",
//!   "layer": "domain",
//!   "types": { "TypeName": { ... } },
//!   "traits": { "TraitName": { ... } },
//!   "functions": { "crate::fn_name": { ... } }
//! }
//! ```
//!
//! `CatalogueDocument.schema_version` is always `5` (bumped from 4 to 5 in
//! tddd-pattern-semantics-extension: `TypeEntry.role` wire format changed from a bare
//! string to a discriminated-object; schema-version 4 catalogues require migration and
//! are rejected with `SchemaVersionRequiresMigration`). Older v1/v2/v3 formats are
//! rejected by this codec as `UnsupportedSchemaVersion`.
//!
//! ## Error variants
//!
//! - `Json(serde_json::Error)` — serde deserialization failed.
//! - `Io(std::io::Error)` — file I/O failed.
//! - `SchemaVersionRequiresMigration { from, to, reason }` — catalogue uses an older
//!   schema that has a known breaking format change and must be migrated before use.
//! - `UnsupportedSchemaVersion { actual, expected }` — version is neither the current
//!   supported version nor a known migration-gated predecessor.
//! - `InvalidEntry { entry_name, reason }` — an entry's fields failed validation.
//! - `CrateNameMismatch { expected, actual }` — `crate_name` field vs filename stem.

use std::collections::BTreeMap;
use std::path::Path;

use domain::tddd::catalogue_v2::CatalogueDocument;
use serde::{Deserialize, de};
use thiserror::Error;

mod decode;
mod decode_assoc;
mod decode_impls;
mod decode_roles;
mod dto;
mod dto_roles;
mod encode;
mod validate;

use decode::dto_to_domain;
use dto::SchemaVersionProbe;
use encode::domain_to_dto;

// ---------------------------------------------------------------------------
// Supported schema version
// ---------------------------------------------------------------------------

/// The schema version this codec reads and writes.
///
/// Bumped from 4 to 5 in tddd-pattern-semantics-extension: `TypeEntry.role` wire format
/// changed from a bare string to a discriminated-object (ADR D3 Stage 1 breaking change).
/// Schema-version 4 catalogues are rejected with `SchemaVersionRequiresMigration` (fail-closed).
/// Schema-version 3 and below are rejected with `UnsupportedSchemaVersion` (no migration path).
pub const SCHEMA_VERSION: u32 = 5;

// ---------------------------------------------------------------------------
// StrictMap — duplicate-key-rejecting BTreeMap deserializer
// ---------------------------------------------------------------------------

/// A thin newtype over `BTreeMap<K, V>` that rejects duplicate keys during
/// JSON deserialization instead of silently applying last-wins semantics.
///
/// Used for the `types`, `traits`, and `functions` maps in
/// [`dto::CatalogueDocumentDto`] so that a tampered catalogue containing duplicate
/// keys fails closed rather than silently dropping entries.
struct StrictMap<K: Ord, V>(BTreeMap<K, V>);

impl<'de, K, V> Deserialize<'de> for StrictMap<K, V>
where
    K: Deserialize<'de> + Ord + std::fmt::Debug,
    V: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct StrictMapVisitor<K, V>(std::marker::PhantomData<(K, V)>);

        impl<'de, K, V> de::Visitor<'de> for StrictMapVisitor<K, V>
        where
            K: Deserialize<'de> + Ord + std::fmt::Debug,
            V: Deserialize<'de>,
        {
            type Value = StrictMap<K, V>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a map with unique keys")
            }

            fn visit_map<A>(self, mut access: A) -> Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de>,
            {
                let mut map = BTreeMap::new();
                while let Some((key, value)) = access.next_entry::<K, V>()? {
                    if map.insert(key, value).is_some() {
                        return Err(de::Error::custom("duplicate key in catalogue map"));
                    }
                }
                Ok(StrictMap(map))
            }
        }

        deserializer.deserialize_map(StrictMapVisitor(std::marker::PhantomData))
    }
}

// ---------------------------------------------------------------------------
// CatalogueDocumentCodecError
// ---------------------------------------------------------------------------

/// Error type for [`CatalogueDocumentCodec`] operations.
#[derive(Debug, Error)]
pub enum CatalogueDocumentCodecError {
    /// `serde_json` failed to deserialize the JSON content.
    #[error("JSON deserialization error: {0}")]
    Json(#[from] serde_json::Error),

    /// Filesystem I/O error (file not found, permission denied, …).
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// The catalogue uses a known older schema version that has a breaking format change.
    ///
    /// The file must be migrated (re-generated from source or manually updated) before use.
    /// This is a distinct variant from [`Self::UnsupportedSchemaVersion`] so callers can
    /// provide actionable migration guidance rather than a generic version mismatch message.
    #[error("catalogue schema_version={from} requires migration to schema_version={to}: {reason}")]
    SchemaVersionRequiresMigration {
        /// The schema version found in the JSON file.
        from: u32,
        /// The schema version this codec expects.
        to: u32,
        /// Human-readable description of the breaking change that requires migration.
        reason: &'static str,
    },

    /// The JSON file's `schema_version` does not match [`SCHEMA_VERSION`] and there is no
    /// known migration path (e.g. the version is from the future or a completely unknown format).
    #[error(
        "unsupported catalogue schema_version: file has {actual}, codec expects {expected}. \
         Migrate the catalogue file to schema_version={expected}."
    )]
    UnsupportedSchemaVersion {
        /// Version number found in the JSON file.
        actual: u32,
        /// Version number expected by this codec.
        expected: u32,
    },

    /// An entry field failed identifier or type-reference validation.
    #[error("invalid entry '{entry_name}': {reason}")]
    InvalidEntry {
        /// The entry name (BTreeMap key) that triggered the error.
        entry_name: String,
        /// Human-readable reason for the failure.
        reason: String,
    },

    /// `crate_name` field does not match the expected filename stem.
    #[error(
        "crate_name mismatch: filename stem is '{expected}' but crate_name field is '{actual}'"
    )]
    CrateNameMismatch {
        /// The filename stem (portion before `-types.json`).
        expected: String,
        /// The `crate_name` field value found in the JSON.
        actual: String,
    },

    /// A function map key does not start with the catalogue's own `crate_name::` prefix.
    ///
    /// Cross-crate function paths are rejected at decode time (D4 constraint).
    #[error(
        "cross-crate function path '{key}': all function paths must start with '{expected_crate}::'"
    )]
    CrossCrateFunctionPath {
        /// The rejected function path key.
        key: String,
        /// The crate prefix that was expected.
        expected_crate: String,
    },
}

// ---------------------------------------------------------------------------
// CatalogueDocumentCodec — stateless codec
// ---------------------------------------------------------------------------

/// Stateless codec that converts between [`CatalogueDocument`] and JSON.
///
/// Construct with [`CatalogueDocumentCodec::new`] or use the static methods
/// [`CatalogueDocumentCodec::decode`] and [`CatalogueDocumentCodec::encode`] directly.
#[derive(Debug, Clone, Default)]
pub struct CatalogueDocumentCodec;

impl CatalogueDocumentCodec {
    /// Creates a new codec instance.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Decode a JSON string into a [`CatalogueDocument`].
    ///
    /// `filename_stem` is the portion of the filename before `-types.json`
    /// (e.g. `"domain"` for `domain-types.json`). It is used to validate the
    /// `crate_name` field.
    ///
    /// # Errors
    ///
    /// Returns `CatalogueDocumentCodecError::Json` if the JSON is malformed.
    ///
    /// Returns `CatalogueDocumentCodecError::SchemaVersionRequiresMigration` if
    /// the `schema_version` field is 4 (the immediately preceding schema, which has a
    /// known breaking wire-format change for `TypeEntry.role`).
    ///
    /// Returns `CatalogueDocumentCodecError::UnsupportedSchemaVersion` if
    /// the `schema_version` field is neither [`SCHEMA_VERSION`] nor a known
    /// migration-gated predecessor.
    ///
    /// Returns `CatalogueDocumentCodecError::CrateNameMismatch` if the
    /// `crate_name` field does not match `filename_stem`.
    ///
    /// Returns `CatalogueDocumentCodecError::InvalidEntry` if any entry's
    /// fields fail validation.
    pub fn decode(
        json: &str,
        filename_stem: &str,
    ) -> Result<CatalogueDocument, CatalogueDocumentCodecError> {
        // Phase 1: check schema_version before full parse.
        let version_probe: SchemaVersionProbe = serde_json::from_str(json)?;
        match version_probe.schema_version {
            v if v == SCHEMA_VERSION => {
                // Supported version — proceed to full parse below.
            }
            4 => {
                // v4 → v5 is a breaking change: TypeEntry.role wire format changed from a bare
                // string to a discriminated-object. Attempting a full parse would produce a
                // JSON type error, which is confusing. Reject with an actionable migration gate.
                return Err(CatalogueDocumentCodecError::SchemaVersionRequiresMigration {
                    from: 4,
                    to: SCHEMA_VERSION,
                    reason: "role wire format changed from string to discriminated-object in v5; \
                             re-generate the catalogue via the type-designer agent, \
                             then run `sotp track type-signals`",
                });
            }
            actual => {
                return Err(CatalogueDocumentCodecError::UnsupportedSchemaVersion {
                    actual,
                    expected: SCHEMA_VERSION,
                });
            }
        }

        // Phase 2: full parse.
        let dto: dto::CatalogueDocumentDto = serde_json::from_str(json)?;

        // Validate crate_name vs filename stem.
        if dto.crate_name != filename_stem {
            return Err(CatalogueDocumentCodecError::CrateNameMismatch {
                expected: filename_stem.to_owned(),
                actual: dto.crate_name,
            });
        }

        // Convert DTO → domain.
        dto_to_domain(dto)
    }

    /// Load and decode a `CatalogueDocument` from a file on disk.
    ///
    /// The `filename_stem` is derived from the file's stem portion before
    /// `-types.json` (e.g. `"domain"` for `domain-types.json`).
    ///
    /// # Errors
    ///
    /// Returns `CatalogueDocumentCodecError::Io` if the file cannot be read.
    ///
    /// See [`CatalogueDocumentCodec::decode`] for other error conditions.
    pub fn load(path: &Path) -> Result<CatalogueDocument, CatalogueDocumentCodecError> {
        let content = std::fs::read_to_string(path)?;
        let filename_stem = derive_filename_stem(path);
        Self::decode(&content, &filename_stem)
    }

    /// Encode a [`CatalogueDocument`] as a pretty-printed JSON string.
    ///
    /// # Errors
    ///
    /// Returns `CatalogueDocumentCodecError::InvalidEntry` if any
    /// `WherePredicateDecl` has an empty `bounds` vec (empty bounds cannot
    /// round-trip through the JSON codec — the decoder rejects them).
    ///
    /// Returns `CatalogueDocumentCodecError::Json` if serialization fails
    /// (this is extremely unlikely for valid domain types, but the error variant
    /// is kept for API completeness).
    pub fn encode(doc: &CatalogueDocument) -> Result<String, CatalogueDocumentCodecError> {
        let dto = domain_to_dto(doc)?;
        let json = serde_json::to_string_pretty(&dto)?;
        Ok(json)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract the filename stem (part before `-types.json`).
///
/// For `domain-types.json`, returns `"domain"`.
/// For files not matching the pattern, returns the full file stem.
pub(crate) fn derive_filename_stem(path: &Path) -> String {
    let stem = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .strip_suffix("-types.json")
        .map(str::to_owned);

    stem.unwrap_or_else(|| path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_owned())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;
    use domain::tddd::LayerId;
    use domain::tddd::catalogue_v2::ItemAction;
    use domain::tddd::catalogue_v2::composite::{StructShape, TypeKindV2};
    use domain::tddd::catalogue_v2::entries::FunctionEntry;
    use domain::tddd::catalogue_v2::identifiers::{CrateName, FunctionName, FunctionPath};
    use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole, FunctionRole};
    use domain::tddd::catalogue_v2::{TypeRef, WherePredicateDecl};

    fn minimal_v5_json(crate_name: &str, layer: &str) -> String {
        format!(
            r#"{{
  "schema_version": 5,
  "crate_name": "{crate_name}",
  "layer": "{layer}",
  "types": {{}},
  "traits": {{}},
  "functions": {{}}
}}"#
        )
    }

    fn trait_method_catalogue_with_generics(generics_json: &str) -> String {
        format!(
            r#"{{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {{}},
  "traits": {{
    "MyPort": {{
      "action": "add",
      "role": {{ "SpecificationPort": {{}} }},
      "methods": [
        {{
          "name": "do_it",
          "returns": "Result",
          "generics": {generics_json}
        }}
      ]
    }}
  }},
  "functions": {{}}
}}"#
        )
    }

    fn mixed_trait_with_default_impl_json() -> &'static str {
        r#"{
  "schema_version": 5,
  "crate_name": "usecase",
  "layer": "usecase",
  "types": {},
  "traits": {
    "MixedTrait": {
      "action": "add",
      "role": { "SpecificationPort": {} },
      "methods": [
        {
          "name": "required",
          "receiver": "&self",
          "params": [],
          "returns": "()",
          "is_async": false
        },
        {
          "name": "provided",
          "receiver": "&self",
          "params": [],
          "returns": "()",
          "is_async": false,
          "has_default_impl": true
        }
      ]
    }
  },
  "functions": {}
}"#
    }

    fn usecase_function_with_where_predicate_json(
        function_name: &str,
        predicate_json: &str,
    ) -> String {
        format!(
            r#"{{
  "schema_version": 5,
  "crate_name": "usecase",
  "layer": "usecase",
  "types": {{}},
  "traits": {{}},
  "functions": {{
    "usecase::{function_name}": {{
      "action": "add",
      "role": "FreeFunction",
      "params": [],
      "returns": "()",
      "generics": [{{ "name": "T", "bounds": [] }}],
      "where_predicates": [
        {predicate_json}
      ]
    }}
  }}
}}"#
        )
    }

    fn usecase_function_doc_with_where_predicate(
        function_name: &str,
        predicate: WherePredicateDecl,
    ) -> CatalogueDocument {
        let entry = FunctionEntry {
            action: ItemAction::Add,
            role: FunctionRole::FreeFunction,
            params: vec![],
            returns: TypeRef::new("()".to_string()).unwrap(),
            is_async: false,
            generics: vec![],
            where_predicates: vec![predicate],
            docs: None,
            spec_refs: vec![],
            informal_grounds: vec![],
        };
        let crate_name = CrateName::new("usecase".to_string()).unwrap();
        let layer = LayerId::try_new("usecase").unwrap();
        let mut doc = CatalogueDocument::new(4, crate_name.clone(), layer);
        let fn_name = FunctionName::new(function_name).unwrap();
        let path = FunctionPath::at_root(crate_name, fn_name);
        doc.functions.insert(path, entry);
        doc
    }

    #[test]
    fn test_decode_minimal_v5_json_succeeds() {
        let json = minimal_v5_json("domain", "domain");
        let doc = CatalogueDocumentCodec::decode(&json, "domain").unwrap();
        assert_eq!(doc.schema_version, 5);
        assert_eq!(doc.crate_name.as_str(), "domain");
        assert!(doc.types.is_empty());
    }

    #[test]
    fn test_encode_pins_schema_version_to_current_codec_version() {
        let crate_name = CrateName::new("domain").unwrap();
        let layer = LayerId::try_new("domain").unwrap();
        let doc = CatalogueDocument::new(3, crate_name, layer);

        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        let value: serde_json::Value = serde_json::from_str(&encoded).unwrap();

        assert_eq!(value["schema_version"], SCHEMA_VERSION);
        let decoded = CatalogueDocumentCodec::decode(&encoded, "domain").unwrap();
        assert_eq!(decoded.schema_version, SCHEMA_VERSION);
    }

    #[test]
    fn test_decode_schema_version_2_returns_unsupported_schema_version() {
        let json = r#"{"schema_version": 2, "crate_name": "domain", "layer": "domain"}"#;
        let err = CatalogueDocumentCodec::decode(json, "domain").unwrap_err();
        assert!(
            matches!(
                err,
                CatalogueDocumentCodecError::UnsupportedSchemaVersion { actual: 2, expected: 5 }
            ),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn test_decode_schema_version_3_returns_unsupported_schema_version() {
        // Schema version 3 catalogues (containing spec_refs[].hash) must be rejected
        // fail-closed after the spec-ref-embedded-hash-removal breaking change.
        let json = r#"{"schema_version": 3, "crate_name": "domain", "layer": "domain"}"#;
        let err = CatalogueDocumentCodec::decode(json, "domain").unwrap_err();
        assert!(
            matches!(
                err,
                CatalogueDocumentCodecError::UnsupportedSchemaVersion { actual: 3, expected: 5 }
            ),
            "schema_version 3 must be rejected: {err:?}"
        );
    }

    #[test]
    fn test_decode_schema_version_4_returns_migration_required_error() {
        // Schema version 4 → v5 is a known breaking change (TypeEntry.role wire format
        // changed from bare string to discriminated-object). v4 catalogues must be
        // rejected with SchemaVersionRequiresMigration, not a confusing JSON type error.
        let json = r#"{"schema_version": 4, "crate_name": "domain", "layer": "domain"}"#;
        let err = CatalogueDocumentCodec::decode(json, "domain").unwrap_err();
        assert!(
            matches!(
                err,
                CatalogueDocumentCodecError::SchemaVersionRequiresMigration { from: 4, to: 5, .. }
            ),
            "schema_version 4 must be rejected with SchemaVersionRequiresMigration: {err:?}"
        );
        // The error message must mention both versions and hint at re-generation.
        let msg = err.to_string();
        assert!(
            msg.contains("4") && msg.contains("5"),
            "error message must mention both versions, got: {msg}"
        );
    }

    #[test]
    fn test_decode_schema_version_future_returns_unsupported_schema_version() {
        // A version higher than the current SCHEMA_VERSION has no migration path —
        // reject with UnsupportedSchemaVersion (the codec is too old to handle it).
        let json = r#"{"schema_version": 99, "crate_name": "domain", "layer": "domain"}"#;
        let err = CatalogueDocumentCodec::decode(json, "domain").unwrap_err();
        assert!(
            matches!(
                err,
                CatalogueDocumentCodecError::UnsupportedSchemaVersion { actual: 99, expected: 5 }
            ),
            "future schema_version must be rejected with UnsupportedSchemaVersion: {err:?}"
        );
    }

    #[test]
    fn test_decode_crate_name_mismatch_returns_error() {
        let json = minimal_v5_json("domain", "domain");
        let err = CatalogueDocumentCodec::decode(&json, "usecase").unwrap_err();
        assert!(matches!(err, CatalogueDocumentCodecError::CrateNameMismatch { .. }), "{err:?}");
    }

    #[test]
    fn test_decode_invalid_json_returns_json_error() {
        let err = CatalogueDocumentCodec::decode("{not json}", "domain").unwrap_err();
        assert!(matches!(err, CatalogueDocumentCodecError::Json(_)), "{err:?}");
    }

    #[test]
    fn test_decode_type_entry_with_plain_struct_kind() {
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "UserId": {
      "action": "add",
      "role": { "ValueObject": {} },
      "kind": { "kind": "struct", "shape": { "kind": "plain", "fields": [] } }
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        assert_eq!(doc.types.len(), 1);
        let entry = doc.types.values().next().unwrap();
        assert_eq!(entry.action, ItemAction::Add);
        assert_eq!(entry.role, DataRole::value_object());
        assert!(
            matches!(&entry.kind, TypeKindV2::Struct(sk) if matches!(sk.shape, StructShape::Plain { .. }))
        );
    }

    #[test]
    fn test_decode_type_entry_with_unit_struct_kind() {
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "Marker": {
      "action": "add",
      "role": { "ValueObject": {} },
      "kind": { "kind": "struct", "shape": { "kind": "unit" } }
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        assert_eq!(doc.types.len(), 1);
        let entry = doc.types.values().next().unwrap();
        assert!(
            matches!(&entry.kind, TypeKindV2::Struct(sk) if matches!(sk.shape, StructShape::Unit))
        );
    }

    #[test]
    fn test_decode_type_entry_with_tuple_struct_kind() {
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "UserId": {
      "action": "add",
      "role": { "ValueObject": {} },
      "kind": { "kind": "struct", "shape": { "kind": "tuple", "fields": ["String"] } }
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        let entry = doc.types.values().next().unwrap();
        match &entry.kind {
            TypeKindV2::Struct(sk) => match &sk.shape {
                StructShape::Tuple { fields, has_stripped_fields } => {
                    assert_eq!(fields.len(), 1);
                    assert_eq!(fields[0].as_str(), "String");
                    assert!(!has_stripped_fields);
                }
                _ => panic!("expected Tuple shape"),
            },
            _ => panic!("expected Struct kind"),
        }
    }

    #[test]
    fn test_decode_type_entry_with_plain_struct_and_typestate_marker() {
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "IdleState": {
      "action": "add",
      "role": { "ValueObject": {} },
      "kind": {
        "kind": "struct",
        "shape": { "kind": "plain", "fields": [] },
        "typestate": { "state_name": "ReviewMachine", "transition_methods": ["approve"] }
      }
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        let entry = doc.types.values().next().unwrap();
        match &entry.kind {
            TypeKindV2::Struct(sk) => {
                assert!(sk.typestate.is_some(), "expected typestate to be Some");
                let ts = sk.typestate.as_ref().unwrap();
                assert_eq!(ts.state_name().as_str(), "ReviewMachine");
                assert_eq!(ts.transitions().transition_methods().len(), 1);
            }
            _ => panic!("expected Struct kind with typestate"),
        }
    }

    #[test]
    fn test_decode_trait_entry_with_secondary_port_role() {
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {
    "UserRepository": {
      "action": "add",
      "role": { "SecondaryPort": {} },
      "methods": []
    }
  },
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        assert_eq!(doc.traits.len(), 1);
        let entry = doc.traits.values().next().unwrap();
        assert_eq!(entry.role, ContractRole::SecondaryPort);
    }

    // -----------------------------------------------------------------------
    // TraitEntry generics + where_predicates (T006 / AC-07)
    // -----------------------------------------------------------------------

    #[test]
    fn test_decode_trait_entry_with_generics_and_where_predicates() {
        // AC-07: `trait Foo<T> where T: Clone` is expressible in JSON.
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {
    "Foo": {
      "action": "add",
      "role": { "SecondaryPort": {} },
      "generics": [{ "name": "T", "bounds": [] }],
      "where_predicates": [{ "lhs": "T", "rhs": ["Clone"] }]
    }
  },
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        let entry = doc.traits.values().next().unwrap();
        assert_eq!(entry.generics.len(), 1);
        assert_eq!(entry.generics[0].name.as_str(), "T");
        assert_eq!(entry.where_predicates.len(), 1);
        assert_eq!(entry.where_predicates[0].lhs.as_str(), "T");
        assert_eq!(entry.where_predicates[0].rhs[0].as_str(), "Clone");
    }

    #[test]
    fn test_decode_trait_entry_missing_generics_and_where_predicates_defaults_to_empty() {
        // CN-01 / OS-04 backward compat: catalogues that predate generics/where_predicates
        // on traits must still decode successfully with both fields defaulting to empty.
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {
    "OldTrait": {
      "action": "add",
      "role": { "SecondaryPort": {} }
    }
  },
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        let entry = doc.traits.values().next().unwrap();
        assert!(entry.generics.is_empty(), "generics must default to empty Vec");
        assert!(entry.where_predicates.is_empty(), "where_predicates must default to empty Vec");
    }

    #[test]
    fn test_encode_decode_round_trip_preserves_trait_generics_and_where_predicates() {
        // Round-trip: encode → JSON → decode preserves generics and where_predicates.
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {
    "Bar": {
      "action": "add",
      "role": { "SecondaryPort": {} },
      "generics": [{ "name": "T", "bounds": ["Clone"] }],
      "where_predicates": [{ "lhs": "T", "rhs": ["Send"] }]
    }
  },
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        let doc2 = CatalogueDocumentCodec::decode(&encoded, "domain").unwrap();
        assert_eq!(doc, doc2);
        let entry = doc2.traits.values().next().unwrap();
        assert_eq!(entry.generics[0].name.as_str(), "T");
        assert_eq!(entry.generics[0].bounds[0].as_str(), "Clone");
        assert_eq!(entry.where_predicates[0].lhs.as_str(), "T");
        assert_eq!(entry.where_predicates[0].rhs[0].as_str(), "Send");
    }

    #[test]
    fn test_encode_trait_entry_with_empty_generics_omits_field_from_json() {
        // Byte-stable: empty generics/where_predicates must not appear in JSON
        // (skip_serializing_if = "Vec::is_empty").
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {
    "Plain": {
      "action": "add",
      "role": { "SecondaryPort": {} }
    }
  },
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        assert!(
            !encoded.contains("\"generics\""),
            "empty generics must be omitted from JSON (byte-stable); encoded:\n{encoded}"
        );
        assert!(
            !encoded.contains("\"where_predicates\""),
            "empty where_predicates must be omitted from JSON (byte-stable); encoded:\n{encoded}"
        );
    }

    #[test]
    fn test_decode_trait_entry_with_invalid_generics_returns_error() {
        // Codec must reject malformed generic param names at decode time.
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {
    "BadTrait": {
      "action": "add",
      "role": { "SecondaryPort": {} },
      "generics": [{ "name": "", "bounds": [] }]
    }
  },
  "functions": {}
}"#;
        let result = CatalogueDocumentCodec::decode(json, "domain");
        assert!(
            matches!(result, Err(CatalogueDocumentCodecError::InvalidEntry { .. })),
            "expected InvalidEntry for empty generic param name, got: {result:?}"
        );
    }

    #[test]
    fn test_encode_decode_round_trip_preserves_data() {
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "UserId": {
      "action": "add",
      "role": { "ValueObject": {} },
      "kind": { "kind": "struct", "shape": { "kind": "tuple", "fields": ["String"] } },
      "methods": [],
      "module_path": ""
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        let doc2 = CatalogueDocumentCodec::decode(&encoded, "domain").unwrap();
        assert_eq!(doc, doc2);
    }

    #[test]
    fn test_decode_function_entry() {
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {},
  "functions": {
    "domain::register_user": {
      "action": "add",
      "role": "FreeFunction",
      "params": [],
      "returns": "()"
    }
  }
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        assert_eq!(doc.functions.len(), 1);
    }

    #[test]
    fn test_derive_filename_stem_with_types_suffix() {
        use std::path::Path;
        let path = Path::new("/track/items/my-track/domain-types.json");
        assert_eq!(derive_filename_stem(path), "domain");
    }

    #[test]
    fn test_derive_filename_stem_without_types_suffix() {
        use std::path::Path;
        let path = Path::new("/track/items/my-track/domain.json");
        assert_eq!(derive_filename_stem(path), "domain");
    }

    #[test]
    fn test_decode_top_level_trait_impls_with_generic_args_in_trait_ref() {
        // ADR `2026-05-20-0048` D1/D2: trait_impls are top-level; trait_ref encodes generic args.
        let json = r#"{
  "schema_version": 5,
  "crate_name": "usecase",
  "layer": "usecase",
  "types": {
    "RenderContractMapError": {
      "action": "modify",
      "role": { "ErrorType": {} },
      "kind": { "kind": "enum", "variants": [] }
    }
  },
  "traits": {},
  "functions": {},
  "trait_impls": [
    { "trait_ref": "core::convert::From<CatalogueLoaderError>", "for_type": "RenderContractMapError" },
    { "trait_ref": "core::convert::From<ContractMapWriterError>", "for_type": "RenderContractMapError" },
    { "trait_ref": "core::fmt::Display", "for_type": "RenderContractMapError" }
  ]
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "usecase").unwrap();
        assert_eq!(doc.trait_impls.len(), 3);
        assert_eq!(
            doc.trait_impls[0].trait_ref.as_str(),
            "core::convert::From<CatalogueLoaderError>"
        );
        assert_eq!(
            doc.trait_impls[1].trait_ref.as_str(),
            "core::convert::From<ContractMapWriterError>"
        );
        assert_eq!(doc.trait_impls[2].trait_ref.as_str(), "core::fmt::Display");
        for ti in &doc.trait_impls {
            assert_eq!(ti.for_type.as_str(), "RenderContractMapError");
        }
    }

    #[test]
    fn test_decode_top_level_trait_impl_without_generic_args() {
        // trait_ref without angle brackets (no generic args) decodes correctly.
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "MyType": {
      "action": "add",
      "role": { "ErrorType": {} },
      "kind": { "kind": "enum", "variants": [] }
    }
  },
  "traits": {},
  "functions": {},
  "trait_impls": [
    { "trait_ref": "core::convert::From", "for_type": "MyType" }
  ]
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        assert_eq!(doc.trait_impls.len(), 1);
        assert_eq!(doc.trait_impls[0].trait_ref.as_str(), "core::convert::From");
        assert_eq!(doc.trait_impls[0].for_type.as_str(), "MyType");
    }

    #[test]
    fn test_encode_decode_round_trip_preserves_top_level_trait_impls() {
        let json = r#"{
  "schema_version": 5,
  "crate_name": "usecase",
  "layer": "usecase",
  "types": {
    "MyError": {
      "action": "add",
      "role": { "ErrorType": {} },
      "kind": { "kind": "enum", "variants": [] }
    }
  },
  "traits": {},
  "functions": {},
  "trait_impls": [
    { "trait_ref": "core::convert::From<IoError>", "for_type": "MyError" },
    { "trait_ref": "core::fmt::Display", "for_type": "MyError" }
  ]
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "usecase").unwrap();
        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        let doc2 = CatalogueDocumentCodec::decode(&encoded, "usecase").unwrap();
        assert_eq!(doc, doc2);
        assert_eq!(doc2.trait_impls[0].trait_ref.as_str(), "core::convert::From<IoError>");
        assert_eq!(doc2.trait_impls[1].trait_ref.as_str(), "core::fmt::Display");
    }

    #[test]
    fn test_decode_old_schema_type_entry_trait_impls_is_rejected() {
        // Old schema (TypeEntry-level trait_impls) must be rejected by deny_unknown_fields.
        // ADR `2026-05-20-0048`: catalogues must be completely rewritten; no backward compat.
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "BadError": {
      "action": "add",
      "role": { "ErrorType": {} },
      "kind": { "kind": "enum", "variants": [] },
      "trait_impls": [
        { "trait_name": "From", "origin_crate": "core", "generic_args": "" }
      ]
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let result = CatalogueDocumentCodec::decode(json, "domain");
        assert!(
            result.is_err(),
            "old TypeEntry-level trait_impls must be rejected (deny_unknown_fields), got: {result:?}"
        );
    }

    #[test]
    fn test_decode_top_level_trait_impl_with_empty_trait_ref_returns_error() {
        // An empty trait_ref string must be rejected (TypeRef::new validates non-empty).
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {},
  "functions": {},
  "trait_impls": [
    { "trait_ref": "", "for_type": "MyType" }
  ]
}"#;
        let result = CatalogueDocumentCodec::decode(json, "domain");
        assert!(result.is_err(), "empty trait_ref must be rejected, got: {result:?}");
    }

    #[test]
    fn test_decode_old_unit_struct_wire_tag_fails() {
        // The old `kind: "unit_struct"` wire tag is no longer supported after CN-02 breaking change.
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "Marker": {
      "action": "add",
      "role": { "ValueObject": {} },
      "kind": { "kind": "unit_struct" }
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let result = CatalogueDocumentCodec::decode(json, "domain");
        assert!(result.is_err(), "old unit_struct wire tag must be rejected: {result:?}");
    }

    // -----------------------------------------------------------------------
    // T005 / AC-03: unit struct + typestate round-trip
    // -----------------------------------------------------------------------

    #[test]
    fn test_unit_struct_with_typestate_round_trips() {
        // AC-03: `kind: "struct"` + `shape: {"kind": "unit"}` + `typestate` decode → domain → encode round-trip.
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "Locked": {
      "action": "add",
      "role": { "ValueObject": {} },
      "kind": {
        "kind": "struct",
        "shape": { "kind": "unit" },
        "typestate": { "state_name": "LockMachine", "transition_methods": ["unlock"] }
      },
      "methods": [],
      "module_path": ""
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        let entry = doc.types.values().next().unwrap();
        // A-side retains typestate marker.
        let sk = match &entry.kind {
            domain::tddd::catalogue_v2::TypeKindV2::Struct(sk) => sk,
            _ => panic!("expected Struct"),
        };
        assert!(
            matches!(sk.shape, domain::tddd::catalogue_v2::StructShape::Unit),
            "shape must be Unit"
        );
        assert!(sk.typestate.is_some(), "typestate marker must be present");
        assert_eq!(sk.typestate.as_ref().unwrap().state_name().as_str(), "LockMachine");
        // Round-trip: encode → decode must produce the same doc.
        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        let doc2 = CatalogueDocumentCodec::decode(&encoded, "domain").unwrap();
        assert_eq!(doc, doc2);
    }

    // -----------------------------------------------------------------------
    // T005 / AC-04: tuple struct + typestate round-trip
    // -----------------------------------------------------------------------

    #[test]
    fn test_tuple_struct_with_typestate_round_trips() {
        // AC-04: `kind: "struct"` + `shape: {"kind": "tuple", ...}` + `typestate` decode → domain → encode round-trip.
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "Pending": {
      "action": "add",
      "role": { "ValueObject": {} },
      "kind": {
        "kind": "struct",
        "shape": { "kind": "tuple", "fields": ["Uuid"] },
        "typestate": { "state_name": "ApprovalMachine", "transition_methods": ["approve", "reject"] }
      },
      "methods": [],
      "module_path": ""
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        let entry = doc.types.values().next().unwrap();
        // A-side retains typestate marker.
        let sk = match &entry.kind {
            domain::tddd::catalogue_v2::TypeKindV2::Struct(sk) => sk,
            _ => panic!("expected Struct"),
        };
        assert!(
            matches!(sk.shape, domain::tddd::catalogue_v2::StructShape::Tuple { .. }),
            "shape must be Tuple"
        );
        assert!(sk.typestate.is_some(), "typestate marker must be present");
        assert_eq!(sk.typestate.as_ref().unwrap().state_name().as_str(), "ApprovalMachine");
        // The tuple shape retains the positional fields.
        if let domain::tddd::catalogue_v2::StructShape::Tuple { fields, .. } = &sk.shape {
            assert_eq!(fields.len(), 1);
            assert_eq!(fields[0].as_str(), "Uuid");
        } else {
            panic!("expected Tuple shape");
        }
        // Round-trip: encode → decode must produce the same doc.
        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        let doc2 = CatalogueDocumentCodec::decode(&encoded, "domain").unwrap();
        assert_eq!(doc, doc2);
    }

    #[test]
    fn test_decode_method_with_invalid_generics_returns_invalid_entry_error() {
        for (case, generics_json) in [
            ("empty generic param name", r#"[{ "name": "", "bounds": ["Send"] }]"#),
            ("empty generic bound", r#"[{ "name": "T", "bounds": [""] }]"#),
            ("hyphenated generic param name", r#"[{ "name": "T-U", "bounds": [] }]"#),
            ("qualified generic param name", r#"[{ "name": "T::X", "bounds": [] }]"#),
            ("angle-bracket generic param name", r#"[{ "name": "<T>", "bounds": [] }]"#),
            ("digit-leading generic param name", r#"[{ "name": "123T", "bounds": [] }]"#),
            ("space-containing generic param name", r#"[{ "name": "T U", "bounds": [] }]"#),
        ] {
            let json = trait_method_catalogue_with_generics(generics_json);
            let result = CatalogueDocumentCodec::decode(&json, "domain");
            assert!(
                matches!(result, Err(CatalogueDocumentCodecError::InvalidEntry { .. })),
                "expected InvalidEntry for {case}, got: {result:?}"
            );
        }
    }

    #[test]
    fn test_decode_trait_with_empty_supertrait_bound_returns_invalid_entry_error() {
        // An empty supertrait_bounds entry must be rejected at the codec boundary.
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {
    "MyPort": {
      "action": "add",
      "role": { "SpecificationPort": {} },
      "supertrait_bounds": [""],
      "methods": []
    }
  },
  "functions": {}
}"#;
        let result = CatalogueDocumentCodec::decode(json, "domain");
        assert!(
            matches!(result, Err(CatalogueDocumentCodecError::InvalidEntry { .. })),
            "expected InvalidEntry for empty supertrait bound, got: {result:?}"
        );
    }

    // --- T011 round-trip tests for spec_refs / informal_grounds ---

    #[test]
    fn test_type_entry_round_trip_with_spec_refs_and_informal_grounds() {
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "UserId": {
      "action": "add",
      "role": { "ValueObject": {} },
      "kind": { "kind": "struct", "shape": { "kind": "unit" } },
      "spec_refs": [
        { "file": "track/items/x/spec.json", "anchor": "IN-01" }
      ],
      "informal_grounds": [
        { "kind": "discussion", "summary": "planning session note" }
      ]
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        let entry = doc.types.values().next().unwrap();
        assert_eq!(entry.spec_refs.len(), 1);
        assert_eq!(entry.spec_refs[0].anchor.as_ref(), "IN-01");
        assert_eq!(entry.informal_grounds.len(), 1);

        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        let doc2 = CatalogueDocumentCodec::decode(&encoded, "domain").unwrap();
        assert_eq!(doc, doc2);
    }

    #[test]
    fn test_trait_entry_round_trip_with_spec_refs_and_informal_grounds() {
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {
    "UserRepository": {
      "action": "add",
      "role": { "SecondaryPort": {} },
      "methods": [],
      "spec_refs": [
        { "file": "track/items/x/spec.json", "anchor": "AC-02" }
      ],
      "informal_grounds": [
        { "kind": "user_directive", "summary": "user directive to defer anchor" }
      ]
    }
  },
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        let entry = doc.traits.values().next().unwrap();
        assert_eq!(entry.spec_refs.len(), 1);
        assert_eq!(entry.spec_refs[0].anchor.as_ref(), "AC-02");
        assert_eq!(entry.informal_grounds.len(), 1);

        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        let doc2 = CatalogueDocumentCodec::decode(&encoded, "domain").unwrap();
        assert_eq!(doc, doc2);
    }

    #[test]
    fn test_decode_trait_without_assoc_items_defaults_to_empty() {
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {
    "PlainPort": {
      "action": "add",
      "role": { "SpecificationPort": {} },
      "methods": []
    }
  },
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        let entry = doc.traits.values().next().unwrap();

        assert!(entry.assoc_types.is_empty(), "omitted assoc_types must default to empty");
        assert!(entry.assoc_consts.is_empty(), "omitted assoc_consts must default to empty");
    }

    #[test]
    fn test_trait_assoc_items_decode_and_round_trip() {
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {
    "ProjectionPort": {
      "action": "add",
      "role": { "SpecificationPort": {} },
      "methods": [],
      "assoc_types": [
        { "name": "Output", "bounds": ["Send"], "default": "Vec<u8>" }
      ],
      "assoc_consts": [
        { "name": "ID", "ty": "usize", "default_value": "42" }
      ]
    }
  },
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        let entry = doc.traits.values().next().unwrap();

        assert_eq!(entry.assoc_types.len(), 1);
        assert_eq!(entry.assoc_types[0].name.as_str(), "Output");
        assert_eq!(entry.assoc_types[0].bounds[0].as_str(), "Send");
        assert_eq!(entry.assoc_types[0].default.as_ref().unwrap().as_str(), "Vec<u8>");
        assert_eq!(entry.assoc_consts.len(), 1);
        assert_eq!(entry.assoc_consts[0].name.as_str(), "ID");
        assert_eq!(entry.assoc_consts[0].ty.as_str(), "usize");
        assert_eq!(entry.assoc_consts[0].default_value.as_deref(), Some("42"));

        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        assert!(encoded.contains("\"assoc_types\""), "assoc_types must encode: {encoded}");
        assert!(encoded.contains("\"assoc_consts\""), "assoc_consts must encode: {encoded}");
        assert!(
            encoded.contains("\"default_value\": \"42\""),
            "assoc const default_value must encode: {encoded}"
        );
        let doc2 = CatalogueDocumentCodec::decode(&encoded, "domain").unwrap();
        assert_eq!(doc, doc2);
    }

    #[test]
    fn test_decode_trait_assoc_type_with_invalid_name_returns_error() {
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {
    "ProjectionPort": {
      "action": "add",
      "role": { "SpecificationPort": {} },
      "methods": [],
      "assoc_types": [
        { "name": "Bad-Name", "bounds": [], "default": "usize" }
      ]
    }
  },
  "functions": {}
}"#;
        let result = CatalogueDocumentCodec::decode(json, "domain");
        assert!(
            matches!(result, Err(CatalogueDocumentCodecError::InvalidEntry { .. })),
            "invalid associated type names must fail decode, got: {result:?}"
        );
    }

    #[test]
    fn test_decode_trait_assoc_const_empty_default_value_returns_error() {
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {
    "ProjectionPort": {
      "action": "add",
      "role": { "SpecificationPort": {} },
      "methods": [],
      "assoc_consts": [
        { "name": "ID", "ty": "usize", "default_value": "" }
      ]
    }
  },
  "functions": {}
}"#;
        let result = CatalogueDocumentCodec::decode(json, "domain");
        assert!(
            matches!(result, Err(CatalogueDocumentCodecError::InvalidEntry { .. })),
            "empty associated const default_value must fail decode, got: {result:?}"
        );
    }

    #[test]
    fn test_decode_trait_method_and_assoc_const_same_name_returns_error() {
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {
    "ProjectionPort": {
      "action": "add",
      "role": { "SpecificationPort": {} },
      "methods": [
        { "name": "ID", "params": [], "returns": "()" }
      ],
      "assoc_consts": [
        { "name": "ID", "ty": "usize" }
      ]
    }
  },
  "functions": {}
}"#;
        let result = CatalogueDocumentCodec::decode(json, "domain");
        assert!(
            matches!(result, Err(CatalogueDocumentCodecError::InvalidEntry { .. })),
            "methods and associated consts share the value namespace, got: {result:?}"
        );
    }

    #[test]
    fn test_function_entry_round_trip_with_spec_refs_and_informal_grounds() {
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {},
  "functions": {
    "domain::register_user": {
      "action": "add",
      "role": "FreeFunction",
      "params": [],
      "returns": "()",
      "spec_refs": [
        { "file": "track/items/x/spec.json", "anchor": "UC-01" }
      ],
      "informal_grounds": [
        { "kind": "memory", "summary": "session context decision" }
      ]
    }
  }
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        let entry = doc.functions.values().next().unwrap();
        assert_eq!(entry.spec_refs.len(), 1);
        assert_eq!(entry.spec_refs[0].anchor.as_ref(), "UC-01");
        assert_eq!(entry.informal_grounds.len(), 1);

        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        let doc2 = CatalogueDocumentCodec::decode(&encoded, "domain").unwrap();
        assert_eq!(doc, doc2);
    }

    #[test]
    fn test_type_entry_with_empty_grounding_fields_round_trips_to_empty_arrays() {
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "Marker": {
      "action": "add",
      "role": { "ValueObject": {} },
      "kind": { "kind": "struct", "shape": { "kind": "unit" } }
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        let entry = doc.types.values().next().unwrap();
        assert!(entry.spec_refs.is_empty());
        assert!(entry.informal_grounds.is_empty());

        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        // Encoded output must contain the grounding fields (always emitted).
        assert!(encoded.contains("\"spec_refs\""));
        assert!(encoded.contains("\"informal_grounds\""));
        let doc2 = CatalogueDocumentCodec::decode(&encoded, "domain").unwrap();
        assert_eq!(doc, doc2);
    }

    // spec_refs[].hash field is rejected (deny_unknown_fields — spec-ref-embedded-hash-removal)
    #[test]
    fn test_type_entry_spec_ref_with_hash_field_is_rejected() {
        // After schema_version 4 (spec-ref-embedded-hash-removal), spec_refs[].hash is removed.
        // A v4 catalogue that still carries "hash" in spec_refs must be rejected fail-closed
        // by deny_unknown_fields on SpecRefDto.
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "UserId": {
      "action": "add",
      "role": { "ValueObject": {} },
      "kind": { "kind": "struct", "shape": { "kind": "unit" } },
      "spec_refs": [
        { "file": "track/items/x/spec.json", "anchor": "IN-01", "hash": "0000000000000000000000000000000000000000000000000000000000000000" }
      ]
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let result = CatalogueDocumentCodec::decode(json, "domain");
        assert!(
            result.is_err(),
            "spec_refs[].hash field must be rejected in schema_version 4 (deny_unknown_fields), \
             got: {result:?}"
        );
    }

    // --- T012 cross-crate function path tests ---

    #[test]
    fn test_decode_function_with_own_crate_prefix_succeeds() {
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {},
  "functions": {
    "domain::create_user": {
      "action": "add",
      "role": "FreeFunction",
      "params": [],
      "returns": "()"
    }
  }
}"#;
        let result = CatalogueDocumentCodec::decode(json, "domain");
        assert!(result.is_ok(), "own-crate prefix must succeed: {result:?}");
    }

    #[test]
    fn test_decode_function_with_cross_crate_prefix_returns_cross_crate_error() {
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {},
  "functions": {
    "infrastructure::some_fn": {
      "action": "add",
      "role": "FreeFunction",
      "params": [],
      "returns": "()"
    }
  }
}"#;
        let result = CatalogueDocumentCodec::decode(json, "domain");
        assert!(
            matches!(result, Err(CatalogueDocumentCodecError::CrossCrateFunctionPath { .. })),
            "expected CrossCrateFunctionPath error, got: {result:?}"
        );
    }

    // -----------------------------------------------------------------------
    // ADR 0248 D13: MethodDeclaration.has_default_impl JSON round-trip (Gap 1)
    // -----------------------------------------------------------------------

    #[test]
    fn test_decode_trait_method_with_has_default_impl_true_preserves_field() {
        let json = r#"{
  "schema_version": 5,
  "crate_name": "usecase",
  "layer": "usecase",
  "types": {},
  "traits": {
    "Describable": {
      "action": "add",
      "role": { "SpecificationPort": {} },
      "methods": [
        {
          "name": "describe",
          "receiver": "&self",
          "params": [],
          "returns": "String",
          "is_async": false,
          "has_default_impl": true
        }
      ]
    }
  },
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "usecase").unwrap();
        let trait_entry = doc.traits.values().next().unwrap();
        assert_eq!(trait_entry.methods.len(), 1);
        assert!(
            trait_entry.methods[0].has_default_impl,
            "has_default_impl=true must round-trip through decode"
        );
    }

    #[test]
    fn test_encode_decode_round_trip_preserves_has_default_impl() {
        let doc = CatalogueDocumentCodec::decode(mixed_trait_with_default_impl_json(), "usecase")
            .unwrap();
        let trait_entry = doc.traits.values().next().unwrap();
        let required = trait_entry.methods.iter().find(|m| m.name.as_str() == "required").unwrap();
        assert!(!required.has_default_impl, "omitted has_default_impl must default to false");

        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        let doc2 = CatalogueDocumentCodec::decode(&encoded, "usecase").unwrap();
        assert_eq!(doc, doc2);
        let trait_entry = doc2.traits.values().next().unwrap();
        let required = trait_entry.methods.iter().find(|m| m.name.as_str() == "required").unwrap();
        let provided = trait_entry.methods.iter().find(|m| m.name.as_str() == "provided").unwrap();
        assert!(!required.has_default_impl);
        assert!(provided.has_default_impl);
    }

    #[test]
    fn test_decode_trait_method_with_has_default_impl_true_omits_default_false_on_encode() {
        // `has_default_impl: false` is the JSON default and must be omitted on encode
        // (skip_serializing_if). Only `true` should appear in the rendered JSON.
        let doc = CatalogueDocumentCodec::decode(mixed_trait_with_default_impl_json(), "usecase")
            .unwrap();
        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        // The required method must NOT carry has_default_impl in the rendered JSON.
        // The provided method MUST carry it.
        assert!(
            encoded.contains("\"name\": \"provided\""),
            "expected 'provided' method in encoded JSON: {encoded}"
        );
        let provided_idx = encoded.find("\"name\": \"provided\"").unwrap();
        let provided_slice = &encoded[provided_idx..];
        let next_method_idx =
            provided_slice[1..].find("\"name\":").map_or(provided_slice.len(), |i| i + 1);
        let provided_block = &provided_slice[..next_method_idx];
        assert!(
            provided_block.contains("\"has_default_impl\": true"),
            "provided method block must contain has_default_impl=true: {provided_block}"
        );
        let required_idx = encoded.find("\"name\": \"required\"").unwrap();
        let required_slice = &encoded[required_idx..];
        let next_after_required =
            required_slice[1..].find("\"name\":").map_or(required_slice.len(), |i| i + 1);
        let required_block = &required_slice[..next_after_required];
        assert!(
            !required_block.contains("has_default_impl"),
            "required method block must omit has_default_impl when false: {required_block}"
        );
    }

    // -----------------------------------------------------------------------
    // ADR 0248 D14: FunctionEntry.generics JSON round-trip (Gap 2)
    // -----------------------------------------------------------------------

    #[test]
    fn test_decode_function_with_generics_preserves_field() {
        let json = r#"{
  "schema_version": 5,
  "crate_name": "usecase",
  "layer": "usecase",
  "types": {},
  "traits": {},
  "functions": {
    "usecase::check_strict_merge_gate": {
      "action": "add",
      "role": "FreeFunction",
      "params": [{ "name": "reader", "ty": "R" }],
      "returns": "Result<(), CheckStrictMergeGateError<E>>",
      "is_async": false,
      "generics": [
        { "name": "R", "bounds": ["TrackBlobReader<Error = E>"] },
        { "name": "E", "bounds": ["std::error::Error"] }
      ]
    }
  }
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "usecase").unwrap();
        let entry = doc.functions.values().next().unwrap();
        assert_eq!(entry.generics.len(), 2);
        assert_eq!(entry.generics[0].name.as_str(), "R");
        assert_eq!(entry.generics[0].bounds[0].as_str(), "TrackBlobReader<Error = E>");
        assert_eq!(entry.generics[1].name.as_str(), "E");
        assert_eq!(entry.generics[1].bounds[0].as_str(), "std::error::Error");
    }

    #[test]
    fn test_decode_function_without_generics_field_defaults_to_empty() {
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {},
  "functions": {
    "domain::do_thing": {
      "action": "add",
      "role": "FreeFunction",
      "params": [],
      "returns": "()"
    }
  }
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        let entry = doc.functions.values().next().unwrap();
        assert!(
            entry.generics.is_empty(),
            "omitted generics must default to empty Vec for backward compatibility"
        );
    }

    #[test]
    fn test_encode_decode_round_trip_preserves_function_generics() {
        let json = r#"{
  "schema_version": 5,
  "crate_name": "usecase",
  "layer": "usecase",
  "types": {},
  "traits": {},
  "functions": {
    "usecase::generic_fn": {
      "action": "add",
      "role": "FreeFunction",
      "params": [{ "name": "x", "ty": "T" }],
      "returns": "T",
      "is_async": false,
      "generics": [
        { "name": "T", "bounds": ["Clone"] }
      ]
    }
  }
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "usecase").unwrap();
        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        let doc2 = CatalogueDocumentCodec::decode(&encoded, "usecase").unwrap();
        assert_eq!(doc, doc2);
        let entry = doc2.functions.values().next().unwrap();
        assert_eq!(entry.generics.len(), 1);
        assert_eq!(entry.generics[0].name.as_str(), "T");
    }

    #[test]
    fn test_encode_function_with_empty_generics_omits_field() {
        // Vec::is_empty must skip the generics field on encode so legacy
        // catalogues (no generics) stay byte-stable.
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {},
  "functions": {
    "domain::simple": {
      "action": "add",
      "role": "FreeFunction",
      "params": [],
      "returns": "()"
    }
  }
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        assert!(
            !encoded.contains("\"generics\""),
            "generics field must be omitted from encoded JSON when empty: {encoded}"
        );
    }

    #[test]
    fn test_decode_function_with_duplicate_generic_param_names_returns_error() {
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {},
  "functions": {
    "domain::bad": {
      "action": "add",
      "role": "FreeFunction",
      "params": [],
      "returns": "()",
      "generics": [
        { "name": "T", "bounds": [] },
        { "name": "T", "bounds": [] }
      ]
    }
  }
}"#;
        let result = CatalogueDocumentCodec::decode(json, "domain");
        assert!(
            matches!(result, Err(CatalogueDocumentCodecError::InvalidEntry { .. })),
            "duplicate function generic param names must be rejected, got: {result:?}"
        );
    }

    // -----------------------------------------------------------------------
    // ADR 2026-05-13-1153 D2: WherePredicateDecl JSON round-trip (T035 codec)
    // -----------------------------------------------------------------------

    #[test]
    fn test_decode_trait_method_with_where_predicates_round_trips() {
        let json = r#"{
  "schema_version": 5,
  "crate_name": "usecase",
  "layer": "usecase",
  "types": {},
  "traits": {
    "GenericRepo": {
      "action": "add",
      "role": { "SecondaryPort": {} },
      "methods": [
        {
          "name": "save",
          "receiver": "&self",
          "params": [{ "name": "item", "ty": "T" }],
          "returns": "Result",
          "generics": [{ "name": "T", "bounds": [] }],
          "where_predicates": [
            { "type": "T", "bounds": ["Clone", "Send"] }
          ]
        }
      ]
    }
  },
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "usecase").unwrap();
        let entry = doc.traits.values().next().unwrap();
        let method = &entry.methods[0];
        assert_eq!(method.where_predicates.len(), 1);
        assert_eq!(method.where_predicates[0].lhs.as_str(), "T");
        assert_eq!(method.where_predicates[0].rhs.len(), 2);
        assert_eq!(method.where_predicates[0].rhs[0].as_str(), "Clone");
        assert_eq!(method.where_predicates[0].rhs[1].as_str(), "Send");

        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        let doc2 = CatalogueDocumentCodec::decode(&encoded, "usecase").unwrap();
        assert_eq!(doc, doc2);
    }

    #[test]
    fn test_decode_function_with_where_predicates_round_trips() {
        let json = r#"{
  "schema_version": 5,
  "crate_name": "usecase",
  "layer": "usecase",
  "types": {},
  "traits": {},
  "functions": {
    "usecase::generic_fn": {
      "action": "add",
      "role": "FreeFunction",
      "params": [{ "name": "x", "ty": "T" }],
      "returns": "T",
      "generics": [{ "name": "T", "bounds": [] }],
      "where_predicates": [
        { "type": "T", "bounds": ["Clone"] }
      ]
    }
  }
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "usecase").unwrap();
        let entry = doc.functions.values().next().unwrap();
        assert_eq!(entry.where_predicates.len(), 1);
        assert_eq!(entry.where_predicates[0].lhs.as_str(), "T");
        assert_eq!(entry.where_predicates[0].rhs[0].as_str(), "Clone");

        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        let doc2 = CatalogueDocumentCodec::decode(&encoded, "usecase").unwrap();
        assert_eq!(doc, doc2);
    }

    // -----------------------------------------------------------------------
    // ADR 2026-05-18-1223 D1: backward-compatible alias decode (CN-01 / OS-04)
    // -----------------------------------------------------------------------

    #[test]
    fn test_decode_legacy_type_bounds_fields_decode_via_alias() {
        // Legacy catalogues that use `"type"` / `"bounds"` fields (pre-ADR-2026-05-18-1223)
        // must decode correctly via the `#[serde(alias)]` backward-compat path.
        // The decoded domain value must use `lhs` / `rhs` with `BoundOp::Bound` default.
        let json = r#"{
  "schema_version": 5,
  "crate_name": "usecase",
  "layer": "usecase",
  "types": {},
  "traits": {
    "LegacyRepo": {
      "action": "add",
      "role": { "SecondaryPort": {} },
      "methods": [
        {
          "name": "find",
          "receiver": "&self",
          "params": [],
          "returns": "Option<T>",
          "generics": [{ "name": "T", "bounds": [] }],
          "where_predicates": [
            { "type": "T", "bounds": ["Clone", "Send"] }
          ]
        }
      ]
    }
  },
  "functions": {}
}"#;
        use domain::tddd::catalogue_v2::methods::BoundOp;
        let doc = CatalogueDocumentCodec::decode(json, "usecase").unwrap();
        let method = &doc.traits.values().next().unwrap().methods[0];
        assert_eq!(method.where_predicates.len(), 1, "expected 1 where predicate");
        let pred = &method.where_predicates[0];
        assert_eq!(pred.lhs.as_str(), "T", "legacy `type` field maps to `lhs`");
        assert_eq!(pred.rhs.len(), 2, "legacy `bounds` field maps to `rhs`");
        assert_eq!(pred.rhs[0].as_str(), "Clone");
        assert_eq!(pred.rhs[1].as_str(), "Send");
        assert_eq!(pred.operator, BoundOp::Bound, "missing operator defaults to BoundOp::Bound");
    }

    #[test]
    fn test_encode_new_fields_produces_lhs_rhs_operator_json() {
        // After encode, the JSON output for where_predicates must use `"lhs"` / `"rhs"` / `"operator"`
        // field names rather than the legacy `"type"` / `"bounds"` names.
        let json_in = r#"{
  "schema_version": 5,
  "crate_name": "usecase",
  "layer": "usecase",
  "types": {},
  "traits": {},
  "functions": {
    "usecase::fn_with_where": {
      "action": "add",
      "role": "FreeFunction",
      "params": [],
      "returns": "()",
      "where_predicates": [
        { "lhs": "T", "rhs": ["Clone"], "operator": "Bound" }
      ]
    }
  }
}"#;
        let doc = CatalogueDocumentCodec::decode(json_in, "usecase").unwrap();
        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        // The where_predicates must use "lhs" and "rhs" (new schema).
        assert!(
            encoded.contains("\"lhs\""),
            "encoded JSON where_predicates must use \"lhs\" field: {encoded}"
        );
        assert!(
            encoded.contains("\"rhs\""),
            "encoded JSON where_predicates must use \"rhs\" field: {encoded}"
        );
        // The legacy field names must not appear in the where_predicates context.
        // We verify by checking that the encoded where_predicates block (which exists)
        // does not contain "\"type\": " (the legacy LHS key).
        // Note: "bounds" may appear in other fields (e.g. MethodGenericParamDto.bounds),
        // so we only check that the legacy "type" key is absent (it's only used in where_predicates).
        assert!(
            !encoded.contains("\"type\":"),
            "encoded JSON must NOT contain legacy \"type\" field in where_predicates: {encoded}"
        );
    }

    #[test]
    fn test_decode_without_where_predicates_field_defaults_to_empty() {
        // Legacy catalogues that predate T035 omit where_predicates; it must default to
        // empty for forward-compat (serde default + skip_serializing_if = Vec::is_empty).
        let json = r#"{
  "schema_version": 5,
  "crate_name": "usecase",
  "layer": "usecase",
  "types": {},
  "traits": {
    "SimplePort": {
      "action": "add",
      "role": { "SpecificationPort": {} },
      "methods": [
        { "name": "do_it", "receiver": "&self", "params": [], "returns": "()" }
      ]
    }
  },
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "usecase").unwrap();
        let method = &doc.traits.values().next().unwrap().methods[0];
        assert!(
            method.where_predicates.is_empty(),
            "omitted where_predicates must default to empty Vec"
        );
    }

    #[test]
    fn test_encode_with_empty_where_predicates_omits_field() {
        // `skip_serializing_if = "Vec::is_empty"` must suppress the field
        // so legacy catalogues (no where_predicates) stay byte-stable.
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {
    "SimplePort": {
      "action": "add",
      "role": { "SpecificationPort": {} },
      "methods": [
        { "name": "do_it", "receiver": "&self", "params": [], "returns": "()" }
      ]
    }
  },
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        assert!(
            !encoded.contains("\"where_predicates\""),
            "where_predicates field must be omitted when empty: {encoded}"
        );
    }

    #[test]
    fn test_decode_where_predicate_with_empty_bounds_returns_invalid_entry_error() {
        // A where predicate with no bounds (`where T:`) is invalid Rust and
        // must be rejected by the decoder at the codec boundary.
        let json = r#"{
  "schema_version": 5,
  "crate_name": "usecase",
  "layer": "usecase",
  "types": {},
  "traits": {
    "BadPort": {
      "action": "add",
      "role": { "SpecificationPort": {} },
      "methods": [
        {
          "name": "do_it",
          "receiver": "&self",
          "params": [],
          "returns": "()",
          "generics": [{ "name": "T", "bounds": [] }],
          "where_predicates": [
            { "type": "T", "bounds": [] }
          ]
        }
      ]
    }
  },
  "functions": {}
}"#;
        let result = CatalogueDocumentCodec::decode(json, "usecase");
        assert!(
            matches!(result, Err(CatalogueDocumentCodecError::InvalidEntry { .. })),
            "expected InvalidEntry for empty where_predicate bounds, got: {result:?}"
        );
    }

    #[test]
    fn test_encode_where_predicate_with_empty_rhs_returns_invalid_entry_error() {
        use domain::tddd::catalogue_v2::methods::BoundOp;

        // Construct a FunctionEntry with a WherePredicateDecl that has empty rhs.
        // The encoder must return an error in release builds (not just fire a debug_assert).
        let bad_predicate = WherePredicateDecl {
            lhs: TypeRef::new("T".to_string()).unwrap(),
            rhs: vec![], // empty — violates the codec invariant
            operator: BoundOp::Bound,
        };
        let doc = usecase_function_doc_with_where_predicate("bad_fn", bad_predicate);

        let result = CatalogueDocumentCodec::encode(&doc);
        assert!(
            matches!(result, Err(CatalogueDocumentCodecError::InvalidEntry { .. })),
            "expected InvalidEntry when encoding WherePredicateDecl with empty rhs, got: {result:?}"
        );
    }

    #[test]
    fn test_decode_where_predicate_with_empty_lhs_returns_invalid_entry_error() {
        // ADR 2026-05-18-1223 D1: `WherePredicateDecl.lhs` must be a non-empty
        // TypeRef. `where_predicates_from_dtos` rejects empty strings at decode time so
        // the validation rule does not silently regress.
        // Legacy alias `"type"` field is used here to verify backward-compat reads.
        let json = usecase_function_with_where_predicate_json(
            "bad_fn",
            r#"{ "type": "", "bounds": ["Clone"] }"#,
        );
        let result = CatalogueDocumentCodec::decode(&json, "usecase");
        assert!(
            matches!(result, Err(CatalogueDocumentCodecError::InvalidEntry { .. })),
            "expected InvalidEntry for empty WherePredicateDecl.lhs, got: {result:?}"
        );
    }

    #[test]
    fn test_decode_where_predicate_equal_with_multiple_rhs_returns_invalid_entry_error() {
        // `operator: Equal` requires exactly one RHS. Multiple RHS entries are invalid
        // (`where T::Assoc = U + V` is not valid Rust) and must be rejected at decode time.
        let json = usecase_function_with_where_predicate_json(
            "bad_fn",
            r#"{ "lhs": "T::Assoc", "rhs": ["u32", "String"], "operator": "Equal" }"#,
        );
        let result = CatalogueDocumentCodec::decode(&json, "usecase");
        assert!(
            matches!(result, Err(CatalogueDocumentCodecError::InvalidEntry { .. })),
            "expected InvalidEntry for Equal predicate with multiple rhs, got: {result:?}"
        );
    }

    #[test]
    fn test_encode_where_predicate_equal_with_multiple_rhs_returns_invalid_entry_error() {
        // `where_predicate_decl_to_dto` (encode.rs) must reject Equal predicates with
        // rhs.len() != 1, because the decoder enforces exactly one RHS for Equal.
        use domain::tddd::catalogue_v2::methods::BoundOp;

        let bad_predicate = WherePredicateDecl {
            lhs: TypeRef::new("T::Assoc".to_string()).unwrap(),
            rhs: vec![
                TypeRef::new("u32".to_string()).unwrap(),
                TypeRef::new("String".to_string()).unwrap(),
            ],
            operator: BoundOp::Equal,
        };
        let doc = usecase_function_doc_with_where_predicate("bad_eq_fn", bad_predicate);

        let result = CatalogueDocumentCodec::encode(&doc);
        assert!(
            matches!(result, Err(CatalogueDocumentCodecError::InvalidEntry { .. })),
            "expected InvalidEntry when encoding Equal WherePredicateDecl with multiple rhs, \
             got: {result:?}"
        );
    }

    #[test]
    fn test_encode_decode_round_trip_preserves_equal_where_predicate() {
        // A catalogue with `operator: Equal` must survive a decode → encode round-trip
        // with the Equal operator and single RHS intact.
        use domain::tddd::catalogue_v2::methods::BoundOp;

        let json = usecase_function_with_where_predicate_json(
            "eq_fn",
            r#"{ "lhs": "T::Assoc", "rhs": ["u32"], "operator": "Equal" }"#,
        );
        let doc = CatalogueDocumentCodec::decode(&json, "usecase").unwrap();
        let fn_entry = doc.functions.values().next().expect("expected one function entry");
        assert_eq!(fn_entry.where_predicates.len(), 1);
        let pred = &fn_entry.where_predicates[0];
        assert_eq!(pred.lhs.as_str(), "T::Assoc");
        assert_eq!(pred.rhs.len(), 1);
        assert_eq!(pred.rhs[0].as_str(), "u32");
        assert_eq!(pred.operator, BoundOp::Equal, "operator must survive round-trip");

        // Encode and check that operator appears in JSON output.
        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        assert!(
            encoded.contains("\"Equal\""),
            "encoded JSON must contain operator: Equal, got: {encoded}"
        );
    }

    #[test]
    fn test_encode_where_predicate_equal_with_bare_type_param_lhs_succeeds() {
        // Permissive principle (ADR make-catalogue-schema-permissive): Equal predicates
        // with a bare type param LHS (no `::`) must be accepted. Shape validation was
        // reverted; any syn-parseable LHS is valid at the codec boundary.
        use domain::tddd::catalogue_v2::methods::BoundOp;

        let predicate = WherePredicateDecl {
            lhs: TypeRef::new("T".to_string()).unwrap(),
            rhs: vec![TypeRef::new("u32".to_string()).unwrap()],
            operator: BoundOp::Equal,
        };
        let doc = usecase_function_doc_with_where_predicate("bare_lhs_eq_fn", predicate);

        let result = CatalogueDocumentCodec::encode(&doc);
        assert!(
            result.is_ok(),
            "permissive principle: bare type param lhs for Equal predicate must be accepted, \
             got: {result:?}"
        );
    }

    #[test]
    fn test_decode_where_predicate_equal_with_bare_type_param_lhs_succeeds() {
        // Permissive principle (ADR make-catalogue-schema-permissive): a bare type parameter
        // like `"T"` as the LHS of an Equal where-predicate must be accepted at decode time.
        // Shape validation (requiring `::`) was reverted; any syn-parseable expression is valid.
        use domain::tddd::catalogue_v2::methods::BoundOp;
        let json = usecase_function_with_where_predicate_json(
            "bare_lhs_fn",
            r#"{ "lhs": "T", "rhs": ["u32"], "operator": "Equal" }"#,
        );
        let result = CatalogueDocumentCodec::decode(&json, "usecase");
        assert!(
            result.is_ok(),
            "permissive principle: bare type param as Equal predicate lhs must be accepted, \
             got: {result:?}"
        );
        let doc = result.unwrap();
        let fn_entry = doc.functions.values().next().expect("expected one function entry");
        assert_eq!(fn_entry.where_predicates.len(), 1);
        assert_eq!(fn_entry.where_predicates[0].lhs.as_str(), "T");
        assert_eq!(fn_entry.where_predicates[0].operator, BoundOp::Equal);
    }

    // -----------------------------------------------------------------------
    // InherentImplDeclV2 (D2-schema): decode / encode / round-trip
    // -----------------------------------------------------------------------

    #[test]
    fn test_decode_inherent_impl_with_minimal_fields_succeeds() {
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {},
  "functions": {},
  "inherent_impls": [
    {
      "type_name": "Email",
      "methods": [
        { "name": "as_str", "receiver": "&self", "params": [], "returns": "str" }
      ]
    }
  ]
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        assert_eq!(doc.inherent_impls.len(), 1);
        assert_eq!(doc.inherent_impls[0].type_name.as_str(), "Email");
        assert_eq!(doc.inherent_impls[0].methods.len(), 1);
        assert_eq!(doc.inherent_impls[0].methods[0].name.as_str(), "as_str");
    }

    #[test]
    fn test_decode_inherent_impls_absent_defaults_to_empty_vec() {
        // Catalogues that predate InherentImplDeclV2 omit the field;
        // serde default must produce an empty Vec.
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {},
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        assert!(
            doc.inherent_impls.is_empty(),
            "omitted inherent_impls must default to empty Vec for backward compatibility"
        );
    }

    #[test]
    fn test_decode_one_struct_multiple_inherent_impl_blocks() {
        // The primary design constraint: 1 struct represented by N entries.
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {},
  "functions": {},
  "inherent_impls": [
    {
      "type_name": "Email",
      "methods": [
        { "name": "as_str", "receiver": "&self", "params": [], "returns": "str" }
      ]
    },
    {
      "type_name": "Email",
      "methods": [
        { "name": "validate", "receiver": "&self", "params": [], "returns": "Result<(), DomainError>" }
      ]
    }
  ]
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        assert_eq!(doc.inherent_impls.len(), 2, "two impl blocks must be decoded as two entries");
        assert_eq!(doc.inherent_impls[0].type_name.as_str(), "Email");
        assert_eq!(doc.inherent_impls[1].type_name.as_str(), "Email");
        assert_eq!(doc.inherent_impls[0].methods[0].name.as_str(), "as_str");
        assert_eq!(doc.inherent_impls[1].methods[0].name.as_str(), "validate");
    }

    #[test]
    fn test_decode_inherent_impl_with_generics_and_where_predicates_round_trips() {
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {},
  "functions": {},
  "inherent_impls": [
    {
      "type_name": "Container",
      "impl_generics": [
        { "name": "T", "bounds": ["Clone"] }
      ],
      "impl_where_predicates": [
        { "lhs": "Vec<T>", "rhs": ["Send"], "operator": "Bound" }
      ],
      "methods": []
    }
  ]
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        let impl_block = &doc.inherent_impls[0];
        assert_eq!(impl_block.type_name.as_str(), "Container");
        assert_eq!(impl_block.impl_generics.len(), 1);
        assert_eq!(impl_block.impl_generics[0].name.as_str(), "T");
        assert_eq!(impl_block.impl_generics[0].bounds[0].as_str(), "Clone");
        assert_eq!(impl_block.impl_where_predicates.len(), 1);
        assert_eq!(impl_block.impl_where_predicates[0].lhs.as_str(), "Vec<T>");

        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        let doc2 = CatalogueDocumentCodec::decode(&encoded, "domain").unwrap();
        assert_eq!(doc, doc2, "round-trip must preserve inherent_impl with generics");
    }

    #[test]
    fn test_encode_decode_inherent_impls_round_trip_preserves_multiple_blocks() {
        let json_in = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {},
  "functions": {},
  "inherent_impls": [
    {
      "type_name": "Email",
      "methods": [
        { "name": "as_str", "receiver": "&self", "params": [], "returns": "str" }
      ]
    },
    {
      "type_name": "Email",
      "methods": [
        { "name": "validate", "receiver": "&self", "params": [], "returns": "Result<(), DomainError>" }
      ]
    }
  ]
}"#;
        let doc = CatalogueDocumentCodec::decode(json_in, "domain").unwrap();
        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        let doc2 = CatalogueDocumentCodec::decode(&encoded, "domain").unwrap();
        assert_eq!(doc, doc2, "encode-decode round-trip must preserve inherent_impls");
        assert_eq!(doc2.inherent_impls.len(), 2);
    }

    #[test]
    fn test_encode_empty_inherent_impls_omits_field_from_json() {
        // When inherent_impls is empty the field must be omitted from the encoded JSON
        // so legacy catalogues stay byte-stable.
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {},
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        assert!(
            !encoded.contains("\"inherent_impls\""),
            "empty inherent_impls must be omitted from encoded JSON: {encoded}"
        );
    }

    #[test]
    fn test_decode_function_with_no_crate_prefix_returns_cross_crate_error() {
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {},
  "functions": {
    "create_user": {
      "action": "add",
      "role": "FreeFunction",
      "params": [],
      "returns": "()"
    }
  }
}"#;
        let result = CatalogueDocumentCodec::decode(json, "domain");
        assert!(
            matches!(result, Err(CatalogueDocumentCodecError::CrossCrateFunctionPath { .. })),
            "expected CrossCrateFunctionPath error for no-prefix path, got: {result:?}"
        );
    }

    // -----------------------------------------------------------------------
    // IN-06: TraitImplDeclV2 impl_generics + impl_where_predicates codec tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_decode_top_level_trait_impl_with_impl_generics_and_where_predicates_succeeds() {
        // ADR `2026-05-20-0048` D1/D2: top-level trait_impls; `impl<L, R, W> Trait for Foo<L, R, W> where L: Send`.
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "Foo": {
      "action": "add",
      "role": { "ValueObject": {} },
      "kind": { "kind": "struct", "shape": { "kind": "plain" } }
    }
  },
  "traits": {},
  "functions": {},
  "trait_impls": [
    {
      "trait_ref": "MyTrait",
      "for_type": "Foo",
      "impl_generics": [
        { "name": "L", "bounds": [] },
        { "name": "R", "bounds": [] },
        { "name": "W", "bounds": [] }
      ],
      "impl_where_predicates": [
        { "lhs": "L", "rhs": ["Send"], "operator": "Bound" }
      ]
    }
  ]
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        assert_eq!(doc.trait_impls.len(), 1);
        let impl_decl = &doc.trait_impls[0];
        assert_eq!(impl_decl.trait_ref.as_str(), "MyTrait");
        assert_eq!(impl_decl.for_type.as_str(), "Foo");
        assert_eq!(impl_decl.impl_generics.len(), 3);
        assert_eq!(impl_decl.impl_generics[0].name.as_str(), "L");
        assert_eq!(impl_decl.impl_generics[1].name.as_str(), "R");
        assert_eq!(impl_decl.impl_generics[2].name.as_str(), "W");
        assert_eq!(impl_decl.impl_where_predicates.len(), 1);
        assert_eq!(impl_decl.impl_where_predicates[0].lhs.as_str(), "L");
        assert_eq!(impl_decl.impl_where_predicates[0].rhs[0].as_str(), "Send");
    }

    #[test]
    fn test_decode_top_level_trait_impl_without_impl_generics_defaults_to_empty() {
        // Omitting impl_generics/impl_where_predicates must default to empty Vec (serde default).
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {},
  "functions": {},
  "trait_impls": [
    { "trait_ref": "core::fmt::Display", "for_type": "MyType" }
  ]
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        let impl_decl = &doc.trait_impls[0];
        assert!(
            impl_decl.impl_generics.is_empty(),
            "omitted impl_generics must default to empty Vec"
        );
        assert!(
            impl_decl.impl_where_predicates.is_empty(),
            "omitted impl_where_predicates must default to empty Vec"
        );
    }

    #[test]
    fn test_encode_decode_round_trip_preserves_top_level_trait_impl_generics_and_where_predicates()
    {
        // ADR `2026-05-20-0048` D1/D2 round-trip: decode → encode → decode must be stable.
        use domain::tddd::catalogue_v2::methods::BoundOp;

        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "Foo": {
      "action": "add",
      "role": { "ValueObject": {} },
      "kind": { "kind": "struct", "shape": { "kind": "plain" } }
    }
  },
  "traits": {},
  "functions": {},
  "trait_impls": [
    {
      "trait_ref": "MyTrait",
      "for_type": "Foo",
      "impl_generics": [
        { "name": "L", "bounds": ["Send"] },
        { "name": "R", "bounds": [] }
      ],
      "impl_where_predicates": [
        { "lhs": "L", "rhs": ["Clone"], "operator": "Bound" }
      ]
    },
    {
      "trait_ref": "core::fmt::Display",
      "for_type": "Foo"
    }
  ]
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        let doc2 = CatalogueDocumentCodec::decode(&encoded, "domain").unwrap();
        assert_eq!(doc, doc2, "encode-decode round-trip must be stable for impl_generics/where");

        let generic_impl = &doc2.trait_impls[0];
        assert_eq!(generic_impl.trait_ref.as_str(), "MyTrait");
        assert_eq!(generic_impl.for_type.as_str(), "Foo");
        assert_eq!(generic_impl.impl_generics.len(), 2);
        assert_eq!(generic_impl.impl_generics[0].name.as_str(), "L");
        assert_eq!(generic_impl.impl_generics[0].bounds[0].as_str(), "Send");
        assert_eq!(generic_impl.impl_generics[1].name.as_str(), "R");
        assert_eq!(generic_impl.impl_where_predicates.len(), 1);
        assert_eq!(generic_impl.impl_where_predicates[0].lhs.as_str(), "L");
        assert_eq!(generic_impl.impl_where_predicates[0].rhs[0].as_str(), "Clone");
        assert_eq!(generic_impl.impl_where_predicates[0].operator, BoundOp::Bound);

        // The second trait impl (Display) must have empty impl_generics/where_predicates.
        let display_impl = &doc2.trait_impls[1];
        assert!(display_impl.impl_generics.is_empty());
        assert!(display_impl.impl_where_predicates.is_empty());
    }

    #[test]
    fn test_encode_top_level_trait_impl_with_empty_impl_generics_omits_field() {
        // `skip_serializing_if = "Vec::is_empty"` must suppress impl_generics and
        // impl_where_predicates from the encoded JSON when they are empty.
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {},
  "functions": {},
  "trait_impls": [
    { "trait_ref": "core::fmt::Display", "for_type": "Simple" }
  ]
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        // The encoded JSON must NOT contain impl_generics or impl_where_predicates for
        // this simple impl (both are empty Vecs → skip_serializing_if suppresses them).
        assert!(
            !encoded.contains("\"impl_generics\""),
            "impl_generics must be omitted when empty: {encoded}"
        );
        assert!(
            !encoded.contains("\"impl_where_predicates\""),
            "impl_where_predicates must be omitted when empty: {encoded}"
        );
    }

    // -----------------------------------------------------------------------
    // ADR 2026-05-25-0000 D16: DataRole::EventPolicy codec tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_decode_event_policy_type_role_with_non_empty_reacts_to_succeeds() {
        // D16: EventPolicy.reacts_to is a NonEmptyVec<TypeRef>. Happy-path: a valid
        // list of event type references must decode without error.
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "OrderPolicyHandler": {
      "action": "add",
      "role": { "EventPolicy": { "reacts_to": ["OrderPlaced", "OrderCancelled"] } },
      "kind": { "kind": "struct", "shape": { "kind": "unit" } },
      "methods": []
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        let entry = doc.types.values().next().unwrap();
        match &entry.role {
            domain::tddd::catalogue_v2::roles::DataRole::EventPolicy { reacts_to } => {
                let slice = reacts_to.as_slice();
                assert_eq!(slice.len(), 2);
                assert_eq!(slice[0].as_str(), "OrderPlaced");
                assert_eq!(slice[1].as_str(), "OrderCancelled");
            }
            other => panic!("expected EventPolicy role, got: {other:?}"),
        }
    }

    #[test]
    fn test_encode_decode_round_trip_preserves_event_policy_role() {
        // D16: encode → decode round-trip must preserve EventPolicy.reacts_to exactly.
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "PaymentPolicyHandler": {
      "action": "add",
      "role": { "EventPolicy": { "reacts_to": ["PaymentReceived"] } },
      "kind": { "kind": "struct", "shape": { "kind": "unit" } },
      "methods": []
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        let doc2 = CatalogueDocumentCodec::decode(&encoded, "domain").unwrap();
        assert_eq!(doc, doc2, "round-trip must preserve EventPolicy role");
        // Encoded JSON must use discriminated-object form, not bare string.
        assert!(
            encoded.contains("\"EventPolicy\""),
            "encoded JSON must use discriminated-object form for EventPolicy: {encoded}"
        );
        assert!(
            encoded.contains("\"reacts_to\""),
            "encoded JSON must contain reacts_to field: {encoded}"
        );
    }

    #[test]
    fn test_decode_event_policy_with_empty_reacts_to_returns_invalid_entry_error() {
        // D16: EventPolicy.reacts_to must be non-empty (NonEmptyVec). An empty array
        // must be rejected at the codec boundary with InvalidEntry.
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "EmptyPolicyHandler": {
      "action": "add",
      "role": { "EventPolicy": { "reacts_to": [] } },
      "kind": { "kind": "struct", "shape": { "kind": "unit" } },
      "methods": []
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let result = CatalogueDocumentCodec::decode(json, "domain");
        assert!(
            matches!(result, Err(CatalogueDocumentCodecError::InvalidEntry { .. })),
            "empty EventPolicy.reacts_to must be rejected with InvalidEntry, got: {result:?}"
        );
    }

    // -----------------------------------------------------------------------
    // ADR 2026-05-25-0000 D10: ContractRole::Repository codec tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_decode_repository_trait_role_with_aggregate_succeeds() {
        // D10: ContractRole::Repository requires a non-empty aggregate TypeRef. Happy-path:
        // a valid aggregate type reference must decode without error.
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {
    "OrderRepository": {
      "action": "add",
      "role": { "Repository": { "aggregate": "Order" } },
      "methods": []
    }
  },
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        let entry = doc.traits.values().next().unwrap();
        match &entry.role {
            domain::tddd::catalogue_v2::roles::ContractRole::Repository { aggregate } => {
                assert_eq!(aggregate.as_str(), "Order");
            }
            other => panic!("expected Repository role, got: {other:?}"),
        }
    }

    #[test]
    fn test_encode_decode_round_trip_preserves_repository_role() {
        // D10: encode → decode round-trip must preserve Repository.aggregate exactly.
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {
    "UserRepository": {
      "action": "add",
      "role": { "Repository": { "aggregate": "User" } },
      "methods": []
    }
  },
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        let doc2 = CatalogueDocumentCodec::decode(&encoded, "domain").unwrap();
        assert_eq!(doc, doc2, "round-trip must preserve Repository role");
        // Encoded JSON must use discriminated-object form, not bare string.
        assert!(
            encoded.contains("\"Repository\""),
            "encoded JSON must use discriminated-object form for Repository: {encoded}"
        );
        assert!(
            encoded.contains("\"aggregate\""),
            "encoded JSON must contain aggregate field: {encoded}"
        );
    }

    #[test]
    fn test_decode_repository_with_empty_aggregate_returns_invalid_entry_error() {
        // D10: Repository.aggregate must be a non-empty TypeRef. An empty string must
        // be rejected at the codec boundary with InvalidEntry.
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {
    "BadRepository": {
      "action": "add",
      "role": { "Repository": { "aggregate": "" } },
      "methods": []
    }
  },
  "functions": {}
}"#;
        let result = CatalogueDocumentCodec::decode(json, "domain");
        assert!(
            matches!(result, Err(CatalogueDocumentCodecError::InvalidEntry { .. })),
            "empty Repository.aggregate must be rejected with InvalidEntry, got: {result:?}"
        );
    }

    // -----------------------------------------------------------------------
    // ADR 2026-05-25-0000: DataRole payload variant round-trip tests (T011)
    // -----------------------------------------------------------------------

    #[test]
    fn test_decode_value_object_with_empty_invariants_round_trips() {
        // ValueObject with no invariants: invariants field is omitted (skip_serializing_if).
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "Money": {
      "action": "add",
      "role": { "ValueObject": {} },
      "kind": { "kind": "struct", "shape": { "kind": "plain", "fields": [] } },
      "methods": []
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        let entry = doc.types.values().next().unwrap();
        assert_eq!(entry.role, DataRole::value_object());
        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        let doc2 = CatalogueDocumentCodec::decode(&encoded, "domain").unwrap();
        assert_eq!(doc, doc2);
    }

    #[test]
    fn test_decode_value_object_with_invariant_decl_round_trips() {
        // ValueObject with one non-empty invariants payload must survive a round-trip.
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "Email": {
      "action": "add",
      "role": {
        "ValueObject": {
          "invariants": [
            { "name": "email_is_valid", "predicate": { "SelfMethod": "is_email_valid" } }
          ]
        }
      },
      "kind": { "kind": "struct", "shape": { "kind": "tuple", "fields": ["String"] } },
      "methods": []
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        let entry = doc.types.values().next().unwrap();
        match &entry.role {
            DataRole::ValueObject { invariants } => {
                assert_eq!(invariants.len(), 1);
                assert_eq!(invariants[0].name.as_str(), "email_is_valid");
            }
            other => panic!("expected ValueObject role, got: {other:?}"),
        }
        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        let doc2 = CatalogueDocumentCodec::decode(&encoded, "domain").unwrap();
        assert_eq!(doc, doc2);
    }

    #[test]
    fn test_decode_entity_role_with_identity_and_invariants_round_trips() {
        // Entity requires identity; invariants are optional.
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "User": {
      "action": "add",
      "role": {
        "Entity": {
          "identity": { "method_name": "id" },
          "invariants": [
            { "name": "email_is_valid", "predicate": { "SelfMethod": "validate_email" } }
          ]
        }
      },
      "kind": { "kind": "struct", "shape": { "kind": "plain", "fields": [] } },
      "methods": []
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        let entry = doc.types.values().next().unwrap();
        match &entry.role {
            DataRole::Entity { identity, invariants } => {
                assert_eq!(identity.method_name().as_str(), "id");
                assert_eq!(invariants.len(), 1);
                assert_eq!(invariants[0].name.as_str(), "email_is_valid");
            }
            other => panic!("expected Entity role, got: {other:?}"),
        }
        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        let doc2 = CatalogueDocumentCodec::decode(&encoded, "domain").unwrap();
        assert_eq!(doc, doc2);
    }

    #[test]
    fn test_decode_aggregate_root_role_with_all_payload_fields_round_trips() {
        // AggregateRoot with all optional payload fields populated.
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "Order": {
      "action": "add",
      "role": {
        "AggregateRoot": {
          "identity": { "method_name": "order_id" },
          "invariants": [
            { "name": "total_is_positive", "predicate": { "SelfMethod": "is_total_positive" } }
          ],
          "exclusive_members": ["OrderLine"],
          "shared_value_objects": ["Money"],
          "emits": ["OrderPlaced"]
        }
      },
      "kind": { "kind": "struct", "shape": { "kind": "plain", "fields": [] } },
      "methods": []
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        let entry = doc.types.values().next().unwrap();
        match &entry.role {
            DataRole::AggregateRoot {
                identity,
                invariants,
                exclusive_members,
                shared_value_objects,
                emits,
            } => {
                assert_eq!(identity.method_name().as_str(), "order_id");
                assert_eq!(invariants.len(), 1);
                assert_eq!(exclusive_members.len(), 1);
                assert_eq!(exclusive_members[0].as_str(), "OrderLine");
                assert_eq!(shared_value_objects.len(), 1);
                assert_eq!(shared_value_objects[0].as_str(), "Money");
                assert_eq!(emits.len(), 1);
                assert_eq!(emits[0].as_str(), "OrderPlaced");
            }
            other => panic!("expected AggregateRoot role, got: {other:?}"),
        }
        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        let doc2 = CatalogueDocumentCodec::decode(&encoded, "domain").unwrap();
        assert_eq!(doc, doc2);
    }

    #[test]
    fn test_decode_domain_service_role_with_emits_round_trips() {
        // DomainService.emits is optional; when provided it must survive round-trip.
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "PricingService": {
      "action": "add",
      "role": { "DomainService": { "emits": ["PriceCalculated"] } },
      "kind": { "kind": "struct", "shape": { "kind": "unit" } },
      "methods": []
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        let entry = doc.types.values().next().unwrap();
        match &entry.role {
            DataRole::DomainService { emits } => {
                assert_eq!(emits.len(), 1);
                assert_eq!(emits[0].as_str(), "PriceCalculated");
            }
            other => panic!("expected DomainService role, got: {other:?}"),
        }
        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        let doc2 = CatalogueDocumentCodec::decode(&encoded, "domain").unwrap();
        assert_eq!(doc, doc2);
    }

    #[test]
    fn test_decode_use_case_role_with_handles_round_trips() {
        // UseCase.handles is optional; when provided it must survive round-trip.
        let json = r#"{
  "schema_version": 5,
  "crate_name": "usecase",
  "layer": "usecase",
  "types": {
    "RegisterUserUseCase": {
      "action": "add",
      "role": { "UseCase": { "handles": ["RegisterUserCommand"] } },
      "kind": { "kind": "struct", "shape": { "kind": "unit" } },
      "methods": []
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "usecase").unwrap();
        let entry = doc.types.values().next().unwrap();
        match &entry.role {
            DataRole::UseCase { handles } => {
                assert_eq!(handles.len(), 1);
                assert_eq!(handles[0].as_str(), "RegisterUserCommand");
            }
            other => panic!("expected UseCase role, got: {other:?}"),
        }
        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        let doc2 = CatalogueDocumentCodec::decode(&encoded, "usecase").unwrap();
        assert_eq!(doc, doc2);
    }

    // ADR 2026-05-25-0000 D16 / T013: DataRole::DomainEvent codec round-trip
    #[test]
    fn test_decode_domain_event_data_role_round_trips() {
        // DomainEvent is a unit variant (no payload fields); the JSON form is `{"DomainEvent": {}}`.
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "UserRegistered": {
      "action": "add",
      "role": { "DomainEvent": {} },
      "kind": { "kind": "struct", "shape": { "kind": "unit" } },
      "methods": []
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        let entry = doc.types.values().next().unwrap();
        assert_eq!(
            entry.role,
            domain::tddd::catalogue_v2::roles::DataRole::DomainEvent,
            "decoded role must be DomainEvent"
        );
        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        assert!(
            encoded.contains("\"DomainEvent\""),
            "encoded JSON must use DomainEvent discriminant key: {encoded}"
        );
        let doc2 = CatalogueDocumentCodec::decode(&encoded, "domain").unwrap();
        assert_eq!(doc, doc2, "round-trip must preserve DomainEvent role");
    }

    #[test]
    fn test_decode_repository_trait_role_with_missing_aggregate_field_returns_error() {
        // D10: Repository requires the `aggregate` field. Omitting it must fail decode.
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {
    "MissingAggregateRepo": {
      "action": "add",
      "role": { "Repository": {} },
      "methods": []
    }
  },
  "functions": {}
}"#;
        let result = CatalogueDocumentCodec::decode(json, "domain");
        assert!(
            result.is_err(),
            "Repository role with missing aggregate field must fail decode, got: {result:?}"
        );
    }

    #[test]
    fn test_decode_data_role_unknown_variant_returns_error() {
        // DataRoleDto uses deny_unknown_fields; an unknown role key must be rejected.
        let json = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "Foo": {
      "action": "add",
      "role": { "UnknownRole": {} },
      "kind": { "kind": "struct", "shape": { "kind": "unit" } },
      "methods": []
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let result = CatalogueDocumentCodec::decode(json, "domain");
        assert!(
            result.is_err(),
            "unknown DataRole variant must be rejected by deny_unknown_fields, got: {result:?}"
        );
    }
}
