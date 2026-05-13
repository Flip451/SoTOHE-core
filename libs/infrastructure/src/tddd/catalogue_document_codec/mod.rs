//! JSON codec for [`CatalogueDocument`] (schema_version = 3).
//!
//! `CatalogueDocumentCodec` converts between `CatalogueDocument` domain types
//! and JSON using serde DTO structs defined here in the infrastructure layer.
//! The domain layer is serialization-free (ADR `2026-04-14-1531-domain-serde-ripout.md`).
//!
//! ## Wire format (schema_version = 3)
//!
//! ```json
//! {
//!   "schema_version": 3,
//!   "crate_name": "domain",
//!   "layer": "domain",
//!   "types": { "TypeName": { ... } },
//!   "traits": { "TraitName": { ... } },
//!   "functions": { "crate::fn_name": { ... } }
//! }
//! ```
//!
//! `CatalogueDocument.schema_version` is always `3` (the domain schema_version field
//! carries the wire format version; older v1/v2 formats are rejected by this codec).
//!
//! ## Error variants
//!
//! - `Json(serde_json::Error)` — serde deserialization failed.
//! - `Io(std::io::Error)` — file I/O failed.
//! - `UnsupportedSchemaVersion { actual, expected }` — version mismatch.
//! - `InvalidEntry { entry_name, reason }` — an entry's fields failed validation.
//! - `CrateNameMismatch { expected, actual }` — `crate_name` field vs filename stem.

use std::collections::BTreeMap;
use std::path::Path;

use domain::tddd::catalogue_v2::CatalogueDocument;
use serde::{Deserialize, de};
use thiserror::Error;

mod decode;
mod dto;
mod encode;

use decode::dto_to_domain;
use dto::SchemaVersionProbe;
use encode::domain_to_dto;

// ---------------------------------------------------------------------------
// Supported schema version
// ---------------------------------------------------------------------------

