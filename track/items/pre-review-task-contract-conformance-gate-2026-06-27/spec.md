<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 33, yellow: 0, red: 0 }
---

# タスク単位の契約履行 pre-review ゲート

## Goal

- [GO-01] 型カタログと実装の構造的一致（impl_catalog 信号）を確認する最初の hard gate を、現行の merge-gate（strict）から reviewer 入場前（per-task）へ shift-left する。契約違反（宣言シンボル欠落・shape ずれ）を authoring 直後・per-task で表面化させ、merge 直前まで遅延する手戻りコストを除去する。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D2]
- [GO-02] 新 Phase 3 artifact `task-contract.json` を導入し、タスク → 型カタログ entry の attribution を complete relation として保持する。impl-planner（Phase 3 writer）が本 artifact を author し、どのタスクがどの entry の履行責務を持つかを明記する。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D1]
- [GO-03] ゲートの合否基準は「現在のタスク（in_progress）と完了済タスク（done）に帰属するエントリの impl_catalog 信号が全て 🔵 か」とし、未着手タスク（todo）に帰属するエントリは 🟡 を許容する。🔴 はタスク状態に関わらず常に blocker。body の liveness（`todo!()` でないか等）はゲートの管轄外として reviewer の責務に留置する（ゲート: 構造的型契約履行カバレッジ ⊥ LLM レビュー: body 意味論妥当性、の peer layer 責務分離）。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D7]

## Scope

### In Scope
- [IN-01] 新 Phase 3 artifact `task-contract.json` のスキーマ定義と codec 実装。artifact はタスクリスト（各タスクが履行責務を持つ型カタログ entry 識別子のリスト）と schema_version を含む。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D1] [tasks: T001, T003]
- [IN-02] impl-planner の Phase 3 output 責務拡張: impl-plan.json・task-coverage.json に加えて `task-contract.json` を author する。type-designer（Phase 2）の責務は変更しない。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D1] [tasks: T001]
- [IN-03] attribution の complete relation invariant 検証: (a) 型カタログの全エントリが ≥1 タスクに attribution されていること（completeness）、(b) orphan entry（どのタスクにも attribution されていないカタログ entry）が存在しないこと、(c) attribution された entry がカタログに実在すること（referential integrity）。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D1] [tasks: T002, T009, T010]
- [IN-04] pre-review blocking binary check ゲートの実装（`bin/sotp task-contract check`）: 現在のタスク（in_progress）と完了済タスク（done）に帰属するエントリの impl_catalog 信号を既存信号テーブルと entry キーで JOIN し、全エントリが 🔵 かを確認する。判定対象タスクは `impl-plan.json` のタスク状態を参照して特定する。`--track-id` 引数が省略された場合は現在の git ブランチ (`track/<id>` 形式) から active track を auto-resolve する。ゲートは fail-closed（artifact 不在・非 🔵 エントリ存在のいずれでもブロック）。todo タスクに帰属するエントリは 🔵 必須の対象からは外すが、🔴 はタスク状態に関わらず常にブロックし、🟡 のみ未着手として許容する。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D2, knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D4, knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D7] [tasks: T001, T002, T003, T004, T005, T006, T011, T015]
- [IN-05] review-fix ワークフロー（`sotp review fix-local` / fixpoint_resolve または同等の呼び出し経路）への pre-review ゲートの配線。ゲートは `sotp review fix-local` が reviewer invocation を起動する前に実行され、ゲートがブロックした場合は reviewer invocation が実行されない。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D2] [tasks: T008]
- [IN-07] `bin/sotp task-contract coverage` サブコマンドの実装: 型カタログの全エントリが漏れなくタスクに attribution されているか（完全性）を専用に検証する subcommand。cargo make ci の検証 chain に統合し、commit ごとに attribution drift を検出する。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D5] [tasks: T009, T010, T011, T012, T013, T014, T016]
- [IN-08] cargo-make `dependencies` による pre-review ゲートの配線: `cargo make track-local-review` と `cargo make track-local-review-fix` の両方の `dependencies` に `task-contract-check` タスクを追加し、per-review-round で完全性判定（coverage）→ 生存性判定（check）の順で自動発火させる。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D6] [tasks: T016]
- [IN-09] usecase 層への `ImplPlanReaderPort`（secondary port）追加: `bin/sotp task-contract check` の usecase 層（`PreReviewGateInteractor` 等）に `impl-plan.json` を読む secondary port を追加し、`task-contract.json` の attribution を impl-plan.json のタスク状態（todo / in_progress / done）でフィルタする。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D7] [tasks: T010, T011, T014, T015]
- [IN-10] Claude provider 経由の review-fix dispatch sentinel (exit code 64 + stdout `SUBAGENT_DISPATCH_REQUIRED` + JSON payload) の cross-layer pass-through 機構の実装: usecase 層 `RunReviewFixError` enum に `SubagentDispatchRequired(String)` variant を追加し、payload を typed error として運ぶ。cli_composition 層 shim が composition root から exit 64 + sentinel prefix を検出したとき、`Err(RunReviewFixError::SubagentDispatchRequired(payload))` を return する。cli_driver 層 dispatch arm は payload を `CommandOutcome { stdout: Some(payload), exit_code: 64 }` に reflect して呼び元 (orchestrator) に届ける。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D8] [tasks: T017]

