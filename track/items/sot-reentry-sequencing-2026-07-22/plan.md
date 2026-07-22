<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# SoT 再入の順次処理規律 — ルーティング後のフェーズ収束 Prerequisite

## Summary

GO-01: T1, T2, T3, T4.

## Tasks (4/4 resolved)

### convention — Re-entry convention and discovery

> Add the convention and regenerate its index entry. IN-01, IN-08.

- [x] **T1**: Add `knowledge/conventions/sot-reentry-sequencing.md` and regenerate its `knowledge/conventions/README.md` index entry. IN-01, IN-02, IN-03, IN-04, IN-08, CN-01, CN-02, CN-03, AC-01, AC-03, AC-04, AC-05, OUT-01, OUT-02. (`b5cd26b760bc479e7d2cd89a930e80af0fc43377`)

### writer-contracts — Writer capability re-entry contracts

> Update the writer capability contracts in two reviewable batches. IN-05, IN-06, AC-02.

- [x] **T2**: Update `.harness/capabilities/spec-designer.md` and `.harness/capabilities/type-designer.md` with the required convention pointers and prerequisite checks. IN-02, IN-03, IN-05, IN-06, CN-02, CN-03, AC-02, AC-03, AC-05, OUT-01. (`b5cd26b760bc479e7d2cd89a930e80af0fc43377`)
- [x] **T3**: Update `.harness/capabilities/impl-planner.md` and `.harness/capabilities/implementer.md` with the required convention pointers and prerequisite checks. IN-02, IN-03, IN-04, IN-05, IN-06, CN-02, CN-03, AC-02, AC-03, AC-05, OUT-01. (`b5cd26b760bc479e7d2cd89a930e80af0fc43377`)

### diagnostic-boundary — Rollback diagnosis boundary

> Update the rollback-diagnoser cross-reference pointer. IN-07.

- [x] **T4**: Update `.harness/capabilities/rollback-diagnoser.md` with the convention cross-reference pointer. IN-02, IN-07, CN-02, CN-03, AC-03, AC-05, OUT-01. (`b5cd26b760bc479e7d2cd89a930e80af0fc43377`)
