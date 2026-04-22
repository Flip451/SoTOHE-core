//! Serde codec for spec.json (SpecDocument SSoT).
//!
//! Schema version 2: ADR 2026-04-19-1242 §D1.2 — approved-lifecycle fields
//! (`status`, `approved_at`, `content_hash`) removed; each requirement now
//! carries a required `id`; `sources: Vec<String>` replaced by three typed
//! ref arrays; `task_refs` removed (moved to task-coverage.json); `goal`
//! promoted from `Vec<String>` to `Vec<SpecRequirementDto>`; `related_conventions`
//! promoted from `Vec<String>` to `Vec<ConventionRefDto>`.

use std::path::PathBuf;

use domain::{
    AdrAnchor, AdrRef, ConventionAnchor, ConventionRef, HearingMode, HearingRecord,
    HearingSignalDelta, HearingSignalSnapshot, InformalGroundKind, InformalGroundRef,
    InformalGroundSummary, SignalCounts, SpecDocument, SpecElementId, SpecRequirement, SpecScope,
    SpecSection, SpecValidationError, Timestamp, ValidationError,
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

    #[error("domain validation error: {0}")]
    DomainValidation(#[from] ValidationError),

    #[error("unsupported schema_version: expected 2, got {0}")]
    UnsupportedSchemaVersion(u32),

    #[error("invalid field '{field}': {reason}")]
    InvalidField { field: String, reason: String },
}

// ---------------------------------------------------------------------------
// DTO types
// ---------------------------------------------------------------------------

/// Top-level DTO for spec.json (schema_version 2).
///
/// `deny_unknown_fields` rejects legacy v1 fields (`status`, `approved_at`,
/// `content_hash`, `sources`, `task_refs`) so that `verify_spec_schema()`
/// actually enforces the v2 schema contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SpecDocumentDto {
    pub schema_version: u32,
    pub version: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub goal: Vec<SpecRequirementDto>,
    pub scope: SpecScopeDto,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub constraints: Vec<SpecRequirementDto>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acceptance_criteria: Vec<SpecRequirementDto>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_sections: Vec<SpecSectionDto>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related_conventions: Vec<ConventionRefDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signals: Option<SignalCountsDto>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hearing_history: Vec<HearingRecordDto>,
}

/// DTO for a hearing session record.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct HearingRecordDto {
    pub date: String,
    pub mode: String,
    pub signal_delta: HearingSignalDeltaDto,
    pub questions_asked: u32,
    pub items_added: u32,
    pub items_modified: u32,
}

/// DTO for signal delta (before + after snapshots).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct HearingSignalDeltaDto {
    pub before: SignalCountsDto,
    pub after: SignalCountsDto,
}

/// DTO for a single requirement with typed provenance references.
///
/// `deny_unknown_fields` rejects legacy v1 per-requirement fields (`sources`,
/// `task_refs`) so that v1 spec.json files are not silently accepted.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SpecRequirementDto {
    pub id: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub adr_refs: Vec<AdrRefDto>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub convention_refs: Vec<ConventionRefDto>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub informal_grounds: Vec<InformalGroundRefDto>,
}

/// DTO for a reference to a section in an ADR document.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdrRefDto {
    pub file: String,
    pub anchor: String,
}

/// DTO for a reference to a section in a convention document.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConventionRefDto {
    pub file: String,
    pub anchor: String,
}

/// DTO for an informal (unpersisted) ground reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InformalGroundRefDto {
    pub kind: String,
    pub summary: String,
}

/// DTO for the scope section.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SpecScopeDto {
    #[serde(default)]
    pub in_scope: Vec<SpecRequirementDto>,
    #[serde(default)]
    pub out_of_scope: Vec<SpecRequirementDto>,
}

/// DTO for a free-form additional section.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SpecSectionDto {
    pub title: String,
    #[serde(default)]
    pub content: Vec<String>,
}

/// DTO for aggregate signal counts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SignalCountsDto {
    pub blue: u32,
    pub yellow: u32,
    pub red: u32,
}

// ---------------------------------------------------------------------------
// Decode: JSON -> domain
// ---------------------------------------------------------------------------

