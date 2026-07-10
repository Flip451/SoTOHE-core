//! JSON codec for the obligation-fulfillment verdict cache.
//!
//! Serialises the domain [`ObligationFulfillmentCacheDocument`] to a track-scoped
//! `obligation-fulfillment-cache.json` and validates it back (IN-09 / AC-06 /
//! CN-04). Each entry freezes a verdict against the three-hash cache key
//! (bound-tests-set hash, entry-declaration hash, anchor-text hash, ADR D6): the
//! hashes are serialised as lowercase hex and any change to a component produces a
//! different key. Each entry also persists its verifier-prompt fingerprint; an
//! absent legacy fingerprint remains readable but is fail-closed by cache readers.
//! Recovery is only via re-evaluation (CN-04). A passing verdict structurally
//! carries its evidence citation, so "pass without citation" cannot be represented.

use std::path::PathBuf;

use domain::tddd::test_obligation::errors::VerifyCacheError;
use domain::tddd::test_obligation::hashes::{
    AnchorTextHash, BoundTestsSetHash, DeclarationHash, VerifierPromptFingerprint,
};
use domain::tddd::test_obligation::ids::DiagnosticMessage;
use domain::tddd::test_obligation::ports::ObligationFulfillmentCachePort;
use domain::tddd::test_obligation::verdict::{
    ObligationFulfillmentCacheDocument, ObligationFulfillmentCacheEntry,
    ObligationFulfillmentCacheKey, ObligationFulfillmentVerdict,
};
use domain::tddd::test_obligation::vocab::FulfillmentFailCategory;
use domain::{EvidenceCitation, TrackId};
use serde::{Deserialize, Serialize};

use crate::test_obligation::bindings_codec::{
    TestObligationEdgeIdDto, edge_id_from_dto, edge_id_to_dto,
};
use crate::test_obligation::obligations_codec::{
    TestObligationIdDto, obligation_id_from_dto, obligation_id_to_dto,
};
use crate::test_obligation::{diagnostic, reject_symlinked_items_root};
use crate::track::symlink_guard::reject_symlinks_below;

/// Artifact filename for the obligation-fulfillment verdict cache.
const FULFILLMENT_CACHE_ARTIFACT: &str = "obligation-fulfillment-cache.json";

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

/// Serde DTO for [`FulfillmentFailCategory`] (IN-12 / AC-08).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FulfillmentFailCategoryDto {
    /// A test asserts the opposite of the anchor's promise.
    Contradiction,
    /// A test cites the anchor but verifies unrelated content.
    Substitution,
    /// The anchor's central behavior is left unverified.
    CentralUnverified,
}

/// Serde DTO for [`ObligationFulfillmentVerdict`] (IN-09 / AC-06 / AC-08).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ObligationFulfillmentVerdictDto {
    /// The bound tests fulfill the obligation; carries the evidence citation.
    Fulfilled {
        /// Verbatim quotation from the tests that fulfills the obligation.
        citation: String,
    },
    /// The bound tests fail to fulfill the obligation.
    Fail {
        /// Which of the three fail categories the failure falls into.
        category: FulfillmentFailCategoryDto,
        /// Human-readable description of the failure.
        reason: String,
    },
    /// The reviewer could not confirm fulfillment; treated as fail at the gate.
    Pending,
}

/// Wire form of the three-component fulfillment cache key (IN-09 / CN-04).
///
/// Private helper: the domain [`ObligationFulfillmentCacheKey`] has no dedicated
/// DTO in the type contract, so its three hashes are carried inline as lowercase
/// hex strings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct FulfillmentCacheKeyWire {
    bound_tests_set_hash: String,
    declaration_hash: String,
    anchor_text_hash: String,
}

/// Serde DTO for [`ObligationFulfillmentCacheEntry`] (IN-09).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObligationFulfillmentCacheEntryDto {
    edge_id: TestObligationEdgeIdDto,
    obligation_id: TestObligationIdDto,
    key: FulfillmentCacheKeyWire,
    verdict: ObligationFulfillmentVerdictDto,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    verifier_fingerprint: Option<String>,
}

