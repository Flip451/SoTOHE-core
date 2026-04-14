//! Serde codec for the type-catalogue file (`TypeCatalogueDocument` SSoT).
//!
//! T006 (TDDD-01 Phase 1 Task 6): schema bumped to v2. Top-level key renamed
//! from `domain_types` → `type_definitions`. `trait_port.expected_methods`
//! changed from `Vec<String>` to `Vec<MethodDto>` (L1 signatures with
//! name/receiver/params/returns/is_async). Any `ty` or `returns` string
//! containing `::` is rejected by the codec (last-segment enforcement per
//! ADR 0002 §D2).
//!
//! T002 (TDDD-02 Task 2): added 7 new variants (`secondary_port`,
//! `application_service`, `use_case`, `interactor`, `dto`, `command`, `query`,
//! `factory`). `trait_port` removed — any JSON containing `"kind":
//! "trait_port"` is now rejected with `InvalidEntry`. `schema_version` stays
//! at 2; this is an additive change within v2.
//!
//! The JSON schema uses an internally-tagged enum (`"kind"` field) with
//! `#[serde(flatten)]` so that kind-specific fields are required at the type
//! level — illegal field combinations are rejected by serde, not by manual
//! validation.

use domain::tddd::catalogue::{MethodDeclaration, ParamDeclaration};
use domain::{
    ConfidenceSignal, SpecValidationError, TypeAction, TypeCatalogueDocument, TypeCatalogueEntry,
    TypeDefinitionKind, TypeSignal, TypestateTransitions,
};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Codec error for the type-catalogue JSON file.
#[derive(Debug, thiserror::Error)]
pub enum TypeCatalogueCodecError {
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("validation error: {0}")]
    Validation(#[from] SpecValidationError),

    #[error(
        "unsupported schema_version: expected 2, got {0}. \
         Regenerate the catalogue with the v2 layout (top-level key `type_definitions`, \
         `trait_port.expected_methods` as structured signatures)."
    )]
    UnsupportedSchemaVersion(u32),

    #[error("invalid entry '{name}': {reason}")]
    InvalidEntry { name: String, reason: String },
}

// ---------------------------------------------------------------------------
// DTO types
// ---------------------------------------------------------------------------

/// Top-level DTO for the catalogue JSON file.
///
/// `deny_unknown_fields` ensures that a file that accidentally retains the old
/// `domain_types` key (or any other stale key) is rejected at decode time
/// rather than silently accepted with an empty `type_definitions` list.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TypeCatalogueDocDto {
    pub schema_version: u32,
    #[serde(default)]
    pub type_definitions: Vec<TypeCatalogueEntryDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signals: Option<Vec<TypeSignalDto>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TypeActionDto {
    Add,
    Modify,
    Reference,
    Delete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TypeCatalogueEntryDto {
    pub name: String,
    pub description: String,
    #[serde(default = "default_approved")]
    pub approved: bool,
    #[serde(default = "default_action", skip_serializing_if = "is_add_action")]
    pub action: TypeActionDto,
    #[serde(flatten)]
    pub kind: TypeDefinitionKindDto,
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
///
/// T002: `TraitPort` removed; `SecondaryPort` and `ApplicationService` added
/// (same shape — `expected_methods`). Seven existence-check-only variants
/// added: `UseCase`, `Interactor`, `Dto`, `Command`, `Query`, `Factory`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum TypeDefinitionKindDto {
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
    /// Hexagonal secondary (driven) port — replaces `TraitPort`.
    SecondaryPort {
        expected_methods: Vec<MethodDto>,
    },
    /// Hexagonal primary (driving) port — same shape as `SecondaryPort`.
    ApplicationService {
        expected_methods: Vec<MethodDto>,
    },
    /// Struct-only use case; existence check only.
    UseCase {},
    /// ApplicationService implementation struct; existence check only.
    Interactor {},
    /// Pure data-transfer object struct; existence check only.
    Dto {},
    /// CQRS command object struct; existence check only.
    Command {},
    /// CQRS query object struct; existence check only.
    Query {},
    /// Aggregate/entity factory struct; existence check only.
    Factory {},
}

/// T006 method signature DTO — mirrors `MethodDeclaration`.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ParamDto {
    name: String,
    ty: String,
}

/// DTO for a per-type signal evaluation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TypeSignalDto {
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

