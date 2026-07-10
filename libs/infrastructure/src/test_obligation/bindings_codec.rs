//! JSON codec for the implementer-authored test-bindings artifact.
//!
//! Serialises the domain [`TestBindingsDocument`] to a track-scoped
//! `test-bindings.json` and validates it back, enforcing the fail-closed
//! decode-time contract (IN-06 / OS-06 / AC-04): unknown fields are rejected and
//! the non-empty test-evidence invariant of the bound variants is re-checked via
//! [`NonEmptyTestLocations::try_new`], so an empty `tests` array is surfaced as a
//! domain-invariant error rather than silently accepted.
//!
//! The three binding forms — a fulfilling binding, a justified waiver, and a
//! voluntary binding for an edge with no derived obligation — are the closed
//! vocabulary an author records against obligation edges (IN-06). Bindings live
//! in a track artifact, not in source marker comments (OS-06), so test sources
//! stay plain Rust.

use std::path::PathBuf;

use domain::TrackId;
use domain::tddd::layer_id::LayerId;
use domain::tddd::semantic_verify::CatalogueEntryKey;
use domain::tddd::test_obligation::binding::{
    NonEmptyTestLocations, TestBindingRecord, TestBindingsDocument, TestLocation,
};
use domain::tddd::test_obligation::errors::ArtifactCodecError;
use domain::tddd::test_obligation::ids::{
    DiagnosticMessage, TestFunctionName, TestModulePath, TestObligationEdgeId, WaivedReason,
};
use domain::tddd::test_obligation::ports::TestBindingsArtifactPort;
use serde::{Deserialize, Serialize};

use crate::test_obligation::obligations_codec::{
    TestObligationAnchorIdDto, TestObligationIdDto, anchor_id_from_dto, anchor_id_to_dto,
    obligation_id_from_dto, obligation_id_to_dto,
};
use crate::test_obligation::{diagnostic, reject_symlinked_items_root};
use crate::track::symlink_guard::reject_symlinks_below;

/// Artifact filename for the test-bindings document.
const BINDINGS_ARTIFACT: &str = "test-bindings.json";

/// Infrastructure-local alias for the artifact codec error surfaced by
/// [`JsonTestBindingsCodec`] (IN-06).
pub type TestBindingsCodecError = ArtifactCodecError;

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

/// Serde DTO for [`TestLocation`] (IN-06).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestLocationDto {
    layer: String,
    module_path: String,
    test_name: String,
}

/// Serde DTO for [`TestObligationEdgeId`] (IN-06).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestObligationEdgeIdDto {
    entry_key: String,
    anchor_id: TestObligationAnchorIdDto,
}

/// Serde DTO for [`TestBindingRecord`] — the three-form binding union (IN-06).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TestBindingRecordDto {
    /// Tests that fulfill a derived obligation.
    Fulfillment {
        /// The obligation being fulfilled.
        obligation_id: TestObligationIdDto,
        /// The tests bound to the obligation (non-empty; re-checked on decode).
        tests: Vec<TestLocationDto>,
    },
    /// A justified waiver for an obligation edge.
    Waiver {
        /// The obligation edge being waived.
        edge_id: TestObligationEdgeIdDto,
        /// The reason the edge is waived.
        reason: String,
    },
    /// Tests voluntarily bound to an edge with no derived obligation.
    VoluntaryBinding {
        /// The edge the tests are voluntarily bound to.
        edge_id: TestObligationEdgeIdDto,
        /// The voluntarily bound tests (non-empty; re-checked on decode).
        tests: Vec<TestLocationDto>,
    },
}

/// Serde DTO for [`TestBindingsDocument`] (IN-06).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestBindingsDocumentDto {
    track_id: String,
    records: Vec<TestBindingRecordDto>,
}

impl TestBindingsDocumentDto {
    /// Projects a domain [`TestBindingsDocument`] onto its wire DTO.
    #[must_use]
    pub fn from_domain(doc: &TestBindingsDocument) -> Self {
        Self {
            track_id: doc.track_id().as_ref().to_owned(),
            records: doc.records().iter().map(record_to_dto).collect(),
        }
    }

    /// Projects the wire DTO back onto a domain [`TestBindingsDocument`].
    ///
    /// # Errors
    ///
    /// Returns a [`TestBindingsCodecError`] when the `track_id` or a binding
    /// record fails a domain invariant (including an empty `tests` array for a
    /// bound variant).
    pub fn into_domain(self) -> Result<TestBindingsDocument, TestBindingsCodecError> {
        let track_id = TrackId::try_new(self.track_id.clone()).map_err(|e| {
            ArtifactCodecError::DomainInvariant(diagnostic(&format!(
                "invalid track id '{}': {e}",
                self.track_id
            )))
        })?;
        let mut records = Vec::with_capacity(self.records.len());
        for dto in self.records {
            records.push(record_from_dto(dto)?);
        }
        Ok(TestBindingsDocument::new(track_id, records))
    }
}

