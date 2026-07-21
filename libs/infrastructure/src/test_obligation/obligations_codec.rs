//! JSON codec for the derived obligations artifact.
//!
//! Serialises the domain [`ObligationsDocument`] to a track-scoped
//! `obligations.json` and validates it back, enforcing the fail-closed
//! decode-time contract (IN-05 / IN-07 / AC-03 / CN-01 / CN-11): unknown fields,
//! malformed content hashes, and empty validated newtypes are each reported with
//! a precise [`ObligationsCodecError`].
//!
//! Obligation identity ([`TestObligationId`]) is derived purely from the entry
//! key, obligation kind, and item identifier — content hashes and positional
//! indices are excluded (ADR D9 / CN-01), so an upstream declaration change keeps
//! the id stable and surfaces only as a freshness drift downstream.

use std::path::PathBuf;
use std::str::FromStr;

use domain::ContentHash;
use domain::TrackId;
use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole, FunctionRole};
use domain::tddd::semantic_verify::CatalogueEntryKey;
use domain::tddd::test_obligation::errors::ArtifactCodecError;
use domain::tddd::test_obligation::hashes::DeclarationHash;
use domain::tddd::test_obligation::ids::{
    DiagnosticMessage, TestObligationAnchorId, TestObligationBrief, TestObligationId,
    TestObligationItemIdentifier,
};
use domain::tddd::test_obligation::obligations::{ObligationsDocument, TestObligation};
use domain::tddd::test_obligation::ports::ObligationsArtifactPort;
use domain::tddd::test_obligation::vocab::{
    TargetEntryRoleKind, TestObligationKind, TestObligationPatternKind,
};
use serde::{Deserialize, Serialize};

use crate::tddd::semantic_verify_codec::CatalogueEntryRefDto;
use crate::test_obligation::{diagnostic, reject_symlinked_items_root};
use crate::track::symlink_guard::reject_symlinks_below;

/// Artifact filename for the derived obligations document.
const OBLIGATIONS_ARTIFACT: &str = "obligations.json";

/// Infrastructure-local alias for the artifact codec error surfaced by
/// [`JsonObligationsCodec`] (IN-05).
pub type ObligationsCodecError = ArtifactCodecError;

// ---------------------------------------------------------------------------
// DTOs
// ---------------------------------------------------------------------------

/// Serde DTO for [`TestObligationKind`] (IN-05 / CN-10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestObligationKindDto {
    /// Boundary tests for an invariant.
    Boundary,
    /// Invariant-preservation test across a mutating operation.
    InvariantPreservation,
    /// Event-emission test for a declared `emits` event.
    EventEmission,
    /// Logic-result test for a domain-service method.
    LogicResult,
    /// Predicate test covering both branches.
    PredicateBothBranches,
    /// Construction-result test for a factory.
    ConstructionResult,
    /// Result test for a use-case / application-service entry.
    Result,
    /// Reaction test for an event policy.
    Reaction,
    /// Transition test for a typestate transition.
    Transition,
    /// Contract test for a port trait method.
    Contract,
    /// Contract-conformance test for a trait implementation.
    ContractConformance,
    /// Generic logic test for a free function.
    Logic,
}

/// Serde DTO for [`TestObligationId`] (IN-05 / CN-01).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestObligationIdDto {
    entry_key: String,
    obligation_kind: TestObligationKindDto,
    item_identifier: String,
}

/// Serde DTO for [`TestObligationAnchorId`] (IN-06).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestObligationAnchorIdDto {
    file_path: String,
    element_id: String,
}

/// Wire form of the target-section discriminant [`TargetEntryRoleKind`].
///
/// Private helper: only the role discriminant is meaningful for an obligation's
/// target section, so each variant carries the payload-free role name (the domain
/// projection re-hydrates a placeholder-payload role, matching the rules codec).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "role", rename_all = "snake_case", deny_unknown_fields)]
enum TargetEntryRoleWire {
    DataRole(String),
    ContractRole(String),
    FunctionRole(String),
    TraitImpl(String),
    Pattern(String),
}

