//! [`RefVerifyCheckApprovedInteractor`] — application service for the
//! `sotp ref-verify check-approved` subcommand (commit gate read-only path).

use std::collections::HashMap;
use std::sync::Arc;

use domain::tddd::semantic_verify::SemanticVerdict;

use super::{
    RefVerifyCachePort, RefVerifyCacheScope, RefVerifyCommand, RefVerifyConfig, RefVerifyError,
    RefVerifyPair, RefVerifyPairSourcePort,
};

// ── Outcome ───────────────────────────────────────────────────────────────────

/// Outcome of the `check-approved` gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckApprovedOutcome {
    /// No production reference pairs found — gate passes vacuously.
    NoPairs,
    /// All production reference pairs have verified Pass cache entries.
    AllApproved,
    /// One or more production reference pairs lack a Pass cache entry.
    NotApproved {
        /// Human-readable descriptions of each missing/non-pass pair.
        missing_or_non_pass: Vec<String>,
    },
}

// ── Primary port ──────────────────────────────────────────────────────────────

/// Primary port for the `ref-verify check-approved` use case.
///
/// Reads the pair source and verify-cache artifacts to determine whether all
/// production reference pairs have verified Pass cache entries.
pub trait RefVerifyCheckApprovedService: Send + Sync {
    /// Execute the check-approved gate.
    ///
    /// # Errors
    ///
    /// Returns [`RefVerifyError`] on infrastructure failure (pair-source read,
    /// cache read).  Returns `Ok(CheckApprovedOutcome)` for all gate outcomes.
    fn check_approved(
        &self,
        cmd: &RefVerifyCommand,
    ) -> Result<CheckApprovedOutcome, RefVerifyError>;
}

// ── Interactor ────────────────────────────────────────────────────────────────

/// Default [`RefVerifyCheckApprovedService`] implementation.
///
/// Loads production pairs (excluding known-bad probes) via `pair_source`,
/// then reads per-scope verify-cache artifacts to verify all pairs are Pass.
pub struct RefVerifyCheckApprovedInteractor {
    pair_source: Arc<dyn RefVerifyPairSourcePort>,
    cache: Arc<dyn RefVerifyCachePort>,
}

impl RefVerifyCheckApprovedInteractor {
    /// Construct a new interactor.
    #[must_use]
    pub fn new(
        pair_source: Arc<dyn RefVerifyPairSourcePort>,
        cache: Arc<dyn RefVerifyCachePort>,
    ) -> Self {
        Self { pair_source, cache }
    }
}