/// Deserializes a spec.json string into a `SpecDocument`.
///
/// # Errors
///
/// Returns `SpecCodecError::Json` if the JSON is malformed.
/// Returns `SpecCodecError::UnsupportedSchemaVersion` if `schema_version != 2`.
/// Returns `SpecCodecError::Validation` if any domain type construction fails.
/// Returns `SpecCodecError::DomainValidation` if any plan_ref newtype fails validation.
pub fn decode(json: &str) -> Result<SpecDocument, SpecCodecError> {
    let dto: SpecDocumentDto = serde_json::from_str(json)?;

    if dto.schema_version != 2 {
        return Err(SpecCodecError::UnsupportedSchemaVersion(dto.schema_version));
    }

    let goal = dto.goal.into_iter().map(requirement_from_dto).collect::<Result<Vec<_>, _>>()?;

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

    let acceptance_criteria = dto
        .acceptance_criteria
        .into_iter()
        .map(requirement_from_dto)
        .collect::<Result<Vec<_>, _>>()?;

    let additional_sections =
        dto.additional_sections.into_iter().map(section_from_dto).collect::<Result<Vec<_>, _>>()?;

    let related_conventions = dto
        .related_conventions
        .into_iter()
        .map(convention_ref_from_dto)
        .collect::<Result<Vec<_>, _>>()?;

    let signals = dto.signals.map(signal_counts_from_dto);

    let mut doc = SpecDocument::new(
        dto.title,
        dto.version,
        goal,
        scope,
        constraints,
        acceptance_criteria,
        additional_sections,
        related_conventions,
        signals,
    )?;

    // Decode hearing history (append-only).
    for record in decode_hearing_history(&dto.hearing_history)? {
        doc.append_hearing_record(record);
    }

    Ok(doc)
}

fn decode_hearing_history(dtos: &[HearingRecordDto]) -> Result<Vec<HearingRecord>, SpecCodecError> {
    dtos.iter()
        .enumerate()
        .map(|(i, dto)| {
            let date = Timestamp::new(&dto.date).map_err(|e| SpecCodecError::InvalidField {
                field: format!("hearing_history[{i}].date"),
                reason: e.to_string(),
            })?;
            let mode = hearing_mode_from_str(&dto.mode).map_err(|reason| {
                SpecCodecError::InvalidField { field: format!("hearing_history[{i}].mode"), reason }
            })?;
            let before = HearingSignalSnapshot::new(
                dto.signal_delta.before.blue,
                dto.signal_delta.before.yellow,
                dto.signal_delta.before.red,
            );
            let after = HearingSignalSnapshot::new(
                dto.signal_delta.after.blue,
                dto.signal_delta.after.yellow,
                dto.signal_delta.after.red,
            );
            Ok(HearingRecord::new(
                date,
                mode,
                HearingSignalDelta::new(before, after),
                dto.questions_asked,
                dto.items_added,
                dto.items_modified,
            ))
        })
        .collect()
}

fn hearing_mode_from_str(s: &str) -> Result<HearingMode, String> {
    match s {
        "full" => Ok(HearingMode::Full),
        "focused" => Ok(HearingMode::Focused),
        "quick" => Ok(HearingMode::Quick),
        other => Err(format!("unknown hearing mode: {other}")),
    }
}

fn requirement_from_dto(dto: SpecRequirementDto) -> Result<SpecRequirement, SpecCodecError> {
    let id = SpecElementId::try_new(dto.id)?;
    let adr_refs = dto.adr_refs.into_iter().map(adr_ref_from_dto).collect::<Result<Vec<_>, _>>()?;
    let convention_refs = dto
        .convention_refs
        .into_iter()
        .map(convention_ref_from_dto)
        .collect::<Result<Vec<_>, _>>()?;
    let informal_grounds = dto
        .informal_grounds
        .into_iter()
        .map(informal_ground_ref_from_dto)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SpecRequirement::new(id, dto.text, adr_refs, convention_refs, informal_grounds)?)
}

fn adr_ref_from_dto(dto: AdrRefDto) -> Result<AdrRef, SpecCodecError> {
    let anchor = AdrAnchor::try_new(dto.anchor)?;
    Ok(AdrRef::new(PathBuf::from(dto.file), anchor))
}

fn convention_ref_from_dto(dto: ConventionRefDto) -> Result<ConventionRef, SpecCodecError> {
    let anchor = ConventionAnchor::try_new(dto.anchor)?;
    Ok(ConventionRef::new(PathBuf::from(dto.file), anchor))
}

fn informal_ground_ref_from_dto(
    dto: InformalGroundRefDto,
) -> Result<InformalGroundRef, SpecCodecError> {
    let kind = informal_ground_kind_from_str(&dto.kind).map_err(|reason| {
        SpecCodecError::InvalidField { field: "informal_grounds[].kind".to_owned(), reason }
    })?;
    let summary = InformalGroundSummary::try_new(dto.summary)?;
    Ok(InformalGroundRef::new(kind, summary))
}

