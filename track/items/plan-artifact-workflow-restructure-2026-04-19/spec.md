<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: approved
approved_at: "2026-04-20T10:59:01Z"
version: "1.0"
signals: { blue: 44, yellow: 0, red: 0 }
---

# 計画成果物ワークフローの再構築 (Scope D: T1 + T2 + T3)

## Goal

README の SoT Chain (ADR ← 仕様書 ← 型契約 ← 実装) に沿って計画成果物のフェーズと責務を分離する
approved / Status / トップレベル content_hash といった形骸化した状態フィールドを廃止する
Phase 3 成果物 (tasks と task_refs) を独立ファイル (impl-plan.json / task-coverage.json) に分離し、spec.json を Phase 3 で書き戻さない構造にする
CI コミットゲートを file 存在 = phase 状態 方式に再定義し、空カタログも有効状態として受け入れる
sources[] 単一 field を 4 独立 ref 構造体 (AdrRef / ConventionRef / SpecRef / InformalGroundRef) + 値オブジェクト newtype 6 種に置き換え、role dispatch を消す。InformalGroundRef により未永続化根拠 (議論 / feedback / memory / user directive) の 🟡 semantics を保持する
task-completion gate (check_tasks_resolved_from_git_ref) を metadata.json 読みから impl-plan.json 読みに切り替える

## Scope

### In Scope
- knowledge/conventions/workflow-ceremony-minimization.md および knowledge/conventions/pre-track-adr-authoring.md を新設する [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D5, knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D4] [tasks: T001]
- CLAUDE.md / DEVELOPER_AI_WORKFLOW.md / track/workflow.md に track 前段階 + 3 フェーズ構成を明記する [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D0.0, knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §移行] [tasks: T001]
- .claude/skills/track-plan/SKILL.md / .claude/commands/track/plan.md / .claude/commands/track/design.md / .claude/agents/planner.md / .claude/agents/designer.md から approved 廃止 + ADR 事前確認 + D1.6 research 配置 convention を反映する [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D1.1, knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D1.6, knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D3.5] [tasks: T001]
- libs/domain/src/plan_ref/ 新モジュール (ref 種別ごとに 1 ファイル: mod.rs + adr_ref.rs + convention_ref.rs + spec_ref.rs + informal_ground_ref.rs) を導入し、値オブジェクト newtype 6 種 (SpecElementId / AdrAnchor / ConventionAnchor / ContentHash / InformalGroundKind / InformalGroundSummary) を配置する [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D2.1, convention — .claude/rules/04-coding-principles.md §Newtype パターン] [tasks: T002]
- libs/domain/src/plan_ref/ に 4 独立 ref 構造体 (AdrRef { file, anchor: AdrAnchor } / ConventionRef { file, anchor: ConventionAnchor } / SpecRef { file, anchor: SpecElementId, hash: ContentHash } / InformalGroundRef { kind: InformalGroundKind, summary: InformalGroundSummary }) を配置する。InformalGroundRef は未永続化根拠 (議論 / feedback / memory / user directive) を構造化して citing し、signal 評価で 🟡 を発火する。共通 trait / enum 抽象化は行わない [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D2.1, knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D2.2] [tasks: T002]
- spec.json schema を刷新する (status / approved_at / トップレベル content_hash / 各要素 task_refs を削除、各要素に id: SpecElementId を必須化、sources を adr_refs + convention_refs + informal_grounds の 3 分割、top-level related_conventions を Vec<ConventionRef> に) [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D1.2, knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D2.1, knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §Q13] [tasks: T003]
- impl-plan.json schema (ImplPlanDocument = schema_version + tasks + plan) と task-coverage.json schema (TaskCoverageDocument = 4 セクションごとの task_refs) を新設する [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D1.4] [tasks: T004]
- 型カタログエントリに spec_refs: Vec<SpecRef> + informal_grounds: Vec<InformalGroundRef> field を追加する [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D1.3, knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D2.1] [tasks: T005]
- metadata.json を identity-only に縮小する (tasks / plan を削除、既存 v3 との並立戦略を含む) [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D1.4, knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D6.1] [tasks: T005]
- verify-track-metadata-local / verify-latest-track-local を file 存在ベースに改訂する (metadata = identity のみ、latest-track = impl-plan.json 存在条件で task 項目チェック) [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D6.1] [tasks: T006]
- sotp track type-signals / baseline-capture / 関連 verify から空カタログ拒否ロジックを撤廃し、エントリ 0 件の空カタログを有効状態として受け入れる [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D6.4] [tasks: T006]
- libs/usecase/src/task_completion.rs の check_tasks_resolved_from_git_ref を metadata.json 読みから impl-plan.json 読みに切り替え、関連する TrackBlobReader port および apps/cli/src/commands/pr.rs 呼び出し側を改修する [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D6.2, knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D9] [tasks: T007]
- libs/infrastructure/src/track/render.rs の plan.md / spec.md renderer を集約形式 (plan.md = metadata.json + impl-plan.json / spec.md = spec.json + task-coverage.json) に変更する [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §移行] [tasks: T008]
- cargo make spec-approve を廃止し、関連 CLI コマンド・spec-signals ツール・spec schema 参照コードから status / approved_at / content_hash 依存を除去する [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D1.2, knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D3] [tasks: T009]
- sotp verify plan-artifact-refs CLI を新設し、ref field (adr_refs / convention_refs / spec_refs / related_conventions / informal_grounds) の schema 検証 + file 存在チェック (file ベース ref のみ) + SpecRef.anchor 解決 + SpecRef.hash 照合 + AdrAnchor / ConventionAnchor の loose validation + InformalGroundRef の newtype validation (kind variant / summary 非空) を実装する [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D2.3] [tasks: T010]
- sotp verify plan-artifact-refs に task-coverage.json 突合検証 (coverage 強制 + referential integrity、現行 spec_coverage::verify 踏襲) + canonical block 疑惑検出 (警告のみ) を追加し、cargo make ci に組み込む [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D2.3, knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D3.3] [tasks: T011]
- 既存 libs/infrastructure/src/verify/spec_coverage.rs の spec_coverage::verify ロジックを新 CLI に統合し、旧呼び出し経路を新 CLI 経由に移行して不要コードを削除する [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D2.3, knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D3.3] [tasks: T012]

