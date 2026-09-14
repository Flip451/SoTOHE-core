//! JSON codec for the waiver verdict cache.
//!
//! Serialises the domain [`WaiverCacheDocument`] to a track-scoped
//! `waiver-cache.json` and validates it back (IN-09 / AC-06 / CN-04). Each entry
//! freezes a waiver verdict against the four-hash cache key (waived-reason hash,
//! entry-declaration hash, specification-element hash, and
//! obligation-responsibility hash): the hashes are serialised as lowercase hex
//! and any change to a component produces a different key, so a stale verdict is
//! treated as absent. Each entry also persists its verifier-prompt fingerprint;
//! an absent legacy fingerprint remains readable but is fail-closed by cache
//! readers, and recovery is only via re-evaluation (CN-04). Historical
//! three-hash keys remain readable for informational consumers but are marked
//! fingerprint-less before any lookup can reuse them. A passing verdict
//! structurally carries its evidence citation.

use std::fs::{File, OpenOptions};
use std::io::{Error, ErrorKind, Read, Write};
use std::path::{Path, PathBuf};

use domain::TrackId;
use domain::tddd::test_obligation::errors::VerifyCacheError;
use domain::tddd::test_obligation::hashes::{
    DeclarationHash, ObligationResponsibilityHash, SpecElementHash, VerifierPromptFingerprint,
    WaivedReasonHash,
};
use domain::tddd::test_obligation::ids::DiagnosticMessage;
use domain::tddd::test_obligation::ports::WaiverCachePort;
use domain::tddd::test_obligation::verdict::{
    WaiverCacheDocument, WaiverCacheEntry, WaiverCacheKey, WaiverVerdict,
};
use serde::{Deserialize, Serialize};

use crate::test_obligation::bindings_codec::{
    TestObligationEdgeIdDto, edge_id_from_dto, edge_id_to_dto,
};
use crate::test_obligation::fulfillment_cache_codec::{
    cache_error_from_artifact, open_fulfillment_cache_for_write_guarded, parse_cache_citation,
    parse_cache_hash, parse_cache_reason,
};
use crate::test_obligation::obligations_codec::{
    TestObligationIdDto, obligation_id_from_dto, obligation_id_to_dto,
};
use crate::test_obligation::{diagnostic, reject_symlinked_items_root};
use crate::track::symlink_guard::reject_symlinks_below;

/// Artifact filename for the waiver verdict cache.
const WAIVER_CACHE_ARTIFACT: &str = "waiver-cache.json";
const MAX_WAIVER_CACHE_BYTES: u64 = 4 * 1024 * 1024;

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

/// Serde DTO for [`WaiverVerdict`] (IN-09 / AC-06).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WaiverVerdictDto {
    /// The waiver reason holds for the edge; carries the evidence citation.
    Waived {
        /// Verbatim quotation supporting the waiver.
        citation: String,
    },
    /// The waiver reason does not hold for the edge.
    Fail {
        /// Human-readable description of why the waiver was rejected.
        reason: String,
    },
    /// The reviewer could not confirm the waiver; treated as fail at the gate.
    Pending,
}

/// Wire form of the four-component waiver cache key (IN-09 / CN-04).
///
/// Private helper: the domain [`WaiverCacheKey`] has no dedicated DTO in the type
/// contract, so its four hashes are carried inline as lowercase hex strings.
/// The optional legacy field permits historical three-component rows to remain
/// readable; such rows are marked fingerprint-less instead of being reused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WaiverCacheKeyWire {
    waived_reason_hash: String,
    declaration_hash: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    spec_element_hash: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    responsibility_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    anchor_text_hash: Option<String>,
}

/// Serde DTO for [`WaiverCacheEntry`] (IN-09).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaiverCacheEntryDto {
    edge_id: TestObligationEdgeIdDto,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    obligation_id: Option<TestObligationIdDto>,
    key: WaiverCacheKeyWire,
    verdict: WaiverVerdictDto,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    verifier_fingerprint: Option<String>,
}

/// Serde DTO for [`WaiverCacheDocument`] (IN-09).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WaiverCacheDocumentDto {
    track_id: String,
    entries: Vec<WaiverCacheEntryDto>,
}

