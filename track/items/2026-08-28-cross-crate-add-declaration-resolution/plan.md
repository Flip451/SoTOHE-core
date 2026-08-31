<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# 参照先 crate の add 宣言を解決集合に加える

## Summary

GO-01 original implementation and authoritative-context completion remain T001-T009; D3-D8 reuse, snapshot, export-limit, fingerprint, lock, platform, writer-participation, and ABA work is decomposed into T010-T019.

## Tasks (17/19 resolved)

### SECTION-001 — Original resolution boundary

> Completed the original domain::tddd::catalogue_to_extended_crate_port::CatalogueToExtendedCratePort boundary and focused contract-conformance validation (GO-01, IN-01, IN-02, CO-01, CO-02, CO-03, AC-01, AC-02, AC-03).

- [x] **T001**: Modify domain::tddd::catalogue_to_extended_crate_port::CatalogueToExtendedCratePort; add focused contract-conformance validation for the port boundary (GO-01, IN-01, IN-02, CO-01, CO-02, CO-03, AC-01, AC-02, AC-03). (`76e029f0ffe4440a3e48fb5e0af9105655c7a86f`)

### SECTION-002 — Original cross-crate resolution

> Completed the original infrastructure::tddd::catalogue_to_extended_crate_codec::CatalogueToExtendedCrateCodec implementation and focused validation in dependency order (IN-01, IN-02, OS-01, OS-02, OS-03, CO-01, CO-02, CO-03, AC-01, AC-02, AC-03).

- [x] **T002**: Modify infrastructure::tddd::catalogue_to_extended_crate_codec::CatalogueToExtendedCrateCodec and add focused encode and resolution_paths_for_catalogue validation (IN-01, IN-02, OS-01, CO-01, CO-02, CO-03, AC-01, AC-02). (`76e029f0ffe4440a3e48fb5e0af9105655c7a86f`)
- [x] **T003**: After T002, modify infrastructure::tddd::catalogue_to_extended_crate_codec::CatalogueToExtendedCrateCodec and add focused precedence and negative resolution validation (IN-02, OS-02, OS-03, CO-01, CO-03, AC-03). (`c86805d91aa9f560f05139bdef310ffb98fee18d`)

### SECTION-003 — Original catalogue handoff

> Completed the original catalogue_impl_signals and type-signals executor/evaluator handoff paths as separate validation units (IN-01, CO-01, CO-02, AC-01, AC-03).

- [x] **T004**: Modify usecase::catalogue_impl_signals to pass track-catalogue input to CatalogueToExtendedCratePort::encode; add focused catalogue_impl_signals handoff validation (IN-01, CO-01, AC-01). (`76e029f0ffe4440a3e48fb5e0af9105655c7a86f`)
- [x] **T005**: Modify usecase::type_signals::ports, infrastructure::tddd::type_signals_executor_adapter, and infrastructure::tddd::type_signals_evaluator::evaluate_and_write to pass track-catalogue input to CatalogueToExtendedCratePort::encode; add focused executor/evaluator handoff validation (IN-01, CO-01, CO-02, AC-01, AC-03). (`54bdad993b88b3ba44dd737d73dccd3af0386422`)

### SECTION-004 — Authoritative rustdoc boundary

> Add domain::tddd::catalogue_to_extended_crate_port::AuthoritativeRustdocContext and revise the catalogue encoding boundary (GO-01, IN-01, IN-02, CO-01, CO-02, CO-03, AC-01, AC-02, AC-03).

- [x] **T006**: Add domain::tddd::catalogue_to_extended_crate_port::AuthoritativeRustdocContext and modify CatalogueToExtendedCratePort::encode; add focused boundary contract validation (GO-01, IN-01, IN-02, CO-01, CO-02, CO-03, AC-01, AC-02, AC-03). (`1a0afaede9d35c1ed05a041d4948dfa850c05bb1`)

### SECTION-005 — Declaring-layer placement

> Modify infrastructure::tddd::catalogue_to_extended_crate_codec::CatalogueToExtendedCrateCodec and add focused placement/regression validation (IN-01, IN-02, OS-01, OS-02, OS-03, CO-01, CO-03, AC-01, AC-02, AC-03).

- [x] **T007**: After T006, modify infrastructure::tddd::catalogue_to_extended_crate_codec::CatalogueToExtendedCrateCodec; add focused resolution/precedence/negative validation (IN-01, IN-02, OS-01, OS-02, OS-03, CO-01, CO-03, AC-01, AC-02, AC-03). (`1a0afaede9d35c1ed05a041d4948dfa850c05bb1`)

### SECTION-006 — Complete rustdoc context assembly

> Preserve the completed catalogue_impl_signals context assembly and finish the outstanding executor/evaluator compile closure without absorbing the later D3-D8 work (IN-01, IN-02, CO-01, CO-02, CO-03, AC-01, AC-02, AC-03).

- [x] **T008**: After T006, modify usecase::catalogue_impl_signals::interactor::CatalogueImplSignalsInteractor to load every configured TDDD layer catalogue and authoritative baseline/current rustdoc pair, assemble the complete LayerId-keyed context map before encode, and add focused handoff validation (IN-01, IN-02, CO-01, CO-02, CO-03, AC-01, AC-02, AC-03). (`1a0afaede9d35c1ed05a041d4948dfa850c05bb1`)
- [x] **T009**: After T006, finish the outstanding compile closure for the type-signals executor/evaluator authoritative-context handoff and its focused validation; do not absorb the D3-D8 fingerprint, export-limit, lock, or snapshot work (IN-01, IN-02, CO-01, CO-02, CO-03, AC-01, AC-02, AC-03). (`a85a73d45298014af3eda04ab9cbe40159ec8be4`)

### SECTION-007 — Complete reuse identity

