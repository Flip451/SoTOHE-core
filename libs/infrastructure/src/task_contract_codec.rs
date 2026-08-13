//! Serde codec for `task-contract.json` (TaskContractDocument SSoT).
//!
//! Schema version 1: introduced by ADR `knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md`.
//!
//! The on-disk format is:
//! ```json
//! {
//!   "schema_version": 1,
//!   "track_id": "<track-id>",
//!   "entries": {
//!     "T001": [
//!       { "layer": "domain", "entry_key": "MyType" }
//!     ]
//!   }
//! }
//! ```

use std::collections::BTreeMap;
use std::fmt;

use domain::task_contract::{ContractedEntryRef, TaskContractDocument};
use domain::tddd::layer_id::LayerId;
use domain::tddd::semantic_verify::CatalogueEntryKey;
use domain::{TaskId, TrackId, ValidationError};
use serde::de::{Deserializer, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize};
use thiserror::Error;

const SUPPORTED_SCHEMA_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

/// Error type for `task-contract.json` codec operations.
///
/// - `Json`: serde_json parse or serialization failure.
/// - `UnsupportedSchemaVersion`: the `schema_version` field in the file does
///   not match the supported version.
/// - `Validation`: domain-level validation failed when constructing
///   `TaskContractDocument` from the DTO; message is an opaque `String` from
///   the domain `ValidationError`.
#[derive(Debug, Error)]
pub enum TaskContractCodecError {
    /// JSON parse or serialization failure.
    #[error("{0}")]
    Json(#[from] serde_json::Error),

    /// Schema version mismatch.
    #[error("unsupported schema_version: expected {expected}, got {found}")]
    UnsupportedSchemaVersion {
        /// The schema version found in the file.
        found: u32,
        /// The schema version this codec supports.
        expected: u32,
    },

    /// Domain validation failure (e.g. invalid task id or layer id).
    #[error("validation error: {0}")]
    Validation(String),
}

impl From<ValidationError> for TaskContractCodecError {
    fn from(e: ValidationError) -> Self {
        Self::Validation(e.to_string())
    }
}

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

/// Deserialize task-contract entries while rejecting duplicate task IDs.
///
/// Standard serde map deserialization silently keeps the last duplicate key,
/// which could drop contracted entries before domain validation sees them.
fn deserialize_entries_map<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<TaskIdDto, Vec<ContractedEntryRefDto>>, D::Error>
where
    D: Deserializer<'de>,
{
    struct StrictEntriesVisitor;

    impl<'de> Visitor<'de> for StrictEntriesVisitor {
        type Value = BTreeMap<TaskIdDto, Vec<ContractedEntryRefDto>>;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a map from task id to contracted entry refs")
        }

        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: MapAccess<'de>,
        {
            let mut result = BTreeMap::new();
            while let Some(key) = map.next_key::<TaskIdDto>()? {
                let value: Vec<ContractedEntryRefDto> = map.next_value()?;
                if result.contains_key(&key) {
                    return Err(serde::de::Error::custom(format!(
                        "duplicate task id '{}' in task-contract entries",
                        key.0
                    )));
                }
                result.insert(key, value);
            }
            Ok(result)
        }
    }

    deserializer.deserialize_map(StrictEntriesVisitor)
}

/// JSON-serializable mirror of `domain::task_contract::TaskContractDocument`.
///
/// Used exclusively as an intermediate serde representation inside
/// [`decode`] and [`encode`]. Must derive `Serialize + Deserialize`;
/// `Debug` for error diagnostics.
#[derive(Debug)]
pub struct TaskContractDocumentDto {
    pub schema_version: TaskContractSchemaVersionDto,
    pub track_id: TrackIdDto,
    /// Map from task id mirror (e.g. `"T001"`) to list of contracted entry DTOs.
    pub entries: BTreeMap<TaskIdDto, Vec<ContractedEntryRefDto>>,
}

/// Serde mirror of the task-contract schema version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TaskContractSchemaVersionDto {
    pub value: u32,
}

/// Non-validating transparent serde mirror of `domain::TrackId`.
///
/// `Deserialize` performs no domain validation; the conversion to `TrackId`
/// happens in [`decode`], which maps `ValidationError` to
/// [`TaskContractCodecError::Validation`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TrackIdDto(pub String);

/// Non-validating transparent serde mirror of `domain::TaskId`.
///
/// `Deserialize` performs no domain validation; the conversion to `TaskId`
/// happens in [`decode`], which maps `ValidationError` to
/// [`TaskContractCodecError::Validation`].
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TaskIdDto(pub String);

