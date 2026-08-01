<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# Deterministic JSON Serialization

## Summary

GO-01: T1-T29.

## Tasks (29/29 resolved)

### core-track-review-codecs — Core track and review codecs

> Update track, impl-plan, and review persistence writers; add local regressions. [IN-01; IN-02; OS-01; CN-01; CN-03; AC-01; AC-02]

- [x] **T1**: Update `libs/infrastructure/src/track/codec.rs::encode`; add codec-local serialization regression. [IN-01; IN-02; OS-01; CN-01; CN-03; AC-01] (`74197666e0b75a3199712b42f791c0ae3ee2d6a9`)
- [x] **T2**: Update `libs/infrastructure/src/impl_plan_codec.rs::encode`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01] (`7309757b2582e1511500123f1985bf12592ff6c1`)
- [x] **T3**: Update `libs/infrastructure/src/review_v2/persistence/mod.rs::write_atomic`; add fast- and final-round fixtures in `apps/cli-composition/src/review_v2/run.rs`. [IN-01; IN-02; CN-01; AC-01; AC-02] (`7309757b2582e1511500123f1985bf12592ff6c1`)

### track-artifact-codecs — Track artifact codecs

> Update spec, task-coverage, schema-export, signal, and task-contract codecs; add local regressions. [IN-01; IN-02; CN-01; AC-01]

- [x] **T4**: Update `libs/infrastructure/src/spec/codec.rs`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01] (`7309757b2582e1511500123f1985bf12592ff6c1`)
- [x] **T5**: Update `libs/infrastructure/src/task_coverage_codec.rs`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01] (`7309757b2582e1511500123f1985bf12592ff6c1`)
- [x] **T6**: Update `libs/infrastructure/src/schema_export_codec.rs`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01] (`7309757b2582e1511500123f1985bf12592ff6c1`)
- [x] **T7**: Update `libs/infrastructure/src/signal.rs`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01] (`5ca034ea1a46028e7ed522823a42546b73963106`)
- [x] **T23**: Update `libs/infrastructure/src/task_contract_codec.rs::encode`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01] (`768d65c819d7a5966a8f8bccd38a91221da94146`)

### tddd-artifact-codecs — TDDD artifact codecs

> Update TDDD catalogue and signal codecs; add local regressions. [IN-01; IN-02; CN-01; AC-01]

- [x] **T8**: Update `libs/infrastructure/src/tddd/catalogue_document_codec/mod.rs`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01] (`5ca034ea1a46028e7ed522823a42546b73963106`)
- [x] **T9**: Update `libs/infrastructure/src/tddd/catalogue_spec_signals_codec.rs`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01] (`5ca034ea1a46028e7ed522823a42546b73963106`)
- [x] **T10**: Update `libs/infrastructure/src/tddd/semantic_verify_codec.rs::SpecAdrVerifyCacheDocumentCodec::encode`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01] (`5ca034ea1a46028e7ed522823a42546b73963106`)
- [x] **T11**: Update `libs/infrastructure/src/tddd/semantic_verify_codec.rs::CatalogueSpecVerifyCacheDocumentCodec::encode`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01] (`940c6788dae096b8d43b7ffa6629a4ee2b3b273d`)
- [x] **T12**: Update `libs/infrastructure/src/tddd/type_signals_codec.rs`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01] (`5ca034ea1a46028e7ed522823a42546b73963106`)
- [x] **T13**: Update `libs/infrastructure/src/tddd/catalog_gen/fs_access.rs`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01] (`5ca034ea1a46028e7ed522823a42546b73963106`)

### baseline-dry-check-codecs — Baseline and dry-check codecs

> Update ADR-baseline, dry-check, corpus-root, and coverage writers; add local regressions. [IN-01; IN-02; CN-01; AC-01]

