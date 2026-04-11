//! Serde codec for domain-types.json (`DomainTypesDocument` SSoT).
//!
//! The JSON schema uses an internally-tagged enum (`"kind"` field) with
//! `#[serde(flatten)]` so that kind-specific fields are required at the type
//! level — illegal field combinations are rejected by serde, not by manual
//! validation.
//!
//! Schema version 1 is the only supported version.

use domain::{
    ConfidenceSignal, DomainTypeEntry, DomainTypeKind, DomainTypeSignal, DomainTypesDocument,
    SpecValidationError, TypestateTransitions,
};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Codec error for domain-types.json serialization/deserialization.
#[derive(Debug, thiserror::Error)]
pub enum DomainTypesCodecError {
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("validation error: {0}")]
    Validation(#[from] SpecValidationError),

    #[error("unsupported schema_version: expected 1, got {0}")]
    UnsupportedSchemaVersion(u32),

    #[error("invalid entry '{name}': {reason}")]
    InvalidEntry { name: String, reason: String },
}

// ---------------------------------------------------------------------------
// DTO types
// ---------------------------------------------------------------------------

/// Top-level DTO for domain-types.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DomainTypesDocDto {
    pub schema_version: u32,
    #[serde(default)]
    pub domain_types: Vec<DomainTypeEntryDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signals: Option<Vec<DomainTypeSignalDto>>,
}

/// Entry DTO with tagged kind enum.
///
/// Common fields (`name`, `description`, `approved`) live at the struct level.
/// Kind-specific fields are encoded via `DomainTypeKindDto` which is flattened
/// into the same JSON object and uses `"kind"` as the tag discriminator.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DomainTypeEntryDto {
    pub name: String,
    pub description: String,
    #[serde(default = "default_approved")]
    pub approved: bool,
    #[serde(flatten)]
    pub kind: DomainTypeKindDto,
}

fn default_approved() -> bool {
    true
}

/// Internally-tagged enum for kind-specific fields.
///
/// serde enforces that each variant's fields are present in the JSON.
/// `"enum"` is a Rust keyword, so we rename the variant via serde.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum DomainTypeKindDto {
    Typestate {
        transitions_to: Vec<String>,
    },
    #[serde(rename = "enum")]
    Enum {
        expected_variants: Vec<String>,
    },
    ValueObject {},
    ErrorType {
        expected_variants: Vec<String>,
    },
    TraitPort {
        expected_methods: Vec<String>,
    },
}

/// DTO for a per-type signal evaluation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DomainTypeSignalDto {
    pub type_name: String,
    pub kind_tag: String,
    pub signal: String,
    pub found_type: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub found_items: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing_items: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_items: Vec<String>,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Decodes a `domain-types.json` string into a `DomainTypesDocument`.
///
/// # Errors
///
/// Returns `DomainTypesCodecError` when:
/// - The string is not valid JSON.
/// - `schema_version` is not 1.
/// - Any entry has an unknown `kind` tag or missing required fields.
/// - Any entry fails domain validation (e.g. empty name).
pub fn decode(json: &str) -> Result<DomainTypesDocument, DomainTypesCodecError> {
    let dto: DomainTypesDocDto = serde_json::from_str(json)?;

    if dto.schema_version != 1 {
        return Err(DomainTypesCodecError::UnsupportedSchemaVersion(dto.schema_version));
    }

    let mut entries = Vec::with_capacity(dto.domain_types.len());
    for entry_dto in &dto.domain_types {
        entries.push(domain_type_entry_from_dto(entry_dto)?);
    }

    // Reject duplicate entry names
    let mut seen_names = std::collections::HashSet::new();
    for entry in &entries {
        if !seen_names.insert(entry.name()) {
            return Err(DomainTypesCodecError::InvalidEntry {
                name: entry.name().to_owned(),
                reason: "duplicate entry name".to_owned(),
            });
        }
    }

    // Typestate transitions_to referential integrity: targets must exist AND be typestate kind
    let typestate_names: std::collections::HashSet<&str> = entries
        .iter()
        .filter(|e| matches!(e.kind(), DomainTypeKind::Typestate { .. }))
        .map(|e| e.name())
        .collect();
    for entry in &entries {
        if let DomainTypeKind::Typestate { transitions: TypestateTransitions::To(targets) } =
            entry.kind()
        {
            for target in targets {
                if !typestate_names.contains(target.as_str()) {
                    return Err(DomainTypesCodecError::InvalidEntry {
                        name: entry.name().to_owned(),
                        reason: format!(
                            "transitions_to target '{target}' is not a typestate entry"
                        ),
                    });
                }
            }
        }
    }

    let mut doc = DomainTypesDocument::new(dto.schema_version, entries);

    if let Some(signal_dtos) = dto.signals {
        let signals =
            signal_dtos.iter().map(domain_type_signal_from_dto).collect::<Result<Vec<_>, _>>()?;
        doc.set_signals(signals);
    }

    Ok(doc)
}

