<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# Base Merge and Conflict Recovery

## Summary

GO-01 → T001–T004, T010, T011, T013, and T014.
GO-02 → T005–T009, T012, T015, T016, T017, and T018.

## Tasks (18/18 resolved)

### S1 — Guarded base merge

> Modify `libs/domain/src/branch_strategy.rs`, `libs/usecase/src/base_merge.rs`, `libs/infrastructure/src/base_merge.rs`, and the named track CLI surfaces through T001–T004, T010, and T011. D1; IN-01; IN-02; IN-06; CN-05; CN-06; CN-07; AC-01; AC-02; AC-03; AC-04; AC-10; AC-11; AC-12.

- [x] **T001**: Add `BaseBranchName`, `BaseMergeDirection`, `BaseMergeDirectionError`, and `derive_base_merge_direction` in `libs/domain/src/branch_strategy.rs`, updating `TrackMetadata` in `libs/domain/src/track.rs`; add regression coverage. D1; IN-01. (`daf758c60802d14be799f267826bec46a2cd8782`)
- [x] **T002**: Add `libs/usecase/src/base_merge.rs` with `BaseMergeContextPort`, `BaseMergeGitPort`, `BaseMergeCleanupPort`, `BaseMergeService`, `BaseMergeInteractor`, command, outcomes, and errors; export it from `libs/usecase/src/lib.rs` and add regression coverage. D1; IN-01; IN-02; OS-01; CN-01; CN-02; AC-01; AC-02; AC-03; AC-04. (`a5259acf836d40408c7940ef73798a2f76ba721e`)
- [x] **T010**: Update `BaseMergeAttemptOutcome`, `BaseMergeError`, `BaseMergeCleanupRequest`, `PostMergeCleanupError`, `ViewsRegenerationError`, `BaselineReplacementError`, `SyncBaseRecordError`, the base-merge cleanup and git ports, `SyncBaseRecordSchemaVersion` and `SyncBaseRecord` DTO/schema entries, and the sync-base record adapter with their regression coverage. D1; IN-02; IN-06; CN-05; CN-06; CN-07; AC-03; AC-10; AC-11; AC-12. (`80437ad2`)
- [x] **T003**: Add `FsBaseMergeContextAdapter` and `FsBaseMergeGitAdapter` in `libs/infrastructure/src/base_merge.rs`; export the module from `libs/infrastructure/src/lib.rs` and add focused adapter regression coverage. D1; IN-01; IN-02; OS-01; CN-01; AC-01; AC-02; AC-03; AC-04. (`80437ad2`)
- [x] **T011**: Add `FsBaseMergeCleanupAdapter` in `libs/infrastructure/src/base_merge.rs` and focused cleanup adapter regression coverage. D1; IN-02; IN-06; CN-02; CN-05; CN-06; CN-07; AC-03; AC-04; AC-10; AC-11; AC-12. (`80437ad2`)
- [x] **T004**: Add `BaseMergeInput` and `TrackDriver` handling in `apps/cli-driver/src/track.rs`, the `TrackCommand` variant and `execute` dispatch in `apps/cli/src/commands/track/{mod.rs,dispatch.rs}`, and `TrackCompositionRoot::track_driver` wiring in `apps/cli-composition/src/track/composition_root.rs`; add command regression coverage. D1; IN-01; OS-01; CN-01; AC-01; AC-02; AC-03; AC-04. (`18b2e9a8`)

### S2 — Conflict recovery and guarded stash

> Add `.harness/workflows/track/recover.md`, its named Claude/Codex adapters, `libs/usecase/src/git_stash.rs`, and the named stash adapters in T006. D2; D3; IN-03; IN-04; AC-05; AC-06.

