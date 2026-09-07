<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# Catalogue schema declaration guidance and field-only positional keys

## Summary

Clarify the supported catalogue declaration surface in the schema reference and DTO documentation without changing runtime behaviour.
Correct field-only positional-key assignment and lock the fix with regression coverage.

## Tasks (0/3 resolved)

### S1 — Catalogue schema-reference guidance

> Update .harness/reference/catalogue-schema.md with lifetime trait-implementation spelling, inherent-method placement, and single-trailing-private-tuple-field guidance; IN-01, CN-01, CN-02, AC-01, AC-02.

- [ ] **T001**: Update .harness/reference/catalogue-schema.md with lifetime trait-implementation spelling, inherent-method placement, and single-trailing-private-tuple-field guidance; IN-01, CN-01, CN-02, AC-01, AC-02.

### S2 — Implementation-generics DTO documentation

> Align the impl_generics DTO documentation in libs/domain/src/tddd/catalogue_v2/entries.rs and traits.rs with the supported lifetime declaration spelling; IN-02, CN-01, AC-03.

- [ ] **T003**: Align the impl_generics DTO documentation in libs/domain/src/tddd/catalogue_v2/entries.rs and traits.rs with the supported lifetime declaration spelling; IN-02, CN-01, AC-03.

### S3 — Field-only positional matching

> Update positional-key handling and regression coverage in libs/infrastructure/src/tddd/signal_evaluator_v2/structural_eq.rs; IN-03, IN-04, CN-03, CN-04, AC-04, AC-05, AC-06.

- [ ] **T002**: Update positional-key handling and add focused regression coverage in libs/infrastructure/src/tddd/signal_evaluator_v2/structural_eq.rs; IN-03, IN-04, CN-03, CN-04, AC-04, AC-05, AC-06.
