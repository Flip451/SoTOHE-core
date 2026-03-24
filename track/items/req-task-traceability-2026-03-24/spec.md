<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: draft
version: "1.0"
---

# CC-SDD-01 要件-タスク双方向トレーサビリティ

## Goal

spec.json の全要件項目 (scope, constraints, acceptance_criteria) に task_refs フィールドを追加し、metadata.json のタスクと双方向に紐付ける。
CI ゲート (sotp verify spec-coverage) は in_scope と acceptance_criteria の紐付き漏れを検出する。constraints の紐付きは任意 (CI error にしない)。

## Scope

### In Scope
- SpecRequirement に task_refs: Vec<TaskId> フィールドを追加 [source: discussion — 2026-03-24 CC-SDD-01 計画]
- SpecDocument::evaluate_coverage() で in_scope + acceptance_criteria の task_refs 紐付き検証 [source: knowledge/adr/2026-03-24-0900-coverage-not-a-signal.md]
- spec/codec.rs の DTO に task_refs を追加し永続化対応 [source: discussion — 2026-03-24 CC-SDD-01 計画]
- spec/render.rs で task_refs を spec.md に表示 [source: discussion — 2026-03-24 CC-SDD-01 計画]
- verify/spec_coverage.rs で CI ゲートを新規実装 [source: knowledge/adr/2026-03-24-0900-coverage-not-a-signal.md]
- sotp verify spec-coverage CLI コマンド追加 [source: discussion — 2026-03-24 CC-SDD-01 計画]
- Makefile.toml の CI gate に verify-spec-coverage を追加 [source: discussion — 2026-03-24 CC-SDD-01 計画]
- CI gate は最新アクティブ track の spec.json のみ検証 (verify-latest-track と同スコープ)。completed tracks の retroactive 更新は不要 [source: inference — verify-latest-track との一貫性]
- /track:plan skill の spec.json 生成テンプレートに task_refs フィールドを追加し、新規 track が CI gate を通過するようにする [source: discussion — 2026-03-24 CC-SDD-01 計画]

### Out of Scope
- completed tracks の spec.json への retroactive task_refs 追加 [source: inference — historical artifacts are immutable records]
- constraints と out_of_scope の coverage 強制 (CI エラーにはしない) [source: discussion — 2026-03-24 CC-SDD-01 計画]
- task_refs からの逆引き orphan task 検出は warning のみ (error にしない) [source: discussion — 2026-03-24 CC-SDD-01 計画]
- coverage を ConfidenceSignal に統合 (ADR で却下済み) [source: knowledge/adr/2026-03-24-0900-coverage-not-a-signal.md]

## Constraints
- domain 層は I/O を含まない (hexagonal purity) [source: convention — project-docs/conventions/hexagonal-architecture.md]
- coverage は信号機 (ConfidenceSignal) に組み込まず CI ゲートとして実装する [source: knowledge/adr/2026-03-24-0900-coverage-not-a-signal.md]
- 逆方向 (task → requirements) は task_refs から計算で導出し、冗長な二重管理を避ける [source: discussion — 2026-03-24 CC-SDD-01 計画]
- 既存の SpecRequirement コンストラクタ互換性を維持する (task_refs は optional) [source: inference — backward compatibility]
- TDD (Red-Green-Refactor) に従う [source: convention — .claude/rules/05-testing.md]

## Domain States

| State | Description |
|-------|-------------|
| CoverageResult | coverage 検証の結果。covered/uncovered/invalid_refs を保持する値オブジェクト |

## Acceptance Criteria
- [ ] SpecRequirement に task_refs フィールドが追加され、空配列がデフォルトとなる [source: discussion — 2026-03-24 CC-SDD-01 計画]
- [ ] spec.json で task_refs を JSON 配列として永続化・復元できる [source: discussion — 2026-03-24 CC-SDD-01 計画]
- [ ] spec.md のレンダリングに task_refs が表示される [source: discussion — 2026-03-24 CC-SDD-01 計画]
- [ ] sotp verify spec-coverage が in_scope + acceptance_criteria の task_refs 未設定を検出する [source: knowledge/adr/2026-03-24-0900-coverage-not-a-signal.md]
- [ ] sotp verify spec-coverage が metadata.json に存在しない TaskId への参照を検出する [source: inference — referential integrity]
- [ ] sotp verify spec-coverage が constraints の task_refs 未設定を CI エラーにしない (warning のみ) [source: discussion — 2026-03-24 CC-SDD-01 計画]
- [ ] /track:plan skill が新規 spec.json 生成時に task_refs フィールドを含める [source: discussion — 2026-03-24 CC-SDD-01 計画]
- [ ] cargo make ci に verify-spec-coverage が含まれ全テスト通過する [source: convention — .claude/rules/07-dev-environment.md]

## Related Conventions (Required Reading)
- project-docs/conventions/hexagonal-architecture.md
- project-docs/conventions/source-attribution.md
- project-docs/conventions/typed-deserialization.md
- .claude/rules/05-testing.md
- .claude/rules/07-dev-environment.md