### Out of Scope
- 独立 phase コマンド /track:init / /track:spec / /track:impl-plan の新設 (T5 / T7 の別 track) [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D0.0, knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §展開フェーズ 4, knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §展開フェーズ 6]
- /track:design 責務刷新 (Phase 2 専任化、T6 別 track) [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §展開フェーズ 5]
- /track:plan orchestrator 再定義 + adr-editor capability 新設 (T7.5 別 track) [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §コマンド境界, knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §展開フェーズ 6.5]
- catalogue-signal (型契約 → 仕様書 signal) の実装 (別 ADR、tddd-ci-gate-and-signals-separation 系) [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D3.2]
- canonical block / ## Canonical Blocks セクションの最終形決定 (Q14 別 ADR) [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §Q14]
- AdrAnchor / ConventionAnchor の semantic 厳密化 + AdrRef.hash / ConventionRef.hash 追加 (Q15 別 ADR、本 track では loose validation のみ) [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §Q15]
- writer 境界の自動強制 (Q16 後続、本 track では subagent プロンプト + 人手レビュー運用) [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §Q16]
- hook 強制 (T8 別 track、展開フェーズ 7) [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §展開フェーズ 7]

## Constraints
- 既存の完成済みおよび稼働中 track には遡及適用しない。新旧 2 系統が一時的に並立する [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §移行]
- 後方互換のための Option<> / nullable required 混在は作らない (feedback_no_backward_compat) [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D2.1, memory — feedback_no_backward_compat]
- ルールを書くだけでは乖離は止まらない前提で、CI とスキーマで構造的に強制する (feedback_enforce_by_mechanism) [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §3 原則 6, memory — feedback_enforce_by_mechanism]
- SoT Chain 方向性 (下流 → 上流の一方向依存) を schema で enforce し、逆流を型レベルで不可能にする (3 独立 ref 構造体 + field 分割) [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D1.5, knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D2.1]
- 値オブジェクト newtype のコンストラクタに validation を閉じ込め、String で持ちまわらない (Rust 慣用の newtype パターン) [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D2.1, convention — .claude/rules/04-coding-principles.md §Newtype パターン]