fn informal_ground_kind_from_str(s: &str) -> Result<InformalGroundKind, String> {
    match s {
        "discussion" => Ok(InformalGroundKind::Discussion),
        "feedback" => Ok(InformalGroundKind::Feedback),
        "memory" => Ok(InformalGroundKind::Memory),
        "user_directive" => Ok(InformalGroundKind::UserDirective),
        other => Err(format!(
            "unknown kind '{other}': expected 'discussion', 'feedback', 'memory', or 'user_directive'"
        )),
    }
}

fn section_from_dto(dto: SpecSectionDto) -> Result<SpecSection, SpecValidationError> {
    SpecSection::new(dto.title, dto.content)
}

fn signal_counts_from_dto(dto: SignalCountsDto) -> SignalCounts {
    SignalCounts::new(dto.blue, dto.yellow, dto.red)
}

// ---------------------------------------------------------------------------
// Encode: domain -> JSON
// ---------------------------------------------------------------------------

/// Serializes a `SpecDocument` to a pretty-printed spec.json string.
///
/// # Errors
///
/// Returns `SpecCodecError::Json` if serialization fails.
/// Returns `SpecCodecError::InvalidField` if any file path is not valid UTF-8.
pub fn encode(doc: &SpecDocument) -> Result<String, SpecCodecError> {
    let dto = spec_document_to_dto(doc)?;
    Ok(serde_json::to_string_pretty(&dto)?)
}

fn spec_document_to_dto(doc: &SpecDocument) -> Result<SpecDocumentDto, SpecCodecError> {
    let goal = doc.goal().iter().map(requirement_to_dto).collect::<Result<Vec<_>, _>>()?;
    let in_scope =
        doc.scope().in_scope().iter().map(requirement_to_dto).collect::<Result<Vec<_>, _>>()?;
    let out_of_scope =
        doc.scope().out_of_scope().iter().map(requirement_to_dto).collect::<Result<Vec<_>, _>>()?;
    let constraints =
        doc.constraints().iter().map(requirement_to_dto).collect::<Result<Vec<_>, _>>()?;
    let acceptance_criteria =
        doc.acceptance_criteria().iter().map(requirement_to_dto).collect::<Result<Vec<_>, _>>()?;
    let related_conventions = doc
        .related_conventions()
        .iter()
        .map(convention_ref_to_dto)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SpecDocumentDto {
        schema_version: 2,
        version: doc.version().to_owned(),
        title: doc.title().to_owned(),
        goal,
        scope: SpecScopeDto { in_scope, out_of_scope },
        constraints,
        acceptance_criteria,
        additional_sections: doc.additional_sections().iter().map(section_to_dto).collect(),
        related_conventions,
        signals: doc.signals().map(signal_counts_to_dto),
        hearing_history: doc.hearing_history().iter().map(hearing_record_to_dto).collect(),
    })
}

fn hearing_record_to_dto(rec: &HearingRecord) -> HearingRecordDto {
    HearingRecordDto {
        date: rec.date().as_str().to_owned(),
        mode: rec.mode().as_str().to_owned(),
        signal_delta: HearingSignalDeltaDto {
            before: SignalCountsDto {
                blue: rec.signal_delta().before().blue(),
                yellow: rec.signal_delta().before().yellow(),
                red: rec.signal_delta().before().red(),
            },
            after: SignalCountsDto {
                blue: rec.signal_delta().after().blue(),
                yellow: rec.signal_delta().after().yellow(),
                red: rec.signal_delta().after().red(),
            },
        },
        questions_asked: rec.questions_asked(),
        items_added: rec.items_added(),
        items_modified: rec.items_modified(),
    }
}

fn requirement_to_dto(req: &SpecRequirement) -> Result<SpecRequirementDto, SpecCodecError> {
    let adr_refs = req.adr_refs().iter().map(adr_ref_to_dto).collect::<Result<Vec<_>, _>>()?;
    let convention_refs =
        req.convention_refs().iter().map(convention_ref_to_dto).collect::<Result<Vec<_>, _>>()?;
    Ok(SpecRequirementDto {
        id: req.id().as_ref().to_owned(),
        text: req.text().to_owned(),
        adr_refs,
        convention_refs,
        informal_grounds: req.informal_grounds().iter().map(informal_ground_ref_to_dto).collect(),
    })
}

fn adr_ref_to_dto(r: &AdrRef) -> Result<AdrRefDto, SpecCodecError> {
    let file = r.file.to_str().ok_or_else(|| SpecCodecError::InvalidField {
        field: "adr_refs[].file".to_owned(),
        reason: format!("path is not valid UTF-8: {:?}", r.file),
    })?;
    Ok(AdrRefDto { file: file.to_owned(), anchor: r.anchor.as_ref().to_owned() })
}

