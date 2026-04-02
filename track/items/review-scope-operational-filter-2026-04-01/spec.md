<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: draft
version: "1.0"
signals: { blue: 7, yellow: 6, red: 0 }
---

# review_operational パターンによる review.json scope 除外

## Goal

review-scope.json の review_operational パターンを実行時に展開・適用し、review.json 等の operational ファイルを review scope から除外する。
これにより multi-group レビューの check-approved が安定動作し、review.json 変更による hash 循環を解消する。

## Scope

### In Scope
- review_operational パターンのローディングと <track-id> placeholder 展開 [source: track/review-scope.json §review_operational] [tasks: T001]
- partition 前の pre-filter: operational パターンにマッチするパスを diff リストから除外 [source: inference — partition() は変更せず前段でフィルタ] [tasks: T002]
- execute() の2箇所（新規 cycle 作成 + 既存 cycle 追記）+ check-approved の1箇所への適用 [source: libs/infrastructure/src/review_adapters.rs §execute, apps/cli/src/commands/review/mod.rs §run_check_approved] [tasks: T002]
- 回帰テスト: review.json が frozen scope に含まれないこと、連続 record-round で hash が安定すること [source: discussion] [tasks: T003, T004]

### Out of Scope
- partition() の変更（既存のグループ分類ロジックは変更しない） [source: inference — 既存テスト安定性]
- RVW-34 (StoredFinding lossy conversion) の修正 [source: knowledge/strategy/TODO.md §RVW-34]

## Constraints
- review_adapters.rs が 994 行のため、新ロジックは別モジュールに配置 [source: architecture-rules.json §module_limits]
- TDD: テストを先に書く [source: convention — .claude/rules/05-testing.md]
- パニック禁止 [source: convention — .claude/rules/04-coding-principles.md]

## Acceptance Criteria
- [ ] review.json が other グループの frozen scope に含まれない [source: discussion] [tasks: T003]
- [ ] 連続 record-round 後も other グループの hash が変化しない（review.json 変更の影響を受けない） [source: discussion] [tasks: T004]
- [ ] 既存の partition() テストが全てパスする [source: inference — 既存テスト安定性] [tasks: T002]
- [ ] cargo make ci が通る [source: convention — .claude/rules/07-dev-environment.md] [tasks: T001, T002, T003, T004]

## Related Conventions (Required Reading)
- knowledge/conventions/hexagonal-architecture.md

## Signal Summary

### Stage 1: Spec Signals
🔵 7  🟡 6  🔴 0

