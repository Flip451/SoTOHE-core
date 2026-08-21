<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# 型シグナル評価の型識別を完全修飾パスで行う

## Summary

GO-01 -> T001, T002, T004, T005, T006, T011, T012; GO-02 -> T002, T004, T007, T008, T009, T010, T011, T013, T014, T015, T016, T017.

## Tasks (1/17 resolved)

### identity-foundation — Fully-qualified identity foundation

> Review the existing `libs/infrastructure/src/tddd/signal_evaluator_v2/` bootstrap diff and add the Phase 2 catalogue-domain identity types under `libs/domain/src/tddd/`. [IN-01; IN-02; CO-01; CO-02; CO-04; AC-01; AC-02; AC-05]

- [x] **T001**: Review the existing uncommitted bootstrap diff in `libs/infrastructure/src/tddd/signal_evaluator_v2/{mod.rs,phase1/builder/main_fn.rs,phase1/child_items.rs,phase1/state.rs,phase2.rs,tests.rs}`, apply only review corrections, and run the module tests; do not recreate the bootstrap. [IN-02; CO-01; CO-04; AC-02; AC-05]
- [ ] **T002**: Add the Phase 2 `CatalogueEntryKey` and `FullyQualifiedItemPath` changes in `libs/domain/src/tddd/{semantic_verify.rs,catalogue_v2/identifiers.rs,catalogue_v2/identifiers_tests.rs}` with construction, ordering, and display tests; keep the change additive until T004. [IN-01; OS-04; CO-01; CO-02; CO-04; AC-01; AC-02]

### catalogue-resolution — Catalogue representation and resolution

> Migrate the catalogue document aggregate, schema-version value object, document codecs, and local resolver in their named domain and infrastructure modules. [IN-01; IN-04; IN-05; OS-04; CO-01; CO-02; CO-03; CO-04; AC-01; AC-02; AC-03; AC-04; AC-09]

- [ ] **T003**: Add `CatalogueSchemaVersion` in `libs/domain/src/tddd/catalogue_v2/document.rs`, migrate its document accessors there, and update `libs/infrastructure/src/tddd/catalogue_document_codec/{dto.rs,encode.rs,decode.rs,validate.rs,mod.rs}` with focused domain and codec tests. [IN-05; CO-04; AC-09]
- [ ] **T004**: Migrate catalogue keys and local references in `libs/domain/src/tddd/catalogue_v2/{document.rs,entries.rs,deletions.rs,traits.rs}` and `new_typegraph_codec_error.rs`; migrate the codec and resolver in `libs/infrastructure/src/tddd/{catalogue_to_extended_crate_codec.rs,catalogue_to_extended_crate_codec/**,catalogue_document_codec/**}`, remove `catalogue_to_extended_crate_codec_error.rs`, and add resolver tests. [IN-01; IN-04; CO-01; CO-02; CO-03; CO-04; AC-01; AC-02; AC-03; AC-04; AC-05]

### evaluation-consumers — Evaluator and contract-map consumers

> Migrate the remaining `signal_evaluator_v2`, type-signal owner, and contract-map identity consumers in the named infrastructure modules. [IN-02; CO-01; CO-04; AC-02; AC-05; AC-06]

- [ ] **T005**: Migrate impl, local TypeRef, and generic identity consumers in `libs/infrastructure/src/tddd/signal_evaluator_v2/{impl_identity.rs,resolve_type.rs,structural_eq.rs,generics_eq/**,tests.rs}` and add focused evaluator cases. [IN-02; CO-01; CO-04; AC-02; AC-05]
- [ ] **T006**: Migrate owner joining and contract-map lookup in `libs/infrastructure/src/tddd/{type_signals_evaluator.rs,type_signals_evaluator/**,contract_map_adapter.rs,contract_map_renderer_adapter/**}` and add focused owner, node, and edge tests. [IN-02; OS-03; CO-01; CO-04; AC-06]

### downstream-consumers — Downstream identity consumers

> Migrate the named catalogue-lint rules, obligation derivation, task-contract/pre-review attribution, and catalogue writer targets. [IN-03; OS-01; OS-03; CO-01; CO-02; CO-04; AC-07; AC-08; AC-09]

