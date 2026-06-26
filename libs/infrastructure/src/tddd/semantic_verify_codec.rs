//! Serde codecs for semantic verify-cache artifacts (schema_version 1).
//!
//! Two artifact families are handled here:
//!
//! - **`spec-adr-verify-cache.json`** — Chain-1 (`spec.json` → ADR) frozen
//!   verdict pairs for one track, encoded/decoded by
//!   [`SpecAdrVerifyCacheDocumentCodec`].
//! - **`<layer>-catalogue-spec-verify-cache.json`** — Chain-2
//!   (`<layer>-types.json` → `spec.json`) frozen verdict pairs per layer,
//!   encoded/decoded by [`CatalogueSpecVerifyCacheDocumentCodec`].
//!
//! # Responsibility split
//!
//! - Encode (`fn encode`) accepts a domain aggregate root and emits a
//!   deterministic pretty-printed JSON string.
//! - Decode (`fn decode`) parses the JSON string back into the domain aggregate.
//!   Unknown fields at any nesting level are rejected via
//!   `#[serde(deny_unknown_fields)]` (fail-closed typed-deserialization).
//!
//! No filesystem I/O lives here. Callers handle `std::fs` and symlink guards.

use domain::plan_ref::SpecElementId;
use domain::tddd::layer_id::LayerId;
use domain::tddd::semantic_verify::{
    AdrDecisionRef, CatalogueEntryKey, CatalogueEntryRef, CatalogueSectionKey, SpecElementRef,
    SpecSectionKind, VerifyOriginRef,
};
use domain::{
    CatalogueSpecVerifyCacheDocument, ContentHash, EvidenceCitation, SemanticVerdict,
    SemanticVerifyEntry, SpecAdrVerifyCacheDocument, ValidationError,
};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Error type for the semantic verify-cache JSON codecs.
///
/// Covers JSON parse failures, unsupported `schema_version`, and validation
/// failures (e.g. malformed hex hash, empty citation) while preserving
/// fail-closed typed-deserialization behaviour.
#[derive(Debug)]
pub enum SemanticVerifyCodecError {
    /// The payload is not valid JSON or fails DTO deserialization (including
    /// `deny_unknown_fields` rejections at any nesting level).
    Json {
        /// Human-readable description of the JSON error.
        message: String,
    },
    /// `schema_version` is not the expected value.
    UnsupportedSchemaVersion {
        /// The version this codec was compiled for.
        expected: u32,
        /// The version present in the artifact.
        actual: u32,
    },
    /// A field value is semantically invalid (e.g. non-hex hash, empty
    /// citation, unrecognised `kind` tag).
    Validation {
        /// Human-readable description of the validation failure.
        message: String,
    },
}

impl std::fmt::Display for SemanticVerifyCodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json { message } => write!(f, "JSON error: {message}"),
            Self::UnsupportedSchemaVersion { expected, actual } => {
                write!(f, "unsupported schema_version: expected {expected}, got {actual}")
            }
            Self::Validation { message } => write!(f, "validation error: {message}"),
        }
    }
}

impl std::error::Error for SemanticVerifyCodecError {}

impl From<serde_json::Error> for SemanticVerifyCodecError {
    fn from(err: serde_json::Error) -> Self {
        Self::Json { message: err.to_string() }
    }
}

impl From<ValidationError> for SemanticVerifyCodecError {
    fn from(err: ValidationError) -> Self {
        Self::Validation { message: err.to_string() }
    }
}

// ---------------------------------------------------------------------------
// Schema version constant
// ---------------------------------------------------------------------------

const SEMANTIC_VERIFY_CACHE_SCHEMA_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// DTOs — private to this module
// ---------------------------------------------------------------------------

/// Wire format for [`SpecSectionKind`].
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum SpecSectionKindDto {
    Goal,
    InScope,
    OutOfScope,
    Constraint,
    AcceptanceCriteria,
}

/// Wire format for [`CatalogueSectionKey`].
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum CatalogueSectionKeyDto {
    Types,
    Traits,
    Functions,
}

/// Wire format for [`SpecElementRef`].
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SpecElementRefDto {
    section: SpecSectionKindDto,
    element_id: String,
    text_label: String,
}

/// Wire format for [`AdrDecisionRef`].
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdrDecisionRefDto {
    file_path: String,
    decision_id: String,
}

/// Wire format for [`CatalogueEntryRef`].
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogueEntryRefDto {
    file_path: String,
    section_key: CatalogueSectionKeyDto,
    entry_key: String,
}

/// Wire format for [`VerifyOriginRef`].
///
/// Uses adjacently-tagged encoding: `{ "kind": "spec_element", "data": { ... } }`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case", deny_unknown_fields)]
enum VerifyOriginRefDto {
    SpecElement(SpecElementRefDto),
    AdrDecision(AdrDecisionRefDto),
    CatalogueEntry(CatalogueEntryRefDto),
}

