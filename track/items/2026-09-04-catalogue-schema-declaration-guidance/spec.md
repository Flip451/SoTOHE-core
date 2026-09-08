<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 17, yellow: 0, red: 0 }
---

# Catalogue schema declaration guidance and field-only positional keys

## Goal

- [GO-01] Make supported catalogue declarations discoverable and make structural matching consistently recognize their field and inherent-implementation children. [adr: knowledge/adr/2026-09-04-0057-catalogue-schema-declaration-guidance.md#D1, knowledge/adr/2026-09-04-0057-catalogue-schema-declaration-guidance.md#D2]

## Scope

### In Scope
- [IN-01] Document the supported lifetime trait-implementation spelling, inherent-method placement guidance, and private tuple-field matching limitation in the catalogue schema reference. [adr: knowledge/adr/2026-09-04-0057-catalogue-schema-declaration-guidance.md#D1] [tasks: T001]
- [IN-02] Align implementation-generics documentation in the catalogue DTO surface with the supported lifetime declaration guidance. [adr: knowledge/adr/2026-09-04-0057-catalogue-schema-declaration-guidance.md#D1] [tasks: T003]
- [IN-03] Restrict structural-matching positional keys to tuple field children, leaving inherent-implementation-derived children unnumbered. [adr: knowledge/adr/2026-09-04-0057-catalogue-schema-declaration-guidance.md#D2] [tasks: T002]
- [IN-04] Add regression coverage for the matchable one-trailing-private-tuple-field form when its inherent method is declared through a top-level inherent-implementation entry. [adr: knowledge/adr/2026-09-04-0057-catalogue-schema-declaration-guidance.md#D2] [tasks: T002]

### Out of Scope
- [OS-01] Supporting structural matching for tuple structs with two or more private fields or with a private field before a public field. [adr: knowledge/adr/2026-09-04-0057-catalogue-schema-declaration-guidance.md#D1] [tasks: T001, T002]
- [OS-02] Changing the codec's single-trailing-private-field None encoding. [adr: knowledge/adr/2026-09-04-0057-catalogue-schema-declaration-guidance.md#D1] [tasks: T002]

## Constraints
- [CN-01] The declaration guidance and DTO documentation alignment do not change runtime behaviour. [adr: knowledge/adr/2026-09-04-0057-catalogue-schema-declaration-guidance.md#D1] [tasks: T001, T003]
- [CN-02] Top-level inherent-implementation declarations remain supported for cases that cannot be expressed through an entry's methods; the guidance must not turn their use into an unsupported declaration form. [adr: knowledge/adr/2026-09-04-0057-catalogue-schema-declaration-guidance.md#D1] [tasks: T001]
- [CN-03] The positional-key decision is determined by the child item's field identity rather than by comparing its position with the number of fields. [adr: knowledge/adr/2026-09-04-0057-catalogue-schema-declaration-guidance.md#D2] [tasks: T002]
- [CN-04] The existing codec representation of one trailing private tuple field remains unchanged. [adr: knowledge/adr/2026-09-04-0057-catalogue-schema-declaration-guidance.md#D1] [tasks: T002]

## Acceptance Criteria
- [ ] [AC-01] The catalogue schema reference documents the supported declaration spelling for trait implementations with lifetime arguments, including the distinct lifetime notation used in the trait reference and implementation-generics declaration. [adr: knowledge/adr/2026-09-04-0057-catalogue-schema-declaration-guidance.md#D1] [tasks: T001]
- [ ] [AC-02] The catalogue schema reference identifies entry-level methods as the normal inherent-method declaration location, reserves top-level inherent-implementation declarations for entry-level limitations, and documents the currently matchable single trailing private tuple-field form. [adr: knowledge/adr/2026-09-04-0057-catalogue-schema-declaration-guidance.md#D1] [tasks: T001]
- [ ] [AC-03] The catalogue DTO documentation for implementation generics no longer states that lifetimes are outside the supported surface and instead aligns with the schema reference's lifetime declaration rule. [adr: knowledge/adr/2026-09-04-0057-catalogue-schema-declaration-guidance.md#D1] [tasks: T003]
- [ ] [AC-04] Structural matching assigns a positional key only to a tuple-struct or tuple-variant field item; a child originating from an inherent implementation receives no positional suffix. [adr: knowledge/adr/2026-09-04-0057-catalogue-schema-declaration-guidance.md#D2] [tasks: T002]
- [ ] [AC-05] A catalogue fixture with exactly one trailing private tuple field and an inherent method declared through a top-level inherent-implementation entry evaluates blue. [adr: knowledge/adr/2026-09-04-0057-catalogue-schema-declaration-guidance.md#D2] [tasks: T002]
- [ ] [AC-06] Previously blue structural-matching verdicts remain blue; the only changed verdict class is the combination of a tuple struct, a private field, and an inherent implementation. [adr: knowledge/adr/2026-09-04-0057-catalogue-schema-declaration-guidance.md#D2] [tasks: T002]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 17  🟡 0  🔴 0

