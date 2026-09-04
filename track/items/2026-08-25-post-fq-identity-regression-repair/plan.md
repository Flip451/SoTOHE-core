<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# 完全修飾識別の 2 リグレッション(実装前 add 型・bin root 別名)を修復する

## Summary

GO-01 → T001, T002, T003, T004, T005, T006, T007, T008, T009, T011, T012, T010, T014, T013, T015, T016. Implement the domain, infrastructure, and usecase changes in dependency order and cover the listed anchors.

## Tasks (16/16 resolved)

### S1 — Action-aware catalogue identity

> Modify the domain catalogue identity modules and tests for placement-aware identity resolution. Anchors: IN-01, IN-03, CO-03, AC-03.

- [x] **T001**: Modify libs/domain/src/tddd/catalogue_v2/identity_resolution.rs, identifiers.rs, entries.rs, libs/domain/src/tddd/catalogue_linter.rs, and their tests for placement-aware catalogue identity, namespace-aware extracted type-reference classification, and omitted-module_path resolution. Anchors: IN-01, IN-03, CO-03, AC-03. (`6381ecbb46f89d10cdd04c39a27996a6da798a93`)

### S2 — Shared resolution universe and canonicalization

> Add the infrastructure resolution-set and canonical-identity entry points with production-construction tests. Anchors: IN-01, IN-02, IN-03, OS-01, CO-01, CO-02, CO-03, AC-01, AC-02, AC-03.

- [x] **T002**: Modify libs/infrastructure/src/tddd/catalogue_to_extended_crate_codec.rs, canonical_type_identity.rs, and their tests to provide the shared resolution-set and canonical-identity entry points, with tests through the production construction. Anchors: IN-01, IN-02, IN-03, OS-01, CO-01, CO-02, CO-03, AC-01, AC-02, AC-03. (`6381ecbb46f89d10cdd04c39a27996a6da798a93`)

### S3 — Closed-set resolution-route migration

> Migrate the codec/deletion, type-signal identity-index, and Phase 1 authority routes through separate production and regression-test ownership boundaries. Anchors: IN-01, IN-02, IN-03, CO-01, CO-02, CO-03, AC-01, AC-02, AC-03.

- [x] **T003**: Update libs/infrastructure/src/tddd/catalogue_to_extended_crate_codec/encoder.rs and encoder_deletions.rs for the codec and deletion routes, with regression coverage only in libs/infrastructure/src/tddd/catalogue_to_extended_crate_codec_tests.rs; use T002's shared resolution set and remove route-local add/deletion fallbacks. Anchors: IN-01, IN-02, IN-03, CO-01, CO-02, CO-03, AC-01, AC-02, AC-03. (`d97eba047d5a0fe7a6d3793ad3804f5eabd04203`)
- [x] **T006**: Update libs/infrastructure/src/tddd/type_signals_evaluator.rs and libs/infrastructure/src/tddd/type_signals_evaluator/signal_builder.rs for the type-signal identity index, with route regression coverage only in the inline tests in type_signals_evaluator.rs; use T002's shared resolution set and remove add tolerance and raw-key fallback behavior. Anchors: IN-01, IN-02, CO-01, CO-02, AC-01, AC-02. (`d97eba047d5a0fe7a6d3793ad3804f5eabd04203`)
- [x] **T007**: Update libs/infrastructure/src/tddd/signal_evaluator_v2/phase1/builder/main_fn.rs and phase1/mod.rs for Phase 1 definition-path authority, adding its regression coverage only in a dedicated libs/infrastructure/src/tddd/signal_evaluator_v2/phase1/definition_path_authority_tests.rs; use T002's shared resolution set and remove the Phase 1-specific fallback. Anchors: IN-01, IN-02, IN-03, CO-01, CO-02, CO-03, AC-01, AC-02, AC-03. (`d97eba047d5a0fe7a6d3793ad3804f5eabd04203`)

### S4 — Bin-root alias consumers

> Migrate function identity and catalog import/bin-root resolution through separate production and regression-test ownership boundaries. Anchors: IN-02, OS-01, CO-02, AC-02.

- [x] **T004**: Update function-identity canonicalization in libs/infrastructure/src/tddd/signal_evaluator_v2/mod.rs, with function-path and bin-root regression coverage only in libs/infrastructure/src/tddd/signal_evaluator_v2/tests.rs; remove the function-specific root-alias handling. Anchors: IN-02, OS-01, CO-02, AC-02. (`6381ecbb46f89d10cdd04c39a27996a6da798a93`)
- [x] **T008**: Update catalog-import and bin-root handling in libs/infrastructure/src/tddd/catalog_gen/import_shape.rs and libs/infrastructure/src/schema_export/bin_target.rs, with catalog-import tests only in libs/infrastructure/src/tddd/catalog_gen/adapter_tests.rs and bin-target tests only in libs/infrastructure/src/schema_export_tests.rs; remove their local alias handling. Anchors: IN-02, OS-01, CO-02, AC-02. (`d97eba047d5a0fe7a6d3793ad3804f5eabd04203`)

### S5 — Fully-qualified obligation carriers

> Update test-obligation derive and tests to use catalogue identity for trait-impl carriers. Anchors: IN-04, CO-04, AC-04.

