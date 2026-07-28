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

use std::io::Write;
use std::path::PathBuf;

use domain::tddd::test_obligation::binding::NonEmptyTestLocations;
use domain::tddd::test_obligation::errors::VerifyCacheError;
use domain::tddd::test_obligation::hashes::{
    AnchorTextHash, BoundTestsSetHash, DeclarationHash, VerifierPromptFingerprint,
};
use domain::tddd::test_obligation::ids::DiagnosticMessage;
use domain::tddd::test_obligation::verdict::{
    ObligationFulfillmentCacheDocument, ObligationFulfillmentCacheEntry,
    ObligationFulfillmentCacheEntryState, ObligationFulfillmentCacheKey,
    ObligationFulfillmentVerdict,
};
use domain::tddd::test_obligation::vocab::FulfillmentFailCategory;
use domain::{EvidenceCitation, TrackId};
use serde::{Deserialize, Serialize};
use usecase::test_obligation::ports::ObligationFulfillmentCachePort;

use crate::test_obligation::bindings_codec::{
    TestLocationDto, TestObligationEdgeIdDto, edge_id_from_dto, edge_id_to_dto, location_from_dto,
    location_to_dto,
};
use crate::test_obligation::obligations_codec::{
    TestObligationIdDto, obligation_id_from_dto, obligation_id_to_dto,
};
use crate::test_obligation::{diagnostic, reject_symlinked_items_root};
use crate::track::symlink_guard::reject_symlinks_below;

mod fulfillment_cache_io;

use fulfillment_cache_io::{
    open_fulfillment_cache_for_write_guarded, read_bounded_fulfillment_cache,
    serialize_bounded_fulfillment_cache,
};

/// Artifact filename for the obligation-fulfillment verdict cache.
const FULFILLMENT_CACHE_ARTIFACT: &str = "obligation-fulfillment-cache.json";
const MAX_FULFILLMENT_CACHE_ENTRIES: usize = 1_024;
const MAX_BOUND_TESTS_PER_ENTRY: usize = 64;

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    bound_tests: Option<Vec<TestLocationDto>>,
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
#[derive(Clone)]
pub struct JsonObligationFulfillmentCacheCodec {
    items_dir: PathBuf,
}

