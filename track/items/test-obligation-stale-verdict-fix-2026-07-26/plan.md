<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# テスト義務ゲートが正しい実装を Stale と報告し続ける欠陥の修正（ADR 準拠回復）

## Summary

T001 adds `FulfillmentCacheLookupError`, `FulfillmentCacheReevaluationReason`, and `TestBindingConsistencyError`, then completes the `ObligationCheckError` payload surface and updates `ObligationFulfillmentCacheDocument` and `CheckTestObligationsInteractor`; GO-01, GO-02.
T002 updates the fulfillment-cache load contract, row/DTO persistence, adapter, recovery errors, check outcome, and cache-port consumers; GO-01, GO-02.
T003 updates `ObligationsDocument`, `CheckTestObligationsInteractor`, and `EvaluateTestObligationsInteractor` to apply voluntary-binding ownership validation through `TestBindingConsistencyError`; GO-02.
T004 updates `EdgeVerdictRecord`, `EdgeResolutionOutcome`, `EvaluateTestObligationsInteractor`, `TestObligationResultsInteractor`, and `JsonObligationFulfillmentCacheCodec`; GO-01.

## Tasks (1/4 resolved)

### S1 — Batch T001

- [x] **T001**: Add `FulfillmentCacheLookupError`, `FulfillmentCacheReevaluationReason`, and `TestBindingConsistencyError`; update `ObligationFulfillmentCacheDocument`, the complete `ObligationCheckError` payload surface, and `CheckTestObligationsInteractor` with full-key lookup and check migration; add domain and usecase regressions. IN-01, IN-02; OUT-04; CN-01, CN-02, CN-03; AC-01, AC-02.

### S2 — Batch T002

- [ ] **T002**: Update `ObligationFulfillmentCacheLoad`, `VerifyCacheError`, `ObligationFulfillmentCacheEntryError`, `ObligationEvaluateError`, `ObligationFulfillmentCacheEntry`, `ResolvedBoundTests`, `ObligationFulfillmentCachePort`, `ObligationFulfillmentCacheEntryDto`, `JsonObligationFulfillmentCacheCodec`, and the `CheckTestObligationsInteractor`, `EvaluateTestObligationsInteractor`, and `TestObligationResultsInteractor` cache-port consumers as one recovery boundary. Add the load-state classification and its error propagation, persist `ResolvedBoundTests` during reevaluation, adapt each port consumer to the changed `load` result, and add active-track stale-cache recovery and evaluation-error regressions. IN-01, IN-02; OUT-01, OUT-02; CN-02, CN-03; AC-01, AC-06, AC-07.

### S3 — Batch T003

- [ ] **T003**: Update `ObligationsDocument`, `CheckTestObligationsInteractor`, and `EvaluateTestObligationsInteractor` to apply voluntary-binding ownership validation through `TestBindingConsistencyError`, with finding/exit regression fixtures. IN-03; OUT-03; CN-03; AC-03.

### S4 — Batch T004

- [ ] **T004**: Update `EdgeVerdictRecord`, `EdgeResolutionOutcome`, `EvaluateTestObligationsInteractor`, `TestObligationResultsInteractor`, and `JsonObligationFulfillmentCacheCodec`: remove the sibling result-reason field, revise nested-verdict and result-projection handling, and update codec result-entry mappings. Add domain, codec, and evaluate/result fixtures. IN-04, IN-05; OUT-04; CN-04; AC-04, AC-05.
