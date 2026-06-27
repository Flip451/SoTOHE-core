//! Semantic review cache value objects for SoT Chain integrity gates.
//!
//! Defines the domain types used by the two semantic-review cache documents:
//!
//! - [`SpecAdrVerifyCacheDocument`] — Chain-1 (`spec.json` → ADR), one file per
//!   track.
//! - [`CatalogueSpecVerifyCacheDocument`] — Chain-2 (`<layer>-types.json` →
//!   `spec.json`), one file per layer.
//!
//! Each cache document stores a list of [`SemanticVerifyEntry`] records, where
//! each entry pairs a `claim_hash` with an `evidence_hash` and a frozen
//! [`SemanticVerdict`].  When either hash changes the entry must be re-reviewed.
//!
//! All types are serde-free (hexagonal architecture: serialisation lives in the
//! infrastructure codec, task T004).

use std::fmt;

use crate::ValidationError;
use crate::plan_ref::ContentHash;
use crate::plan_ref::SpecElementId;
use crate::tddd::layer_id::LayerId;

// ── EvidenceCitation ──────────────────────────────────────────────────────────

/// Non-empty validated quotation from the evidence (ADR or spec element) that
/// backs a semantic review pass verdict (D6).
///
/// Required for any [`SemanticVerdict::Pass`] — absence forces
/// [`SemanticVerdict::Pending`].  Rejects empty strings and whitespace-only
/// strings at construction time, making "pass without citation" structurally
/// impossible.
///
/// # Errors
///
/// [`try_new`] returns [`ValidationError::EmptyString`] when `citation`
/// is empty or contains only whitespace (trimmed to empty).
///
/// [`try_new`]: EvidenceCitation::try_new
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceCitation(String);

impl EvidenceCitation {
    /// Validate and wrap `citation` as an [`EvidenceCitation`].
    ///
    /// The input is trimmed before the emptiness check, so whitespace-only
    /// strings are treated as empty.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::EmptyString`] when `citation` is empty or
    /// contains only whitespace characters.
    pub fn try_new(citation: String) -> Result<Self, ValidationError> {
        if citation.trim().is_empty() {
            Err(ValidationError::EmptyString)
        } else {
            Ok(Self(citation))
        }
    }

    /// Return the inner citation string as a `&str`.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EvidenceCitation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// ── SemanticVerdict ───────────────────────────────────────────────────────────

/// Result of a single semantic review of a (claim, evidence) reference pair.
///
/// [`Pass`] requires a citation quotation (D6). [`Pending`] means the reviewer
/// could not confirm — treated as [`Fail`] at the gate.
///
/// The `Pass { citation }` variant structurally enforces that a passing verdict
/// always carries evidence: it is impossible to construct a `Pass` without an
/// [`EvidenceCitation`].
///
/// [`Pass`]: SemanticVerdict::Pass
/// [`Fail`]: SemanticVerdict::Fail
/// [`Pending`]: SemanticVerdict::Pending
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticVerdict {
    /// The semantic claim is backed by the evidence.  `citation` is a
    /// non-empty quotation from the evidence that supports the claim.
    Pass {
        /// Verbatim quotation from the evidence document that supports the claim.
        citation: EvidenceCitation,
    },
    /// The semantic claim is not backed by the evidence.  `reason` describes
    /// why the claim was rejected.
    Fail {
        /// Human-readable description of the mismatch or contradiction.
        reason: String,
    },
    /// The reviewer was unable to confirm or deny the claim.  Treated as
    /// [`Fail`] at the gate level.
    ///
    /// [`Fail`]: SemanticVerdict::Fail
    Pending,
}

// ── SemanticVerifyEntry ───────────────────────────────────────────────────────

/// Single frozen verdict record in a semantic verify cache artifact.
///
/// Keyed by (`claim_hash`, `evidence_hash`): when either hash changes the
/// entry must be re-reviewed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticVerifyEntry {
    /// SHA-256 hash of the claim element (e.g. a spec element or catalogue
    /// entry subtree).
    pub claim_hash: ContentHash,
    /// SHA-256 hash of the evidence element (e.g. an ADR decision or spec
    /// element subtree).
    pub evidence_hash: ContentHash,
    /// Frozen verdict for this (claim, evidence) pair.
    pub verdict: SemanticVerdict,
    /// Origin reference identifying which artifact and location the claim came from.
    pub claim_origin: VerifyOriginRef,
    /// Origin reference identifying which artifact and location the evidence came from.
    pub evidence_origin: VerifyOriginRef,
}

