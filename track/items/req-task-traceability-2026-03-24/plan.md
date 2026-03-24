<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# CC-SDD-01 要件-タスク双方向トレーサビリティ

spec.json の要件項目 (scope.in_scope, constraints, acceptance_criteria) と metadata.json のタスクを双方向に紐付ける。
ADR 決定に基づき coverage は信号機ではなく CI ゲート (sotp verify spec-coverage) として実装する。
spec.json 側の SpecRequirement に task_refs フィールドを追加し、逆方向は計算で導出する。

## Domain: task_refs + coverage 計算

SpecRequirement に task_refs: Vec<TaskId> を追加。
CoverageResult 型と SpecDocument::evaluate_coverage() で紐付き検証ロジックを提供。

- [ ] domain: SpecRequirement に task_refs: Vec<TaskId> 追加 + テスト
- [ ] domain: CoverageResult 型 + SpecDocument::evaluate_coverage() + テスト

## Infrastructure: codec + render + CI ゲート

spec/codec.rs の DTO に task_refs を追加し永続化対応。
spec/render.rs で task_refs を spec.md に表示。
verify/spec_coverage.rs で CI ゲートを実装。

- [ ] infra: spec/codec.rs の DTO に task_refs 追加 + round-trip テスト
- [ ] infra: spec/render.rs に task_refs 表示追加 + テスト
- [ ] infra: verify/spec_coverage.rs CI ゲート実装 + テスト

## CLI + CI 統合

sotp verify spec-coverage CLI コマンドを追加。
Makefile.toml の CI gate に組み込み (最新アクティブ track のみ検証)。
/track:plan skill の spec.json テンプレートに task_refs を追加。

- [ ] CLI: sotp verify spec-coverage コマンド追加
- [ ] Makefile: verify-spec-coverage を CI gate に追加 + /track:plan skill の spec.json 生成に task_refs を含める

## 統合テスト + ドキュメント

end-to-end テストで coverage 検証が動作することを確認。
DESIGN.md, TRACK_TRACEABILITY.md を更新。

- [ ] 統合テスト + ドキュメント更新 (DESIGN.md, TRACK_TRACEABILITY.md)