/// Decodes a type-catalogue JSON string into a `TypeCatalogueDocument`.
///
/// # Errors
///
/// Returns `TypeCatalogueCodecError` when:
/// - The string is not valid JSON.
/// - `schema_version` is not 2.
/// - Any entry has an unknown `kind` tag or missing required fields.
/// - Any `ty` or `returns` string contains `::` (last-segment enforcement).
/// - Any entry fails domain validation (e.g. empty name).
pub fn decode(json: &str) -> Result<TypeCatalogueDocument, TypeCatalogueCodecError> {
    // Phase 1: extract schema_version first so that non-v2 payloads (e.g. v1 with
    // the old `domain_types` key) get the actionable `UnsupportedSchemaVersion`
    // error with migration guidance rather than a generic unknown-field error from
    // `deny_unknown_fields`.  Parsing as `serde_json::Value` is cheap and avoids
    // a second full deserialization on the happy path.
    let raw: serde_json::Value = serde_json::from_str(json)?;
    let schema_version =
        raw.get("schema_version").and_then(|v| v.as_u64()).map(|v| v as u32).unwrap_or(0);
    if schema_version != 2 {
        return Err(TypeCatalogueCodecError::UnsupportedSchemaVersion(schema_version));
    }

    // Phase 1.5: reject existence-only entries that carry stale kind-specific fields.
    //
    // `#[serde(flatten)]` + internally-tagged enum means `deny_unknown_fields` on
    // `TypeCatalogueEntryDto` does not propagate to the individual variant struct
    // deserialization (known serde limitation).  Fields like `expected_methods` or
    // `expected_variants` on `use_case`/`interactor`/`dto`/`command`/`query`/`factory`
    // would be silently dropped by serde instead of triggering an error.  This pass
    // catches illegal kind/field combinations at the codec boundary, before Phase 2.
    if let Some(entries) = raw.get("type_definitions").and_then(|v| v.as_array()) {
        for entry in entries {
            let entry_obj = entry.as_object();
            let kind = entry_obj.and_then(|o| o.get("kind")).and_then(|v| v.as_str()).unwrap_or("");
            let name = entry_obj
                .and_then(|o| o.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("<unnamed>");
            // Existence-only kinds must not carry `expected_methods`, `expected_variants`,
            // or `transitions_to` (those fields belong to other variants).
            const EXISTENCE_ONLY_KINDS: &[&str] =
                &["use_case", "interactor", "dto", "command", "query", "factory", "value_object"];
            const FORBIDDEN_FIELDS: &[&str] =
                &["expected_methods", "expected_variants", "transitions_to"];
            if EXISTENCE_ONLY_KINDS.contains(&kind) {
                if let Some(obj) = entry_obj {
                    for forbidden in FORBIDDEN_FIELDS {
                        if obj.contains_key(*forbidden) {
                            return Err(TypeCatalogueCodecError::InvalidEntry {
                                name: name.to_owned(),
                                reason: format!(
                                    "kind '{}' does not support field '{}' — \
                                     existence-only kinds carry no structural fields",
                                    kind, forbidden
                                ),
                            });
                        }
                    }
                }
            }
        }
    }

    // Phase 2: full deserialisation with `deny_unknown_fields` to catch stale
    // keys (e.g. a file that keeps both `type_definitions` and `domain_types`).
    let dto: TypeCatalogueDocDto = serde_json::from_value(raw)?;

    let mut entries = Vec::with_capacity(dto.type_definitions.len());
    for entry_dto in &dto.type_definitions {
        entries.push(type_catalogue_entry_from_dto(entry_dto)?);
    }

    // Validate entry name uniqueness with delete+add pair exception.
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
                return Err(TypeCatalogueCodecError::InvalidEntry {
                    name: (*name).to_owned(),
                    reason: format!(
                        "name appears {} times (max 2 for delete+add pair)",
                        pairs.len()
                    ),
                });
            }
            let actions: Vec<TypeAction> = pairs.iter().map(|(a, _)| *a).collect();
            let has_delete = actions.contains(&TypeAction::Delete);
            let has_add = actions.contains(&TypeAction::Add);
            if !(has_delete && has_add) {
                return Err(TypeCatalogueCodecError::InvalidEntry {
                    name: (*name).to_owned(),
                    reason: format!(
                        "duplicate name requires exactly one delete + one add (got {:?})",
                        actions.iter().map(|a| a.action_tag()).collect::<Vec<_>>()
                    ),
                });
            }
            if let [(_, kind_a), (_, kind_b)] = pairs.as_slice() {
                if kind_a == kind_b {
                    return Err(TypeCatalogueCodecError::InvalidEntry {
                        name: (*name).to_owned(),
                        reason: format!(
                            "delete+add pair must have different kinds to avoid signal key \
                             collision (both are '{kind_a}')"
                        ),
                    });
                }
                let is_method_bearing =
                    |k: &str| -> bool { k == "secondary_port" || k == "application_service" };
                let is_trait_a = is_method_bearing(kind_a);
                let is_trait_b = is_method_bearing(kind_b);
                if is_trait_a == is_trait_b {
                    return Err(TypeCatalogueCodecError::InvalidEntry {
                        name: (*name).to_owned(),
                        reason: format!(
                            "delete+add pair must cross the trait/non-trait partition \
                             ('{kind_a}' and '{kind_b}' are in the same partition). \
                             Use action:\"modify\" for same-partition kind changes"
                        ),
                    });
                }
            }
        }
    }

    // Typestate transitions_to referential integrity.
    let all_typestate_names: std::collections::HashSet<&str> = entries
        .iter()
        .filter(|e| matches!(e.kind(), TypeDefinitionKind::Typestate { .. }))
        .map(|e| e.name())
        .collect();
    let live_typestate_names: std::collections::HashSet<&str> = entries
        .iter()
        .filter(|e| {
            matches!(e.kind(), TypeDefinitionKind::Typestate { .. })
                && e.action() != TypeAction::Delete
        })
        .map(|e| e.name())
        .collect();
    for entry in &entries {
        if let TypeDefinitionKind::Typestate { transitions: TypestateTransitions::To(targets) } =
            entry.kind()
        {
            let valid_targets = if entry.action() == TypeAction::Delete {
                &all_typestate_names
            } else {
                &live_typestate_names
            };
            for target in targets {
                if !valid_targets.contains(target.as_str()) {
                    return Err(TypeCatalogueCodecError::InvalidEntry {
                        name: entry.name().to_owned(),
                        reason: format!(
                            "transitions_to target '{target}' is not a typestate entry"
                        ),
                    });
                }
            }
        }
    }

    let mut doc = TypeCatalogueDocument::new(dto.schema_version, entries);

    if let Some(signal_dtos) = dto.signals {
        let signals =
            signal_dtos.iter().map(type_signal_from_dto).collect::<Result<Vec<_>, _>>()?;
        doc.set_signals(signals);
    }

    Ok(doc)
}

