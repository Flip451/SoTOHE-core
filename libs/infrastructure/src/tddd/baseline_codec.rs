//! Serde codec for `domain-types-baseline.json` (`TypeBaseline` SSoT).
//!
//! The JSON schema uses type/trait names as object keys (HashMap-natural)
//! with a `schema_version` and `captured_at` envelope.
//!
//! Baseline schema v2 — members are captured as structured `MemberDeclaration`
//! (enum variant or struct field with L1 type string) and methods as
//! structured `MethodDeclaration` (name/receiver/params/returns/is_async).
//! Schema v1 (flat `Vec<String>`) baselines are rejected with a re-run hint.

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::marker::PhantomData;

use domain::schema::TypeKind;
use domain::tddd::catalogue::{MemberDeclaration, MethodDeclaration, ParamDeclaration};
use domain::{Timestamp, TraitBaselineEntry, TypeBaseline, TypeBaselineEntry, ValidationError};
use serde::de::{Error as _, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

// ---------------------------------------------------------------------------
// Duplicate-key-rejecting map deserializer
// ---------------------------------------------------------------------------

/// Deserializes a JSON object into a `BTreeMap` while rejecting duplicate keys.
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

    /// The `schema_version` field is not `2`. The body contains a re-run
    /// hint directing the caller to delete the stale baseline file, since
    /// `baseline-capture` is unconditionally idempotent and skips on
    /// existence — it would otherwise trap callers on an outdated v1 file.
    #[error(
        "unsupported baseline schema_version: expected 2, got {0}. \
         Delete the stale `<layer>-types-baseline.json` file, then \
         re-run `sotp track baseline-capture <track-id>` to regenerate at v2."
    )]
    UnsupportedSchemaVersion(u32),

    #[error("invalid timestamp: {0}")]
    InvalidTimestamp(#[from] ValidationError),
}

// ---------------------------------------------------------------------------
// DTO types
// ---------------------------------------------------------------------------

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
    members: Vec<MemberDto>,
    #[serde(default)]
    methods: Vec<MethodDto>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TraitEntryDto {
    #[serde(default)]
    methods: Vec<MethodDto>,
}