/// Wire format for [`SemanticVerdict`].
///
/// Uses internally-tagged `#[serde(tag = "kind")]` so JSON looks like:
///
/// ```json
/// { "kind": "pass", "citation": "..." }
/// { "kind": "fail", "reason": "..." }
/// { "kind": "pending" }
/// ```
///
/// `deny_unknown_fields` at every nesting level rejects unknown or misspelled
/// keys. An unknown `kind` value is rejected at the JSON deserialization level.
/// A `pass` variant without `citation` is rejected as a missing-field error.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum SemanticVerdictDto {
    /// The semantic claim is backed by the evidence.
    Pass {
        /// Non-empty verbatim quotation from the evidence that supports the
        /// claim.  Absence is a decode error — no citation-free passes.
        citation: String,
    },
    /// The semantic claim is not backed by the evidence.
    Fail {
        /// Human-readable description of the mismatch.
        reason: String,
    },
    /// The reviewer could not confirm or deny the claim.  Treated as Fail at
    /// the gate level.
    Pending,
}

/// Wire format for a single [`SemanticVerifyEntry`].
///
/// All fields are required; missing `claim_hash` / `evidence_hash` /
/// `verdict` / `claim_origin` / `evidence_origin` are decode errors.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SemanticVerifyEntryDto {
    /// Lowercase hex SHA-256 of the claim element.
    claim_hash: String,
    /// Lowercase hex SHA-256 of the evidence element.
    evidence_hash: String,
    /// Frozen verdict for this (claim, evidence) pair.
    verdict: SemanticVerdictDto,
    /// Origin reference for the claim.
    claim_origin: VerifyOriginRefDto,
    /// Origin reference for the evidence.
    evidence_origin: VerifyOriginRefDto,
}

/// Wire format for `spec-adr-verify-cache.json` (schema_version 1).
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SpecAdrVerifyCacheDocumentDto {
    schema_version: u32,
    entries: Vec<SemanticVerifyEntryDto>,
}

/// Wire format for `<layer>-catalogue-spec-verify-cache.json` (schema_version 1).
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogueSpecVerifyCacheDocumentDto {
    schema_version: u32,
    layer: String,
    entries: Vec<SemanticVerifyEntryDto>,
}

// ---------------------------------------------------------------------------
// DTO ↔ domain conversion helpers
// ---------------------------------------------------------------------------

/// Validates `schema_version` against the expected constant and maps each
/// entry DTO through [`entry_from_dto`].
///
/// This is the shared decode core used by both codec types; codec-specific
/// fields (e.g. `layer`) are handled by the individual callers after this
/// returns.
///
/// # Errors
///
/// - [`SemanticVerifyCodecError::UnsupportedSchemaVersion`] when
///   `schema_version` does not equal [`SEMANTIC_VERIFY_CACHE_SCHEMA_VERSION`].
/// - [`SemanticVerifyCodecError::Validation`] when any entry DTO contains a
///   malformed hash or invalid verdict data.
fn check_schema_version_and_collect_entries(
    schema_version: u32,
    entries: Vec<SemanticVerifyEntryDto>,
) -> Result<Vec<SemanticVerifyEntry>, SemanticVerifyCodecError> {
    if schema_version != SEMANTIC_VERIFY_CACHE_SCHEMA_VERSION {
        return Err(SemanticVerifyCodecError::UnsupportedSchemaVersion {
            expected: SEMANTIC_VERIFY_CACHE_SCHEMA_VERSION,
            actual: schema_version,
        });
    }
    entries.into_iter().map(entry_from_dto).collect::<Result<Vec<_>, SemanticVerifyCodecError>>()
}

fn verdict_to_dto(verdict: &SemanticVerdict) -> SemanticVerdictDto {
    match verdict {
        SemanticVerdict::Pass { citation } => {
            SemanticVerdictDto::Pass { citation: citation.as_str().to_owned() }
        }
        SemanticVerdict::Fail { reason } => SemanticVerdictDto::Fail { reason: reason.clone() },
        SemanticVerdict::Pending => SemanticVerdictDto::Pending,
    }
}

fn verdict_from_dto(dto: SemanticVerdictDto) -> Result<SemanticVerdict, SemanticVerifyCodecError> {
    match dto {
        SemanticVerdictDto::Pass { citation } => {
            let evidence_citation = EvidenceCitation::try_new(citation).map_err(|e| {
                SemanticVerifyCodecError::Validation {
                    message: format!("invalid citation in pass verdict: {e}"),
                }
            })?;
            Ok(SemanticVerdict::Pass { citation: evidence_citation })
        }
        SemanticVerdictDto::Fail { reason } => Ok(SemanticVerdict::Fail { reason }),
        SemanticVerdictDto::Pending => Ok(SemanticVerdict::Pending),
    }
}

fn section_kind_to_dto(kind: &SpecSectionKind) -> SpecSectionKindDto {
    match kind {
        SpecSectionKind::Goal => SpecSectionKindDto::Goal,
        SpecSectionKind::InScope => SpecSectionKindDto::InScope,
        SpecSectionKind::OutOfScope => SpecSectionKindDto::OutOfScope,
        SpecSectionKind::Constraint => SpecSectionKindDto::Constraint,
        SpecSectionKind::AcceptanceCriteria => SpecSectionKindDto::AcceptanceCriteria,
    }
}

