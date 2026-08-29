<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# 参照先 crate の add 宣言を解決集合に加える

## Summary

GO-01 → T001/T002/T003/T004/T005.

## Tasks (4/5 resolved)

### SECTION-001 — Resolution boundary

> Modify domain::tddd::catalogue_to_extended_crate_port::CatalogueToExtendedCratePort and run focused contract-conformance validation (GO-01, IN-01, IN-02, CO-01, CO-02, CO-03, AC-01, AC-02, AC-03).

- [x] **T001**: Modify domain::tddd::catalogue_to_extended_crate_port::CatalogueToExtendedCratePort; add focused contract-conformance validation for the port boundary (GO-01, IN-01, IN-02, CO-01, CO-02, CO-03, AC-01, AC-02, AC-03). (`76e029f0ffe4440a3e48fb5e0af9105655c7a86f`)

### SECTION-002 — Cross-crate resolution

> Modify infrastructure::tddd::catalogue_to_extended_crate_codec::CatalogueToExtendedCrateCodec and add focused validation in dependency order (IN-01, IN-02, OS-01, OS-02, OS-03, CO-01, CO-02, CO-03, AC-01, AC-02, AC-03).

- [x] **T002**: Modify infrastructure::tddd::catalogue_to_extended_crate_codec::CatalogueToExtendedCrateCodec and add focused encode and resolution_paths_for_catalogue validation (IN-01, IN-02, OS-01, CO-01, CO-02, CO-03, AC-01, AC-02). (`76e029f0ffe4440a3e48fb5e0af9105655c7a86f`)
- [ ] **T003**: After T002, modify infrastructure::tddd::catalogue_to_extended_crate_codec::CatalogueToExtendedCrateCodec and add focused precedence and negative resolution validation (IN-02, OS-02, OS-03, CO-01, CO-03, AC-03).

### SECTION-003 — Catalogue handoff

> Modify the catalogue_impl_signals and type-signals executor/evaluator handoff paths as separate validation units (IN-01, CO-01, CO-02, AC-01, AC-03).

- [x] **T004**: Modify usecase::catalogue_impl_signals to pass track-catalogue input to CatalogueToExtendedCratePort::encode; add focused catalogue_impl_signals handoff validation (IN-01, CO-01, AC-01). (`76e029f0ffe4440a3e48fb5e0af9105655c7a86f`)
- [x] **T005**: Modify usecase::type_signals::ports, infrastructure::tddd::type_signals_executor_adapter, and infrastructure::tddd::type_signals_evaluator::evaluate_and_write to pass track-catalogue input to CatalogueToExtendedCratePort::encode; add focused executor/evaluator handoff validation (IN-01, CO-01, CO-02, AC-01, AC-03). (`54bdad993b88b3ba44dd737d73dccd3af0386422`)
