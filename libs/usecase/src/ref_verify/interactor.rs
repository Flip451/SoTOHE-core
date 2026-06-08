//! [`VerifySemanticRefsInteractor`] — default implementation of
//! [`super::RefVerifyApplicationService`].

use std::collections::HashMap;
use std::sync::Arc;

use domain::tddd::semantic_verify::{ModelTier, SemanticVerdict, SemanticVerifyEntry};

use super::{
    RefVerifierPort, RefVerifyApplicationService, RefVerifyCachePort, RefVerifyCacheScope,
    RefVerifyCommand, RefVerifyConfig, RefVerifyError, RefVerifyPair, RefVerifyPairSourcePort,
};

// ── VerifySemanticRefsInteractor ──────────────────────────────────────────────

/// Default [`super::RefVerifyApplicationService`] implementation.
///
/// Orchestrates pair loading, cache hit checks, verifier calls, final-tier
/// escalation, and cache persistence through injected secondary ports; it does
/// not perform filesystem/config I/O directly.
pub struct VerifySemanticRefsInteractor {
    pair_source: Arc<dyn RefVerifyPairSourcePort>,
    cache: Arc<dyn RefVerifyCachePort>,
    verifier: Arc<dyn RefVerifierPort>,
    config: RefVerifyConfig,
}

impl VerifySemanticRefsInteractor {
    /// Construct a new interactor by injecting all secondary ports and config.
    #[must_use]
    pub fn new(
        pair_source: Arc<dyn RefVerifyPairSourcePort>,
        cache: Arc<dyn RefVerifyCachePort>,
        verifier: Arc<dyn RefVerifierPort>,
        config: RefVerifyConfig,
    ) -> Self {
        Self { pair_source, cache, verifier, config }
    }
}

