<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# TDDD Catalogue Generic Type Aliases

## Summary

GO-01: T001, T002, T003, T004.

## Tasks (0/4 resolved)

### S1 — Catalogue declaration and validation

> Update alias declaration schema and lint boundaries. [IN-01; IN-02; CN-01; CN-03; AC-01; AC-02]

- [ ] **T001**: Update `TypeKindV2::TypeAlias`, `TypeKindDto::TypeAlias`, and `type_kind_{from,to}_dto` to carry the alias generic-parameter declaration; cover codec boundary cases in `catalogue_document_codec` tests. [GO-01; IN-01; OUT-01; CN-01; CN-03; AC-01]
- [ ] **T002**: Extend `catalogue_linter::evaluate_catalogue_lint` and its alias-entry test fixtures to validate the alias generic-parameter declaration. [GO-01; IN-02; CN-01; CN-03; AC-01; AC-02]

### S2 — Lexical comparison and compatibility

> Update alias comparison and its regression coverage. [IN-03; OUT-01; OUT-02; CN-01; CN-02; CN-03; AC-03; AC-04; AC-05]

- [ ] **T003**: Update `catalogue_to_extended_crate_codec::encode_type_alias` and `signal_evaluator_v2::structural_eq::items_structurally_equal` for alias generic-parameter comparison; add evaluator match/mismatch tests. [GO-01; IN-03; OUT-01; OUT-02; CN-02; CN-03; AC-03; AC-04]
- [ ] **T004**: Add the existing implementation-side generic-alias regression fixture to `signal_evaluator_v2::tests`. [IN-01; CN-01; CN-03; AC-05]