fn convention_ref_to_dto(r: &ConventionRef) -> Result<ConventionRefDto, SpecCodecError> {
    let file = r.file.to_str().ok_or_else(|| SpecCodecError::InvalidField {
        field: "convention_refs[].file".to_owned(),
        reason: format!("path is not valid UTF-8: {:?}", r.file),
    })?;
    Ok(ConventionRefDto { file: file.to_owned(), anchor: r.anchor.as_ref().to_owned() })
}

fn informal_ground_ref_to_dto(r: &InformalGroundRef) -> InformalGroundRefDto {
    InformalGroundRefDto {
        kind: r.kind.as_str().to_owned(),
        summary: r.summary.as_ref().to_owned(),
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
  "schema_version": 2,
  "version": "1.0",
  "title": "Feature Title",
  "scope": {
    "in_scope": [],
    "out_of_scope": []
  }
}"#;

    const FULL_JSON: &str = r#"{
  "schema_version": 2,
  "version": "1.0",
  "title": "Feature Title",
  "goal": [
    {
      "id": "GL-01",
      "text": "Goal item",
      "adr_refs": [{"file": "knowledge/adr/2026-04-19-1242.md", "anchor": "D1.2"}]
    }
  ],
  "scope": {
    "in_scope": [
      {
        "id": "IN-01",
        "text": "Req 1",
        "adr_refs": [{"file": "knowledge/adr/2026-04-19-1242.md", "anchor": "D1.2"}]
      }
    ],
    "out_of_scope": [
      {
        "id": "OS-01",
        "text": "Excluded 1",
        "informal_grounds": [{"kind": "discussion", "summary": "agreed out of scope"}]
      }
    ]
  },
  "constraints": [
    {
      "id": "CO-01",
      "text": "Constraint 1",
      "convention_refs": [{"file": ".claude/rules/04-coding-principles.md", "anchor": "newtype-pattern"}]
    }
  ],
  "acceptance_criteria": [
    {
      "id": "AC-01",
      "text": "AC 1",
      "adr_refs": [{"file": "knowledge/adr/2026-04-19-1242.md", "anchor": "D3.1"}]
    }
  ],
  "additional_sections": [{"title": "Custom Section", "content": ["Line 1"]}],
  "related_conventions": [
    {"file": "knowledge/conventions/source-attribution.md", "anchor": "intro"}
  ],
  "signals": { "blue": 3, "yellow": 1, "red": 0 }
}"#;

    // --- decode: happy path ---

    #[test]
    fn test_decode_minimal_json_succeeds() {
        let doc = decode(MINIMAL_JSON).unwrap();
        assert_eq!(doc.title(), "Feature Title");
        assert_eq!(doc.version(), "1.0");
        assert!(doc.goal().is_empty());
        assert!(doc.scope().in_scope().is_empty());
        assert!(doc.scope().out_of_scope().is_empty());
        assert!(doc.constraints().is_empty());
        assert!(doc.acceptance_criteria().is_empty());
        assert!(doc.additional_sections().is_empty());
        assert!(doc.related_conventions().is_empty());
        assert!(doc.signals().is_none());
    }

    #[test]
    fn test_decode_full_json_succeeds() {
        let doc = decode(FULL_JSON).unwrap();
        assert_eq!(doc.title(), "Feature Title");
        assert_eq!(doc.goal().len(), 1);
        assert_eq!(doc.goal()[0].id().as_ref(), "GL-01");
        assert_eq!(doc.goal()[0].text(), "Goal item");
        assert_eq!(doc.goal()[0].adr_refs().len(), 1);
        assert_eq!(doc.scope().in_scope().len(), 1);
        assert_eq!(doc.scope().in_scope()[0].id().as_ref(), "IN-01");
        assert_eq!(doc.scope().in_scope()[0].text(), "Req 1");
        assert_eq!(doc.scope().out_of_scope().len(), 1);
        assert_eq!(doc.scope().out_of_scope()[0].informal_grounds().len(), 1);
        assert_eq!(doc.constraints().len(), 1);
        assert_eq!(doc.constraints()[0].convention_refs().len(), 1);
        assert_eq!(doc.acceptance_criteria().len(), 1);
        assert_eq!(doc.additional_sections().len(), 1);
        assert_eq!(doc.additional_sections()[0].title(), "Custom Section");
        assert_eq!(doc.related_conventions().len(), 1);
        let signals = doc.signals().unwrap();
        assert_eq!(signals.blue(), 3);
        assert_eq!(signals.yellow(), 1);
        assert_eq!(signals.red(), 0);
    }

    // --- decode: optional fields default correctly ---

    #[test]
    fn test_decode_with_absent_goal_defaults_to_empty() {
        let json = r#"{"schema_version":2,"version":"1","title":"T","scope":{"in_scope":[],"out_of_scope":[]}}"#;
        let doc = decode(json).unwrap();
        assert!(doc.goal().is_empty());
    }

    #[test]
    fn test_decode_with_null_signals_gives_none() {
        let json = r#"{"schema_version":2,"version":"1","title":"T","scope":{"in_scope":[],"out_of_scope":[]},"signals":null}"#;
        let doc = decode(json).unwrap();
        assert!(doc.signals().is_none());
    }

    #[test]
    fn test_decode_additional_sections_defaults_to_empty() {
        let json = r#"{"schema_version":2,"version":"1","title":"T","scope":{"in_scope":[],"out_of_scope":[]}}"#;
        let doc = decode(json).unwrap();
        assert!(doc.additional_sections().is_empty());
    }

    #[test]
    fn test_decode_related_conventions_defaults_to_empty() {
        let json = r#"{"schema_version":2,"version":"1","title":"T","scope":{"in_scope":[],"out_of_scope":[]}}"#;
        let doc = decode(json).unwrap();
        assert!(doc.related_conventions().is_empty());
    }

    #[test]
    fn test_decode_requirement_without_refs_defaults_to_empty() {
        let json = r#"{
          "schema_version": 2, "version": "1", "title": "T",
          "scope": {
            "in_scope": [{"id": "IN-01", "text": "needs req"}],
            "out_of_scope": []
          }
        }"#;
        let doc = decode(json).unwrap();
        assert!(doc.scope().in_scope()[0].adr_refs().is_empty());
        assert!(doc.scope().in_scope()[0].convention_refs().is_empty());
        assert!(doc.scope().in_scope()[0].informal_grounds().is_empty());
    }

    // --- decode: schema_version validation ---

    #[test]
    fn test_decode_with_unsupported_schema_version_1_returns_error() {
        // schema_version 1 without any unknown fields — the version check fires first.
        // (v1 files with unknown fields like `status` will hit a Json error first, which
        //  is also a valid rejection; this test covers the schema_version gate path.)
        let json = r#"{"schema_version":1,"version":"1","title":"T","scope":{"in_scope":[],"out_of_scope":[]}}"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, SpecCodecError::UnsupportedSchemaVersion(1)));
    }

    #[test]
    fn test_decode_v1_with_unknown_status_field_is_rejected() {
        // v1 spec.json with legacy `status` field — deny_unknown_fields rejects it.
        let json = r#"{"schema_version":1,"status":"draft","version":"1","title":"T","scope":{"in_scope":[],"out_of_scope":[]}}"#;
        let err = decode(json).unwrap_err();
        // Rejected either as Json (unknown field) or UnsupportedSchemaVersion — both are correct.
        assert!(
            matches!(err, SpecCodecError::Json(_) | SpecCodecError::UnsupportedSchemaVersion(1)),
            "expected Json or UnsupportedSchemaVersion, got: {err}"
        );
    }

    #[test]
    fn test_decode_with_schema_version_zero_returns_error() {
        let json = r#"{"schema_version":0,"version":"1","title":"T","scope":{"in_scope":[],"out_of_scope":[]}}"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, SpecCodecError::UnsupportedSchemaVersion(0)));
    }

    // --- decode: domain validation errors ---

    #[test]
    fn test_decode_with_empty_title_returns_validation_error() {
        let json = r#"{"schema_version":2,"version":"1","title":"","scope":{"in_scope":[],"out_of_scope":[]}}"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, SpecCodecError::Validation(SpecValidationError::EmptyTitle)));
    }

    #[test]
    fn test_decode_with_empty_version_returns_validation_error() {
        let json = r#"{"schema_version":2,"version":"","title":"T","scope":{"in_scope":[],"out_of_scope":[]}}"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, SpecCodecError::Validation(SpecValidationError::EmptyVersion)));
    }

    #[test]
    fn test_decode_with_empty_requirement_text_returns_validation_error() {
        let json = r#"{
          "schema_version": 2, "version": "1", "title": "T",
          "scope": {"in_scope": [{"id": "IN-01", "text": ""}], "out_of_scope": []}
        }"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(
            err,
            SpecCodecError::Validation(SpecValidationError::EmptyRequirementText)
        ));
    }

    #[test]
    fn test_decode_with_empty_section_title_returns_validation_error() {
        let json = r#"{
          "schema_version": 2, "version": "1", "title": "T",
          "scope": {"in_scope": [], "out_of_scope": []},
          "additional_sections": [{"title": "", "content": []}]
        }"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, SpecCodecError::Validation(SpecValidationError::EmptySectionTitle)));
    }

    #[test]
    fn test_decode_with_invalid_spec_element_id_returns_domain_validation_error() {
        let json = r#"{
          "schema_version": 2, "version": "1", "title": "T",
          "scope": {"in_scope": [{"id": "A-01", "text": "bad id"}], "out_of_scope": []}
        }"#;
        // "A-01" has only one uppercase letter prefix — invalid
        let err = decode(json).unwrap_err();
        assert!(matches!(
            err,
            SpecCodecError::DomainValidation(ValidationError::InvalidSpecElementId(_))
        ));
    }

    #[test]
    fn test_decode_with_duplicate_element_ids_returns_duplicate_error() {
        let json = r#"{
          "schema_version": 2, "version": "1", "title": "T",
          "scope": {
            "in_scope": [{"id": "IN-01", "text": "first"}],
            "out_of_scope": []
          },
          "constraints": [{"id": "IN-01", "text": "duplicate"}]
        }"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(
            err,
            SpecCodecError::Validation(SpecValidationError::DuplicateElementId(_))
        ));
    }

    #[test]
    fn test_decode_with_invalid_informal_ground_kind_returns_error() {
        let json = r#"{
          "schema_version": 2, "version": "1", "title": "T",
          "scope": {
            "in_scope": [{"id": "IN-01", "text": "req", "informal_grounds": [{"kind": "typo", "summary": "test"}]}],
            "out_of_scope": []
          }
        }"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, SpecCodecError::InvalidField { .. }));
    }

    #[test]
    fn test_decode_with_empty_adr_anchor_returns_error() {
        let json = r#"{
          "schema_version": 2, "version": "1", "title": "T",
          "scope": {
            "in_scope": [{"id": "IN-01", "text": "req", "adr_refs": [{"file": "x.md", "anchor": ""}]}],
            "out_of_scope": []
          }
        }"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, SpecCodecError::DomainValidation(ValidationError::EmptyAdrAnchor)));
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
            "1.0",
            vec![],
            SpecScope::new(vec![], vec![]),
            vec![],
            vec![],
            vec![],
            vec![],
            None,
        )
        .unwrap();

        let json = encode(&doc).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["schema_version"], 2);
        assert_eq!(parsed["title"], "T");
        assert_eq!(parsed["version"], "1.0");
        // No status field in schema version 2
        assert!(parsed.get("status").is_none());
    }

    #[test]
    fn test_encode_omits_status_approved_at_content_hash() {
        let doc = SpecDocument::new(
            "T",
            "1",
            vec![],
            SpecScope::new(vec![], vec![]),
            vec![],
            vec![],
            vec![],
            vec![],
            None,
        )
        .unwrap();
        let json = encode(&doc).unwrap();
        assert!(!json.contains("\"status\""), "status must not be in schema v2 output");
        assert!(!json.contains("\"approved_at\""), "approved_at must not be in schema v2 output");
        assert!(!json.contains("\"content_hash\""), "content_hash must not be in schema v2 output");
    }

    #[test]
    fn test_encode_omits_signals_when_none() {
        let doc = SpecDocument::new(
            "T",
            "1",
            vec![],
            SpecScope::new(vec![], vec![]),
            vec![],
            vec![],
            vec![],
            vec![],
            None,
        )
        .unwrap();
        let json = encode(&doc).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("signals").is_none());
    }

    #[test]
    fn test_encode_includes_signals_when_present() {
        let mut doc = SpecDocument::new(
            "T",
            "1",
            vec![],
            SpecScope::new(vec![], vec![]),
            vec![],
            vec![],
            vec![],
            vec![],
            None,
        )
        .unwrap();
        doc.set_signals(SignalCounts::new(5, 2, 1));
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
            "1",
            vec![],
            SpecScope::new(vec![], vec![]),
            vec![],
            vec![],
            vec![],
            vec![],
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
            "1",
            vec![],
            SpecScope::new(vec![], vec![]),
            vec![],
            vec![],
            vec![],
            vec![],
            None,
        )
        .unwrap();
        let json = encode(&doc).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("related_conventions").is_none());
    }

    #[test]
    fn test_encode_requirement_with_adr_refs() {
        use domain::{AdrAnchor, AdrRef};
        let anchor = AdrAnchor::try_new("D1.2").unwrap();
        let adr_ref = AdrRef::new(PathBuf::from("knowledge/adr/x.md"), anchor);
        let id = SpecElementId::try_new("IN-01").unwrap();
        let req = SpecRequirement::new(id, "req", vec![adr_ref], vec![], vec![]).unwrap();
        let doc = SpecDocument::new(
            "T",
            "1",
            vec![],
            SpecScope::new(vec![req], vec![]),
            vec![],
            vec![],
            vec![],
            vec![],
            None,
        )
        .unwrap();
        let json = encode(&doc).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let in_scope = &parsed["scope"]["in_scope"];
        assert_eq!(in_scope[0]["id"], "IN-01");
        assert_eq!(in_scope[0]["adr_refs"][0]["file"], "knowledge/adr/x.md");
        assert_eq!(in_scope[0]["adr_refs"][0]["anchor"], "D1.2");
    }

    #[test]
    fn test_encode_requirement_with_convention_refs() {
        use domain::{ConventionAnchor, ConventionRef};
        let anchor = ConventionAnchor::try_new("newtype-pattern").unwrap();
        let conv_ref = ConventionRef::new(PathBuf::from(".claude/rules/04.md"), anchor);
        let id = SpecElementId::try_new("CO-01").unwrap();
        let req = SpecRequirement::new(id, "constraint", vec![], vec![conv_ref], vec![]).unwrap();
        let doc = SpecDocument::new(
            "T",
            "1",
            vec![],
            SpecScope::new(vec![], vec![]),
            vec![req],
            vec![],
            vec![],
            vec![],
            None,
        )
        .unwrap();
        let json = encode(&doc).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let constraints = &parsed["constraints"];
        assert_eq!(constraints[0]["convention_refs"][0]["anchor"], "newtype-pattern");
    }

    #[test]
    fn test_encode_requirement_with_informal_grounds() {
        use domain::{InformalGroundKind, InformalGroundRef, InformalGroundSummary};
        let summary = InformalGroundSummary::try_new("user directive").unwrap();
        let informal = InformalGroundRef::new(InformalGroundKind::UserDirective, summary);
        let id = SpecElementId::try_new("AC-01").unwrap();
        let req = SpecRequirement::new(id, "AC 1", vec![], vec![], vec![informal]).unwrap();
        let doc = SpecDocument::new(
            "T",
            "1",
            vec![],
            SpecScope::new(vec![], vec![]),
            vec![],
            vec![req],
            vec![],
            vec![],
            None,
        )
        .unwrap();
        let json = encode(&doc).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        let ac = &parsed["acceptance_criteria"];
        assert_eq!(ac[0]["informal_grounds"][0]["kind"], "user_directive");
        assert_eq!(ac[0]["informal_grounds"][0]["summary"], "user directive");
    }

    #[test]
    fn test_encode_related_conventions_as_struct() {
        use domain::{ConventionAnchor, ConventionRef};
        let anchor = ConventionAnchor::try_new("intro").unwrap();
        let conv = ConventionRef::new(
            PathBuf::from("knowledge/conventions/source-attribution.md"),
            anchor,
        );
        let doc = SpecDocument::new(
            "T",
            "1",
            vec![],
            SpecScope::new(vec![], vec![]),
            vec![],
            vec![],
            vec![],
            vec![conv],
            None,
        )
        .unwrap();
        let json = encode(&doc).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(
            parsed["related_conventions"][0]["file"],
            "knowledge/conventions/source-attribution.md"
        );
        assert_eq!(parsed["related_conventions"][0]["anchor"], "intro");
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
    fn test_round_trip_all_informal_ground_kinds() {
        let kinds = [
            ("discussion", InformalGroundKind::Discussion),
            ("feedback", InformalGroundKind::Feedback),
            ("memory", InformalGroundKind::Memory),
            ("user_directive", InformalGroundKind::UserDirective),
        ];
        for (kind_str, kind_enum) in &kinds {
            let json = format!(
                r#"{{
                  "schema_version": 2, "version": "1", "title": "T",
                  "scope": {{
                    "in_scope": [{{
                      "id": "IN-01", "text": "req",
                      "informal_grounds": [{{"kind": "{kind_str}", "summary": "test"}}]
                    }}],
                    "out_of_scope": []
                  }}
                }}"#
            );
            let doc = decode(&json).unwrap();
            assert_eq!(
                doc.scope().in_scope()[0].informal_grounds()[0].kind,
                *kind_enum,
                "kind {kind_str} should round-trip"
            );
        }
    }

    #[test]
    fn test_round_trip_multiple_requirements_with_typed_refs() {
        let json = r#"{
          "schema_version": 2, "version": "1.0", "title": "Multi",
          "scope": {
            "in_scope": [
              {
                "id": "IN-01",
                "text": "R1",
                "adr_refs": [{"file": "adr/x.md", "anchor": "D1"}, {"file": "adr/y.md", "anchor": "D2"}]
              },
              {
                "id": "IN-02",
                "text": "R2",
                "convention_refs": [{"file": "conventions/style.md", "anchor": "section-1"}]
              }
            ],
            "out_of_scope": [
              {
                "id": "OS-01",
                "text": "X1",
                "informal_grounds": [{"kind": "feedback", "summary": "low value"}]
              }
            ]
          },
          "acceptance_criteria": [
            {"id": "AC-01", "text": "AC1"},
            {"id": "AC-02", "text": "AC2", "informal_grounds": [{"kind": "discussion", "summary": "agreed"}]}
          ]
        }"#;
        let doc = decode(json).unwrap();
        let encoded = encode(&doc).unwrap();
        let doc2 = decode(&encoded).unwrap();
        assert_eq!(doc2.scope().in_scope().len(), 2);
        assert_eq!(doc2.scope().in_scope()[0].adr_refs().len(), 2);
        assert_eq!(doc2.scope().in_scope()[1].convention_refs().len(), 1);
        assert_eq!(doc2.scope().out_of_scope().len(), 1);
        assert_eq!(doc2.acceptance_criteria().len(), 2);
        assert!(doc2.acceptance_criteria()[0].informal_grounds().is_empty());
        assert_eq!(doc2.acceptance_criteria()[1].informal_grounds().len(), 1);
    }

    #[test]
    fn test_round_trip_additional_sections() {
        let json = r#"{
          "schema_version": 2, "version": "1", "title": "T",
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
        assert!(json.contains('\n'));
    }

    #[test]
    fn test_encode_schema_version_is_always_2() {
        let doc = decode(FULL_JSON).unwrap();
        let json = encode(&doc).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["schema_version"], 2);
    }

    #[test]
    fn test_encode_omits_empty_refs_from_requirements() {
        let id = SpecElementId::try_new("IN-01").unwrap();
        let req = SpecRequirement::new(id, "req", vec![], vec![], vec![]).unwrap();
        let doc = SpecDocument::new(
            "T",
            "1",
            vec![],
            SpecScope::new(vec![req], vec![]),
            vec![],
            vec![],
            vec![],
            vec![],
            None,
        )
        .unwrap();
        let json = encode(&doc).unwrap();
        // Empty arrays must be omitted (skip_serializing_if = Vec::is_empty)
        assert!(!json.contains("\"adr_refs\""), "empty adr_refs must be omitted");
        assert!(!json.contains("\"convention_refs\""), "empty convention_refs must be omitted");
        assert!(!json.contains("\"informal_grounds\""), "empty informal_grounds must be omitted");
    }

    // --- Hearing history round-trip ---

    #[test]
    fn test_hearing_history_roundtrip() {
        let json = r#"{
            "schema_version": 2,
            "version": "1.0",
            "title": "Feature",
            "scope": {"in_scope": [], "out_of_scope": []},
            "hearing_history": [
                {
                    "date": "2026-04-01T10:00:00Z",
                    "mode": "focused",
                    "signal_delta": {
                        "before": {"blue": 5, "yellow": 3, "red": 2},
                        "after": {"blue": 8, "yellow": 2, "red": 0}
                    },
                    "questions_asked": 4,
                    "items_added": 1,
                    "items_modified": 3
                }
            ]
        }"#;
        let doc = decode(json).unwrap();
        assert_eq!(doc.hearing_history().len(), 1);
        let rec = &doc.hearing_history()[0];
        assert_eq!(rec.mode(), domain::HearingMode::Focused);
        assert_eq!(rec.signal_delta().before().blue(), 5);
        assert_eq!(rec.signal_delta().after().red(), 0);
        assert_eq!(rec.questions_asked(), 4);

        let re_encoded = encode(&doc).unwrap();
        let doc2 = decode(&re_encoded).unwrap();
        assert_eq!(doc2.hearing_history().len(), 1);
        assert_eq!(doc2.hearing_history()[0].mode(), domain::HearingMode::Focused);
    }

    #[test]
    fn test_hearing_history_absent_defaults_to_empty() {
        let json = r#"{"schema_version":2,"version":"1.0","title":"Old","scope":{"in_scope":[],"out_of_scope":[]}}"#;
        let doc = decode(json).unwrap();
        assert!(doc.hearing_history().is_empty());
    }
}
