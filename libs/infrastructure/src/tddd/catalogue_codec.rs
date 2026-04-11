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
    SpecValidationError, TypeAction, TypestateTransitions,
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
/// DTO for the `action` field on a domain type entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TypeActionDto {
    Add,
    Modify,
    Reference,
    Delete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DomainTypeEntryDto {
    pub name: String,
    pub description: String,
    #[serde(default = "default_approved")]
    pub approved: bool,
    #[serde(default = "default_action", skip_serializing_if = "is_add_action")]
    pub action: TypeActionDto,
    #[serde(flatten)]
    pub kind: DomainTypeKindDto,
}

fn default_approved() -> bool {
    true
}

fn default_action() -> TypeActionDto {
    TypeActionDto::Add
}

fn is_add_action(action: &TypeActionDto) -> bool {
    matches!(action, TypeActionDto::Add)
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

    // Validate entry name uniqueness with delete+add pair exception.
    // Same name is allowed only when exactly 2 entries exist: one delete + one add,
    // and the two entries must have different kinds.
    // Rationale: signals are keyed by (type_name, kind_tag); a same-kind pair would
    // produce two signals with the same key, making one unaddressable and leading to
    // ambiguous rendering. Different kinds is also the primary use case (kind migration).
    {
        let mut name_entries: std::collections::HashMap<&str, Vec<(TypeAction, &str)>> =
            std::collections::HashMap::new();
        for entry in &entries {
            name_entries
                .entry(entry.name())
                .or_default()
                .push((entry.action(), entry.kind().kind_tag()));
        }
        for (name, pairs) in &name_entries {
            if pairs.len() < 2 {
                continue;
            }
            if pairs.len() > 2 {
                return Err(DomainTypesCodecError::InvalidEntry {
                    name: (*name).to_owned(),
                    reason: format!(
                        "name appears {} times (max 2 for delete+add pair)",
                        pairs.len()
                    ),
                });
            }
            // Exactly 2: must be one Delete + one Add
            let actions: Vec<TypeAction> = pairs.iter().map(|(a, _)| *a).collect();
            let has_delete = actions.contains(&TypeAction::Delete);
            let has_add = actions.contains(&TypeAction::Add);
            if !(has_delete && has_add) {
                return Err(DomainTypesCodecError::InvalidEntry {
                    name: (*name).to_owned(),
                    reason: format!(
                        "duplicate name requires exactly one delete + one add (got {:?})",
                        actions.iter().map(|a| a.action_tag()).collect::<Vec<_>>()
                    ),
                });
            }
            // The two entries must have different kinds.
            // Same kind would produce two signals with the same (type_name, kind_tag) key,
            // making one signal unaddressable and leading to ambiguous rendering.
            if let [(_, kind_a), (_, kind_b)] = pairs.as_slice() {
                if kind_a == kind_b {
                    return Err(DomainTypesCodecError::InvalidEntry {
                        name: (*name).to_owned(),
                        reason: format!(
                            "delete+add pair must have different kinds to avoid signal key \
                             collision (both are '{kind_a}')"
                        ),
                    });
                }
            }
        }
    }

    // Typestate transitions_to referential integrity: targets must exist AND be typestate kind.
    //
    // Two target sets:
    // - `live_typestate_names`: non-delete typestate entries (valid targets for non-delete entries)
    // - `all_typestate_names`: all typestate entries (valid targets for delete entries, which
    //   document an outgoing transition from the type's pre-deletion state)
    let all_typestate_names: std::collections::HashSet<&str> = entries
        .iter()
        .filter(|e| matches!(e.kind(), DomainTypeKind::Typestate { .. }))
        .map(|e| e.name())
        .collect();
    let live_typestate_names: std::collections::HashSet<&str> = entries
        .iter()
        .filter(|e| {
            matches!(e.kind(), DomainTypeKind::Typestate { .. }) && e.action() != TypeAction::Delete
        })
        .map(|e| e.name())
        .collect();
    for entry in &entries {
        if let DomainTypeKind::Typestate { transitions: TypestateTransitions::To(targets) } =
            entry.kind()
        {
            // Delete entries may reference any typestate (including other delete entries).
            // Non-delete entries must only reference live (non-delete) typestates.
            let valid_targets = if entry.action() == TypeAction::Delete {
                &all_typestate_names
            } else {
                &live_typestate_names
            };
            for target in targets {
                if !valid_targets.contains(target.as_str()) {
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
    let action = type_action_from_dto(dto.action);
    DomainTypeEntry::new(&dto.name, &dto.description, kind, action, dto.approved)
        .map_err(DomainTypesCodecError::Validation)
}

fn type_action_from_dto(dto: TypeActionDto) -> TypeAction {
    match dto {
        TypeActionDto::Add => TypeAction::Add,
        TypeActionDto::Modify => TypeAction::Modify,
        TypeActionDto::Reference => TypeAction::Reference,
        TypeActionDto::Delete => TypeAction::Delete,
    }
}

fn type_action_to_dto(action: TypeAction) -> TypeActionDto {
    match action {
        TypeAction::Add => TypeActionDto::Add,
        TypeAction::Modify => TypeActionDto::Modify,
        TypeAction::Reference => TypeActionDto::Reference,
        TypeAction::Delete => TypeActionDto::Delete,
    }
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
        action: type_action_to_dto(entry.action()),
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

    // --- TypeAction codec ---

    #[test]
    fn test_decode_action_absent_defaults_to_add() {
        let json = r#"{
  "schema_version": 1,
  "domain_types": [
    { "name": "Foo", "kind": "value_object", "description": "d" }
  ]
}"#;
        let doc = decode(json).unwrap();
        assert_eq!(doc.entries()[0].action(), TypeAction::Add);
    }

    #[test]
    fn test_decode_action_delete_parsed_correctly() {
        let json = r#"{
  "schema_version": 1,
  "domain_types": [
    { "name": "OldType", "kind": "value_object", "description": "d", "action": "delete" }
  ]
}"#;
        let doc = decode(json).unwrap();
        assert_eq!(doc.entries()[0].action(), TypeAction::Delete);
    }

    #[test]
    fn test_decode_action_modify_parsed_correctly() {
        let json = r#"{
  "schema_version": 1,
  "domain_types": [
    { "name": "Changed", "kind": "value_object", "description": "d", "action": "modify" }
  ]
}"#;
        let doc = decode(json).unwrap();
        assert_eq!(doc.entries()[0].action(), TypeAction::Modify);
    }

    #[test]
    fn test_decode_action_reference_parsed_correctly() {
        let json = r#"{
  "schema_version": 1,
  "domain_types": [
    { "name": "Ref", "kind": "value_object", "description": "d", "action": "reference" }
  ]
}"#;
        let doc = decode(json).unwrap();
        assert_eq!(doc.entries()[0].action(), TypeAction::Reference);
    }

    #[test]
    fn test_encode_add_action_omits_field() {
        let json = r#"{
  "schema_version": 1,
  "domain_types": [
    { "name": "Foo", "kind": "value_object", "description": "d" }
  ]
}"#;
        let doc = decode(json).unwrap();
        let encoded = encode(&doc).unwrap();
        assert!(!encoded.contains("\"action\""));
    }

    #[test]
    fn test_encode_delete_action_includes_field() {
        let json = r#"{
  "schema_version": 1,
  "domain_types": [
    { "name": "OldType", "kind": "value_object", "description": "d", "action": "delete" }
  ]
}"#;
        let doc = decode(json).unwrap();
        let encoded = encode(&doc).unwrap();
        assert!(encoded.contains("\"action\": \"delete\""));
    }

    #[test]
    fn test_round_trip_preserves_delete_action() {
        let json = r#"{
  "schema_version": 1,
  "domain_types": [
    { "name": "OldType", "kind": "value_object", "description": "d", "action": "delete" }
  ]
}"#;
        let doc = decode(json).unwrap();
        let encoded = encode(&doc).unwrap();
        let doc2 = decode(&encoded).unwrap();
        assert_eq!(doc2.entries()[0].action(), TypeAction::Delete);
    }

    #[test]
    fn test_decode_unknown_action_returns_error() {
        let json = r#"{
  "schema_version": 1,
  "domain_types": [
    { "name": "Foo", "kind": "value_object", "description": "d", "action": "rename" }
  ]
}"#;
        assert!(decode(json).is_err());
    }

    // --- Duplicate name validation (delete+add pair) ---

    #[test]
    fn test_decode_delete_add_pair_succeeds() {
        let json = r#"{
  "schema_version": 1,
  "domain_types": [
    { "name": "Foo", "kind": "value_object", "description": "old", "action": "delete" },
    { "name": "Foo", "kind": "trait_port", "description": "new", "action": "add", "expected_methods": ["find"] }
  ]
}"#;
        let doc = decode(json).unwrap();
        assert_eq!(doc.entries().len(), 2);
        assert_eq!(doc.entries()[0].action(), TypeAction::Delete);
        assert_eq!(doc.entries()[1].action(), TypeAction::Add);
    }

    #[test]
    fn test_decode_add_add_duplicate_returns_error() {
        let json = r#"{
  "schema_version": 1,
  "domain_types": [
    { "name": "Foo", "kind": "value_object", "description": "a" },
    { "name": "Foo", "kind": "value_object", "description": "b" }
  ]
}"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, DomainTypesCodecError::InvalidEntry { .. }));
    }

    #[test]
    fn test_decode_delete_delete_duplicate_returns_error() {
        let json = r#"{
  "schema_version": 1,
  "domain_types": [
    { "name": "Foo", "kind": "value_object", "description": "a", "action": "delete" },
    { "name": "Foo", "kind": "value_object", "description": "b", "action": "delete" }
  ]
}"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, DomainTypesCodecError::InvalidEntry { .. }));
    }

    #[test]
    fn test_decode_three_same_name_returns_error() {
        let json = r#"{
  "schema_version": 1,
  "domain_types": [
    { "name": "Foo", "kind": "value_object", "description": "a", "action": "delete" },
    { "name": "Foo", "kind": "value_object", "description": "b", "action": "add" },
    { "name": "Foo", "kind": "value_object", "description": "c", "action": "add" }
  ]
}"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, DomainTypesCodecError::InvalidEntry { .. }));
    }

    #[test]
    fn test_decode_delete_add_same_kind_returns_error() {
        // Signals are keyed by (type_name, kind_tag). A same-kind delete+add pair produces
        // two signals with the same key, making one unaddressable.
        let json = r#"{
  "schema_version": 1,
  "domain_types": [
    { "name": "Foo", "kind": "value_object", "description": "old", "action": "delete" },
    { "name": "Foo", "kind": "value_object", "description": "new" }
  ]
}"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, DomainTypesCodecError::InvalidEntry { .. }));
    }

    #[test]
    fn test_decode_delete_typestate_graph_succeeds() {
        // Deleting a connected typestate graph: OldDraft -> OldPublished, both deleted.
        // Delete entries may reference other delete-action typestates as targets.
        let json = r#"{
  "schema_version": 1,
  "domain_types": [
    { "name": "OldDraft", "kind": "typestate", "description": "old draft", "action": "delete", "transitions_to": ["OldPublished"] },
    { "name": "OldPublished", "kind": "typestate", "description": "old published", "action": "delete", "transitions_to": [] }
  ]
}"#;
        let doc = decode(json).unwrap();
        assert_eq!(doc.entries().len(), 2);
        assert_eq!(doc.entries()[0].action(), TypeAction::Delete);
        assert_eq!(doc.entries()[1].action(), TypeAction::Delete);
    }

    #[test]
    fn test_decode_non_delete_typestate_cannot_target_delete_entry() {
        // A live (non-delete) typestate must not transition to a delete-marked typestate.
        let json = r#"{
  "schema_version": 1,
  "domain_types": [
    { "name": "Active", "kind": "typestate", "description": "live state", "transitions_to": ["OldState"] },
    { "name": "OldState", "kind": "typestate", "description": "being deleted", "action": "delete", "transitions_to": [] }
  ]
}"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, DomainTypesCodecError::InvalidEntry { .. }));
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
