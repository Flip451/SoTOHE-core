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

use domain::tddd::catalogue::{MethodDeclaration, ParamDeclaration, TraitImplDecl};
use domain::{
    ConfidenceSignal, SpecValidationError, TypeAction, TypeCatalogueDocument, TypeCatalogueEntry,
    TypeDefinitionKind, TypestateTransitions,
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
    /// Backward-compat decode slot for legacy files that still carry inline
    /// `signals`. T007 (ADR 2026-04-18-1400 §D1 / §D6) moved signals out of
    /// the declaration file into `<layer>-type-signals.json`. The DTO
    /// accepts (and discards) any inline signals blob so that decoding a
    /// legacy catalogue file does not fail `deny_unknown_fields`, but the
    /// encoder always emits `None` so freshly-written declaration files
    /// never carry signals again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signals: Option<serde_json::Value>,
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
    /// Struct implementing one or more hexagonal secondary port traits.
    SecondaryAdapter {
        #[serde(default)]
        implements: Vec<TraitImplDeclDto>,
    },
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

/// DTO for a single trait implementation declaration within a `SecondaryAdapter`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TraitImplDeclDto {
    trait_name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    expected_methods: Vec<MethodDto>,
}

// NOTE: T007 removed the `TypeSignalDto` struct — signal payloads live in
// `<layer>-type-signals.json` (see `type_signals_codec`), not in the
// declaration file.

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

    // Phase 1.5: reject entries that carry stale cross-kind fields.
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
            // Existence-only kinds must not carry any structural fields.
            const EXISTENCE_ONLY_KINDS: &[&str] =
                &["use_case", "interactor", "dto", "command", "query", "factory", "value_object"];
            const FORBIDDEN_FIELDS_EXISTENCE_ONLY: &[&str] =
                &["expected_methods", "expected_variants", "transitions_to", "implements"];
            if EXISTENCE_ONLY_KINDS.contains(&kind) {
                if let Some(obj) = entry_obj {
                    for forbidden in FORBIDDEN_FIELDS_EXISTENCE_ONLY {
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
            // `secondary_adapter` carries `implements` but must not carry fields
            // belonging to other structured variants (`expected_methods`,
            // `expected_variants`, `transitions_to`).  Without this guard those
            // fields would be silently dropped by serde.
            if kind == "secondary_adapter" {
                const FORBIDDEN_FIELDS_SECONDARY_ADAPTER: &[&str] =
                    &["expected_methods", "expected_variants", "transitions_to"];
                if let Some(obj) = entry_obj {
                    for forbidden in FORBIDDEN_FIELDS_SECONDARY_ADAPTER {
                        if obj.contains_key(*forbidden) {
                            return Err(TypeCatalogueCodecError::InvalidEntry {
                                name: name.to_owned(),
                                reason: format!(
                                    "kind 'secondary_adapter' does not support field '{}' — \
                                     use 'implements' for per-trait method declarations",
                                    forbidden
                                ),
                            });
                        }
                    }
                }
            }
            // Structured non-existence-only kinds (`typestate`, `enum`, `error_type`,
            // `secondary_port`, `application_service`) do not carry `implements`.
            // Without this guard, a payload like `{"kind":"enum","implements":[...]}`
            // would be silently accepted with the `implements` field dropped by serde.
            const STRUCTURED_KINDS: &[&str] =
                &["typestate", "enum", "error_type", "secondary_port", "application_service"];
            if STRUCTURED_KINDS.contains(&kind) {
                if let Some(obj) = entry_obj {
                    if obj.contains_key("implements") {
                        return Err(TypeCatalogueCodecError::InvalidEntry {
                            name: name.to_owned(),
                            reason: format!(
                                "kind '{}' does not support field 'implements' — \
                                 'implements' is only valid for 'secondary_adapter'",
                                kind
                            ),
                        });
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

    let doc = TypeCatalogueDocument::new(dto.schema_version, entries);

    // T007 (ADR 2026-04-18-1400 §D1 / §D6): the declaration file no longer
    // carries signals. The DTO still accepts the field for backward-compatible
    // decode of legacy files written before the split, but the values are
    // discarded — the authoritative signals live in `<layer>-type-signals.json`
    // (read by `evaluate_layer_catalogue`, T005). This is the read-side half of
    // the declaration/evaluation split; the write-side half is `encode` below.
    let _legacy_inline_signals = dto.signals;

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
        TypeDefinitionKindDto::SecondaryAdapter { implements } => {
            let decls = decode_trait_impl_list(entry_name, implements)?;
            Ok(TypeDefinitionKind::SecondaryAdapter { implements: decls })
        }
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

/// Decode a `Vec<TraitImplDeclDto>` into `Vec<TraitImplDecl>`.
fn decode_trait_impl_list(
    entry_name: &str,
    dtos: &[TraitImplDeclDto],
) -> Result<Vec<TraitImplDecl>, TypeCatalogueCodecError> {
    let mut decls = Vec::with_capacity(dtos.len());
    for dto in dtos {
        // L1 enforcement: trait_name must not contain `::` (last-segment only).
        if dto.trait_name.contains("::") {
            return Err(TypeCatalogueCodecError::InvalidEntry {
                name: entry_name.to_owned(),
                reason: format!(
                    "implements trait_name contains '::' — L1 catalogue entries must use \
                     last-segment short names: '{}'",
                    dto.trait_name
                ),
            });
        }
        let methods = decode_method_list(entry_name, &dto.expected_methods)?;
        decls.push(TraitImplDecl::new(dto.trait_name.clone(), methods));
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

#[allow(dead_code)] // retained behind `#[allow]` for potential legacy-decode hook restoration.
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
    // T007 (ADR 2026-04-18-1400 §D1 / §D6): the declaration file is authored
    // content; evaluation results live in `<layer>-type-signals.json`. Emit
    // `signals: None` so the DTO's `skip_serializing_if = "Option::is_none"`
    // elides the field entirely from the on-disk JSON. `doc.signals()` is
    // intentionally ignored here — the in-memory aggregate still holds the
    // evaluated signals for the CLI writer / CI reader to pass around, but
    // the declaration file never carries them again.
    let signals = None;
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
        TypeDefinitionKind::SecondaryAdapter { implements } => {
            let dtos = encode_trait_impl_list(entry.name(), implements)?;
            TypeDefinitionKindDto::SecondaryAdapter { implements: dtos }
        }
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

/// Encode a `Vec<TraitImplDecl>` into `Vec<TraitImplDeclDto>`.
fn encode_trait_impl_list(
    entry_name: &str,
    decls: &[TraitImplDecl],
) -> Result<Vec<TraitImplDeclDto>, TypeCatalogueCodecError> {
    let mut dtos = Vec::with_capacity(decls.len());
    for decl in decls {
        // L1 enforcement at encode time: mirror the same check as decode_trait_impl_list.
        if decl.trait_name().contains("::") {
            return Err(TypeCatalogueCodecError::InvalidEntry {
                name: entry_name.to_owned(),
                reason: format!(
                    "implements trait_name contains '::' — L1 catalogue entries must use \
                     last-segment short names: '{}'",
                    decl.trait_name()
                ),
            });
        }
        let methods = encode_method_list(entry_name, decl.expected_methods())?;
        dtos.push(TraitImplDeclDto {
            trait_name: decl.trait_name().to_owned(),
            expected_methods: methods,
        });
    }
    Ok(dtos)
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

// NOTE: T007 removed `type_signal_to_dto`, `confidence_signal_to_str`, and
// `type_signal_from_dto` helpers — signals now live in
// `<layer>-type-signals.json` via `type_signals_codec`, not in the
// declaration file.

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
    fn test_decode_silently_drops_legacy_inline_signals() {
        // T007 (ADR 2026-04-18-1400 §D1 / §D6): the declaration file no longer
        // carries signals. Legacy declaration files that still have an inline
        // `signals` blob must be decodable (backward compat), but the value
        // is discarded — the authoritative signals live in
        // `<layer>-type-signals.json`.
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
        assert!(
            doc.signals().is_none(),
            "legacy inline signals must be dropped after T007 codec strip"
        );

        let encoded = encode(&doc).unwrap();
        assert!(
            !encoded.contains("\"signals\""),
            "encode must not emit a `signals` field, got:\n{encoded}"
        );

        // Decode→encode→decode round-trip is stable (both times: signals=None).
        let doc2 = decode(&encoded).unwrap();
        assert!(doc2.signals().is_none());
        assert_eq!(doc.entries().len(), doc2.entries().len());
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
    fn test_encode_never_emits_signals_even_with_inline_red_input() {
        // T007: even when the legacy declaration file has rich inline signals
        // (Red + missing_items), the decoder drops them and the encoder emits
        // a clean declaration with no `signals` field. This closes the
        // Migration §5b write-side contract: after T007 lands, every written
        // declaration file is authored-only.
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
        assert!(doc.signals().is_none(), "legacy inline signals must be dropped");

        let encoded = encode(&doc).unwrap();
        assert!(
            !encoded.contains("\"signals\""),
            "encoded output must not contain a `signals` field, got:\n{encoded}"
        );
        assert!(
            !encoded.contains("\"missing_items\""),
            "encoded output must not contain `missing_items` (signal-only key), got:\n{encoded}"
        );
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
    fn test_round_trip_all_13_variants() {
        // Verifies that all 13 TypeDefinitionKind variants round-trip through
        // JSON encode/decode correctly.
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
    { "name": "AggFactory", "kind": "factory", "description": "factory" },
    {
      "name": "FsStore",
      "kind": "secondary_adapter",
      "description": "adapter",
      "implements": [{ "trait_name": "TrackReader" }]
    }
  ]
}"#;
        let doc = decode(json).unwrap();
        assert_eq!(doc.entries().len(), 14);
        let encoded = encode(&doc).unwrap();
        let doc2 = decode(&encoded).unwrap();
        assert_eq!(doc2.entries().len(), 14);
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

    // --- TDDD-05 T002: SecondaryAdapter decode/encode tests ---

    #[test]
    fn test_decode_secondary_adapter_single_trait_no_methods() {
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    {
      "name": "FsReviewStore",
      "kind": "secondary_adapter",
      "description": "Adapter implementing ReviewReader",
      "implements": [
        { "trait_name": "ReviewReader" }
      ]
    }
  ]
}"#;
        let doc = decode(json).unwrap();
        let kind = doc.entries()[0].kind();
        let TypeDefinitionKind::SecondaryAdapter { implements } = kind else {
            panic!("expected SecondaryAdapter kind, got {:?}", kind);
        };
        assert_eq!(implements.len(), 1);
        assert_eq!(implements[0].trait_name(), "ReviewReader");
        assert!(implements[0].expected_methods().is_empty());
    }

    #[test]
    fn test_decode_secondary_adapter_two_traits_with_methods() {
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    {
      "name": "SystemGitRepo",
      "kind": "secondary_adapter",
      "description": "Adapter implementing WorktreeReader and TrackWriter",
      "implements": [
        {
          "trait_name": "WorktreeReader",
          "expected_methods": [
            { "name": "read_worktree", "receiver": "&self", "params": [], "returns": "Result<Worktree, DomainError>", "is_async": false }
          ]
        },
        { "trait_name": "TrackWriter" }
      ]
    }
  ]
}"#;
        let doc = decode(json).unwrap();
        let kind = doc.entries()[0].kind();
        let TypeDefinitionKind::SecondaryAdapter { implements } = kind else {
            panic!("expected SecondaryAdapter kind, got {:?}", kind);
        };
        assert_eq!(implements.len(), 2);
        assert_eq!(implements[0].trait_name(), "WorktreeReader");
        assert_eq!(implements[0].expected_methods().len(), 1);
        assert_eq!(implements[0].expected_methods()[0].name(), "read_worktree");
        assert_eq!(implements[1].trait_name(), "TrackWriter");
        assert!(implements[1].expected_methods().is_empty());
    }

    #[test]
    fn test_encode_secondary_adapter_round_trip() {
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    {
      "name": "FsReviewStore",
      "kind": "secondary_adapter",
      "description": "Adapter implementing ReviewReader",
      "implements": [
        {
          "trait_name": "ReviewReader",
          "expected_methods": [
            { "name": "find", "receiver": "&self", "params": [{ "name": "id", "ty": "ReviewId" }], "returns": "Option<Review>", "is_async": false }
          ]
        }
      ]
    }
  ]
}"#;
        let doc = decode(json).unwrap();
        let encoded = encode(&doc).unwrap();
        let doc2 = decode(&encoded).unwrap();
        assert_eq!(doc2.entries()[0].kind(), doc.entries()[0].kind());
        assert!(encoded.contains("\"secondary_adapter\""));
        assert!(encoded.contains("\"implements\""));
    }

    #[test]
    fn test_existence_only_kinds_excludes_secondary_adapter() {
        // secondary_adapter carries `implements` field — it is NOT existence-only.
        // Phase 1.5 must NOT reject secondary_adapter entries that carry `implements`.
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    {
      "name": "FsReviewStore",
      "kind": "secondary_adapter",
      "description": "Adapter",
      "implements": [{ "trait_name": "ReviewReader" }]
    }
  ]
}"#;
        assert!(
            decode(json).is_ok(),
            "secondary_adapter with implements must not be rejected by Phase 1.5"
        );
    }

    #[test]
    fn test_is_method_bearing_excludes_secondary_adapter() {
        // secondary_adapter is in the type (non-trait) partition.
        // A delete+add pair where both are non-trait (value_object + secondary_adapter)
        // must be rejected as same-partition.
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    { "name": "FsStore", "kind": "value_object", "description": "old", "action": "delete" },
    {
      "name": "FsStore",
      "kind": "secondary_adapter",
      "description": "new",
      "action": "add",
      "implements": [{ "trait_name": "ReviewReader" }]
    }
  ]
}"#;
        let err = decode(json).unwrap_err();
        match err {
            TypeCatalogueCodecError::InvalidEntry { reason, .. } => {
                assert!(
                    reason.contains("same partition"),
                    "expected same-partition error, got: {reason}"
                );
            }
            other => panic!("expected InvalidEntry, got {other:?}"),
        }
    }

    #[test]
    fn test_decode_existence_only_kind_with_implements_rejected() {
        // Phase 1.5: existence-only kinds (use_case, etc.) must not carry `implements`
        // (which belongs to secondary_adapter). Without this guard, the field would be
        // silently dropped by serde instead of triggering an error.
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    {
      "name": "SaveUseCase",
      "kind": "use_case",
      "description": "use case with stale implements field",
      "implements": [{ "trait_name": "SomePort" }]
    }
  ]
}"#;
        let err = decode(json).unwrap_err();
        assert!(
            matches!(err, TypeCatalogueCodecError::InvalidEntry { .. }),
            "expected InvalidEntry for stale implements on use_case, got: {:?}",
            err
        );
    }

    #[test]
    fn test_decode_secondary_adapter_trait_name_with_double_colon_rejected() {
        // L1 enforcement: trait_name in implements must not contain `::`.
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    {
      "name": "FsReviewStore",
      "kind": "secondary_adapter",
      "description": "Adapter",
      "implements": [{ "trait_name": "domain::ports::ReviewReader" }]
    }
  ]
}"#;
        let err = decode(json).unwrap_err();
        match err {
            TypeCatalogueCodecError::InvalidEntry { reason, .. } => {
                assert!(
                    reason.contains("'::'"),
                    "expected '::' rejection in trait_name, got: {reason}"
                );
            }
            other => panic!("expected InvalidEntry, got {other:?}"),
        }
    }

    #[test]
    fn test_decode_secondary_adapter_with_stale_expected_methods_rejected() {
        // Phase 1.5: secondary_adapter must not carry `expected_methods` (that field
        // belongs to secondary_port / application_service). Without this guard, serde
        // would silently drop the field and the data loss would be invisible.
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    {
      "name": "FsReviewStore",
      "kind": "secondary_adapter",
      "description": "Adapter with stale expected_methods",
      "implements": [{ "trait_name": "ReviewReader" }],
      "expected_methods": [
        { "name": "find", "receiver": "&self", "params": [], "returns": "()", "is_async": false }
      ]
    }
  ]
}"#;
        let err = decode(json).unwrap_err();
        assert!(
            matches!(err, TypeCatalogueCodecError::InvalidEntry { .. }),
            "expected InvalidEntry for stale expected_methods on secondary_adapter, got: {:?}",
            err
        );
    }

    #[test]
    fn test_decode_structured_kind_with_stale_implements_rejected() {
        // Phase 1.5: structured non-existence-only kinds (`typestate`, `enum`,
        // `error_type`, `secondary_port`, `application_service`) must not carry
        // `implements` (which belongs only to `secondary_adapter`).  Without this
        // guard, serde would silently accept the field and drop the adapter
        // declarations instead of failing at the codec boundary.
        let json = r#"{
  "schema_version": 2,
  "type_definitions": [
    {
      "name": "TrackStatus",
      "kind": "enum",
      "description": "enum with stale implements field",
      "expected_variants": ["Planned", "Done"],
      "implements": [{ "trait_name": "SomePort" }]
    }
  ]
}"#;
        let err = decode(json).unwrap_err();
        assert!(
            matches!(err, TypeCatalogueCodecError::InvalidEntry { .. }),
            "expected InvalidEntry for stale implements on enum, got: {:?}",
            err
        );
    }
}