/// Serde DTO for [`ObligationFulfillmentCacheDocument`] (IN-09).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObligationFulfillmentCacheDocumentDto {
    track_id: String,
    entries: Vec<ObligationFulfillmentCacheEntryDto>,
}

// ---------------------------------------------------------------------------
// Codec adapter
// ---------------------------------------------------------------------------

/// JSON codec adapter for [`ObligationFulfillmentCachePort`] (IN-09 / AC-06 / CN-04).
#[derive(Debug, Clone)]
pub struct JsonObligationFulfillmentCacheCodec {
    items_dir: PathBuf,
}

impl JsonObligationFulfillmentCacheCodec {
    /// Creates a codec that resolves cache artifacts under `items_dir` (the track
    /// items root, e.g. `track/items`).
    #[must_use]
    pub fn new(items_dir: PathBuf) -> Self {
        Self { items_dir }
    }

    fn artifact_path(&self, track_id: &TrackId) -> PathBuf {
        self.items_dir.join(track_id.as_ref()).join(FULFILLMENT_CACHE_ARTIFACT)
    }
}

impl ObligationFulfillmentCachePort for JsonObligationFulfillmentCacheCodec {
    fn load(
        &self,
        track_id: &TrackId,
    ) -> Result<Option<ObligationFulfillmentCacheDocument>, VerifyCacheError> {
        reject_symlinked_items_root(&self.items_dir).map_err(|source| {
            VerifyCacheError::Io(diagnostic(&format!(
                "refusing to read fulfillment cache under {}: {source}",
                self.items_dir.display()
            )))
        })?;
        let path = self.artifact_path(track_id);
        match reject_symlinks_below(&path, &self.items_dir) {
            Ok(true) => {}
            Ok(false) => return Ok(None),
            Err(source) => {
                return Err(VerifyCacheError::Io(diagnostic(&format!(
                    "refusing to read fulfillment cache {}: {source}",
                    path.display()
                ))));
            }
        }
        let content = std::fs::read_to_string(&path).map_err(|e| {
            VerifyCacheError::Io(diagnostic(&format!(
                "failed to read fulfillment cache {}: {e}",
                path.display()
            )))
        })?;
        let dto: ObligationFulfillmentCacheDocumentDto = serde_json::from_str(&content)
            .map_err(|e| VerifyCacheError::MalformedJson(diagnostic(&e.to_string())))?;
        let doc = document_from_dto(dto)?;
        // Fail closed when the on-disk cache was copied from another track: a
        // matching filename is not proof of matching content, and the caller
        // trusts a `load(track_id)` result to describe exactly that track.
        if doc.track_id() != track_id {
            return Err(VerifyCacheError::MalformedJson(diagnostic(&format!(
                "fulfillment cache track id mismatch: requested '{}', got '{}'",
                track_id.as_ref(),
                doc.track_id().as_ref()
            ))));
        }
        Ok(Some(doc))
    }

    fn save(&self, doc: &ObligationFulfillmentCacheDocument) -> Result<(), DiagnosticMessage> {
        reject_symlinked_items_root(&self.items_dir).map_err(|source| {
            diagnostic(&format!(
                "refusing to write fulfillment cache under {}: {source}",
                self.items_dir.display()
            ))
        })?;
        let path = self.artifact_path(doc.track_id());
        let Some(parent) = path.parent() else {
            return Err(diagnostic(&format!(
                "fulfillment cache path {} has no parent directory",
                path.display()
            )));
        };
        std::fs::create_dir_all(parent).map_err(|e| {
            diagnostic(&format!("failed to create track directory {}: {e}", parent.display()))
        })?;
        reject_symlinked_items_root(&self.items_dir).map_err(|source| {
            diagnostic(&format!(
                "refusing to write fulfillment cache under {}: {source}",
                self.items_dir.display()
            ))
        })?;
        if let Err(source) = reject_symlinks_below(&path, &self.items_dir) {
            return Err(diagnostic(&format!(
                "refusing to write fulfillment cache {}: {source}",
                path.display()
            )));
        }
        let dto = document_to_dto(doc);
        let json = serde_json::to_string_pretty(&dto)
            .map_err(|e| diagnostic(&format!("failed to encode fulfillment cache: {e}")))?;
        std::fs::write(&path, json).map_err(|e| {
            diagnostic(&format!("failed to write fulfillment cache {}: {e}", path.display()))
        })
    }
}