impl SemanticVerifyEntry {
    /// Construct a new [`SemanticVerifyEntry`].
    pub fn new(
        claim_hash: ContentHash,
        evidence_hash: ContentHash,
        verdict: SemanticVerdict,
        claim_origin: VerifyOriginRef,
        evidence_origin: VerifyOriginRef,
    ) -> Self {
        Self { claim_hash, evidence_hash, verdict, claim_origin, evidence_origin }
    }
}

// ── ModelTier ─────────────────────────────────────────────────────────────────

/// Tier selection for the three-tier model escalation funnel (D5).
///
/// `Fast` selects the lightweight model (`fast_provider`/`fast_model` in
/// `agent-profiles.json`); `Final` selects the heavyweight model.
///
/// This type is independent of `infrastructure::agent_profiles::RoundType`
/// (CN-01): it lives in the domain layer and has its own semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelTier {
    /// Lightweight model for initial semantic review pass.
    Fast,
    /// Heavyweight model for final semantic review pass.
    Final,
}

// ── SpecSectionKind ───────────────────────────────────────────────────────────

/// Identifies which top-level section of spec.json a [`SpecElementRef`] points to.
///
/// Used as the `section` discriminant in [`SpecElementRef`] so origin-tracking
/// can locate the exact spec element without reparsing the file.
/// Distinct from the existing `domain::spec::SpecSection` struct (a free-form
/// additional section with title/content).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecSectionKind {
    /// The `goal` section of spec.json.
    Goal,
    /// The `in_scope` section of spec.json.
    InScope,
    /// The `out_of_scope` section of spec.json.
    OutOfScope,
    /// The `constraint` section of spec.json.
    Constraint,
    /// The `acceptance_criteria` section of spec.json.
    AcceptanceCriteria,
}

// ── CatalogueSectionKey ───────────────────────────────────────────────────────

/// Identifies which BTreeMap section of a `<layer>-types.json` catalogue file
/// a [`CatalogueEntryRef`] points to.
///
/// The three variants mirror the top-level `types`, `traits`, and `functions`
/// keys of the v5 catalogue schema.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogueSectionKey {
    /// The `types` section of the catalogue.
    Types,
    /// The `traits` section of the catalogue.
    Traits,
    /// The `functions` section of the catalogue.
    Functions,
}

// ── CatalogueEntryKey ─────────────────────────────────────────────────────────

/// Validated non-empty key for an entry in a catalogue section BTreeMap (a
/// type name, trait name, or function path).
///
/// Used as the `entry_key` field of [`CatalogueEntryRef`].
///
/// # Errors
///
/// [`try_new`] returns [`ValidationError::EmptyString`] when the trimmed value
/// is empty.
///
/// [`try_new`]: CatalogueEntryKey::try_new
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogueEntryKey(String);