impl RefVerifyApplicationService for VerifySemanticRefsInteractor {
    /// Execute the full three-tier semantic review pipeline.
    ///
    /// # Algorithm
    ///
    /// 1. Load all pairs (production + known-bad probes) via `pair_source`.
    /// 2. Separate `known_bad` probes from production pairs.
    /// 3. For each `cache_scope` group of production pairs, load existing
    ///    cache entries and skip pairs whose `(claim_hash, evidence_hash)` is
    ///    unchanged (AC-07).
    /// 4. Evaluate all remaining production pairs **and** all known-bad probes
    ///    at `ModelTier::Fast`.
    /// 5. Check known-bad detection rate:
    ///    - If >= threshold → calibration healthy; fast Pass production pairs
    ///      are trusted and will be cached (AC-08).
    ///    - If < threshold → calibration failure; re-evaluate known-bad probes
    ///      AND all fast-evaluated production pairs (including fast Passes) at
    ///      `ModelTier::Final` (AC-09).
    /// 6. Escalate remaining production Fail/Pending to `ModelTier::Final`.
    /// 7. After final-tier re-evaluation:
    ///    - Persistent Fail → collect for `SemanticFailuresConfirmed`.
    ///    - Persistent Pending → collect for `HumanEscalationRequired`.
    ///    - Check final known-bad detection rate (if re-evaluated); if still
    ///      below threshold → `HumanEscalationRequired`.
    /// 8. Save trusted verdicts grouped by `cache_scope`.
    /// 9. Return `Ok(())` when all production pairs are Pass.
    ///
    /// # Errors
    ///
    /// See [`RefVerifyError`] variants.
    fn execute(&self, cmd: &RefVerifyCommand) -> Result<(), RefVerifyError> {
        // Step 0: enforce active-track guard.
        // The expected branch for track_id "foo" is "track/foo".
        let expected_branch = format!("track/{}", cmd.track_id.as_ref());
        if cmd.current_branch != expected_branch {
            return Err(RefVerifyError::TrackNotActive { branch: cmd.current_branch.clone() });
        }

        // Step 1: load all pairs.
        let all_pairs = self.pair_source.load_pairs(cmd, &self.config)?;

        // Step 2: separate known-bad probes from production pairs.
        let (production_pairs, known_bad_probes): (Vec<_>, Vec<_>) =
            all_pairs.into_iter().partition(|p| !p.known_bad);

        // Step 3: for each cache_scope group, load existing cache and partition into hits/misses.
        // A production pair is a cache hit when (claim_hash, evidence_hash) is unchanged.
        let mut scope_cache: HashMap<RefVerifyCacheScope, Vec<SemanticVerifyEntry>> =
            HashMap::new();

        // Collect distinct scopes from production pairs.
        let scopes: Vec<RefVerifyCacheScope> = {
            let mut seen: Vec<RefVerifyCacheScope> = Vec::new();
            for pair in &production_pairs {
                if !seen.contains(&pair.cache_scope) {
                    seen.push(pair.cache_scope.clone());
                }
            }
            seen
        };

        for scope in &scopes {
            let entries = self.cache.load_entries(cmd, scope)?;
            scope_cache.insert(scope.clone(), entries);
        }

        // Partition production pairs into cache hits and cache misses.
        // A cache hit means (claim_hash, evidence_hash) is frozen in the cache for ANY verdict.
        // Cache-hit pairs are NOT sent to the verifier; their frozen verdict is preserved as-is.
        let (cache_hits, cache_misses): (Vec<_>, Vec<_>) =
            production_pairs.iter().partition(|pair| {
                scope_cache
                    .get(&pair.cache_scope)
                    .map(|entries| {
                        entries.iter().any(|e| {
                            e.claim_hash == pair.claim_hash && e.evidence_hash == pair.evidence_hash
                        })
                    })
                    .unwrap_or(false)
            });

        // Collect frozen verdicts for cache-hit pairs so they participate in
        // confirmed_fails / confirmed_pending categorisation below.
        let cache_hit_verdicts: Vec<(&RefVerifyPair, SemanticVerdict)> = cache_hits
            .iter()
            .filter_map(|pair| {
                scope_cache.get(&pair.cache_scope).and_then(|entries| {
                    entries
                        .iter()
                        .find(|e| {
                            e.claim_hash == pair.claim_hash && e.evidence_hash == pair.evidence_hash
                        })
                        .map(|e| (*pair, e.verdict.clone()))
                })
            })
            .collect();

        // Step 4: evaluate cache-miss production pairs and all known-bad probes at Fast tier,
        // with parallelism bounded by max_parallelism (CN-05).
        let max_par = self.config.max_parallelism.as_usize();

        // Convert &[&RefVerifyPair] to owned Vec<RefVerifyPair> for parallel_verify.
        let cache_miss_owned: Vec<RefVerifyPair> =
            cache_misses.iter().map(|p| (*p).clone()).collect();

        let fast_production_verdicts: Vec<(RefVerifyPair, SemanticVerdict)> =
            parallel_verify(&self.verifier, &cache_miss_owned, ModelTier::Fast, max_par)?;

        let fast_known_bad_verdicts: Vec<(RefVerifyPair, SemanticVerdict)> =
            parallel_verify(&self.verifier, &known_bad_probes, ModelTier::Fast, max_par)?;

        // Step 5: check known-bad detection rate at fast tier.
        let fast_detection_rate = compute_detection_rate_owned(&fast_known_bad_verdicts);
        let threshold = self.config.known_bad_detection_threshold_percent.as_u8();
        let fast_calibration_healthy = fast_detection_rate >= threshold;

        // Step 6/7: escalation logic.
        // final_production_verdicts accumulates freshly-evaluated verdicts (cache misses only).
        let (fresh_final_production_verdicts, final_known_bad_verdicts) =
            if fast_calibration_healthy {
                // Healthy calibration: trusted fast Pass production pairs are cached as-is.
                // Only Fail/Pending are escalated to Final tier.
                let (fast_pass, fast_not_pass): (Vec<_>, Vec<_>) = fast_production_verdicts
                    .into_iter()
                    .partition(|(_, v)| matches!(v, SemanticVerdict::Pass { .. }));

                // Escalate Fail and Pending to Final tier.
                let escalation_verdicts = parallel_verify(
                    &self.verifier,
                    &fast_not_pass_pairs(&fast_not_pass),
                    ModelTier::Final,
                    max_par,
                )?;

                // Trusted fast Passes + escalated verdicts = final fresh production verdicts.
                let mut prod = fast_pass;
                prod.extend(escalation_verdicts);

                // known-bad probes were evaluated at Fast; no final re-evaluation needed.
                (prod, fast_known_bad_verdicts)
            } else {
                // Calibration failure: re-evaluate known-bad probes AND all fast production
                // pairs (including fast Passes) at Final tier (AC-09).
                let re_evaluated_production = parallel_verify(
                    &self.verifier,
                    &all_miss_pairs(&fast_production_verdicts),
                    ModelTier::Final,
                    max_par,
                )?;

                let re_evaluated_probes =
                    parallel_verify(&self.verifier, &known_bad_probes, ModelTier::Final, max_par)?;

                (re_evaluated_production, re_evaluated_probes)
            };

        // Check final known-bad detection rate.
        let final_detection_rate = compute_detection_rate_owned(&final_known_bad_verdicts);
        let final_calibration_healthy = final_detection_rate >= threshold;

        // Categorise all production verdicts (fresh evaluations + frozen cache-hit verdicts).
        let mut confirmed_fails: usize = 0;
        let mut confirmed_pending: usize = 0;

        for (_, verdict) in &fresh_final_production_verdicts {
            match verdict {
                SemanticVerdict::Fail { .. } => confirmed_fails += 1,
                SemanticVerdict::Pending => confirmed_pending += 1,
                SemanticVerdict::Pass { .. } => {}
            }
        }

        // Cache-hit verdicts participate in the gate (CN-06: frozen verdicts are not discarded).
        for (_, verdict) in &cache_hit_verdicts {
            match verdict {
                SemanticVerdict::Fail { .. } => confirmed_fails += 1,
                SemanticVerdict::Pending => confirmed_pending += 1,
                SemanticVerdict::Pass { .. } => {}
            }
        }

        // Step 9: return appropriate error or Ok(()).
        // Calibration failure takes precedence over confirmed production failures (OS-04):
        // when the verifier is known to be degraded, its Fail verdicts cannot be trusted and
        // must NOT be persisted to the verify-cache.
        if !final_calibration_healthy {
            // Report the total unresolved count: pending production pairs plus the
            // degraded known-bad probe set. Both categories require human review.
            let count = confirmed_pending + known_bad_probes.len();
            return Err(RefVerifyError::HumanEscalationRequired { pair_count: count });
        }

        // Step 8: build updated cache contents grouped by cache_scope.
        // Only reached when calibration is healthy; degraded-verifier verdicts are never cached.
        // For each scope, start from the existing cache entries, replace the entries that
        // correspond to re-evaluated pairs with fresh verdicts, and add new ones.
        let mut new_entries_by_scope: HashMap<RefVerifyCacheScope, Vec<SemanticVerifyEntry>> =
            HashMap::new();

        // Retain existing cache entries whose hashes were NOT freshly evaluated in this scope.
        // The fresh_hashes check MUST be scope-local: caches are stored in separate artifacts
        // per scope, so a (claim_hash, evidence_hash) collision across scopes must not
        // cause one scope's entry to be silently deleted by a re-evaluation in another scope.
        for scope in &scopes {
            // Build the set of freshly-evaluated hash pairs for THIS scope only.
            let fresh_hashes_in_scope: Vec<_> = fresh_final_production_verdicts
                .iter()
                .filter(|(pair, _)| &pair.cache_scope == scope)
                .map(|(pair, _)| (&pair.claim_hash, &pair.evidence_hash))
                .collect();

            if let Some(existing) = scope_cache.get(scope) {
                let retained: Vec<SemanticVerifyEntry> = existing
                    .iter()
                    .filter(|e| {
                        !fresh_hashes_in_scope
                            .iter()
                            .any(|(ch, eh)| *ch == &e.claim_hash && *eh == &e.evidence_hash)
                    })
                    .cloned()
                    .collect();

                if !retained.is_empty() {
                    new_entries_by_scope.entry(scope.clone()).or_default().extend(retained);
                }
            }
        }

        // Add freshly-evaluated Pass and Fail verdicts (Pending is not cached).
        for (pair, verdict) in &fresh_final_production_verdicts {
            match verdict {
                SemanticVerdict::Pass { .. } | SemanticVerdict::Fail { .. } => {
                    let entry = SemanticVerifyEntry::new(
                        pair.claim_hash.clone(),
                        pair.evidence_hash.clone(),
                        verdict.clone(),
                    );
                    new_entries_by_scope.entry(pair.cache_scope.clone()).or_default().push(entry);
                }
                SemanticVerdict::Pending => {
                    // Pending verdicts are not persisted (CN-06 fail-closed).
                }
            }
        }

        // Persist updated caches.
        for (scope, entries) in new_entries_by_scope {
            self.cache.save_entries(cmd, &scope, entries)?;
        }

        // SemanticFailuresConfirmed takes precedence over HumanEscalationRequired:
        // Confirmed production Fail verdicts are actionable by the writer/fix loop
        // and should be surfaced first so they can be resolved. If only Pending
        // verdicts remain after the writer loop, HumanEscalationRequired is returned.
        if confirmed_fails > 0 {
            return Err(RefVerifyError::SemanticFailuresConfirmed { pair_count: confirmed_fails });
        }

        if confirmed_pending > 0 {
            return Err(RefVerifyError::HumanEscalationRequired { pair_count: confirmed_pending });
        }

        Ok(())
    }
}

