<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 12, yellow: 0, red: 0 }
---

# Base Merge Commit Record Advance

## Goal

- [GO-01] Keep the active track's commit record aligned with merge HEAD after base-merge cleanup so post-merge review scope and signal evaluation exclude changes owned by the incorporated base. [adr: knowledge/adr/2026-08-14-1049-base-merge-commit-record-advance.md#D1]

## Scope

### In Scope
- [IN-01] Advance the active track's commit record to merge-result HEAD as part of clean base-merge cleanup. [adr: knowledge/adr/2026-08-14-1049-base-merge-commit-record-advance.md#D1] [tasks: T1]
- [IN-02] Advance the active track's commit record to merge-result HEAD when conflict recovery completes. [adr: knowledge/adr/2026-08-14-1049-base-merge-commit-record-advance.md#D1] [tasks: T2]

### Out of Scope
- [OS-01] Changing the existing Baseline → Views cleanup ordering is out of scope. [adr: knowledge/adr/2026-08-14-1049-base-merge-commit-record-advance.md#D1, knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D3] [tasks: T1, T2]
- [OS-02] Creating a Git commit as part of commit-record advancement is out of scope. [adr: knowledge/adr/2026-08-14-1049-base-merge-commit-record-advance.md#D1] [tasks: T1, T2]

## Constraints
- [CN-01] Both clean-merge and conflict-recovery completion must use the existing commit-record update path to record the merge-result HEAD. [adr: knowledge/adr/2026-08-14-1049-base-merge-commit-record-advance.md#D1] [tasks: T1, T2]
- [CN-02] Commit-record advancement is part of merge cleanup's completion condition and must preserve the existing Baseline → Views ordering. [adr: knowledge/adr/2026-08-14-1049-base-merge-commit-record-advance.md#D1, knowledge/adr/2026-08-02-0715-base-merge-cleanup-state.md#D3] [tasks: T1, T2]
- [CN-03] Failure of commit-record advancement is fail-closed: cleanup must not claim success while the required record is absent. [adr: knowledge/adr/2026-08-14-1049-base-merge-commit-record-advance.md#D1] [tasks: T1, T2]

## Acceptance Criteria
- [ ] [AC-01] After a clean base merge completes, the merge-result HEAD is recorded as the active track's commit record through the existing commit-record update path. [adr: knowledge/adr/2026-08-14-1049-base-merge-commit-record-advance.md#D1] [tasks: T1]
- [ ] [AC-02] After conflict recovery completes, the merge-result HEAD is recorded as the active track's commit record through the existing commit-record update path. [adr: knowledge/adr/2026-08-14-1049-base-merge-commit-record-advance.md#D1] [tasks: T2]
- [ ] [AC-03] If the commit-record update fails, cleanup reports failure and does not report a partially completed cleanup as successful. [adr: knowledge/adr/2026-08-14-1049-base-merge-commit-record-advance.md#D1] [tasks: T1, T2]
- [ ] [AC-04] The commit-record update rewrites only track state and does not create a Git commit. [adr: knowledge/adr/2026-08-14-1049-base-merge-commit-record-advance.md#D1] [tasks: T1, T2]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 12  🟡 0  🔴 0

