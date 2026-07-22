<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# SoT 再入の順次処理規律 — ルーティング後のフェーズ収束 Prerequisite

## Summary

GO-01: T1, T2, T3, T4, T5, T6, T7.

## Tasks (7/7 resolved)

### convention — Re-entry convention and discovery

> Add `knowledge/conventions/sot-reentry-sequencing.md` and regenerate its `knowledge/conventions/README.md` index entry. IN-01, IN-08, IN-09, IN-10, AC-07, AC-08.

- [x] **T1**: Add `knowledge/conventions/sot-reentry-sequencing.md` and regenerate its `knowledge/conventions/README.md` index entry. IN-01, IN-02, IN-03, IN-04, IN-08, IN-09, IN-10, CN-01, CN-02, CN-03, AC-01, AC-03, AC-04, AC-05, AC-07, AC-08, OUT-01, OUT-02. (`b5cd26b760bc479e7d2cd89a930e80af0fc43377`)

### writer-contracts — Writer capability re-entry contracts

> Update the writer capability contracts in two reviewable batches, including deferred-verification briefing wording where applicable. IN-05, IN-06, IN-10, AC-02, AC-08.

- [x] **T2**: Update `.harness/capabilities/spec-designer.md` and `.harness/capabilities/type-designer.md` with the required convention pointers, prerequisite checks, and deferred-verification briefing wording. IN-02, IN-03, IN-05, IN-06, IN-10, CN-02, CN-03, AC-02, AC-03, AC-05, AC-08, OUT-01. (`b5cd26b760bc479e7d2cd89a930e80af0fc43377`)
- [x] **T3**: Update `.harness/capabilities/impl-planner.md` and `.harness/capabilities/implementer.md` with the required convention pointers and prerequisite checks. IN-02, IN-03, IN-04, IN-05, IN-06, CN-02, CN-03, AC-02, AC-03, AC-05, OUT-01. (`b5cd26b760bc479e7d2cd89a930e80af0fc43377`)

### diagnostic-boundary — Rollback diagnosis and workflow re-entry dispatch

> Update the rollback-diagnoser cross-reference plus diagnose and plan workflow re-entry briefing requirements, including deferred-verification recording. IN-02, IN-03, IN-07, IN-10, AC-06, AC-08.

- [x] **T4**: Update `.harness/capabilities/rollback-diagnoser.md` with the convention cross-reference pointer. IN-02, IN-07, CN-02, CN-03, AC-03, AC-05, OUT-01. (`b5cd26b760bc479e7d2cd89a930e80af0fc43377`)
- [x] **T5**: Update `.harness/workflows/track/diagnose.md` Step 3 with the pre-dispatch convergence-evidence check. IN-02, AC-05, AC-06. (`6b3f712a543c552e3531befee6c874ab40505ef5`)
- [x] **T6**: Update `.harness/workflows/track/plan.md` Phase 1 and Phase 2 back-and-forth loops with the required pre-re-dispatch confirmation and deferred-verification recording. IN-02, IN-03, IN-10, CN-02, CN-03, AC-05, AC-06, AC-08. (`c7754771f75ee698db257d16249839c793a535ff`)

### single-scope-review-reentry — Single-scope review re-entry lifecycle

> Add the designated single-SoT-scope review re-entry round and align plan-loop references. IN-02, CN-02, CN-03, AC-05, AC-06.

- [x] **T7**: Add the designated single-SoT-scope re-entry round in `.harness/workflows/track/review.md` and align the `.harness/workflows/track/plan.md` references. IN-02, CN-02, CN-03, AC-05, AC-06. (`c7754771f75ee698db257d16249839c793a535ff`)
