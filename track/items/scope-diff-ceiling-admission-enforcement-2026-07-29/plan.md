<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# per-scope diff ceiling の実装開始前 admission 強制

## Summary

GO-01 → T001–T012、T019、T020、T013、T016、T018: 宣言された batch plan、依存宣言、設定参照、check command、transition admission を fail-closed に接続する。
GO-02 → T003、T004、T006、T012、T018: Phase 3 終端と todo → in_progress 遷移の実装開始前 gate を配置する。
GO-03 → T001、T002、T019、T020、T007、T008、T013、T014、T017: batch と依存宣言を Phase 3 宣言成果物へ移し、impl-plan review と full-cycle consumption の対象にする。
GO-01 / GO-02 → T021: PR #228 round 1 のデルタ 3 件（scope 名の設定照合、宣言対象の未 settle 限定、判定対象遷移集合の拡張）を実装する。

## Tasks (21/21 resolved)

### S1 — ドメイン中核 — 見積り・batch 宣言と 2 つの純粋判定

- [x] **T001**: `libs/domain/src/batch_plan/` に LineCount、ScopeCeiling、IndivisibilityJustification、TaskDecomposition、ScopeLineEstimate、TaskEstimate、BatchId、BatchDeclaration、MeasuredScopeDiff、BatchPlanValidationError を実装する。IN-02、IN-03、IN-04、AC-02、AC-03。 (`8ee295dd`)
- [x] **T002**: `libs/domain/src/batch_plan/` に BatchPlanDocument 集約を実装する。IN-05、AC-04、CN-01。 (`8ee295dd`)
- [x] **T019**: `libs/domain/` の TrackTask に depends_on とアクセサ・それを受理する constructor を追加し、ValidationError variant と plan document constructor の検査を更新する。IN-19、IN-20、AC-21、AC-22。 (`7573d1c1`)
- [x] **T003**: T019 の完了後に `libs/domain/src/batch_plan/` へ check_batch_plan、BatchPlanGateOutcome、NonEmptyGateViolations、BatchPlanGateViolation を実装し、ReviewScopeConfig を参照して declared-dependency batch-order validation を追加する。IN-06、IN-07、AC-06、AC-07、AC-08、CN-02。 (`185d6f08`)
- [x] **T004**: `libs/domain/src/batch_plan/` に evaluate_admission、AdmissionDecision、AdmissionRejection、AdmissionEvaluationError、NonZeroLineCount を実装する。IN-09、IN-10、IN-11、AC-10、AC-11、AC-12、AC-13、AC-14、CN-03、CN-04。 (`283f508e`)

### S2 — 適用層と外界接続 — port / gate service / codec / adapter

- [x] **T005**: `libs/usecase/src/batch_plan/` に BatchPlanReaderPort、PlannedTaskReaderPort、ScopeConfigReaderPort、ScopeDiffMeasurePort、PlanArtifactReadError、ScopeConfigReadError、ScopeDiffMeasureError を定義する。IN-01、AC-01、AC-05、CN-09。 (`283f508e`)
- [x] **T006**: `libs/usecase/src/batch_plan/` に BatchPlanCheckService、BatchPlanCheckCommand、BatchPlanCheckError、BatchPlanCheckInteractor、BatchPlanCheckOutput、BatchPlanViolationOutput、NonEmptyViolationOutputs を実装する。IN-06、IN-07、AC-05、AC-06、AC-07、CN-09。 (`283f508e`)
- [x] **T007**: `libs/infrastructure/src/batch_plan_codec/` に BatchPlanDocumentDto、TaskEstimateDto、ScopeLineEstimateDto、BatchDeclarationDto、BatchPlanCodecError、decode を実装する。IN-01、IN-02、IN-03、IN-04、IN-05、AC-01、AC-02、AC-03、AC-04。 (`283f508e`)
- [x] **T020**: `libs/infrastructure/` の ImplPlanTaskDto に serde default 付きの depends_on を追加し、impl-plan codec の schema read/write を更新する。IN-19、IN-20、AC-05、AC-21、AC-22。 (`283f508e`)
- [x] **T008**: T020 の完了後に `libs/infrastructure/` へ FsBatchPlanReader と FsPlannedTaskReader を実装して usecase port へ接続する。IN-01、IN-06、IN-07、AC-01、AC-05、AC-07、CN-09。 (`89e46b20`)
- [x] **T009**: `libs/infrastructure/` に FsReviewScopeConfigReader と GitScopeDiffMeasurer を実装して usecase port へ接続する。IN-14、AC-08、AC-17、CN-05。 (`89e46b20`)