// ---------------------------------------------------------------------------
// Codec adapter
// ---------------------------------------------------------------------------

/// JSON codec adapter for [`WaiverCachePort`] (IN-09 / AC-06 / CN-04).
#[derive(Debug, Clone)]
pub struct JsonWaiverCacheCodec {
    items_dir: PathBuf,
}

impl JsonWaiverCacheCodec {
    /// Creates a codec that resolves cache artifacts under `items_dir` (the track
    /// items root, e.g. `track/items`).
    #[must_use]
    pub fn new(items_dir: PathBuf) -> Self {
        Self { items_dir }
    }

    fn artifact_path(&self, track_id: &TrackId) -> PathBuf {
        self.items_dir.join(track_id.as_ref()).join(WAIVER_CACHE_ARTIFACT)
    }
}

impl WaiverCachePort for JsonWaiverCacheCodec {
    fn load(&self, track_id: &TrackId) -> Result<Option<WaiverCacheDocument>, VerifyCacheError> {
        reject_symlinked_items_root(&self.items_dir).map_err(|source| {
            VerifyCacheError::Io(diagnostic(&format!(
                "refusing to read waiver cache under {}: {source}",
                self.items_dir.display()
            )))
        })?;
        let path = self.artifact_path(track_id);
        match reject_symlinks_below(&path, &self.items_dir) {
            Ok(true) => {}
            Ok(false) => return Ok(None),
            Err(source) => {
                return Err(VerifyCacheError::Io(diagnostic(&format!(
                    "refusing to read waiver cache {}: {source}",
                    path.display()
                ))));
            }
        }
        let content = read_bounded_waiver_cache(&path)?;
        let dto: WaiverCacheDocumentDto = serde_json::from_str(&content)
            .map_err(|e| VerifyCacheError::MalformedJson(diagnostic(&e.to_string())))?;
        let doc = document_from_dto(dto)?;
        // Fail closed when the on-disk cache was copied from another track: a
        // matching filename is not proof of matching content, and the caller
        // trusts a `load(track_id)` result to describe exactly that track.
        if doc.track_id() != track_id {
            return Err(VerifyCacheError::MalformedJson(diagnostic(&format!(
                "waiver cache track id mismatch: requested '{}', got '{}'",
                track_id.as_ref(),
                doc.track_id().as_ref()
            ))));
        }
        Ok(Some(doc))
    }

    fn save(&self, doc: &WaiverCacheDocument) -> Result<(), DiagnosticMessage> {
        reject_symlinked_items_root(&self.items_dir).map_err(|source| {
            diagnostic(&format!(
                "refusing to write waiver cache under {}: {source}",
                self.items_dir.display()
            ))
        })?;
        let path = self.artifact_path(doc.track_id());
        let Some(_parent) = path.parent() else {
            return Err(diagnostic(&format!(
                "waiver cache path {} has no parent directory",
                path.display()
            )));
        };
        reject_symlinked_items_root(&self.items_dir).map_err(|source| {
            diagnostic(&format!(
                "refusing to write waiver cache under {}: {source}",
                self.items_dir.display()
            ))
        })?;
        if let Err(source) = reject_symlinks_below(&path, &self.items_dir) {
            return Err(diagnostic(&format!(
                "refusing to write waiver cache {}: {source}",
                path.display()
            )));
        }
        let dto = document_to_dto(doc);
        let json = serialize_bounded_waiver_cache(&dto)?;
        let mut file =
            open_fulfillment_cache_for_write_guarded(&path, &self.items_dir).map_err(|e| {
                diagnostic(&format!("failed to write waiver cache {}: {e}", path.display()))
            })?;
        let metadata = file.metadata().map_err(|e| {
            diagnostic(&format!("failed to inspect waiver cache {}: {e}", path.display()))
        })?;
        if !metadata.is_file() {
            return Err(diagnostic(&format!(
                "refusing to write non-regular waiver cache {}",
                path.display()
            )));
        }
        file.write_all(&json).map_err(|e| {
            diagnostic(&format!("failed to write waiver cache {}: {e}", path.display()))
        })
    }
}