### Out of Scope
- [OS-01] body の liveness / stub 検査（test-pass・stub-scan・syn による body walking）。ゲートの直接入力は既存 impl_catalog / type-signals document であり、上流の impl_catalog 信号は rustdoc JSON（body を含まない）から生成されるため、🔵 判定は宣言シンボルの存在と shape 一致のみを保証する。stub が通過することは意図的なスコープ限定である。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D3]
- [OS-02] 新 SoT Chain（task_conformance 等）の追加。ゲートは binary check として実装し、既存 impl_catalog 信号を再利用する。chain matrix・strictness 設定・views の表面積は増やさない。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D4]
- [OS-03] 型カタログ（`<layer>-types.json`）へのタスク情報の追加。Phase 2 成果物に Phase 3 概念（task）への後方依存を持たせることは SoT Chain 逆流であり禁止。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D1]
- [OS-04] `task-coverage.json`（spec coverage artifact）への attribution 情報の追加。参照 SSoT・供給先ゲート・invariant がいずれも異なる別責務 artifact とし、同居させない。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D1]
- [OS-05] pre-review ゲートの advisory モード・緩和モード（per-task / per-track のブロック回避オプション）。ゲートは blocking かつ fail-closed で実装する。interim 的運用の緩和は本 track 対象外。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D2]
- [OS-06] SignalEvaluatorV2（既存の型カタログ ↔ 実装比較エンジン）の変更。ゲートは本エンジンの評価結果（per-entry impl_catalog 信号）を読み取るだけであり、エンジン自体は変更しない。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D4]

## Constraints
- [CN-01] attribution は型カタログの全エントリに対する complete relation でなければならない。orphan entry（どのタスクにも attribution されていないカタログ entry）が存在する場合、その entry の impl_catalog 信号がゲート入力から漏れ shift-left の保証が崩れるため、ゲートは orphan 検出時に明示的にブロックする。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D1] [tasks: T002, T009, T010]
- [CN-02] task-contract.json が参照するカタログ entry 識別子は現行カタログに実在しなければならない（referential integrity）。存在しない entry への attribution はゲートエラーとし、stale な task-contract.json の検出手段とする。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D1] [tasks: T002, T009, T010]
- [CN-03] ゲートの合否基準は「現在のタスク（in_progress）と完了済タスク（done）に帰属するエントリの impl_catalog 信号が全て 🔵 か」。todo タスクに帰属するエントリは 🟡 を許容する（まだ実装していないため shape mismatch 等は想定範囲内）。🔴 はタスク状態に関わらず常に blocker。body の確認・test 結果・stub 走査は判定に含めない。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D3, knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D7] [tasks: T002, T015]
- [CN-05] pre-review ゲートは既存 impl_catalog 信号テーブルを entry キーで JOIN して動作し、新 chain・新信号エンジン・型カタログ拡張を必要としない。SignalEvaluatorV2 は変更せずに再利用する。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D4] [tasks: T002, T003]
- [CN-06] task-contract.json の author は impl-planner（Phase 3）に限定する。type-designer（Phase 2）は本 artifact を書かない。Phase 2 成果物が Phase 3 概念（task）を参照することは SoT Chain の順序違反であり禁止。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D1] [tasks: T001]
- [CN-07] attribution 完全性（completeness）の検証と生存性（liveness）の検証は別 subcommand に分割する（`bin/sotp task-contract coverage` と `bin/sotp task-contract check`）。両者は失敗時の責任者と修正経路が異なり（planner が attribution を author する vs implementer が impl を 🔵 化する）、1 コマンドに混在させると fixer の判断分岐コストが恒久化する。両者は `task-contract` ドメイン配下に置き、`verify-*` ファミリーには逃さない。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D5] [tasks: T009, T010, T011, T012, T013, T014, T016]
- [CN-08] `cargo make task-contract-check` の `dependencies` に `task-contract-coverage` を宣言することで完全性 → 生存性の実行順を保証する。`bin/sotp` バイナリ内部でのコマンド連結（hardcode）は禁止。各 subcommand は単一責務を維持し、配線は cargo-make 層に局所化する。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D6] [tasks: T016]