/// Member DTO — discriminator `kind` selects `variant` vs `field`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum MemberDto {
    Variant { name: String },
    Field { name: String, ty: String },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MethodDto {
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    receiver: Option<String>,
    #[serde(default)]
    params: Vec<ParamDto>,
    returns: String,
    #[serde(default)]
    is_async: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ParamDto {
    name: String,
    ty: String,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Decodes a `domain-types-baseline.json` string into a `TypeBaseline`.
///
/// # Errors
///
/// Returns `BaselineCodecError` when JSON is invalid, schema_version != 2,
/// timestamp is invalid, or a type entry has an unknown kind.
pub fn decode(json: &str) -> Result<TypeBaseline, BaselineCodecError> {
    let dto: BaselineDto = serde_json::from_str(json)?;

    if dto.schema_version != 2 {
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
        let members: Vec<MemberDeclaration> =
            entry_dto.members.into_iter().map(member_from_dto).collect();
        let methods: Vec<MethodDeclaration> =
            entry_dto.methods.into_iter().map(method_from_dto).collect();
        types.insert(name, TypeBaselineEntry::new(kind, members, methods));
    }

    let mut traits = HashMap::with_capacity(dto.traits.len());
    for (name, entry_dto) in dto.traits {
        let methods: Vec<MethodDeclaration> =
            entry_dto.methods.into_iter().map(method_from_dto).collect();
        traits.insert(name, TraitBaselineEntry::new(methods));
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

fn member_from_dto(dto: MemberDto) -> MemberDeclaration {
    match dto {
        MemberDto::Variant { name } => MemberDeclaration::variant(name),
        MemberDto::Field { name, ty } => MemberDeclaration::field(name, ty),
    }
}

fn member_to_dto(member: &MemberDeclaration) -> MemberDto {
    match member {
        MemberDeclaration::Variant(name) => MemberDto::Variant { name: name.clone() },
        MemberDeclaration::Field { name, ty } => {
            MemberDto::Field { name: name.clone(), ty: ty.clone() }
        }
    }
}

fn method_from_dto(dto: MethodDto) -> MethodDeclaration {
    let params: Vec<ParamDeclaration> =
        dto.params.into_iter().map(|p| ParamDeclaration::new(p.name, p.ty)).collect();
    MethodDeclaration::new(dto.name, dto.receiver, params, dto.returns, dto.is_async)
}

fn method_to_dto(method: &MethodDeclaration) -> MethodDto {
    let params: Vec<ParamDto> = method
        .params()
        .iter()
        .map(|p| ParamDto { name: p.name().to_string(), ty: p.ty().to_string() })
        .collect();
    MethodDto {
        name: method.name().to_string(),
        receiver: method.receiver().map(str::to_string),
        params,
        returns: method.returns().to_string(),
        is_async: method.is_async(),
    }
}

fn baseline_to_dto(baseline: &TypeBaseline) -> BaselineDto {
    let types: BTreeMap<String, TypeEntryDto> = baseline
        .types()
        .iter()
        .map(|(name, entry)| {
            (
                name.clone(),
                TypeEntryDto {
                    kind: type_kind_to_str(entry.kind()).to_owned(),
                    members: entry.members().iter().map(member_to_dto).collect(),
                    methods: entry.methods().iter().map(method_to_dto).collect(),
                },
            )
        })
        .collect();

    let traits: BTreeMap<String, TraitEntryDto> = baseline
        .traits()
        .iter()
        .map(|(name, entry)| {
            (
                name.clone(),
                TraitEntryDto { methods: entry.methods().iter().map(method_to_dto).collect() },
            )
        })
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
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;

    const SAMPLE_JSON: &str = r#"{
  "schema_version": 2,
  "captured_at": "2026-04-13T00:01:00Z",
  "types": {
    "TrackId": {
      "kind": "struct",
      "members": [
        { "kind": "field", "name": "0", "ty": "u64" }
      ],
      "methods": []
    },
    "TaskStatus": {
      "kind": "enum",
      "members": [
        { "kind": "variant", "name": "Todo" },
        { "kind": "variant", "name": "InProgress" },
        { "kind": "variant", "name": "Done" },
        { "kind": "variant", "name": "Skipped" }
      ],
      "methods": [
        { "name": "kind", "receiver": "&self", "params": [], "returns": "TaskStatusKind", "is_async": false }
      ]
    }
  },
  "traits": {
    "TrackReader": {
      "methods": [
        { "name": "find", "receiver": "&self", "params": [{"name":"id","ty":"TrackId"}], "returns": "Option<Track>", "is_async": false }
      ]
    },
    "TrackWriter": {
      "methods": [
        { "name": "save", "receiver": "&self", "params": [{"name":"track","ty":"Track"}], "returns": "Result<(), Error>", "is_async": false },
        { "name": "update", "receiver": "&self", "params": [{"name":"track","ty":"Track"}], "returns": "Result<(), Error>", "is_async": false }
      ]
    }
  }
}"#;

    #[test]
    fn test_decode_sample_json_succeeds() {
        let bl = decode(SAMPLE_JSON).unwrap();
        assert_eq!(bl.schema_version(), 2);
        assert_eq!(bl.captured_at().as_str(), "2026-04-13T00:01:00Z");
        assert_eq!(bl.types().len(), 2);
        assert_eq!(bl.traits().len(), 2);
    }

    #[test]
    fn test_decode_type_kind_struct() {
        let bl = decode(SAMPLE_JSON).unwrap();
        let entry = bl.get_type("TrackId").unwrap();
        assert_eq!(entry.kind(), &TypeKind::Struct);
        assert_eq!(entry.members().len(), 1);
        assert_eq!(entry.members()[0].name(), "0");
        assert_eq!(entry.members()[0].ty(), Some("u64"));
    }

    #[test]
    fn test_decode_type_kind_enum_and_method() {
        let bl = decode(SAMPLE_JSON).unwrap();
        let entry = bl.get_type("TaskStatus").unwrap();
        assert_eq!(entry.kind(), &TypeKind::Enum);
        let names: Vec<&str> = entry.members().iter().map(|m| m.name()).collect();
        // Members are sorted at construction
        assert_eq!(names, vec!["Done", "InProgress", "Skipped", "Todo"]);
        assert_eq!(entry.methods().len(), 1);
        assert_eq!(entry.methods()[0].name(), "kind");
        assert_eq!(entry.methods()[0].receiver(), Some("&self"));
        assert_eq!(entry.methods()[0].returns(), "TaskStatusKind");
    }

    #[test]
    fn test_decode_trait_entry() {
        let bl = decode(SAMPLE_JSON).unwrap();
        let entry = bl.get_trait("TrackWriter").unwrap();
        let names: Vec<&str> = entry.methods().iter().map(|m| m.name()).collect();
        assert_eq!(names, vec!["save", "update"]);
    }

    #[test]
    fn test_decode_v1_rejected_with_rerun_hint() {
        let json = r#"{ "schema_version": 1, "captured_at": "2026-04-11T00:00:00Z", "types": {}, "traits": {} }"#;
        let err = decode(json).unwrap_err();
        match &err {
            BaselineCodecError::UnsupportedSchemaVersion(1) => {}
            _ => panic!("expected UnsupportedSchemaVersion(1), got {err:?}"),
        }
        // The Display impl must include the re-run hint for v1 migration.
        let msg = err.to_string();
        assert!(msg.contains("baseline-capture"), "expected re-run hint, got: {msg}");
    }

    #[test]
    fn test_decode_wrong_schema_version() {
        let json = r#"{ "schema_version": 99, "captured_at": "2026-04-13T00:00:00Z", "types": {}, "traits": {} }"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, BaselineCodecError::UnsupportedSchemaVersion(99)));
    }

    #[test]
    fn test_decode_invalid_timestamp() {
        let json = r#"{ "schema_version": 2, "captured_at": "not-a-timestamp", "types": {}, "traits": {} }"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, BaselineCodecError::InvalidTimestamp(_)));
    }

    #[test]
    fn test_decode_unknown_type_kind() {
        let json = r#"{
  "schema_version": 2,
  "captured_at": "2026-04-13T00:00:00Z",
  "types": { "Bad": { "kind": "unknown_kind" } },
  "traits": {}
}"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, BaselineCodecError::Json(_)));
    }

    #[test]
    fn test_decode_empty_types_and_traits() {
        let json = r#"{ "schema_version": 2, "captured_at": "2026-04-13T00:00:00Z", "types": {}, "traits": {} }"#;
        let bl = decode(json).unwrap();
        assert!(bl.types().is_empty());
        assert!(bl.traits().is_empty());
    }

    #[test]
    fn test_decode_missing_types_and_traits_is_rejected() {
        let json = r#"{ "schema_version": 2, "captured_at": "2026-04-13T00:00:00Z" }"#;
        assert!(decode(json).is_err());
    }

    #[test]
    fn test_decode_duplicate_type_key_is_rejected() {
        let json = r#"{
  "schema_version": 2,
  "captured_at": "2026-04-13T00:00:00Z",
  "types": { "TrackId": { "kind": "struct" }, "TrackId": { "kind": "enum" } },
  "traits": {}
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
        let _: serde_json::Value = serde_json::from_str(&encoded).unwrap();
    }
}