/// Fixed-capacity JSON sink that rejects a cache before its serialized output can grow unbounded.
struct BoundedWaiverCacheBuffer {
    bytes: Vec<u8>,
    exceeded_limit: bool,
}

impl BoundedWaiverCacheBuffer {
    fn new() -> Self {
        Self { bytes: Vec::with_capacity(MAX_WAIVER_CACHE_BYTES as usize), exceeded_limit: false }
    }

    fn into_inner(self) -> Vec<u8> {
        self.bytes
    }

    fn clear(&mut self) {
        self.bytes.clear();
        self.exceeded_limit = false;
    }
}

impl Write for BoundedWaiverCacheBuffer {
    fn write(&mut self, bytes: &[u8]) -> Result<usize, Error> {
        let remaining = (MAX_WAIVER_CACHE_BYTES as usize).saturating_sub(self.bytes.len());
        if bytes.len() > remaining {
            self.exceeded_limit = true;
            return Err(Error::new(
                ErrorKind::WriteZero,
                "waiver cache serialized output exceeds the byte limit",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> Result<(), Error> {
        Ok(())
    }
}

/// Serializes a cache into a bounded buffer before replacing the on-disk artifact.
///
/// Holding at most [`MAX_WAIVER_CACHE_BYTES`] in memory keeps both serialization and the
/// eventual write bounded. On overflow, no cache file is written, so the prior cache remains
/// intact rather than being replaced with a partial document.
fn serialize_bounded_waiver_cache(
    dto: &WaiverCacheDocumentDto,
) -> Result<Vec<u8>, DiagnosticMessage> {
    let mut writer = BoundedWaiverCacheBuffer::new();
    // Bound the JSON before constructing the canonical representation so an
    // oversized cache cannot allocate an unbounded intermediate tree.
    serde_json::to_writer_pretty(&mut writer, dto).map_err(|error| {
        if writer.exceeded_limit {
            diagnostic(&format!("waiver cache exceeds {MAX_WAIVER_CACHE_BYTES} bytes"))
        } else {
            diagnostic(&format!("failed to encode waiver cache: {error}"))
        }
    })?;

    // Reparse only the bounded bytes. `Value` recursively orders every object
    // map, then the same bounded buffer is reused for the final artifact.
    let value: serde_json::Value = serde_json::from_slice(&writer.bytes)
        .map_err(|error| diagnostic(&format!("failed to encode waiver cache: {error}")))?;
    writer.clear();
    serde_json::to_writer_pretty(&mut writer, &value).map_err(|error| {
        if writer.exceeded_limit {
            diagnostic(&format!("waiver cache exceeds {MAX_WAIVER_CACHE_BYTES} bytes"))
        } else {
            diagnostic(&format!("failed to encode waiver cache: {error}"))
        }
    })?;
    Ok(writer.into_inner())
}

/// Reads a cache through a regular-file handle while enforcing a fixed byte limit.
///
/// The leaf is opened atomically without following symlinks before its handle is inspected.
/// Metadata is only a snapshot, so the read is capped one byte past the limit as well. This
/// rejects a cache that grows after inspection without allocating based on its full size.
fn read_bounded_waiver_cache(path: &Path) -> Result<String, VerifyCacheError> {
    let file = open_waiver_cache_nofollow(path).map_err(|error| {
        VerifyCacheError::Io(diagnostic(&format!(
            "failed to open waiver cache {}: {error}",
            path.display()
        )))
    })?;
    let opened_metadata = file.metadata().map_err(|error| {
        VerifyCacheError::Io(diagnostic(&format!(
            "cannot inspect opened waiver cache {}: {error}",
            path.display()
        )))
    })?;
    if !opened_metadata.is_file() {
        return Err(VerifyCacheError::Io(diagnostic(&format!(
            "waiver cache {} is not a regular file",
            path.display()
        ))));
    }
    if opened_metadata.len() > MAX_WAIVER_CACHE_BYTES {
        return Err(VerifyCacheError::Io(diagnostic(&format!(
            "waiver cache {} exceeds {MAX_WAIVER_CACHE_BYTES} bytes",
            path.display()
        ))));
    }

    let mut reader = file.take(MAX_WAIVER_CACHE_BYTES.saturating_add(1));
    let mut content = String::new();
    reader.read_to_string(&mut content).map_err(|error| {
        VerifyCacheError::Io(diagnostic(&format!(
            "failed to read waiver cache {}: {error}",
            path.display()
        )))
    })?;
    if content.len() > MAX_WAIVER_CACHE_BYTES as usize {
        return Err(VerifyCacheError::Io(diagnostic(&format!(
            "waiver cache {} exceeds {MAX_WAIVER_CACHE_BYTES} bytes",
            path.display()
        ))));
    }
    Ok(content)
}

/// Atomically opens a waiver cache leaf without following a symlink.
///
/// A `symlink_metadata` check before `File::open` would leave a replacement window. Platforms
/// without an equivalent atomic no-follow operation fail closed rather than following a link.
fn open_waiver_cache_nofollow(path: &Path) -> Result<File, std::io::Error> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;

        OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC).open(path)
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt as _;

        // FILE_FLAG_OPEN_REPARSE_POINT opens the reparse point itself, so the handle metadata
        // check above rejects it rather than following a symlink or junction.
        OpenOptions::new().read(true).custom_flags(0x0020_0000).open(path)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "atomic no-follow cache open is unavailable on this platform",
        ))
    }
}

