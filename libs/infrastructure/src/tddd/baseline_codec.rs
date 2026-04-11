//! Serde codec for `domain-types-baseline.json` (`TypeBaseline` SSoT).
//!
//! The JSON schema uses type/trait names as object keys (HashMap-natural)
//! with a `schema_version` and `captured_at` envelope.

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::marker::PhantomData;

use domain::schema::TypeKind;
use domain::{Timestamp, TraitBaselineEntry, TypeBaseline, TypeBaselineEntry, ValidationError};
use serde::de::{Error as _, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

// ---------------------------------------------------------------------------
// Duplicate-key-rejecting map deserializer
// ---------------------------------------------------------------------------

/// Deserializes a JSON object into a `BTreeMap` while rejecting duplicate keys.
///
/// By default, `serde_json` silently keeps the last value when an object has
/// duplicate keys. This helper treats duplicates as a deserialization error so
/// that malformed baseline artifacts are surfaced rather than silently truncated.
fn deserialize_no_duplicate_keys<'de, D, V>(
    deserializer: D,
) -> Result<BTreeMap<String, V>, D::Error>
where
    D: Deserializer<'de>,
    V: Deserialize<'de>,
{
    struct NoDupMapVisitor<V>(PhantomData<V>);

    impl<'de, V: Deserialize<'de>> Visitor<'de> for NoDupMapVisitor<V> {
        type Value = BTreeMap<String, V>;

        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a JSON object with unique keys")
        }

        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut result = BTreeMap::new();
            while let Some(key) = map.next_key::<String>()? {
                let value = map.next_value::<V>()?;
                if result.contains_key(&key) {
                    return Err(A::Error::custom(format!("duplicate key `{key}`")));
                }
                result.insert(key, value);
            }
            Ok(result)
        }
    }

    deserializer.deserialize_map(NoDupMapVisitor(PhantomData))
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Codec error for domain-types-baseline.json.
#[derive(Debug, thiserror::Error)]
pub enum BaselineCodecError {
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("unsupported schema_version: expected 1, got {0}")]
    UnsupportedSchemaVersion(u32),

    #[error("invalid timestamp: {0}")]
    InvalidTimestamp(#[from] ValidationError),
}

// ---------------------------------------------------------------------------
// DTO types
// ---------------------------------------------------------------------------