/// Serde DTO for [`TestObligation`] (IN-05).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestObligationDto {
    id: TestObligationIdDto,
    target_entry: CatalogueEntryRefDto,
    target_role: TargetEntryRoleWire,
    brief: String,
    declaration_hash: String,
    spec_refs: Vec<TestObligationAnchorIdDto>,
}

/// Serde DTO for [`ObligationsDocument`] (IN-05).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObligationsDocumentDto {
    track_id: String,
    obligations: Vec<TestObligationDto>,
}

impl ObligationsDocumentDto {
    /// Projects a domain [`ObligationsDocument`] onto its wire DTO.
    #[must_use]
    pub fn from_domain(doc: &ObligationsDocument) -> Self {
        Self {
            track_id: doc.track_id().as_ref().to_owned(),
            obligations: doc.obligations().iter().map(obligation_to_dto).collect(),
        }
    }

    /// Projects the wire DTO back onto a domain [`ObligationsDocument`].
    ///
    /// # Errors
    ///
    /// Returns an [`ObligationsCodecError`] when the `track_id`, a content hash,
    /// or a validated newtype fails its domain invariant.
    pub fn into_domain(self) -> Result<ObligationsDocument, ObligationsCodecError> {
        let track_id = TrackId::try_new(self.track_id.clone()).map_err(|e| {
            ArtifactCodecError::DomainInvariant(diagnostic(&format!(
                "invalid track id '{}': {e}",
                self.track_id
            )))
        })?;
        let mut obligations = Vec::with_capacity(self.obligations.len());
        for dto in self.obligations {
            obligations.push(obligation_from_dto(dto)?);
        }
        Ok(ObligationsDocument::new(track_id, obligations))
    }
}

// ---------------------------------------------------------------------------
// Codec adapter
// ---------------------------------------------------------------------------

/// JSON codec adapter for [`ObligationsArtifactPort`] (IN-05 / IN-07 / AC-03).
#[derive(Debug, Clone)]
pub struct JsonObligationsCodec {
    items_dir: PathBuf,
}

impl JsonObligationsCodec {
    /// Creates a codec that resolves artifacts under `items_dir` (the track
    /// items root, e.g. `track/items`).
    #[must_use]
    pub fn new(items_dir: PathBuf) -> Self {
        Self { items_dir }
    }

    fn artifact_path(&self, track_id: &TrackId) -> PathBuf {
        self.items_dir.join(track_id.as_ref()).join(OBLIGATIONS_ARTIFACT)
    }
}

impl ObligationsArtifactPort for JsonObligationsCodec {
    fn load(&self, track_id: &TrackId) -> Result<Option<ObligationsDocument>, ArtifactCodecError> {
        reject_symlinked_items_root(&self.items_dir).map_err(|source| {
            ArtifactCodecError::Io(diagnostic(&format!(
                "refusing to read obligations artifact under {}: {source}",
                self.items_dir.display()
            )))
        })?;
        let path = self.artifact_path(track_id);
        match reject_symlinks_below(&path, &self.items_dir) {
            Ok(true) => {}
            Ok(false) => return Ok(None),
            Err(source) => {
                return Err(ArtifactCodecError::Io(diagnostic(&format!(
                    "refusing to read obligations artifact {}: {source}",
                    path.display()
                ))));
            }
        }
        let content = std::fs::read_to_string(&path).map_err(|e| {
            ArtifactCodecError::Io(diagnostic(&format!(
                "failed to read obligations artifact {}: {e}",
                path.display()
            )))
        })?;
        let dto: ObligationsDocumentDto = serde_json::from_str(&content)
            .map_err(|e| ArtifactCodecError::MalformedJson(diagnostic(&e.to_string())))?;
        let doc = dto.into_domain()?;
        // Fail closed when the on-disk artifact was copied from another track: a
        // matching filename is not proof of matching content, and the caller
        // trusts a `load(track_id)` result to describe exactly that track.
        if doc.track_id() != track_id {
            return Err(ArtifactCodecError::DomainInvariant(diagnostic(&format!(
                "obligations artifact track id mismatch: requested '{}', got '{}'",
                track_id.as_ref(),
                doc.track_id().as_ref()
            ))));
        }
        Ok(Some(doc))
    }