// ---------------------------------------------------------------------------
// Codec adapter
// ---------------------------------------------------------------------------

/// JSON codec adapter for [`TestBindingsArtifactPort`] (IN-06 / AC-04).
#[derive(Debug, Clone)]
pub struct JsonTestBindingsCodec {
    items_dir: PathBuf,
}

impl JsonTestBindingsCodec {
    /// Creates a codec that resolves artifacts under `items_dir` (the track
    /// items root, e.g. `track/items`).
    #[must_use]
    pub fn new(items_dir: PathBuf) -> Self {
        Self { items_dir }
    }

    fn artifact_path(&self, track_id: &TrackId) -> PathBuf {
        self.items_dir.join(track_id.as_ref()).join(BINDINGS_ARTIFACT)
    }
}

impl TestBindingsArtifactPort for JsonTestBindingsCodec {
    fn load(&self, track_id: &TrackId) -> Result<Option<TestBindingsDocument>, ArtifactCodecError> {
        reject_symlinked_items_root(&self.items_dir).map_err(|source| {
            ArtifactCodecError::Io(diagnostic(&format!(
                "refusing to read test-bindings artifact under {}: {source}",
                self.items_dir.display()
            )))
        })?;
        let path = self.artifact_path(track_id);
        match reject_symlinks_below(&path, &self.items_dir) {
            Ok(true) => {}
            Ok(false) => return Ok(None),
            Err(source) => {
                return Err(ArtifactCodecError::Io(diagnostic(&format!(
                    "refusing to read test-bindings artifact {}: {source}",
                    path.display()
                ))));
            }
        }
        let content = std::fs::read_to_string(&path).map_err(|e| {
            ArtifactCodecError::Io(diagnostic(&format!(
                "failed to read test-bindings artifact {}: {e}",
                path.display()
            )))
        })?;
        let dto: TestBindingsDocumentDto = serde_json::from_str(&content)
            .map_err(|e| ArtifactCodecError::MalformedJson(diagnostic(&e.to_string())))?;
        let doc = dto.into_domain()?;
        // Fail closed when the on-disk artifact was copied from another track: a
        // matching filename is not proof of matching content, and the caller
        // trusts a `load(track_id)` result to describe exactly that track.
        if doc.track_id() != track_id {
            return Err(ArtifactCodecError::DomainInvariant(diagnostic(&format!(
                "test-bindings artifact track id mismatch: requested '{}', got '{}'",
                track_id.as_ref(),
                doc.track_id().as_ref()
            ))));
        }
        Ok(Some(doc))
    }

    fn save(&self, doc: &TestBindingsDocument) -> Result<(), DiagnosticMessage> {
        reject_symlinked_items_root(&self.items_dir).map_err(|source| {
            diagnostic(&format!(
                "refusing to write test-bindings artifact under {}: {source}",
                self.items_dir.display()
            ))
        })?;
        let path = self.artifact_path(doc.track_id());
        let Some(parent) = path.parent() else {
            return Err(diagnostic(&format!(
                "test-bindings artifact path {} has no parent directory",
                path.display()
            )));
        };
        std::fs::create_dir_all(parent).map_err(|e| {
            diagnostic(&format!("failed to create track directory {}: {e}", parent.display()))
        })?;
        reject_symlinked_items_root(&self.items_dir).map_err(|source| {
            diagnostic(&format!(
                "refusing to write test-bindings artifact under {}: {source}",
                self.items_dir.display()
            ))
        })?;
        if let Err(source) = reject_symlinks_below(&path, &self.items_dir) {
            return Err(diagnostic(&format!(
                "refusing to write test-bindings artifact {}: {source}",
                path.display()
            )));
        }
        let dto = TestBindingsDocumentDto::from_domain(doc);
        let json = serde_json::to_string_pretty(&dto)
            .map_err(|e| diagnostic(&format!("failed to encode test-bindings artifact: {e}")))?;
        std::fs::write(&path, json).map_err(|e| {
            diagnostic(&format!("failed to write test-bindings artifact {}: {e}", path.display()))
        })
    }
}

// ---------------------------------------------------------------------------
// Conversion helpers (private)
// ---------------------------------------------------------------------------