- [ ] **T007**: Update the `ReferencedRoleConstraint`, `FieldElementUniqueAcrossEntries`, and `NoExternalReferenceInMethods` arms in `libs/domain/src/tddd/catalogue_linter_eval.rs` and their cases in `libs/domain/src/tddd/catalogue_linter.rs`. [IN-03; OS-03; CO-01; CO-04; AC-07]
- [ ] **T008**: Migrate `TraitRoleEntry`, `index_trait_roles`, and `resolve_trait_role` in `libs/usecase/src/test_obligation/derive/mod.rs` and add focused cases in `libs/usecase/src/test_obligation/derive_tests.rs`. [IN-03; OS-03; CO-01; CO-04; AC-08]
- [ ] **T009**: Migrate entry attribution and checks in `apps/cli/src/commands/task_contract.rs` and `libs/usecase/src/pre_review_gate{.rs,/helpers.rs}`, and add command and gate tests in those modules. [IN-03; CO-01; CO-04; AC-08]
- [ ] **T010**: Migrate entry-key writing in `libs/infrastructure/src/tddd/catalog_gen/{verb_add.rs,verb_cite.rs,verb_import.rs,json_build.rs}` and `apps/cli/src/commands/catalog.rs`, and add focused writer tests. [IN-03; OS-01; CO-01; CO-02; CO-04; AC-08; AC-09]

### cross-surface-validation — Cross-surface regression validation

> Add focused identity fixtures and preservation assertions to the named domain, usecase, infrastructure, and CLI test modules. [IN-01; IN-02; IN-03; IN-04; OS-01; OS-02; OS-03; OS-04; CO-01; CO-03; CO-04; AC-01; AC-02; AC-03; AC-04; AC-05; AC-06; AC-07; AC-08; AC-09]

- [ ] **T011**: Add duplicate-path catalogue and resolver cases to `libs/infrastructure/src/tddd/catalogue_to_extended_crate_codec_tests.rs` and evaluator impl, inherent, generic, and TypeRef cases to `libs/infrastructure/src/tddd/signal_evaluator_v2/tests.rs`; run both focused test modules. [IN-01; IN-02; IN-04; OS-04; CO-01; CO-03; CO-04; AC-01; AC-02; AC-03; AC-04; AC-05]
- [ ] **T012**: Add owner collision cases to the test module in `libs/infrastructure/src/tddd/type_signals_evaluator.rs` and node/edge collision cases to `libs/infrastructure/src/tddd/contract_map_renderer_adapter/render/node_index.rs`; run both focused test modules. [IN-02; OS-03; CO-01; CO-04; AC-06]
- [ ] **T013**: Add duplicate-path cases for `ReferencedRoleConstraint`, `FieldElementUniqueAcrossEntries`, and `NoExternalReferenceInMethods` to the test module in `libs/domain/src/tddd/catalogue_linter.rs`; run the focused linter tests. [IN-03; OS-03; CO-01; CO-04; AC-07]
- [ ] **T014**: Add same-name trait-role and multi-catalogue cases for `derive_obligations_document` to `libs/usecase/src/test_obligation/derive_tests.rs`; run the focused derivation tests. [IN-03; OS-03; CO-01; CO-04; AC-08]
- [ ] **T015**: Add fully-qualified attribution cases to the test modules in `apps/cli/src/commands/task_contract.rs` and `libs/usecase/src/pre_review_gate.rs`; run the command and gate tests. [IN-03; CO-01; CO-04; AC-08]
- [ ] **T016**: Add add/cite/import round-trip cases to `libs/infrastructure/src/tddd/catalog_gen/adapter_tests.rs` and command cases to the test module in `apps/cli/src/commands/catalog.rs`; run both focused test modules. [IN-03; OS-01; CO-01; CO-02; CO-04; AC-08]
- [ ] **T017**: Add OS-01 codec assertions to `libs/infrastructure/src/tddd/catalogue_document_codec/mod.rs`, OS-02 assertions to `baseline_rustdoc_codec.rs` and `baseline_graph_renderer_adapter/mod.rs`, and OS-03 label assertions to `contract_map_renderer_adapter/render/emit.rs`; run the focused preservation tests and `cargo make ci-track`. [OS-01; OS-02; OS-03; CO-04; AC-09]
