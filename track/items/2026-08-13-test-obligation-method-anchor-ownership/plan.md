<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# trait_method 義務の anchor 所有権を method 単位にする

## Summary

GO-01 → T1, T2, T3, T4, T5, T6, T7, T8, T9, T12, T13, T11, T14.
GO-02 → T1, T2, T3, T10, T11, T14.

## Tasks (14/14 resolved)

### method-declaration-staging — MethodDeclaration staging and cutover

> Targets: libs/domain/src/tddd/catalogue_v2/methods.rs, libs/infrastructure/src/tddd/catalogue_document_codec/, libs/infrastructure/src/tddd/catalogue_to_extended_crate_codec/, libs/infrastructure/src/tddd/contract_map_renderer_adapter/, and MethodDeclaration construction call sites. Operation: stage, migrate cross-crate accessor uses, integrate, and cut over. Anchors: GO-01, GO-02, IN-01, IN-05, AC-01, AC-06, CN-05, OS-05.

- [x] **T1**: Targets: libs/domain/src/tddd/catalogue_v2/methods.rs. Operation: implement and test lint remediation, accessors, the backward-compatible method-level spec_refs storage/accessor API, and compatibility staging. Anchors: GO-01, GO-02, IN-01, IN-05, AC-01, AC-06, CN-05, OS-05. (`79af41935c3b7f5ff1812eb6490625792ca7da3c`)
- [x] **T2**: Targets: libs/infrastructure/src/tddd/catalogue_document_codec/, libs/infrastructure/src/tddd/catalogue_to_extended_crate_codec/, libs/infrastructure/src/tddd/contract_map_renderer_adapter/, and libs/infrastructure/src/tddd/catalogue_to_extended_crate_codec_tests.rs. Operation: implement and test MethodDeclaration codec support and compatibility integration, and migrate every cross-crate MethodDeclaration field read to the staged accessors. Anchors: GO-01, GO-02, IN-01, AC-01, AC-06, CN-05. (`79af41935c3b7f5ff1812eb6490625792ca7da3c`)
- [x] **T3**: Targets: libs/domain/src/tddd/catalogue_v2/methods.rs and MethodDeclaration construction call sites. Operation: migrate and test the constructor cutover. Anchors: GO-01, GO-02, IN-01, AC-01, OS-05. (`12f0d9b7`)

### obligation-ownership — Obligation ownership

> Targets: libs/domain/src/tddd/test_obligation/obligations.rs, libs/usecase/src/test_obligation/derive.rs, libs/usecase/src/test_obligation/evaluate/, and libs/infrastructure/src/test_obligation/fulfillment_verifier.rs. Operation: implement and test. Anchors: GO-01, IN-01, IN-02, IN-03, AC-01, AC-02, AC-04, CN-01, CN-02, CN-03, OS-01, OS-02, OS-03, OS-04.

- [x] **T4**: Targets: libs/domain/src/tddd/test_obligation/obligations.rs and libs/usecase/src/test_obligation/derive.rs. Operation: implement and test derivation and structural validation. Anchors: GO-01, IN-01, IN-02, AC-01, AC-02, CN-01, CN-02, OS-01, OS-02, OS-03. (`ab81694a58e888cdaad845767d4c01168f697df2`)
- [x] **T5**: Targets: libs/usecase/src/test_obligation/evaluate/ and libs/infrastructure/src/test_obligation/fulfillment_verifier.rs. Operation: implement and test fulfillment verification and instruction rendering. Anchors: GO-01, IN-03, AC-04, CN-03, OS-02, OS-04. (`d4b3bc80034f984676a968bca84c3a44fcaa40d9`)

### phase-command-d3-recovery — PhaseCommandService D3 recovery

> Targets: the method-local PhaseCommandService grounding and fulfillment evidence in libs/usecase/src/phase_command/, plus the named deviation clause in .harness/custom/review-prompts/harness-policy.md. Operation: independently restore and test validate, explain, and enter, then remove only the named track's conditional Known Accepted Deviations clause after all three recoveries. Anchors: GO-01, IN-09, IN-10, IN-11, IN-12, AC-10, AC-11, AC-12, AC-13.

