<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# Base Merge and Conflict Recovery

## Summary

GO-01 → T001–T004, T010, and T011.
GO-02 → T005–T009.

## Tasks (7/11 resolved)

### S1 — Guarded base merge

> Modify `libs/domain/src/branch_strategy.rs`, `libs/usecase/src/base_merge.rs`, `libs/infrastructure/src/base_merge.rs`, and the named track CLI surfaces through T001–T004, T010, and T011. D1; IN-01; IN-02; IN-06; CN-05; CN-06; CN-07; AC-01; AC-02; AC-03; AC-04; AC-10; AC-11; AC-12.

- [x] **T001**: Add `BaseBranchName`, `BaseMergeDirection`, `BaseMergeDirectionError`, and `derive_base_merge_direction` in `libs/domain/src/branch_strategy.rs`, updating `TrackMetadata` in `libs/domain/src/track.rs`; add regression coverage. D1; IN-01. (`daf758c60802d14be799f267826bec46a2cd8782`)
- [x] **T002**: Add `libs/usecase/src/base_merge.rs` with `BaseMergeContextPort`, `BaseMergeGitPort`, `BaseMergeCleanupPort`, `BaseMergeService`, `BaseMergeInteractor`, command, outcomes, and errors; export it from `libs/usecase/src/lib.rs` and add regression coverage. D1; IN-01; IN-02; OS-01; CN-01; CN-02; AC-01; AC-02; AC-03; AC-04. (`a5259acf836d40408c7940ef73798a2f76ba721e`)
- [x] **T010**: Update `BaseMergeAttemptOutcome`, `BaseMergeError`, `BaseMergeCleanupRequest`, `PostMergeCleanupError`, `ViewsRegenerationError`, `BaselineReplacementError`, `SyncBaseRecordError`, the base-merge cleanup and git ports, and the sync-base record adapter with their regression coverage. D1; IN-02; IN-06; CN-05; CN-06; CN-07; AC-03; AC-10; AC-11; AC-12.
- [x] **T003**: Add `FsBaseMergeContextAdapter` and `FsBaseMergeGitAdapter` in `libs/infrastructure/src/base_merge.rs`; export the module from `libs/infrastructure/src/lib.rs` and add focused adapter regression coverage. D1; IN-01; IN-02; OS-01; CN-01; AC-01; AC-02; AC-03; AC-04.
- [ ] **T011**: Add `FsBaseMergeCleanupAdapter` in `libs/infrastructure/src/base_merge.rs` and focused cleanup adapter regression coverage. D1; IN-02; IN-06; CN-02; CN-05; CN-06; CN-07; AC-03; AC-04; AC-10; AC-11; AC-12.
- [ ] **T004**: Add `BaseMergeInput` and `TrackDriver` handling in `apps/cli-driver/src/track.rs`, the `TrackCommand` variant and `execute` dispatch in `apps/cli/src/commands/track/{mod.rs,dispatch.rs}`, and `TrackCompositionRoot::track_driver` wiring in `apps/cli-composition/src/track/composition_root.rs`; add command regression coverage. D1; IN-01; OS-01; CN-01; AC-01; AC-02; AC-03; AC-04.

### S2 — Conflict recovery and guarded stash

> Add `.harness/workflows/track/recover.md`, its named Claude/Codex adapters, `libs/usecase/src/git_stash.rs`, and the named stash adapters in T006. D2; D3; IN-03; IN-04; AC-05; AC-06.

- [ ] **T005**: Add `.harness/workflows/track/recover.md`, `.claude/commands/track/recover.md`, and `.agents/skills/track-recover/SKILL.md` for the `recover` command surface; add workflow/adapter conformance coverage. D2; IN-03; OS-02; CN-02; AC-04; AC-05.
- [ ] **T006**: Add `libs/usecase/src/git_stash.rs` with `GitStashPort`, `GitStashService`, `GitStashInteractor`, command, and error; add `FsGitStashAdapter` in `libs/infrastructure/src/git_cli/stash_adapter.rs`, wire `GitDriver` and `GitStashInput` in `apps/cli-driver/src/git.rs`, and add `GitCommand`, `GitStashAction`, and `execute` integration in `apps/cli/src/commands/git.rs` with composition wiring in `apps/cli-composition/src/git.rs`; add regression coverage. D3; IN-04; AC-06.

### S3 — Baseline-hash-aware type signals

> Modify `libs/domain/src/tddd/type_signals_doc.rs`, `libs/usecase/src/type_signals/{service.rs,ports.rs,interactor.rs}`, and `libs/infrastructure/src/tddd/{type_signals_executor_adapter.rs,type_signals_codec.rs}`. D4; IN-05; AC-07; AC-08; AC-09.

- [x] **T007**: At the domain boundary, modify `BaselineHash`, `TypeSignalsCacheKey`, `TypeSignalsDocument`, and `decide_type_signals_reuse` in `libs/domain/src/tddd/type_signals_doc.rs`; migrate every domain-local constructor and caller to supply and compare the three-hash cache key, with regression coverage. T008 owns the usecase port/service/interactor migrations and T009 owns the infrastructure adapter/codec migrations. D4; IN-05; OS-03; CN-03; CN-04; AC-07; AC-08; AC-09. (`daf758c60802d14be799f267826bec46a2cd8782`)
- [x] **T008**: At the usecase boundary, modify `TypeSignalsError` in `libs/usecase/src/type_signals/service.rs`, `TypeSignalsExecutionError` and `TypeSignalsExecutorPort` in `ports.rs`, and `TypeSignalsInteractor` in `interactor.rs`; migrate every usecase constructor and caller, with regression coverage in `interactor/tests.rs`. T007 owns domain constructors and T009 owns adapter/codec callers. D4; IN-05; OS-03; CN-03; CN-04; AC-07; AC-08; AC-09. (`daf758c60802d14be799f267826bec46a2cd8782`)
- [x] **T009**: At the infrastructure boundary, modify `EvaluateSignalsError` and preserve the public `execute_type_signals_for_layer` evaluator entrypoint in `type_signals_evaluator.rs`, and modify `TypeSignalsExecutorAdapter` in `libs/infrastructure/src/tddd/type_signals_executor_adapter.rs` and `baseline_hash`, `decode`, and `encode` in `type_signals_codec.rs`; migrate every adapter and codec constructor/caller, with evaluator/adapter/codec regression coverage. T007 owns domain constructors and T008 owns usecase port/interactor callers. D4; IN-05; OS-03; CN-03; CN-04; AC-07; AC-08; AC-09. (`daf758c60802d14be799f267826bec46a2cd8782`)