## Acceptance Criteria
- [ ] cargo make ci が新スキーマで pass する (fmt-check + clippy + nextest + test-doc + deny + check-layers + verify-* 一式) [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §利点, convention — .claude/rules/07-dev-environment.md §Pre-commit Checklist] [tasks: T001, T002, T003, T004, T005, T006, T007, T008, T009, T010, T011, T012]
- [ ] 新規 spec.json の各要素に id: SpecElementId が必須化され、schema validator と codec round-trip が pass する [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D1.2, knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §Q13] [tasks: T003]
- [ ] 値オブジェクト newtype 6 種 (SpecElementId / AdrAnchor / ConventionAnchor / ContentHash / InformalGroundKind / InformalGroundSummary) のコンストラクタに validation が閉じ込められ、使用サイト (codec / verify / signal) に Option<String> が露出しない [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D2.1] [tasks: T002, T003, T005, T010]
- [ ] 型カタログエントリの spec_refs[] 各要素が SpecRef { file, anchor: SpecElementId, hash: ContentHash } 形式、informal_grounds[] 各要素が InformalGroundRef { kind, summary } 形式で保持され、codec で round-trip する [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D1.3, knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D2.1, knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D2.2] [tasks: T005]
- [ ] spec.json の各要素の informal_grounds[] 非空で、仕様書 → ADR signal (D3.1) が 🟡 を発火する (未永続化根拠の存在を表し、merge 前に formal ref へ昇格要)。spec-signals ツールが adr_refs と informal_grounds を合成して総合 signal を算出する。型カタログエントリの informal_grounds[] による catalogue 側 signal (D3.2) の実装は後続 ADR が担う (本 track は catalogue schema field 追加のみ) [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D3.1, knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D3.2] [tasks: T003]
- [ ] sotp verify plan-artifact-refs が ref field の schema 検証 + file 存在 + SpecRef.anchor 解決 + SpecRef.hash 照合 + AdrAnchor / ConventionAnchor の loose validation + InformalGroundRef (kind / summary) の newtype validation をすべて実行し、違反時に CI fail する [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D2.3] [tasks: T010]
- [ ] sotp verify plan-artifact-refs が task-coverage.json の coverage 強制 + referential integrity 検査を実行し、既存 spec_coverage::verify と同等の判定結果を返す [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D2.3, knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D3.3] [tasks: T011, T012]
- [ ] check_tasks_resolved_from_git_ref が impl-plan.json を読んで全 task resolved (DonePending / DoneTraced / Skipped) 判定する。K1-K7 MockReader tests が新 port で pass する [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D6.2, knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md §D9] [tasks: T007]
- [ ] verify-track-metadata-local が identity field のみ検証し、verify-latest-track-local が impl-plan.json 存在条件で task 項目チェックを条件分岐する [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D6.1] [tasks: T006]
- [ ] エントリ 0 件の空の型カタログが sotp track type-signals / baseline-capture / verify-*-local で pass する [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D6.4] [tasks: T006]
- [ ] cargo make spec-approve が廃止され、approved 概念が消滅する。spec-signals 等の関連コードが status / approved_at / content_hash 依存なしで動作する [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D1.2, knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D3] [tasks: T009]
- [ ] plan.md が metadata.json + impl-plan.json の集約から、spec.md が spec.json + task-coverage.json の集約から render される [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §移行] [tasks: T008]
- [ ] convention 2 本 (workflow-ceremony-minimization.md / pre-track-adr-authoring.md) が新規作成され、knowledge/conventions/ に存在する [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D4, knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D5] [tasks: T001]
- [ ] SKILL / command / agent (planner, designer) から approved 状態依存が除去され、ADR 事前確認 + D1.6 research 配置 convention が反映される [source: knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §移行, knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D1.1, knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md §D1.6] [tasks: T001]

## Related Conventions (Required Reading)
- knowledge/conventions/source-attribution.md
- knowledge/conventions/hexagonal-architecture.md
- knowledge/conventions/security.md
- .claude/rules/04-coding-principles.md
- .claude/rules/05-testing.md
- .claude/rules/07-dev-environment.md
- .claude/rules/08-orchestration.md
- .claude/rules/09-maintainer-checklist.md
- .claude/rules/10-guardrails.md

## Signal Summary

### Stage 1: Spec Signals
🔵 44  🟡 0  🔴 0