/// Encodes a `DomainTypesDocument` to a pretty-printed JSON string.
///
/// # Errors
///
/// Returns `DomainTypesCodecError::Json` if serialization fails.
pub fn encode(doc: &DomainTypesDocument) -> Result<String, DomainTypesCodecError> {
    let dto = domain_types_doc_to_dto(doc);
    serde_json::to_string_pretty(&dto).map_err(DomainTypesCodecError::Json)
}

// ---------------------------------------------------------------------------
// Conversion helpers: DTO → domain
// ---------------------------------------------------------------------------

fn domain_type_entry_from_dto(
    dto: &DomainTypeEntryDto,
) -> Result<DomainTypeEntry, DomainTypesCodecError> {
    let kind = domain_type_kind_from_dto(&dto.kind);
    DomainTypeEntry::new(&dto.name, &dto.description, kind, dto.approved)
        .map_err(DomainTypesCodecError::Validation)
}

fn domain_type_kind_from_dto(dto: &DomainTypeKindDto) -> DomainTypeKind {
    match dto {
        DomainTypeKindDto::Typestate { transitions_to } => {
            let transitions = if transitions_to.is_empty() {
                TypestateTransitions::Terminal
            } else {
                TypestateTransitions::To(transitions_to.clone())
            };
            DomainTypeKind::Typestate { transitions }
        }
        DomainTypeKindDto::Enum { expected_variants } => {
            DomainTypeKind::Enum { expected_variants: expected_variants.clone() }
        }
        DomainTypeKindDto::ValueObject {} => DomainTypeKind::ValueObject,
        DomainTypeKindDto::ErrorType { expected_variants } => {
            DomainTypeKind::ErrorType { expected_variants: expected_variants.clone() }
        }
        DomainTypeKindDto::TraitPort { expected_methods } => {
            DomainTypeKind::TraitPort { expected_methods: expected_methods.clone() }
        }
    }
}

fn domain_type_signal_from_dto(
    dto: &DomainTypeSignalDto,
) -> Result<DomainTypeSignal, DomainTypesCodecError> {
    let signal = confidence_signal_from_str(&dto.signal).ok_or_else(|| {
        DomainTypesCodecError::InvalidEntry {
            name: dto.type_name.clone(),
            reason: format!("unknown signal value '{}'", dto.signal),
        }
    })?;
    Ok(DomainTypeSignal::new(
        &dto.type_name,
        &dto.kind_tag,
        signal,
        dto.found_type,
        dto.found_items.clone(),
        dto.missing_items.clone(),
        dto.extra_items.clone(),
    ))
}