    fn save(&self, doc: &ObligationsDocument) -> Result<(), DiagnosticMessage> {
        reject_symlinked_items_root(&self.items_dir).map_err(|source| {
            diagnostic(&format!(
                "refusing to write obligations artifact under {}: {source}",
                self.items_dir.display()
            ))
        })?;
        let path = self.artifact_path(doc.track_id());
        let Some(parent) = path.parent() else {
            return Err(diagnostic(&format!(
                "obligations artifact path {} has no parent directory",
                path.display()
            )));
        };
        std::fs::create_dir_all(parent).map_err(|e| {
            diagnostic(&format!("failed to create track directory {}: {e}", parent.display()))
        })?;
        reject_symlinked_items_root(&self.items_dir).map_err(|source| {
            diagnostic(&format!(
                "refusing to write obligations artifact under {}: {source}",
                self.items_dir.display()
            ))
        })?;
        if let Err(source) = reject_symlinks_below(&path, &self.items_dir) {
            return Err(diagnostic(&format!(
                "refusing to write obligations artifact {}: {source}",
                path.display()
            )));
        }
        let dto = ObligationsDocumentDto::from_domain(doc);
        let json = serde_json::to_string_pretty(&dto)
            .map_err(|e| diagnostic(&format!("failed to encode obligations artifact: {e}")))?;
        std::fs::write(&path, json).map_err(|e| {
            diagnostic(&format!("failed to write obligations artifact {}: {e}", path.display()))
        })
    }
}

// ---------------------------------------------------------------------------
// Conversion helpers (private)
// ---------------------------------------------------------------------------

fn obligation_to_dto(obligation: &TestObligation) -> TestObligationDto {
    TestObligationDto {
        id: obligation_id_to_dto(obligation.id()),
        target_entry: CatalogueEntryRefDto::from_domain(obligation.target_entry()),
        target_role: target_role_to_wire(obligation.target_role()),
        brief: obligation.brief().as_str().to_owned(),
        declaration_hash: obligation.declaration_hash().as_hash().to_hex(),
        spec_refs: obligation.spec_refs().iter().map(anchor_id_to_dto).collect(),
    }
}

fn obligation_from_dto(dto: TestObligationDto) -> Result<TestObligation, ObligationsCodecError> {
    let id = obligation_id_from_dto(dto.id)?;
    let target_entry = dto.target_entry.into_domain().map_err(|e| {
        ArtifactCodecError::DomainInvariant(diagnostic(&format!(
            "invalid target entry reference: {e}"
        )))
    })?;
    let target_role = target_role_from_wire(dto.target_role)?;
    let brief = TestObligationBrief::try_new(dto.brief).map_err(|e| {
        ArtifactCodecError::DomainInvariant(diagnostic(&format!("invalid obligation brief: {e}")))
    })?;
    let declaration_hash = DeclarationHash::new(parse_hash(&dto.declaration_hash)?);
    let mut spec_refs = Vec::with_capacity(dto.spec_refs.len());
    for anchor in dto.spec_refs {
        spec_refs.push(anchor_id_from_dto(anchor)?);
    }
    Ok(TestObligation::new(id, target_entry, target_role, brief, declaration_hash, spec_refs))
}

pub(crate) fn obligation_id_to_dto(id: &TestObligationId) -> TestObligationIdDto {
    TestObligationIdDto {
        entry_key: id.entry_key().as_str().to_owned(),
        obligation_kind: kind_to_dto(id.obligation_kind()),
        item_identifier: id.item_identifier().as_str().to_owned(),
    }
}

pub(crate) fn obligation_id_from_dto(
    dto: TestObligationIdDto,
) -> Result<TestObligationId, ObligationsCodecError> {
    let entry_key = CatalogueEntryKey::try_new(dto.entry_key).map_err(|e| {
        ArtifactCodecError::DomainInvariant(diagnostic(&format!("invalid entry key: {e}")))
    })?;
    let item_identifier =
        TestObligationItemIdentifier::try_new(dto.item_identifier).map_err(|e| {
            ArtifactCodecError::DomainInvariant(diagnostic(&format!(
                "invalid item identifier: {e}"
            )))
        })?;
    Ok(TestObligationId::new(entry_key, kind_from_dto(dto.obligation_kind), item_identifier))
}