- [x] **T005**: Update libs/usecase/src/test_obligation/derive/identity.rs and its tests to resolve trait_impls[].for_type through catalogue identity and bind the result to obligation carriers; remove the raw short-name fallback. Anchors: IN-04, CO-04, AC-04. (`6381ecbb46f89d10cdd04c39a27996a6da798a93`)

### S6 — Three-way identity repair units

> Modify the domain identity-row surface, infrastructure persistence and report comparison, type-signal accumulation, and unplaced-import handling in their assigned files; modify libs/usecase/src/catalogue_document_loader.rs, libs/usecase/src/catalogue_impl_signals/interactor.rs, libs/usecase/src/test_obligation/{check,evaluate,results}.rs, libs/usecase/src/pre_review_gate.rs, libs/usecase/src/pre_review_gate/helpers.rs, libs/infrastructure/src/tddd/tddd_catalogue_document_loader.rs, libs/domain/src/tddd/catalogue_v2/catalogue_impl_signals_ports.rs, and apps/cli-composition/src/task_contract.rs with their regression tests for the T014 propagation boundary. Anchors: IN-03, IN-05, CO-03, CO-05, AC-03, AC-05. Planning policy: .harness/policies/no-upstream-restatement.md.
> Modify libs/infrastructure/src/lib.rs and apps/cli-composition/src/task_contract.rs with nested-items-directory, non-repository, and repository-selection-isolation regression tests for the T016 propagation boundary. Anchors: IN-05, CO-05, AC-05. Planning policy: .harness/policies/no-upstream-restatement.md.

- [x] **T009**: Modify libs/domain/src/tddd/signal_evaluator/region.rs and libs/infrastructure/src/tddd/signal_evaluator_v2/phase2.rs; migrate constructor call sites in their regression tests and the affected codec and type-signal regression tests. Anchors: IN-05, CO-05, AC-05. Planning policy: .harness/policies/no-upstream-restatement.md. (`6732c240e57d94293ca0c40e402a89b967b03d00`)
- [x] **T011**: Modify libs/infrastructure/src/tddd/catalog_gen/verb_import.rs with its regression tests. Anchors: IN-03, CO-03, AC-03. Planning policy: .harness/policies/no-upstream-restatement.md. (`6732c240e57d94293ca0c40e402a89b967b03d00`)
- [x] **T012**: Modify libs/domain/src/tddd/catalogue.rs and libs/domain/src/tddd/signal_evaluator/region.rs with their regression tests, plus the compile-driven error boundary in libs/infrastructure/src/tddd/type_signals_codec.rs. Anchors: IN-05, CO-05, AC-05. Planning policy: .harness/policies/no-upstream-restatement.md. (`21864697bb6ae2d39dc684f1fce90a7df2c3a46f`)
- [x] **T010**: Modify libs/infrastructure/src/tddd/type_signals_evaluator/signal_builder.rs and libs/infrastructure/src/tddd/type_signals_evaluator/signal_tags.rs with their regression tests. Anchors: IN-05, CO-05, AC-05. Planning policy: .harness/policies/no-upstream-restatement.md. (`21864697bb6ae2d39dc684f1fce90a7df2c3a46f`)
- [x] **T014**: Modify libs/domain/src/tddd/catalogue_v2/identity_resolution.rs (resolve_contract_entry_namespace) and libs/domain/src/tddd/catalogue_v2/catalogue_impl_signals_ports.rs (AttestedCatalogueDocument::attest and the domain loader port); relocate the loader port in libs/usecase/src/catalogue_document_loader.rs; propagate its constructor type through libs/usecase/src/catalogue_impl_signals/interactor.rs and libs/usecase/src/test_obligation/{check,evaluate,results}.rs; modify libs/usecase/src/pre_review_gate.rs and libs/usecase/src/pre_review_gate/helpers.rs (PreReviewGateError); modify libs/infrastructure/src/tddd/tddd_catalogue_document_loader.rs; wire the loader in apps/cli-composition/src/task_contract.rs; add regression tests for each affected module. Anchors: IN-03, IN-05, CO-03, CO-05, AC-03, AC-05. Planning policy: .harness/policies/no-upstream-restatement.md. (`21864697bb6ae2d39dc684f1fce90a7df2c3a46f`)
- [x] **T013**: Modify libs/infrastructure/src/tddd/type_signals_codec.rs with its round-trip regression tests. Anchors: IN-05, CO-05, AC-05. Planning policy: .harness/policies/no-upstream-restatement.md. (`ba33fcc55349dcadbff5347e5d398f65f727052d`)
- [x] **T015**: Modify libs/infrastructure/src/signal_report/mod.rs with its coverage-comparison regression tests. Anchors: IN-05, CO-05, AC-05. Planning policy: .harness/policies/no-upstream-restatement.md. (`ba33fcc55349dcadbff5347e5d398f65f727052d`)
- [x] **T016**: Modify libs/infrastructure/src/lib.rs (infrastructure::repository_root_for_items_dir) and apps/cli-composition/src/task_contract.rs with nested-items-directory, non-repository, and repository-selection-isolation regression tests. Anchors: IN-05, CO-05, AC-05. Planning policy: .harness/policies/no-upstream-restatement.md. (`d4489fcc29bab7a58413793feb177ffdde8f1def`)