fn section_kind_from_dto(dto: SpecSectionKindDto) -> SpecSectionKind {
    match dto {
        SpecSectionKindDto::Goal => SpecSectionKind::Goal,
        SpecSectionKindDto::InScope => SpecSectionKind::InScope,
        SpecSectionKindDto::OutOfScope => SpecSectionKind::OutOfScope,
        SpecSectionKindDto::Constraint => SpecSectionKind::Constraint,
        SpecSectionKindDto::AcceptanceCriteria => SpecSectionKind::AcceptanceCriteria,
    }
}

fn catalogue_section_key_to_dto(key: &CatalogueSectionKey) -> CatalogueSectionKeyDto {
    match key {
        CatalogueSectionKey::Types => CatalogueSectionKeyDto::Types,
        CatalogueSectionKey::Traits => CatalogueSectionKeyDto::Traits,
        CatalogueSectionKey::Functions => CatalogueSectionKeyDto::Functions,
    }
}

fn catalogue_section_key_from_dto(dto: CatalogueSectionKeyDto) -> CatalogueSectionKey {
    match dto {
        CatalogueSectionKeyDto::Types => CatalogueSectionKey::Types,
        CatalogueSectionKeyDto::Traits => CatalogueSectionKey::Traits,
        CatalogueSectionKeyDto::Functions => CatalogueSectionKey::Functions,
    }
}

fn origin_to_dto(origin: &VerifyOriginRef) -> VerifyOriginRefDto {
    match origin {
        VerifyOriginRef::SpecElement(r) => VerifyOriginRefDto::SpecElement(SpecElementRefDto {
            section: section_kind_to_dto(&r.section),
            element_id: r.element_id.as_ref().to_owned(),
            text_label: r.text_label.clone(),
        }),
        VerifyOriginRef::AdrDecision(r) => VerifyOriginRefDto::AdrDecision(AdrDecisionRefDto {
            file_path: r.file_path.clone(),
            decision_id: r.decision_id.clone(),
        }),
        VerifyOriginRef::CatalogueEntry(r) => {
            VerifyOriginRefDto::CatalogueEntry(CatalogueEntryRefDto {
                file_path: r.file_path.clone(),
                section_key: catalogue_section_key_to_dto(&r.section_key),
                entry_key: r.entry_key.as_str().to_owned(),
            })
        }
    }
}

fn origin_from_dto(dto: VerifyOriginRefDto) -> Result<VerifyOriginRef, SemanticVerifyCodecError> {
    match dto {
        VerifyOriginRefDto::SpecElement(r) => {
            let element_id = SpecElementId::try_new(r.element_id).map_err(|e| {
                SemanticVerifyCodecError::Validation {
                    message: format!("invalid spec element id in origin: {e}"),
                }
            })?;
            Ok(VerifyOriginRef::SpecElement(SpecElementRef::new(
                section_kind_from_dto(r.section),
                element_id,
                r.text_label,
            )))
        }
        VerifyOriginRefDto::AdrDecision(r) => {
            Ok(VerifyOriginRef::AdrDecision(AdrDecisionRef::new(r.file_path, r.decision_id)))
        }
        VerifyOriginRefDto::CatalogueEntry(r) => {
            let entry_key = CatalogueEntryKey::try_new(r.entry_key).map_err(|e| {
                SemanticVerifyCodecError::Validation {
                    message: format!("invalid catalogue entry key in origin: {e}"),
                }
            })?;
            Ok(VerifyOriginRef::CatalogueEntry(CatalogueEntryRef::new(
                r.file_path,
                catalogue_section_key_from_dto(r.section_key),
                entry_key,
            )))
        }
    }
}

fn entry_to_dto(entry: &SemanticVerifyEntry) -> SemanticVerifyEntryDto {
    SemanticVerifyEntryDto {
        claim_hash: entry.claim_hash.to_hex(),
        evidence_hash: entry.evidence_hash.to_hex(),
        verdict: verdict_to_dto(&entry.verdict),
        claim_origin: origin_to_dto(&entry.claim_origin),
        evidence_origin: origin_to_dto(&entry.evidence_origin),
    }
}

fn entry_from_dto(
    dto: SemanticVerifyEntryDto,
) -> Result<SemanticVerifyEntry, SemanticVerifyCodecError> {
    let claim_hash = ContentHash::try_from_hex(&dto.claim_hash)?;
    let evidence_hash = ContentHash::try_from_hex(&dto.evidence_hash)?;
    let verdict = verdict_from_dto(dto.verdict)?;
    let claim_origin = origin_from_dto(dto.claim_origin)?;
    let evidence_origin = origin_from_dto(dto.evidence_origin)?;
    Ok(SemanticVerifyEntry::new(claim_hash, evidence_hash, verdict, claim_origin, evidence_origin))
}

// ---------------------------------------------------------------------------
// SpecAdrVerifyCacheDocumentCodec
// ---------------------------------------------------------------------------

/// Stateless codec for `spec-adr-verify-cache.json`.
///
/// Encodes and decodes schema_version 1 documents through an internal DTO type
/// and returns typed fail-closed errors for malformed JSON, unsupported
/// versions, missing fields, or invalid domain values.
#[derive(Debug)]
pub struct SpecAdrVerifyCacheDocumentCodec;

