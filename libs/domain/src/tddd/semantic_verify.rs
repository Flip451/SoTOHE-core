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
}

impl SemanticVerifyEntry {
    /// Construct a new [`SemanticVerifyEntry`].
    pub fn new(
        claim_hash: ContentHash,
        evidence_hash: ContentHash,
        verdict: SemanticVerdict,
    ) -> Self {
        Self { claim_hash, evidence_hash, verdict }
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

    #[test]
    fn test_semantic_verify_entry_new_stores_all_fields() {
        let claim = make_hash(1);
        let evidence = make_hash(2);
        let verdict = SemanticVerdict::Pending;

        let entry = SemanticVerifyEntry::new(claim.clone(), evidence.clone(), verdict.clone());

        assert_eq!(entry.claim_hash, claim);
        assert_eq!(entry.evidence_hash, evidence);
        assert_eq!(entry.verdict, verdict);
    }

    // ── SpecAdrVerifyCacheDocument ────────────────────────────────────────

    #[test]
    fn test_spec_adr_verify_cache_document_new_stores_entries() {
        let entry = SemanticVerifyEntry::new(make_hash(1), make_hash(2), SemanticVerdict::Pending);
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
}