// ---------------------------------------------------------------------------
// Conversion helpers (private)
// ---------------------------------------------------------------------------

fn document_to_dto(doc: &WaiverCacheDocument) -> WaiverCacheDocumentDto {
    WaiverCacheDocumentDto {
        track_id: doc.track_id().as_ref().to_owned(),
        entries: doc.entries().iter().map(entry_to_dto).collect(),
    }
}

fn document_from_dto(dto: WaiverCacheDocumentDto) -> Result<WaiverCacheDocument, VerifyCacheError> {
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
    Ok(WaiverCacheDocument::new(track_id, entries))
}

fn entry_to_dto(entry: &WaiverCacheEntry) -> WaiverCacheEntryDto {
    WaiverCacheEntryDto {
        edge_id: edge_id_to_dto(entry.edge_id()),
        obligation_id: entry.obligation_id().map(obligation_id_to_dto),
        key: key_to_wire(entry.key()),
        verdict: verdict_to_dto(entry.verdict()),
        verifier_fingerprint: entry
            .verifier_fingerprint()
            .map(|fingerprint| fingerprint.as_hash().to_hex()),
    }
}

fn entry_from_dto(dto: WaiverCacheEntryDto) -> Result<WaiverCacheEntry, VerifyCacheError> {
    let edge_id = edge_id_from_dto(dto.edge_id).map_err(cache_error_from_artifact)?;
    let obligation_id = dto
        .obligation_id
        .map(obligation_id_from_dto)
        .transpose()
        .map_err(cache_error_from_artifact)?;
    let (key, legacy_key) = key_from_wire(dto.key)?;
    let verdict = verdict_from_dto(dto.verdict)?;
    let verifier_fingerprint = dto
        .verifier_fingerprint
        .map(|fingerprint| parse_cache_hash(&fingerprint).map(VerifierPromptFingerprint::new))
        .transpose()?
        .filter(|_| !legacy_key);
    Ok(WaiverCacheEntry::new(edge_id, obligation_id, key, verdict, verifier_fingerprint))
}

fn key_to_wire(key: &WaiverCacheKey) -> WaiverCacheKeyWire {
    WaiverCacheKeyWire {
        waived_reason_hash: key.waived_reason_hash().as_hash().to_hex(),
        declaration_hash: key.declaration_hash().as_hash().to_hex(),
        spec_element_hash: key.spec_element_hash().as_hash().to_hex(),
        responsibility_hash: key.responsibility_hash().as_hash().to_hex(),
        anchor_text_hash: None,
    }
}