/// Serialization uses `BTreeMap` so that JSON keys are always written in sorted order,
/// producing a deterministic committed artifact.
///
/// Both `types` and `traits` are required fields (no `#[serde(default)]`) so that a
/// truncated or partially-written artifact is rejected at decode time rather than
/// silently treated as an empty baseline. The `deserialize_with` attribute ensures
/// that duplicate type or trait names within the JSON object are rejected as errors
/// rather than silently resolved to the last value.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BaselineDto {
    schema_version: u32,
    captured_at: String,
    #[serde(deserialize_with = "deserialize_no_duplicate_keys")]
    types: BTreeMap<String, TypeEntryDto>,
    #[serde(deserialize_with = "deserialize_no_duplicate_keys")]
    traits: BTreeMap<String, TraitEntryDto>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TypeEntryDto {
    kind: String,
    #[serde(default)]
    members: Vec<String>,
    #[serde(default)]
    method_return_types: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TraitEntryDto {
    #[serde(default)]
    methods: Vec<String>,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Decodes a `domain-types-baseline.json` string into a `TypeBaseline`.
///
/// # Errors
///
/// Returns `BaselineCodecError` when JSON is invalid, schema_version != 1,
/// timestamp is invalid, or a type entry has an unknown kind.
pub fn decode(json: &str) -> Result<TypeBaseline, BaselineCodecError> {
    let dto: BaselineDto = serde_json::from_str(json)?;

    if dto.schema_version != 1 {
        return Err(BaselineCodecError::UnsupportedSchemaVersion(dto.schema_version));
    }

    let captured_at = Timestamp::new(&dto.captured_at)?;

    let mut types = HashMap::with_capacity(dto.types.len());
    for (name, entry_dto) in dto.types {
        let kind = type_kind_from_str(&entry_dto.kind).ok_or_else(|| {
            BaselineCodecError::Json(serde_json::Error::custom(format!(
                "unknown type kind '{}' for '{name}'",
                entry_dto.kind
            )))
        })?;
        types.insert(
            name,
            TypeBaselineEntry::new(kind, entry_dto.members, entry_dto.method_return_types),
        );
    }

    let mut traits = HashMap::with_capacity(dto.traits.len());
    for (name, entry_dto) in dto.traits {
        traits.insert(name, TraitBaselineEntry::new(entry_dto.methods));
    }

    Ok(TypeBaseline::new(dto.schema_version, captured_at, types, traits))
}

/// Encodes a `TypeBaseline` to a pretty-printed JSON string.
///
/// # Errors
///
/// Returns `BaselineCodecError::Json` if serialization fails.
pub fn encode(baseline: &TypeBaseline) -> Result<String, BaselineCodecError> {
    let dto = baseline_to_dto(baseline);
    serde_json::to_string_pretty(&dto).map_err(BaselineCodecError::Json)
}

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

fn type_kind_from_str(s: &str) -> Option<TypeKind> {
    match s {
        "struct" => Some(TypeKind::Struct),
        "enum" => Some(TypeKind::Enum),
        "type_alias" => Some(TypeKind::TypeAlias),
        _ => None,
    }
}

fn type_kind_to_str(kind: &TypeKind) -> &'static str {
    match kind {
        TypeKind::Struct => "struct",
        TypeKind::Enum => "enum",
        TypeKind::TypeAlias => "type_alias",
    }
}

fn baseline_to_dto(baseline: &TypeBaseline) -> BaselineDto {
    // Collect into BTreeMap so that keys are serialized in sorted order,
    // producing a deterministic JSON artifact for VCS commits.
    let types: BTreeMap<String, TypeEntryDto> = baseline
        .types()
        .iter()
        .map(|(name, entry)| {
            (
                name.clone(),
                TypeEntryDto {
                    kind: type_kind_to_str(entry.kind()).to_owned(),
                    members: entry.members().to_vec(),
                    method_return_types: entry.method_return_types().to_vec(),
                },
            )
        })
        .collect();

    let traits: BTreeMap<String, TraitEntryDto> = baseline
        .traits()
        .iter()
        .map(|(name, entry)| (name.clone(), TraitEntryDto { methods: entry.methods().to_vec() }))
        .collect();

    BaselineDto {
        schema_version: baseline.schema_version(),
        captured_at: baseline.captured_at().as_str().to_owned(),
        types,
        traits,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    const SAMPLE_JSON: &str = r#"{
  "schema_version": 1,
  "captured_at": "2026-04-11T00:01:00Z",
  "types": {
    "TrackId": { "kind": "struct", "members": ["0"], "method_return_types": [] },
    "TaskStatus": { "kind": "enum", "members": ["Todo", "InProgress", "Done", "Skipped"], "method_return_types": ["TaskStatusKind"] }
  },
  "traits": {
    "TrackReader": { "methods": ["find"] },
    "TrackWriter": { "methods": ["save", "update"] }
  }
}"#;

    #[test]
    fn test_decode_sample_json_succeeds() {
        let bl = decode(SAMPLE_JSON).unwrap();
        assert_eq!(bl.schema_version(), 1);
        assert_eq!(bl.captured_at().as_str(), "2026-04-11T00:01:00Z");
        assert_eq!(bl.types().len(), 2);
        assert_eq!(bl.traits().len(), 2);
    }

    #[test]
    fn test_decode_type_kind_struct() {
        let bl = decode(SAMPLE_JSON).unwrap();
        let entry = bl.get_type("TrackId").unwrap();
        assert_eq!(entry.kind(), &TypeKind::Struct);
        assert_eq!(entry.members(), &["0"]);
    }

    #[test]
    fn test_decode_type_kind_enum() {
        let bl = decode(SAMPLE_JSON).unwrap();
        let entry = bl.get_type("TaskStatus").unwrap();
        assert_eq!(entry.kind(), &TypeKind::Enum);
        // Members are sorted at construction
        assert_eq!(entry.members(), &["Done", "InProgress", "Skipped", "Todo"]);
        assert_eq!(entry.method_return_types(), &["TaskStatusKind"]);
    }

    #[test]
    fn test_decode_trait_entry() {
        let bl = decode(SAMPLE_JSON).unwrap();
        let entry = bl.get_trait("TrackWriter").unwrap();
        // Methods are sorted at construction
        assert_eq!(entry.methods(), &["save", "update"]);
    }

    #[test]
    fn test_decode_wrong_schema_version() {
        let json = r#"{ "schema_version": 99, "captured_at": "2026-04-11T00:00:00Z", "types": {}, "traits": {} }"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, BaselineCodecError::UnsupportedSchemaVersion(99)));
    }

    #[test]
    fn test_decode_invalid_timestamp() {
        let json = r#"{ "schema_version": 1, "captured_at": "not-a-timestamp", "types": {}, "traits": {} }"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, BaselineCodecError::InvalidTimestamp(_)));
    }

    #[test]
    fn test_decode_unknown_type_kind() {
        let json = r#"{
  "schema_version": 1,
  "captured_at": "2026-04-11T00:00:00Z",
  "types": { "Bad": { "kind": "unknown_kind" } },
  "traits": {}
}"#;
        let err = decode(json).unwrap_err();
        // Must be a JSON error containing the unknown-kind message, not a missing-field error.
        assert!(matches!(err, BaselineCodecError::Json(_)));
    }

    #[test]
    fn test_decode_empty_types_and_traits() {
        // Both keys must be present; empty objects are valid.
        let json = r#"{ "schema_version": 1, "captured_at": "2026-04-11T00:00:00Z", "types": {}, "traits": {} }"#;
        let bl = decode(json).unwrap();
        assert!(bl.types().is_empty());
        assert!(bl.traits().is_empty());
    }

    #[test]
    fn test_decode_missing_types_and_traits_is_rejected() {
        // A truncated payload that omits both required top-level maps must fail,
        // not silently decode to an empty baseline.
        let json = r#"{ "schema_version": 1, "captured_at": "2026-04-11T00:00:00Z" }"#;
        assert!(decode(json).is_err());
    }

    #[test]
    fn test_decode_missing_traits_is_rejected() {
        let json = r#"{ "schema_version": 1, "captured_at": "2026-04-11T00:00:00Z", "types": {} }"#;
        assert!(decode(json).is_err());
    }

    #[test]
    fn test_decode_missing_types_is_rejected() {
        let json =
            r#"{ "schema_version": 1, "captured_at": "2026-04-11T00:00:00Z", "traits": {} }"#;
        assert!(decode(json).is_err());
    }

    #[test]
    fn test_decode_duplicate_type_key_is_rejected() {
        // serde_json by default silently keeps the last value for duplicate object keys;
        // the custom deserializer must surface this as an error instead.
        let json = r#"{
  "schema_version": 1,
  "captured_at": "2026-04-11T00:00:00Z",
  "types": { "TrackId": { "kind": "struct" }, "TrackId": { "kind": "enum" } },
  "traits": {}
}"#;
        assert!(decode(json).is_err());
    }

    #[test]
    fn test_decode_duplicate_trait_key_is_rejected() {
        let json = r#"{
  "schema_version": 1,
  "captured_at": "2026-04-11T00:00:00Z",
  "types": {},
  "traits": { "TrackReader": { "methods": ["find"] }, "TrackReader": { "methods": ["load"] } }
}"#;
        assert!(decode(json).is_err());
    }

    #[test]
    fn test_round_trip_preserves_data() {
        let bl = decode(SAMPLE_JSON).unwrap();
        let encoded = encode(&bl).unwrap();
        let bl2 = decode(&encoded).unwrap();

        assert_eq!(bl.schema_version(), bl2.schema_version());
        assert_eq!(bl.captured_at(), bl2.captured_at());
        assert_eq!(bl.types().len(), bl2.types().len());
        assert_eq!(bl.traits().len(), bl2.traits().len());

        for (name, entry) in bl.types() {
            let entry2 = bl2.get_type(name).unwrap();
            assert!(entry.structurally_equal(entry2), "type '{name}' mismatch after round-trip");
        }

        for (name, entry) in bl.traits() {
            let entry2 = bl2.get_trait(name).unwrap();
            assert!(entry.structurally_equal(entry2), "trait '{name}' mismatch after round-trip");
        }
    }

    #[test]
    fn test_encode_produces_valid_json() {
        let bl = decode(SAMPLE_JSON).unwrap();
        let encoded = encode(&bl).unwrap();
        // Must be valid JSON
        let _: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    }
}