impl std::fmt::Debug for JsonObligationFulfillmentCacheCodec {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("JsonObligationFulfillmentCacheCodec")
            .field("items_dir", &self.items_dir)
            .finish_non_exhaustive()
    }
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
            Ok(false) => {
                return Ok(None);
            }
            Err(source) => {
                return Err(VerifyCacheError::Io(diagnostic(&format!(
                    "refusing to read fulfillment cache {}: {source}",
                    path.display()
                ))));
            }
        }
        let content = read_bounded_fulfillment_cache(&path, &self.items_dir)
            .map_err(verify_cache_error_from_verify_cache)?;
        let dto: ObligationFulfillmentCacheDocumentDto = serde_json::from_str(&content)
            .map_err(|e| VerifyCacheError::MalformedJson(diagnostic(&e.to_string())))?;
        let doc = self.document_from_dto(dto)?;
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
        validate_fulfillment_cache_structure(doc)?;
        let dto = document_to_dto(doc);
        let json = serialize_bounded_fulfillment_cache(&dto)?;
        let mut file =
            open_fulfillment_cache_for_write_guarded(&path, &self.items_dir).map_err(|e| {
                diagnostic(&format!("failed to write fulfillment cache {}: {e}", path.display()))
            })?;
        let metadata = file.metadata().map_err(|e| {
            diagnostic(&format!("failed to inspect fulfillment cache {}: {e}", path.display()))
        })?;
        if !metadata.is_file() {
            return Err(diagnostic(&format!(
                "refusing to write non-regular fulfillment cache {}",
                path.display()
            )));
        }
        file.write_all(&json).map_err(|e| {
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

fn validate_fulfillment_cache_structure(
    doc: &ObligationFulfillmentCacheDocument,
) -> Result<(), DiagnosticMessage> {
    if doc.entries().len() > MAX_FULFILLMENT_CACHE_ENTRIES {
        return Err(diagnostic(&format!(
            "fulfillment cache has more than {MAX_FULFILLMENT_CACHE_ENTRIES} entries"
        )));
    }
    if doc.entries().iter().any(|entry| {
        entry.bound_tests().is_some_and(|tests| tests.as_slice().len() > MAX_BOUND_TESTS_PER_ENTRY)
    }) {
        return Err(diagnostic(&format!(
            "fulfillment cache entry has more than {MAX_BOUND_TESTS_PER_ENTRY} bound tests"
        )));
    }
    Ok(())
}

impl JsonObligationFulfillmentCacheCodec {
    fn document_from_dto(
        &self,
        dto: ObligationFulfillmentCacheDocumentDto,
    ) -> Result<ObligationFulfillmentCacheDocument, VerifyCacheError> {
        let track_id = TrackId::try_new(dto.track_id.clone()).map_err(|e| {
            VerifyCacheError::MalformedJson(diagnostic(&format!(
                "invalid track id '{}': {e}",
                dto.track_id
            )))
        })?;
        if dto.entries.len() > MAX_FULFILLMENT_CACHE_ENTRIES {
            return Err(VerifyCacheError::MalformedJson(diagnostic(&format!(
                "fulfillment cache has more than {MAX_FULFILLMENT_CACHE_ENTRIES} entries"
            ))));
        }
        let mut entries = Vec::with_capacity(dto.entries.len());
        for entry in dto.entries {
            entries.push(self.entry_from_dto(entry)?);
        }
        Ok(ObligationFulfillmentCacheDocument::new(track_id, entries))
    }

    fn entry_from_dto(
        &self,
        dto: ObligationFulfillmentCacheEntryDto,
    ) -> Result<ObligationFulfillmentCacheEntry, VerifyCacheError> {
        let edge_id = edge_id_from_dto(dto.edge_id).map_err(verify_cache_error_from_artifact)?;
        let obligation_id =
            obligation_id_from_dto(dto.obligation_id).map_err(verify_cache_error_from_artifact)?;
        let key = key_from_wire(dto.key)?;
        let verdict = verdict_from_dto(dto.verdict)?;
        let verifier_fingerprint = dto
            .verifier_fingerprint
            .map(|fingerprint| {
                parse_cache_hash(&fingerprint)
                    .map(VerifierPromptFingerprint::new)
                    .map_err(verify_cache_error_from_verify_cache)
            })
            .transpose()?;
        let bound_tests = dto.bound_tests.map(parse_bound_tests).transpose()?;
        let state = match verifier_fingerprint {
            Some(verifier_fingerprint) => ObligationFulfillmentCacheEntryState::Identified {
                verifier_fingerprint,
                bound_tests,
            },
            None => ObligationFulfillmentCacheEntryState::Legacy,
        };
        Ok(ObligationFulfillmentCacheEntry::new(edge_id, obligation_id, key, verdict, state))
    }
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
        bound_tests: entry
            .bound_tests()
            .map(|tests| tests.as_slice().iter().map(location_to_dto).collect()),
    }
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
    let bound_tests_set_hash = BoundTestsSetHash::new(
        parse_cache_hash(&wire.bound_tests_set_hash)
            .map_err(verify_cache_error_from_verify_cache)?,
    );
    let declaration_hash = DeclarationHash::new(
        parse_cache_hash(&wire.declaration_hash).map_err(verify_cache_error_from_verify_cache)?,
    );
    let anchor_text_hash = AnchorTextHash::new(
        parse_cache_hash(&wire.anchor_text_hash).map_err(verify_cache_error_from_verify_cache)?,
    );
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
            ObligationFulfillmentVerdict::Fulfilled {
                citation: parse_cache_citation(citation)
                    .map_err(verify_cache_error_from_verify_cache)?,
            }
        }
        ObligationFulfillmentVerdictDto::Fail { category, reason } => {
            ObligationFulfillmentVerdict::Fail {
                category: category_from_dto(category),
                reason: parse_cache_reason(reason).map_err(verify_cache_error_from_verify_cache)?,
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

fn verify_cache_error_from_verify_cache(error: VerifyCacheError) -> VerifyCacheError {
    error
}

fn verify_cache_error_from_artifact(
    error: domain::tddd::test_obligation::errors::ArtifactCodecError,
) -> VerifyCacheError {
    cache_error_from_artifact(error)
}

fn parse_bound_tests(
    bound_tests: Vec<TestLocationDto>,
) -> Result<NonEmptyTestLocations, VerifyCacheError> {
    if bound_tests.len() > MAX_BOUND_TESTS_PER_ENTRY {
        return Err(VerifyCacheError::MalformedJson(diagnostic(&format!(
            "fulfillment cache entry has more than {MAX_BOUND_TESTS_PER_ENTRY} bound tests"
        ))));
    }
    let locations = bound_tests
        .into_iter()
        .map(location_from_dto)
        .collect::<Result<Vec<_>, _>>()
        .map_err(verify_cache_error_from_artifact)?;
    NonEmptyTestLocations::try_new(locations).map_err(|error| {
        VerifyCacheError::MalformedJson(diagnostic(&format!("invalid bound tests: {error}")))
    })
}

#[cfg(all(test, unix))]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
#[path = "fulfillment_cache_codec/tests.rs"]
mod tests;
