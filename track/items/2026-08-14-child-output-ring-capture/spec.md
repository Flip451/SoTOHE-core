<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 14, yellow: 0, red: 0 }
---

# 子プロセスの診断出力を末尾リングで保持し、出力量による kill を廃止する

## Goal

- [GO-01] Allow long-running child processes to complete while retaining bounded, useful diagnostic output, without weakening the fail-closed output handling required when child output is itself a verification input. [adr: knowledge/adr/2026-08-14-1048-child-output-ring-capture.md#D1, knowledge/adr/2026-08-14-1048-child-output-ring-capture.md#D2]

## Scope

### In Scope
- [IN-01] Change diagnostic stdout and stderr capture in the review-fix runner to fixed-capacity tail-ring retention, including a clear indication when earlier output was truncated. [adr: knowledge/adr/2026-08-14-1048-child-output-ring-capture.md#D1] [tasks: T001]
- [IN-02] Apply the same diagnostic capture behavior to the program runner's equivalent child-process output surface, so reaching the diagnostic capacity does not kill the child. [adr: knowledge/adr/2026-08-14-1048-child-output-ring-capture.md#D1] [tasks: T002, T003]
- [IN-03] Preserve the separate bounded, fail-closed handling for child output whose contents are consumed as validation input, including verdict-envelope extraction. [adr: knowledge/adr/2026-08-14-1048-child-output-ring-capture.md#D2] [tasks: T001]

### Out of Scope
- [OS-01] Using tail-ring capture for verdict envelopes or any other child-output path whose contents are used as verification input. [adr: knowledge/adr/2026-08-14-1048-child-output-ring-capture.md#D2] [tasks: T001]
- [OS-02] Changing timeout behavior, invocation-config validation, repository-root working-directory enforcement, recursion detection, or first-failure command sequencing. [adr: knowledge/adr/2026-08-14-1048-child-output-ring-capture.md#D1, knowledge/adr/2026-08-02-0806-operator-owned-phase-command-config.md#D5] [tasks: T002, T003]
- [OS-03] Providing durable full-log storage for diagnostic output beyond the bounded tail retained by the process runner. [adr: knowledge/adr/2026-08-14-1048-child-output-ring-capture.md#D1] [tasks: T001, T002]

## Constraints
- [CN-01] Diagnostic child-output capture has a fixed memory capacity and preserves the tail rather than treating capacity exhaustion as a process failure; the existing timeout remains responsible for stopping a non-terminating child. [adr: knowledge/adr/2026-08-14-1048-child-output-ring-capture.md#D1] [tasks: T002]
- [CN-02] The diagnostic-capture policy must remain separate from output used as a verification input, so that validation paths retain their bounded, fail-closed behavior. [adr: knowledge/adr/2026-08-14-1048-child-output-ring-capture.md#D2] [tasks: T001]
- [CN-03] The change refines only diagnostic-output handling within infrastructure-owned bounded process execution; it does not alter command sequencing, command interpretation, or the existing invocation-validation contract. [adr: knowledge/adr/2026-08-14-1048-child-output-ring-capture.md#D1, knowledge/adr/2026-08-02-0806-operator-owned-phase-command-config.md#D3, knowledge/adr/2026-08-02-0806-operator-owned-phase-command-config.md#D5] [tasks: T001, T002, T003]

## Acceptance Criteria
- [ ] [AC-01] A review-fix runner child that emits diagnostic stdout or stderr beyond the fixed capture capacity continues running until it exits or the existing timeout stops it; the captured diagnostic output contains the most recent retained content and an explicit truncation indication. [adr: knowledge/adr/2026-08-14-1048-child-output-ring-capture.md#D1] [tasks: T001]
- [ ] [AC-02] The program runner provides the same fixed-capacity tail capture and explicit truncation indication for diagnostic child output without terminating a child merely because its diagnostic output reaches the capture capacity. [adr: knowledge/adr/2026-08-14-1048-child-output-ring-capture.md#D1] [tasks: T002, T003]
- [ ] [AC-03] A path that reads child output as validation input, including verdict-envelope extraction, retains its existing output bound and fails closed when that bound is exceeded. [adr: knowledge/adr/2026-08-14-1048-child-output-ring-capture.md#D2] [tasks: T001]
- [ ] [AC-04] Phase-command and pre-review execution continue to reject invalid invocation configuration, use the repository-root working directory, enforce their timeout and recursion protections, and stop at the first command failure. [adr: knowledge/adr/2026-08-14-1048-child-output-ring-capture.md#D1, knowledge/adr/2026-08-02-0806-operator-owned-phase-command-config.md#D5] [tasks: T002, T003]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 14  🟡 0  🔴 0