## Acceptance Criteria
- [ ] [AC-01] `task-contract.json` のスキーマが定義されており、impl-planner が Phase 3 output として本 artifact を author する。artifact には schema_version・track id・タスクリスト（各タスクが履行責務を持つ型カタログ entry 識別子のリスト）が含まれる。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D1] [tasks: T001, T003]
- [ ] [AC-02] attribution の complete relation invariant が機械的に検証可能である: (a) 型カタログの全エントリが ≥1 タスクに attribution されている、(b) orphan entry が存在しない、(c) attribution 対象 entry がカタログに実在する。invariant 違反時にゲートが非ゼロ終了コードで返り、違反内容（orphan entry リスト・存在しない entry 識別子）を出力する。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D1] [tasks: T002, T007, T009, T010]
- [ ] [AC-03] `bin/sotp task-contract check` が実装されており、`cargo make track-local-review` および `cargo make track-local-review-fix` の `dependencies` に `task-contract-check` タスクが組み込まれている。ゲートは reviewer invocation を起動する前に実行され、fail-closed。`task-contract.json` が不在の場合、ゲートはファイル不在を明示するエラーメッセージで失敗する。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D2, knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D6] [tasks: T004, T005, T006, T008, T016]
- [ ] [AC-04] 現在のタスク（in_progress）と完了済タスク（done）に帰属するエントリの impl_catalog 信号が全て 🔵 のとき、ゲートが unit/binary OK としてパスする。todo タスクに帰属するエントリの 🟡 はパス条件に影響しない。🔴 エントリはタスク状態に関わらず常にゲートをブロックし、非 🔵 エントリの一覧と信号状態を出力する。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D2, knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D7] [tasks: T002, T007, T015]
- [ ] [AC-06] ゲートがブロックした場合、`sotp review fix-local` 経由の reviewer invocation が実行されない。`bin/sotp task-contract coverage` および `bin/sotp task-contract check` はいずれも fail-closed であり、どちらかのゲートエラーが上位ワークフローに伝播して review invocation を中断する。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D2] [tasks: T008, T016]
- [ ] [AC-07] `cargo make ci`（fmt-check + clippy + nextest + deny + check-layers + verify-*）が pass する。ゲートロジックのユニットテストとして次のケースを網羅する: (a) `task-contract.json` 不在で check がブロック、(b) orphan entry 存在で coverage がブロック、(c) attribution referential integrity 違反で coverage がブロック、(d) 🔴 エントリ存在でタスク状態に関わらず check がブロック、(e) in_progress / done タスクの全 attributed entry が 🔵 かつ coverage 完全でパス、(f) todo タスクに 🟡 エントリが存在しても check がパス（🔴 なし前提）、(g) `cargo make track-local-review` 実行時に coverage → check の dependency が順序通り発火する。既存テストへのリグレッションがない。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D6, knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D7] [tasks: T007, T010, T015, T016]
- [ ] [AC-08] `bin/sotp task-contract coverage` が実装されており、`cargo make task-contract-coverage` として呼び出せる。`cargo make task-contract-check` は `dependencies = ["task-contract-coverage"]` を宣言し、coverage → check の順で必ず実行される。attribution 不完全（orphan あり）の場合、coverage がブロックし check は実行されない。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D5, knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D6] [tasks: T009, T010, T011, T012, T013, T014, T016]
- [ ] [AC-09] ユニットテストで以下を検証する: (a) composition root が exit 64 + `SUBAGENT_DISPATCH_REQUIRED` prefix を含む CommandOutcome を return した場合、`ReviewServiceImpl::run_fix_local` は `Err(RunReviewFixError::SubagentDispatchRequired(payload))` を返す (payload は exit 64 出力の stdout 文字列をそのまま carry)、(b) cli_driver の `review_run_fix_local` が `Err(RunReviewFixError::SubagentDispatchRequired(payload))` を受け取ったとき、`CommandOutcome { stdout: Some(payload), stderr: None, exit_code: 64 }` を return し、`REVIEW_FIX_STATUS: failed` への remap が発生しない、(c) Claude provider 経由で `ReviewCompositionRoot::review_driver().handle(RunFixLocal {...})` を呼ぶと exit 64 + sentinel が stdout に preserve される。 [adr: knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md#D8] [tasks: T017]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#No Panics in Library Code
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule
- knowledge/conventions/workflow-ceremony-minimization.md#Rules

## Signal Summary

### Stage 1: Spec Signals
🔵 33  🟡 0  🔴 0