fn record_to_dto(record: &TestBindingRecord) -> TestBindingRecordDto {
    match record {
        TestBindingRecord::Fulfillment { obligation_id, tests } => {
            TestBindingRecordDto::Fulfillment {
                obligation_id: obligation_id_to_dto(obligation_id),
                tests: locations_to_dto(tests),
            }
        }
        TestBindingRecord::Waiver { edge_id, reason } => TestBindingRecordDto::Waiver {
            edge_id: edge_id_to_dto(edge_id),
            reason: reason.as_str().to_owned(),
        },
        TestBindingRecord::VoluntaryBinding { edge_id, tests } => {
            TestBindingRecordDto::VoluntaryBinding {
                edge_id: edge_id_to_dto(edge_id),
                tests: locations_to_dto(tests),
            }
        }
    }
}

fn record_from_dto(dto: TestBindingRecordDto) -> Result<TestBindingRecord, TestBindingsCodecError> {
    let record = match dto {
        TestBindingRecordDto::Fulfillment { obligation_id, tests } => {
            TestBindingRecord::Fulfillment {
                obligation_id: obligation_id_from_dto(obligation_id)?,
                tests: locations_from_dto(tests)?,
            }
        }
        TestBindingRecordDto::Waiver { edge_id, reason } => {
            let reason = WaivedReason::try_new(reason).map_err(|e| {
                ArtifactCodecError::DomainInvariant(diagnostic(&format!(
                    "invalid waived reason: {e}"
                )))
            })?;
            TestBindingRecord::Waiver { edge_id: edge_id_from_dto(edge_id)?, reason }
        }
        TestBindingRecordDto::VoluntaryBinding { edge_id, tests } => {
            TestBindingRecord::VoluntaryBinding {
                edge_id: edge_id_from_dto(edge_id)?,
                tests: locations_from_dto(tests)?,
            }
        }
    };
    Ok(record)
}

fn locations_to_dto(locations: &NonEmptyTestLocations) -> Vec<TestLocationDto> {
    locations.as_slice().iter().map(location_to_dto).collect()
}

fn locations_from_dto(
    dtos: Vec<TestLocationDto>,
) -> Result<NonEmptyTestLocations, TestBindingsCodecError> {
    let mut locations = Vec::with_capacity(dtos.len());
    for dto in dtos {
        locations.push(location_from_dto(dto)?);
    }
    NonEmptyTestLocations::try_new(locations).map_err(|e| {
        ArtifactCodecError::DomainInvariant(diagnostic(&format!(
            "binding must reference at least one test: {e}"
        )))
    })
}

fn location_to_dto(location: &TestLocation) -> TestLocationDto {
    TestLocationDto {
        layer: location.layer().as_ref().to_owned(),
        module_path: location.module_path().as_str().to_owned(),
        test_name: location.test_name().as_str().to_owned(),
    }
}

fn location_from_dto(dto: TestLocationDto) -> Result<TestLocation, TestBindingsCodecError> {
    let layer = LayerId::try_new(dto.layer).map_err(|e| {
        ArtifactCodecError::DomainInvariant(diagnostic(&format!("invalid test layer: {e}")))
    })?;
    let module_path = TestModulePath::try_new(dto.module_path).map_err(|e| {
        ArtifactCodecError::DomainInvariant(diagnostic(&format!("invalid test module path: {e}")))
    })?;
    let test_name = TestFunctionName::try_new(dto.test_name).map_err(|e| {
        ArtifactCodecError::DomainInvariant(diagnostic(&format!("invalid test name: {e}")))
    })?;
    Ok(TestLocation::new(layer, module_path, test_name))
}

pub(crate) fn edge_id_to_dto(edge_id: &TestObligationEdgeId) -> TestObligationEdgeIdDto {
    TestObligationEdgeIdDto {
        entry_key: edge_id.entry_key().as_str().to_owned(),
        anchor_id: anchor_id_to_dto(edge_id.anchor_id()),
    }
}