// ---------------------------------------------------------------------------
// Conversion helpers (private)
// ---------------------------------------------------------------------------

fn document_to_dto(
    doc: &ObligationFulfillmentCacheDocument,
) -> ObligationFulfillmentCacheDocumentDto {
    ObligationFulfillmentCacheDocumentDto {
        track_id: doc.track_id().as_ref().to_owned(),
        entries: doc.entries().iter().map(entry_to_dto).collect(),
    }
}

fn document_from_dto(
    dto: ObligationFulfillmentCacheDocumentDto,
) -> Result<ObligationFulfillmentCacheDocument, VerifyCacheError> {
    let track_id = TrackId::try_new(dto.track_id.clone()).map_err(|e| {
        VerifyCacheError::MalformedJson(diagnostic(&format!(
            "invalid track id '{}': {e}",
            dto.track_id
        )))
    })?;
    let mut entries = Vec::with_capacity(dto.entries.len());
    for entry in dto.entries {
        entries.push(entry_from_dto(entry)?);
    }
    Ok(ObligationFulfillmentCacheDocument::new(track_id, entries))
}

fn entry_to_dto(entry: &ObligationFulfillmentCacheEntry) -> ObligationFulfillmentCacheEntryDto {
    ObligationFulfillmentCacheEntryDto {
        edge_id: edge_id_to_dto(entry.edge_id()),
        obligation_id: obligation_id_to_dto(entry.obligation_id()),
        key: key_to_wire(entry.key()),
        verdict: verdict_to_dto(entry.verdict()),
        verifier_fingerprint: entry
            .verifier_fingerprint()
            .map(|fingerprint| fingerprint.as_hash().to_hex()),
    }
}

fn entry_from_dto(
    dto: ObligationFulfillmentCacheEntryDto,
) -> Result<ObligationFulfillmentCacheEntry, VerifyCacheError> {
    let edge_id = edge_id_from_dto(dto.edge_id).map_err(cache_error_from_artifact)?;
    let obligation_id =
        obligation_id_from_dto(dto.obligation_id).map_err(cache_error_from_artifact)?;
    let key = key_from_wire(dto.key)?;
    let verdict = verdict_from_dto(dto.verdict)?;
    let verifier_fingerprint = dto
        .verifier_fingerprint
        .map(|fingerprint| parse_cache_hash(&fingerprint).map(VerifierPromptFingerprint::new))
        .transpose()?;
    Ok(ObligationFulfillmentCacheEntry::new(
        edge_id,
        obligation_id,
        key,
        verdict,
        verifier_fingerprint,
    ))
}

fn key_to_wire(key: &ObligationFulfillmentCacheKey) -> FulfillmentCacheKeyWire {
    FulfillmentCacheKeyWire {
        bound_tests_set_hash: key.bound_tests_set_hash().as_hash().to_hex(),
        declaration_hash: key.declaration_hash().as_hash().to_hex(),
        anchor_text_hash: key.anchor_text_hash().as_hash().to_hex(),
    }
}

fn key_from_wire(
    wire: FulfillmentCacheKeyWire,
) -> Result<ObligationFulfillmentCacheKey, VerifyCacheError> {
    let bound_tests_set_hash =
        BoundTestsSetHash::new(parse_cache_hash(&wire.bound_tests_set_hash)?);
    let declaration_hash = DeclarationHash::new(parse_cache_hash(&wire.declaration_hash)?);
    let anchor_text_hash = AnchorTextHash::new(parse_cache_hash(&wire.anchor_text_hash)?);
    Ok(ObligationFulfillmentCacheKey::new(bound_tests_set_hash, declaration_hash, anchor_text_hash))
}