> Implement the domain-owned complete reuse identity and fail-closed reuse decision vocabulary from the approved type catalogue (IN-03, IN-06, OS-04, CO-04, AC-04, AC-08).

- [x] **T010**: Modify libs/domain/src/tddd/type_signals_doc.rs::ImplementationFingerprint, ResolutionFingerprint, RustdocExecutionIdentity, TypeSignalsCacheKey, TypeSignalsReuseDecision, and TypeSignalsReuseInput; add focused value, validation, equality, and reuse-decision tests (IN-03, IN-06, OS-04, CO-04, AC-04, AC-08). (`d08f4eff64c6c4dec9bc5eeb4a0380b7efc4c6eb`)

### SECTION-008 — Immutable rustdoc snapshot contract

> Implement immutable rustdoc capture and identity-bearing snapshot construction at the domain boundary (IN-03, IN-05, IN-07, CO-06, CO-07, AC-04, AC-06, AC-07).

- [x] **T011**: After T010, modify libs/domain/src/tddd/type_signals_doc.rs::CapturedRustdocJson, RustdocSnapshot, construct_captured_rustdoc_json, and construct_rustdoc_snapshot, plus libs/domain/src/tddd/catalogue_v2/catalogue_impl_signals_ports.rs::RustdocCratePort; add focused same-bytes and mixed-generation rejection tests (IN-03, IN-05, IN-07, CO-06, CO-07, AC-04, AC-06, AC-07). (`3c360756c3b0745799a5ba2c7e1308116083d1fa`)

### SECTION-009 — Bounded rustdoc export planning

> Modify the rustdoc export plan and catalogue implementation-signals orchestration; add boundary tests for sixty-four and sixty-five layers (IN-04, OS-05, CO-05, AC-05).

- [x] **T012**: Modify libs/usecase/src/catalogue_impl_signals/service.rs::RustdocExportPlan and CatalogueImplSignalsError, and libs/usecase/src/catalogue_impl_signals/interactor.rs::CatalogueImplSignalsInteractor; add boundary tests for sixty-four and sixty-five layers (IN-04, OS-05, CO-05, AC-05). (`a85a73d45298014af3eda04ab9cbe40159ec8be4`)

### SECTION-010 — Persisted identity decoding

> Modify infrastructure typed decoding to reject invalid persisted rustdoc execution identities as TypeSignalsCodecError::InvalidExecutionIdentity and add focused invalid-identity decode validation (IN-03, CO-04, AC-04).

- [x] **T013**: After T009 and T010, modify infrastructure::tddd::type_signals_codec::TypeSignalsCodecError and its typed persisted-document decoder to reject invalid rustdoc execution identities as TypeSignalsCodecError::InvalidExecutionIdentity; add focused invalid-identity decode tests (IN-03, CO-04, AC-04). (`d08f4eff64c6c4dec9bc5eeb4a0380b7efc4c6eb`)

### SECTION-011 — Locked rustdoc export and snapshot integration

> Modify libs/infrastructure/src/tddd/rustdoc_crate_adapter.rs::RustdocCrateAdapter, libs/infrastructure/src/schema_export.rs::RustdocSchemaExporter, and libs/infrastructure/src/tddd/type_signals_evaluator.rs::execute_type_signals_for_layer and EvaluateSignalsError; add focused validation (IN-03, IN-05, IN-06, IN-07, OS-04, OS-05, CO-06, CO-07, AC-04, AC-06, AC-07, AC-08).

- [x] **T014**: After T011-T013, modify infrastructure::tddd::rustdoc_crate_adapter::RustdocCrateAdapter lock acquisition and failure propagation; add focused serialization, 120-second timeout, lock-operation failure, no-retry, and no-fallback tests (IN-05, OS-05, CO-06, AC-07). (`3c360756c3b0745799a5ba2c7e1308116083d1fa`)
- [x] **T015**: After T014, modify infrastructure::tddd::rustdoc_crate_adapter::RustdocCrateAdapter and infrastructure::tddd::type_signals_evaluator::execute_type_signals_for_layer to require exclusive target ownership and common-lock participation by every expected-output writer; add focused cooperative-writer and non-cooperative-writer rejection tests (CO-06, CO-07, AC-06). (`e7f8355bd40d8d6b74cddb2d42deac164422ee8e`)
- [x] **T016**: After T015, modify infrastructure::schema_export::RustdocSchemaExporter to expose immutable Vec<u8> export data, and modify infrastructure::tddd::rustdoc_crate_adapter::RustdocCrateAdapter to determine, verify, read, hash, decode, and construct the current rustdoc snapshot from that one immutable byte capture while the common lock remains held; add focused same-bytes and output-replacement tests (IN-07, CO-07, AC-06).
- [ ] **T017**: After T016, continue modifying the same infrastructure::tddd::rustdoc_crate_adapter::RustdocCrateAdapter write target for trusted-root admission and failure propagation; keep this work in a later batch to avoid overlapping T016 ownership, and add focused non-Unix, relative escape, unverifiable root, and symlinked target-component rejection tests (IN-06, OS-05, CO-06, AC-08).
- [x] **T018**: After T016, modify infrastructure::tddd::type_signals_evaluator::execute_type_signals_for_layer and its EvaluateSignalsError integration to capture authoritative implementation/resolution fingerprints and integrate complete reuse keys and fail-closed reuse decisions; add focused mutation, exclusion, limit, and stale-reuse tests (IN-03, OS-04, CO-04, AC-04).
- [ ] **T019**: After T018, continue modifying infrastructure::tddd::type_signals_evaluator::execute_type_signals_for_layer and its EvaluateSignalsError integration to revalidate start/end generations before persisting; add focused changed-generation, discard-before-persist, path-reread, and ABA tests (IN-07, CO-07, AC-06).
