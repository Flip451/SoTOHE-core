<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: draft
version: "1.0.0"
---

# RVW-13/15/17 Review infrastructure quality hardening

## Goal

レビュー基盤の品質を実運用レベルに引き上げる。
GitDiffScopeProvider のアダプタ契約テストを追加し、--auto-record フローを実戦検証し、codex-reviewer agent のツール制限を確認する。

## Scope

### In Scope
- GitDiffScopeProvider の tempdir git fixture テスト（merge-base, staged, unstaged, untracked, rename, delete, error propagation） [source: knowledge/strategy/TODO.md §RVW-15] [tasks: T001]
- codex-reviewer agent の tools: frontmatter 制限が機能するか検証。機能しない場合は代替策を実装 [source: knowledge/strategy/TODO.md §RVW-17, knowledge/strategy/TODO.md §RVW-18] [tasks: T002]
- /track:review で --auto-record フラグを常時使用するよう移行。このトラック自身のレビューで実戦検証 [source: knowledge/strategy/TODO.md §RVW-13] [tasks: T003]
- /track:plan skill の spec.json 生成後に sotp track signals を自動実行して signals フィールドを埋める [source: inference — signal system was implemented but not integrated into /track:plan flow] [tasks: T004]

### Out of Scope
- path normalization 改善 (RVW-14) — 別トラック [source: knowledge/strategy/TODO.md §RVW-14]
- escalation block exit 3 統合テスト (RVW-16) — 手動検証で代替可能 [source: knowledge/strategy/TODO.md §RVW-16]

## Constraints
- 新規ロジックは Rust で実装（Python 禁止） [source: feedback — Rust-first policy]
- TDD: テストを先に書く (Red → Green → Refactor) [source: convention — .claude/rules/05-testing.md]
- T003 はこのトラック自身のレビューサイクルで --auto-record を使用して検証する [source: inference — eat your own dog food]

## Acceptance Criteria
- [ ] GitDiffScopeProvider の tempdir テストが merge-base/staged/unstaged/untracked/rename/delete の全ケースをカバー [source: knowledge/strategy/TODO.md §RVW-15] [tasks: T001]
- [ ] git コマンド失敗時に DiffScopeProviderError が返される（空スコープではない）ことをテストで検証 [source: knowledge/strategy/TODO.md §RVW-15] [tasks: T001]
- [ ] codex-reviewer agent の tools: 制限動作が確認済み、または代替策が実装済み [source: knowledge/strategy/TODO.md §RVW-17] [tasks: T002]
- [ ] このトラックの /track:review が --auto-record フラグ付きで正常完了 [source: knowledge/strategy/TODO.md §RVW-13] [tasks: T003]
- [ ] /track:plan で作成した spec.json の signals フィールドが null ではなく評価済みの値を持つ [source: inference — signal system integration] [tasks: T004]
- [ ] cargo make ci が通る [source: convention — .claude/rules/07-dev-environment.md] [tasks: T001, T002, T003, T004]

## Related Conventions (Required Reading)
- project-docs/conventions/hexagonal-architecture.md
- project-docs/conventions/source-attribution.md
- project-docs/conventions/task-completion-flow.md
- .claude/rules/05-testing.md
- .claude/rules/07-dev-environment.md

