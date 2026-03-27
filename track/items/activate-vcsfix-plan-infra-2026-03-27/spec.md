<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: draft
version: "1.0.0"
signals: { blue: 10, yellow: 6, red: 0 }
---

# track:activate gitignore 修正 + planner 移譲 cargo make コマンド追加

## Goal

track:activate が gitignored な rendered view (track/registry.md) を git add しようとして失敗するバグを修正する。
planner capability 移譲用の cargo make track-local-plan コマンドを追加し、hook にブロックされずにブリーフィングファイル経由で Codex planner を呼び出せるようにする。

## Scope

### In Scope
- persist_activation_commit() で gitignored パスをフィルタする [source: apps/cli/src/commands/track/activate.rs L271-320] [tasks: T001, T002]
- activation_artifact_paths() から registry.md を削除し spec.md を追加する [source: apps/cli/src/commands/track/activate.rs L528-548] [tasks: T003]
- sync_rendered_views() のドキュメントコメントに VCS フィルタ責任を明示 [source: libs/infrastructure/src/track/render.rs L441] [tasks: T005]
- sotp plan codex-local サブコマンドを review codex-local と同じパターンで実装 [source: apps/cli/src/commands/review/codex_local.rs, discussion] [tasks: T006]
- Makefile.toml + permissions.allow に track-local-plan を登録 [source: discussion] [tasks: T007]
- codex 移譲ドキュメント更新（briefing file パターン正式化） [source: .claude/rules/02-codex-delegation.md, .claude/rules/10-guardrails.md, discussion] [tasks: T008]
- scripts/test_make_wrappers.py に track-local-plan ラッパーの回帰テストケースを追加 [source: inference — existing wrapper test pattern in scripts/test_make_wrappers.py] [tasks: T009]

### Out of Scope
- plan.md / spec.md の gitignore 化（これらは意図的に git-tracked） [source: discussion]
- render.rs の sync_rendered_views() 戻り値のセマンティクス変更 [source: inference — Codex planner recommendation: render.rs should stay VCS-agnostic]
- planner の verdict 解析や auto-record（review 固有機能） [source: inference — planner output is free-form text, not structured verdict]

## Constraints
- 新規ロジックは Rust で実装する [source: feedback — Rust-first policy]
- TDD ワークフロー必須（Red → Green → Refactor） [source: convention — .claude/rules/05-testing.md]
- reviewer review cycle を commit 前に実施する [source: convention — .claude/rules/10-guardrails.md]

## Domain States

| State | Description |
|-------|-------------|
| GitignoredRenderedView | sync_rendered_views() が返すパスのうち .gitignore に登録された rendered view（現在は track/registry.md のみ） |
| TrackedRenderedView | sync_rendered_views() が返すパスのうち git-tracked な rendered view（plan.md, spec.md） |

## Acceptance Criteria
- [ ] track:activate が gitignored な registry.md を含む rendered_paths で成功する [source: inference — primary bug fix validation] [tasks: T001, T002, T004]
- [ ] activation_artifact_paths() が metadata.json, plan.md, spec.md を返し registry.md を含まない [source: apps/cli/src/commands/track/activate.rs L528] [tasks: T003, T004]
- [ ] cargo make track-local-plan -- --briefing-file <path> で Codex planner を hook ブロックなしに呼び出せる（-- セパレータ必須） [source: discussion] [tasks: T006, T007]
- [ ] cargo make ci が通る [source: convention — .claude/rules/07-dev-environment.md] [tasks: T001, T002, T003, T004, T005, T006, T007, T008, T009]

## Related Conventions (Required Reading)
- project-docs/conventions/source-attribution.md
- .claude/rules/02-codex-delegation.md
- .claude/rules/05-testing.md
- .claude/rules/07-dev-environment.md
- .claude/rules/10-guardrails.md
- project-docs/conventions/hexagonal-architecture.md

## Signal Summary

### Stage 1: Spec Signals
🔵 10  🟡 6  🔴 0

