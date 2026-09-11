<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# テスト義務検証の仕様区分・対象責務・鮮度の整合性修復

## Summary

T001-T009 implement GO-01 through the cited specification anchors.
The dependency order closes API migration in B1, then completes shared freshness regressions and the validation handoff in B2.

## Tasks (0/9 resolved)

### S1 — Typed request and compatibility closure

> `libs/domain/src/tddd/{semantic_verify,test_obligation}/`: update the catalogued request, pair, hash, cache-key, and verifier-port symbols; add domain unit tests. IN-01; IN-03; IN-04; OUT-01; OUT-02; OUT-04; CN-01; CN-02; AC-01; AC-03.
> Non-evaluate use-case, infrastructure, CLI-driver, and CLI-composition constructor/codec call sites, followed by the evaluate request/cache-key files: migrate the domain API within one buildable batch. IN-01; IN-03; IN-04; OUT-01; OUT-02; OUT-04; CN-01; CN-02; AC-01; AC-03.

- [ ] **T001**: `libs/domain/src/tddd/test_obligation/{hashes,pair,verdict,ports}.rs` and `libs/domain/src/tddd/semantic_verify/`: add `SpecElementHash` and `ObligationResponsibilityHash`; replace `AnchorText`/`AnchorTextHash` uses with the catalogued `SpecElementRef`; update pair, cache-key, and `WaiverVerifierPort` constructors/accessors; update domain tests. IN-01; IN-03; IN-04; OUT-01; OUT-02; OUT-04; CN-01; CN-02; AC-01; AC-03.
- [ ] **T002**: Exclusively migrate the non-evaluate pair/cache-key/port/interactor call sites in `libs/usecase/src/test_obligation/{check.rs,check_tests.rs,results.rs,results_tests.rs,evaluate/calibration_runner.rs}`, `libs/infrastructure/src/test_obligation/{fulfillment_cache_codec.rs,fulfillment_cache_codec/tests.rs,waiver_cache_codec.rs,fulfillment_escalation_driver.rs,waiver_escalation_driver.rs,sha256_content_hasher.rs}`, `apps/cli-driver/src/test_obligation/check.rs`, and `apps/cli-composition/src/test_obligation.rs`; update the fulfillment and waiver cache wire conversions and compatibility tests. IN-01; IN-03; IN-04; OUT-01; OUT-02; OUT-04; CN-01; CN-02; AC-01; AC-03.

### S2 — Evaluate, check, and results freshness

> `libs/usecase/src/test_obligation/evaluate/`: update request construction and freshness operations with focused tests. IN-01; IN-03; IN-04; IN-05; OUT-01; OUT-02; CN-01; CN-02; AC-01; AC-02; AC-03; AC-04.
> `libs/usecase/src/test_obligation/{check,results,check_support}.rs`: update shared freshness operations and focused tests. IN-01; IN-02; IN-03; IN-04; IN-05; OUT-02; OUT-03; CN-01; AC-01; AC-02; AC-03; AC-04.

- [ ] **T003**: Exclusively update `EvaluateTestObligationsInteractor`, request builders, cache-key derivation, constructor migrations, and focused unit tests in `libs/usecase/src/test_obligation/evaluate/{mod.rs,edges.rs,plan.rs,cache.rs,tests.rs}`. IN-01; IN-03; IN-04; IN-05; OUT-01; OUT-02; CN-01; CN-02; AC-01; AC-02; AC-03; AC-04.
- [ ] **T004**: `libs/usecase/src/test_obligation/{check.rs,check_support.rs,check_tests.rs,results.rs,results_tests.rs}`: update `CheckTestObligationsInteractor` and `TestObligationResultsInteractor` freshness checks; add focused unit tests. IN-01; IN-02; IN-03; IN-04; IN-05; OUT-02; OUT-03; CN-01; AC-01; AC-02; AC-03; AC-04.

### S3 — Verifier adapters and fingerprints

> `libs/infrastructure/src/test_obligation/fulfillment_verifier.rs`: update the catalogued adapter and fingerprint with focused tests. IN-01; IN-02; IN-03; IN-04; IN-05; OUT-01; OUT-04; CN-01; CN-02; AC-01; AC-02; AC-03; AC-04.
> `libs/infrastructure/src/test_obligation/waiver_verifier.rs`: update the catalogued adapters and fingerprint with focused tests. IN-01; IN-02; IN-03; IN-04; IN-05; OUT-01; OUT-04; CN-01; CN-02; AC-01; AC-02; AC-03; AC-04.

- [ ] **T005**: `libs/infrastructure/src/test_obligation/fulfillment_verifier.rs`: update `ObligationFulfillmentVerifierAdapter`, prompt rendering, and `fulfillment_verifier_fingerprint`; update its test module. IN-01; IN-02; IN-03; IN-04; IN-05; OUT-01; OUT-04; CN-01; CN-02; AC-01; AC-02; AC-03; AC-04.
- [ ] **T006**: `libs/infrastructure/src/test_obligation/waiver_verifier.rs`: update `WaiverVerifierAdapter`, `FailingWaiverVerifier`, prompt rendering, and `waiver_verifier_fingerprint`; update its test module. IN-01; IN-02; IN-03; IN-04; IN-05; OUT-01; OUT-04; CN-01; CN-02; AC-01; AC-02; AC-03; AC-04.

### S4 — Deterministic regression units

> Verifier adapter test modules and evaluate/check/results regression test modules: add the bounded deterministic cases cited by the specification. IN-01; IN-02; IN-03; IN-04; IN-05; OUT-01; OUT-02; OUT-03; OUT-04; CN-01; CN-02; AC-01; AC-02; AC-03; AC-04.

- [ ] **T007**: The `#[cfg(test)]` modules in `libs/infrastructure/src/test_obligation/{fulfillment_verifier.rs,waiver_verifier.rs}`: add table-driven verifier-lane/spec-section and bounded diagnostic/calibration cases using `track/items/2026-09-11-test-obligation-verifier-context-integrity/research/verifier-input-diagnosis.md` only as diagnostic evidence. IN-01; IN-02; IN-03; OUT-01; CN-02; AC-01; AC-02.
- [ ] **T008**: `libs/usecase/src/test_obligation/{evaluate/tests.rs,check_tests.rs,results_tests.rs}` and `libs/infrastructure/src/test_obligation/{fulfillment_cache_codec/tests.rs,waiver_cache_codec.rs}` test modules: add stale-Pass/stale-Fail evaluate/check/results and cache-wire regression matrices. IN-04; IN-05; OUT-02; OUT-03; OUT-04; CN-01; AC-03; AC-04.

### S5 — Binary and root-host validation

> `target/release/sotp`, root-host test-obligation commands, and the existing consumer cache artifacts: execute the normal validation handoff. IN-05; OUT-03; CN-01; AC-04.

- [ ] **T009**: `target/release/sotp`, root-host `bin/sotp test-obligation evaluate`, `bin/sotp test-obligation check`, and `bin/sotp test-obligation results`, plus the consumer `obligation-fulfillment-cache.json` and `waiver-cache.json`: run the normal binary update and cache-preserving validation handoff without repository or consumer production edits. IN-05; OUT-03; CN-01; AC-04.