- [x] **T14**: Update `libs/infrastructure/src/adr_baseline.rs`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01] (`940c6788dae096b8d43b7ffa6629a4ee2b3b273d`)
- [x] **T15**: Update `libs/infrastructure/src/dry_check/store.rs`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01] (`940c6788dae096b8d43b7ffa6629a4ee2b3b273d`)
- [x] **T16**: Update `libs/infrastructure/src/dry_check/dry_write_driver/manifest.rs`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01] (`940c6788dae096b8d43b7ffa6629a4ee2b3b273d`)
- [x] **T24**: Update `libs/infrastructure/src/dry_check/dry_write_driver.rs::FsDryCorpusRootManifestAdapter::write`; add writer-local serialization regression. [IN-01; IN-02; CN-01; AC-01] (`768d65c819d7a5966a8f8bccd38a91221da94146`)
- [x] **T25**: Update `libs/infrastructure/src/dry_check/coverage.rs::FsDryCheckCoverageAdapter::write_coverage`; add writer-local serialization regression. [IN-01; IN-02; CN-01; AC-01]

### test-obligation-codecs — Test-obligation codecs

> Update test-obligation codecs; add local regressions. [IN-01; IN-02; CN-01; AC-01]

- [x] **T17**: Update `libs/infrastructure/src/test_obligation/bindings_codec.rs`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01] (`940c6788dae096b8d43b7ffa6629a4ee2b3b273d`)
- [x] **T18**: Update `libs/infrastructure/src/test_obligation/obligations_codec.rs`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01] (`768d65c819d7a5966a8f8bccd38a91221da94146`)
- [x] **T19**: Update `libs/infrastructure/src/test_obligation/waiver_cache_codec.rs`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01] (`768d65c819d7a5966a8f8bccd38a91221da94146`)
- [x] **T20**: Update `libs/infrastructure/src/test_obligation/fulfillment_cache_codec/fulfillment_cache_io.rs`; add codec-local serialization regression. [IN-01; IN-02; CN-01; AC-01] (`768d65c819d7a5966a8f8bccd38a91221da94146`)

### operational-cache-telemetry-writers — Operational cache and telemetry writers

> Update provider-session, telemetry, archived-track telemetry, and PR trigger-state writers; add local regressions. [IN-01; IN-02; CN-01; AC-01]

- [x] **T26**: Update `libs/infrastructure/src/provider_session.rs::FsProviderSessionCacheAdapter::save`; add writer-local serialization regression. [IN-01; IN-02; CN-01; AC-01]
- [x] **T27**: Update `libs/infrastructure/src/telemetry/writer.rs::build_line`; add writer-local serialization regression. [IN-01; IN-02; CN-01; AC-01]
- [x] **T28**: Update `libs/infrastructure/src/telemetry/archived_track.rs::FsArchivedTrackTelemetryAdapter::emit`; add writer-local serialization regression. [IN-01; IN-02; CN-01; AC-01]
- [x] **T29**: Update `libs/infrastructure/src/pr/poll.rs::save_trigger_state`; add writer-local serialization regression. [IN-01; IN-02; CN-01; AC-01]

### track-lifecycle-fixtures — Track lifecycle fixtures

> Extend `test_obligation.rs::TestObligationCompositionRoot::derive_handler` fixtures. [IN-03; OS-02; CN-02; AC-03; AC-04]

- [x] **T21**: Extend `apps/cli-composition/src/test_obligation.rs::TestObligationCompositionRoot::derive_handler` fixtures for two active-branch invocations. [IN-03; CN-02; AC-03] (`7309757b2582e1511500123f1985bf12592ff6c1`)
- [x] **T22**: Implement `libs/domain/src/track.rs::TrackStatus::frozen_status` with focused domain tests; update `libs/usecase/src/test_obligation/derive.rs::DeriveTestObligationsInteractor::execute` and its pre-write guard tests; use/verify `libs/infrastructure/src/track/track_status_reader_adapter.rs::FsTrackStatusReaderAdapter` with focused adapter coverage; wire it into `apps/cli-composition/src/test_obligation.rs::TestObligationCompositionRoot::derive_handler` and retain completed-track no-rewrite invocation fixtures. [OS-02; CN-02; AC-04] (`5ca034ea1a46028e7ed522823a42546b73963106`)