/// Evaluate a slice of pairs at the given `tier` with up to `max_par` concurrent workers.
///
/// Each pair is sent to `verifier.verify_pair`; results are returned in the same order
/// as the input slice.  The `max_par` bound is honoured by chunking: up to `max_par`
/// threads are spawned per chunk so the verifier adapter is never overwhelmed (CN-05).
fn parallel_verify(
    verifier: &Arc<dyn RefVerifierPort>,
    pairs: &[RefVerifyPair],
    tier: ModelTier,
    max_par: usize,
) -> Result<Vec<(RefVerifyPair, SemanticVerdict)>, RefVerifyError> {
    if pairs.is_empty() {
        return Ok(Vec::new());
    }

    let mut results: Vec<(RefVerifyPair, SemanticVerdict)> = Vec::with_capacity(pairs.len());

    // Process pairs in chunks of at most max_par to bound concurrency.
    for chunk in pairs.chunks(max_par) {
        // Spawn one thread per pair in the chunk.
        let mut handles = Vec::with_capacity(chunk.len());
        for pair in chunk {
            let pair_owned = pair.clone();
            let verifier_ref = Arc::clone(verifier);
            let tier_copy = tier.clone();
            let handle = std::thread::spawn(move || {
                verifier_ref
                    .verify_pair(pair_owned.claim.clone(), pair_owned.evidence.clone(), tier_copy)
                    .map(|verdict| (pair_owned, verdict))
            });
            handles.push(handle);
        }

        // Collect results in order; on the first error, join all remaining
        // handles so no worker thread is left running after we return
        // (fail-closed: the gate must not have side effects after failure).
        let mut first_err: Option<RefVerifyError> = None;
        for handle in handles {
            let outcome = handle.join();
            if first_err.is_some() {
                // Already failed; still join to avoid detached threads.
                continue;
            }
            match outcome {
                Ok(Ok(pair_verdict)) => results.push(pair_verdict),
                Ok(Err(e)) => {
                    first_err = Some(e);
                }
                Err(_) => {
                    first_err = Some(RefVerifyError::VerifierPort {
                        message: "verifier thread panicked".to_owned(),
                    });
                }
            }
        }
        if let Some(e) = first_err {
            return Err(e);
        }
    }

    Ok(results)
}

/// Extract the `RefVerifyPair` values from `fast_not_pass` verdicts for re-evaluation.
fn fast_not_pass_pairs(fast_not_pass: &[(RefVerifyPair, SemanticVerdict)]) -> Vec<RefVerifyPair> {
    fast_not_pass.iter().map(|(p, _)| p.clone()).collect()
}

/// Extract the `RefVerifyPair` values from any `(RefVerifyPair, SemanticVerdict)` slice.
fn all_miss_pairs(verdicts: &[(RefVerifyPair, SemanticVerdict)]) -> Vec<RefVerifyPair> {
    verdicts.iter().map(|(p, _)| p.clone()).collect()
}

/// Compute the detection rate percentage (0..=100) of known-bad probes (owned-pair variant):
/// how many of the known-bad pairs were correctly identified as `Fail`.
///
/// Returns 100 when there are no probes (no probes → treat as healthy).
fn compute_detection_rate_owned(verdicts: &[(RefVerifyPair, SemanticVerdict)]) -> u8 {
    if verdicts.is_empty() {
        // No known-bad probes → no calibration data. Treat as healthy (100%).
        return 100;
    }
    let detected =
        verdicts.iter().filter(|(_, v)| matches!(v, SemanticVerdict::Fail { .. })).count();
    // detected / total * 100, rounded down.
    let rate = (detected * 100) / verdicts.len();
    // Safe: rate is 0..=100 and we computed it from usize division.
    #[allow(clippy::cast_possible_truncation)]
    (rate as u8)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::too_many_lines
)]
mod tests {
    use std::sync::{Arc, Mutex};

    use domain::ContentHash;
    use domain::tddd::LayerId;
    use domain::tddd::semantic_verify::{
        EvidenceCitation, ModelTier, SemanticVerdict, SemanticVerifyEntry,
    };

    use super::super::{
        RefVerifierPort, RefVerifyApplicationService, RefVerifyCachePort, RefVerifyCacheScope,
        RefVerifyCommand, RefVerifyConfig, RefVerifyError, RefVerifyPair, RefVerifyPairSourcePort,
        RefVerifyParallelism, RefVerifyPercent, RefVerifyScope,
    };
    use super::VerifySemanticRefsInteractor;
    use domain::TrackId;

    // ── helpers ───────────────────────────────────────────────────────────────

    fn hash(byte: u8) -> ContentHash {
        ContentHash::from_bytes([byte; 32])
    }

    fn pass_verdict() -> SemanticVerdict {
        SemanticVerdict::Pass {
            citation: EvidenceCitation::try_new("the spec states X".to_owned()).unwrap(),
        }
    }

    fn fail_verdict() -> SemanticVerdict {
        SemanticVerdict::Fail { reason: "does not match".to_owned() }
    }

