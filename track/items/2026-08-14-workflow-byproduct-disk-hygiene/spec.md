<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 19, yellow: 0, red: 0 }
---

# Workflow byproduct disk hygiene

## Goal

- [GO-01] Eliminate the workflow failures and disk-pressure recurrence caused by untracked nested repositories and template-export test scaffolds, while preserving the production template-export contract. [adr: knowledge/adr/2026-08-02-0643-workflow-byproduct-disk-hygiene.md#D1, knowledge/adr/2026-08-02-0643-workflow-byproduct-disk-hygiene.md#D3]

## Scope

### In Scope
- [IN-01] Replace the hardcoded scope-diff exclusion set with an operator-owned configuration under `.harness/config/`; load it for untracked-path enumeration as top-level git exclusion pathspecs. The shipped configuration includes the current exclusions plus `.cache/**`, `tmp/**`, `libs/**/tmp/**`, and `apps/**/tmp/**`. [adr: knowledge/adr/2026-08-02-0643-workflow-byproduct-disk-hygiene.md#D1] [tasks: T001]
- [IN-02] Exclude trailing-`/` directory entries reported by `git ls-files --others` from the untracked measurement set, while retaining existing fail-closed handling for other non-regular filesystem entries. [adr: knowledge/adr/2026-08-02-0643-workflow-byproduct-disk-hygiene.md#D2] [tasks: T002]
- [IN-03] Place template-export test scaffolds under `CARGO_TARGET_TMPDIR` when it is available, with a defined fallback for test invocation paths where that environment variable is absent. [adr: knowledge/adr/2026-08-02-0643-workflow-byproduct-disk-hygiene.md#D3] [tasks: T003]
- [IN-04] Remove the process-lifetime `static OnceLock<TempDir>` sharing pattern from `consumer_scaffold_host_first.rs` so its scaffold lifecycle no longer bypasses cleanup. [adr: knowledge/adr/2026-08-02-0643-workflow-byproduct-disk-hygiene.md#D4] [tasks: T004]
- [IN-05] Use a same-filesystem hard link for the test-path transplant of `bin/sotp`, retaining byte-identity verification for the transplanted binary. [adr: knowledge/adr/2026-08-02-0643-workflow-byproduct-disk-hygiene.md#D5] [tasks: T005]

### Out of Scope
- [OS-01] Changing the production template-export behavior, output artifacts, or its binary-copy transplant mechanism. [adr: knowledge/adr/2026-08-02-0643-workflow-byproduct-disk-hygiene.md#D5] [tasks: T005]
- [OS-02] Treating periodic `/tmp` cleanup as the permanent remediation for the scaffold leak. [adr: knowledge/adr/2026-08-02-0643-workflow-byproduct-disk-hygiene.md#D3] [tasks: T003]

## Constraints
- [CN-01] The scope-diff exclusion configuration is mandatory and fail-closed: absent, invalid, or empty configuration is a hard error rather than an implicit fallback to a partial or hardcoded policy. [adr: knowledge/adr/2026-08-02-0643-workflow-byproduct-disk-hygiene.md#D1] [tasks: T001]
- [CN-02] The directory-entry exception is structural and non-configurable; it must not relax fail-closed behavior for symlinks or other non-regular files. [adr: knowledge/adr/2026-08-02-0643-workflow-byproduct-disk-hygiene.md#D2] [tasks: T002]
- [CN-03] Test scaffold placement must keep retained artifacts within a cargo-cleanable or `sotp maintenance`-cleanable location instead of the shared `/tmp` area. [adr: knowledge/adr/2026-08-02-0643-workflow-byproduct-disk-hygiene.md#D3] [tasks: T003]
- [CN-04] The hard-link optimization is limited to the test transplant path and must preserve the existing byte-identical assertion; production export continues to copy its binary. [adr: knowledge/adr/2026-08-02-0643-workflow-byproduct-disk-hygiene.md#D5] [tasks: T005]

## Acceptance Criteria
- [ ] [AC-01] With a valid shipped scope-diff configuration, untracked enumeration excludes the configured patterns, including `.cache/**`, `tmp/**`, `libs/**/tmp/**`, and `apps/**/tmp/**`, without relying on a hardcoded exclusion array. [adr: knowledge/adr/2026-08-02-0643-workflow-byproduct-disk-hygiene.md#D1] [tasks: T001]
- [ ] [AC-02] A missing, malformed, or empty scope-diff configuration causes measurement to fail with a hard error; it never silently proceeds with an incomplete exclusion policy. [adr: knowledge/adr/2026-08-02-0643-workflow-byproduct-disk-hygiene.md#D1] [tasks: T001]
- [ ] [AC-03] When untracked enumeration reports a trailing-`/` nested-repository entry, scope-diff measurement completes without attempting to count that directory; an encountered symlink or other unsupported non-regular file still fails closed. [adr: knowledge/adr/2026-08-02-0643-workflow-byproduct-disk-hygiene.md#D2] [tasks: T002]
- [ ] [AC-04] Template-export integration and in-process tests create scaffolds beneath `CARGO_TARGET_TMPDIR` when cargo supplies it, and remain runnable through an invocation path where the variable is not defined. [adr: knowledge/adr/2026-08-02-0643-workflow-byproduct-disk-hygiene.md#D3] [tasks: T003]
- [ ] [AC-05] The host-first consumer scaffold tests no longer retain a `TempDir` in static process-lifetime storage, and their scaffold lifecycle permits cleanup after test use. [adr: knowledge/adr/2026-08-02-0643-workflow-byproduct-disk-hygiene.md#D4] [tasks: T004]
- [ ] [AC-06] The test-path binary transplant uses a same-filesystem hard link and the resulting target remains byte-identical to its source. [adr: knowledge/adr/2026-08-02-0643-workflow-byproduct-disk-hygiene.md#D5] [tasks: T005]
- [ ] [AC-07] Production template export continues to copy its binary and produces the same production export behavior and artifacts as before this track. [adr: knowledge/adr/2026-08-02-0643-workflow-byproduct-disk-hygiene.md#D5] [tasks: T005]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 19  🟡 0  🔴 0