impl SpecAdrVerifyCacheDocumentCodec {
    /// Encode a [`SpecAdrVerifyCacheDocument`] as a pretty-printed JSON string.
    ///
    /// Output is deterministic: no wall-clock fields, entry order preserved.
    ///
    /// # Errors
    ///
    /// Returns [`SemanticVerifyCodecError::Json`] if serialization fails
    /// (should not occur for well-formed domain values).
    pub fn encode(doc: &SpecAdrVerifyCacheDocument) -> Result<String, SemanticVerifyCodecError> {
        let dto = SpecAdrVerifyCacheDocumentDto {
            schema_version: SEMANTIC_VERIFY_CACHE_SCHEMA_VERSION,
            entries: doc.entries.iter().map(entry_to_dto).collect(),
        };
        Ok(serde_json::to_string_pretty(&dto)?)
    }

    /// Decode a `spec-adr-verify-cache.json` string into a
    /// [`SpecAdrVerifyCacheDocument`].
    ///
    /// # Errors
    ///
    /// - [`SemanticVerifyCodecError::Json`] when the input is not valid JSON
    ///   or contains unknown fields at any nesting level.
    /// - [`SemanticVerifyCodecError::UnsupportedSchemaVersion`] when
    ///   `schema_version != 1`.
    /// - [`SemanticVerifyCodecError::Validation`] when a hash is not a
    ///   canonical 64-character lowercase hex string, a `pass` citation is
    ///   empty or whitespace-only, or a verdict `kind` is unrecognised.
    pub fn decode(json: &str) -> Result<SpecAdrVerifyCacheDocument, SemanticVerifyCodecError> {
        let dto: SpecAdrVerifyCacheDocumentDto = serde_json::from_str(json)?;
        let entries = check_schema_version_and_collect_entries(dto.schema_version, dto.entries)?;
        Ok(SpecAdrVerifyCacheDocument::new(entries))
    }
}

// ---------------------------------------------------------------------------
// CatalogueSpecVerifyCacheDocumentCodec
// ---------------------------------------------------------------------------

/// Stateless codec for `<layer>-catalogue-spec-verify-cache.json`.
///
/// Encodes and decodes schema_version 1 documents through an internal DTO
/// type, preserving the `layer` field and failing closed on malformed or
/// semantically invalid artifact content.
#[derive(Debug)]
pub struct CatalogueSpecVerifyCacheDocumentCodec;

impl CatalogueSpecVerifyCacheDocumentCodec {
    /// Encode a [`CatalogueSpecVerifyCacheDocument`] as a pretty-printed JSON
    /// string.
    ///
    /// Output is deterministic: no wall-clock fields, entry order preserved.
    ///
    /// # Errors
    ///
    /// Returns [`SemanticVerifyCodecError::Json`] if serialization fails
    /// (should not occur for well-formed domain values).
    pub fn encode(
        doc: &CatalogueSpecVerifyCacheDocument,
    ) -> Result<String, SemanticVerifyCodecError> {
        let dto = CatalogueSpecVerifyCacheDocumentDto {
            schema_version: SEMANTIC_VERIFY_CACHE_SCHEMA_VERSION,
            layer: doc.layer.as_ref().to_owned(),
            entries: doc.entries.iter().map(entry_to_dto).collect(),
        };
        Ok(serde_json::to_string_pretty(&dto)?)
    }

