<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# テスト義務ゲートが正しい実装を Stale と報告し続ける欠陥の修正（ADR 準拠回復）

## Summary

T001 establishes shared cache-lookup and binding fail-closed errors, then updates `ObligationFulfillmentCacheDocument` and `CheckTestObligationsInteractor`; GO-01, GO-02.
T002 updates the fulfillment-cache load contract, adapter, recovery errors, check outcome, and `EvaluateTestObligationsInteractor` together to reuse the established full-key lookup and recover stale active-track cache state; GO-01, GO-02.
T003 updates `ObligationsDocument`, `CheckTestObligationsInteractor`, and `EvaluateTestObligationsInteractor` with voluntary-binding ownership validation; GO-02.
T004 updates `EdgeVerdictRecord`, `EdgeResolutionOutcome`, `ObligationFulfillmentCacheEntry`, `ResolvedBoundTests`, `EvaluateTestObligationsInteractor`, `TestObligationResultsInteractor`, `ObligationFulfillmentCacheEntryDto`, and `JsonObligationFulfillmentCacheCodec`; GO-01.

## Tasks (0/4 resolved)

### S1 — Batch T001

- [ ] **T001**: Establish `FulfillmentCacheLookupError` and `TestBindingConsistencyError` as the shared fail-closed error surface; update `ObligationFulfillmentCacheDocument` and `CheckTestObligationsInteractor` with full-key lookup and check migration; add domain and usecase regressions. IN-01, IN-02; OUT-04; CN-01, CN-02, CN-03; AC-01, AC-02.

### S2 — Batch T002

- [ ] **T002**: Update `FulfillmentCacheReevaluationReason`, `ObligationFulfillmentCacheLoad`, `VerifyCacheError`, `ObligationFulfillmentCacheEntryError`, `ObligationEvaluateError`, `ObligationCheckError`, `ObligationFulfillmentCachePort`, `JsonObligationFulfillmentCacheCodec`, and the `CheckTestObligationsInteractor`, `EvaluateTestObligationsInteractor`, and `TestObligationResultsInteractor` cache-port consumers as one recovery boundary. Add the load-state classification and its error propagation, adapt each port consumer to the changed `load` result, and add active-track stale-cache recovery and evaluation-error regressions. IN-01, IN-02; OUT-01, OUT-02; CN-02, CN-03; AC-01, AC-06, AC-07.

### S3 — Batch T003

- [ ] **T003**: Update `ObligationsDocument`, `CheckTestObligationsInteractor`, and `EvaluateTestObligationsInteractor` with voluntary-binding ownership validation that returns the established binding-consistency errors; add regression fixtures. IN-03; OUT-03; CN-03; AC-03.

### S4 — Batch T004

- [ ] **T004**: Update `EdgeVerdictRecord`, `EdgeResolutionOutcome`, `ObligationFulfillmentCacheEntry`, `ResolvedBoundTests`, `EvaluateTestObligationsInteractor`, `TestObligationResultsInteractor`, `ObligationFulfillmentCacheEntryDto`, and `JsonObligationFulfillmentCacheCodec` for nested-verdict-reason canonicalization and cache-entry bound-test diagnostics; add domain, DTO/codec, and evaluate/result fixtures. IN-04, IN-05; OUT-04; CN-04; AC-04, AC-05.
