//! Serde codec for spec.json (SpecDocument SSoT).
//!
//! Mirrors the pattern of `crate::track::codec` but for the spec document schema.

use domain::{
    DomainStateEntry, SignalCounts, SpecDocument, SpecRequirement, SpecScope, SpecSection,
    SpecValidationError,
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
}

/// DTO for a single requirement (text + provenance sources).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SpecRequirementDto {
    pub text: String,
    #[serde(default)]
    pub sources: Vec<String>,
}

/// DTO for a domain state entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DomainStateEntryDto {
    pub name: String,
    #[serde(default)]
    pub description: String,
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

    let acceptance_criteria = dto
        .acceptance_criteria
        .into_iter()
        .map(requirement_from_dto)
        .collect::<Result<Vec<_>, _>>()?;

    let additional_sections =
        dto.additional_sections.into_iter().map(section_from_dto).collect::<Result<Vec<_>, _>>()?;

    let signals = dto.signals.map(signal_counts_from_dto);

    let doc = SpecDocument::new(
        dto.title,
        dto.status,
        dto.version,
        dto.goal,
        scope,
        constraints,
        domain_states,
        acceptance_criteria,
        additional_sections,
        dto.related_conventions,
        signals,
    )?;

    Ok(doc)
}

fn requirement_from_dto(dto: SpecRequirementDto) -> Result<SpecRequirement, SpecValidationError> {
    SpecRequirement::new(dto.text, dto.sources)
}

fn domain_state_from_dto(
    dto: DomainStateEntryDto,
) -> Result<DomainStateEntry, SpecValidationError> {
    DomainStateEntry::new(dto.name, dto.description)
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
pub fn encode(doc: &SpecDocument) -> Result<String, SpecCodecError> {
    let dto = spec_document_to_dto(doc);
    Ok(serde_json::to_string_pretty(&dto)?)
}

fn spec_document_to_dto(doc: &SpecDocument) -> SpecDocumentDto {
    SpecDocumentDto {
        schema_version: 1,
        status: doc.status().to_owned(),
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
    }
}

fn requirement_to_dto(req: &SpecRequirement) -> SpecRequirementDto {
    SpecRequirementDto { text: req.text().to_owned(), sources: req.sources().to_vec() }
}

fn domain_state_to_dto(entry: &DomainStateEntry) -> DomainStateEntryDto {
    DomainStateEntryDto {
        name: entry.name().to_owned(),
        description: entry.description().to_owned(),
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
        assert_eq!(doc.status(), "draft");
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
        let json = r#"{"schema_version":1,"status":"s","version":"1","title":"T","scope":{"in_scope":[],"out_of_scope":[]}}"#;
        let doc = decode(json).unwrap();
        assert!(doc.goal().is_empty());
    }

    #[test]
    fn test_decode_with_null_signals_gives_none() {
        let json = r#"{"schema_version":1,"status":"s","version":"1","title":"T","scope":{"in_scope":[],"out_of_scope":[]},"signals":null}"#;
        // null is not the same as absent — serde(default) + skip_serializing_if handles absent,
        // but explicit null must also be tolerated. Using Option<> on the DTO absorbs null as None.
        let doc = decode(json).unwrap();
        assert!(doc.signals().is_none());
    }

    #[test]
    fn test_decode_additional_sections_defaults_to_empty() {
        let json = r#"{"schema_version":1,"status":"s","version":"1","title":"T","scope":{"in_scope":[],"out_of_scope":[]}}"#;
        let doc = decode(json).unwrap();
        assert!(doc.additional_sections().is_empty());
    }

    #[test]
    fn test_decode_related_conventions_defaults_to_empty() {
        let json = r#"{"schema_version":1,"status":"s","version":"1","title":"T","scope":{"in_scope":[],"out_of_scope":[]}}"#;
        let doc = decode(json).unwrap();
        assert!(doc.related_conventions().is_empty());
    }

    #[test]
    fn test_decode_requirement_without_sources_defaults_to_empty() {
        let json = r#"{
          "schema_version": 1, "status": "s", "version": "1", "title": "T",
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
        let json = r#"{"schema_version":2,"status":"s","version":"1","title":"T","scope":{"in_scope":[],"out_of_scope":[]}}"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, SpecCodecError::UnsupportedSchemaVersion(2)));
    }

    #[test]
    fn test_decode_with_schema_version_zero_returns_error() {
        let json = r#"{"schema_version":0,"status":"s","version":"1","title":"T","scope":{"in_scope":[],"out_of_scope":[]}}"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, SpecCodecError::UnsupportedSchemaVersion(0)));
    }

    // --- decode: domain validation errors ---

    #[test]
    fn test_decode_with_empty_title_returns_validation_error() {
        let json = r#"{"schema_version":1,"status":"s","version":"1","title":"","scope":{"in_scope":[],"out_of_scope":[]}}"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, SpecCodecError::Validation(SpecValidationError::EmptyTitle)));
    }

    #[test]
    fn test_decode_with_empty_status_returns_validation_error() {
        let json = r#"{"schema_version":1,"status":"","version":"1","title":"T","scope":{"in_scope":[],"out_of_scope":[]}}"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, SpecCodecError::Validation(SpecValidationError::EmptyStatus)));
    }

    #[test]
    fn test_decode_with_empty_version_returns_validation_error() {
        let json = r#"{"schema_version":1,"status":"s","version":"","title":"T","scope":{"in_scope":[],"out_of_scope":[]}}"#;
        let err = decode(json).unwrap_err();
        assert!(matches!(err, SpecCodecError::Validation(SpecValidationError::EmptyVersion)));
    }

    #[test]
    fn test_decode_with_empty_requirement_text_returns_validation_error() {
        let json = r#"{
          "schema_version": 1, "status": "s", "version": "1", "title": "T",
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
          "schema_version": 1, "status": "s", "version": "1", "title": "T",
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
          "schema_version": 1, "status": "s", "version": "1", "title": "T",
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
            "draft",
            "1.0",
            vec![],
            SpecScope::new(vec![], vec![]),
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
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
            "s",
            "1",
            vec![],
            SpecScope::new(vec![], vec![]),
            vec![],
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
        let doc = SpecDocument::new(
            "T",
            "s",
            "1",
            vec![],
            SpecScope::new(vec![], vec![]),
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            Some(SignalCounts::new(5, 2, 1)),
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
            "s",
            "1",
            vec![],
            SpecScope::new(vec![], vec![]),
            vec![],
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
            "s",
            "1",
            vec![],
            SpecScope::new(vec![], vec![]),
            vec![],
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
          "schema_version": 1, "status": "active", "version": "2.0", "title": "States Test",
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
          "schema_version": 1, "status": "s", "version": "1", "title": "T",
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
}