- [x] **T005**: Add `.harness/workflows/track/recover.md`, `.claude/commands/track/recover.md`, and `.agents/skills/track-recover/SKILL.md` for the `recover` command surface; add workflow/adapter conformance coverage. D2; IN-03; OS-02; CN-02; AC-04; AC-05. (`18b2e9a8`)
- [x] **T006**: Add `libs/usecase/src/git_stash.rs` with `GitStashPort`, `GitStashService`, `GitStashInteractor`, command, and error; add `FsGitStashAdapter` in `libs/infrastructure/src/git_cli/stash_adapter.rs`, wire `GitDriver` and `GitStashInput` in `apps/cli-driver/src/git.rs`, and add `GitCommand`, `GitStashAction`, and `execute` integration in `apps/cli/src/commands/git.rs` with composition wiring in `apps/cli-composition/src/git.rs`; add regression coverage. D3; IN-04; AC-06. (`4e72b48a9371a977700f2dc8e74cae90f4828466`)

### S3 — Baseline-hash-aware type signals

> Modify `libs/domain/src/tddd/type_signals_doc.rs`, `libs/usecase/src/type_signals/{service.rs,ports.rs,interactor.rs}`, and `libs/infrastructure/src/tddd/{type_signals_executor_adapter.rs,type_signals_codec.rs}`; update `TrackBlobReader` in `libs/usecase/src/merge_gate.rs` and `check_impl_catalog_from_signals_file` in `libs/infrastructure/src/verify/spec_states.rs` through T012. D4; IN-05; AC-07; AC-08; AC-09.

- [x] **T007**: At the domain boundary, modify `BaselineHash`, `TypeSignalsCacheKey`, `TypeSignalsDocument`, and `decide_type_signals_reuse` in `libs/domain/src/tddd/type_signals_doc.rs`; migrate every domain-local constructor and caller to supply and compare the three-hash cache key, with regression coverage. T008 owns the usecase port/service/interactor migrations and T009 owns the infrastructure adapter/codec migrations. D4; IN-05; OS-03; CN-03; CN-04; AC-07; AC-08; AC-09. (`daf758c60802d14be799f267826bec46a2cd8782`)
- [x] **T008**: At the usecase boundary, modify `TypeSignalsError` in `libs/usecase/src/type_signals/service.rs`, `TypeSignalsExecutionError` and `TypeSignalsExecutorPort` in `ports.rs`, and `TypeSignalsInteractor` in `interactor.rs`; migrate every usecase constructor and caller, with regression coverage in `interactor/tests.rs`. T007 owns domain constructors and T009 owns adapter/codec callers. D4; IN-05; OS-03; CN-03; CN-04; AC-07; AC-08; AC-09. (`daf758c60802d14be799f267826bec46a2cd8782`)
- [x] **T009**: At the infrastructure boundary, modify `EvaluateSignalsError` and preserve the public `execute_type_signals_for_layer` evaluator entrypoint in `type_signals_evaluator.rs`, and modify `TypeSignalsExecutorAdapter` in `libs/infrastructure/src/tddd/type_signals_executor_adapter.rs` and `baseline_hash`, `decode`, and `encode` in `type_signals_codec.rs`; migrate every adapter and codec constructor/caller, with evaluator/adapter/codec regression coverage. T007 owns domain constructors and T008 owns usecase port/interactor callers. D4; IN-05; OS-03; CN-03; CN-04; AC-07; AC-08; AC-09. (`daf758c60802d14be799f267826bec46a2cd8782`)
- [x] **T012**: Update `infrastructure::verify::spec_states::check_impl_catalog_from_signals_file` in `libs/infrastructure/src/verify/spec_states.rs` to accept explicit baseline-path and catalogue-hash inputs; update `TrackBlobReader` in `libs/usecase/src/merge_gate.rs` and `GitShowTrackBlobReader` in `libs/infrastructure/src/verify/merge_gate_adapter.rs` to read implementation-input hashes with catalogue declarations; update `SystemSignalCommandAdapter::run_catalogue_check` in `libs/infrastructure/src/signal.rs`, migrate callers, and add regression coverage. D4; IN-05; OS-03; CN-03; CN-04; AC-07; AC-09. (`b307b114c7493685fa1514d6b4673cc98319dc6a`)

