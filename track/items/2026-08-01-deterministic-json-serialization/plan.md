<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# Deterministic JSON Serialization

## Summary

GO-01: T1-T29.

## Tasks (1/29 resolved)

### core-track-review-codecs — Core track and review codecs

> Update track, impl-plan, and review persistence writers; add local regressions. [IN-01; IN-02; OS-01; CN-01; CN-03; AC-01; AC-02]

- [x] **T1**: Update `libs/infrastructure/src/track/codec.rs::encode`; add codec-local serialization regression. [IN-01; IN-02; OS-01; CN-01; CN-03; AC-01]
- [ ] **T2**: Update `libs/infrastructure/src/impl_plan_codec.rs::encode`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01]
- [ ] **T3**: Update `libs/infrastructure/src/review_v2/persistence/mod.rs::write_atomic`; add fast- and final-round fixtures in `apps/cli-composition/src/review_v2/run.rs`. [IN-01; IN-02; CN-01; AC-01; AC-02]

### track-artifact-codecs — Track artifact codecs

> Update spec, task-coverage, schema-export, signal, and task-contract codecs; add local regressions. [IN-01; IN-02; CN-01; AC-01]

- [ ] **T4**: Update `libs/infrastructure/src/spec/codec.rs`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01]
- [ ] **T5**: Update `libs/infrastructure/src/task_coverage_codec.rs`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01]
- [ ] **T6**: Update `libs/infrastructure/src/schema_export_codec.rs`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01]
- [ ] **T7**: Update `libs/infrastructure/src/signal.rs`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01]
- [ ] **T23**: Update `libs/infrastructure/src/task_contract_codec.rs::encode`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01]

### tddd-artifact-codecs — TDDD artifact codecs

> Update TDDD catalogue and signal codecs; add local regressions. [IN-01; IN-02; CN-01; AC-01]

- [ ] **T8**: Update `libs/infrastructure/src/tddd/catalogue_document_codec/mod.rs`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01]
- [ ] **T9**: Update `libs/infrastructure/src/tddd/catalogue_spec_signals_codec.rs`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01]
- [ ] **T10**: Update `libs/infrastructure/src/tddd/semantic_verify_codec.rs::SpecAdrVerifyCacheDocumentCodec::encode`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01]
- [ ] **T11**: Update `libs/infrastructure/src/tddd/semantic_verify_codec.rs::CatalogueSpecVerifyCacheDocumentCodec::encode`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01]
- [ ] **T12**: Update `libs/infrastructure/src/tddd/type_signals_codec.rs`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01]
- [ ] **T13**: Update `libs/infrastructure/src/tddd/catalog_gen/fs_access.rs`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01]

### baseline-dry-check-codecs — Baseline and dry-check codecs

> Update ADR-baseline, dry-check, corpus-root, and coverage writers; add local regressions. [IN-01; IN-02; CN-01; AC-01]

- [ ] **T14**: Update `libs/infrastructure/src/adr_baseline.rs`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01]
- [ ] **T15**: Update `libs/infrastructure/src/dry_check/store.rs`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01]
- [ ] **T16**: Update `libs/infrastructure/src/dry_check/dry_write_driver/manifest.rs`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01]
- [ ] **T24**: Update `libs/infrastructure/src/dry_check/dry_write_driver.rs::FsDryCorpusRootManifestAdapter::write`; add writer-local serialization regression. [IN-01; IN-02; CN-01; AC-01]
- [ ] **T25**: Update `libs/infrastructure/src/dry_check/coverage.rs::FsDryCheckCoverageAdapter::write_coverage`; add writer-local serialization regression. [IN-01; IN-02; CN-01; AC-01]

### test-obligation-codecs — Test-obligation codecs

> Update test-obligation codecs; add local regressions. [IN-01; IN-02; CN-01; AC-01]

- [ ] **T17**: Update `libs/infrastructure/src/test_obligation/bindings_codec.rs`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01]
- [ ] **T18**: Update `libs/infrastructure/src/test_obligation/obligations_codec.rs`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01]
- [ ] **T19**: Update `libs/infrastructure/src/test_obligation/waiver_cache_codec.rs`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01]
- [ ] **T20**: Update `libs/infrastructure/src/test_obligation/fulfillment_cache_codec/fulfillment_cache_io.rs`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01]

### operational-cache-telemetry-writers — Operational cache and telemetry writers

> Update provider-session, telemetry, archived-track telemetry, and PR trigger-state writers; add local regressions. [IN-01; IN-02; CN-01; AC-01]

- [ ] **T26**: Update `libs/infrastructure/src/provider_session.rs::FsProviderSessionCacheAdapter::save`; add writer-local serialization regression. [IN-01; IN-02; CN-01; AC-01]
- [ ] **T27**: Update `libs/infrastructure/src/telemetry/writer.rs::build_line`; add writer-local serialization regression. [IN-01; IN-02; CN-01; AC-01]
- [ ] **T28**: Update `libs/infrastructure/src/telemetry/archived_track.rs::FsArchivedTrackTelemetryAdapter::emit`; add writer-local serialization regression. [IN-01; IN-02; CN-01; AC-01]
- [ ] **T29**: Update `libs/infrastructure/src/pr/poll.rs::save_trigger_state`; add writer-local serialization regression. [IN-01; IN-02; CN-01; AC-01]

### track-lifecycle-fixtures — Track lifecycle fixtures

> Extend `test_obligation.rs::TestObligationCompositionRoot::derive_handler` fixtures. [IN-03; OS-02; CN-02; AC-03; AC-04]

- [ ] **T21**: Extend `apps/cli-composition/src/test_obligation.rs::TestObligationCompositionRoot::derive_handler` fixtures for two active-branch invocations. [IN-03; CN-02; AC-03]
- [ ] **T22**: Extend `apps/cli-composition/src/test_obligation.rs::TestObligationCompositionRoot::derive_handler` fixtures for a completed-track invocation. [OS-02; CN-02; AC-04]