- [x] **T6**: Targets: the validate-specific PhaseCommandService grounding and fulfillment evidence in libs/usecase/src/phase_command/. Operation: restore and test validate's method-scoped anchors and method-local fulfillment evidence. Anchors: GO-01, IN-09, AC-10. (`ce56ca5490c31ac24d6d986b424f1c2031fe7dd6`)
- [x] **T7**: Targets: the explain-specific PhaseCommandService grounding and fulfillment evidence in libs/usecase/src/phase_command/. Operation: restore and test explain's method-scoped anchors and method-local fulfillment evidence. Anchors: GO-01, IN-10, AC-11. (`203fcdff70083dfeb409f925b1a9cc2f65d06cbc`)
- [x] **T8**: Targets: the enter-specific PhaseCommandService grounding and fulfillment evidence in libs/usecase/src/phase_command/. Operation: restore and test enter's method-scoped anchors and method-local fulfillment evidence. Anchors: GO-01, IN-11, AC-12. (`1696565bbb5f5cd9ace924ff288cbe5d8701031a`)
- [x] **T9**: Targets: .harness/custom/review-prompts/harness-policy.md. Operation: after validate, explain, and enter are re-grounded, remove only the named track's conditional Known Accepted Deviations clause; preserve all other deviation records. Anchors: GO-01, IN-12, AC-13. (`1696565bbb5f5cd9ace924ff288cbe5d8701031a`)

### method-action-reentry — Method action enforcement and command scoping

> Targets: MethodDeclaration schema/codec and all construction call sites, then domain validation, action-aware anchor projection, and command-directed selection in libs/usecase/src/test_obligation/. Operation: make the schema/codec, validation, projection, and command-scoping units separately implementable and testable in that order. Anchors: GO-01, GO-02, IN-13, IN-14, AC-03, AC-14, AC-15, CN-01, CN-02, OS-01.

- [x] **T10**: Targets: libs/domain/src/tddd/catalogue_v2/methods.rs; libs/infrastructure/src/tddd/catalogue_document_codec/ DTO, decode, encode, and codec tests; and every libs/domain, libs/usecase, and libs/infrastructure MethodDeclaration::new construction call site. Operation: extend the MethodDeclaration schema, update the JSON codec, migrate constructor arity, and add focused tests. Anchors: GO-02, IN-13, AC-14. (`21e4faba31d390705e93f3d0bd94c224880c0af1`)
- [x] **T12**: Targets: libs/domain/src/tddd/test_obligation/obligations.rs and related domain tests. Operation: implement and test method spec_refs validation. Anchors: GO-01, IN-14, AC-03, CN-01. (`701256308832e5ddff442732ff7574cf814edf98`)
- [x] **T13**: Targets: libs/domain/src/tddd/test_obligation/projection.rs and related domain tests. Operation: implement and test action-aware method-obligation projection. Anchors: AC-14. (`701256308832e5ddff442732ff7574cf814edf98`)
- [x] **T11**: Targets: libs/usecase/src/test_obligation/derive.rs, its command-directed catalogue selection boundary, and related tests. Operation: implement and test the command-directed catalogue selection boundary. Anchors: GO-01, GO-02, IN-14, AC-03, AC-14, AC-15, CN-02, OS-01. (`701256308832e5ddff442732ff7574cf814edf98`)

### parent-forbids-method-spec-refs — Parent-action method spec_refs validation

> Targets: libs/domain/src/tddd/test_obligation/obligations.rs and libs/usecase/src/test_obligation/derive/mod.rs. Operation: implement, wire, and test. Anchors: IN-15, AC-16.

- [x] **T14**: Targets: libs/domain/src/tddd/test_obligation/obligations.rs, related domain tests, and libs/usecase/src/test_obligation/derive/mod.rs. Operation: implement and test validate_parent_forbids_method_spec_refs and wire it into the named-catalogue validation path. Anchors: IN-15, AC-16.