pub(crate) fn anchor_id_to_dto(anchor: &TestObligationAnchorId) -> TestObligationAnchorIdDto {
    TestObligationAnchorIdDto {
        file_path: anchor.file_path().to_owned(),
        element_id: anchor.element_id().to_owned(),
    }
}

pub(crate) fn anchor_id_from_dto(
    dto: TestObligationAnchorIdDto,
) -> Result<TestObligationAnchorId, ObligationsCodecError> {
    TestObligationAnchorId::try_new(dto.file_path, dto.element_id).map_err(|e| {
        ArtifactCodecError::DomainInvariant(diagnostic(&format!("invalid anchor id: {e}")))
    })
}

fn parse_hash(hex: &str) -> Result<ContentHash, ObligationsCodecError> {
    ContentHash::try_from_hex(hex).map_err(|e| {
        ArtifactCodecError::DomainInvariant(diagnostic(&format!(
            "invalid declaration hash '{hex}': {e}"
        )))
    })
}

fn kind_to_dto(kind: &TestObligationKind) -> TestObligationKindDto {
    match kind {
        TestObligationKind::Boundary => TestObligationKindDto::Boundary,
        TestObligationKind::InvariantPreservation => TestObligationKindDto::InvariantPreservation,
        TestObligationKind::EventEmission => TestObligationKindDto::EventEmission,
        TestObligationKind::LogicResult => TestObligationKindDto::LogicResult,
        TestObligationKind::PredicateBothBranches => TestObligationKindDto::PredicateBothBranches,
        TestObligationKind::ConstructionResult => TestObligationKindDto::ConstructionResult,
        TestObligationKind::Result => TestObligationKindDto::Result,
        TestObligationKind::Reaction => TestObligationKindDto::Reaction,
        TestObligationKind::Transition => TestObligationKindDto::Transition,
        TestObligationKind::Contract => TestObligationKindDto::Contract,
        TestObligationKind::ContractConformance => TestObligationKindDto::ContractConformance,
        TestObligationKind::Logic => TestObligationKindDto::Logic,
    }
}

fn kind_from_dto(dto: TestObligationKindDto) -> TestObligationKind {
    match dto {
        TestObligationKindDto::Boundary => TestObligationKind::Boundary,
        TestObligationKindDto::InvariantPreservation => TestObligationKind::InvariantPreservation,
        TestObligationKindDto::EventEmission => TestObligationKind::EventEmission,
        TestObligationKindDto::LogicResult => TestObligationKind::LogicResult,
        TestObligationKindDto::PredicateBothBranches => TestObligationKind::PredicateBothBranches,
        TestObligationKindDto::ConstructionResult => TestObligationKind::ConstructionResult,
        TestObligationKindDto::Result => TestObligationKind::Result,
        TestObligationKindDto::Reaction => TestObligationKind::Reaction,
        TestObligationKindDto::Transition => TestObligationKind::Transition,
        TestObligationKindDto::Contract => TestObligationKind::Contract,
        TestObligationKindDto::ContractConformance => TestObligationKind::ContractConformance,
        TestObligationKindDto::Logic => TestObligationKind::Logic,
    }
}

fn target_role_to_wire(role: &TargetEntryRoleKind) -> TargetEntryRoleWire {
    match role {
        TargetEntryRoleKind::DataRole(r) => {
            TargetEntryRoleWire::DataRole(r.variant_name().to_owned())
        }
        TargetEntryRoleKind::ContractRole(r) => {
            TargetEntryRoleWire::ContractRole(r.variant_name().to_owned())
        }
        TargetEntryRoleKind::FunctionRole(r) => TargetEntryRoleWire::FunctionRole(r.to_string()),
        TargetEntryRoleKind::TraitImpl(r) => {
            TargetEntryRoleWire::TraitImpl(r.variant_name().to_owned())
        }
        TargetEntryRoleKind::Pattern(p) => {
            TargetEntryRoleWire::Pattern(pattern_kind_as_str(p).to_owned())
        }
    }
}

