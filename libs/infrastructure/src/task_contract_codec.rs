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

use domain::task_contract::{
    ContractedEntryRef, TASK_CONTRACT_SCHEMA_VERSION, TaskContractDocument,
};
use domain::tddd::layer_id::LayerId;
use domain::tddd::semantic_verify::CatalogueEntryKey;
use domain::{TaskId, TrackId, ValidationError};
use serde::de::{Deserializer, MapAccess, Visitor};
use serde::{Deserialize, Serialize};
use thiserror::Error;

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
) -> Result<BTreeMap<String, Vec<ContractedEntryRefDto>>, D::Error>
where
    D: Deserializer<'de>,
{
    struct StrictEntriesVisitor;

    impl<'de> Visitor<'de> for StrictEntriesVisitor {
        type Value = BTreeMap<String, Vec<ContractedEntryRefDto>>;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a map from task id to contracted entry refs")
        }

        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: MapAccess<'de>,
        {
            let mut result = BTreeMap::new();
            while let Some(key) = map.next_key::<String>()? {
                let value: Vec<ContractedEntryRefDto> = map.next_value()?;
                if result.contains_key(&key) {
                    return Err(serde::de::Error::custom(format!(
                        "duplicate task id '{key}' in task-contract entries"
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
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskContractDocumentDto {
    pub schema_version: u32,
    pub track_id: String,
    /// Map from task id string (e.g. `"T001"`) to list of contracted entry DTOs.
    #[serde(deserialize_with = "deserialize_entries_map")]
    pub entries: BTreeMap<String, Vec<ContractedEntryRefDto>>,
}

/// Serde DTO for a single `(layer, entry_key)` pair.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContractedEntryRefDto {
    pub layer: String,
    pub entry_key: String,
}

// ---------------------------------------------------------------------------
// decode
// ---------------------------------------------------------------------------

/// Decode raw bytes (UTF-8 JSON) into a
/// `domain::task_contract::TaskContractDocument`.
///
/// Validates `schema_version` before constructing the domain object.
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
    let dto: TaskContractDocumentDto = serde_json::from_slice(bytes)?;

    if dto.schema_version != TASK_CONTRACT_SCHEMA_VERSION {
        return Err(TaskContractCodecError::UnsupportedSchemaVersion {
            found: dto.schema_version,
            expected: TASK_CONTRACT_SCHEMA_VERSION,
        });
    }

    let track_id = TrackId::try_new(dto.track_id).map_err(TaskContractCodecError::from)?;

    let mut entries: BTreeMap<TaskId, Vec<ContractedEntryRef>> = BTreeMap::new();
    for (task_id_str, refs_dto) in dto.entries {
        let task_id = TaskId::try_new(task_id_str).map_err(TaskContractCodecError::from)?;
        let mut refs = Vec::with_capacity(refs_dto.len());
        for ref_dto in refs_dto {
            let layer = LayerId::try_new(ref_dto.layer).map_err(TaskContractCodecError::from)?;
            let entry_key = CatalogueEntryKey::try_new(ref_dto.entry_key)
                .map_err(TaskContractCodecError::from)?;
            refs.push(ContractedEntryRef::new(layer, entry_key));
        }
        entries.insert(task_id, refs);
    }

    TaskContractDocument::new(track_id, entries).map_err(TaskContractCodecError::from)
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
    let mut entries_dto: BTreeMap<String, Vec<ContractedEntryRefDto>> = BTreeMap::new();
    for (task_id, refs) in doc.entries() {
        let refs_dto = refs
            .iter()
            .map(|r| ContractedEntryRefDto {
                layer: r.layer().as_ref().to_owned(),
                entry_key: r.entry_key().as_str().to_owned(),
            })
            .collect();
        entries_dto.insert(task_id.to_string(), refs_dto);
    }

    let dto = TaskContractDocumentDto {
        schema_version: doc.schema_version(),
        track_id: doc.track_id().to_string(),
        entries: entries_dto,
    };

    Ok(serde_json::to_vec_pretty(&dto)?)
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
    fn decode_rejects_wrong_schema_version() {
        let json = r#"{"schema_version":2,"track_id":"t","entries":{"T001":[]}}"#;
        let err = decode(json.as_bytes()).unwrap_err();
        assert!(
            matches!(err, TaskContractCodecError::UnsupportedSchemaVersion { found: 2, .. }),
            "expected UnsupportedSchemaVersion, got: {err}"
        );
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
}