/// Non-validating transparent serde mirror of the layer identifier.
///
/// `Deserialize` performs no domain validation; the conversion to `LayerId`
/// happens in [`decode`], which maps `ValidationError` to
/// [`TaskContractCodecError::Validation`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LayerIdDto(pub String);

/// Non-validating transparent serde mirror of the catalogue entry key.
///
/// `Deserialize` performs no domain validation; the conversion to
/// `CatalogueEntryKey` happens in [`decode`], which maps `ValidationError` to
/// [`TaskContractCodecError::Validation`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EntryKeyDto(pub String);

/// Minimal schema envelope used to reject unsupported documents before their
/// version-specific payload is decoded.
#[derive(Debug, Deserialize)]
struct TaskContractSchemaVersionProbe {
    schema_version: TaskContractSchemaVersionDto,
}

/// Serde DTO for a single `(layer, entry_key)` pair.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractedEntryRefDto {
    pub layer: LayerIdDto,
    pub entry_key: EntryKeyDto,
}

impl Serialize for TaskContractDocumentDto {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(3))?;
        map.serialize_entry("schema_version", &self.schema_version)?;
        map.serialize_entry("track_id", &self.track_id)?;
        struct Entries<'a>(&'a BTreeMap<TaskIdDto, Vec<ContractedEntryRefDto>>);
        impl Serialize for Entries<'_> {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                let mut map = serializer.serialize_map(Some(self.0.len()))?;
                for (task_id, entries) in self.0 {
                    map.serialize_entry(task_id, entries)?;
                }
                map.end()
            }
        }
        map.serialize_entry("entries", &Entries(&self.entries))?;
        map.end()
    }
}

impl<'de> Deserialize<'de> for TaskContractDocumentDto {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawTaskContractDocumentDto {
            schema_version: TaskContractSchemaVersionDto,
            track_id: TrackIdDto,
            #[serde(deserialize_with = "deserialize_entries_map")]
            entries: BTreeMap<TaskIdDto, Vec<ContractedEntryRefDto>>,
        }

        let raw = RawTaskContractDocumentDto::deserialize(deserializer)?;
        Ok(Self {
            schema_version: raw.schema_version,
            track_id: raw.track_id,
            entries: raw.entries,
        })
    }
}

// ---------------------------------------------------------------------------
// decode
// ---------------------------------------------------------------------------

/// Decode raw bytes (UTF-8 JSON) into a
/// `domain::task_contract::TaskContractDocument`.
///
/// Validates `schema_version` before decoding its version-specific payload or
/// constructing the domain object.
///
/// # Errors
///
/// - [`TaskContractCodecError::Json`]: the input is not valid JSON or contains
///   unknown fields.
/// - [`TaskContractCodecError::UnsupportedSchemaVersion`]: `schema_version` is
///   not 1.
/// - [`TaskContractCodecError::Validation`]: domain validation of the decoded
///   data fails (e.g. invalid task id or layer id).
pub fn decode(bytes: &[u8]) -> Result<TaskContractDocument, TaskContractCodecError> {
    let version_probe: TaskContractSchemaVersionProbe = serde_json::from_slice(bytes)?;
    if version_probe.schema_version.value != SUPPORTED_SCHEMA_VERSION {
        return Err(TaskContractCodecError::UnsupportedSchemaVersion {
            found: version_probe.schema_version.value,
            expected: SUPPORTED_SCHEMA_VERSION,
        });
    }

    let dto: TaskContractDocumentDto = serde_json::from_slice(bytes)?;

    let mut entries: BTreeMap<TaskId, Vec<ContractedEntryRef>> = BTreeMap::new();
    for (task_id, refs_dto) in dto.entries {
        let task_id = TaskId::try_new(task_id.0)?;
        let mut refs = Vec::with_capacity(refs_dto.len());
        for ref_dto in refs_dto {
            refs.push(ContractedEntryRef::new(
                LayerId::try_new(ref_dto.layer.0)?,
                CatalogueEntryKey::try_new(ref_dto.entry_key.0)?,
            ));
        }
        entries.insert(task_id, refs);
    }

    TaskContractDocument::new(TrackId::try_new(dto.track_id.0)?, entries)
        .map_err(TaskContractCodecError::from)
}

// ---------------------------------------------------------------------------
// encode
// ---------------------------------------------------------------------------

