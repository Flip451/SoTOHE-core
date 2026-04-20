//! Serde codec for `task-coverage.json` (TaskCoverageDocument SSoT).
//!
//! Schema version 1: introduced by ADR 2026-04-19-1242 §D1.4.
//! Each section maps a `SpecElementId` (string key) to a list of `TaskId`s.

use std::collections::BTreeMap;
use std::fmt;

use domain::{DomainError, SpecElementId, TaskCoverageDocument, TaskId, ValidationError};
use serde::de::{Deserializer, MapAccess, Visitor};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Codec error for `task-coverage.json` serialization/deserialization.
#[derive(Debug, thiserror::Error)]
pub enum TaskCoverageCodecError {
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("unsupported schema_version: expected 1, got {0}")]
    UnsupportedSchemaVersion(u32),

    #[error("validation error: {0}")]
    Validation(String),
}

impl From<DomainError> for TaskCoverageCodecError {
    fn from(e: DomainError) -> Self {
        Self::Validation(e.to_string())
    }
}

impl From<ValidationError> for TaskCoverageCodecError {
    fn from(e: ValidationError) -> Self {
        Self::Validation(e.to_string())
    }
}

// ---------------------------------------------------------------------------
// Duplicate-key-rejecting map deserializer
// ---------------------------------------------------------------------------

/// Deserialize a `BTreeMap<String, Vec<String>>` while rejecting duplicate keys.
///
/// Standard serde map deserialization silently overwrites duplicate keys, which
/// can silently drop coverage entries. This visitor returns an error instead.
fn deserialize_section_map<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<String, Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    struct StrictMapVisitor;

    impl<'de> Visitor<'de> for StrictMapVisitor {
        type Value = BTreeMap<String, Vec<String>>;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a map from spec element id to task id list")
        }

        fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where
            A: MapAccess<'de>,
        {
            let mut result = BTreeMap::new();
            while let Some(key) = map.next_key::<String>()? {
                let value: Vec<String> = map.next_value()?;
                if result.contains_key(&key) {
                    return Err(serde::de::Error::custom(format!(
                        "duplicate key '{key}' in coverage section"
                    )));
                }
                result.insert(key, value);
            }
            Ok(result)
        }
    }

    deserializer.deserialize_map(StrictMapVisitor)
}

// ---------------------------------------------------------------------------
// DTO
// ---------------------------------------------------------------------------

/// Top-level DTO for `task-coverage.json` (schema_version 1).
///
/// Each section is a map from `SpecElementId` string to a list of `TaskId` strings.
/// `deny_unknown_fields` rejects unrecognised fields. Duplicate keys within a section
/// are rejected by `deserialize_section_map` to prevent silent data loss.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TaskCoverageDocumentDto {
    pub schema_version: u32,
    #[serde(default, deserialize_with = "deserialize_section_map")]
    pub in_scope: BTreeMap<String, Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_section_map")]
    pub out_of_scope: BTreeMap<String, Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_section_map")]
    pub constraints: BTreeMap<String, Vec<String>>,
    #[serde(default, deserialize_with = "deserialize_section_map")]
    pub acceptance_criteria: BTreeMap<String, Vec<String>>,
}

// ---------------------------------------------------------------------------
// Decode: JSON -> domain
// ---------------------------------------------------------------------------

/// Deserializes a `task-coverage.json` string into a [`TaskCoverageDocument`].
///
/// # Errors
///
/// Returns `TaskCoverageCodecError::Json` if the JSON is malformed.
/// Returns `TaskCoverageCodecError::UnsupportedSchemaVersion` if `schema_version != 1`.
/// Returns `TaskCoverageCodecError::Validation` if any domain type construction fails.
pub fn decode(json: &str) -> Result<TaskCoverageDocument, TaskCoverageCodecError> {
    let dto: TaskCoverageDocumentDto = serde_json::from_str(json)?;

    if dto.schema_version != 1 {
        return Err(TaskCoverageCodecError::UnsupportedSchemaVersion(dto.schema_version));
    }

    let in_scope = section_from_dto(dto.in_scope)?;
    let out_of_scope = section_from_dto(dto.out_of_scope)?;
    let constraints = section_from_dto(dto.constraints)?;
    let acceptance_criteria = section_from_dto(dto.acceptance_criteria)?;

    Ok(TaskCoverageDocument::new(in_scope, out_of_scope, constraints, acceptance_criteria)?)
}