fn verdict_to_dto(verdict: &ObligationFulfillmentVerdict) -> ObligationFulfillmentVerdictDto {
    match verdict {
        ObligationFulfillmentVerdict::Fulfilled { citation } => {
            ObligationFulfillmentVerdictDto::Fulfilled { citation: citation.as_str().to_owned() }
        }
        ObligationFulfillmentVerdict::Fail { category, reason } => {
            ObligationFulfillmentVerdictDto::Fail {
                category: category_to_dto(category),
                reason: reason.as_str().to_owned(),
            }
        }
        ObligationFulfillmentVerdict::Pending => ObligationFulfillmentVerdictDto::Pending,
    }
}

fn verdict_from_dto(
    dto: ObligationFulfillmentVerdictDto,
) -> Result<ObligationFulfillmentVerdict, VerifyCacheError> {
    let verdict = match dto {
        ObligationFulfillmentVerdictDto::Fulfilled { citation } => {
            ObligationFulfillmentVerdict::Fulfilled { citation: parse_cache_citation(citation)? }
        }
        ObligationFulfillmentVerdictDto::Fail { category, reason } => {
            ObligationFulfillmentVerdict::Fail {
                category: category_from_dto(category),
                reason: parse_cache_reason(reason)?,
            }
        }
        ObligationFulfillmentVerdictDto::Pending => ObligationFulfillmentVerdict::Pending,
    };
    Ok(verdict)
}

fn category_to_dto(category: &FulfillmentFailCategory) -> FulfillmentFailCategoryDto {
    match category {
        FulfillmentFailCategory::Contradiction => FulfillmentFailCategoryDto::Contradiction,
        FulfillmentFailCategory::Substitution => FulfillmentFailCategoryDto::Substitution,
        FulfillmentFailCategory::CentralUnverified => FulfillmentFailCategoryDto::CentralUnverified,
    }
}

fn category_from_dto(dto: FulfillmentFailCategoryDto) -> FulfillmentFailCategory {
    match dto {
        FulfillmentFailCategoryDto::Contradiction => FulfillmentFailCategory::Contradiction,
        FulfillmentFailCategoryDto::Substitution => FulfillmentFailCategory::Substitution,
        FulfillmentFailCategoryDto::CentralUnverified => FulfillmentFailCategory::CentralUnverified,
    }
}

/// Parses a lowercase-hex cache-key hash, mapping failure onto the verdict-cache
/// error surface. Shared by both verdict-cache codecs (fulfillment / waiver).
pub(crate) fn parse_cache_hash(hex: &str) -> Result<domain::ContentHash, VerifyCacheError> {
    domain::ContentHash::try_from_hex(hex).map_err(|e| {
        VerifyCacheError::MalformedJson(diagnostic(&format!("invalid cache-key hash '{hex}': {e}")))
    })
}

/// Validates an evidence citation from cache JSON.
pub(crate) fn parse_cache_citation(citation: String) -> Result<EvidenceCitation, VerifyCacheError> {
    EvidenceCitation::try_new(citation).map_err(|e| {
        VerifyCacheError::MalformedJson(diagnostic(&format!("invalid evidence citation: {e}")))
    })
}

/// Validates a fail-reason diagnostic from cache JSON.
pub(crate) fn parse_cache_reason(reason: String) -> Result<DiagnosticMessage, VerifyCacheError> {
    DiagnosticMessage::try_new(reason).map_err(|e| {
        VerifyCacheError::MalformedJson(diagnostic(&format!("invalid fail reason: {e}")))
    })
}