/// The schema version this codec reads and writes.
pub const SCHEMA_VERSION: u32 = 3;

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

    /// The JSON file's `schema_version` does not match [`SCHEMA_VERSION`].
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
    /// Returns `CatalogueDocumentCodecError::UnsupportedSchemaVersion` if
    /// the `schema_version` field is not [`SCHEMA_VERSION`].
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
        if version_probe.schema_version != SCHEMA_VERSION {
            return Err(CatalogueDocumentCodecError::UnsupportedSchemaVersion {
                actual: version_probe.schema_version,
                expected: SCHEMA_VERSION,
            });
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
    /// Returns `CatalogueDocumentCodecError::Json` if serialization fails
    /// (this is extremely unlikely for valid domain types, but the error variant
    /// is kept for API completeness).
    pub fn encode(doc: &CatalogueDocument) -> Result<String, CatalogueDocumentCodecError> {
        let dto = domain_to_dto(doc);
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
fn derive_filename_stem(path: &Path) -> String {
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
    use domain::tddd::catalogue_v2::ItemAction;
    use domain::tddd::catalogue_v2::composite::TypeKindV2;
    use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole};

    fn minimal_v3_json(crate_name: &str, layer: &str) -> String {
        format!(
            r#"{{
  "schema_version": 3,
  "crate_name": "{crate_name}",
  "layer": "{layer}",
  "types": {{}},
  "traits": {{}},
  "functions": {{}}
}}"#
        )
    }

    #[test]
    fn test_decode_minimal_v3_json_succeeds() {
        let json = minimal_v3_json("domain", "domain");
        let doc = CatalogueDocumentCodec::decode(&json, "domain").unwrap();
        assert_eq!(doc.schema_version, 3);
        assert_eq!(doc.crate_name.as_str(), "domain");
        assert!(doc.types.is_empty());
    }

    #[test]
    fn test_decode_wrong_schema_version_returns_unsupported_schema_version() {
        let json = r#"{"schema_version": 2, "crate_name": "domain", "layer": "domain"}"#;
        let err = CatalogueDocumentCodec::decode(json, "domain").unwrap_err();
        assert!(
            matches!(
                err,
                CatalogueDocumentCodecError::UnsupportedSchemaVersion { actual: 2, expected: 3 }
            ),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn test_decode_crate_name_mismatch_returns_error() {
        let json = minimal_v3_json("domain", "domain");
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
  "schema_version": 3,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "UserId": {
      "action": "add",
      "role": "ValueObject",
      "kind": { "kind": "plain_struct", "fields": [] }
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        assert_eq!(doc.types.len(), 1);
        let entry = doc.types.values().next().unwrap();
        assert_eq!(entry.action, ItemAction::Add);
        assert_eq!(entry.role, DataRole::ValueObject);
        assert!(matches!(entry.kind, TypeKindV2::PlainStruct { .. }));
    }

    #[test]
    fn test_decode_type_entry_with_unit_struct_kind() {
        let json = r#"{
  "schema_version": 3,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "Marker": {
      "action": "add",
      "role": "ValueObject",
      "kind": { "kind": "unit_struct" }
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        assert_eq!(doc.types.len(), 1);
        let entry = doc.types.values().next().unwrap();
        assert!(matches!(entry.kind, TypeKindV2::UnitStruct));
    }

    #[test]
    fn test_decode_type_entry_with_tuple_struct_kind() {
        let json = r#"{
  "schema_version": 3,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "UserId": {
      "action": "add",
      "role": "ValueObject",
      "kind": { "kind": "tuple_struct", "fields": ["String"] }
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        let entry = doc.types.values().next().unwrap();
        match &entry.kind {
            TypeKindV2::TupleStruct { fields, has_stripped_fields } => {
                assert_eq!(fields.len(), 1);
                assert_eq!(fields[0].as_str(), "String");
                assert!(!has_stripped_fields);
            }
            _ => panic!("expected TupleStruct"),
        }
    }

    #[test]
    fn test_decode_type_entry_with_plain_struct_and_typestate_marker() {
        let json = r#"{
  "schema_version": 3,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "IdleState": {
      "action": "add",
      "role": "ValueObject",
      "kind": {
        "kind": "plain_struct",
        "fields": [],
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
            TypeKindV2::PlainStruct { typestate: Some(ts), .. } => {
                assert_eq!(ts.state_name().as_str(), "ReviewMachine");
                assert_eq!(ts.transitions().transition_methods().len(), 1);
            }
            _ => panic!("expected PlainStruct with typestate"),
        }
    }

    #[test]
    fn test_decode_trait_entry_with_secondary_port_role() {
        let json = r#"{
  "schema_version": 3,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {
    "UserRepository": {
      "action": "add",
      "role": "SecondaryPort",
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

    #[test]
    fn test_encode_decode_round_trip_preserves_data() {
        let json = r#"{
  "schema_version": 3,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "UserId": {
      "action": "add",
      "role": "ValueObject",
      "kind": { "kind": "tuple_struct", "fields": ["String"] },
      "methods": [],
      "trait_impls": [],
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
  "schema_version": 3,
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
    fn test_decode_trait_impl_with_generic_args_present() {
        let json = r#"{
  "schema_version": 3,
  "crate_name": "usecase",
  "layer": "usecase",
  "types": {
    "RenderContractMapError": {
      "action": "modify",
      "role": "ErrorType",
      "kind": { "kind": "enum", "variants": [] },
      "trait_impls": [
        { "trait_name": "From", "origin_crate": "core", "generic_args": "CatalogueLoaderError" },
        { "trait_name": "From", "origin_crate": "core", "generic_args": "ContractMapWriterError" },
        { "trait_name": "Display", "origin_crate": "core" }
      ]
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "usecase").unwrap();
        let entry = doc.types.values().next().unwrap();
        assert_eq!(entry.trait_impls.len(), 3);
        assert_eq!(entry.trait_impls[0].generic_args(), Some("CatalogueLoaderError"));
        assert_eq!(entry.trait_impls[1].generic_args(), Some("ContractMapWriterError"));
        assert_eq!(entry.trait_impls[2].generic_args(), None);
    }

    #[test]
    fn test_decode_trait_impl_missing_generic_args_defaults_to_none() {
        // Catalogues that predate generic_args omit the field; default to None for compat.
        let json = r#"{
  "schema_version": 3,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "OldErrorType": {
      "action": "add",
      "role": "ErrorType",
      "kind": { "kind": "enum", "variants": [] },
      "trait_impls": [
        { "trait_name": "From", "origin_crate": "core" }
      ]
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "domain").unwrap();
        let entry = doc.types.values().next().unwrap();
        assert_eq!(entry.trait_impls.len(), 1);
        assert_eq!(entry.trait_impls[0].trait_name.as_str(), "From");
        assert_eq!(entry.trait_impls[0].generic_args(), None);
    }

    #[test]
    fn test_encode_decode_round_trip_preserves_generic_args() {
        let json = r#"{
  "schema_version": 3,
  "crate_name": "usecase",
  "layer": "usecase",
  "types": {
    "MyError": {
      "action": "add",
      "role": "ErrorType",
      "kind": { "kind": "enum", "variants": [] },
      "trait_impls": [
        { "trait_name": "From", "origin_crate": "core", "generic_args": "IoError" },
        { "trait_name": "Display", "origin_crate": "core" }
      ]
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "usecase").unwrap();
        let encoded = CatalogueDocumentCodec::encode(&doc).unwrap();
        let doc2 = CatalogueDocumentCodec::decode(&encoded, "usecase").unwrap();
        assert_eq!(doc, doc2);
        let entry = doc2.types.values().next().unwrap();
        assert_eq!(entry.trait_impls[0].generic_args(), Some("IoError"));
        assert_eq!(entry.trait_impls[1].generic_args(), None);
    }

    #[test]
    fn test_decode_trait_impl_with_empty_generic_args_returns_invalid_entry_error() {
        // `generic_args: ""` must be rejected — empty string is not a valid type argument.
        let json = r#"{
  "schema_version": 3,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "BadError": {
      "action": "add",
      "role": "ErrorType",
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
            matches!(result, Err(CatalogueDocumentCodecError::InvalidEntry { .. })),
            "expected InvalidEntry for empty generic_args, got: {result:?}"
        );
    }

    #[test]
    fn test_decode_trait_impl_with_angle_bracketed_generic_args_returns_invalid_entry_error() {
        // `generic_args: "<T>"` must be rejected — the caller must pass the bare type name.
        let json = r#"{
  "schema_version": 3,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "BadError": {
      "action": "add",
      "role": "ErrorType",
      "kind": { "kind": "enum", "variants": [] },
      "trait_impls": [
        { "trait_name": "From", "origin_crate": "core", "generic_args": "<T>" }
      ]
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let result = CatalogueDocumentCodec::decode(json, "domain");
        assert!(
            matches!(result, Err(CatalogueDocumentCodecError::InvalidEntry { .. })),
            "expected InvalidEntry for angle-bracketed generic_args, got: {result:?}"
        );
    }

    #[test]
    fn test_decode_unit_struct_has_no_fields_variant_so_illegal_state_is_structurally_impossible() {
        // UnitStruct has no `fields` field in the DTO — the schema enforces absence of fields
        // at the structural level. An old-format `kind: "struct", pattern: "unit"` entry would
        // fail with an unknown-variant error because "struct" is no longer a valid kind tag.
        let json = r#"{
  "schema_version": 3,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "Marker": {
      "action": "add",
      "role": "ValueObject",
      "kind": { "kind": "unit_struct" }
    }
  },
  "traits": {},
  "functions": {}
}"#;
        let result = CatalogueDocumentCodec::decode(json, "domain");
        assert!(result.is_ok(), "unit_struct decodes successfully: {result:?}");
        let entry = result.unwrap().types.into_values().next().unwrap();
        assert!(matches!(entry.kind, TypeKindV2::UnitStruct));
    }

    #[test]
    fn test_decode_method_with_empty_generic_param_name_returns_invalid_entry_error() {
        // An empty generic param name must be rejected at the codec boundary so that
        // `CatalogueDocumentCodec` fails closed rather than propagating a malformed string
        // to `catalogue_to_extended_crate_codec`.
        let json = r#"{
  "schema_version": 3,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {
    "MyPort": {
      "action": "add",
      "role": "SpecificationPort",
      "methods": [
        {
          "name": "do_it",
          "returns": "Result",
          "generics": [{ "name": "", "bounds": ["Send"] }]
        }
      ]
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
    fn test_decode_method_with_empty_generic_bound_returns_invalid_entry_error() {
        // An empty bound string in generics[].bounds must be rejected at the codec boundary.
        let json = r#"{
  "schema_version": 3,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {
    "MyPort": {
      "action": "add",
      "role": "SpecificationPort",
      "methods": [
        {
          "name": "do_it",
          "returns": "Result",
          "generics": [{ "name": "T", "bounds": [""] }]
        }
      ]
    }
  },
  "functions": {}
}"#;
        let result = CatalogueDocumentCodec::decode(json, "domain");
        assert!(
            matches!(result, Err(CatalogueDocumentCodecError::InvalidEntry { .. })),
            "expected InvalidEntry for empty generic bound, got: {result:?}"
        );
    }

    #[test]
    fn test_decode_method_with_invalid_generic_param_name_returns_invalid_entry_error() {
        // Generic param names like "T-U" or "T::X" must be rejected at the codec boundary.
        for bad_name in &["T-U", "T::X", "<T>", "123T", "T U", ""] {
            let json = format!(
                r#"{{
  "schema_version": 3,
  "crate_name": "domain",
  "layer": "domain",
  "types": {{
    "Foo": {{
      "action": "add",
      "role": "ValueObject",
      "kind": {{ "kind": "plain_struct", "fields": [] }},
      "methods": [
        {{
          "name": "do_something",
          "params": [],
          "returns": "()",
          "generics": [{{"name": "{bad_name}", "bounds": []}}]
        }}
      ],
      "trait_impls": []
    }}
  }},
  "traits": {{}},
  "functions": {{}}
}}"#
            );
            let result = CatalogueDocumentCodec::decode(&json, "domain");
            assert!(
                matches!(result, Err(CatalogueDocumentCodecError::InvalidEntry { .. })),
                "expected InvalidEntry for bad generic name '{bad_name}', got: {result:?}"
            );
        }
    }

    #[test]
    fn test_decode_trait_with_empty_supertrait_bound_returns_invalid_entry_error() {
        // An empty supertrait_bounds entry must be rejected at the codec boundary.
        let json = r#"{
  "schema_version": 3,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {
    "MyPort": {
      "action": "add",
      "role": "SpecificationPort",
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
  "schema_version": 3,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "UserId": {
      "action": "add",
      "role": "ValueObject",
      "kind": { "kind": "unit_struct" },
      "spec_refs": [
        { "file": "track/items/x/spec.json", "anchor": "IN-01", "hash": "0000000000000000000000000000000000000000000000000000000000000000" }
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
  "schema_version": 3,
  "crate_name": "domain",
  "layer": "domain",
  "types": {},
  "traits": {
    "UserRepository": {
      "action": "add",
      "role": "SecondaryPort",
      "methods": [],
      "spec_refs": [
        { "file": "track/items/x/spec.json", "anchor": "AC-02", "hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" }
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
    fn test_function_entry_round_trip_with_spec_refs_and_informal_grounds() {
        let json = r#"{
  "schema_version": 3,
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
        { "file": "track/items/x/spec.json", "anchor": "UC-01", "hash": "1111111111111111111111111111111111111111111111111111111111111111" }
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
  "schema_version": 3,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "Marker": {
      "action": "add",
      "role": "ValueObject",
      "kind": { "kind": "unit_struct" }
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

    // --- T012 cross-crate function path tests ---

    #[test]
    fn test_decode_function_with_own_crate_prefix_succeeds() {
        let json = r#"{
  "schema_version": 3,
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
  "schema_version": 3,
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
  "schema_version": 3,
  "crate_name": "usecase",
  "layer": "usecase",
  "types": {},
  "traits": {
    "Describable": {
      "action": "add",
      "role": "SpecificationPort",
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
    fn test_decode_trait_method_without_has_default_impl_field_defaults_to_false() {
        // Older catalogues that predate ADR 0248 D13 omit the field; default to false
        // so existing required-method declarations remain correctly classified.
        let json = r#"{
  "schema_version": 3,
  "crate_name": "usecase",
  "layer": "usecase",
  "types": {},
  "traits": {
    "RequiredOps": {
      "action": "add",
      "role": "SpecificationPort",
      "methods": [
        {
          "name": "do_op",
          "receiver": "&self",
          "params": [],
          "returns": "()",
          "is_async": false
        }
      ]
    }
  },
  "functions": {}
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "usecase").unwrap();
        let trait_entry = doc.traits.values().next().unwrap();
        assert!(
            !trait_entry.methods[0].has_default_impl,
            "omitted has_default_impl must default to false"
        );
    }

    #[test]
    fn test_encode_decode_round_trip_preserves_has_default_impl() {
        let json = r#"{
  "schema_version": 3,
  "crate_name": "usecase",
  "layer": "usecase",
  "types": {},
  "traits": {
    "MixedTrait": {
      "action": "add",
      "role": "SpecificationPort",
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
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "usecase").unwrap();
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
        let json = r#"{
  "schema_version": 3,
  "crate_name": "usecase",
  "layer": "usecase",
  "types": {},
  "traits": {
    "MixedTrait": {
      "action": "add",
      "role": "SpecificationPort",
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
}"#;
        let doc = CatalogueDocumentCodec::decode(json, "usecase").unwrap();
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
  "schema_version": 3,
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
  "schema_version": 3,
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
  "schema_version": 3,
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
  "schema_version": 3,
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
  "schema_version": 3,
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

    #[test]
    fn test_decode_function_with_no_crate_prefix_returns_cross_crate_error() {
        let json = r#"{
  "schema_version": 3,
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
}