### S3 — 配送面 — driver / composition root / CLI コマンド

- [x] **T010**: `apps/cli-driver/` に BatchPlanDriver と BatchPlanInput を実装する。IN-06、AC-06、AC-07。 (`89e46b20`)
- [x] **T011**: `libs/infrastructure/` に LazyBranchReader を追加し、`apps/cli-composition/` に BatchPlanCompositionRoot を追加して batch-plan check の依存を配線する。IN-06、AC-06。 (`89e46b20`)
- [x] **T012**: `apps/cli/` に BatchPlanCheckArgs、BatchPlanCommand、CliCommand の BatchPlan variant、batch_plan::execute を追加する。IN-06、AC-05、AC-06。 (`89e46b20`)

### S4 — 正本伝播 — capability 契約 / review 指示書 / workflow

- [x] **T013**: `.harness/capabilities/impl-planner.md` の Phase 3 authoring と write ownership を batch-plan.json に対応させて更新する。IN-01、IN-02、IN-03、IN-04、IN-17、AC-01、AC-02、AC-03、AC-18、CN-07、CN-08。 (`ab4eed67`)
- [x] **T014**: `.harness/custom/review-prompts/impl-plan.md` と `.harness/config/review-scope.json` を更新し、batch-plan review checks と artifact pattern を追加する。IN-08、AC-01、AC-09、CN-02。 (`ab4eed67`)
- [x] **T015**: `.harness/capabilities/implementer.md` に task-state pre-work precondition を追加する。IN-13、AC-16、CN-10。 (`ab4eed67`)
- [x] **T016**: `.harness/capabilities/{spec-designer,type-designer,adr-editor,implementer,researcher,review-fix-lead,dry-fix-lead,rollback-diagnoser}.md` の writes-forbidden 列に batch-plan.json を追加して impl-planner 専有を伝播し、`.harness/workflows/track/impl-plan.md`、`.claude/commands/track/impl-plan.md`（impl-planner 専有の forwarding）、`.agents/skills/track-impl-plan/SKILL.md`（同じ forwarding）、Makefile.toml を更新する。IN-01、IN-18、AC-01、AC-19、AC-20、CN-06。 (`a81ed7d1`)
- [x] **T017**: plan order 上で先行する全タスクの完了後に `.harness/workflows/track/full-cycle.md` の batch-consumption steps と scope-diff reference を更新する。IN-12、IN-15、IN-16、AC-15、AC-18、AC-19、CN-06、CN-09。 (`a81ed7d1`)

### S5 — 遷移経路への admission 内蔵（track 終端）

- [x] **T018**: plan order 上で先行する全タスクの完了後、本 track の最終タスクとして `TaskOperationInteractor`、TaskOperationService、task-ops composition root を admission 依存へ更新し、`ImplPlanDocument` に settled_task_ids と in_progress_task_ids を追加し、TaskTransitionOutcome と AdmissionRejectionOutput を実装し、`TransitionTaskUseCase` を公開 API から撤去する。IN-09、IN-10、IN-11、AC-10、AC-11、AC-12、AC-13、AC-14、CN-04、CN-10。 (`9beee43f`)

### S6 — PR #228 round 1 デルタ — scope 名の設定照合と宣言対象の未 settle 限定

- [x] **T021**: `libs/domain/src/batch_plan/{mod,gate,admission}.rs`: check_batch_plan に scope 名の設定照合検査を追加、BatchPlanGateViolation と AdmissionEvaluationError に UnknownMainScopeName を追加、collect_task_set_violations の UnplannedTask 報告域を限定（IN-03、IN-07、IN-11、IN-21、AC-07、AC-08、AC-23、AC-24、AC-25）。`libs/domain/src/impl_plan.rs`: ImplPlanDocument に transition_for を追加。`libs/usecase/src/{batch_plan/output,task_admission,task_ops}.rs`: BatchPlanViolationOutput の鏡像 variant と写像、admission error の写像、judge_start_of_work の判定対象条件を transition_for 経由へ差し替え（IN-09、AC-26、AC-27）。`apps/cli-driver/src/batch_plan.rs`: 描画 arm を追加（AC-07）。 (`3551f266`)
