//! Serde codec for spec.json (SpecDocument SSoT).
//!
//! Mirrors the pattern of `crate::track::codec` but for the spec document schema.

use domain::{
    ConfidenceSignal, DomainStateEntry, DomainStateSignal, SignalCounts, SpecDocument,
    SpecRequirement, SpecScope, SpecSection, SpecStatus, SpecValidationError, TaskId, Timestamp,
};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Codec error for spec.json serialization/deserialization.
#[derive(Debug, thiserror::Error)]
pub enum SpecCodecError {
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("validation error: {0}")]
    Validation(#[from] SpecValidationError),

    #[error("unsupported schema_version: expected 1, got {0}")]
    UnsupportedSchemaVersion(u32),

    #[error("domain state '{state}' has transition to unknown state '{target}'")]
    InvalidTransitionTarget { state: String, target: String },

    #[error("unknown signal string '{0}': expected 'blue', 'yellow', or 'red'")]
    InvalidSignalString(String),

    #[error("invalid field '{field}': {reason}")]
    InvalidField { field: String, reason: String },
}

// ---------------------------------------------------------------------------
// DTO types
// ---------------------------------------------------------------------------

/// Top-level DTO for spec.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SpecDocumentDto {
    pub schema_version: u32,
    pub status: String,
    pub version: String,
    pub title: String,
    #[serde(default)]
    pub goal: Vec<String>,
    pub scope: SpecScopeDto,
    #[serde(default)]
    pub constraints: Vec<SpecRequirementDto>,
    #[serde(default)]
    pub domain_states: Vec<DomainStateEntryDto>,
    #[serde(default)]
    pub acceptance_criteria: Vec<SpecRequirementDto>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_sections: Vec<SpecSectionDto>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_conventions: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signals: Option<SignalCountsDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain_state_signals: Option<Vec<DomainStateSignalDto>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub approved_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
}

/// DTO for a single requirement (text + provenance sources + task references).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SpecRequirementDto {
    pub text: String,
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub task_refs: Vec<String>,
}

/// DTO for a domain state entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DomainStateEntryDto {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transitions_to: Option<Vec<String>>,
}

/// DTO for the scope section.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SpecScopeDto {
    #[serde(default)]
    pub in_scope: Vec<SpecRequirementDto>,
    #[serde(default)]
    pub out_of_scope: Vec<SpecRequirementDto>,
}

/// DTO for a free-form additional section.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SpecSectionDto {
    pub title: String,
    #[serde(default)]
    pub content: Vec<String>,
}

/// DTO for aggregate signal counts.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SignalCountsDto {
    pub blue: u32,
    pub yellow: u32,
    pub red: u32,
}

/// DTO for a per-state domain signal evaluation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DomainStateSignalDto {
    pub state_name: String,
    /// Signal level string: "blue", "yellow", or "red".
    pub signal: String,
    pub found_type: bool,
    #[serde(default)]
    pub found_transitions: Vec<String>,
    #[serde(default)]
    pub missing_transitions: Vec<String>,
}

// ---------------------------------------------------------------------------
// Decode: JSON -> domain
// ---------------------------------------------------------------------------

