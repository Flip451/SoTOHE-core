<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# TDDD chain ③ の rustdoc 抽出を track 単位の feature 宣言に基づかせる

## Summary

GO-01: T001, T002, T003.
GO-02: T003, T004.

## Tasks (4/5 resolved)

### S1 — Declaration foundation

> Add validated declaration values in `libs/domain/src/tddd/feature_declaration.rs` and the filesystem port adapter in `libs/infrastructure/src/tddd/feature_declaration_adapter.rs` (GO-01, IN-01, IN-04).

- [x] **T001**: Add `CargoFeatureName`, `CargoFeatureNameError`, `TdddFeatureDeclaration`, `TdddFeatureDeclarationError`, and `TdddFeatureLookupError` in `libs/domain/src/tddd/feature_declaration.rs`; export the module through `libs/domain/src/tddd/mod.rs`; and add their unit tests (GO-01, IN-01, IN-04, CN-03, AC-01, AC-09).
- [x] **T002**: Add `TdddFeatureDeclarationReadError`, `TdddBaselineFeatureDeclarationPort`, `TdddBaselineFeatureDeclarationPortError`, `TdddActualFeatureDeclarationPort`, and `TdddActualFeatureDeclarationPortError` in `libs/usecase/src/tddd_feature_declaration.rs`; export the module from `libs/usecase/src/lib.rs`; implement and export both client-specific port traits on `FsTdddFeatureDeclarationAdapter` in `libs/infrastructure/src/tddd/feature_declaration_adapter.rs`; and add adapter tests (IN-01, IN-03, IN-05, CN-01, CN-03, AC-03, AC-05, AC-06).

### S2 — Feature-aware capture paths

> Modify baseline and actual capture ports, interactors, composition wiring, and `libs/infrastructure/src/schema_export/bin_target.rs` to use the declaration (IN-02, IN-03, IN-05).

- [x] **T003**: Update `.harness/capabilities/type-designer.md` and `.harness/workflows/track/type-design.md` before their `baseline-capture` pipeline step to author `tddd-features.json`; modify `RustdocBaselineCapturePort` in `libs/domain/src/tddd/catalogue_v2/catalogue_impl_signals_ports.rs`, `BaselineCaptureInteractor` and `BaselineCaptureError` in `libs/usecase/src/baseline_capture/{interactor.rs,service.rs}`, and `RustdocBaselineCaptureAdapter` in `libs/infrastructure/src/tddd/rustdoc_baseline_capture_adapter.rs`; inject `TdddBaselineFeatureDeclarationPort` through `FsTdddFeatureDeclarationAdapter` at `apps/cli-composition/src/track/tddd.rs`; extend `RustdocSchemaExporter` in `libs/infrastructure/src/schema_export.rs` and `build_rustdoc_args` in `libs/infrastructure/src/schema_export/bin_target.rs`; and add workflow and baseline-capture tests (IN-02, IN-03, IN-04, CN-01, CN-02, CN-05, CN-06, AC-01, AC-02, AC-03, AC-04, AC-08).
- [x] **T004**: Modify `RustdocCratePort` in `libs/domain/src/tddd/catalogue_v2/catalogue_impl_signals_ports.rs` and `CatalogueImplSignalsInteractor` / `CatalogueImplSignalsError` in `libs/usecase/src/catalogue_impl_signals/{interactor.rs,service.rs}`; update `RustdocCrateAdapter` in `libs/infrastructure/src/tddd/rustdoc_crate_adapter.rs` and `RustdocSchemaExporter` in `libs/infrastructure/src/schema_export.rs`; inject `TdddActualFeatureDeclarationPort` through `FsTdddFeatureDeclarationAdapter` at `apps/cli-composition/src/track/tddd.rs`; and add actual-capture tests (IN-03, IN-05, CN-01, CN-02, CN-03, CN-05, AC-03, AC-04, AC-05, AC-06, AC-08).

### S3 — Declared semantic-dup surface

> Complete the feature-selected public infrastructure surface under `libs/infrastructure/src/semantic_dup/` and verify its implementation signals (CN-04, AC-07).

- [ ] **T005**: Complete the catalogue-declared `semantic-dup` public items in `libs/infrastructure/src/semantic_dup/{extractor.rs,fragment_extractor_adapter.rs,embedding.rs,index.rs,noop_adapter.rs,null_insert_proxy.rs}` and their corresponding usecase ports/errors; add feature-enabled rustdoc and signal regression coverage (CN-04, CN-05, AC-07, AC-08).