pub(crate) fn edge_id_from_dto(
    dto: TestObligationEdgeIdDto,
) -> Result<TestObligationEdgeId, TestBindingsCodecError> {
    let entry_key = CatalogueEntryKey::try_new(dto.entry_key).map_err(|e| {
        ArtifactCodecError::DomainInvariant(diagnostic(&format!("invalid edge entry key: {e}")))
    })?;
    let anchor_id = anchor_id_from_dto(dto.anchor_id)?;
    Ok(TestObligationEdgeId::new(entry_key, anchor_id))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use domain::tddd::semantic_verify::CatalogueEntryKey;
    use domain::tddd::test_obligation::ids::{
        TestObligationAnchorId, TestObligationId, TestObligationItemIdentifier,
    };
    use domain::tddd::test_obligation::vocab::TestObligationKind;

    fn location() -> TestLocation {
        TestLocation::new(
            LayerId::try_new("domain").unwrap(),
            TestModulePath::try_new("domain::user::tests".to_owned()).unwrap(),
            TestFunctionName::try_new("test_rejects_empty".to_owned()).unwrap(),
        )
    }

    fn obligation_id() -> TestObligationId {
        TestObligationId::new(
            CatalogueEntryKey::try_new("domain::User".to_owned()).unwrap(),
            TestObligationKind::Boundary,
            TestObligationItemIdentifier::try_new("invariant:non_empty".to_owned()).unwrap(),
        )
    }

    fn edge_id() -> TestObligationEdgeId {
        TestObligationEdgeId::new(
            CatalogueEntryKey::try_new("domain::User".to_owned()).unwrap(),
            TestObligationAnchorId::try_new("spec.json".to_owned(), "IN-06".to_owned()).unwrap(),
        )
    }

    fn tests() -> NonEmptyTestLocations {
        NonEmptyTestLocations::try_new(vec![location()]).unwrap()
    }

    fn document(records: Vec<TestBindingRecord>) -> TestBindingsDocument {
        TestBindingsDocument::new(TrackId::try_new("my-track").unwrap(), records)
    }

    fn round_trip(record: TestBindingRecord) {
        let doc = document(vec![record]);
        let dto = TestBindingsDocumentDto::from_domain(&doc);
        let json = serde_json::to_string_pretty(&dto).unwrap();
        let decoded: TestBindingsDocumentDto = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.into_domain().unwrap(), doc);
    }

    // IN-06: the fulfillment form round-trips through JSON.
    #[test]
    fn test_fulfillment_record_round_trips() {
        round_trip(TestBindingRecord::Fulfillment {
            obligation_id: obligation_id(),
            tests: tests(),
        });
    }

    // IN-06 / AC-11: the waiver form round-trips through JSON.
    #[test]
    fn test_waiver_record_round_trips() {
        round_trip(TestBindingRecord::Waiver {
            edge_id: edge_id(),
            reason: WaivedReason::try_new("covered by integration suite".to_owned()).unwrap(),
        });
    }

    // IN-06 / AC-12: the voluntary-binding form round-trips through JSON.
    #[test]
    fn test_voluntary_binding_record_round_trips() {
        round_trip(TestBindingRecord::VoluntaryBinding { edge_id: edge_id(), tests: tests() });
    }

    // IN-06: an empty document is valid and round-trips.
    #[test]
    fn test_empty_document_round_trips() {
        let doc = document(vec![]);
        let dto = TestBindingsDocumentDto::from_domain(&doc);
        assert_eq!(dto.into_domain().unwrap(), doc);
    }

    // IN-06 fail-closed: a bound variant with an empty `tests` array is rejected.
    #[test]
    fn test_empty_tests_is_domain_invariant() {
        let json = r#"{
            "track_id": "my-track",
            "records": [
                { "kind": "fulfillment",
                  "obligation_id": { "entry_key": "domain::User", "obligation_kind": "boundary", "item_identifier": "invariant:non_empty" },
                  "tests": [] }
            ]
        }"#;
        let dto: TestBindingsDocumentDto = serde_json::from_str(json).unwrap();
        assert!(matches!(dto.into_domain(), Err(ArtifactCodecError::DomainInvariant(_))));
    }

    // IN-06 fail-closed: an unknown field is rejected by deny_unknown_fields.
    #[test]
    fn test_unknown_field_is_rejected() {
        let json = r#"{ "track_id": "t", "records": [], "extra": true }"#;
        assert!(serde_json::from_str::<TestBindingsDocumentDto>(json).is_err());
    }

    // IN-06 / AC-04: the codec persists and reloads a document via the port.
    #[test]
    fn test_codec_save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let codec = JsonTestBindingsCodec::new(dir.path().to_path_buf());
        let doc = document(vec![
            TestBindingRecord::Fulfillment { obligation_id: obligation_id(), tests: tests() },
            TestBindingRecord::Waiver {
                edge_id: edge_id(),
                reason: WaivedReason::try_new("covered elsewhere".to_owned()).unwrap(),
            },
        ]);
        codec.save(&doc).unwrap();
        assert_eq!(codec.load(doc.track_id()).unwrap(), Some(doc));
    }

    #[test]
    fn test_codec_load_is_read_only_for_check_gate_artifacts() {
        let dir = tempfile::tempdir().unwrap();
        let codec = JsonTestBindingsCodec::new(dir.path().to_path_buf());
        let doc = document(vec![TestBindingRecord::Fulfillment {
            obligation_id: obligation_id(),
            tests: tests(),
        }]);
        codec.save(&doc).unwrap();
        let artifact = dir.path().join(doc.track_id().as_ref()).join(BINDINGS_ARTIFACT);
        let before = std::fs::read(&artifact).unwrap();

        let loaded = codec.load(doc.track_id()).unwrap();

        assert_eq!(loaded, Some(doc));
        assert_eq!(std::fs::read(&artifact).unwrap(), before);
    }

    // IN-06 / CN-04: the trusted items root itself must not be a symlink.
    #[cfg(unix)]
    #[test]
    fn test_codec_symlinked_items_root_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let real_items = dir.path().join("real-items");
        let link_items = dir.path().join("items-link");
        std::fs::create_dir_all(&real_items).unwrap();
        std::os::unix::fs::symlink(&real_items, &link_items).unwrap();

        let codec = JsonTestBindingsCodec::new(link_items);
        let doc = document(vec![TestBindingRecord::Fulfillment {
            obligation_id: obligation_id(),
            tests: tests(),
        }]);
        match codec.load(doc.track_id()) {
            Err(ArtifactCodecError::Io(message)) => {
                assert!(message.as_str().contains("symlinked track items root"));
            }
            other => panic!("expected symlinked items root load error, got {other:?}"),
        }
        let save_error = codec.save(&doc).unwrap_err();
        assert!(save_error.as_str().contains("symlinked track items root"));
    }

    // IN-14 / AC-10: a missing artifact loads as `None`.
    #[test]
    fn test_load_missing_artifact_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let codec = JsonTestBindingsCodec::new(dir.path().to_path_buf());
        assert!(codec.load(&TrackId::try_new("absent-track").unwrap()).unwrap().is_none());
    }

    // IN-06 / CN-04: a symlinked *ancestor* of the items root (not just the leaf)
    // must be rejected — otherwise `lstat(items_dir)` follows the parent symlink
    // and returns the redirected leaf's metadata, letting the codec read / write
    // outside the worktree.
    #[cfg(unix)]
    #[test]
    fn test_codec_symlinked_items_root_ancestor_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let real_track = dir.path().join("real-track");
        std::fs::create_dir_all(real_track.join("items")).unwrap();
        let track_link = dir.path().join("track-link");
        std::os::unix::fs::symlink(&real_track, &track_link).unwrap();

        let items_via_link = track_link.join("items");
        let codec = JsonTestBindingsCodec::new(items_via_link);
        let doc = document(vec![TestBindingRecord::Fulfillment {
            obligation_id: obligation_id(),
            tests: tests(),
        }]);

        match codec.load(doc.track_id()) {
            Err(ArtifactCodecError::Io(message)) => {
                assert!(message.as_str().contains("symlinked track items root"));
            }
            other => panic!("expected symlinked ancestor load error, got {other:?}"),
        }
        let save_error = codec.save(&doc).unwrap_err();
        assert!(save_error.as_str().contains("symlinked track items root"));
    }

    // CN-04: a test-bindings artifact whose embedded `track_id` disagrees with
    // the requested id must not be returned — a copy from another track would
    // otherwise satisfy or block the gate on unrelated data.
    #[test]
    fn test_codec_load_rejects_track_id_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let codec = JsonTestBindingsCodec::new(dir.path().to_path_buf());
        let doc = document(vec![TestBindingRecord::Fulfillment {
            obligation_id: obligation_id(),
            tests: tests(),
        }]);
        codec.save(&doc).unwrap();

        let other_track = TrackId::try_new("other-track").unwrap();
        let source = dir.path().join(doc.track_id().as_ref()).join(BINDINGS_ARTIFACT);
        let target_dir = dir.path().join(other_track.as_ref());
        std::fs::create_dir_all(&target_dir).unwrap();
        std::fs::rename(&source, target_dir.join(BINDINGS_ARTIFACT)).unwrap();

        match codec.load(&other_track) {
            Err(ArtifactCodecError::DomainInvariant(message)) => {
                let text = message.as_str();
                assert!(text.contains("test-bindings artifact track id mismatch"), "got: {text}");
                assert!(text.contains(other_track.as_ref()), "got: {text}");
                assert!(text.contains(doc.track_id().as_ref()), "got: {text}");
            }
            other => panic!("expected DomainInvariant track-id mismatch error, got {other:?}"),
        }
    }
}