    fn production_pair(claim_byte: u8, evidence_byte: u8) -> RefVerifyPair {
        RefVerifyPair {
            claim: format!("claim-{claim_byte}"),
            evidence: format!("evidence-{evidence_byte}"),
            claim_hash: hash(claim_byte),
            evidence_hash: hash(evidence_byte),
            cache_scope: RefVerifyCacheScope::SpecAdr,
            known_bad: false,
        }
    }

    fn known_bad_pair() -> RefVerifyPair {
        RefVerifyPair {
            claim: "known-bad-claim".to_owned(),
            evidence: "known-bad-evidence".to_owned(),
            claim_hash: hash(0xff),
            evidence_hash: hash(0xfe),
            cache_scope: RefVerifyCacheScope::SpecAdr,
            known_bad: true,
        }
    }

    fn track_cmd() -> RefVerifyCommand {
        RefVerifyCommand {
            track_id: TrackId::try_new("test-track").unwrap(),
            scope: RefVerifyScope::All,
            current_branch: "track/test-track".to_owned(),
        }
    }

    fn non_track_cmd() -> RefVerifyCommand {
        RefVerifyCommand {
            track_id: TrackId::try_new("test-track").unwrap(),
            scope: RefVerifyScope::All,
            current_branch: "main".to_owned(),
        }
    }

