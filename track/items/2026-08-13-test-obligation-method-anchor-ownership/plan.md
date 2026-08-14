<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# trait_method 義務の anchor 所有権を method 単位にする

## Summary

GO-01 → T1, T2, T3, T4, T5.
GO-02 → T1, T2, T3.

## Tasks (0/5 resolved)

### method-declaration-staging — MethodDeclaration staging and cutover

> Targets: libs/domain/src/tddd/catalogue_v2/methods.rs, libs/infrastructure/src/tddd/catalogue_document_codec/, libs/infrastructure/src/tddd/catalogue_to_extended_crate_codec/, libs/infrastructure/src/tddd/contract_map_renderer_adapter/, and MethodDeclaration construction call sites. Operation: stage, migrate cross-crate accessor uses, integrate, and cut over. Anchors: GO-01, GO-02, IN-01, IN-05, AC-01, AC-06, CN-05, OS-05.

- [ ] **T1**: Targets: libs/domain/src/tddd/catalogue_v2/methods.rs. Operation: implement and test lint remediation, accessors, the backward-compatible method-level spec_refs storage/accessor API, and compatibility staging. Anchors: GO-01, GO-02, IN-01, IN-05, AC-01, AC-06, CN-05, OS-05.
- [ ] **T2**: Targets: libs/infrastructure/src/tddd/catalogue_document_codec/, libs/infrastructure/src/tddd/catalogue_to_extended_crate_codec/, libs/infrastructure/src/tddd/contract_map_renderer_adapter/, and libs/infrastructure/src/tddd/catalogue_to_extended_crate_codec_tests.rs. Operation: implement and test MethodDeclaration codec support and compatibility integration, and migrate every cross-crate MethodDeclaration field read to the staged accessors. Anchors: GO-01, GO-02, IN-01, AC-01, AC-06, CN-05.
- [ ] **T3**: Targets: libs/domain/src/tddd/catalogue_v2/methods.rs and MethodDeclaration construction call sites. Operation: migrate and test the constructor cutover. Anchors: GO-01, GO-02, IN-01, AC-01, OS-05.

### obligation-ownership — Obligation ownership

> Targets: libs/domain/src/tddd/test_obligation/obligations.rs, libs/usecase/src/test_obligation/derive.rs, libs/usecase/src/test_obligation/evaluate/, and libs/infrastructure/src/test_obligation/fulfillment_verifier.rs. Operation: implement and test. Anchors: GO-01, IN-01, IN-02, IN-03, AC-01, AC-02, AC-03, AC-04, CN-01, CN-02, CN-03, OS-01, OS-02, OS-03, OS-04.

- [ ] **T4**: Targets: libs/domain/src/tddd/test_obligation/obligations.rs and libs/usecase/src/test_obligation/derive.rs. Operation: implement and test derivation and structural validation. Anchors: GO-01, IN-01, IN-02, AC-01, AC-02, AC-03, CN-01, CN-02, OS-01, OS-02, OS-03.
- [ ] **T5**: Targets: libs/usecase/src/test_obligation/evaluate/ and libs/infrastructure/src/test_obligation/fulfillment_verifier.rs. Operation: implement and test fulfillment verification and instruction rendering. Anchors: GO-01, IN-03, AC-04, CN-03, OS-02, OS-04.
