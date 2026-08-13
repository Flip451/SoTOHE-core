<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 13, yellow: 0, red: 0 }
---

# sotp generated JSON serialization determinism

## Goal

- [GO-01] Ensure that every JSON artifact generated or updated by sotp, with review.json prioritized, has a deterministic key order so identical logical content produces an identical byte sequence. [adr: knowledge/adr/2026-07-29-0839-deterministic-json-serialization.md#D1]

## Scope

### In Scope
- [IN-01] Apply deterministic serialization to every JSON artifact that sotp generates or updates, prioritizing review.json. [adr: knowledge/adr/2026-07-29-0839-deterministic-json-serialization.md#D1] [tasks: T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16, T17, T18, T19, T20, T23, T24, T25, T26, T27, T28, T29]
- [IN-02] Make repeated serialization of identical logical JSON content yield identical bytes, eliminating key-order-only diff and freshness-check churn. [adr: knowledge/adr/2026-07-29-0839-deterministic-json-serialization.md#D1] [tasks: T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16, T17, T18, T19, T20, T23, T24, T25, T26, T27, T28, T29]
- [IN-03] Permit a one-time key-order-only rearrangement when an active track's generated JSON artifacts are regenerated under the deterministic behavior. [adr: knowledge/adr/2026-07-29-0839-deterministic-json-serialization.md#D1] [tasks: T21]

### Out of Scope
- [OS-01] Selecting the concrete deterministic-serialization mechanism. [adr: knowledge/adr/2026-07-29-0839-deterministic-json-serialization.md#D1] [tasks: T1]
- [OS-02] Bulk reserializing JSON artifacts belonging to completed tracks. [adr: knowledge/adr/2026-07-29-0839-deterministic-json-serialization.md#D1] [tasks: T22]

## Constraints
- [CN-01] The behavior must be implementation-mechanism-neutral; the implementation track selects how deterministic serialization is achieved without weakening the byte-identical result. [adr: knowledge/adr/2026-07-29-0839-deterministic-json-serialization.md#D1] [tasks: T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16, T17, T18, T19, T20, T23, T24, T25, T26, T27, T28, T29]
- [CN-02] Migration effects are limited to the key order of regenerated artifacts on active tracks; completed-track artifacts must remain untouched. [adr: knowledge/adr/2026-07-29-0839-deterministic-json-serialization.md#D1] [tasks: T21, T22]
- [CN-03] This track introduces no artifact-size or serialization-latency requirement; any performance reassessment is a separate future ADR decision and does not change the deterministic-byte acceptance criteria. [adr: knowledge/adr/2026-07-29-0839-deterministic-json-serialization.md#D1] [tasks: T1]

## Acceptance Criteria
- [ ] [AC-01] For each sotp JSON artifact-generation or update path, two writes from identical logical JSON content produce byte-identical output. [adr: knowledge/adr/2026-07-29-0839-deterministic-json-serialization.md#D1] [tasks: T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15, T16, T17, T18, T19, T20, T23, T24, T25, T26, T27, T28, T29]
- [ ] [AC-02] review.json is covered by the deterministic behavior and does not produce a diff when only serialization is repeated for unchanged logical content. [adr: knowledge/adr/2026-07-29-0839-deterministic-json-serialization.md#D1] [tasks: T3]
- [ ] [AC-03] Regenerating an active track's JSON artifacts may produce a one-time diff that changes key order only; subsequent unchanged regenerations produce no byte-level churn. [adr: knowledge/adr/2026-07-29-0839-deterministic-json-serialization.md#D1] [tasks: T21]
- [ ] [AC-04] The implementation does not regenerate or rewrite JSON artifacts for completed tracks solely to normalize key order. [adr: knowledge/adr/2026-07-29-0839-deterministic-json-serialization.md#D1] [tasks: T22]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 13  🟡 0  🔴 0