impl RefVerifyCheckApprovedService for RefVerifyCheckApprovedInteractor {
    fn check_approved(
        &self,
        cmd: &RefVerifyCommand,
    ) -> Result<CheckApprovedOutcome, RefVerifyError> {
        // Active-track guard (mirrors VerifySemanticRefsInteractor).
        let expected_branch = format!("track/{}", cmd.track_id.as_ref());
        if cmd.current_branch != expected_branch {
            return Err(RefVerifyError::TrackNotActive { branch: cmd.current_branch.clone() });
        }

        let config = RefVerifyConfig::default();
        let pairs = self.pair_source.load_pairs(cmd, &config)?;
        let production_pairs: Vec<_> = pairs.into_iter().filter(|p| !p.known_bad).collect();

        if production_pairs.is_empty() {
            return Ok(CheckApprovedOutcome::NoPairs);
        }

        // Group pairs by cache scope, preserving the full pair so that origin
        // fields are available for the four-field cache lookup below.
        let mut scope_groups: HashMap<RefVerifyCacheScope, Vec<&RefVerifyPair>> = HashMap::new();
        for pair in &production_pairs {
            scope_groups.entry(pair.cache_scope.clone()).or_default().push(pair);
        }

        let mut missing_or_non_pass: Vec<String> = Vec::new();

        for (cache_scope, pairs) in &scope_groups {
            let entries = self.cache.load_entries(cmd, cache_scope)?;

            for pair in pairs {
                // A cache hit requires the full key
                // (claim_hash, evidence_hash, claim_origin, evidence_origin) to
                // match. Same-content pairs at different ADR anchors or
                // catalogue entries must not share each other's cached verdict.
                let matching_entries = entries
                    .iter()
                    .filter(|entry| {
                        entry.claim_hash == pair.claim_hash
                            && entry.evidence_hash == pair.evidence_hash
                            && entry.claim_origin == pair.claim_origin
                            && entry.evidence_origin == pair.evidence_origin
                    })
                    .collect::<Vec<_>>();

                if matching_entries.is_empty() {
                    missing_or_non_pass.push(format!(
                        "pair ({}, {}) has no Pass cache entry",
                        pair.claim_hash.to_hex(),
                        pair.evidence_hash.to_hex()
                    ));
                } else if matching_entries
                    .iter()
                    .any(|entry| !matches!(entry.verdict, SemanticVerdict::Pass { .. }))
                {
                    missing_or_non_pass.push(format!(
                        "pair ({}, {}) has non-Pass cache entry",
                        pair.claim_hash.to_hex(),
                        pair.evidence_hash.to_hex()
                    ));
                }
            }
        }

        if missing_or_non_pass.is_empty() {
            Ok(CheckApprovedOutcome::AllApproved)
        } else {
            Ok(CheckApprovedOutcome::NotApproved { missing_or_non_pass })
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use domain::ContentHash;
    use domain::tddd::semantic_verify::{
        AdrDecisionRef, EvidenceCitation, SemanticVerdict, SemanticVerifyEntry, VerifyOriginRef,
    };

    use super::super::{
        RefVerifyCachePort, RefVerifyCacheScope, RefVerifyCommand, RefVerifyConfig, RefVerifyError,
        RefVerifyPair, RefVerifyPairSourcePort, RefVerifyScope,
    };
    use super::{
        CheckApprovedOutcome, RefVerifyCheckApprovedInteractor, RefVerifyCheckApprovedService,
    };
    use domain::TrackId;

    // ── helpers ───────────────────────────────────────────────────────────────

    fn hash(byte: u8) -> ContentHash {
        ContentHash::from_bytes([byte; 32])
    }

    fn pass_verdict() -> SemanticVerdict {
        SemanticVerdict::Pass {
            citation: EvidenceCitation::try_new("evidence supports claim".to_owned()).unwrap(),
        }
    }

    /// Two distinct ADR-decision origins used to exercise the four-field key.
    fn origin_a_claim() -> VerifyOriginRef {
        VerifyOriginRef::AdrDecision(AdrDecisionRef::new("adr-A.md".to_owned(), "D1".to_owned()))
    }
    fn origin_a_evidence() -> VerifyOriginRef {
        VerifyOriginRef::AdrDecision(AdrDecisionRef::new("adr-A.md".to_owned(), "D2".to_owned()))
    }
    fn origin_b_claim() -> VerifyOriginRef {
        VerifyOriginRef::AdrDecision(AdrDecisionRef::new("adr-B.md".to_owned(), "D1".to_owned()))
    }
    fn origin_b_evidence() -> VerifyOriginRef {
        VerifyOriginRef::AdrDecision(AdrDecisionRef::new("adr-B.md".to_owned(), "D2".to_owned()))
    }
    fn origin_c_claim() -> VerifyOriginRef {
        VerifyOriginRef::AdrDecision(AdrDecisionRef::new("adr-C.md".to_owned(), "D1".to_owned()))
    }
    fn origin_c_evidence() -> VerifyOriginRef {
        VerifyOriginRef::AdrDecision(AdrDecisionRef::new("adr-C.md".to_owned(), "D2".to_owned()))
    }

    fn make_pair(
        claim_byte: u8,
        evidence_byte: u8,
        claim_origin: VerifyOriginRef,
        evidence_origin: VerifyOriginRef,
    ) -> RefVerifyPair {
        RefVerifyPair {
            claim: format!("claim-{claim_byte}"),
            evidence: format!("evidence-{evidence_byte}"),
            claim_hash: hash(claim_byte),
            evidence_hash: hash(evidence_byte),
            cache_scope: RefVerifyCacheScope::SpecAdr,
            known_bad: false,
            claim_origin,
            evidence_origin,
        }
    }

    fn make_entry(
        claim_byte: u8,
        evidence_byte: u8,
        verdict: SemanticVerdict,
        claim_origin: VerifyOriginRef,
        evidence_origin: VerifyOriginRef,
    ) -> SemanticVerifyEntry {
        SemanticVerifyEntry::new(
            hash(claim_byte),
            hash(evidence_byte),
            verdict,
            claim_origin,
            evidence_origin,
        )
    }

    fn track_cmd() -> RefVerifyCommand {
        RefVerifyCommand {
            track_id: TrackId::try_new("test-track").unwrap(),
            scope: RefVerifyScope::All,
            current_branch: "track/test-track".to_owned(),
        }
    }

    // ── stubs ─────────────────────────────────────────────────────────────────

    struct StubPairSource {
        pairs: Vec<RefVerifyPair>,
    }
    impl RefVerifyPairSourcePort for StubPairSource {
        fn load_pairs(
            &self,
            _cmd: &RefVerifyCommand,
            _config: &RefVerifyConfig,
        ) -> Result<Vec<RefVerifyPair>, RefVerifyError> {
            Ok(self.pairs.clone())
        }
    }

    struct StubCache {
        entries: Vec<SemanticVerifyEntry>,
    }
    impl RefVerifyCachePort for StubCache {
        fn load_entries(
            &self,
            _cmd: &RefVerifyCommand,
            _scope: &RefVerifyCacheScope,
        ) -> Result<Vec<SemanticVerifyEntry>, RefVerifyError> {
            Ok(self.entries.clone())
        }
        fn save_entries(
            &self,
            _cmd: &RefVerifyCommand,
            _scope: &RefVerifyCacheScope,
            _entries: Vec<SemanticVerifyEntry>,
        ) -> Result<(), RefVerifyError> {
            Ok(())
        }
    }

    // ── invariant tests ───────────────────────────────────────────────────────

    /// Cache has two Pass entries with the same hashes but different origins.
    /// Production pair P (hash=H, origin_A) → matches entry_A → has Pass cache.
    /// Production pair Q (hash=H, origin_C) → matches neither entry → missing Pass cache.
    /// Gate reports NotApproved with exactly Q in the missing list.
    #[test]
    fn check_approved_distinguishes_pass_by_origin() {
        let entry_a = make_entry(1, 2, pass_verdict(), origin_a_claim(), origin_a_evidence());
        let entry_b = make_entry(1, 2, pass_verdict(), origin_b_claim(), origin_b_evidence());

        let pair_p = make_pair(1, 2, origin_a_claim(), origin_a_evidence());
        let pair_q = make_pair(1, 2, origin_c_claim(), origin_c_evidence());

        let interactor = RefVerifyCheckApprovedInteractor::new(
            Arc::new(StubPairSource { pairs: vec![pair_p, pair_q] }),
            Arc::new(StubCache { entries: vec![entry_a, entry_b] }),
        );

        let outcome = interactor.check_approved(&track_cmd()).unwrap();

        // P is covered by entry_A; Q has no matching entry → NotApproved with one item.
        match outcome {
            CheckApprovedOutcome::NotApproved { ref missing_or_non_pass } => {
                assert_eq!(
                    missing_or_non_pass.len(),
                    1,
                    "expected exactly one missing pair (pair_q), got {missing_or_non_pass:?}"
                );
            }
            other => panic!("expected NotApproved, got {other:?}"),
        }
    }

    /// Cache has one Pass entry for origin_A. Production pair carries the same
    /// hashes but origin_B. The gate must not inherit entry_A's Pass and must
    /// report missing Pass cache for the pair.
    #[test]
    fn check_approved_rejects_origin_mismatch_with_hash_match() {
        let entry = make_entry(1, 2, pass_verdict(), origin_a_claim(), origin_a_evidence());

        let pair = make_pair(1, 2, origin_b_claim(), origin_b_evidence());

        let interactor = RefVerifyCheckApprovedInteractor::new(
            Arc::new(StubPairSource { pairs: vec![pair] }),
            Arc::new(StubCache { entries: vec![entry] }),
        );

        let outcome = interactor.check_approved(&track_cmd()).unwrap();

        assert!(
            matches!(outcome, CheckApprovedOutcome::NotApproved { .. }),
            "expected NotApproved when origin mismatches hash-only cache entry, got {outcome:?}"
        );
    }
}