fn confidence_signal_from_str(s: &str) -> Option<ConfidenceSignal> {
    match s {
        "blue" => Some(ConfidenceSignal::Blue),
        "yellow" => Some(ConfidenceSignal::Yellow),
        "red" => Some(ConfidenceSignal::Red),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Conversion helpers: domain → DTO
// ---------------------------------------------------------------------------

fn domain_types_doc_to_dto(doc: &DomainTypesDocument) -> DomainTypesDocDto {
    let domain_types = doc.entries().iter().map(domain_type_entry_to_dto).collect();
    let signals = doc.signals().map(|sigs| sigs.iter().map(domain_type_signal_to_dto).collect());
    DomainTypesDocDto { schema_version: doc.schema_version(), domain_types, signals }
}

fn domain_type_entry_to_dto(entry: &DomainTypeEntry) -> DomainTypeEntryDto {
    let kind = match entry.kind() {
        DomainTypeKind::Typestate { transitions } => {
            let transitions_to = match transitions {
                TypestateTransitions::Terminal => vec![],
                TypestateTransitions::To(v) => v.clone(),
            };
            DomainTypeKindDto::Typestate { transitions_to }
        }
        DomainTypeKind::Enum { expected_variants } => {
            DomainTypeKindDto::Enum { expected_variants: expected_variants.clone() }
        }
        DomainTypeKind::ValueObject => DomainTypeKindDto::ValueObject {},
        DomainTypeKind::ErrorType { expected_variants } => {
            DomainTypeKindDto::ErrorType { expected_variants: expected_variants.clone() }
        }
        DomainTypeKind::TraitPort { expected_methods } => {
            DomainTypeKindDto::TraitPort { expected_methods: expected_methods.clone() }
        }
    };
    DomainTypeEntryDto {
        name: entry.name().to_owned(),
        description: entry.description().to_owned(),
        approved: entry.approved(),
        kind,
    }
}

fn domain_type_signal_to_dto(sig: &DomainTypeSignal) -> DomainTypeSignalDto {
    DomainTypeSignalDto {
        type_name: sig.type_name().to_owned(),
        kind_tag: sig.kind_tag().to_owned(),
        signal: confidence_signal_to_str(sig.signal()).to_owned(),
        found_type: sig.found_type(),
        found_items: sig.found_items().to_vec(),
        missing_items: sig.missing_items().to_vec(),
        extra_items: sig.extra_items().to_vec(),
    }
}

fn confidence_signal_to_str(signal: ConfidenceSignal) -> &'static str {
    match signal {
        ConfidenceSignal::Blue => "blue",
        ConfidenceSignal::Yellow => "yellow",
        ConfidenceSignal::Red => "red",
        _ => "unknown",
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    const FULL_JSON: &str = r#"{
  "schema_version": 1,
  "domain_types": [
    { "name": "Draft", "kind": "typestate", "description": "Draft state", "transitions_to": ["Published"], "approved": true },
    { "name": "Published", "kind": "typestate", "description": "Published state", "transitions_to": [], "approved": true },
    { "name": "TrackStatus", "kind": "enum", "description": "Track status", "expected_variants": ["Planned", "Done"], "approved": true },
    { "name": "TrackId", "kind": "value_object", "description": "Track identifier", "approved": true },
    { "name": "SchemaExportError", "kind": "error_type", "description": "Export error", "expected_variants": ["NightlyNotFound"], "approved": true },
    { "name": "SchemaExporter", "kind": "trait_port", "description": "Export port", "expected_methods": ["export"], "approved": true }
  ]
}"#;

    #[test]
    fn test_decode_full_json_succeeds() {
        let doc = decode(FULL_JSON).unwrap();
        assert_eq!(doc.entries().len(), 6);
    }

    #[test]
    fn test_decode_typestate_kind() {
        let doc = decode(FULL_JSON).unwrap();
        assert!(matches!(
            doc.entries()[0].kind(),
            DomainTypeKind::Typestate { transitions: TypestateTransitions::To(v) } if v == &["Published"]
        ));
    }

    #[test]
    fn test_decode_enum_kind() {
        let doc = decode(FULL_JSON).unwrap();
        assert!(matches!(
            doc.entries()[2].kind(),
            DomainTypeKind::Enum { expected_variants } if expected_variants == &["Planned", "Done"]
        ));
    }

    #[test]
    fn test_decode_value_object_kind() {
        let doc = decode(FULL_JSON).unwrap();
        assert!(matches!(doc.entries()[3].kind(), DomainTypeKind::ValueObject));
    }

    #[test]
    fn test_decode_error_type_kind() {
        let doc = decode(FULL_JSON).unwrap();
        assert!(matches!(
            doc.entries()[4].kind(),
            DomainTypeKind::ErrorType { expected_variants } if expected_variants == &["NightlyNotFound"]
        ));
    }

    #[test]
    fn test_decode_trait_port_kind() {
        let doc = decode(FULL_JSON).unwrap();
        assert!(matches!(
            doc.entries()[5].kind(),
            DomainTypeKind::TraitPort { expected_methods } if expected_methods == &["export"]
        ));
    }

    #[test]
    fn test_decode_approved_field() {
        let doc = decode(FULL_JSON).unwrap();
        assert!(doc.entries()[0].approved());
    }

    #[test]
    fn test_decode_approved_defaults_to_true_when_absent() {
        let json = r#"{
  "schema_version": 1,
  "domain_types": [
    { "name": "Foo", "kind": "value_object", "description": "no approved field" }
  ]
}"#;
        let doc = decode(json).unwrap();
        assert!(doc.entries()[0].approved());
    }

    #[test]
    fn test_decode_empty_domain_types_array() {
        let json = r#"{ "schema_version": 1, "domain_types": [] }"#;
        let doc = decode(json).unwrap();
        assert_eq!(doc.entries().len(), 0);
    }

    #[test]
    fn test_decode_wrong_schema_version_returns_error() {
        let json = r#"{ "schema_version": 99, "domain_types": [] }"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, DomainTypesCodecError::UnsupportedSchemaVersion(99)));
    }

    #[test]
    fn test_decode_unknown_kind_returns_error() {
        let json = r#"{
  "schema_version": 1,
  "domain_types": [
    { "name": "Foo", "kind": "unknown_kind", "description": "bad", "approved": true }
  ]
}"#;
        assert!(decode(json).is_err());
    }

    #[test]
    fn test_decode_empty_name_returns_validation_error() {
        let json = r#"{
  "schema_version": 1,
  "domain_types": [
    { "name": "", "kind": "value_object", "description": "bad", "approved": true }
  ]
}"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, DomainTypesCodecError::Validation(_)));
    }

    #[test]
    fn test_decode_enum_without_expected_variants_returns_error() {
        let json = r#"{
  "schema_version": 1,
  "domain_types": [
    { "name": "Bad", "kind": "enum", "description": "missing field" }
  ]
}"#;
        assert!(decode(json).is_err());
    }

    #[test]
    fn test_decode_trait_port_without_expected_methods_returns_error() {
        let json = r#"{
  "schema_version": 1,
  "domain_types": [
    { "name": "Bad", "kind": "trait_port", "description": "missing field" }
  ]
}"#;
        assert!(decode(json).is_err());
    }

    #[test]
    fn test_decode_invalid_transition_target_returns_error() {
        let json = r#"{
  "schema_version": 1,
  "domain_types": [
    { "name": "Draft", "kind": "typestate", "description": "d", "transitions_to": ["NonExistent"] }
  ]
}"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, DomainTypesCodecError::InvalidEntry { .. }));
    }

    #[test]
    fn test_round_trip_preserves_all_kinds() {
        let doc = decode(FULL_JSON).unwrap();
        let encoded = encode(&doc).unwrap();
        let doc2 = decode(&encoded).unwrap();
        assert_eq!(doc.entries().len(), doc2.entries().len());
        for (a, b) in doc.entries().iter().zip(doc2.entries()) {
            assert_eq!(a.name(), b.name());
            assert_eq!(a.kind(), b.kind());
            assert_eq!(a.approved(), b.approved());
        }
    }

    #[test]
    fn test_round_trip_with_signals() {
        let json = r#"{
  "schema_version": 1,
  "domain_types": [
    { "name": "Draft", "kind": "typestate", "description": "Draft state", "transitions_to": [] }
  ],
  "signals": [
    { "type_name": "Draft", "kind_tag": "typestate", "signal": "blue", "found_type": true }
  ]
}"#;
        let doc = decode(json).unwrap();
        assert!(doc.signals().is_some());
        let encoded = encode(&doc).unwrap();
        let doc2 = decode(&encoded).unwrap();
        assert_eq!(doc.signals().unwrap().len(), doc2.signals().unwrap().len());
    }

    #[test]
    fn test_encode_value_object_omits_kind_specific_fields() {
        let json = r#"{
  "schema_version": 1,
  "domain_types": [
    { "name": "TrackId", "kind": "value_object", "description": "ID", "approved": true }
  ]
}"#;
        let doc = decode(json).unwrap();
        let encoded = encode(&doc).unwrap();
        assert!(!encoded.contains("transitions_to"));
        assert!(!encoded.contains("expected_variants"));
        assert!(!encoded.contains("expected_methods"));
    }

    #[test]
    fn test_decode_signals_absent_returns_none() {
        let json = r#"{
  "schema_version": 1,
  "domain_types": [
    { "name": "Draft", "kind": "value_object", "description": "Draft" }
  ]
}"#;
        let doc = decode(json).unwrap();
        assert!(doc.signals().is_none());
    }

    #[test]
    fn test_round_trip_signals_with_items() {
        let json = r#"{
  "schema_version": 1,
  "domain_types": [
    { "name": "Draft", "kind": "value_object", "description": "Draft", "approved": true }
  ],
  "signals": [
    {
      "type_name": "Draft",
      "kind_tag": "value_object",
      "signal": "red",
      "found_type": false,
      "missing_items": ["Draft"]
    }
  ]
}"#;
        let doc = decode(json).unwrap();
        let sigs = doc.signals().unwrap();
        assert_eq!(sigs[0].signal(), ConfidenceSignal::Red);
        assert_eq!(sigs[0].missing_items(), &["Draft"]);

        let encoded = encode(&doc).unwrap();
        let doc2 = decode(&encoded).unwrap();
        let sigs2 = doc2.signals().unwrap();
        assert_eq!(sigs2[0].signal(), ConfidenceSignal::Red);
        assert_eq!(sigs2[0].missing_items(), &["Draft"]);
    }
}