/// Deserializes a spec.json string into a `SpecDocument`.
///
/// # Errors
///
/// Returns `SpecCodecError::Json` if the JSON is malformed.
/// Returns `SpecCodecError::UnsupportedSchemaVersion` if `schema_version != 1`.
/// Returns `SpecCodecError::Validation` if any domain type construction fails.
pub fn decode(json: &str) -> Result<SpecDocument, SpecCodecError> {
    let dto: SpecDocumentDto = serde_json::from_str(json)?;

    if dto.schema_version != 1 {
        return Err(SpecCodecError::UnsupportedSchemaVersion(dto.schema_version));
    }

    let in_scope =
        dto.scope.in_scope.into_iter().map(requirement_from_dto).collect::<Result<Vec<_>, _>>()?;

    let out_of_scope = dto
        .scope
        .out_of_scope
        .into_iter()
        .map(requirement_from_dto)
        .collect::<Result<Vec<_>, _>>()?;

    let scope = SpecScope::new(in_scope, out_of_scope);

    let constraints =
        dto.constraints.into_iter().map(requirement_from_dto).collect::<Result<Vec<_>, _>>()?;

    let domain_states =
        dto.domain_states.into_iter().map(domain_state_from_dto).collect::<Result<Vec<_>, _>>()?;

    // Reference integrity: every transitions_to target must name an existing domain state.
    {
        use std::collections::HashSet;
        let state_names: HashSet<&str> = domain_states.iter().map(|s| s.name()).collect();
        for state in &domain_states {
            if let Some(targets) = state.transitions_to() {
                for target in targets {
                    if !state_names.contains(target.as_str()) {
                        return Err(SpecCodecError::InvalidTransitionTarget {
                            state: state.name().to_owned(),
                            target: target.clone(),
                        });
                    }
                }
            }
        }
    }

    let acceptance_criteria = dto
        .acceptance_criteria
        .into_iter()
        .map(requirement_from_dto)
        .collect::<Result<Vec<_>, _>>()?;

    let additional_sections =
        dto.additional_sections.into_iter().map(section_from_dto).collect::<Result<Vec<_>, _>>()?;

    let signals = dto.signals.map(signal_counts_from_dto);

    let domain_state_signals = dto
        .domain_state_signals
        .map(|dtos| {
            dtos.into_iter().map(domain_state_signal_from_dto).collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;

    let status = status_from_str(&dto.status)?;

    let approved_at = dto
        .approved_at
        .map(|s| {
            Timestamp::new(s).map_err(|e| SpecCodecError::InvalidField {
                field: "approved_at".into(),
                reason: e.to_string(),
            })
        })
        .transpose()?;

    let mut doc = SpecDocument::new(
        dto.title,
        status,
        dto.version,
        dto.goal,
        scope,
        constraints,
        domain_states,
        acceptance_criteria,
        additional_sections,
        dto.related_conventions,
        signals,
        domain_state_signals,
        approved_at,
        dto.content_hash,
    )?;

    // Auto-demote: if status is Approved but content hash doesn't match, revert to Draft.
    if doc.status() == SpecStatus::Approved {
        let current_hash = compute_content_hash(&doc)?;
        if !doc.is_approval_valid(&current_hash) {
            doc.demote();
        }
    }

    Ok(doc)
}

/// Parses a status string into `SpecStatus`.
fn status_from_str(s: &str) -> Result<SpecStatus, SpecCodecError> {
    match s {
        "draft" => Ok(SpecStatus::Draft),
        "approved" => Ok(SpecStatus::Approved),
        other => Err(SpecCodecError::InvalidField {
            field: "status".into(),
            reason: format!("unknown status '{other}': expected 'draft' or 'approved'"),
        }),
    }
}

fn requirement_from_dto(dto: SpecRequirementDto) -> Result<SpecRequirement, SpecCodecError> {
    let task_refs: Vec<TaskId> = dto
        .task_refs
        .into_iter()
        .map(|s| {
            TaskId::try_new(s).map_err(|e| SpecCodecError::InvalidField {
                field: "task_refs".to_owned(),
                reason: e.to_string(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SpecRequirement::with_task_refs(dto.text, dto.sources, task_refs)?)
}

fn domain_state_from_dto(
    dto: DomainStateEntryDto,
) -> Result<DomainStateEntry, SpecValidationError> {
    DomainStateEntry::new(dto.name, dto.description, dto.transitions_to)
}

fn section_from_dto(dto: SpecSectionDto) -> Result<SpecSection, SpecValidationError> {
    SpecSection::new(dto.title, dto.content)
}

fn signal_counts_from_dto(dto: SignalCountsDto) -> SignalCounts {
    SignalCounts::new(dto.blue, dto.yellow, dto.red)
}

fn confidence_signal_from_str(s: &str) -> Result<ConfidenceSignal, SpecCodecError> {
    match s {
        "blue" => Ok(ConfidenceSignal::Blue),
        "yellow" => Ok(ConfidenceSignal::Yellow),
        "red" => Ok(ConfidenceSignal::Red),
        other => Err(SpecCodecError::InvalidSignalString(other.to_owned())),
    }
}

fn confidence_signal_to_str(signal: ConfidenceSignal) -> &'static str {
    match signal {
        ConfidenceSignal::Blue => "blue",
        ConfidenceSignal::Yellow => "yellow",
        ConfidenceSignal::Red => "red",
        // ConfidenceSignal is #[non_exhaustive]; future variants fall back to "red" (safe side).
        _ => "red",
    }
}

fn domain_state_signal_from_dto(
    dto: DomainStateSignalDto,
) -> Result<DomainStateSignal, SpecCodecError> {
    let signal = confidence_signal_from_str(&dto.signal)?;
    Ok(DomainStateSignal::new(
        dto.state_name,
        signal,
        dto.found_type,
        dto.found_transitions,
        dto.missing_transitions,
    ))
}

fn domain_state_signal_to_dto(sig: &DomainStateSignal) -> DomainStateSignalDto {
    DomainStateSignalDto {
        state_name: sig.state_name().to_owned(),
        signal: confidence_signal_to_str(sig.signal()).to_owned(),
        found_type: sig.found_type(),
        found_transitions: sig.found_transitions().to_vec(),
        missing_transitions: sig.missing_transitions().to_vec(),
    }
}

// ---------------------------------------------------------------------------
// Content hash computation
// ---------------------------------------------------------------------------

/// Computes a SHA-256 content hash of the substantive fields of a spec document.
///
/// Hashed fields: title, version, goal, scope (in + out), constraints,
/// domain_states, acceptance_criteria.
/// Excluded: status, signals, domain_state_signals, additional_sections,
/// related_conventions, approved_at, content_hash, task_refs (bookkeeping metadata).
///
/// # Errors
///
/// Returns `SpecCodecError::Json` if the DTO cannot be serialized (should not happen
/// in practice since all fields are primitive/String types).
pub fn compute_content_hash(doc: &SpecDocument) -> Result<String, SpecCodecError> {
    use sha2::{Digest, Sha256};

    let hashable = ContentHashDto {
        title: doc.title().to_owned(),
        version: doc.version().to_owned(),
        goal: doc.goal().to_vec(),
        scope: HashScopeDto {
            in_scope: doc.scope().in_scope().iter().map(requirement_to_hash_dto).collect(),
            out_of_scope: doc.scope().out_of_scope().iter().map(requirement_to_hash_dto).collect(),
        },
        constraints: doc.constraints().iter().map(requirement_to_hash_dto).collect(),
        domain_states: doc.domain_states().iter().map(domain_state_to_dto).collect(),
        acceptance_criteria: doc
            .acceptance_criteria()
            .iter()
            .map(requirement_to_hash_dto)
            .collect(),
    };

    // Deterministic JSON: serde_json serializes struct fields in declaration order.
    let json = serde_json::to_string(&hashable)?;
    let hash = Sha256::digest(json.as_bytes());
    Ok(format!("sha256:{hash:x}"))
}

/// DTO for the subset of fields included in the content hash.
/// Uses `HashRequirementDto` (text + sources only, no task_refs) to avoid
/// bookkeeping changes from invalidating the approval hash.
#[derive(Serialize)]
struct ContentHashDto {
    title: String,
    version: String,
    goal: Vec<String>,
    scope: HashScopeDto,
    constraints: Vec<HashRequirementDto>,
    domain_states: Vec<DomainStateEntryDto>,
    acceptance_criteria: Vec<HashRequirementDto>,
}

/// Scope DTO for content hash (uses HashRequirementDto).
#[derive(Serialize)]
struct HashScopeDto {
    in_scope: Vec<HashRequirementDto>,
    out_of_scope: Vec<HashRequirementDto>,
}

/// Requirement DTO for content hash — text + sources only, excludes task_refs.
#[derive(Serialize)]
struct HashRequirementDto {
    text: String,
    sources: Vec<String>,
}

fn requirement_to_hash_dto(req: &SpecRequirement) -> HashRequirementDto {
    HashRequirementDto { text: req.text().to_owned(), sources: req.sources().to_vec() }
}

// ---------------------------------------------------------------------------
// Encode: domain -> JSON
// ---------------------------------------------------------------------------

/// Serializes a `SpecDocument` to a pretty-printed spec.json string.
///
/// # Errors
///
/// Returns `SpecCodecError::Json` if serialization fails.
pub fn encode(doc: &SpecDocument) -> Result<String, SpecCodecError> {
    let dto = spec_document_to_dto(doc);
    Ok(serde_json::to_string_pretty(&dto)?)
}

fn spec_document_to_dto(doc: &SpecDocument) -> SpecDocumentDto {
    SpecDocumentDto {
        schema_version: 1,
        status: doc.status().as_str().to_owned(),
        version: doc.version().to_owned(),
        title: doc.title().to_owned(),
        goal: doc.goal().to_vec(),
        scope: SpecScopeDto {
            in_scope: doc.scope().in_scope().iter().map(requirement_to_dto).collect(),
            out_of_scope: doc.scope().out_of_scope().iter().map(requirement_to_dto).collect(),
        },
        constraints: doc.constraints().iter().map(requirement_to_dto).collect(),
        domain_states: doc.domain_states().iter().map(domain_state_to_dto).collect(),
        acceptance_criteria: doc.acceptance_criteria().iter().map(requirement_to_dto).collect(),
        additional_sections: doc.additional_sections().iter().map(section_to_dto).collect(),
        related_conventions: doc.related_conventions().to_vec(),
        signals: doc.signals().map(signal_counts_to_dto),
        domain_state_signals: doc
            .domain_state_signals()
            .map(|sigs| sigs.iter().map(domain_state_signal_to_dto).collect()),
        approved_at: doc.approved_at().map(|ts| ts.as_str().to_owned()),
        content_hash: doc.content_hash().map(|s| s.to_owned()),
    }
}

fn requirement_to_dto(req: &SpecRequirement) -> SpecRequirementDto {
    SpecRequirementDto {
        text: req.text().to_owned(),
        sources: req.sources().to_vec(),
        task_refs: req.task_refs().iter().map(|id| id.to_string()).collect(),
    }
}

fn domain_state_to_dto(entry: &DomainStateEntry) -> DomainStateEntryDto {
    DomainStateEntryDto {
        name: entry.name().to_owned(),
        description: entry.description().to_owned(),
        transitions_to: entry.transitions_to().map(|v| v.to_vec()),
    }
}

fn section_to_dto(section: &SpecSection) -> SpecSectionDto {
    SpecSectionDto { title: section.title().to_owned(), content: section.content().to_vec() }
}

fn signal_counts_to_dto(counts: &SignalCounts) -> SignalCountsDto {
    SignalCountsDto { blue: counts.blue(), yellow: counts.yellow(), red: counts.red() }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    // --- Fixtures ---

    const MINIMAL_JSON: &str = r#"{
  "schema_version": 1,
  "status": "draft",
  "version": "1.0",
  "title": "Feature Title",
  "scope": {
    "in_scope": [],
    "out_of_scope": []
  }
}"#;

    const FULL_JSON: &str = r#"{
  "schema_version": 1,
  "status": "draft",
  "version": "1.0",
  "title": "Feature Title",
  "goal": ["Line 1"],
  "scope": {
    "in_scope": [{ "text": "Req 1", "sources": ["PRD §3.2"] }],
    "out_of_scope": [{ "text": "Excluded 1", "sources": ["inference — not needed"] }]
  },
  "constraints": [{ "text": "Constraint 1", "sources": ["convention — hex.md"] }],
  "domain_states": [{ "name": "Draft", "description": "Initial state" }],
  "acceptance_criteria": [{ "text": "AC 1", "sources": ["PRD §4.1"] }],
  "additional_sections": [{ "title": "Custom Section", "content": ["Line 1"] }],
  "related_conventions": ["project-docs/conventions/source-attribution.md"],
  "signals": { "blue": 15, "yellow": 0, "red": 0 }
}"#;

    // --- decode: happy path ---

    #[test]
    fn test_decode_minimal_json_succeeds() {
        let doc = decode(MINIMAL_JSON).unwrap();
        assert_eq!(doc.title(), "Feature Title");
        assert_eq!(doc.status(), domain::SpecStatus::Draft);
        assert_eq!(doc.version(), "1.0");
        assert!(doc.goal().is_empty());
        assert!(doc.scope().in_scope().is_empty());
        assert!(doc.scope().out_of_scope().is_empty());
        assert!(doc.constraints().is_empty());
        assert!(doc.domain_states().is_empty());
        assert!(doc.acceptance_criteria().is_empty());
        assert!(doc.additional_sections().is_empty());
        assert!(doc.related_conventions().is_empty());
        assert!(doc.signals().is_none());
    }

    #[test]
    fn test_decode_full_json_succeeds() {
        let doc = decode(FULL_JSON).unwrap();
        assert_eq!(doc.title(), "Feature Title");
        assert_eq!(doc.goal(), &["Line 1"]);
        assert_eq!(doc.scope().in_scope().len(), 1);
        assert_eq!(doc.scope().in_scope()[0].text(), "Req 1");
        assert_eq!(doc.scope().in_scope()[0].sources(), &["PRD §3.2"]);
        assert_eq!(doc.scope().out_of_scope().len(), 1);
        assert_eq!(doc.constraints().len(), 1);
        assert_eq!(doc.constraints()[0].text(), "Constraint 1");
        assert_eq!(doc.domain_states().len(), 1);
        assert_eq!(doc.domain_states()[0].name(), "Draft");
        assert_eq!(doc.domain_states()[0].description(), "Initial state");
        assert_eq!(doc.acceptance_criteria().len(), 1);
        assert_eq!(doc.additional_sections().len(), 1);
        assert_eq!(doc.additional_sections()[0].title(), "Custom Section");
        assert_eq!(doc.additional_sections()[0].content(), &["Line 1"]);
        assert_eq!(doc.related_conventions(), &["project-docs/conventions/source-attribution.md"]);
        let signals = doc.signals().unwrap();
        assert_eq!(signals.blue(), 15);
        assert_eq!(signals.yellow(), 0);
        assert_eq!(signals.red(), 0);
    }

    // --- decode: optional fields default correctly ---

    #[test]
    fn test_decode_with_absent_goal_defaults_to_empty() {
        let json = r#"{"schema_version":1,"status":"draft","version":"1","title":"T","scope":{"in_scope":[],"out_of_scope":[]}}"#;
        let doc = decode(json).unwrap();
        assert!(doc.goal().is_empty());
    }

    #[test]
    fn test_decode_with_null_signals_gives_none() {
        let json = r#"{"schema_version":1,"status":"draft","version":"1","title":"T","scope":{"in_scope":[],"out_of_scope":[]},"signals":null}"#;
        // null is not the same as absent — serde(default) + skip_serializing_if handles absent,
        // but explicit null must also be tolerated. Using Option<> on the DTO absorbs null as None.
        let doc = decode(json).unwrap();
        assert!(doc.signals().is_none());
    }

    #[test]
    fn test_decode_additional_sections_defaults_to_empty() {
        let json = r#"{"schema_version":1,"status":"draft","version":"1","title":"T","scope":{"in_scope":[],"out_of_scope":[]}}"#;
        let doc = decode(json).unwrap();
        assert!(doc.additional_sections().is_empty());
    }

    #[test]
    fn test_decode_related_conventions_defaults_to_empty() {
        let json = r#"{"schema_version":1,"status":"draft","version":"1","title":"T","scope":{"in_scope":[],"out_of_scope":[]}}"#;
        let doc = decode(json).unwrap();
        assert!(doc.related_conventions().is_empty());
    }

    #[test]
    fn test_decode_requirement_without_sources_defaults_to_empty() {
        let json = r#"{
          "schema_version": 1, "status": "draft", "version": "1", "title": "T",
          "scope": {
            "in_scope": [{"text": "needs req"}],
            "out_of_scope": []
          }
        }"#;
        let doc = decode(json).unwrap();
        assert_eq!(doc.scope().in_scope()[0].sources(), &[] as &[String]);
    }

    // --- decode: schema_version validation ---

    #[test]
    fn test_decode_with_unsupported_schema_version_returns_error() {
        let json = r#"{"schema_version":2,"status":"draft","version":"1","title":"T","scope":{"in_scope":[],"out_of_scope":[]}}"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, SpecCodecError::UnsupportedSchemaVersion(2)));
    }

    #[test]
    fn test_decode_with_schema_version_zero_returns_error() {
        let json = r#"{"schema_version":0,"status":"draft","version":"1","title":"T","scope":{"in_scope":[],"out_of_scope":[]}}"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, SpecCodecError::UnsupportedSchemaVersion(0)));
    }

    // --- decode: domain validation errors ---

    #[test]
    fn test_decode_with_empty_title_returns_validation_error() {
        let json = r#"{"schema_version":1,"status":"draft","version":"1","title":"","scope":{"in_scope":[],"out_of_scope":[]}}"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, SpecCodecError::Validation(SpecValidationError::EmptyTitle)));
    }

    #[test]
    fn test_decode_with_empty_status_returns_invalid_field_error() {
        let json = r#"{"schema_version":1,"status":"","version":"1","title":"T","scope":{"in_scope":[],"out_of_scope":[]}}"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, SpecCodecError::InvalidField { .. }));
    }

    #[test]
    fn test_decode_with_empty_version_returns_validation_error() {
        let json = r#"{"schema_version":1,"status":"draft","version":"","title":"T","scope":{"in_scope":[],"out_of_scope":[]}}"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, SpecCodecError::Validation(SpecValidationError::EmptyVersion)));
    }

    #[test]
    fn test_decode_with_empty_requirement_text_returns_validation_error() {
        let json = r#"{
          "schema_version": 1, "status": "draft", "version": "1", "title": "T",
          "scope": {"in_scope": [{"text": ""}], "out_of_scope": []}
        }"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(
            err,
            SpecCodecError::Validation(SpecValidationError::EmptyRequirementText)
        ));
    }

    #[test]
    fn test_decode_with_empty_domain_state_name_returns_validation_error() {
        let json = r#"{
          "schema_version": 1, "status": "draft", "version": "1", "title": "T",
          "scope": {"in_scope": [], "out_of_scope": []},
          "domain_states": [{"name": "", "description": "desc"}]
        }"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(
            err,
            SpecCodecError::Validation(SpecValidationError::EmptyDomainStateName)
        ));
    }

    #[test]
    fn test_decode_with_empty_section_title_returns_validation_error() {
        let json = r#"{
          "schema_version": 1, "status": "draft", "version": "1", "title": "T",
          "scope": {"in_scope": [], "out_of_scope": []},
          "additional_sections": [{"title": "", "content": []}]
        }"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, SpecCodecError::Validation(SpecValidationError::EmptySectionTitle)));
    }

    // --- decode: malformed JSON ---

    #[test]
    fn test_decode_invalid_json_returns_json_error() {
        let err = decode("{not json}").unwrap_err();
        assert!(matches!(err, SpecCodecError::Json(_)));
    }

    // --- encode: happy path ---

    #[test]
    fn test_encode_minimal_document_produces_valid_json() {
        let doc = SpecDocument::new(
            "T",
            domain::SpecStatus::Draft,
            "1.0",
            vec![],
            SpecScope::new(vec![], vec![]),
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let json = encode(&doc).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["schema_version"], 1);
        assert_eq!(parsed["title"], "T");
        assert_eq!(parsed["status"], "draft");
        assert_eq!(parsed["version"], "1.0");
    }

    #[test]
    fn test_encode_omits_signals_when_none() {
        let doc = SpecDocument::new(
            "T",
            domain::SpecStatus::Draft,
            "1",
            vec![],
            SpecScope::new(vec![], vec![]),
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let json = encode(&doc).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("signals").is_none());
    }

    #[test]
    fn test_encode_includes_signals_when_present() {
        let doc = SpecDocument::new(
            "T",
            domain::SpecStatus::Draft,
            "1",
            vec![],
            SpecScope::new(vec![], vec![]),
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            Some(SignalCounts::new(5, 2, 1)),
            None,
            None,
            None,
        )
        .unwrap();
        let json = encode(&doc).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["signals"]["blue"], 5);
        assert_eq!(parsed["signals"]["yellow"], 2);
        assert_eq!(parsed["signals"]["red"], 1);
    }

    #[test]
    fn test_encode_omits_empty_additional_sections() {
        let doc = SpecDocument::new(
            "T",
            domain::SpecStatus::Draft,
            "1",
            vec![],
            SpecScope::new(vec![], vec![]),
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let json = encode(&doc).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("additional_sections").is_none());
    }

    #[test]
    fn test_encode_omits_empty_related_conventions() {
        let doc = SpecDocument::new(
            "T",
            domain::SpecStatus::Draft,
            "1",
            vec![],
            SpecScope::new(vec![], vec![]),
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let json = encode(&doc).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("related_conventions").is_none());
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
    fn test_round_trip_preserves_signals() {
        let doc_orig = decode(FULL_JSON).unwrap();
        let json = encode(&doc_orig).unwrap();
        let doc_rt = decode(&json).unwrap();
        assert_eq!(doc_orig.signals(), doc_rt.signals());
    }

    #[test]
    fn test_round_trip_domain_states() {
        let json = r#"{
          "schema_version": 1, "status": "draft", "version": "2.0", "title": "States Test",
          "scope": {"in_scope": [], "out_of_scope": []},
          "domain_states": [
            {"name": "Draft", "description": "Initial"},
            {"name": "Published", "description": "Live"}
          ]
        }"#;
        let doc = decode(json).unwrap();
        let encoded = encode(&doc).unwrap();
        let doc2 = decode(&encoded).unwrap();
        assert_eq!(doc2.domain_states().len(), 2);
        assert_eq!(doc2.domain_states()[0].name(), "Draft");
        assert_eq!(doc2.domain_states()[1].name(), "Published");
    }

    #[test]
    fn test_round_trip_multiple_requirements_with_sources() {
        let json = r#"{
          "schema_version": 1, "status": "draft", "version": "1.0", "title": "Multi",
          "scope": {
            "in_scope": [
              {"text": "R1", "sources": ["PRD §1", "feedback — user"]},
              {"text": "R2", "sources": ["convention — style.md"]}
            ],
            "out_of_scope": [{"text": "X1", "sources": ["inference — low value"]}]
          },
          "acceptance_criteria": [{"text": "AC1", "sources": []}, {"text": "AC2", "sources": ["discussion"]}]
        }"#;
        let doc = decode(json).unwrap();
        let encoded = encode(&doc).unwrap();
        let doc2 = decode(&encoded).unwrap();
        assert_eq!(doc2.scope().in_scope().len(), 2);
        assert_eq!(doc2.scope().in_scope()[0].sources().len(), 2);
        assert_eq!(doc2.scope().out_of_scope().len(), 1);
        assert_eq!(doc2.acceptance_criteria().len(), 2);
    }

    #[test]
    fn test_round_trip_additional_sections() {
        let json = r#"{
          "schema_version": 1, "status": "draft", "version": "1", "title": "T",
          "scope": {"in_scope": [], "out_of_scope": []},
          "additional_sections": [
            {"title": "Sec A", "content": ["line 1", "line 2"]},
            {"title": "Sec B", "content": []}
          ]
        }"#;
        let doc = decode(json).unwrap();
        let encoded = encode(&doc).unwrap();
        let doc2 = decode(&encoded).unwrap();
        assert_eq!(doc2.additional_sections().len(), 2);
        assert_eq!(doc2.additional_sections()[0].title(), "Sec A");
        assert_eq!(doc2.additional_sections()[0].content(), &["line 1", "line 2"]);
        assert_eq!(doc2.additional_sections()[1].title(), "Sec B");
    }

    // --- encode output format ---

    #[test]
    fn test_encode_output_is_pretty_printed() {
        let doc = decode(MINIMAL_JSON).unwrap();
        let json = encode(&doc).unwrap();
        // Pretty-printed JSON contains newlines
        assert!(json.contains('\n'));
    }

    #[test]
    fn test_encode_schema_version_is_always_1() {
        let doc = decode(FULL_JSON).unwrap();
        let json = encode(&doc).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["schema_version"], 1);
    }

    // --- transitions_to round-trip ---

    #[test]
    fn test_round_trip_transitions_to_with_targets() {
        let json = r#"{
          "schema_version": 1, "status": "draft", "version": "1.0", "title": "Transitions Test",
          "scope": {"in_scope": [], "out_of_scope": []},
          "domain_states": [
            {"name": "Draft", "description": "Initial", "transitions_to": ["Published", "Archived"]},
            {"name": "Published", "description": "Live", "transitions_to": []},
            {"name": "Archived", "description": "Final", "transitions_to": []}
          ]
        }"#;
        let doc = decode(json).unwrap();
        let encoded = encode(&doc).unwrap();
        let doc2 = decode(&encoded).unwrap();
        let states = doc2.domain_states();
        assert_eq!(
            states[0].transitions_to(),
            Some(["Published".to_string(), "Archived".to_string()].as_slice())
        );
        assert_eq!(states[1].transitions_to(), Some([].as_slice()));
        assert_eq!(states[2].transitions_to(), Some([].as_slice()));
    }

    #[test]
    fn test_decode_transitions_to_empty_array_maps_to_some_empty_vec() {
        let json = r#"{
          "schema_version": 1, "status": "draft", "version": "1", "title": "T",
          "scope": {"in_scope": [], "out_of_scope": []},
          "domain_states": [{"name": "Final", "description": "Terminal", "transitions_to": []}]
        }"#;
        let doc = decode(json).unwrap();
        assert_eq!(doc.domain_states()[0].transitions_to(), Some([].as_slice()));
    }

    #[test]
    fn test_decode_transitions_to_absent_maps_to_none() {
        let json = r#"{
          "schema_version": 1, "status": "draft", "version": "1", "title": "T",
          "scope": {"in_scope": [], "out_of_scope": []},
          "domain_states": [{"name": "Draft", "description": "desc"}]
        }"#;
        let doc = decode(json).unwrap();
        assert_eq!(doc.domain_states()[0].transitions_to(), None);
    }

    #[test]
    fn test_decode_invalid_transition_target_returns_error() {
        // "Draft" references "NonExistent" which is not in domain_states list
        let json = r#"{
          "schema_version": 1, "status": "draft", "version": "1", "title": "T",
          "scope": {"in_scope": [], "out_of_scope": []},
          "domain_states": [
            {"name": "Draft", "description": "desc", "transitions_to": ["NonExistent"]}
          ]
        }"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, SpecCodecError::InvalidTransitionTarget { .. }));
    }

    #[test]
    fn test_decode_valid_self_referencing_transition_is_allowed() {
        // A state may reference another valid state name in transitions_to
        let json = r#"{
          "schema_version": 1, "status": "draft", "version": "1", "title": "T",
          "scope": {"in_scope": [], "out_of_scope": []},
          "domain_states": [
            {"name": "Pending", "description": "desc", "transitions_to": ["Active"]},
            {"name": "Active", "description": "desc", "transitions_to": []}
          ]
        }"#;
        let doc = decode(json).unwrap();
        assert_eq!(doc.domain_states().len(), 2);
    }

    // --- domain_state_signals round-trip ---

    #[test]
    fn test_round_trip_domain_state_signals() {
        use domain::{ConfidenceSignal, DomainStateSignal};
        let doc = SpecDocument::new(
            "Signals Test",
            domain::SpecStatus::Draft,
            "1.0",
            vec![],
            SpecScope::new(vec![], vec![]),
            vec![],
            vec![
                DomainStateEntry::new("Draft", "Initial", Some(vec!["Published".into()])).unwrap(),
                DomainStateEntry::new("Published", "Live", Some(vec![])).unwrap(),
            ],
            vec![],
            vec![],
            vec![],
            None,
            Some(vec![
                DomainStateSignal::new(
                    "Draft",
                    ConfidenceSignal::Yellow,
                    true,
                    vec![],
                    vec!["Published".into()],
                ),
                DomainStateSignal::new("Published", ConfidenceSignal::Blue, true, vec![], vec![]),
            ]),
            None,
            None,
        )
        .unwrap();

        let json = encode(&doc).unwrap();
        let doc2 = decode(&json).unwrap();
        let signals =
            doc2.domain_state_signals().expect("signals must be present after round-trip");
        assert_eq!(signals.len(), 2);
        assert_eq!(signals[0].state_name(), "Draft");
        assert_eq!(signals[0].signal(), ConfidenceSignal::Yellow);
        assert!(signals[0].found_type());
        assert!(signals[0].found_transitions().is_empty());
        assert_eq!(signals[0].missing_transitions(), &["Published"]);
        assert_eq!(signals[1].state_name(), "Published");
        assert_eq!(signals[1].signal(), ConfidenceSignal::Blue);
        assert!(signals[1].found_transitions().is_empty());
    }

    #[test]
    fn test_decode_domain_state_signals_absent_gives_none() {
        let doc = decode(MINIMAL_JSON).unwrap();
        assert!(doc.domain_state_signals().is_none());
    }

    #[test]
    fn test_encode_omits_domain_state_signals_when_none() {
        let doc = decode(MINIMAL_JSON).unwrap();
        let json = encode(&doc).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("domain_state_signals").is_none());
    }

    // --- signal string mapping ---

    #[test]
    fn test_decode_domain_state_signals_blue_mapping() {
        use domain::ConfidenceSignal;
        let json = r#"{
          "schema_version": 1, "status": "draft", "version": "1", "title": "T",
          "scope": {"in_scope": [], "out_of_scope": []},
          "domain_state_signals": [
            {"state_name": "S", "signal": "blue", "found_type": true}
          ]
        }"#;
        let doc = decode(json).unwrap();
        let sigs = doc.domain_state_signals().unwrap();
        assert_eq!(sigs[0].signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_decode_domain_state_signals_yellow_mapping() {
        use domain::ConfidenceSignal;
        let json = r#"{
          "schema_version": 1, "status": "draft", "version": "1", "title": "T",
          "scope": {"in_scope": [], "out_of_scope": []},
          "domain_state_signals": [
            {"state_name": "S", "signal": "yellow", "found_type": false}
          ]
        }"#;
        let doc = decode(json).unwrap();
        let sigs = doc.domain_state_signals().unwrap();
        assert_eq!(sigs[0].signal(), ConfidenceSignal::Yellow);
    }

    #[test]
    fn test_decode_domain_state_signals_red_mapping() {
        use domain::ConfidenceSignal;
        let json = r#"{
          "schema_version": 1, "status": "draft", "version": "1", "title": "T",
          "scope": {"in_scope": [], "out_of_scope": []},
          "domain_state_signals": [
            {"state_name": "S", "signal": "red", "found_type": false}
          ]
        }"#;
        let doc = decode(json).unwrap();
        let sigs = doc.domain_state_signals().unwrap();
        assert_eq!(sigs[0].signal(), ConfidenceSignal::Red);
    }

    #[test]
    fn test_decode_domain_state_signals_unknown_signal_returns_error() {
        let json = r#"{
          "schema_version": 1, "status": "draft", "version": "1", "title": "T",
          "scope": {"in_scope": [], "out_of_scope": []},
          "domain_state_signals": [
            {"state_name": "S", "signal": "unknown", "found_type": false}
          ]
        }"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, SpecCodecError::InvalidSignalString(_)));
    }

    #[test]
    fn test_decode_domain_state_signals_default_transitions_empty() {
        use domain::ConfidenceSignal;
        // found_transitions and missing_transitions are #[serde(default)] — absence means empty vec
        let json = r#"{
          "schema_version": 1, "status": "draft", "version": "1", "title": "T",
          "scope": {"in_scope": [], "out_of_scope": []},
          "domain_state_signals": [
            {"state_name": "X", "signal": "blue", "found_type": true}
          ]
        }"#;
        let doc = decode(json).unwrap();
        let sigs = doc.domain_state_signals().unwrap();
        assert!(sigs[0].found_transitions().is_empty());
        assert!(sigs[0].missing_transitions().is_empty());
        assert_eq!(sigs[0].signal(), ConfidenceSignal::Blue);
    }

    // --- task_refs round-trip ---

    #[test]
    fn test_decode_task_refs_present() {
        let json = r#"{
  "schema_version": 1,
  "status": "draft",
  "version": "1.0",
  "title": "Feature",
  "scope": {
    "in_scope": [{ "text": "Req", "sources": ["PRD"], "task_refs": ["T001", "T002"] }],
    "out_of_scope": []
  }
}"#;
        let doc = decode(json).unwrap();
        let refs = doc.scope().in_scope()[0].task_refs();
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].as_ref(), "T001");
        assert_eq!(refs[1].as_ref(), "T002");
    }

    #[test]
    fn test_decode_task_refs_omitted_defaults_to_empty() {
        let json = r#"{
  "schema_version": 1,
  "status": "draft",
  "version": "1.0",
  "title": "Feature",
  "scope": {
    "in_scope": [{ "text": "Req", "sources": ["PRD"] }],
    "out_of_scope": []
  }
}"#;
        let doc = decode(json).unwrap();
        assert!(doc.scope().in_scope()[0].task_refs().is_empty());
    }

    #[test]
    fn test_decode_invalid_task_ref_format_returns_error() {
        let json = r#"{
  "schema_version": 1,
  "status": "draft",
  "version": "1.0",
  "title": "Feature",
  "scope": {
    "in_scope": [{ "text": "Req", "sources": ["PRD"], "task_refs": ["not-valid"] }],
    "out_of_scope": []
  }
}"#;
        let result = decode(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_encode_task_refs_round_trip() {
        let json = r#"{
  "schema_version": 1,
  "status": "draft",
  "version": "1.0",
  "title": "Feature",
  "scope": {
    "in_scope": [{ "text": "Req", "sources": ["PRD"], "task_refs": ["T001"] }],
    "out_of_scope": []
  }
}"#;
        let doc = decode(json).unwrap();
        let encoded = encode(&doc).unwrap();
        let re_decoded = decode(&encoded).unwrap();
        let refs = re_decoded.scope().in_scope()[0].task_refs();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].as_ref(), "T001");
    }

    #[test]
    fn test_encode_empty_task_refs_omitted_from_json() {
        let doc = decode(MINIMAL_JSON).unwrap();
        let encoded = encode(&doc).unwrap();
        // task_refs should not appear in output when empty (skip_serializing_if)
        assert!(!encoded.contains("task_refs"));
    }

    // --- content hash + approval round-trip ---

    #[test]
    fn test_content_hash_is_deterministic() {
        let doc = decode(MINIMAL_JSON).unwrap();
        let h1 = compute_content_hash(&doc).unwrap();
        let h2 = compute_content_hash(&doc).unwrap();
        assert_eq!(h1, h2);
        assert!(h1.starts_with("sha256:"));
    }

    #[test]
    fn test_content_hash_ignores_task_refs_changes() {
        let without_refs = r#"{"schema_version":1,"status":"draft","version":"1.0","title":"T","scope":{"in_scope":[{"text":"item","sources":["PRD"]}],"out_of_scope":[]}}"#;
        let with_refs = r#"{"schema_version":1,"status":"draft","version":"1.0","title":"T","scope":{"in_scope":[{"text":"item","sources":["PRD"],"task_refs":["T001","T002"]}],"out_of_scope":[]}}"#;
        let h1 = compute_content_hash(&decode(without_refs).unwrap()).unwrap();
        let h2 = compute_content_hash(&decode(with_refs).unwrap()).unwrap();
        assert_eq!(h1, h2, "task_refs should not affect content hash");
    }

    #[test]
    fn test_content_hash_changes_when_goal_changes() {
        let json1 = r#"{"schema_version":1,"status":"draft","version":"1.0","title":"T","goal":["A"],"scope":{"in_scope":[],"out_of_scope":[]}}"#;
        let json2 = r#"{"schema_version":1,"status":"draft","version":"1.0","title":"T","goal":["B"],"scope":{"in_scope":[],"out_of_scope":[]}}"#;
        let h1 = compute_content_hash(&decode(json1).unwrap()).unwrap();
        let h2 = compute_content_hash(&decode(json2).unwrap()).unwrap();
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_approval_round_trip() {
        let mut doc = decode(MINIMAL_JSON).unwrap();
        let hash = compute_content_hash(&doc).unwrap();
        let ts = domain::Timestamp::new("2026-03-24T10:00:00Z").unwrap();
        doc.approve(ts, hash.clone()).unwrap();

        let json = encode(&doc).unwrap();
        let reloaded = decode(&json).unwrap();

        assert_eq!(reloaded.status(), domain::SpecStatus::Approved);
        assert!(reloaded.approved_at().is_some());
        assert_eq!(reloaded.content_hash(), Some(hash.as_str()));
        assert!(reloaded.is_approval_valid(&hash));
    }

    #[test]
    fn test_effective_status_draft_after_content_change() {
        let mut doc = decode(MINIMAL_JSON).unwrap();
        let hash = compute_content_hash(&doc).unwrap();
        let ts = domain::Timestamp::new("2026-03-24T10:00:00Z").unwrap();
        doc.approve(ts, hash).unwrap();

        // Simulate content change: decode a modified spec with stale content_hash.
        // decode() auto-demotes when hash doesn't match.
        let modified = r#"{"schema_version":1,"status":"approved","version":"1.0","title":"Feature Title CHANGED","approved_at":"2026-03-24T10:00:00Z","content_hash":"sha256:old","scope":{"in_scope":[],"out_of_scope":[]}}"#;
        let reloaded = decode(modified).unwrap();

        assert_eq!(
            reloaded.status(),
            domain::SpecStatus::Draft,
            "decode should auto-demote when content hash mismatches"
        );
        assert!(reloaded.approved_at().is_none(), "auto-demote should clear approved_at");
    }
}