### S4 — Hook-safe isolated baseline worktree

> Pin isolated baseline-worktree git operations to the main repository's current absolute hook path. IN-02; CN-05; AC-03; AC-10.

- [x] **T013**: Pin every ephemeral base-merge worktree git operation in `libs/infrastructure/src/base_merge.rs` to the main repository's absolute current `.githooks` path, and add regression coverage. IN-02; CN-05; AC-03; AC-10. (`037748e67a34f72bc18caaa352ccc884b507c596`)

### S5 — Second guarded base-merge integration

> Resolve the develop 83c64c2e apps/cli conflict and execute recovery baseline-currency capture. AC-04.

- [x] **T014**: Integrate the second base merge (develop 83c64c2e) by resolving the existing conflict hunks in `apps/cli/src/commands/track/mod.rs` and `apps/cli/src/main.rs`, then run `bin/sotp track baseline-capture --source-workspace <exact-merged-base-disposable-clone>` to refresh the active track's per-layer `<layer>-types-baseline.json` files after recovery review. AC-04. (`e44e0e56`)

### S6 — Baseline-fresh rendered signals

> Update type-signals rendering in `libs/infrastructure/src/track/render/sync.rs` and add focused regression coverage. IN-05; CN-03; CN-04; AC-07; AC-09.

- [x] **T015**: Update type-signals rendering in `libs/infrastructure/src/track/render/sync.rs` and add focused regression coverage. IN-05; CN-03; CN-04; AC-07; AC-09. (`fad2c002`)

### S7 — Baseline-fresh pre-review signals

> Update pre-review type-signal handling in `FsImplCatalogSignalReader::read_signals` and add focused regression coverage. IN-05; CN-03; CN-04; AC-07; AC-09.

- [x] **T016**: In `FsImplCatalogSignalReader::read_signals`, reject stale or unreadable present-baseline type-signals documents before pre-review evaluation, and add focused regression coverage. IN-05; CN-03; CN-04; AC-07; AC-09. (`13f2d1a2`)

### S8 — Path-dependency-aware implementation inputs

> In `libs/infrastructure/src/tddd/type_signals_evaluator/build_inputs.rs`, update `hash_implementation_inputs_with_toolchain_identifier` and `hash_implementation_input_components` to use the architecture-rules layer graph; update `libs/infrastructure/src/verify/branch_implementation_inputs.rs::hash_branch_implementation_inputs` and add component-sensitivity regression coverage. IN-05; CN-03; CN-04; AC-07; AC-09.

- [x] **T017**: In `libs/infrastructure/src/tddd/type_signals_evaluator/build_inputs.rs`, update `hash_implementation_inputs_with_toolchain_identifier` and `hash_implementation_input_components` to use the architecture-rules layer graph; update `libs/infrastructure/src/verify/branch_implementation_inputs.rs::hash_branch_implementation_inputs` and add component-sensitivity regression coverage. IN-05; CN-03; CN-04; AC-07; AC-09. (`b193ed6b`)

### S9 — Mode-independent implementation inputs

> Update `libs/infrastructure/src/tddd/type_signals_evaluator/build_inputs.rs::{hash_implementation_inputs_with_toolchain_identifier,hash_implementation_input_components}` and `libs/infrastructure/src/verify/branch_implementation_inputs.rs::hash_branch_implementation_inputs`; add focused regression coverage. IN-05; CN-03; CN-04; AC-07; AC-09.

- [x] **T018**: Update `libs/infrastructure/src/tddd/type_signals_evaluator/build_inputs.rs::{hash_implementation_inputs_with_toolchain_identifier,hash_implementation_input_components}` and `libs/infrastructure/src/verify/branch_implementation_inputs.rs::hash_branch_implementation_inputs`; add chmod-sensitivity and local/branch parity regression coverage. IN-05; CN-03; CN-04; AC-07; AC-09. (`5ba9b774`)