fn key_from_wire(wire: WaiverCacheKeyWire) -> Result<(WaiverCacheKey, bool), VerifyCacheError> {
    let legacy_key = wire.spec_element_hash.is_empty() || wire.responsibility_hash.is_empty();
    if !legacy_key && wire.anchor_text_hash.is_some() {
        return Err(VerifyCacheError::MalformedJson(diagnostic(
            "current waiver cache key cannot include anchor_text_hash",
        )));
    }
    let spec_element_hex = if wire.spec_element_hash.is_empty() {
        wire.anchor_text_hash.ok_or_else(|| {
            VerifyCacheError::MalformedJson(diagnostic(
                "legacy waiver cache key is missing anchor_text_hash",
            ))
        })?
    } else {
        wire.spec_element_hash
    };
    let waived_reason_hash = WaivedReasonHash::new(parse_cache_hash(&wire.waived_reason_hash)?);
    let declaration_hash = DeclarationHash::new(parse_cache_hash(&wire.declaration_hash)?);
    let spec_element_hash = SpecElementHash::new(parse_cache_hash(&spec_element_hex)?);
    let responsibility_hash = if wire.responsibility_hash.is_empty() {
        ObligationResponsibilityHash::new(domain::ContentHash::from_bytes([0u8; 32]))
    } else {
        ObligationResponsibilityHash::new(parse_cache_hash(&wire.responsibility_hash)?)
    };
    Ok((
        WaiverCacheKey::new(
            waived_reason_hash,
            declaration_hash,
            spec_element_hash,
            responsibility_hash,
        ),
        legacy_key,
    ))
}

fn verdict_to_dto(verdict: &WaiverVerdict) -> WaiverVerdictDto {
    match verdict {
        WaiverVerdict::Waived { citation } => {
            WaiverVerdictDto::Waived { citation: citation.as_str().to_owned() }
        }
        WaiverVerdict::Fail { reason } => {
            WaiverVerdictDto::Fail { reason: reason.as_str().to_owned() }
        }
        WaiverVerdict::Pending => WaiverVerdictDto::Pending,
    }
}

