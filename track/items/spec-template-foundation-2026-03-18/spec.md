---
status: draft
version: "1.0"
---

# Spec: Phase 1 残り — spec.md テンプレート基盤整備

## Goal

Phase 1 の残り 4 項目（SURVEY-03, SURVEY-10, TSUMIKI-02, SURVEY-16）を完了し、
Phase 2（仕様品質の制度化）の受け皿となる spec.md テンプレート基盤を整備する。

## Scope

### In Scope

- テストファイル削除ブロック hook の Rust 実装 (SURVEY-03/#5)
- metadata.json タスク説明の immutable 化を save 経路で有効化 (SURVEY-10/#14)
- spec.md テンプレートへのソース帰属 `[source: ...]` タグ追加 (TSUMIKI-02/#6)
- Spec YAML frontmatter の完了確認 + signals optional field 追加 (SURVEY-16/#23)

### Out of Scope

- 信号機評価 🔵🟡🔴 の実装（Phase 2 TSUMIKI-01）
- 要件-タスク双方向トレーサビリティ（Phase 2 CC-SDD-01）
- cross-artifact 整合性分析（Phase 3）

## Constraints

- 新規ロジックはすべて Rust で実装する（Python 不可）[source: feedback — Rust-first policy]
- TDD で進める（Red → Green → Refactor）[source: project convention]
- 既存の `sotp hook dispatch` パターンに従う [source: libs/domain/src/guard/]
- `cargo make ci` が通ること [source: track/workflow.md]

## Acceptance Criteria

- [ ] `sotp hook dispatch block-test-deletion` が `*_test.rs` / `tests/**/*.rs` の削除をブロックする
- [ ] `FsTrackStore::save()` がタスク説明の変更を `ValidationError` で拒否する
- [ ] `track-plan SKILL.md` の spec.md テンプレートに `[source: ...]` タグが含まれる
- [ ] `project-docs/conventions/source-attribution.md` が追加されている
- [ ] frontmatter パーサーが `signals` フィールドを optional で受け付ける
- [ ] TODO-PLAN 1-8 が完了マークされている
- [ ] 全テストが通り `cargo make ci` が成功する