fn target_role_from_wire(
    wire: TargetEntryRoleWire,
) -> Result<TargetEntryRoleKind, ObligationsCodecError> {
    let role = match wire {
        TargetEntryRoleWire::DataRole(s) => {
            TargetEntryRoleKind::DataRole(DataRole::from_str(&s).map_err(|_| unknown_role(&s))?)
        }
        TargetEntryRoleWire::ContractRole(s) => TargetEntryRoleKind::ContractRole(
            ContractRole::from_str(&s).map_err(|_| unknown_role(&s))?,
        ),
        TargetEntryRoleWire::FunctionRole(s) => TargetEntryRoleKind::FunctionRole(
            FunctionRole::from_str(&s).map_err(|_| unknown_role(&s))?,
        ),
        TargetEntryRoleWire::TraitImpl(s) => TargetEntryRoleKind::TraitImpl(
            ContractRole::from_str(&s).map_err(|_| unknown_role(&s))?,
        ),
        TargetEntryRoleWire::Pattern(s) => {
            TargetEntryRoleKind::Pattern(pattern_kind_from_str(&s).ok_or_else(|| unknown_role(&s))?)
        }
    };
    Ok(role)
}

fn pattern_kind_as_str(pattern: &TestObligationPatternKind) -> &'static str {
    match pattern {
        TestObligationPatternKind::Typestate => "typestate",
    }
}

fn pattern_kind_from_str(value: &str) -> Option<TestObligationPatternKind> {
    match value {
        "typestate" => Some(TestObligationPatternKind::Typestate),
        _ => None,
    }
}

