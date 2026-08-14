<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 10, yellow: 0, red: 0 }
---

# review-yield measurement

## Goal

- [GO-01] Provide reproducible, read-only measurement of findings from each structured local review round, including the reviewer assignment actually used. [adr: knowledge/adr/2026-08-14-0428-review-yield-measurement.md#D1, knowledge/adr/2026-08-14-0428-review-yield-measurement.md#D3]

## Scope

### In Scope
- [IN-01] Record telemetry for every structured review round executed by `sotp review local`, including its scope, round type, actual reviewer provider, model, reasoning effort, and finding count. [adr: knowledge/adr/2026-08-14-0428-review-yield-measurement.md#D1, knowledge/adr/2026-08-14-0428-review-yield-measurement.md#D3] [tasks: T002, T004]
- [IN-02] Provide a read-only aggregation surface that reports execution counts and detection rates grouped or filtered by any recorded measurement axis. [adr: knowledge/adr/2026-08-14-0428-review-yield-measurement.md#D1] [tasks: T001, T003, T005]

### Out of Scope
- [OUT-01] Measure PR review cycles, DRY-gate judgments, test-obligation-fulfillment judges, or binary pre-review gates. [adr: knowledge/adr/2026-08-14-0428-review-yield-measurement.md#D3] [tasks: T004]
- [OUT-02] Reduce, reinforce, or otherwise alter existing inspection volume or review behavior. [adr: knowledge/adr/2026-08-14-0428-review-yield-measurement.md#D2] [tasks: T004, T005]

## Constraints
- [CO-01] Persist resolved reviewer-assignment values rather than a reference to configuration that produced them. [adr: knowledge/adr/2026-08-14-0428-review-yield-measurement.md#D1] [tasks: T002, T004]
- [CO-02] The aggregation surface must not mutate telemetry, review results, configuration, or inspection behavior. [adr: knowledge/adr/2026-08-14-0428-review-yield-measurement.md#D1, knowledge/adr/2026-08-14-0428-review-yield-measurement.md#D2] [tasks: T001, T003, T004, T005]

## Acceptance Criteria
- [ ] [AC-01] After each structured `sotp review local` round, its telemetry contains the round scope and type, the resolved provider, model, and reasoning effort, and the number of findings produced. [adr: knowledge/adr/2026-08-14-0428-review-yield-measurement.md#D1, knowledge/adr/2026-08-14-0428-review-yield-measurement.md#D3] [tasks: T002, T004]
- [ ] [AC-02] The aggregation surface can report execution counts and detection rates for any selected recorded axis without changing stored telemetry or review execution; the detection rate is the proportion of matching recorded rounds with one or more findings, and it reports no rate when no recorded rounds match. [adr: knowledge/adr/2026-08-14-0428-review-yield-measurement.md#D1] [tasks: T001, T003, T005]
- [ ] [AC-03] The feature neither adds nor removes review, gate, or inspection work, and produces no measurement records for the excluded review and gate entry points. [adr: knowledge/adr/2026-08-14-0428-review-yield-measurement.md#D2, knowledge/adr/2026-08-14-0428-review-yield-measurement.md#D3] [tasks: T002, T004]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 10  🟡 0  🔴 0