/// Encode a `domain::task_contract::TaskContractDocument` to UTF-8 JSON bytes.
///
/// Used by the `impl-planner` subcommand to write `task-contract.json`.
///
/// # Errors
///
/// Returns [`TaskContractCodecError::Json`] if serialization fails (defensive;
/// should not happen for well-formed domain objects).
pub fn encode(doc: &TaskContractDocument) -> Result<Vec<u8>, TaskContractCodecError> {
    let mut entries_dto: BTreeMap<TaskIdDto, Vec<ContractedEntryRefDto>> = BTreeMap::new();
    for (task_id, refs) in doc.entries() {
        let refs_dto = refs
            .iter()
            .map(|r| ContractedEntryRefDto {
                layer: LayerIdDto(r.layer().as_ref().to_owned()),
                entry_key: EntryKeyDto(r.entry_key().as_str().to_owned()),
            })
            .collect();
        entries_dto.insert(TaskIdDto(task_id.to_string()), refs_dto);
    }

    let dto = TaskContractDocumentDto {
        schema_version: TaskContractSchemaVersionDto { value: doc.schema_version() },
        track_id: TrackIdDto(doc.track_id().as_ref().to_owned()),
        entries: entries_dto,
    };

    // Serialize through Value so the task-contract envelope and its nested
    // entry references use canonical JSON map-key order.
    let value = serde_json::to_value(&dto)?;
    Ok(serde_json::to_vec_pretty(&value)?)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    const SAMPLE_JSON: &str = r#"{
  "schema_version": 1,
  "track_id": "my-track",
  "entries": {
    "T001": [
      {"layer": "domain", "entry_key": "MyType"}
    ]
  }
}"#;

    #[test]
    fn decode_accepts_valid_json() {
        let doc = decode(SAMPLE_JSON.as_bytes()).unwrap();
        assert_eq!(doc.track_id().as_ref(), "my-track");
        assert_eq!(doc.schema_version(), 1);
        assert_eq!(doc.entries().len(), 1);
        let task_id = TaskId::try_new("T001").unwrap();
        let refs = doc.entries().get(&task_id).unwrap();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].layer().as_ref(), "domain");
        assert_eq!(refs[0].entry_key().as_str(), "MyType");
    }

    #[test]
    fn test_decode_task_attribution_preserves_task_entry_associations() {
        let json = r#"{
  "schema_version": 1,
  "track_id": "valid-track",
  "entries": {
    "T001": [
      {"layer": "usecase", "entry_key": "PhaseCommandInteractor"},
      {"layer": "infrastructure", "entry_key": "TaskContractDocumentDto"}
    ],
    "T002": [
      {"layer": "domain", "entry_key": "TaskContractDocument"}
    ]
  }
}"#;

        let doc = decode(json.as_bytes()).unwrap();
        let task_one = TaskId::try_new("T001").unwrap();
        let task_two = TaskId::try_new("T002").unwrap();

        assert_eq!(
            doc.entries()
                .get(&task_one)
                .unwrap()
                .iter()
                .map(|entry| (entry.layer().as_ref(), entry.entry_key().as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("usecase", "PhaseCommandInteractor"),
                ("infrastructure", "TaskContractDocumentDto"),
            ]
        );
        assert_eq!(
            doc.entries()
                .get(&task_two)
                .unwrap()
                .iter()
                .map(|entry| (entry.layer().as_ref(), entry.entry_key().as_str()))
                .collect::<Vec<_>>(),
            vec![("domain", "TaskContractDocument")]
        );
    }

    #[test]
    fn decode_rejects_wrong_schema_version() {
        let json = r#"{"schema_version":2,"track_id":"t","entries":{"T001":[]}}"#;
        let err = decode(json.as_bytes()).unwrap_err();
        assert!(
            matches!(err, TaskContractCodecError::UnsupportedSchemaVersion { found: 2, .. }),
            "expected UnsupportedSchemaVersion, got: {err}"
        );
    }

    #[test]
    fn decode_rejects_unsupported_schema_before_version_specific_shape() {
        let json = r#"{"schema_version":2,"track_id":false,"entries":"not-an-object"}"#;
        let err = decode(json.as_bytes()).unwrap_err();
        assert!(
            matches!(err, TaskContractCodecError::UnsupportedSchemaVersion { found: 2, .. }),
            "expected UnsupportedSchemaVersion, got: {err}"
        );
    }

    #[test]
    fn decode_maps_domain_identifier_rejections_to_validation_errors() {
        for json in [
            r#"{"schema_version":1,"track_id":"invalid track id","entries":{}}"#,
            r#"{"schema_version":1,"track_id":"valid-track","entries":{"invalid task id":[]}}"#,
            r#"{"schema_version":1,"track_id":"valid-track","entries":{"T001":[{"layer":"invalid layer","entry_key":"Entry"}]}}"#,
            r#"{"schema_version":1,"track_id":"valid-track","entries":{"T001":[{"layer":"domain","entry_key":" "}]}}"#,
        ] {
            assert!(matches!(decode(json.as_bytes()), Err(TaskContractCodecError::Validation(_))));
        }
    }

    #[test]
    fn test_track_id_dto_round_trips_domain_invalid_string_without_validation() {
        let raw = "invalid track id";
        let dto: TrackIdDto = serde_json::from_str(r#""invalid track id""#).unwrap();

        assert_eq!(dto, TrackIdDto(raw.to_owned()));
        assert_eq!(serde_json::to_string(&dto).unwrap(), r#""invalid track id""#);
    }

    #[test]
    fn test_task_id_dto_round_trips_domain_invalid_string_without_validation() {
        let raw = "invalid task id";
        let dto: TaskIdDto = serde_json::from_str(r#""invalid task id""#).unwrap();

        assert_eq!(dto, TaskIdDto(raw.to_owned()));
        assert_eq!(serde_json::to_string(&dto).unwrap(), r#""invalid task id""#);
    }

    #[test]
    fn test_layer_id_dto_round_trips_domain_invalid_string_without_validation() {
        let raw = "invalid layer";
        let dto: LayerIdDto = serde_json::from_str(r#""invalid layer""#).unwrap();

        assert_eq!(dto, LayerIdDto(raw.to_owned()));
        assert_eq!(serde_json::to_string(&dto).unwrap(), r#""invalid layer""#);
    }

    #[test]
    fn test_entry_key_dto_round_trips_domain_invalid_string_without_validation() {
        let raw = " ";
        let dto: EntryKeyDto = serde_json::from_str(r#"" ""#).unwrap();

        assert_eq!(dto, EntryKeyDto(raw.to_owned()));
        assert_eq!(serde_json::to_string(&dto).unwrap(), r#"" ""#);
    }

    #[test]
    fn test_decode_rejects_carrier_variant_attribution_targets() {
        for json in [
            r#"{"schema_version":1,"track_id":"valid-track","entries":{"T001":[{"layer":"domain","entry_key":"Entry","carrier":{"kind":"source_only_baseline_restoration"}}]}}"#,
            r#"{"schema_version":1,"track_id":"valid-track","entries":{"T001":[{"layer":"domain","entry_key":"Entry","carrier":{"kind":"coverage_liveness"}}]}}"#,
        ] {
            assert!(matches!(decode(json.as_bytes()), Err(TaskContractCodecError::Json(_))));
        }
    }

    #[test]
    fn test_decode_rejects_eligibility_and_stale_lifecycle_attribution_targets() {
        for json in [
            r#"{"schema_version":1,"track_id":"valid-track","entries":{"T001":[{"layer":"domain","entry_key":"Entry","eligibility":{"inspector":"baseline"}}]}}"#,
            r#"{"schema_version":1,"track_id":"valid-track","entries":{"T001":[{"layer":"domain","entry_key":"Entry","stale":true}]}}"#,
        ] {
            assert!(matches!(decode(json.as_bytes()), Err(TaskContractCodecError::Json(_))));
        }
    }

    #[test]
    fn decode_rejects_unknown_fields() {
        let json = r#"{"schema_version":1,"track_id":"t","entries":{},"extra":true}"#;
        let err = decode(json.as_bytes()).unwrap_err();
        assert!(matches!(err, TaskContractCodecError::Json(_)), "expected Json error");
    }

    #[test]
    fn test_decode_duplicate_task_id_returns_json_error() {
        let json = r#"{
  "schema_version": 1,
  "track_id": "my-track",
  "entries": {
    "T001": [
      {"layer": "domain", "entry_key": "FirstType"}
    ],
    "T001": [
      {"layer": "domain", "entry_key": "SecondType"}
    ]
  }
}"#;
        let err = decode(json.as_bytes()).unwrap_err();
        assert!(matches!(err, TaskContractCodecError::Json(_)), "expected Json error");
    }

    #[test]
    fn encode_decode_round_trip() {
        let doc = decode(SAMPLE_JSON.as_bytes()).unwrap();
        let bytes = encode(&doc).unwrap();
        let doc2 = decode(&bytes).unwrap();
        assert_eq!(doc, doc2);
    }

    #[test]
    fn test_encode_populated_contract_returns_canonical_deterministic_bytes() {
        let doc = decode(SAMPLE_JSON.as_bytes()).unwrap();

        let first = encode(&doc).unwrap();
        let second = encode(&doc).unwrap();
        let json = std::str::from_utf8(&first).unwrap();

        assert_eq!(first, second);
        assert!(json.starts_with("{\n  \"entries\":"), "root keys must be canonical: {json}");
        assert!(
            json.contains("\"entry_key\": \"MyType\",\n        \"layer\": \"domain\""),
            "nested contract keys must be canonical: {json}"
        );
    }
}
