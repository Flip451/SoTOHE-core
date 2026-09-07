<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 14, yellow: 0, red: 0 }
---

# Catalogue Impl Signals Absolute Workspace Root

## Goal

- [GO-01] Make catalogue-impl-signals reliably exclude Cargo's target directory when invoked with its default or a relative workspace root. [adr: knowledge/adr/2026-09-04-0058-catalogue-impl-signals-absolute-workspace-root.md#D1]

## Scope

### In Scope
- [IN-01] Normalize the default and explicit workspace-root inputs to absolute paths at command startup. [adr: knowledge/adr/2026-09-04-0058-catalogue-impl-signals-absolute-workspace-root.md#D1] [tasks: T1]
- [IN-02] Use normalized absolute paths for target-directory exclusion and other comparable workspace-root path comparisons. [adr: knowledge/adr/2026-09-04-0058-catalogue-impl-signals-absolute-workspace-root.md#D1] [tasks: T1, T2]
- [IN-03] Reject an unavailable or inaccessible workspace root before catalogue processing begins. [adr: knowledge/adr/2026-09-04-0058-catalogue-impl-signals-absolute-workspace-root.md#D1] [tasks: T1]
- [IN-04] Add regression coverage that proves a relative workspace-root input still excludes target/. [adr: knowledge/adr/2026-09-04-0058-catalogue-impl-signals-absolute-workspace-root.md#D1] [tasks: T2]

### Out of Scope
- [OS-01] Requiring users to supply --workspace-root $PWD is out of scope. [adr: knowledge/adr/2026-09-04-0058-catalogue-impl-signals-absolute-workspace-root.md#D1] [tasks: T2]
- [OS-02] Adding relative-versus-absolute path handling separately at every comparison site is out of scope. [adr: knowledge/adr/2026-09-04-0058-catalogue-impl-signals-absolute-workspace-root.md#D1] [tasks: T1]
- [OS-03] Supporting multiple workspace roots in one invocation is out of scope. [adr: knowledge/adr/2026-09-04-0058-catalogue-impl-signals-absolute-workspace-root.md#D1] [tasks: T1]

## Constraints
- [CN-01] The default and explicitly supplied workspace roots must each be normalized to absolute paths relative to the current directory at command startup. [adr: knowledge/adr/2026-09-04-0058-catalogue-impl-signals-absolute-workspace-root.md#D1] [tasks: T1, T2]
- [CN-02] Workspace-root normalization failure is fail-closed. [adr: knowledge/adr/2026-09-04-0058-catalogue-impl-signals-absolute-workspace-root.md#D1] [tasks: T1]

## Acceptance Criteria
- [ ] [AC-01] With the default workspace root, catalogue-impl-signals excludes Cargo's target directory from the rustdoc input corpus. [adr: knowledge/adr/2026-09-04-0058-catalogue-impl-signals-absolute-workspace-root.md#D1] [tasks: T2]
- [ ] [AC-02] With an explicit relative workspace root, catalogue-impl-signals excludes Cargo's target directory from the rustdoc input corpus. [adr: knowledge/adr/2026-09-04-0058-catalogue-impl-signals-absolute-workspace-root.md#D1] [tasks: T2]
- [ ] [AC-03] If the selected workspace root cannot be normalized because it is missing or inaccessible, catalogue-impl-signals stops without processing an input corpus. [adr: knowledge/adr/2026-09-04-0058-catalogue-impl-signals-absolute-workspace-root.md#D1] [tasks: T1]
- [ ] [AC-04] Path comparisons that determine target-directory exclusion use the normalized absolute workspace root, so equivalent relative and absolute root input cannot cause target files to be included. [adr: knowledge/adr/2026-09-04-0058-catalogue-impl-signals-absolute-workspace-root.md#D1] [tasks: T1, T2]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 14  🟡 0  🔴 0

