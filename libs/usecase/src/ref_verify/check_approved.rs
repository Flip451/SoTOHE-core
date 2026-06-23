//! [`RefVerifyCheckApprovedInteractor`] — application service for the
//! `sotp ref-verify check-approved` subcommand (commit gate read-only path).

use std::collections::HashMap;
use std::sync::Arc;

use domain::tddd::semantic_verify::SemanticVerdict;

use super::{
    RefVerifyCachePort, RefVerifyCacheScope, RefVerifyCommand, RefVerifyConfig, RefVerifyError,
    RefVerifyPairSourcePort,
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

        // Group pair keys by cache scope.
        let mut scope_keys: HashMap<
            RefVerifyCacheScope,
            Vec<(domain::ContentHash, domain::ContentHash)>,
        > = HashMap::new();
        for pair in &production_pairs {
            scope_keys
                .entry(pair.cache_scope.clone())
                .or_default()
                .push((pair.claim_hash.clone(), pair.evidence_hash.clone()));
        }

        let mut missing_or_non_pass: Vec<String> = Vec::new();

        for (cache_scope, pair_keys) in &scope_keys {
            let entries = self.cache.load_entries(cmd, cache_scope)?;

            for (claim_hash, evidence_hash) in pair_keys {
                let matching_entries = entries
                    .iter()
                    .filter(|entry| {
                        entry.claim_hash == *claim_hash && entry.evidence_hash == *evidence_hash
                    })
                    .collect::<Vec<_>>();

                if matching_entries.is_empty() {
                    missing_or_non_pass.push(format!(
                        "pair ({}, {}) has no Pass cache entry",
                        claim_hash.to_hex(),
                        evidence_hash.to_hex()
                    ));
                } else if matching_entries
                    .iter()
                    .any(|entry| !matches!(entry.verdict, SemanticVerdict::Pass { .. }))
                {
                    missing_or_non_pass.push(format!(
                        "pair ({}, {}) has non-Pass cache entry",
                        claim_hash.to_hex(),
                        evidence_hash.to_hex()
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