    /// Decode a `<layer>-catalogue-spec-verify-cache.json` string into a
    /// [`CatalogueSpecVerifyCacheDocument`].
    ///
    /// # Errors
    ///
    /// - [`SemanticVerifyCodecError::Json`] when the input is not valid JSON
    ///   or contains unknown fields at any nesting level.
    /// - [`SemanticVerifyCodecError::UnsupportedSchemaVersion`] when
    ///   `schema_version != 1`.
    /// - [`SemanticVerifyCodecError::Validation`] when the `layer` value is
    ///   not a valid [`LayerId`], a hash is malformed, a `pass` citation is
    ///   empty, or a verdict `kind` is unrecognised.
    pub fn decode(
        json: &str,
    ) -> Result<CatalogueSpecVerifyCacheDocument, SemanticVerifyCodecError> {
        let dto: CatalogueSpecVerifyCacheDocumentDto = serde_json::from_str(json)?;
        // Validate schema_version before converting layer so that an unsupported
        // version is always reported as UnsupportedSchemaVersion rather than
        // being masked by a subsequent Validation error on the layer field.
        let entries = check_schema_version_and_collect_entries(dto.schema_version, dto.entries)?;
        let layer = LayerId::try_new(dto.layer)?;
        Ok(CatalogueSpecVerifyCacheDocument::new(layer, entries))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::super::test_support::hex_pattern;
    use super::*;

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn make_hash(byte: u8) -> ContentHash {
        ContentHash::from_bytes([byte; 32])
    }

    fn make_citation() -> EvidenceCitation {
        EvidenceCitation::try_new("The spec states X.".to_string()).unwrap()
    }

    fn make_spec_element_origin() -> VerifyOriginRef {
        use domain::plan_ref::SpecElementId;
        VerifyOriginRef::SpecElement(SpecElementRef::new(
            SpecSectionKind::Goal,
            SpecElementId::try_new("GO-01".to_owned()).unwrap(),
            "test spec element".to_owned(),
        ))
    }

    fn make_adr_decision_origin() -> VerifyOriginRef {
        VerifyOriginRef::AdrDecision(AdrDecisionRef::new(
            "knowledge/adr/test.md".to_owned(),
            "D1".to_owned(),
        ))
    }

    fn make_pass_entry(claim: u8, evidence: u8) -> SemanticVerifyEntry {
        SemanticVerifyEntry::new(
            make_hash(claim),
            make_hash(evidence),
            SemanticVerdict::Pass { citation: make_citation() },
            make_spec_element_origin(),
            make_adr_decision_origin(),
        )
    }

    fn make_fail_entry(claim: u8, evidence: u8) -> SemanticVerifyEntry {
        SemanticVerifyEntry::new(
            make_hash(claim),
            make_hash(evidence),
            SemanticVerdict::Fail { reason: "mismatch".to_string() },
            make_spec_element_origin(),
            make_adr_decision_origin(),
        )
    }

    fn make_pending_entry(claim: u8, evidence: u8) -> SemanticVerifyEntry {
        SemanticVerifyEntry::new(
            make_hash(claim),
            make_hash(evidence),
            SemanticVerdict::Pending,
            make_spec_element_origin(),
            make_adr_decision_origin(),
        )
    }

    fn sample_spec_adr_doc() -> SpecAdrVerifyCacheDocument {
        SpecAdrVerifyCacheDocument::new(vec![
            make_pass_entry(0x01, 0x02),
            make_fail_entry(0x03, 0x04),
            make_pending_entry(0x05, 0x06),
        ])
    }

    fn sample_catalogue_spec_doc() -> CatalogueSpecVerifyCacheDocument {
        let layer = LayerId::try_new("domain".to_string()).unwrap();
        CatalogueSpecVerifyCacheDocument::new(
            layer,
            vec![make_pass_entry(0x0a, 0x0b), make_fail_entry(0x0c, 0x0d)],
        )
    }

    // ── SpecAdrVerifyCacheDocumentCodec — encode/decode round-trip ────────

    #[test]
    fn spec_adr_encode_includes_schema_version_and_entries() {
        let doc = sample_spec_adr_doc();
        let json = SpecAdrVerifyCacheDocumentCodec::encode(&doc).unwrap();
        assert!(json.contains("\"schema_version\": 1"), "must contain schema_version");
        assert!(json.contains("\"entries\""), "must contain entries");
    }

    #[test]
    fn spec_adr_encode_decode_roundtrip_preserves_document() {
        let original = sample_spec_adr_doc();
        let json = SpecAdrVerifyCacheDocumentCodec::encode(&original).unwrap();
        let decoded = SpecAdrVerifyCacheDocumentCodec::decode(&json).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn spec_adr_encode_is_deterministic() {
        let doc = sample_spec_adr_doc();
        let a = SpecAdrVerifyCacheDocumentCodec::encode(&doc).unwrap();
        let b = SpecAdrVerifyCacheDocumentCodec::encode(&doc).unwrap();
        assert_eq!(a, b, "encode must be deterministic");
    }

    // ── CatalogueSpecVerifyCacheDocumentCodec — encode/decode round-trip ─

    #[test]
    fn catalogue_spec_encode_includes_schema_version_layer_and_entries() {
        let doc = sample_catalogue_spec_doc();
        let json = CatalogueSpecVerifyCacheDocumentCodec::encode(&doc).unwrap();
        assert!(json.contains("\"schema_version\": 1"), "must contain schema_version");
        assert!(json.contains("\"layer\""), "must contain layer field");
        assert!(json.contains("\"entries\""), "must contain entries");
        assert!(json.contains("\"domain\""), "must contain layer value");
    }

    #[test]
    fn catalogue_spec_encode_decode_roundtrip_preserves_document() {
        let original = sample_catalogue_spec_doc();
        let json = CatalogueSpecVerifyCacheDocumentCodec::encode(&original).unwrap();
        let decoded = CatalogueSpecVerifyCacheDocumentCodec::decode(&json).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn catalogue_spec_encode_is_deterministic() {
        let doc = sample_catalogue_spec_doc();
        let a = CatalogueSpecVerifyCacheDocumentCodec::encode(&doc).unwrap();
        let b = CatalogueSpecVerifyCacheDocumentCodec::encode(&doc).unwrap();
        assert_eq!(a, b, "encode must be deterministic");
    }

    // ── SemanticVerdict kind tags ─────────────────────────────────────────

    #[test]
    fn pass_kind_encodes_as_pass_with_citation() {
        let doc = SpecAdrVerifyCacheDocument::new(vec![make_pass_entry(0x01, 0x02)]);
        let json = SpecAdrVerifyCacheDocumentCodec::encode(&doc).unwrap();
        assert!(json.contains("\"kind\": \"pass\""), "pass verdict must encode as kind=pass");
        assert!(json.contains("\"citation\""), "pass verdict must include citation field");
    }

    #[test]
    fn fail_kind_encodes_as_fail_with_reason() {
        let doc = SpecAdrVerifyCacheDocument::new(vec![make_fail_entry(0x01, 0x02)]);
        let json = SpecAdrVerifyCacheDocumentCodec::encode(&doc).unwrap();
        assert!(json.contains("\"kind\": \"fail\""), "fail verdict must encode as kind=fail");
        assert!(json.contains("\"reason\""), "fail verdict must include reason field");
    }

    #[test]
    fn pending_kind_encodes_as_pending() {
        let doc = SpecAdrVerifyCacheDocument::new(vec![make_pending_entry(0x01, 0x02)]);
        let json = SpecAdrVerifyCacheDocumentCodec::encode(&doc).unwrap();
        assert!(
            json.contains("\"kind\": \"pending\""),
            "pending verdict must encode as kind=pending"
        );
    }

    #[test]
    fn pending_kind_decodes_to_semantic_verdict_pending() {
        let json = format!(
            r#"{{
              "schema_version": 1,
              "entries": [
                {{
                  "claim_hash": "{}",
                  "evidence_hash": "{}",
                  "verdict": {{ "kind": "pending" }},
                  "claim_origin": {{"kind": "spec_element", "data": {{"section": "goal", "element_id": "GO-01", "text_label": "t"}}}},
                  "evidence_origin": {{"kind": "adr_decision", "data": {{"file_path": "a.md", "decision_id": "D1"}}}}
                }}
              ]
            }}"#,
            hex_pattern(0x01),
            hex_pattern(0x02)
        );
        let doc = SpecAdrVerifyCacheDocumentCodec::decode(&json).unwrap();
        assert_eq!(doc.entries.len(), 1);
        assert!(matches!(doc.entries[0].verdict, SemanticVerdict::Pending));
    }

    // ── Fail-closed: unknown kind ─────────────────────────────────────────

    #[test]
    fn decode_rejects_unknown_verdict_kind() {
        let json = format!(
            r#"{{
              "schema_version": 1,
              "entries": [
                {{
                  "claim_hash": "{}",
                  "evidence_hash": "{}",
                  "verdict": {{ "kind": "maybe" }},
                  "claim_origin": {{"kind": "spec_element", "data": {{"section": "goal", "element_id": "GO-01", "text_label": "t"}}}},
                  "evidence_origin": {{"kind": "adr_decision", "data": {{"file_path": "a.md", "decision_id": "D1"}}}}
                }}
              ]
            }}"#,
            hex_pattern(0x01),
            hex_pattern(0x02)
        );
        let err = SpecAdrVerifyCacheDocumentCodec::decode(&json).unwrap_err();
        assert!(matches!(err, SemanticVerifyCodecError::Json { .. }));
    }

    // ── Fail-closed: citation missing in pass ──────────────────────────────

    #[test]
    fn decode_rejects_pass_without_citation() {
        let json = format!(
            r#"{{
              "schema_version": 1,
              "entries": [
                {{
                  "claim_hash": "{}",
                  "evidence_hash": "{}",
                  "verdict": {{ "kind": "pass" }},
                  "claim_origin": {{"kind": "spec_element", "data": {{"section": "goal", "element_id": "GO-01", "text_label": "t"}}}},
                  "evidence_origin": {{"kind": "adr_decision", "data": {{"file_path": "a.md", "decision_id": "D1"}}}}
                }}
              ]
            }}"#,
            hex_pattern(0x01),
            hex_pattern(0x02)
        );
        let err = SpecAdrVerifyCacheDocumentCodec::decode(&json).unwrap_err();
        assert!(
            matches!(err, SemanticVerifyCodecError::Json { .. }),
            "missing citation must be a JSON decode error, got: {err}"
        );
    }

    // ── Fail-closed: empty citation in pass ────────────────────────────────

    #[test]
    fn decode_rejects_empty_citation_in_pass() {
        let json = format!(
            r#"{{
              "schema_version": 1,
              "entries": [
                {{
                  "claim_hash": "{}",
                  "evidence_hash": "{}",
                  "verdict": {{ "kind": "pass", "citation": "" }},
                  "claim_origin": {{"kind": "spec_element", "data": {{"section": "goal", "element_id": "GO-01", "text_label": "t"}}}},
                  "evidence_origin": {{"kind": "adr_decision", "data": {{"file_path": "a.md", "decision_id": "D1"}}}}
                }}
              ]
            }}"#,
            hex_pattern(0x01),
            hex_pattern(0x02)
        );
        let err = SpecAdrVerifyCacheDocumentCodec::decode(&json).unwrap_err();
        assert!(
            matches!(err, SemanticVerifyCodecError::Validation { .. }),
            "empty citation must be a Validation error, got: {err}"
        );
    }

    // ── Fail-closed: schema version ───────────────────────────────────────

    #[test]
    fn spec_adr_decode_rejects_unsupported_schema_version() {
        let json = r#"{"schema_version": 2, "entries": []}"#;
        let err = SpecAdrVerifyCacheDocumentCodec::decode(json).unwrap_err();
        assert!(matches!(
            err,
            SemanticVerifyCodecError::UnsupportedSchemaVersion { expected: 1, actual: 2 }
        ));
    }

    #[test]
    fn catalogue_spec_decode_rejects_unsupported_schema_version() {
        let json = r#"{"schema_version": 99, "layer": "domain", "entries": []}"#;
        let err = CatalogueSpecVerifyCacheDocumentCodec::decode(json).unwrap_err();
        assert!(matches!(
            err,
            SemanticVerifyCodecError::UnsupportedSchemaVersion { expected: 1, actual: 99 }
        ));
    }

    // ── Fail-closed: unknown fields ───────────────────────────────────────

    #[test]
    fn spec_adr_decode_rejects_unknown_top_level_field() {
        let json = r#"{"schema_version": 1, "entries": [], "extra": "bad"}"#;
        let err = SpecAdrVerifyCacheDocumentCodec::decode(json).unwrap_err();
        assert!(matches!(err, SemanticVerifyCodecError::Json { .. }));
    }

    #[test]
    fn spec_adr_decode_rejects_unknown_entry_field() {
        let json = format!(
            r#"{{
              "schema_version": 1,
              "entries": [
                {{
                  "claim_hash": "{}",
                  "evidence_hash": "{}",
                  "verdict": {{ "kind": "pending" }},
                  "claim_origin": {{"kind": "spec_element", "data": {{"section": "goal", "element_id": "GO-01", "text_label": "t"}}}},
                  "evidence_origin": {{"kind": "adr_decision", "data": {{"file_path": "a.md", "decision_id": "D1"}}}},
                  "extra": "not allowed"
                }}
              ]
            }}"#,
            hex_pattern(0x01),
            hex_pattern(0x02)
        );
        let err = SpecAdrVerifyCacheDocumentCodec::decode(&json).unwrap_err();
        assert!(matches!(err, SemanticVerifyCodecError::Json { .. }));
    }

    #[test]
    fn catalogue_spec_decode_rejects_unknown_top_level_field() {
        let json = r#"{"schema_version": 1, "layer": "domain", "entries": [], "unknown": true}"#;
        let err = CatalogueSpecVerifyCacheDocumentCodec::decode(json).unwrap_err();
        assert!(matches!(err, SemanticVerifyCodecError::Json { .. }));
    }

    // ── Fail-closed: malformed hashes ────────────────────────────────────

    #[test]
    fn spec_adr_decode_rejects_malformed_claim_hash() {
        let json = format!(
            r#"{{
              "schema_version": 1,
              "entries": [
                {{
                  "claim_hash": "not-hex",
                  "evidence_hash": "{}",
                  "verdict": {{ "kind": "pending" }},
                  "claim_origin": {{"kind": "spec_element", "data": {{"section": "goal", "element_id": "GO-01", "text_label": "t"}}}},
                  "evidence_origin": {{"kind": "adr_decision", "data": {{"file_path": "a.md", "decision_id": "D1"}}}}
                }}
              ]
            }}"#,
            hex_pattern(0x02)
        );
        let err = SpecAdrVerifyCacheDocumentCodec::decode(&json).unwrap_err();
        assert!(matches!(err, SemanticVerifyCodecError::Validation { .. }));
    }

    #[test]
    fn spec_adr_decode_rejects_malformed_evidence_hash() {
        let json = format!(
            r#"{{
              "schema_version": 1,
              "entries": [
                {{
                  "claim_hash": "{}",
                  "evidence_hash": "UPPERCASE",
                  "verdict": {{ "kind": "pending" }},
                  "claim_origin": {{"kind": "spec_element", "data": {{"section": "goal", "element_id": "GO-01", "text_label": "t"}}}},
                  "evidence_origin": {{"kind": "adr_decision", "data": {{"file_path": "a.md", "decision_id": "D1"}}}}
                }}
              ]
            }}"#,
            hex_pattern(0x01)
        );
        let err = SpecAdrVerifyCacheDocumentCodec::decode(&json).unwrap_err();
        assert!(matches!(err, SemanticVerifyCodecError::Validation { .. }));
    }

    // ── Fail-closed: invalid layer id in catalogue-spec codec ────────────

    #[test]
    fn catalogue_spec_decode_rejects_invalid_layer_id() {
        let json = r#"{"schema_version": 1, "layer": "", "entries": []}"#;
        let err = CatalogueSpecVerifyCacheDocumentCodec::decode(json).unwrap_err();
        assert!(matches!(err, SemanticVerifyCodecError::Validation { .. }));
    }

    // ── Fail-closed: missing required fields ─────────────────────────────

    #[test]
    fn spec_adr_decode_rejects_missing_schema_version() {
        let json = r#"{"entries": []}"#;
        let err = SpecAdrVerifyCacheDocumentCodec::decode(json).unwrap_err();
        assert!(matches!(err, SemanticVerifyCodecError::Json { .. }));
    }

    #[test]
    fn spec_adr_decode_rejects_missing_entries_array() {
        let json = r#"{"schema_version": 1}"#;
        let err = SpecAdrVerifyCacheDocumentCodec::decode(json).unwrap_err();
        assert!(matches!(err, SemanticVerifyCodecError::Json { .. }));
    }

    #[test]
    fn catalogue_spec_decode_rejects_missing_layer() {
        let json = r#"{"schema_version": 1, "entries": []}"#;
        let err = CatalogueSpecVerifyCacheDocumentCodec::decode(json).unwrap_err();
        assert!(matches!(err, SemanticVerifyCodecError::Json { .. }));
    }

    // ── VerifyOriginRef round-trip tests ─────────────────────────────────────

    #[test]
    fn origin_spec_element_roundtrip_through_codec() {
        use domain::plan_ref::SpecElementId;
        let origin = VerifyOriginRef::SpecElement(SpecElementRef::new(
            SpecSectionKind::InScope,
            SpecElementId::try_new("IN-07".to_owned()).unwrap(),
            "some spec text".to_owned(),
        ));
        let adr_origin = make_adr_decision_origin();
        let entry = SemanticVerifyEntry::new(
            make_hash(0x10),
            make_hash(0x11),
            SemanticVerdict::Pending,
            origin.clone(),
            adr_origin,
        );
        let doc = SpecAdrVerifyCacheDocument::new(vec![entry]);
        let json = SpecAdrVerifyCacheDocumentCodec::encode(&doc).unwrap();
        let decoded = SpecAdrVerifyCacheDocumentCodec::decode(&json).unwrap();
        assert_eq!(decoded.entries[0].claim_origin, origin);
    }

    #[test]
    fn origin_adr_decision_roundtrip_through_codec() {
        let origin = VerifyOriginRef::AdrDecision(AdrDecisionRef::new(
            "knowledge/adr/some-decision.md".to_owned(),
            "D3".to_owned(),
        ));
        let spec_origin = make_spec_element_origin();
        let entry = SemanticVerifyEntry::new(
            make_hash(0x12),
            make_hash(0x13),
            SemanticVerdict::Pending,
            spec_origin,
            origin.clone(),
        );
        let doc = SpecAdrVerifyCacheDocument::new(vec![entry]);
        let json = SpecAdrVerifyCacheDocumentCodec::encode(&doc).unwrap();
        let decoded = SpecAdrVerifyCacheDocumentCodec::decode(&json).unwrap();
        assert_eq!(decoded.entries[0].evidence_origin, origin);
    }

    #[test]
    fn origin_catalogue_entry_roundtrip_through_codec() {
        let entry_key = CatalogueEntryKey::try_new("MyRepository".to_owned()).unwrap();
        let origin = VerifyOriginRef::CatalogueEntry(CatalogueEntryRef::new(
            "track/items/foo/domain-types.json".to_owned(),
            CatalogueSectionKey::Traits,
            entry_key,
        ));
        let spec_origin = make_spec_element_origin();
        let entry = SemanticVerifyEntry::new(
            make_hash(0x14),
            make_hash(0x15),
            SemanticVerdict::Pending,
            origin.clone(),
            spec_origin,
        );
        let layer = domain::tddd::layer_id::LayerId::try_new("domain".to_owned()).unwrap();
        let doc = CatalogueSpecVerifyCacheDocument::new(layer, vec![entry]);
        let json = CatalogueSpecVerifyCacheDocumentCodec::encode(&doc).unwrap();
        let decoded = CatalogueSpecVerifyCacheDocumentCodec::decode(&json).unwrap();
        assert_eq!(decoded.entries[0].claim_origin, origin);
    }

    // ── Fail-closed: missing origin fields ────────────────────────────────────

    #[test]
    fn decode_rejects_entry_missing_claim_origin() {
        let json = format!(
            r#"{{
              "schema_version": 1,
              "entries": [
                {{
                  "claim_hash": "{}",
                  "evidence_hash": "{}",
                  "verdict": {{ "kind": "pending" }}
                }}
              ]
            }}"#,
            hex_pattern(0x01),
            hex_pattern(0x02)
        );
        let err = SpecAdrVerifyCacheDocumentCodec::decode(&json).unwrap_err();
        assert!(
            matches!(err, SemanticVerifyCodecError::Json { .. }),
            "entry without claim_origin must be a JSON decode error, got: {err}"
        );
    }

    #[test]
    fn decode_rejects_entry_missing_evidence_origin() {
        let json = format!(
            r#"{{
              "schema_version": 1,
              "entries": [
                {{
                  "claim_hash": "{}",
                  "evidence_hash": "{}",
                  "verdict": {{ "kind": "pending" }},
                  "claim_origin": {{
                    "kind": "adr_decision",
                    "data": {{ "file_path": "adr.md", "decision_id": "D1" }}
                  }}
                }}
              ]
            }}"#,
            hex_pattern(0x01),
            hex_pattern(0x02)
        );
        let err = SpecAdrVerifyCacheDocumentCodec::decode(&json).unwrap_err();
        assert!(
            matches!(err, SemanticVerifyCodecError::Json { .. }),
            "entry without evidence_origin must be a JSON decode error, got: {err}"
        );
    }
}