impl CatalogueEntryKey {
    /// Validate and wrap `raw` as a [`CatalogueEntryKey`].
    ///
    /// Rejects empty strings and whitespace-only strings (trimmed to empty).
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::EmptyString`] when `raw` is empty or
    /// contains only whitespace characters.
    pub fn try_new(raw: String) -> Result<Self, ValidationError> {
        if raw.trim().is_empty() { Err(ValidationError::EmptyString) } else { Ok(Self(raw)) }
    }

    /// Return the inner key string as a `&str`.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// ── SpecElementRef ────────────────────────────────────────────────────────────

/// Origin reference to a single element in spec.json.
///
/// `section` identifies the top-level spec section ([`SpecSectionKind`]);
/// `element_id` is the element's validated id value (e.g. `IN-01`);
/// `text_label` is the verbatim text value (free text, no constraint).
/// Chain-1 claim origins and Chain-2 evidence origins are encoded as this type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecElementRef {
    /// Top-level spec section this element belongs to.
    pub section: SpecSectionKind,
    /// Validated identifier of the spec element (e.g. `IN-01`, `AC-03`).
    pub element_id: SpecElementId,
    /// Verbatim text of the spec element.
    pub text_label: String,
}

impl SpecElementRef {
    /// Construct a new [`SpecElementRef`].
    pub fn new(section: SpecSectionKind, element_id: SpecElementId, text_label: String) -> Self {
        Self { section, element_id, text_label }
    }
}

// ── AdrDecisionRef ────────────────────────────────────────────────────────────

/// Origin reference to a specific decision in an ADR file.
///
/// `file_path` is the project-relative path to the ADR file; stored as an
/// opaque string since path validity is checked at file-access time.
/// `decision_id` is the ADR decision anchor (e.g. `D1`); stored as an opaque
/// string since format validation occurs at ADR parse time.
/// Chain-1 evidence origins are encoded as this type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdrDecisionRef {
    /// Project-relative path to the ADR file.
    pub file_path: String,
    /// ADR decision anchor string (e.g. `D1`).
    pub decision_id: String,
}

impl AdrDecisionRef {
    /// Construct a new [`AdrDecisionRef`].
    pub fn new(file_path: String, decision_id: String) -> Self {
        Self { file_path, decision_id }
    }
}

// ── CatalogueEntryRef ─────────────────────────────────────────────────────────

/// Origin reference to a specific entry in a `<layer>-types.json` catalogue file.
///
/// `file_path` is the project-relative path to the catalogue file; stored as
/// an opaque string. `section_key` identifies which BTreeMap section
/// (Types/Traits/Functions). `entry_key` is the key in that BTreeMap.
/// Chain-2 claim origins are encoded as this type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogueEntryRef {
    /// Project-relative path to the catalogue file.
    pub file_path: String,
    /// Section of the catalogue this entry belongs to.
    pub section_key: CatalogueSectionKey,
    /// Validated non-empty key for this catalogue entry.
    pub entry_key: CatalogueEntryKey,
}

impl CatalogueEntryRef {
    /// Construct a new [`CatalogueEntryRef`].
    pub fn new(
        file_path: String,
        section_key: CatalogueSectionKey,
        entry_key: CatalogueEntryKey,
    ) -> Self {
        Self { file_path, section_key, entry_key }
    }
}

// ── VerifyOriginRef ───────────────────────────────────────────────────────────

/// Tagged origin reference identifying the artifact and location of a claim or
/// evidence in a [`SemanticVerifyEntry`].
///
/// Chain-1: claim=`SpecElement`, evidence=`AdrDecision`.
/// Chain-2: claim=`CatalogueEntry`, evidence=`SpecElement`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerifyOriginRef {
    /// Origin is a specific element in spec.json.
    SpecElement(SpecElementRef),
    /// Origin is a specific decision in an ADR file.
    AdrDecision(AdrDecisionRef),
    /// Origin is a specific entry in a `<layer>-types.json` catalogue.
    CatalogueEntry(CatalogueEntryRef),
}

// ── SpecAdrVerifyCacheDocument ────────────────────────────────────────────────

/// Aggregate root for `spec-adr-verify-cache.json`.
///
/// Stores frozen (`claim_hash`, `evidence_hash`) verdict pairs for Chain-1
/// (`spec.json` → ADR) semantic review at track granularity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecAdrVerifyCacheDocument {
    /// Frozen verdict entries for this track's spec → ADR pairs.
    pub entries: Vec<SemanticVerifyEntry>,
}

impl SpecAdrVerifyCacheDocument {
    /// Construct a new [`SpecAdrVerifyCacheDocument`].
    pub fn new(entries: Vec<SemanticVerifyEntry>) -> Self {
        Self { entries }
    }
}

// ── CatalogueSpecVerifyCacheDocument ─────────────────────────────────────────

/// Aggregate root for `<layer>-catalogue-spec-verify-cache.json`.
///
/// Stores frozen (`claim_hash`, `evidence_hash`) verdict pairs for Chain-2
/// (`<layer>-types.json` → `spec.json`) semantic review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogueSpecVerifyCacheDocument {
    /// Architecture layer this cache document corresponds to.
    pub layer: LayerId,
    /// Frozen verdict entries for this layer's catalogue → spec pairs.
    pub entries: Vec<SemanticVerifyEntry>,
}

impl CatalogueSpecVerifyCacheDocument {
    /// Construct a new [`CatalogueSpecVerifyCacheDocument`].
    pub fn new(layer: LayerId, entries: Vec<SemanticVerifyEntry>) -> Self {
        Self { layer, entries }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use crate::ValidationError;

    fn make_hash(byte: u8) -> ContentHash {
        ContentHash::from_bytes([byte; 32])
    }

    // ── EvidenceCitation ──────────────────────────────────────────────────

    #[test]
    fn test_evidence_citation_with_non_empty_string_succeeds() {
        let result = EvidenceCitation::try_new("The spec states X.".to_string());
        assert!(result.is_ok());
    }

    #[test]
    fn test_evidence_citation_with_empty_string_returns_error() {
        let err = EvidenceCitation::try_new(String::new()).unwrap_err();
        assert!(matches!(err, ValidationError::EmptyString));
    }

    #[test]
    fn test_evidence_citation_with_whitespace_only_returns_error() {
        let err = EvidenceCitation::try_new("   \t\n  ".to_string()).unwrap_err();
        assert!(matches!(err, ValidationError::EmptyString));
    }

    #[test]
    fn test_evidence_citation_as_str_returns_inner_value() {
        let citation = EvidenceCitation::try_new("some quote".to_string()).unwrap();
        assert_eq!(citation.as_str(), "some quote");
    }

    #[test]
    fn test_evidence_citation_display_matches_inner_string() {
        let citation = EvidenceCitation::try_new("quote text".to_string()).unwrap();
        assert_eq!(citation.to_string(), "quote text");
    }

    // ── SemanticVerdict ───────────────────────────────────────────────────

    #[test]
    fn test_semantic_verdict_pass_holds_citation() {
        let citation = EvidenceCitation::try_new("ADR §D6 states ...".to_string()).unwrap();
        let verdict = SemanticVerdict::Pass { citation: citation.clone() };
        match verdict {
            SemanticVerdict::Pass { citation: c } => assert_eq!(c, citation),
            _ => panic!("expected Pass variant"),
        }
    }

    #[test]
    fn test_semantic_verdict_fail_holds_reason() {
        let verdict = SemanticVerdict::Fail { reason: "claim contradicts evidence".to_string() };
        match verdict {
            SemanticVerdict::Fail { reason } => {
                assert_eq!(reason, "claim contradicts evidence");
            }
            _ => panic!("expected Fail variant"),
        }
    }

    #[test]
    fn test_semantic_verdict_pending_constructs_as_unit_variant() {
        let verdict = SemanticVerdict::Pending;
        assert!(matches!(verdict, SemanticVerdict::Pending));
    }

    // ── SemanticVerifyEntry ───────────────────────────────────────────────

    fn make_spec_element_origin() -> VerifyOriginRef {
        let element_id = SpecElementId::try_new("GO-01".to_string()).unwrap();
        VerifyOriginRef::SpecElement(SpecElementRef::new(
            SpecSectionKind::Goal,
            element_id,
            "test".to_string(),
        ))
    }

    fn make_adr_decision_origin() -> VerifyOriginRef {
        VerifyOriginRef::AdrDecision(AdrDecisionRef::new("adr.md".to_string(), "D1".to_string()))
    }

    #[test]
    fn test_semantic_verify_entry_new_stores_all_fields() {
        let claim = make_hash(1);
        let evidence = make_hash(2);
        let verdict = SemanticVerdict::Pending;
        let claim_origin = make_spec_element_origin();
        let evidence_origin = make_adr_decision_origin();

        let entry = SemanticVerifyEntry::new(
            claim.clone(),
            evidence.clone(),
            verdict.clone(),
            claim_origin.clone(),
            evidence_origin.clone(),
        );

        assert_eq!(entry.claim_hash, claim);
        assert_eq!(entry.evidence_hash, evidence);
        assert_eq!(entry.verdict, verdict);
        assert_eq!(entry.claim_origin, claim_origin);
        assert_eq!(entry.evidence_origin, evidence_origin);
    }

    // ── SpecAdrVerifyCacheDocument ────────────────────────────────────────

    #[test]
    fn test_spec_adr_verify_cache_document_new_stores_entries() {
        let entry = SemanticVerifyEntry::new(
            make_hash(1),
            make_hash(2),
            SemanticVerdict::Pending,
            make_spec_element_origin(),
            make_adr_decision_origin(),
        );
        let doc = SpecAdrVerifyCacheDocument::new(vec![entry.clone()]);

        assert_eq!(doc.entries.len(), 1);
        assert_eq!(doc.entries[0], entry);
    }

    #[test]
    fn test_spec_adr_verify_cache_document_empty_entries() {
        let doc = SpecAdrVerifyCacheDocument::new(vec![]);
        assert!(doc.entries.is_empty());
    }

    // ── CatalogueSpecVerifyCacheDocument ──────────────────────────────────

    #[test]
    fn test_catalogue_spec_verify_cache_document_new_stores_layer_and_entries() {
        let layer = LayerId::try_new("domain".to_string()).unwrap();
        let entry = SemanticVerifyEntry::new(
            make_hash(3),
            make_hash(4),
            SemanticVerdict::Fail { reason: "mismatch".to_string() },
            make_spec_element_origin(),
            make_adr_decision_origin(),
        );
        let doc = CatalogueSpecVerifyCacheDocument::new(layer.clone(), vec![entry.clone()]);

        assert_eq!(doc.layer, layer);
        assert_eq!(doc.entries.len(), 1);
        assert_eq!(doc.entries[0], entry);
    }

    #[test]
    fn test_catalogue_spec_verify_cache_document_empty_entries() {
        let layer = LayerId::try_new("usecase".to_string()).unwrap();
        let doc = CatalogueSpecVerifyCacheDocument::new(layer, vec![]);
        assert!(doc.entries.is_empty());
    }

    // ── CatalogueEntryKey ─────────────────────────────────────────────────

    #[test]
    fn test_catalogue_entry_key_try_new_with_valid_string_succeeds() {
        let result = CatalogueEntryKey::try_new("UserRepository".to_string());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_str(), "UserRepository");
    }

    #[test]
    fn test_catalogue_entry_key_try_new_with_empty_string_returns_error() {
        let err = CatalogueEntryKey::try_new(String::new()).unwrap_err();
        assert!(matches!(err, ValidationError::EmptyString));
    }

    #[test]
    fn test_catalogue_entry_key_try_new_with_whitespace_only_returns_error() {
        let err = CatalogueEntryKey::try_new("   \t  ".to_string()).unwrap_err();
        assert!(matches!(err, ValidationError::EmptyString));
    }

    // ── VerifyOriginRef ───────────────────────────────────────────────────

    #[test]
    fn test_verify_origin_ref_spec_element_variant_roundtrip() {
        let element_id = SpecElementId::try_new("IN-07".to_string()).unwrap();
        let spec_ref =
            SpecElementRef::new(SpecSectionKind::InScope, element_id, "Some spec text".to_string());
        let origin = VerifyOriginRef::SpecElement(spec_ref.clone());
        match origin {
            VerifyOriginRef::SpecElement(r) => assert_eq!(r, spec_ref),
            _ => panic!("expected SpecElement variant"),
        }
    }

    #[test]
    fn test_verify_origin_ref_adr_decision_variant_roundtrip() {
        let adr_ref = AdrDecisionRef::new(
            "knowledge/adr/2026-06-26-0842-ref-verify-results-command.md".to_string(),
            "D3".to_string(),
        );
        let origin = VerifyOriginRef::AdrDecision(adr_ref.clone());
        match origin {
            VerifyOriginRef::AdrDecision(r) => assert_eq!(r, adr_ref),
            _ => panic!("expected AdrDecision variant"),
        }
    }

    #[test]
    fn test_verify_origin_ref_catalogue_entry_variant_roundtrip() {
        let entry_key = CatalogueEntryKey::try_new("UserRepository".to_string()).unwrap();
        let cat_ref = CatalogueEntryRef::new(
            "track/items/foo/domain-types.json".to_string(),
            CatalogueSectionKey::Traits,
            entry_key,
        );
        let origin = VerifyOriginRef::CatalogueEntry(cat_ref.clone());
        match origin {
            VerifyOriginRef::CatalogueEntry(r) => assert_eq!(r, cat_ref),
            _ => panic!("expected CatalogueEntry variant"),
        }
    }
}
