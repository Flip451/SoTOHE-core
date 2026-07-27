<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# テスト義務ゲートが正しい実装を Stale と報告し続ける欠陥の修正（ADR 準拠回復）

## Summary

T001 adds domain lookup and consistency targets. GO-01, GO-02.
T002 updates the cache/evidence targets and their transition consumers. GO-01, GO-02.
T003 updates check and binding-validation targets. GO-01, GO-02.
T004–T008 update result, evaluation, and cache targets. GO-01, GO-02.

## Tasks (8/8 resolved)

### S1 — Batch T001

> Add domain lookup and consistency targets. IN-01, IN-02; OUT-04; CN-01, CN-02, CN-03; AC-01, AC-02.
> Expected review scope: domain approximately 260 changed lines; usecase and infrastructure have no T001-owned changes.

- [x] **T001**: Add `FulfillmentCacheLookupError` and `TestBindingConsistencyError`; update `ObligationFulfillmentCacheDocument` with full-key lookup; add domain regressions. IN-01, IN-02; OUT-04; CN-01, CN-02, CN-03; AC-01, AC-02. (`062ed4ea`)

### S2 — Batch T002 — cache and evidence foundation

> Update cache/evidence targets, remove `FulfillmentCacheReevaluationReason` and its tests, and migrate consumers and codec trait implementations. IN-01, IN-02, IN-05; OUT-01, OUT-02; CN-02, CN-03, CN-04; AC-01, AC-05, AC-06.
> Expected review scope: domain approximately 440 changed lines, usecase approximately 480 changed lines, infrastructure approximately 395 changed lines, composition approximately 80 changed lines; each scope remains below the 500-line ceiling.

- [x] **T002**: Add `ResolvedBoundTestsResolver`, `ResolvedBoundTestsResolverPort`, `ResolvedBoundTests`, `ContentHasherPort`, `Sha256ContentHasher`, and `domain::tddd::test_obligation::ids::unavailable_diagnostic_message`; update cache-entry, cache-port, and `VerifyCacheError` targets; remove `FulfillmentCacheReevaluationReason`; migrate consumers, composition, test doubles, and codec trait implementations; add regressions. IN-01, IN-02, IN-05; OUT-01, OUT-02; CN-02, CN-03, CN-04; AC-01, AC-05, AC-06.

### S3 — Batch T003 — check and binding validation

> Update check and binding-validation targets. IN-01, IN-02, IN-03; OUT-03; CN-01, CN-02, CN-03; AC-01, AC-02, AC-03, AC-06.
> Expected review scope: domain approximately 150 changed lines, usecase approximately 490 changed lines, infrastructure has no T003-owned changes; each scope remains below the 500-line ceiling.

- [x] **T003**: Update `ObligationsDocument`, `ObligationCheckError`, and `CheckTestObligationsInteractor`; use `TestBindingConsistencyError`; add check finding/exit, duplicate-current-key, historical-row, and binding-consistency regressions. IN-01, IN-02, IN-03; OUT-03; CN-01, CN-02, CN-03; AC-01, AC-02, AC-03, AC-06.

### S4 — Batch T004 — result model and projection

> Update result-model and result-projection targets; migrate constructors. IN-04; CN-04; AC-04.
> Expected review scope: domain approximately 260 changed lines, usecase approximately 460 changed lines, infrastructure has no T004-owned changes; each scope remains below the 500-line ceiling.

- [x] **T004**: Update `EdgeResolutionOutcome`, `EdgeVerdictRecord`, and `TestObligationResultsInteractor`; migrate `EvaluateTestObligationsInteractor` constructors; add result fixtures. IN-04; CN-04; AC-04.

### S5 — Batch T005 — evaluation boundary

> Update evaluation targets and add fixtures. IN-01, IN-04, IN-05; OUT-04; CN-04; AC-04, AC-05, AC-07.
> Expected review scope: domain approximately 120 changed lines, usecase approximately 440 changed lines, infrastructure has no T005-owned changes; each scope remains below the 500-line ceiling.

- [x] **T005**: Update `ObligationEvaluateError` and `EvaluateTestObligationsInteractor`; add evaluation fixtures. IN-01, IN-04, IN-05; OUT-04; CN-04; AC-04, AC-05, AC-07.

### S6 — Batch T006 — cache DTO and mapping

> Update cache DTO and in-memory mapping targets. IN-05; OUT-04; CN-04; AC-05.
> Expected review scope: infrastructure approximately 420 changed lines; each scope remains below the 500-line ceiling.

- [x] **T006**: Update `ObligationFulfillmentCacheEntryDto` and in-memory `JsonObligationFulfillmentCacheCodec` mapping; add mapping fixtures. IN-05; OUT-04; CN-04; AC-05.

### S7 — Batch T007 — cache decode

> Update cache-load and legacy-decode targets. IN-05; OUT-04; CN-04; AC-05.
> Expected review scope: infrastructure approximately 410 changed lines; each scope remains below the 500-line ceiling.

- [x] **T007**: Update `JsonObligationFulfillmentCacheCodec` cache-load and legacy-decode paths; add decode regressions. IN-05; OUT-04; CN-04; AC-05.

### S8 — Batch T008 — cache write

> Update cache-file write and persistence-fixture targets. IN-05; OUT-04; CN-04; AC-05.
> Expected review scope: infrastructure approximately 480 changed lines; each scope remains below the 500-line ceiling.

- [x] **T008**: Update the `JsonObligationFulfillmentCacheCodec` cache-file write path and `fulfillment_cache_io`; add persistence fixtures. IN-05; OUT-04; CN-04; AC-05.