/// Encodes a `TypeCatalogueDocument` to a pretty-printed JSON string.
///
/// # Errors
///
/// Returns `TypeCatalogueCodecError` when:
/// - Any `SecondaryPort` or `ApplicationService` method's `returns` or any
///   param's `ty` contains `::` (L1 invariant, same rule enforced by `decode`).
///   This prevents `encode` from producing JSON that `decode` would immediately
///   reject, preserving the round-trip guarantee.
/// - Serialization fails for any other reason (`TypeCatalogueCodecError::Json`).
pub fn encode(doc: &TypeCatalogueDocument) -> Result<String, TypeCatalogueCodecError> {
    let dto = type_catalogue_doc_to_dto(doc)?;
    serde_json::to_string_pretty(&dto).map_err(TypeCatalogueCodecError::Json)
}

// ---------------------------------------------------------------------------
// Conversion helpers: DTO → domain
// ---------------------------------------------------------------------------

fn type_catalogue_entry_from_dto(
    dto: &TypeCatalogueEntryDto,
) -> Result<TypeCatalogueEntry, TypeCatalogueCodecError> {
    let kind = type_definition_kind_from_dto(&dto.name, &dto.kind)?;
    let action = type_action_from_dto(dto.action);
    TypeCatalogueEntry::new(&dto.name, &dto.description, kind, action, dto.approved)
        .map_err(TypeCatalogueCodecError::Validation)
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

fn type_definition_kind_from_dto(
    entry_name: &str,
    dto: &TypeDefinitionKindDto,
) -> Result<TypeDefinitionKind, TypeCatalogueCodecError> {
    match dto {
        TypeDefinitionKindDto::Typestate { transitions_to } => {
            let transitions = if transitions_to.is_empty() {
                TypestateTransitions::Terminal
            } else {
                TypestateTransitions::To(transitions_to.clone())
            };
            Ok(TypeDefinitionKind::Typestate { transitions })
        }
        TypeDefinitionKindDto::Enum { expected_variants } => {
            Ok(TypeDefinitionKind::Enum { expected_variants: expected_variants.clone() })
        }
        TypeDefinitionKindDto::ValueObject {} => Ok(TypeDefinitionKind::ValueObject),
        TypeDefinitionKindDto::ErrorType { expected_variants } => {
            Ok(TypeDefinitionKind::ErrorType { expected_variants: expected_variants.clone() })
        }
        TypeDefinitionKindDto::SecondaryPort { expected_methods } => {
            let decls = decode_method_list(entry_name, expected_methods)?;
            Ok(TypeDefinitionKind::SecondaryPort { expected_methods: decls })
        }
        TypeDefinitionKindDto::ApplicationService { expected_methods } => {
            let decls = decode_method_list(entry_name, expected_methods)?;
            Ok(TypeDefinitionKind::ApplicationService { expected_methods: decls })
        }
        TypeDefinitionKindDto::UseCase {} => Ok(TypeDefinitionKind::UseCase),
        TypeDefinitionKindDto::Interactor {} => Ok(TypeDefinitionKind::Interactor),
        TypeDefinitionKindDto::Dto {} => Ok(TypeDefinitionKind::Dto),
        TypeDefinitionKindDto::Command {} => Ok(TypeDefinitionKind::Command),
        TypeDefinitionKindDto::Query {} => Ok(TypeDefinitionKind::Query),
        TypeDefinitionKindDto::Factory {} => Ok(TypeDefinitionKind::Factory),
    }
}

/// Shared helper: decode a `Vec<MethodDto>` into `Vec<MethodDeclaration>`.
///
/// Used by both `SecondaryPort` and `ApplicationService` decode paths, which
/// share the same `expected_methods` shape (mirrors `evaluate_trait_methods`
/// on the domain side).
fn decode_method_list(
    entry_name: &str,
    dtos: &[MethodDto],
) -> Result<Vec<MethodDeclaration>, TypeCatalogueCodecError> {
    let mut decls = Vec::with_capacity(dtos.len());
    for m in dtos {
        decls.push(method_from_dto(entry_name, m)?);
    }
    Ok(decls)
}

fn method_from_dto(
    entry_name: &str,
    dto: &MethodDto,
) -> Result<MethodDeclaration, TypeCatalogueCodecError> {
    // L1 enforcement: `returns` must not contain `::` (last-segment only).
    if dto.returns.contains("::") {
        return Err(TypeCatalogueCodecError::InvalidEntry {
            name: entry_name.to_owned(),
            reason: format!(
                "method '{}' returns contains '::' — L1 catalogue entries must use \
                 last-segment short names: '{}'",
                dto.name, dto.returns
            ),
        });
    }
    let mut params = Vec::with_capacity(dto.params.len());
    for p in &dto.params {
        if p.ty.contains("::") {
            return Err(TypeCatalogueCodecError::InvalidEntry {
                name: entry_name.to_owned(),
                reason: format!(
                    "method '{}' param '{}' ty contains '::' — L1 catalogue entries \
                     must use last-segment short names: '{}'",
                    dto.name, p.name, p.ty
                ),
            });
        }
        params.push(ParamDeclaration::new(p.name.clone(), p.ty.clone()));
    }
    Ok(MethodDeclaration::new(
        dto.name.clone(),
        dto.receiver.clone(),
        params,
        dto.returns.clone(),
        dto.is_async,
    ))
}

fn type_signal_from_dto(dto: &TypeSignalDto) -> Result<TypeSignal, TypeCatalogueCodecError> {
    let signal = confidence_signal_from_str(&dto.signal).ok_or_else(|| {
        TypeCatalogueCodecError::InvalidEntry {
            name: dto.type_name.clone(),
            reason: format!("unknown signal value '{}'", dto.signal),
        }
    })?;
    Ok(TypeSignal::new(
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

fn type_catalogue_doc_to_dto(
    doc: &TypeCatalogueDocument,
) -> Result<TypeCatalogueDocDto, TypeCatalogueCodecError> {
    let type_definitions =
        doc.entries().iter().map(type_catalogue_entry_to_dto).collect::<Result<Vec<_>, _>>()?;
    let signals = doc.signals().map(|sigs| sigs.iter().map(type_signal_to_dto).collect());
    // Always emit schema_version 2, regardless of the in-memory value, so that
    // encode→decode round-trips correctly. The in-memory schema_version field is
    // an informational tag; v2 is the only version the current codec can decode.
    Ok(TypeCatalogueDocDto { schema_version: 2, type_definitions, signals })
}

fn type_catalogue_entry_to_dto(
    entry: &TypeCatalogueEntry,
) -> Result<TypeCatalogueEntryDto, TypeCatalogueCodecError> {
    let kind = match entry.kind() {
        TypeDefinitionKind::Typestate { transitions } => {
            let transitions_to = match transitions {
                TypestateTransitions::Terminal => vec![],
                TypestateTransitions::To(v) => v.clone(),
            };
            TypeDefinitionKindDto::Typestate { transitions_to }
        }
        TypeDefinitionKind::Enum { expected_variants } => {
            TypeDefinitionKindDto::Enum { expected_variants: expected_variants.clone() }
        }
        TypeDefinitionKind::ValueObject => TypeDefinitionKindDto::ValueObject {},
        TypeDefinitionKind::ErrorType { expected_variants } => {
            TypeDefinitionKindDto::ErrorType { expected_variants: expected_variants.clone() }
        }
        TypeDefinitionKind::SecondaryPort { expected_methods } => {
            let dtos = encode_method_list(entry.name(), expected_methods)?;
            TypeDefinitionKindDto::SecondaryPort { expected_methods: dtos }
        }
        TypeDefinitionKind::ApplicationService { expected_methods } => {
            let dtos = encode_method_list(entry.name(), expected_methods)?;
            TypeDefinitionKindDto::ApplicationService { expected_methods: dtos }
        }
        TypeDefinitionKind::UseCase => TypeDefinitionKindDto::UseCase {},
        TypeDefinitionKind::Interactor => TypeDefinitionKindDto::Interactor {},
        TypeDefinitionKind::Dto => TypeDefinitionKindDto::Dto {},
        TypeDefinitionKind::Command => TypeDefinitionKindDto::Command {},
        TypeDefinitionKind::Query => TypeDefinitionKindDto::Query {},
        TypeDefinitionKind::Factory => TypeDefinitionKindDto::Factory {},
    };
    Ok(TypeCatalogueEntryDto {
        name: entry.name().to_owned(),
        description: entry.description().to_owned(),
        approved: entry.approved(),
        action: type_action_to_dto(entry.action()),
        kind,
    })
}

/// Shared helper: encode a `Vec<MethodDeclaration>` into `Vec<MethodDto>`.
///
/// Used by both `SecondaryPort` and `ApplicationService` encode paths, mirroring
/// `decode_method_list` on the decode side.
fn encode_method_list(
    entry_name: &str,
    methods: &[MethodDeclaration],
) -> Result<Vec<MethodDto>, TypeCatalogueCodecError> {
    methods.iter().map(|m| method_to_dto(entry_name, m)).collect()
}

fn method_to_dto(
    entry_name: &str,
    method: &MethodDeclaration,
) -> Result<MethodDto, TypeCatalogueCodecError> {
    // L1 enforcement at encode time: mirror the same check as `method_from_dto` so that
    // `encode(doc)` never produces JSON that `decode` would immediately reject.
    if method.returns().contains("::") {
        return Err(TypeCatalogueCodecError::InvalidEntry {
            name: entry_name.to_owned(),
            reason: format!(
                "method '{}' returns contains '::' — L1 catalogue entries must use \
                 last-segment short names: '{}'",
                method.name(),
                method.returns()
            ),
        });
    }
    let mut params = Vec::with_capacity(method.params().len());
    for p in method.params() {
        if p.ty().contains("::") {
            return Err(TypeCatalogueCodecError::InvalidEntry {
                name: entry_name.to_owned(),
                reason: format!(
                    "method '{}' param '{}' ty contains '::' — L1 catalogue entries \
                     must use last-segment short names: '{}'",
                    method.name(),
                    p.name(),
                    p.ty()
                ),
            });
        }
        params.push(ParamDto { name: p.name().to_owned(), ty: p.ty().to_owned() });
    }
    Ok(MethodDto {
        name: method.name().to_owned(),
        receiver: method.receiver().map(str::to_owned),
        params,
        returns: method.returns().to_owned(),
        is_async: method.is_async(),
    })
}

fn type_signal_to_dto(sig: &TypeSignal) -> TypeSignalDto {
    TypeSignalDto {
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
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;

    const FULL_JSON: &str = r#"{
  "schema_version": 2,
  "type_definitions": [
    { "name": "Draft", "kind": "typestate", "description": "Draft state", "transitions_to": ["Published"], "approved": true },
    { "name": "Published", "kind": "typestate", "description": "Published state", "transitions_to": [], "approved": true },
    { "name": "TrackStatus", "kind": "enum", "description": "Track status", "expected_variants": ["Planned", "Done"], "approved": true },
    { "name": "TrackId", "kind": "value_object", "description": "Track identifier", "approved": true },
    { "name": "SchemaExportError", "kind": "error_type", "description": "Export error", "expected_variants": ["NightlyNotFound"], "approved": true },
    {
      "name": "SchemaExporter",
      "kind": "secondary_port",
      "description": "Export port",
      "expected_methods": [
        {
          "name": "export",
          "receiver": "&self",
          "params": [{ "name": "crate_name", "ty": "str" }],
          "returns": "Result<SchemaExport, SchemaExportError>",
          "is_async": false
        }
      ],
      "approved": true
    }
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
            TypeDefinitionKind::Typestate { transitions: TypestateTransitions::To(v) } if v == &["Published"]
        ));
    }

    #[test]
    fn test_decode_enum_kind() {
        let doc = decode(FULL_JSON).unwrap();
        assert!(matches!(
            doc.entries()[2].kind(),
            TypeDefinitionKind::Enum { expected_variants } if expected_variants == &["Planned", "Done"]
        ));
    }

    #[test]
    fn test_decode_value_object_kind() {
        let doc = decode(FULL_JSON).unwrap();
        assert!(matches!(doc.entries()[3].kind(), TypeDefinitionKind::ValueObject));
    }

    #[test]
    fn test_decode_error_type_kind() {
        let doc = decode(FULL_JSON).unwrap();
        assert!(matches!(
            doc.entries()[4].kind(),
            TypeDefinitionKind::ErrorType { expected_variants } if expected_variants == &["NightlyNotFound"]
        ));
    }

    #[test]
    fn test_decode_secondary_port_with_structured_methods() {
        let doc = decode(FULL_JSON).unwrap();
        let kind = doc.entries()[5].kind();
        let TypeDefinitionKind::SecondaryPort { expected_methods } = kind else {
            panic!("expected SecondaryPort kind");
        };
        assert_eq!(expected_methods.len(), 1);
        let method = &expected_methods[0];
        assert_eq!(method.name(), "export");
        assert_eq!(method.receiver(), Some("&self"));
        assert_eq!(method.params().len(), 1);
        assert_eq!(method.params()[0].name(), "crate_name");
        assert_eq!(method.params()[0].ty(), "str");
        assert_eq!(method.returns(), "Result<SchemaExport, SchemaExportError>");
        assert!(!method.is_async());
    }

    #[test]
    fn test_decode_approved_field() {
        let doc = decode(FULL_JSON).unwrap();
        assert!(doc.entries()[0].approved());
    }

    #[test]
    fn test_decode_approved_defaults_to_true_when_absent() {
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    { "name": "Foo", "kind": "value_object", "description": "no approved field" }
  ]
}"#;
        let doc = decode(json).unwrap();
        assert!(doc.entries()[0].approved());
    }

    #[test]
    fn test_decode_empty_type_definitions_array() {
        let json = r#"{ "schema_version": 2, "type_definitions": [] }"#;
        let doc = decode(json).unwrap();
        assert_eq!(doc.entries().len(), 0);
    }

    #[test]
    fn test_decode_wrong_schema_version_returns_error() {
        let json = r#"{ "schema_version": 99, "type_definitions": [] }"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, TypeCatalogueCodecError::UnsupportedSchemaVersion(99)));
    }

    #[test]
    fn test_decode_v1_rejected_with_rerun_hint() {
        let json = r#"{ "schema_version": 1, "domain_types": [] }"#;
        let err = decode(json).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("type_definitions"), "expected v2 hint, got: {msg}");
    }

    #[test]
    fn test_decode_unknown_kind_returns_error() {
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    { "name": "Foo", "kind": "unknown_kind", "description": "bad", "approved": true }
  ]
}"#;
        assert!(decode(json).is_err());
    }

    #[test]
    fn test_decode_empty_name_returns_validation_error() {
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    { "name": "", "kind": "value_object", "description": "bad", "approved": true }
  ]
}"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, TypeCatalogueCodecError::Validation(_)));
    }

    #[test]
    fn test_decode_enum_without_expected_variants_returns_error() {
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    { "name": "Bad", "kind": "enum", "description": "missing field" }
  ]
}"#;
        assert!(decode(json).is_err());
    }

    #[test]
    fn test_decode_secondary_port_without_expected_methods_returns_error() {
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    { "name": "Bad", "kind": "secondary_port", "description": "missing field" }
  ]
}"#;
        assert!(decode(json).is_err());
    }

    #[test]
    fn test_decode_trait_port_tag_rejected_with_error() {
        // T002: "trait_port" kind_tag is removed; any JSON that still uses it
        // must be rejected. This documents the no-backward-compatibility decision.
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    {
      "name": "LegacyPort",
      "kind": "trait_port",
      "description": "old tag",
      "expected_methods": [
        { "name": "find", "receiver": "&self", "params": [], "returns": "()", "is_async": false }
      ]
    }
  ]
}"#;
        assert!(
            decode(json).is_err(),
            "\"trait_port\" kind_tag must be rejected after T002 rename"
        );
    }

    #[test]
    fn test_decode_secondary_port_returns_with_double_colon_rejected() {
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    {
      "name": "Repo",
      "kind": "secondary_port",
      "description": "port",
      "expected_methods": [
        { "name": "save", "receiver": "&self", "params": [], "returns": "std::io::Result<()>", "is_async": false }
      ]
    }
  ]
}"#;
        let err = decode(json).unwrap_err();
        match err {
            TypeCatalogueCodecError::InvalidEntry { reason, .. } => {
                assert!(reason.contains("'::'"), "expected '::' rejection, got: {reason}");
            }
            other => panic!("expected InvalidEntry, got {other:?}"),
        }
    }

    #[test]
    fn test_decode_secondary_port_param_ty_with_double_colon_rejected() {
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    {
      "name": "Repo",
      "kind": "secondary_port",
      "description": "port",
      "expected_methods": [
        {
          "name": "save",
          "receiver": "&self",
          "params": [{ "name": "id", "ty": "domain::TrackId" }],
          "returns": "()",
          "is_async": false
        }
      ]
    }
  ]
}"#;
        let err = decode(json).unwrap_err();
        match err {
            TypeCatalogueCodecError::InvalidEntry { reason, .. } => {
                assert!(reason.contains("'::'"), "expected '::' rejection, got: {reason}");
            }
            other => panic!("expected InvalidEntry, got {other:?}"),
        }
    }

    #[test]
    fn test_decode_invalid_transition_target_returns_error() {
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    { "name": "Draft", "kind": "typestate", "description": "d", "transitions_to": ["NonExistent"] }
  ]
}"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, TypeCatalogueCodecError::InvalidEntry { .. }));
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
  "schema_version": 2,
  "type_definitions": [
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
  "schema_version": 2,
  "type_definitions": [
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
  "schema_version": 2,
  "type_definitions": [
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
  "schema_version": 2,
  "type_definitions": [
    { "name": "Foo", "kind": "value_object", "description": "d" }
  ]
}"#;
        let doc = decode(json).unwrap();
        assert_eq!(doc.entries()[0].action(), TypeAction::Add);
    }

    #[test]
    fn test_decode_action_delete_parsed_correctly() {
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    { "name": "OldType", "kind": "value_object", "description": "d", "action": "delete" }
  ]
}"#;
        let doc = decode(json).unwrap();
        assert_eq!(doc.entries()[0].action(), TypeAction::Delete);
    }

    #[test]
    fn test_decode_action_modify_parsed_correctly() {
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    { "name": "Changed", "kind": "value_object", "description": "d", "action": "modify" }
  ]
}"#;
        let doc = decode(json).unwrap();
        assert_eq!(doc.entries()[0].action(), TypeAction::Modify);
    }

    #[test]
    fn test_decode_action_reference_parsed_correctly() {
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    { "name": "Ref", "kind": "value_object", "description": "d", "action": "reference" }
  ]
}"#;
        let doc = decode(json).unwrap();
        assert_eq!(doc.entries()[0].action(), TypeAction::Reference);
    }

    #[test]
    fn test_encode_add_action_omits_field() {
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
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
  "schema_version": 2,
  "type_definitions": [
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
  "schema_version": 2,
  "type_definitions": [
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
  "schema_version": 2,
  "type_definitions": [
    { "name": "Foo", "kind": "value_object", "description": "d", "action": "rename" }
  ]
}"#;
        assert!(decode(json).is_err());
    }

    // --- Duplicate name validation ---

    #[test]
    fn test_decode_delete_add_pair_succeeds() {
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    { "name": "Foo", "kind": "value_object", "description": "old", "action": "delete" },
    {
      "name": "Foo",
      "kind": "secondary_port",
      "description": "new",
      "action": "add",
      "expected_methods": [
        { "name": "find", "receiver": "&self", "params": [], "returns": "()", "is_async": false }
      ]
    }
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
  "schema_version": 2,
  "type_definitions": [
    { "name": "Foo", "kind": "value_object", "description": "a" },
    { "name": "Foo", "kind": "value_object", "description": "b" }
  ]
}"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, TypeCatalogueCodecError::InvalidEntry { .. }));
    }

    #[test]
    fn test_decode_delete_typestate_graph_succeeds() {
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
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
    fn test_round_trip_signals_with_items() {
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
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

    // --- T002: new variant decode tests ---

    #[test]
    fn test_decode_application_service_kind() {
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    {
      "name": "HookHandler",
      "kind": "application_service",
      "description": "Primary port",
      "expected_methods": [
        { "name": "handle", "receiver": "&self", "params": [{ "name": "ctx", "ty": "HookContext" }], "returns": "Result<HookVerdict, HookError>", "is_async": false }
      ]
    }
  ]
}"#;
        let doc = decode(json).unwrap();
        let kind = doc.entries()[0].kind();
        let TypeDefinitionKind::ApplicationService { expected_methods } = kind else {
            panic!("expected ApplicationService kind, got {:?}", kind);
        };
        assert_eq!(expected_methods.len(), 1);
        assert_eq!(expected_methods[0].name(), "handle");
    }

    #[test]
    fn test_decode_use_case_kind() {
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    { "name": "SaveTrackUseCase", "kind": "use_case", "description": "Save track use case" }
  ]
}"#;
        let doc = decode(json).unwrap();
        assert!(matches!(doc.entries()[0].kind(), TypeDefinitionKind::UseCase));
    }

    #[test]
    fn test_decode_interactor_kind() {
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    { "name": "SaveTrackInteractor", "kind": "interactor", "description": "Interactor" }
  ]
}"#;
        let doc = decode(json).unwrap();
        assert!(matches!(doc.entries()[0].kind(), TypeDefinitionKind::Interactor));
    }

    #[test]
    fn test_decode_dto_kind() {
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    { "name": "CreateUserDto", "kind": "dto", "description": "DTO" }
  ]
}"#;
        let doc = decode(json).unwrap();
        assert!(matches!(doc.entries()[0].kind(), TypeDefinitionKind::Dto));
    }

    #[test]
    fn test_decode_command_kind() {
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    { "name": "CreateUserCommand", "kind": "command", "description": "CQRS command" }
  ]
}"#;
        let doc = decode(json).unwrap();
        assert!(matches!(doc.entries()[0].kind(), TypeDefinitionKind::Command));
    }

    #[test]
    fn test_decode_query_kind() {
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    { "name": "GetUserQuery", "kind": "query", "description": "CQRS query" }
  ]
}"#;
        let doc = decode(json).unwrap();
        assert!(matches!(doc.entries()[0].kind(), TypeDefinitionKind::Query));
    }

    #[test]
    fn test_decode_factory_kind() {
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    { "name": "UserFactory", "kind": "factory", "description": "Factory" }
  ]
}"#;
        let doc = decode(json).unwrap();
        assert!(matches!(doc.entries()[0].kind(), TypeDefinitionKind::Factory));
    }

    // --- T002: new variant encode/round-trip tests ---

    #[test]
    fn test_round_trip_application_service() {
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    {
      "name": "HookHandler",
      "kind": "application_service",
      "description": "Primary port",
      "expected_methods": [
        { "name": "handle", "receiver": "&self", "params": [], "returns": "Result<HookVerdict, HookError>", "is_async": false }
      ]
    }
  ]
}"#;
        let doc = decode(json).unwrap();
        let encoded = encode(&doc).unwrap();
        let doc2 = decode(&encoded).unwrap();
        assert_eq!(doc2.entries()[0].kind(), doc.entries()[0].kind());
        assert!(encoded.contains("\"application_service\""));
    }

    #[test]
    fn test_round_trip_use_case() {
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    { "name": "SaveTrackUseCase", "kind": "use_case", "description": "use case" }
  ]
}"#;
        let doc = decode(json).unwrap();
        let encoded = encode(&doc).unwrap();
        let doc2 = decode(&encoded).unwrap();
        assert_eq!(doc2.entries()[0].kind(), doc.entries()[0].kind());
        assert!(encoded.contains("\"use_case\""));
    }

    #[test]
    fn test_round_trip_interactor() {
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    { "name": "SaveTrackInteractor", "kind": "interactor", "description": "interactor" }
  ]
}"#;
        let doc = decode(json).unwrap();
        let encoded = encode(&doc).unwrap();
        let doc2 = decode(&encoded).unwrap();
        assert_eq!(doc2.entries()[0].kind(), doc.entries()[0].kind());
        assert!(encoded.contains("\"interactor\""));
    }

    #[test]
    fn test_round_trip_dto() {
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    { "name": "CreateUserDto", "kind": "dto", "description": "dto" }
  ]
}"#;
        let doc = decode(json).unwrap();
        let encoded = encode(&doc).unwrap();
        let doc2 = decode(&encoded).unwrap();
        assert_eq!(doc2.entries()[0].kind(), doc.entries()[0].kind());
        assert!(encoded.contains("\"dto\""));
    }

    #[test]
    fn test_round_trip_command() {
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    { "name": "CreateUserCommand", "kind": "command", "description": "command" }
  ]
}"#;
        let doc = decode(json).unwrap();
        let encoded = encode(&doc).unwrap();
        let doc2 = decode(&encoded).unwrap();
        assert_eq!(doc2.entries()[0].kind(), doc.entries()[0].kind());
        assert!(encoded.contains("\"command\""));
    }

    #[test]
    fn test_round_trip_query() {
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    { "name": "GetUserQuery", "kind": "query", "description": "query" }
  ]
}"#;
        let doc = decode(json).unwrap();
        let encoded = encode(&doc).unwrap();
        let doc2 = decode(&encoded).unwrap();
        assert_eq!(doc2.entries()[0].kind(), doc.entries()[0].kind());
        assert!(encoded.contains("\"query\""));
    }

    #[test]
    fn test_round_trip_factory() {
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    { "name": "UserFactory", "kind": "factory", "description": "factory" }
  ]
}"#;
        let doc = decode(json).unwrap();
        let encoded = encode(&doc).unwrap();
        let doc2 = decode(&encoded).unwrap();
        assert_eq!(doc2.entries()[0].kind(), doc.entries()[0].kind());
        assert!(encoded.contains("\"factory\""));
    }

    #[test]
    fn test_round_trip_all_12_variants() {
        // Verifies that all 12 TypeDefinitionKind variants round-trip through
        // JSON encode/decode correctly (replaces the old "5 variants" FULL_JSON test).
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    { "name": "Draft", "kind": "typestate", "description": "typestate", "transitions_to": ["Published"] },
    { "name": "Published", "kind": "typestate", "description": "typestate terminal", "transitions_to": [] },
    { "name": "TrackStatus", "kind": "enum", "description": "enum", "expected_variants": ["Planned", "Done"] },
    { "name": "TrackId", "kind": "value_object", "description": "value object" },
    { "name": "AppError", "kind": "error_type", "description": "error type", "expected_variants": ["NotFound"] },
    {
      "name": "TrackRepo",
      "kind": "secondary_port",
      "description": "secondary port",
      "expected_methods": [
        { "name": "save", "receiver": "&self", "params": [], "returns": "()", "is_async": false }
      ]
    },
    {
      "name": "UseHandler",
      "kind": "application_service",
      "description": "application service",
      "expected_methods": [
        { "name": "execute", "receiver": "&self", "params": [], "returns": "()", "is_async": false }
      ]
    },
    { "name": "SaveUseCase", "kind": "use_case", "description": "use case" },
    { "name": "SaveInteractor", "kind": "interactor", "description": "interactor" },
    { "name": "SaveDto", "kind": "dto", "description": "dto" },
    { "name": "SaveCommand", "kind": "command", "description": "command" },
    { "name": "GetQuery", "kind": "query", "description": "query" },
    { "name": "AggFactory", "kind": "factory", "description": "factory" }
  ]
}"#;
        let doc = decode(json).unwrap();
        assert_eq!(doc.entries().len(), 13);
        let encoded = encode(&doc).unwrap();
        let doc2 = decode(&encoded).unwrap();
        assert_eq!(doc2.entries().len(), 13);
        for (a, b) in doc.entries().iter().zip(doc2.entries()) {
            assert_eq!(a.name(), b.name());
            assert_eq!(a.kind(), b.kind());
        }
    }

    // --- T002: existence-only variant stale-field rejection (Phase 1.5) ---

    #[test]
    fn test_decode_use_case_with_stale_expected_methods_rejected() {
        // Phase 1.5: existence-only variants must not carry kind-specific fields
        // that belong to other variants (e.g. expected_methods from SecondaryPort).
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    {
      "name": "SaveUseCase",
      "kind": "use_case",
      "description": "use case with stale field",
      "expected_methods": [
        { "name": "execute", "receiver": "&self", "params": [], "returns": "()", "is_async": false }
      ]
    }
  ]
}"#;
        let err = decode(json).unwrap_err();
        assert!(
            matches!(err, TypeCatalogueCodecError::InvalidEntry { .. }),
            "expected InvalidEntry for stale expected_methods on use_case, got: {:?}",
            err
        );
    }

    #[test]
    fn test_decode_dto_with_stale_expected_variants_rejected() {
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    {
      "name": "CreateUserDto",
      "kind": "dto",
      "description": "dto with stale field",
      "expected_variants": ["A", "B"]
    }
  ]
}"#;
        let err = decode(json).unwrap_err();
        assert!(
            matches!(err, TypeCatalogueCodecError::InvalidEntry { .. }),
            "expected InvalidEntry for stale expected_variants on dto, got: {:?}",
            err
        );
    }

    #[test]
    fn test_decode_value_object_with_stale_transitions_to_rejected() {
        // value_object is also existence-only (pre-existing); same protection applies.
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    {
      "name": "TrackId",
      "kind": "value_object",
      "description": "value object with stale field",
      "transitions_to": ["Published"]
    }
  ]
}"#;
        let err = decode(json).unwrap_err();
        assert!(
            matches!(err, TypeCatalogueCodecError::InvalidEntry { .. }),
            "expected InvalidEntry for stale transitions_to on value_object, got: {:?}",
            err
        );
    }
}