/// Maps an artifact-codec error (from a reused edge/obligation-id conversion)
/// onto the verdict-cache error surface.
pub(crate) fn cache_error_from_artifact(
    error: domain::tddd::test_obligation::errors::ArtifactCodecError,
) -> VerifyCacheError {
    VerifyCacheError::MalformedJson(diagnostic(&error.to_string()))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use domain::ContentHash;
    use domain::tddd::semantic_verify::CatalogueEntryKey;
    use domain::tddd::test_obligation::ids::{
        TestObligationAnchorId, TestObligationId, TestObligationItemIdentifier,
    };
    use domain::tddd::test_obligation::vocab::TestObligationKind;

    fn edge_id() -> domain::tddd::test_obligation::ids::TestObligationEdgeId {
        domain::tddd::test_obligation::ids::TestObligationEdgeId::new(
            CatalogueEntryKey::try_new("domain::User".to_owned()).unwrap(),
            TestObligationAnchorId::try_new("spec.json".to_owned(), "IN-09".to_owned()).unwrap(),
        )
    }

    fn obligation_id() -> TestObligationId {
        TestObligationId::new(
            CatalogueEntryKey::try_new("domain::User".to_owned()).unwrap(),
            TestObligationKind::Result,
            TestObligationItemIdentifier::try_new("entry".to_owned()).unwrap(),
        )
    }

    fn key() -> ObligationFulfillmentCacheKey {
        ObligationFulfillmentCacheKey::new(
            BoundTestsSetHash::new(ContentHash::from_bytes([1u8; 32])),
            DeclarationHash::new(ContentHash::from_bytes([2u8; 32])),
            AnchorTextHash::new(ContentHash::from_bytes([3u8; 32])),
        )
    }

    fn verifier_fingerprint() -> VerifierPromptFingerprint {
        VerifierPromptFingerprint::new(ContentHash::from_bytes([4u8; 32]))
    }

    fn document(verdict: ObligationFulfillmentVerdict) -> ObligationFulfillmentCacheDocument {
        ObligationFulfillmentCacheDocument::new(
            TrackId::try_new("my-track").unwrap(),
            vec![ObligationFulfillmentCacheEntry::new(
                edge_id(),
                obligation_id(),
                key(),
                verdict,
                Some(verifier_fingerprint()),
            )],
        )
    }

    fn round_trip(verdict: ObligationFulfillmentVerdict) {
        let doc = document(verdict);
        let dto = document_to_dto(&doc);
        let json = serde_json::to_string_pretty(&dto).unwrap();
        let decoded: ObligationFulfillmentCacheDocumentDto = serde_json::from_str(&json).unwrap();
        assert_eq!(document_from_dto(decoded).unwrap(), doc);
    }

    // AC-06: the fulfilled verdict (with citation) round-trips, including the
    // three-hash cache key.
    #[test]
    fn test_fulfilled_verdict_round_trips() {
        round_trip(ObligationFulfillmentVerdict::Fulfilled {
            citation: EvidenceCitation::try_new("asserts empty rejected".to_owned()).unwrap(),
        });
    }

    // AC-08: each fail category round-trips.
    #[test]
    fn test_fail_verdict_round_trips_all_categories() {
        for category in [
            FulfillmentFailCategory::Contradiction,
            FulfillmentFailCategory::Substitution,
            FulfillmentFailCategory::CentralUnverified,
        ] {
            round_trip(ObligationFulfillmentVerdict::Fail {
                category,
                reason: DiagnosticMessage::try_new("asserts the opposite".to_owned()).unwrap(),
            });
        }
    }

    // AC-06: the pending verdict round-trips (treated as fail at the gate).
    #[test]
    fn test_pending_verdict_round_trips() {
        round_trip(ObligationFulfillmentVerdict::Pending);
    }

    // CN-04: the three hex hashes survive serialization exactly.
    #[test]
    fn test_hash_triple_serializes_as_hex() {
        let doc = document(ObligationFulfillmentVerdict::Pending);
        let dto = document_to_dto(&doc);
        let wire = &dto.entries[0].key;
        assert_eq!(wire.bound_tests_set_hash, "01".repeat(32));
        assert_eq!(wire.declaration_hash, "02".repeat(32));
        assert_eq!(wire.anchor_text_hash, "03".repeat(32));
        assert_eq!(dto.entries[0].verifier_fingerprint, Some("04".repeat(32)));
    }

    #[test]
    fn test_legacy_entry_without_fingerprint_decodes_as_absent() {
        let dto = document_to_dto(&document(ObligationFulfillmentVerdict::Pending));
        let mut json = serde_json::to_value(dto).unwrap();
        json["entries"][0].as_object_mut().unwrap().remove("verifier_fingerprint");

        let legacy: ObligationFulfillmentCacheDocumentDto = serde_json::from_value(json).unwrap();
        let decoded = document_from_dto(legacy).unwrap();

        assert_eq!(decoded.entries()[0].verifier_fingerprint(), None);
    }

    // IN-09 fail-closed: a malformed cache-key hash is a malformed-cache error.
    #[test]
    fn test_malformed_hash_is_malformed_json() {
        let doc = document(ObligationFulfillmentVerdict::Pending);
        let mut dto = document_to_dto(&doc);
        dto.entries[0].key.declaration_hash = "not-a-hash".to_owned();
        assert!(matches!(document_from_dto(dto), Err(VerifyCacheError::MalformedJson(_))));
    }

    // IN-09 fail-closed: an unknown field is rejected by deny_unknown_fields.
    #[test]
    fn test_unknown_field_is_rejected() {
        let json = r#"{ "track_id": "t", "entries": [], "extra": true }"#;
        assert!(serde_json::from_str::<ObligationFulfillmentCacheDocumentDto>(json).is_err());
    }

    // IN-09 / AC-06: the codec persists and reloads a cache via the port.
    #[test]
    fn test_codec_save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let codec = JsonObligationFulfillmentCacheCodec::new(dir.path().to_path_buf());
        let doc = document(ObligationFulfillmentVerdict::Fulfilled {
            citation: EvidenceCitation::try_new("cite".to_owned()).unwrap(),
        });
        codec.save(&doc).unwrap();
        assert_eq!(codec.load(doc.track_id()).unwrap(), Some(doc));
    }

    // IN-09 / CN-04: the trusted items root itself must not be a symlink.
    #[cfg(unix)]
    #[test]
    fn test_codec_symlinked_items_root_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let real_items = dir.path().join("real-items");
        let link_items = dir.path().join("items-link");
        std::fs::create_dir_all(&real_items).unwrap();
        std::os::unix::fs::symlink(&real_items, &link_items).unwrap();

        let codec = JsonObligationFulfillmentCacheCodec::new(link_items);
        let doc = document(ObligationFulfillmentVerdict::Pending);
        match codec.load(doc.track_id()) {
            Err(VerifyCacheError::Io(message)) => {
                assert!(message.as_str().contains("symlinked track items root"));
            }
            other => panic!("expected symlinked items root load error, got {other:?}"),
        }
        let save_error = codec.save(&doc).unwrap_err();
        assert!(save_error.as_str().contains("symlinked track items root"));
    }

    // IN-14 / AC-10: a missing cache loads as `None`.
    #[test]
    fn test_load_missing_cache_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let codec = JsonObligationFulfillmentCacheCodec::new(dir.path().to_path_buf());
        assert!(codec.load(&TrackId::try_new("absent-track").unwrap()).unwrap().is_none());
    }

    // CN-04: a fulfillment cache whose embedded `track_id` disagrees with the
    // requested id must not be returned — a copy from another track would
    // otherwise satisfy or block the gate on unrelated verdicts.
    #[test]
    fn test_codec_load_rejects_track_id_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let codec = JsonObligationFulfillmentCacheCodec::new(dir.path().to_path_buf());
        let doc = document(ObligationFulfillmentVerdict::Pending);
        codec.save(&doc).unwrap();

        let other_track = TrackId::try_new("other-track").unwrap();
        let source = dir.path().join(doc.track_id().as_ref()).join(FULFILLMENT_CACHE_ARTIFACT);
        let target_dir = dir.path().join(other_track.as_ref());
        std::fs::create_dir_all(&target_dir).unwrap();
        std::fs::rename(&source, target_dir.join(FULFILLMENT_CACHE_ARTIFACT)).unwrap();

        match codec.load(&other_track) {
            Err(VerifyCacheError::MalformedJson(message)) => {
                let text = message.as_str();
                assert!(text.contains("fulfillment cache track id mismatch"), "got: {text}");
                assert!(text.contains(other_track.as_ref()), "got: {text}");
                assert!(text.contains(doc.track_id().as_ref()), "got: {text}");
            }
            other => panic!("expected MalformedJson track-id mismatch error, got {other:?}"),
        }
    }
}