fn section_from_dto(
    raw: BTreeMap<String, Vec<String>>,
) -> Result<BTreeMap<SpecElementId, Vec<TaskId>>, TaskCoverageCodecError> {
    raw.into_iter()
        .map(|(key, values)| {
            let element_id = SpecElementId::try_new(key)?;
            let task_ids = values
                .into_iter()
                .map(|t| TaskId::try_new(t).map_err(TaskCoverageCodecError::from))
                .collect::<Result<Vec<_>, _>>()?;
            Ok((element_id, task_ids))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Encode: domain -> JSON
// ---------------------------------------------------------------------------

/// Serializes a [`TaskCoverageDocument`] to a pretty-printed `task-coverage.json` string.
///
/// # Errors
///
/// Returns `TaskCoverageCodecError::Json` if serialization fails.
pub fn encode(doc: &TaskCoverageDocument) -> Result<String, TaskCoverageCodecError> {
    let dto = task_coverage_to_dto(doc);
    Ok(serde_json::to_string_pretty(&dto)?)
}

fn task_coverage_to_dto(doc: &TaskCoverageDocument) -> TaskCoverageDocumentDto {
    TaskCoverageDocumentDto {
        schema_version: doc.schema_version(),
        in_scope: section_to_dto(doc.in_scope()),
        out_of_scope: section_to_dto(doc.out_of_scope()),
        constraints: section_to_dto(doc.constraints()),
        acceptance_criteria: section_to_dto(doc.acceptance_criteria()),
    }
}

fn section_to_dto(map: &BTreeMap<SpecElementId, Vec<TaskId>>) -> BTreeMap<String, Vec<String>> {
    map.iter()
        .map(|(k, v)| (k.as_ref().to_owned(), v.iter().map(|t| t.to_string()).collect()))
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    const MINIMAL_JSON: &str = r#"{
  "schema_version": 1
}"#;

    const FULL_JSON: &str = r#"{
  "schema_version": 1,
  "in_scope": {
    "IN-01": ["T001", "T002"],
    "IN-02": []
  },
  "out_of_scope": {
    "OS-01": ["T003"]
  },
  "constraints": {
    "CO-01": ["T004"]
  },
  "acceptance_criteria": {
    "AC-01": ["T001"],
    "AC-02": ["T002", "T003"]
  }
}"#;

    // --- decode: happy path ---

    #[test]
    fn test_decode_minimal_json_succeeds() {
        let doc = decode(MINIMAL_JSON).unwrap();
        assert_eq!(doc.schema_version(), 1);
        assert!(doc.in_scope().is_empty());
        assert!(doc.out_of_scope().is_empty());
        assert!(doc.constraints().is_empty());
        assert!(doc.acceptance_criteria().is_empty());
    }

    #[test]
    fn test_decode_full_json_succeeds() {
        let doc = decode(FULL_JSON).unwrap();

        let in01 = domain::SpecElementId::try_new("IN-01").unwrap();
        let in02 = domain::SpecElementId::try_new("IN-02").unwrap();
        assert_eq!(doc.in_scope().len(), 2);
        assert_eq!(doc.in_scope()[&in01].len(), 2);
        assert!(doc.in_scope()[&in02].is_empty());

        let os01 = domain::SpecElementId::try_new("OS-01").unwrap();
        assert_eq!(doc.out_of_scope().len(), 1);
        assert_eq!(doc.out_of_scope()[&os01].len(), 1);

        let co01 = domain::SpecElementId::try_new("CO-01").unwrap();
        assert_eq!(doc.constraints()[&co01].len(), 1);

        let ac01 = domain::SpecElementId::try_new("AC-01").unwrap();
        let ac02 = domain::SpecElementId::try_new("AC-02").unwrap();
        assert_eq!(doc.acceptance_criteria().len(), 2);
        assert_eq!(doc.acceptance_criteria()[&ac01].len(), 1);
        assert_eq!(doc.acceptance_criteria()[&ac02].len(), 2);
    }

    // --- decode: schema_version validation ---

    #[test]
    fn test_decode_with_unsupported_schema_version_returns_error() {
        let json = r#"{"schema_version": 2}"#;
        let err = decode(json).unwrap_err();
        assert!(
            matches!(err, TaskCoverageCodecError::UnsupportedSchemaVersion(2)),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_decode_with_schema_version_zero_returns_error() {
        let json = r#"{"schema_version": 0}"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, TaskCoverageCodecError::UnsupportedSchemaVersion(0)));
    }

    // --- decode: unknown field rejection ---

    #[test]
    fn test_decode_with_unknown_field_is_rejected() {
        let json = r#"{"schema_version": 1, "extra": "bad"}"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, TaskCoverageCodecError::Json(_)), "expected Json error, got: {err}");
    }

    #[test]
    fn test_decode_with_duplicate_key_within_section_is_rejected() {
        // serde_json forwards duplicate JSON object keys to the visitor rather than
        // deduplicating. Our custom StrictMapVisitor returns an error on the second
        // occurrence of a key, so this must fail.
        let err =
            decode(r#"{"schema_version": 1, "in_scope": {"IN-01": ["T001"], "IN-01": ["T002"]}}"#)
                .unwrap_err();
        assert!(
            matches!(err, TaskCoverageCodecError::Json(_)),
            "expected Json error for duplicate key, got: {err}"
        );
    }

    // --- decode: domain validation errors ---

    #[test]
    fn test_decode_with_invalid_spec_element_id_returns_validation_error() {
        let json = r#"{"schema_version": 1, "in_scope": {"A-01": []}}"#;
        // "A-01" has only one uppercase letter — invalid SpecElementId
        let err = decode(json).unwrap_err();
        assert!(matches!(err, TaskCoverageCodecError::Validation(_)), "unexpected error: {err}");
    }

    #[test]
    fn test_decode_with_invalid_task_id_returns_validation_error() {
        let json = r#"{"schema_version": 1, "in_scope": {"IN-01": ["INVALID"]}}"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, TaskCoverageCodecError::Validation(_)), "unexpected error: {err}");
    }

    #[test]
    fn test_decode_with_duplicate_element_id_across_sections_returns_validation_error() {
        let json = r#"{
          "schema_version": 1,
          "in_scope": {"IN-01": []},
          "out_of_scope": {"IN-01": []}
        }"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, TaskCoverageCodecError::Validation(_)), "unexpected error: {err}");
    }

    // --- decode: malformed JSON ---

    #[test]
    fn test_decode_invalid_json_returns_json_error() {
        let err = decode("{not json}").unwrap_err();
        assert!(matches!(err, TaskCoverageCodecError::Json(_)));
    }

    // --- encode: happy path ---

    #[test]
    fn test_encode_empty_document_produces_valid_json() {
        let doc = TaskCoverageDocument::new(
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
        )
        .unwrap();
        let json = encode(&doc).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["schema_version"], 1);
    }

    #[test]
    fn test_encode_output_is_pretty_printed() {
        let doc = decode(MINIMAL_JSON).unwrap();
        let json = encode(&doc).unwrap();
        assert!(json.contains('\n'));
    }

    #[test]
    fn test_encode_schema_version_is_always_1() {
        let doc = decode(FULL_JSON).unwrap();
        let json = encode(&doc).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["schema_version"], 1);
    }

    // --- round-trip tests ---

    #[test]
    fn test_round_trip_minimal_json() {
        let doc = decode(MINIMAL_JSON).unwrap();
        let json = encode(&doc).unwrap();
        let doc2 = decode(&json).unwrap();
        assert_eq!(doc, doc2);
    }

    #[test]
    fn test_round_trip_full_json() {
        let doc = decode(FULL_JSON).unwrap();
        let json = encode(&doc).unwrap();
        let doc2 = decode(&json).unwrap();
        assert_eq!(doc, doc2);
    }

    #[test]
    fn test_round_trip_preserves_btreemap_order() {
        let doc = decode(FULL_JSON).unwrap();
        let json = encode(&doc).unwrap();
        let doc2 = decode(&json).unwrap();
        let keys1: Vec<&str> = doc.in_scope().keys().map(|k| k.as_ref()).collect();
        let keys2: Vec<&str> = doc2.in_scope().keys().map(|k| k.as_ref()).collect();
        assert_eq!(keys1, keys2);
    }

    #[test]
    fn test_round_trip_all_sections() {
        let doc = decode(FULL_JSON).unwrap();
        let json = encode(&doc).unwrap();
        let doc2 = decode(&json).unwrap();
        assert_eq!(doc2.in_scope().len(), doc.in_scope().len());
        assert_eq!(doc2.out_of_scope().len(), doc.out_of_scope().len());
        assert_eq!(doc2.constraints().len(), doc.constraints().len());
        assert_eq!(doc2.acceptance_criteria().len(), doc.acceptance_criteria().len());
    }
}