fn verdict_from_dto(dto: WaiverVerdictDto) -> Result<WaiverVerdict, VerifyCacheError> {
    let verdict = match dto {
        WaiverVerdictDto::Waived { citation } => {
            WaiverVerdict::Waived { citation: parse_cache_citation(citation)? }
        }
        WaiverVerdictDto::Fail { reason } => {
            WaiverVerdict::Fail { reason: parse_cache_reason(reason)? }
        }
        WaiverVerdictDto::Pending => WaiverVerdict::Pending,
    };
    Ok(verdict)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use domain::ContentHash;
    use domain::EvidenceCitation;
    use domain::tddd::semantic_verify::CatalogueEntryKey;
    use domain::tddd::test_obligation::ids::{
        TestObligationAnchorId, TestObligationEdgeId, TestObligationId,
        TestObligationItemIdentifier,
    };
    use domain::tddd::test_obligation::vocab::TestObligationKind;

    fn edge_id() -> TestObligationEdgeId {
        TestObligationEdgeId::new(
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

    fn key() -> WaiverCacheKey {
        WaiverCacheKey::new(
            WaivedReasonHash::new(ContentHash::from_bytes([4u8; 32])),
            DeclarationHash::new(ContentHash::from_bytes([2u8; 32])),
            SpecElementHash::new(ContentHash::from_bytes([3u8; 32])),
            ObligationResponsibilityHash::new(ContentHash::from_bytes([6u8; 32])),
        )
    }

    fn verifier_fingerprint() -> VerifierPromptFingerprint {
        VerifierPromptFingerprint::new(ContentHash::from_bytes([5u8; 32]))
    }

    fn document(verdict: WaiverVerdict) -> WaiverCacheDocument {
        WaiverCacheDocument::new(
            TrackId::try_new("my-track").unwrap(),
            vec![WaiverCacheEntry::new(
                edge_id(),
                Some(obligation_id()),
                key(),
                verdict,
                Some(verifier_fingerprint()),
            )],
        )
    }

    fn round_trip(verdict: WaiverVerdict) {
        let doc = document(verdict);
        let dto = document_to_dto(&doc);
        let json = serde_json::to_string_pretty(&dto).unwrap();
        let decoded: WaiverCacheDocumentDto = serde_json::from_str(&json).unwrap();
        assert_eq!(document_from_dto(decoded).unwrap(), doc);
    }

    // AC-06: the waived verdict (with citation) round-trips, including the
    // four-hash cache key.
    #[test]
    fn test_waived_verdict_round_trips() {
        round_trip(WaiverVerdict::Waived {
            citation: EvidenceCitation::try_new("covered by integration suite".to_owned()).unwrap(),
        });
    }

    // AC-06: the fail verdict round-trips.
    #[test]
    fn test_fail_verdict_round_trips() {
        round_trip(WaiverVerdict::Fail {
            reason: DiagnosticMessage::try_new("reason does not hold".to_owned()).unwrap(),
        });
    }

    // AC-06: the pending verdict round-trips (treated as fail at the gate).
    #[test]
    fn test_pending_verdict_round_trips() {
        round_trip(WaiverVerdict::Pending);
    }

    // CN-04: the four hex hashes survive serialization exactly.
    #[test]
    fn test_hash_quadruple_serializes_as_hex() {
        let doc = document(WaiverVerdict::Pending);
        let dto = document_to_dto(&doc);
        let wire = &dto.entries[0].key;
        assert_eq!(wire.waived_reason_hash, "04".repeat(32));
        assert_eq!(wire.declaration_hash, "02".repeat(32));
        assert_eq!(wire.spec_element_hash, "03".repeat(32));
        assert_eq!(wire.responsibility_hash, "06".repeat(32));
        assert_eq!(dto.entries[0].verifier_fingerprint, Some("05".repeat(32)));
    }

    #[test]
    fn test_legacy_entry_without_fingerprint_decodes_as_absent() {
        let dto = document_to_dto(&document(WaiverVerdict::Pending));
        let mut json = serde_json::to_value(dto).unwrap();
        json["entries"][0].as_object_mut().unwrap().remove("verifier_fingerprint");

        let legacy: WaiverCacheDocumentDto = serde_json::from_value(json).unwrap();
        let decoded = document_from_dto(legacy).unwrap();

        assert_eq!(decoded.entries()[0].verifier_fingerprint(), None);
    }

    #[test]
    fn test_legacy_entry_without_obligation_owner_decodes_as_absent() {
        let dto = document_to_dto(&document(WaiverVerdict::Pending));
        let mut json = serde_json::to_value(dto).unwrap();
        json["entries"][0].as_object_mut().unwrap().remove("obligation_id");

        let legacy: WaiverCacheDocumentDto = serde_json::from_value(json).unwrap();
        let decoded = document_from_dto(legacy).unwrap();

        assert_eq!(decoded.entries()[0].obligation_id(), None);
    }

    // IN-09 fail-closed: a malformed cache-key hash is a malformed-cache error.
    #[test]
    fn test_malformed_hash_is_malformed_json() {
        let doc = document(WaiverVerdict::Pending);
        let mut dto = document_to_dto(&doc);
        dto.entries[0].key.waived_reason_hash = "not-a-hash".to_owned();
        assert!(matches!(document_from_dto(dto), Err(VerifyCacheError::MalformedJson(_))));
    }

    // IN-09 fail-closed: an unknown field is rejected by deny_unknown_fields.
    #[test]
    fn test_unknown_field_is_rejected() {
        let json = r#"{ "track_id": "t", "entries": [], "extra": true }"#;
        assert!(serde_json::from_str::<WaiverCacheDocumentDto>(json).is_err());
    }

    #[test]
    fn test_legacy_three_hash_key_is_readable_but_not_reusable() {
        let mut json =
            serde_json::to_value(document_to_dto(&document(WaiverVerdict::Pending))).unwrap();
        let key = json["entries"][0]["key"].as_object_mut().unwrap();
        key.remove("spec_element_hash");
        key.remove("responsibility_hash");
        key.insert("anchor_text_hash".to_owned(), serde_json::Value::String("03".repeat(32)));

        let legacy: WaiverCacheDocumentDto = serde_json::from_value(json).unwrap();
        let decoded = document_from_dto(legacy).unwrap();
        let entry = &decoded.entries()[0];

        assert_eq!(entry.verifier_fingerprint(), None);
        assert_ne!(
            entry.verifier_fingerprint(),
            Some(&verifier_fingerprint()),
            "a legacy three-hash verdict must not be reusable"
        );
    }

    // IN-09 / AC-06: the codec persists and reloads a cache via the port.
    #[test]
    fn test_codec_save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let codec = JsonWaiverCacheCodec::new(dir.path().to_path_buf());
        let doc = document(WaiverVerdict::Waived {
            citation: EvidenceCitation::try_new("cite".to_owned()).unwrap(),
        });
        codec.save(&doc).unwrap();
        assert_eq!(codec.load(doc.track_id()).unwrap(), Some(doc));
    }

    #[test]
    fn test_codec_replaces_stale_waiver_pass_and_fail_for_subsequent_check() {
        let dir = tempfile::tempdir().unwrap();
        let codec = JsonWaiverCacheCodec::new(dir.path().to_path_buf());
        let current_key = key();
        let stale_key = WaiverCacheKey::new(
            WaivedReasonHash::new(ContentHash::from_bytes([9u8; 32])),
            current_key.declaration_hash().clone(),
            current_key.spec_element_hash().clone(),
            current_key.responsibility_hash().clone(),
        );

        for (stale_verdict, refreshed_verdict) in [
            (
                WaiverVerdict::Waived {
                    citation: EvidenceCitation::try_new("stale waiver pass".to_owned()).unwrap(),
                },
                WaiverVerdict::Fail {
                    reason: DiagnosticMessage::try_new("re-evaluated waiver failure".to_owned())
                        .unwrap(),
                },
            ),
            (
                WaiverVerdict::Fail {
                    reason: DiagnosticMessage::try_new("stale waiver failure".to_owned()).unwrap(),
                },
                WaiverVerdict::Waived {
                    citation: EvidenceCitation::try_new("re-evaluated waiver pass".to_owned())
                        .unwrap(),
                },
            ),
        ] {
            let stale = WaiverCacheDocument::new(
                TrackId::try_new("my-track").unwrap(),
                vec![WaiverCacheEntry::new(
                    edge_id(),
                    Some(obligation_id()),
                    stale_key.clone(),
                    stale_verdict,
                    Some(verifier_fingerprint()),
                )],
            );
            codec.save(&stale).unwrap();
            let loaded = codec.load(stale.track_id()).unwrap().unwrap();
            assert!(
                loaded
                    .lookup_current(
                        &edge_id(),
                        &obligation_id(),
                        &current_key,
                        &verifier_fingerprint()
                    )
                    .unwrap()
                    .is_none()
            );

            let refreshed = WaiverCacheDocument::new(
                stale.track_id().clone(),
                vec![WaiverCacheEntry::new(
                    edge_id(),
                    Some(obligation_id()),
                    current_key.clone(),
                    refreshed_verdict.clone(),
                    Some(verifier_fingerprint()),
                )],
            );
            codec.save(&refreshed).unwrap();
            let loaded = codec.load(refreshed.track_id()).unwrap().unwrap();
            let selected = loaded
                .lookup_current(&edge_id(), &obligation_id(), &current_key, &verifier_fingerprint())
                .unwrap()
                .unwrap();
            assert_eq!(selected.verdict(), &refreshed_verdict);
        }
    }

    #[test]
    fn test_waiver_cache_writer_populated_verdict_returns_canonical_deterministic_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let codec = JsonWaiverCacheCodec::new(dir.path().to_path_buf());
        let doc = document(WaiverVerdict::Waived {
            citation: EvidenceCitation::try_new("canonical evidence".to_owned()).unwrap(),
        });
        let path = dir.path().join(doc.track_id().as_ref()).join(WAIVER_CACHE_ARTIFACT);

        codec.save(&doc).unwrap();
        let first = std::fs::read(&path).unwrap();
        codec.save(&doc).unwrap();
        let second = std::fs::read(path).unwrap();
        let json = std::str::from_utf8(&first).unwrap();

        assert_eq!(first, second);
        assert!(json.starts_with("{\n  \"entries\":"), "root keys must be canonical: {json}");
        assert!(
            json.contains(
                "\"key\": {\n        \"declaration_hash\": \"0202020202020202020202020202020202020202020202020202020202020202\",\n        \"responsibility_hash\": \"0606060606060606060606060606060606060606060606060606060606060606\",\n        \"spec_element_hash\": \"0303030303030303030303030303030303030303030303030303030303030303\",\n        \"waived_reason_hash\": \"0404040404040404040404040404040404040404040404040404040404040404\""
            ),
            "nested cache-key fields must be canonical: {json}"
        );
        let verdict = &json[json.find("\"verdict\"").unwrap()..];
        assert!(
            verdict.find("\"citation\"").unwrap() < verdict.find("\"kind\"").unwrap(),
            "nested waived verdict fields must be canonical: {verdict}"
        );
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

        let codec = JsonWaiverCacheCodec::new(link_items);
        let doc = document(WaiverVerdict::Pending);
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
        let codec = JsonWaiverCacheCodec::new(dir.path().to_path_buf());
        assert!(codec.load(&TrackId::try_new("absent-track").unwrap()).unwrap().is_none());
    }

    #[test]
    fn test_load_oversized_cache_returns_io_error_without_reading_it() {
        let dir = tempfile::tempdir().unwrap();
        let codec = JsonWaiverCacheCodec::new(dir.path().to_path_buf());
        let track_id = TrackId::try_new("oversized-track").unwrap();
        let path = dir.path().join(track_id.as_ref()).join(WAIVER_CACHE_ARTIFACT);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        File::create(&path).unwrap().set_len(MAX_WAIVER_CACHE_BYTES + 1).unwrap();

        match codec.load(&track_id) {
            Err(VerifyCacheError::Io(message)) => {
                assert!(message.as_str().contains("exceeds"));
            }
            other => panic!("expected oversized cache Io error, got {other:?}"),
        }
    }

    #[test]
    fn test_codec_save_oversized_cache_returns_error_without_writing_it() {
        let dir = tempfile::tempdir().unwrap();
        let codec = JsonWaiverCacheCodec::new(dir.path().to_path_buf());
        let track_id = TrackId::try_new("oversized-save-track").unwrap();
        let entry = document(WaiverVerdict::Pending).entries()[0].clone();
        let doc = WaiverCacheDocument::new(track_id.clone(), vec![entry; 10_000]);
        let path = dir.path().join(track_id.as_ref()).join(WAIVER_CACHE_ARTIFACT);

        let error = codec.save(&doc).unwrap_err();

        assert!(error.as_str().contains("exceeds"));
        assert!(!path.exists());
    }

    // The cache reader must enforce its symlink rejection at open time, not only through a
    // pre-open metadata snapshot that an attacker could replace before `open`.
    #[cfg(unix)]
    #[test]
    fn test_read_bounded_waiver_cache_symlink_returns_io_error() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("cache-target.json");
        let link = dir.path().join("waiver-cache.json");
        std::fs::write(&target, "{}").unwrap();
        std::os::unix::fs::symlink(&target, &link).unwrap();

        assert!(matches!(read_bounded_waiver_cache(&link), Err(VerifyCacheError::Io(_))));
    }

    // CN-04: a waiver cache whose embedded `track_id` disagrees with the
    // requested id must not be returned — a copy from another track would
    // otherwise satisfy or block the gate on unrelated verdicts.
    #[test]
    fn test_codec_load_rejects_track_id_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let codec = JsonWaiverCacheCodec::new(dir.path().to_path_buf());
        let doc = document(WaiverVerdict::Pending);
        codec.save(&doc).unwrap();

        let other_track = TrackId::try_new("other-track").unwrap();
        let source = dir.path().join(doc.track_id().as_ref()).join(WAIVER_CACHE_ARTIFACT);
        let target_dir = dir.path().join(other_track.as_ref());
        std::fs::create_dir_all(&target_dir).unwrap();
        std::fs::rename(&source, target_dir.join(WAIVER_CACHE_ARTIFACT)).unwrap();

        match codec.load(&other_track) {
            Err(VerifyCacheError::MalformedJson(message)) => {
                let text = message.as_str();
                assert!(text.contains("waiver cache track id mismatch"), "got: {text}");
                assert!(text.contains(other_track.as_ref()), "got: {text}");
                assert!(text.contains(doc.track_id().as_ref()), "got: {text}");
            }
            other => panic!("expected MalformedJson track-id mismatch error, got {other:?}"),
        }
    }
}
