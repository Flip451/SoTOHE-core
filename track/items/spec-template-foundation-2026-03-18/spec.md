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

- テストファイル削除ブロック hook の Rust 実装 [source: tmp/adoption-candidates-2026-03-17.md #5 (SURVEY-03)]
- metadata.json タスク説明の immutable 化を save 経路で有効化 [source: tmp/adoption-candidates-2026-03-17.md #14 (SURVEY-10)]
- spec.md テンプレートへのソース帰属 `[source: ...]` タグ追加 [source: tmp/adoption-candidates-2026-03-17.md #6 (TSUMIKI-02)]
- Spec YAML frontmatter の完了確認 + signals optional field 追加 [source: tmp/adoption-candidates-2026-03-17.md #23 (SURVEY-16)]

### Out of Scope

- 信号機評価 🔵🟡🔴 の実装 [source: tmp/TODO-PLAN-2026-03-17.md Phase 2]
- 要件-タスク双方向トレーサビリティ [source: tmp/TODO-PLAN-2026-03-17.md Phase 2]
- cross-artifact 整合性分析 [source: tmp/TODO-PLAN-2026-03-17.md Phase 3]

## Constraints

- 新規ロジックはすべて Rust で実装する（Python 不可）[source: feedback — Rust-first policy]
- TDD で進める（Red → Green → Refactor）[source: convention — .claude/rules/05-testing.md]
- 既存の `sotp hook dispatch` パターンに従う [source: libs/domain/src/guard/]
- `cargo make ci` が通ること [source: track/workflow.md]

## Acceptance Criteria

- [ ] `sotp hook dispatch block-test-file-deletion` が `*_test.rs` / `tests/**/*.rs` の削除をブロックする [source: tmp/adoption-candidates-2026-03-17.md #5]
- [ ] `FsTrackStore::save()` がタスク説明の変更を `ValidationError` で拒否する [source: tmp/adoption-candidates-2026-03-17.md #14]
- [ ] `track-plan SKILL.md` の spec.md テンプレートに `[source: ...]` タグが含まれる [source: tmp/adoption-candidates-2026-03-17.md #6]
- [ ] `project-docs/conventions/source-attribution.md` が追加されている [source: tmp/adoption-candidates-2026-03-17.md #6]
- [ ] frontmatter パーサーが `signals` フィールドを optional で受け付ける [source: tmp/adoption-candidates-2026-03-17.md #23]
- [ ] TODO-PLAN 1-8 が完了マークされている [source: tmp/TODO-PLAN-2026-03-17.md Phase 1]
- [ ] 全テストが通り `cargo make ci` が成功する [source: track/workflow.md]