    // ── stub implementations ──────────────────────────────────────────────────

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
        loaded: Vec<SemanticVerifyEntry>,
        saved: Mutex<Vec<(RefVerifyCacheScope, Vec<SemanticVerifyEntry>)>>,
    }
    impl StubCache {
        fn empty() -> Self {
            Self { loaded: vec![], saved: Mutex::new(vec![]) }
        }
        fn with_entries(entries: Vec<SemanticVerifyEntry>) -> Self {
            Self { loaded: entries, saved: Mutex::new(vec![]) }
        }
        fn saved_calls(&self) -> Vec<(RefVerifyCacheScope, Vec<SemanticVerifyEntry>)> {
            self.saved.lock().unwrap().clone()
        }
    }
    impl RefVerifyCachePort for StubCache {
        fn load_entries(
            &self,
            _cmd: &RefVerifyCommand,
            _scope: &RefVerifyCacheScope,
        ) -> Result<Vec<SemanticVerifyEntry>, RefVerifyError> {
            Ok(self.loaded.clone())
        }
        fn save_entries(
            &self,
            _cmd: &RefVerifyCommand,
            scope: &RefVerifyCacheScope,
            entries: Vec<SemanticVerifyEntry>,
        ) -> Result<(), RefVerifyError> {
            self.saved.lock().unwrap().push((scope.clone(), entries));
            Ok(())
        }
    }

    /// A verifier that returns a fixed verdict for every call.
    struct FixedVerifier {
        verdict: SemanticVerdict,
    }
    impl RefVerifierPort for FixedVerifier {
        fn verify_pair(
            &self,
            _claim: String,
            _evidence: String,
            _tier: ModelTier,
        ) -> Result<SemanticVerdict, RefVerifyError> {
            Ok(self.verdict.clone())
        }
    }

    /// A verifier that tracks calls and returns different verdicts for known-bad vs. others.
    struct SpyVerifier {
        /// Verdict returned for production pairs.
        production_verdict: SemanticVerdict,
        /// Verdict returned for known-bad pairs (claim text starts with "known-bad").
        known_bad_verdict: SemanticVerdict,
        calls: Mutex<Vec<(String, ModelTier)>>,
    }
    impl SpyVerifier {
        fn new(production_verdict: SemanticVerdict, known_bad_verdict: SemanticVerdict) -> Self {
            Self { production_verdict, known_bad_verdict, calls: Mutex::new(vec![]) }
        }
        fn calls(&self) -> Vec<(String, ModelTier)> {
            self.calls.lock().unwrap().clone()
        }
    }
    impl RefVerifierPort for SpyVerifier {
        fn verify_pair(
            &self,
            claim: String,
            _evidence: String,
            tier: ModelTier,
        ) -> Result<SemanticVerdict, RefVerifyError> {
            self.calls.lock().unwrap().push((claim.clone(), tier));
            if claim.starts_with("known-bad") {
                Ok(self.known_bad_verdict.clone())
            } else {
                Ok(self.production_verdict.clone())
            }
        }
    }

    // ── RefVerifyPercent ──────────────────────────────────────────────────────

    #[test]
    fn ref_verify_percent_try_new_with_valid_values_succeeds() {
        assert!(RefVerifyPercent::try_new(1).is_ok());
        assert!(RefVerifyPercent::try_new(50).is_ok());
        assert!(RefVerifyPercent::try_new(100).is_ok());
    }

    #[test]
    fn ref_verify_percent_try_new_with_zero_returns_invalid_config_error() {
        let err = RefVerifyPercent::try_new(0).unwrap_err();
        assert!(matches!(err, RefVerifyError::InvalidConfig { .. }));
    }

    #[test]
    fn ref_verify_percent_try_new_with_over_100_returns_invalid_config_error() {
        let err = RefVerifyPercent::try_new(101).unwrap_err();
        assert!(matches!(err, RefVerifyError::InvalidConfig { .. }));
    }

    #[test]
    fn ref_verify_percent_as_u8_returns_inner_value() {
        let p = RefVerifyPercent::try_new(42).unwrap();
        assert_eq!(p.as_u8(), 42);
    }

    // ── RefVerifyParallelism ──────────────────────────────────────────────────

    #[test]
    fn ref_verify_parallelism_try_new_with_nonzero_succeeds() {
        assert!(RefVerifyParallelism::try_new(1).is_ok());
        assert!(RefVerifyParallelism::try_new(8).is_ok());
    }

    #[test]
    fn ref_verify_parallelism_try_new_with_zero_returns_invalid_config_error() {
        let err = RefVerifyParallelism::try_new(0).unwrap_err();
        assert!(matches!(err, RefVerifyError::InvalidConfig { .. }));
    }

    #[test]
    fn ref_verify_parallelism_as_usize_returns_inner_value() {
        let p = RefVerifyParallelism::try_new(4).unwrap();
        assert_eq!(p.as_usize(), 4);
    }

    // ── RefVerifyConfig ───────────────────────────────────────────────────────

    #[test]
    fn ref_verify_config_try_new_with_valid_values_succeeds() {
        let cfg = RefVerifyConfig::try_new(10, 90, 4).unwrap();
        assert_eq!(cfg.known_bad_injection_rate_percent.as_u8(), 10);
        assert_eq!(cfg.known_bad_detection_threshold_percent.as_u8(), 90);
        assert_eq!(cfg.max_parallelism.as_usize(), 4);
    }

    #[test]
    fn ref_verify_config_try_new_with_zero_injection_returns_error() {
        let err = RefVerifyConfig::try_new(0, 90, 4).unwrap_err();
        assert!(matches!(err, RefVerifyError::InvalidConfig { .. }));
    }

    #[test]
    fn ref_verify_config_try_new_with_zero_threshold_returns_error() {
        let err = RefVerifyConfig::try_new(10, 0, 4).unwrap_err();
        assert!(matches!(err, RefVerifyError::InvalidConfig { .. }));
    }

    #[test]
    fn ref_verify_config_try_new_with_zero_parallelism_returns_error() {
        let err = RefVerifyConfig::try_new(10, 90, 0).unwrap_err();
        assert!(matches!(err, RefVerifyError::InvalidConfig { .. }));
    }

    #[test]
    fn ref_verify_config_default_supplies_10_90_and_nonzero_parallelism() {
        let cfg = RefVerifyConfig::default();
        assert_eq!(cfg.known_bad_injection_rate_percent.as_u8(), 10);
        assert_eq!(cfg.known_bad_detection_threshold_percent.as_u8(), 90);
        assert!(cfg.max_parallelism.as_usize() > 0);
    }

    // ── VerifySemanticRefsInteractor ──────────────────────────────────────────

    #[test]
    fn execute_on_non_track_branch_returns_track_not_active_error() {
        let source: Arc<dyn RefVerifyPairSourcePort> = Arc::new(StubPairSource { pairs: vec![] });
        let cache: Arc<dyn RefVerifyCachePort> = Arc::new(StubCache::empty());
        let verifier: Arc<dyn RefVerifierPort> =
            Arc::new(FixedVerifier { verdict: pass_verdict() });
        let interactor =
            VerifySemanticRefsInteractor::new(source, cache, verifier, RefVerifyConfig::default());

        let err = interactor.execute(&non_track_cmd()).unwrap_err();
        assert!(
            matches!(err, RefVerifyError::TrackNotActive { .. }),
            "expected TrackNotActive, got {err:?}"
        );
    }

    #[test]
    fn all_production_pass_with_trusted_fast_returns_ok_and_saves_cache() {
        let pairs = vec![production_pair(0x01, 0x02), production_pair(0x03, 0x04)];
        let source: Arc<dyn RefVerifyPairSourcePort> = Arc::new(StubPairSource { pairs });
        let cache: Arc<StubCache> = Arc::new(StubCache::empty());
        // known-bad probes pass (fast detection = 100%)
        let verifier: Arc<dyn RefVerifierPort> =
            Arc::new(FixedVerifier { verdict: pass_verdict() });
        let cfg = RefVerifyConfig::default();

        let interactor = VerifySemanticRefsInteractor::new(
            source,
            Arc::clone(&cache) as Arc<dyn RefVerifyCachePort>,
            verifier,
            cfg,
        );

        let result = interactor.execute(&track_cmd());
        assert!(result.is_ok(), "expected Ok(()), got: {result:?}");

        // Some save_entries call must have been made.
        let calls = cache.saved_calls();
        assert!(!calls.is_empty(), "expected at least one save_entries call");
        // All saved entries must be Pass verdicts.
        for (_, entries) in &calls {
            for entry in entries {
                assert!(
                    matches!(entry.verdict, SemanticVerdict::Pass { .. }),
                    "saved entry must be Pass"
                );
            }
        }
    }

    #[test]
    fn cache_hit_pairs_are_skipped_by_verifier() {
        let pair = production_pair(0x01, 0x02);
        // Pre-populate cache with an entry matching pair's hashes.
        let cached_entry = SemanticVerifyEntry::new(
            pair.claim_hash.clone(),
            pair.evidence_hash.clone(),
            pass_verdict(),
        );
        let pairs = vec![pair];
        let source: Arc<dyn RefVerifyPairSourcePort> = Arc::new(StubPairSource { pairs });
        let cache: Arc<StubCache> = Arc::new(StubCache::with_entries(vec![cached_entry]));

        // Spy verifier — should never be called for the cache-hit pair.
        let spy: Arc<SpyVerifier> = Arc::new(SpyVerifier::new(pass_verdict(), fail_verdict()));
        let cfg = RefVerifyConfig::default();

        let interactor = VerifySemanticRefsInteractor::new(
            source,
            Arc::clone(&cache) as Arc<dyn RefVerifyCachePort>,
            Arc::clone(&spy) as Arc<dyn RefVerifierPort>,
            cfg,
        );

        let result = interactor.execute(&track_cmd());
        assert!(result.is_ok(), "expected Ok(()), got: {result:?}");

        // The production pair was a cache hit; the verifier must not have been called
        // for it. (No cache-miss production pairs → zero calls to verify production.)
        let calls = spy.calls();
        let production_calls: Vec<_> =
            calls.iter().filter(|(claim, _)| !claim.starts_with("known-bad")).collect();
        assert!(
            production_calls.is_empty(),
            "verifier must not be called for cache-hit production pairs; got calls: {calls:?}"
        );
    }

    #[test]
    fn known_bad_probe_is_not_skipped_even_when_hash_matches_cache_entry() {
        // Even if the cache contains an entry with the same hashes as a known-bad probe,
        // the probe must still be sent to the verifier (cache bypass for known_bad=true).
        let probe = known_bad_pair();
        let cached_entry = SemanticVerifyEntry::new(
            probe.claim_hash.clone(),
            probe.evidence_hash.clone(),
            pass_verdict(),
        );
        let source: Arc<dyn RefVerifyPairSourcePort> =
            Arc::new(StubPairSource { pairs: vec![probe] });
        let cache: Arc<StubCache> = Arc::new(StubCache::with_entries(vec![cached_entry]));

        let spy: Arc<SpyVerifier> = Arc::new(SpyVerifier::new(pass_verdict(), fail_verdict()));
        let cfg = RefVerifyConfig::default();

        let interactor = VerifySemanticRefsInteractor::new(
            source,
            Arc::clone(&cache) as Arc<dyn RefVerifyCachePort>,
            Arc::clone(&spy) as Arc<dyn RefVerifierPort>,
            cfg,
        );

        // We expect Ok because no production pairs → all pass; detection with only known_bad
        // that returns Fail: detection = 100% ≥ 90%.
        let _ = interactor.execute(&track_cmd());

        let calls = spy.calls();
        let kb_calls: Vec<_> =
            calls.iter().filter(|(claim, _)| claim.starts_with("known-bad")).collect();
        assert!(!kb_calls.is_empty(), "known-bad probe must be sent to verifier even on cache hit");
    }

    #[test]
    fn fail_pair_is_escalated_to_final_tier() {
        // A production pair returns Fail at Fast tier; it must be re-evaluated at Final.
        let pair = production_pair(0x10, 0x11);
        let source: Arc<dyn RefVerifyPairSourcePort> =
            Arc::new(StubPairSource { pairs: vec![pair] });
        let cache: Arc<StubCache> = Arc::new(StubCache::empty());

        // Track which tiers are used per claim.
        let spy: Arc<SpyVerifier> = Arc::new(SpyVerifier::new(fail_verdict(), fail_verdict()));
        let cfg = RefVerifyConfig::default();

        let interactor = VerifySemanticRefsInteractor::new(
            source,
            Arc::clone(&cache) as Arc<dyn RefVerifyCachePort>,
            Arc::clone(&spy) as Arc<dyn RefVerifierPort>,
            cfg,
        );

        let result = interactor.execute(&track_cmd());
        // The final-tier verdict is still Fail → SemanticFailuresConfirmed.
        assert!(
            matches!(result, Err(RefVerifyError::SemanticFailuresConfirmed { pair_count: 1 })),
            "expected SemanticFailuresConfirmed(1), got: {result:?}"
        );

        // Must have been called at Fast and then Final tier for the production pair.
        let calls = spy.calls();
        let production_calls: Vec<_> =
            calls.iter().filter(|(claim, _)| !claim.starts_with("known-bad")).collect();
        let has_fast = production_calls.iter().any(|(_, tier)| matches!(tier, ModelTier::Fast));
        let has_final = production_calls.iter().any(|(_, tier)| matches!(tier, ModelTier::Final));
        assert!(has_fast, "production pair must be evaluated at Fast tier first");
        assert!(has_final, "Fail pair must be escalated to Final tier");
    }

    #[test]
    fn known_bad_below_threshold_at_fast_triggers_full_final_reevaluation() {
        // All known-bad probes return Pass at Fast (so detection = 0% < 90%).
        // This should trigger final re-evaluation of all production pairs (including fast Pass).
        let pair = production_pair(0x20, 0x21);
        let probe = known_bad_pair();
        let source: Arc<dyn RefVerifyPairSourcePort> =
            Arc::new(StubPairSource { pairs: vec![pair, probe] });
        let cache: Arc<StubCache> = Arc::new(StubCache::empty());

        // Production pairs get Pass; known-bad get Pass (simulating bad detector).
        // After final re-evaluation with same verdict: known-bad still Pass → still below threshold.
        let spy: Arc<SpyVerifier> = Arc::new(SpyVerifier::new(pass_verdict(), pass_verdict()));
        let cfg = RefVerifyConfig::default(); // threshold = 90%

        let interactor = VerifySemanticRefsInteractor::new(
            source,
            Arc::clone(&cache) as Arc<dyn RefVerifyCachePort>,
            Arc::clone(&spy) as Arc<dyn RefVerifierPort>,
            cfg,
        );

        let result = interactor.execute(&track_cmd());
        // After final re-evaluation, known-bad detection rate is still 0% → HumanEscalationRequired.
        assert!(
            matches!(result, Err(RefVerifyError::HumanEscalationRequired { .. })),
            "expected HumanEscalationRequired when known-bad detection fails, got: {result:?}"
        );

        // Production pair must have been evaluated at both Fast AND Final.
        let calls = spy.calls();
        let production_calls: Vec<_> =
            calls.iter().filter(|(claim, _)| !claim.starts_with("known-bad")).collect();
        let has_fast = production_calls.iter().any(|(_, tier)| matches!(tier, ModelTier::Fast));
        let has_final = production_calls.iter().any(|(_, tier)| matches!(tier, ModelTier::Final));
        assert!(has_fast, "production pair must be evaluated at Fast tier");
        assert!(has_final, "calibration failure: production pair must be re-evaluated at Final");
    }

    #[test]
    fn healthy_fast_calibration_does_not_escalate_fast_pass_to_final() {
        // All known-bad probes return Fail at Fast (detection = 100% ≥ 90%).
        // Fast Pass production pairs must NOT be escalated to Final.
        let pair = production_pair(0x30, 0x31);
        let probe = known_bad_pair();
        let source: Arc<dyn RefVerifyPairSourcePort> =
            Arc::new(StubPairSource { pairs: vec![pair, probe] });
        let cache: Arc<StubCache> = Arc::new(StubCache::empty());

        let spy: Arc<SpyVerifier> = Arc::new(SpyVerifier::new(pass_verdict(), fail_verdict()));
        let cfg = RefVerifyConfig::default();

        let interactor = VerifySemanticRefsInteractor::new(
            source,
            Arc::clone(&cache) as Arc<dyn RefVerifyCachePort>,
            Arc::clone(&spy) as Arc<dyn RefVerifierPort>,
            cfg,
        );

        let result = interactor.execute(&track_cmd());
        assert!(result.is_ok(), "expected Ok(()), got: {result:?}");

        // Production pair must only have been called at Fast (not Final).
        let calls = spy.calls();
        let production_final_calls: Vec<_> = calls
            .iter()
            .filter(|(claim, tier)| {
                !claim.starts_with("known-bad") && matches!(tier, ModelTier::Final)
            })
            .collect();
        assert!(
            production_final_calls.is_empty(),
            "healthy fast calibration: fast Pass must NOT be escalated to Final; \
             unexpected Final calls: {production_final_calls:?}"
        );
    }

    #[test]
    fn final_tier_known_bad_below_threshold_returns_human_escalation() {
        // After calibration failure, both probes and production pairs are re-evaluated at Final.
        // If final-tier known-bad detection is still below threshold → HumanEscalationRequired.
        let probe = known_bad_pair();
        let source: Arc<dyn RefVerifyPairSourcePort> =
            Arc::new(StubPairSource { pairs: vec![probe] });
        let cache: Arc<StubCache> = Arc::new(StubCache::empty());
        // known-bad always returns Pass → detection = 0% at both Fast and Final.
        let spy: Arc<SpyVerifier> = Arc::new(SpyVerifier::new(pass_verdict(), pass_verdict()));
        let cfg = RefVerifyConfig::default();

        let interactor = VerifySemanticRefsInteractor::new(
            source,
            Arc::clone(&cache) as Arc<dyn RefVerifyCachePort>,
            Arc::clone(&spy) as Arc<dyn RefVerifierPort>,
            cfg,
        );

        let result = interactor.execute(&track_cmd());
        assert!(
            matches!(result, Err(RefVerifyError::HumanEscalationRequired { .. })),
            "expected HumanEscalationRequired, got: {result:?}"
        );
    }

    #[test]
    fn final_fail_confirmed_stores_fail_verdict_and_returns_semantic_failures() {
        let pair = production_pair(0x40, 0x41);
        let source: Arc<dyn RefVerifyPairSourcePort> =
            Arc::new(StubPairSource { pairs: vec![pair.clone()] });
        let cache: Arc<StubCache> = Arc::new(StubCache::empty());
        // Fast: known-bad Fail (healthy calibration), production Fail → escalate.
        // Final: production Fail confirmed.
        let spy: Arc<SpyVerifier> = Arc::new(SpyVerifier::new(fail_verdict(), fail_verdict()));
        let cfg = RefVerifyConfig::default();

        let interactor = VerifySemanticRefsInteractor::new(
            source,
            Arc::clone(&cache) as Arc<dyn RefVerifyCachePort>,
            Arc::clone(&spy) as Arc<dyn RefVerifierPort>,
            cfg,
        );

        let result = interactor.execute(&track_cmd());
        assert!(
            matches!(result, Err(RefVerifyError::SemanticFailuresConfirmed { pair_count: 1 })),
            "expected SemanticFailuresConfirmed(1), got: {result:?}"
        );

        // Fail verdict must have been saved to cache.
        let saved = cache.saved_calls();
        let has_fail_entry = saved.iter().flat_map(|(_, entries)| entries).any(|e| {
            e.claim_hash == pair.claim_hash
                && e.evidence_hash == pair.evidence_hash
                && matches!(e.verdict, SemanticVerdict::Fail { .. })
        });
        assert!(has_fail_entry, "Fail verdict must be persisted to cache");
    }

    #[test]
    fn pending_production_pair_at_final_returns_human_escalation() {
        // Production Fail at Fast → escalate to Final → Pending at Final.
        let pair = production_pair(0x50, 0x51);
        // known-bad returns Fail (healthy); production returns Fail at Fast, Pending at Final.
        struct TwoStageVerifier {
            calls: Mutex<usize>,
        }
        impl RefVerifierPort for TwoStageVerifier {
            fn verify_pair(
                &self,
                claim: String,
                _evidence: String,
                _tier: ModelTier,
            ) -> Result<SemanticVerdict, RefVerifyError> {
                let mut c = self.calls.lock().unwrap();
                *c += 1;
                if claim.starts_with("known-bad") {
                    return Ok(fail_verdict());
                }
                // First call (Fast) → Fail; second call (Final) → Pending.
                if *c <= 1 { Ok(fail_verdict()) } else { Ok(SemanticVerdict::Pending) }
            }
        }
        let source: Arc<dyn RefVerifyPairSourcePort> =
            Arc::new(StubPairSource { pairs: vec![pair] });
        let cache: Arc<StubCache> = Arc::new(StubCache::empty());
        let verifier: Arc<dyn RefVerifierPort> =
            Arc::new(TwoStageVerifier { calls: Mutex::new(0) });
        let cfg = RefVerifyConfig::default();

        let interactor = VerifySemanticRefsInteractor::new(
            source,
            Arc::clone(&cache) as Arc<dyn RefVerifyCachePort>,
            verifier,
            cfg,
        );

        let result = interactor.execute(&track_cmd());
        assert!(
            matches!(result, Err(RefVerifyError::HumanEscalationRequired { .. })),
            "expected HumanEscalationRequired for persistent Pending, got: {result:?}"
        );
    }

    #[test]
    fn ref_verify_scope_all_decomposes_to_per_scope_cache_saves() {
        // Two production pairs with different cache_scopes; both Pass at Fast.
        // Both scopes must have save_entries called.
        let layer = LayerId::try_new("domain").unwrap();
        let mut pair_a = production_pair(0x60, 0x61);
        pair_a.cache_scope = RefVerifyCacheScope::SpecAdr;
        let mut pair_b = production_pair(0x62, 0x63);
        pair_b.cache_scope = RefVerifyCacheScope::CatalogueSpec { layer: layer.clone() };

        let source: Arc<dyn RefVerifyPairSourcePort> =
            Arc::new(StubPairSource { pairs: vec![pair_a, pair_b] });
        let cache: Arc<StubCache> = Arc::new(StubCache::empty());
        let verifier: Arc<dyn RefVerifierPort> =
            Arc::new(FixedVerifier { verdict: pass_verdict() });
        let cfg = RefVerifyConfig::default();

        let interactor = VerifySemanticRefsInteractor::new(
            source,
            Arc::clone(&cache) as Arc<dyn RefVerifyCachePort>,
            verifier,
            cfg,
        );

        let result = interactor.execute(&track_cmd());
        assert!(result.is_ok(), "expected Ok(()), got: {result:?}");

        let saved = cache.saved_calls();
        let scopes: Vec<&RefVerifyCacheScope> = saved.iter().map(|(s, _)| s).collect();
        assert!(
            scopes.contains(&&RefVerifyCacheScope::SpecAdr),
            "SpecAdr scope must have been saved"
        );
        assert!(
            scopes.contains(&&RefVerifyCacheScope::CatalogueSpec { layer }),
            "CatalogueSpec scope must have been saved"
        );
    }

    #[test]
    fn adapter_verdict_stored_with_correct_claim_and_evidence_hashes() {
        // Verify that the SemanticVerifyEntry saved to cache uses the pair's
        // claim_hash and evidence_hash (not some fabricated value).
        let pair = production_pair(0x70, 0x71);
        let source: Arc<dyn RefVerifyPairSourcePort> =
            Arc::new(StubPairSource { pairs: vec![pair.clone()] });
        let cache: Arc<StubCache> = Arc::new(StubCache::empty());
        let verifier: Arc<dyn RefVerifierPort> =
            Arc::new(FixedVerifier { verdict: pass_verdict() });
        let cfg = RefVerifyConfig::default();

        let interactor = VerifySemanticRefsInteractor::new(
            source,
            Arc::clone(&cache) as Arc<dyn RefVerifyCachePort>,
            verifier,
            cfg,
        );

        interactor.execute(&track_cmd()).unwrap();

        let saved = cache.saved_calls();
        let found = saved
            .iter()
            .flat_map(|(_, entries)| entries)
            .find(|e| e.claim_hash == pair.claim_hash && e.evidence_hash == pair.evidence_hash);
        assert!(
            found.is_some(),
            "saved entry must carry the pair's original claim_hash and evidence_hash"
        );
    }

    // ── port boundary failure tests ───────────────────────────────────────────

    struct FailingPairSource;
    impl RefVerifyPairSourcePort for FailingPairSource {
        fn load_pairs(
            &self,
            _cmd: &RefVerifyCommand,
            _config: &RefVerifyConfig,
        ) -> Result<Vec<RefVerifyPair>, RefVerifyError> {
            Err(RefVerifyError::VerifierPort { message: "pair-source failure".to_owned() })
        }
    }

    struct FailingCacheLoad;
    impl RefVerifyCachePort for FailingCacheLoad {
        fn load_entries(
            &self,
            _cmd: &RefVerifyCommand,
            _scope: &RefVerifyCacheScope,
        ) -> Result<Vec<SemanticVerifyEntry>, RefVerifyError> {
            Err(RefVerifyError::CachePersistence { message: "cache load failure".to_owned() })
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

    struct FailingCacheSave;
    impl RefVerifyCachePort for FailingCacheSave {
        fn load_entries(
            &self,
            _cmd: &RefVerifyCommand,
            _scope: &RefVerifyCacheScope,
        ) -> Result<Vec<SemanticVerifyEntry>, RefVerifyError> {
            Ok(vec![])
        }
        fn save_entries(
            &self,
            _cmd: &RefVerifyCommand,
            _scope: &RefVerifyCacheScope,
            _entries: Vec<SemanticVerifyEntry>,
        ) -> Result<(), RefVerifyError> {
            Err(RefVerifyError::CachePersistence { message: "cache save failure".to_owned() })
        }
    }

    struct FailingVerifier;
    impl RefVerifierPort for FailingVerifier {
        fn verify_pair(
            &self,
            _claim: String,
            _evidence: String,
            _tier: ModelTier,
        ) -> Result<SemanticVerdict, RefVerifyError> {
            Err(RefVerifyError::VerifierPort { message: "verifier adapter failure".to_owned() })
        }
    }

    #[test]
    fn pair_source_load_failure_propagates_as_verifier_port_error() {
        let source: Arc<dyn RefVerifyPairSourcePort> = Arc::new(FailingPairSource);
        let cache: Arc<dyn RefVerifyCachePort> = Arc::new(StubCache::empty());
        let verifier: Arc<dyn RefVerifierPort> =
            Arc::new(FixedVerifier { verdict: pass_verdict() });
        let interactor =
            VerifySemanticRefsInteractor::new(source, cache, verifier, RefVerifyConfig::default());

        let err = interactor.execute(&track_cmd()).unwrap_err();
        assert!(
            matches!(err, RefVerifyError::VerifierPort { .. }),
            "expected VerifierPort, got {err:?}"
        );
    }

    #[test]
    fn cache_load_failure_propagates_as_cache_persistence_error() {
        let pairs = vec![production_pair(0x10, 0x11)];
        let source: Arc<dyn RefVerifyPairSourcePort> = Arc::new(StubPairSource { pairs });
        let cache: Arc<dyn RefVerifyCachePort> = Arc::new(FailingCacheLoad);
        let verifier: Arc<dyn RefVerifierPort> =
            Arc::new(FixedVerifier { verdict: pass_verdict() });
        let interactor =
            VerifySemanticRefsInteractor::new(source, cache, verifier, RefVerifyConfig::default());

        let err = interactor.execute(&track_cmd()).unwrap_err();
        assert!(
            matches!(err, RefVerifyError::CachePersistence { .. }),
            "expected CachePersistence, got {err:?}"
        );
    }

    #[test]
    fn cache_save_failure_propagates_as_cache_persistence_error() {
        let pairs = vec![production_pair(0x20, 0x21)];
        let source: Arc<dyn RefVerifyPairSourcePort> = Arc::new(StubPairSource { pairs });
        let cache: Arc<dyn RefVerifyCachePort> = Arc::new(FailingCacheSave);
        let verifier: Arc<dyn RefVerifierPort> =
            Arc::new(FixedVerifier { verdict: pass_verdict() });
        let interactor =
            VerifySemanticRefsInteractor::new(source, cache, verifier, RefVerifyConfig::default());

        let err = interactor.execute(&track_cmd()).unwrap_err();
        assert!(
            matches!(err, RefVerifyError::CachePersistence { .. }),
            "expected CachePersistence, got {err:?}"
        );
    }

    #[test]
    fn verifier_adapter_failure_propagates_as_verifier_port_error() {
        let pairs = vec![production_pair(0x30, 0x31)];
        let source: Arc<dyn RefVerifyPairSourcePort> = Arc::new(StubPairSource { pairs });
        let cache: Arc<dyn RefVerifyCachePort> = Arc::new(StubCache::empty());
        let verifier: Arc<dyn RefVerifierPort> = Arc::new(FailingVerifier);
        let interactor =
            VerifySemanticRefsInteractor::new(source, cache, verifier, RefVerifyConfig::default());

        let err = interactor.execute(&track_cmd()).unwrap_err();
        assert!(
            matches!(err, RefVerifyError::VerifierPort { .. }),
            "expected VerifierPort, got {err:?}"
        );
    }
}