fn unknown_role(role: &str) -> ObligationsCodecError {
    ArtifactCodecError::DomainInvariant(diagnostic(&format!("unknown target role '{role}'")))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use domain::tddd::semantic_verify::{CatalogueEntryRef, CatalogueSectionKey};

    fn sample_obligation() -> TestObligation {
        let entry_key = CatalogueEntryKey::try_new("domain::User".to_owned()).unwrap();
        let id = TestObligationId::new(
            entry_key.clone(),
            TestObligationKind::Boundary,
            TestObligationItemIdentifier::try_new("invariant:non_empty".to_owned()).unwrap(),
        );
        let target_entry = CatalogueEntryRef::new(
            "track/items/x/domain-types.json".to_owned(),
            CatalogueSectionKey::Types,
            entry_key,
        );
        TestObligation::new(
            id,
            target_entry,
            TargetEntryRoleKind::DataRole(DataRole::value_object()),
            TestObligationBrief::try_new("cover empty-name rejection".to_owned()).unwrap(),
            DeclarationHash::new(ContentHash::from_bytes([3u8; 32])),
            vec![
                TestObligationAnchorId::try_new("spec.json".to_owned(), "IN-05".to_owned())
                    .unwrap(),
            ],
        )
    }

    fn sample_document() -> ObligationsDocument {
        ObligationsDocument::new(TrackId::try_new("my-track").unwrap(), vec![sample_obligation()])
    }

    // IN-05 / AC-03: a populated obligations document round-trips through JSON.
    #[test]
    fn test_obligations_document_round_trips_through_json() {
        let doc = sample_document();
        let dto = ObligationsDocumentDto::from_domain(&doc);
        let json = serde_json::to_string_pretty(&dto).unwrap();
        let decoded: ObligationsDocumentDto = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.into_domain().unwrap(), doc);
    }

    // IN-05: an empty document is valid and round-trips.
    #[test]
    fn test_empty_obligations_document_round_trips() {
        let doc = ObligationsDocument::new(TrackId::try_new("empty-track").unwrap(), vec![]);
        let dto = ObligationsDocumentDto::from_domain(&doc);
        assert_eq!(dto.into_domain().unwrap(), doc);
    }

    // CN-01: the obligation id is preserved by (entry key, kind, item id) only.
    #[test]
    fn test_obligation_id_identity_is_preserved() {
        let doc = sample_document();
        let dto = ObligationsDocumentDto::from_domain(&doc);
        let decoded = dto.into_domain().unwrap();
        let id = decoded.obligations().first().unwrap().id();
        assert_eq!(id.entry_key().as_str(), "domain::User");
        assert_eq!(id.obligation_kind(), &TestObligationKind::Boundary);
        assert_eq!(id.item_identifier().as_str(), "invariant:non_empty");
    }

    // IN-05: the shared semantic-verify DTO preserves every catalogue section
    // vocabulary value used to locate an obligation target.
    #[test]
    fn test_catalogue_entry_ref_dto_round_trips_each_section_key() {
        for (section_key, expected_wire) in [
            (CatalogueSectionKey::Types, "types"),
            (CatalogueSectionKey::Traits, "traits"),
            (CatalogueSectionKey::Functions, "functions"),
        ] {
            let entry = CatalogueEntryRef::new(
                "track/items/x/domain-types.json".to_owned(),
                section_key,
                CatalogueEntryKey::try_new("domain::User".to_owned()).unwrap(),
            );

            let dto = CatalogueEntryRefDto::from_domain(&entry);
            let wire = serde_json::to_value(&dto).unwrap();

            assert_eq!(wire["section_key"], expected_wire);
            assert_eq!(dto.into_domain().unwrap(), entry);
        }
    }

    // IN-05: the obligation-kind DTO exposes the complete closed wire
    // vocabulary and maps every value back to its domain counterpart.
    #[test]
    fn test_obligation_kind_dto_round_trips_every_variant() {
        for (kind, expected_wire) in [
            (TestObligationKind::Boundary, "boundary"),
            (TestObligationKind::InvariantPreservation, "invariant_preservation"),
            (TestObligationKind::EventEmission, "event_emission"),
            (TestObligationKind::LogicResult, "logic_result"),
            (TestObligationKind::PredicateBothBranches, "predicate_both_branches"),
            (TestObligationKind::ConstructionResult, "construction_result"),
            (TestObligationKind::Result, "result"),
            (TestObligationKind::Reaction, "reaction"),
            (TestObligationKind::Transition, "transition"),
            (TestObligationKind::Contract, "contract"),
            (TestObligationKind::ContractConformance, "contract_conformance"),
            (TestObligationKind::Logic, "logic"),
        ] {
            let dto = kind_to_dto(&kind);
            assert_eq!(serde_json::to_string(&dto).unwrap(), format!("\"{expected_wire}\""));
            assert_eq!(kind_from_dto(dto), kind);
        }
    }

    // IN-05: every target-role section variant round-trips through the wire form.
    #[test]
    fn test_all_target_role_variants_round_trip() {
        let roles = vec![
            TargetEntryRoleKind::DataRole(DataRole::value_object()),
            TargetEntryRoleKind::ContractRole(ContractRole::SecondaryPort),
            TargetEntryRoleKind::FunctionRole(FunctionRole::FreeFunction),
            TargetEntryRoleKind::TraitImpl(ContractRole::ApplicationService),
            TargetEntryRoleKind::Pattern(TestObligationPatternKind::Typestate),
        ];
        for role in roles {
            let wire = target_role_to_wire(&role);
            assert_eq!(target_role_from_wire(wire).unwrap(), role);
        }
    }

    // IN-05 fail-closed: an unknown field is rejected by deny_unknown_fields.
    #[test]
    fn test_unknown_field_is_rejected() {
        let json = r#"{ "track_id": "t", "obligations": [], "extra": true }"#;
        assert!(serde_json::from_str::<ObligationsDocumentDto>(json).is_err());
    }

    // IN-05 fail-closed: a malformed declaration hash is a domain-invariant error.
    #[test]
    fn test_malformed_declaration_hash_is_domain_invariant() {
        let doc = sample_document();
        let mut dto = ObligationsDocumentDto::from_domain(&doc);
        dto.obligations[0].declaration_hash = "not-a-hash".to_owned();
        match dto.into_domain() {
            Err(ArtifactCodecError::DomainInvariant(message)) => {
                assert!(message.as_str().contains("declaration hash"));
            }
            other => panic!("expected DomainInvariant, got {other:?}"),
        }
    }

    // IN-05 fail-closed: an empty brief violates a domain invariant.
    #[test]
    fn test_empty_brief_is_domain_invariant() {
        let doc = sample_document();
        let mut dto = ObligationsDocumentDto::from_domain(&doc);
        dto.obligations[0].brief = "   ".to_owned();
        assert!(matches!(dto.into_domain(), Err(ArtifactCodecError::DomainInvariant(_))));
    }

    // IN-05 / AC-03: the codec persists and reloads a document via the port.
    #[test]
    fn test_codec_save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let codec = JsonObligationsCodec::new(dir.path().to_path_buf());
        let doc = sample_document();
        codec.save(&doc).unwrap();
        let loaded = codec.load(doc.track_id()).unwrap();
        assert_eq!(loaded, Some(doc));
    }

    // IN-05 / CN-04: the trusted items root itself must not be a symlink.
    #[cfg(unix)]
    #[test]
    fn test_codec_symlinked_items_root_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let real_items = dir.path().join("real-items");
        let link_items = dir.path().join("items-link");
        std::fs::create_dir_all(&real_items).unwrap();
        std::os::unix::fs::symlink(&real_items, &link_items).unwrap();

        let codec = JsonObligationsCodec::new(link_items);
        let doc = sample_document();
        match codec.load(doc.track_id()) {
            Err(ArtifactCodecError::Io(message)) => {
                assert!(message.as_str().contains("symlinked track items root"));
            }
            other => panic!("expected symlinked items root load error, got {other:?}"),
        }
        let save_error = codec.save(&doc).unwrap_err();
        assert!(save_error.as_str().contains("symlinked track items root"));
    }

    // IN-14 / AC-10: a missing artifact loads as `None` (zero-pairs scope).
    #[test]
    fn test_load_missing_artifact_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let codec = JsonObligationsCodec::new(dir.path().to_path_buf());
        let loaded = codec.load(&TrackId::try_new("absent-track").unwrap()).unwrap();
        assert!(loaded.is_none());
    }

    // IN-05 / CN-04: a symlinked *ancestor* of the items root (not just the leaf)
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

        // items_dir points at a real directory *through* the symlinked parent.
        let items_via_link = track_link.join("items");
        let codec = JsonObligationsCodec::new(items_via_link);
        let doc = sample_document();

        match codec.load(doc.track_id()) {
            Err(ArtifactCodecError::Io(message)) => {
                assert!(message.as_str().contains("symlinked track items root"));
            }
            other => panic!("expected symlinked ancestor load error, got {other:?}"),
        }
        let save_error = codec.save(&doc).unwrap_err();
        assert!(save_error.as_str().contains("symlinked track items root"));
    }

    // CN-04: an obligations artifact whose embedded `track_id` disagrees with the
    // requested id must not be returned — a copy from another track would
    // otherwise satisfy or block the gate on unrelated data.
    #[test]
    fn test_codec_load_rejects_track_id_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let codec = JsonObligationsCodec::new(dir.path().to_path_buf());
        // Save under the document's own track id.
        let doc = sample_document();
        codec.save(&doc).unwrap();

        // Move the saved artifact into a different track's directory so the
        // requested id does not match the document body.
        let other_track = TrackId::try_new("other-track").unwrap();
        let source = dir.path().join(doc.track_id().as_ref()).join(OBLIGATIONS_ARTIFACT);
        let target_dir = dir.path().join(other_track.as_ref());
        std::fs::create_dir_all(&target_dir).unwrap();
        std::fs::rename(&source, target_dir.join(OBLIGATIONS_ARTIFACT)).unwrap();

        match codec.load(&other_track) {
            Err(ArtifactCodecError::DomainInvariant(message)) => {
                let text = message.as_str();
                assert!(text.contains("obligations artifact track id mismatch"), "got: {text}");
                assert!(text.contains(other_track.as_ref()), "got: {text}");
                assert!(text.contains(doc.track_id().as_ref()), "got: {text}");
            }
            other => panic!("expected DomainInvariant track-id mismatch error, got {other:?}"),
        }
    }
}
