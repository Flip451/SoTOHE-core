<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: draft
version: "1.0"
signals: { blue: 21, yellow: 0, red: 0 }
---

# auto-record review.json 書き込みパス接続 + sandbox 修正

## Goal

前トラック (review-json-per-group-review) で構築済みの review.json domain model/infrastructure を record-round と check-approved の production code path に接続し、review state が review.json に正しく永続化されるようにする。
Codex CLI の --full-auto が --sandbox workspace-write を暗黙適用する問題を修正し、planner/reviewer が常に read-only sandbox で実行されることを保証する。

## Scope

### In Scope
- RecordRoundProtocolImpl を FsReviewJsonStore 経由で review.json に書くよう書き換える [source: track/items/review-json-per-group-review-2026-03-29/spec.md §Acceptance Criteria] [tasks: T002]
- check-approved を ReviewJsonReader 経由で review.json から読むよう移行する [source: track/items/review-json-per-group-review-2026-03-29/spec.md §Acceptance Criteria] [tasks: T003]
- metadata.json の review セクションへの書き込みを停止する [source: track/items/review-json-per-group-review-2026-03-29/spec.md §Acceptance Criteria] [tasks: T002, T004]
- stage_bytes に --add フラグ追加（将来用）。review.json は review_operational ファイルとして cargo make add-all でコミット時に staging する設計 [source: knowledge/adr/2026-03-29-0947-review-json-per-group-review-state.md] [tasks: T004]
- planner/reviewer の Codex 呼び出しから --full-auto を除去し --sandbox read-only を保証する [source: .claude/rules/02-codex-delegation.md §Sandbox and Hook Coverage Warning] [tasks: T001]
- per-group scope hash の実装: record-round で group の frozen scope ファイル群から review-scope manifest hash を計算し review.json に記録する [source: knowledge/adr/2026-03-29-0947-review-json-per-group-review-state.md §5] [tasks: T005]
- check-approved で各 group の latest round hash を current group-scope hash と照合し、hash mismatch を検出する [source: knowledge/adr/2026-03-29-0947-review-json-per-group-review-state.md §5] [tasks: T006]

### Out of Scope
- resolve-escalation の review.json 移行（別トラックで対応） [source: knowledge/adr/2026-03-29-0947-review-json-per-group-review-state.md]
- 旧トラックの metadata.json review state → review.json 自動マイグレーション [source: track/items/review-json-per-group-review-2026-03-29/spec.md §Out of Scope]

## Constraints
- 各タスクの diff は 500 行以下に抑える [source: feedback — small review surface policy]
- TDD ワークフロー必須 [source: convention — .claude/rules/05-testing.md]
- hexagonal architecture: domain は infrastructure/cli に依存しない [source: convention — project-docs/conventions/hexagonal-architecture.md]
- 新規トラック専用。旧トラックの後方互換は不要 [source: track/items/review-json-per-group-review-2026-03-29/spec.md §Out of Scope]

## Domain States

| State | Description |
|-------|-------------|
| broken | record-round が metadata.json に書き、status が review.json を読む不整合状態（現状） |
| wired_placeholder | record-round と check-approved が review.json を読み書きするが、hash は placeholder (T001-T004) |
| wired_complete | per-group scope hash が ADR §5 準拠で計算・検証される完全状態 (T005-T006) |

## Acceptance Criteria
- [ ] record-round 実行後に track/items/<id>/review.json が作成・更新される [source: track/items/review-json-per-group-review-2026-03-29/spec.md §Acceptance Criteria] [tasks: T002]
- [ ] sotp review status が review.json から正しい review state を表示する（NotStarted ではなく実際の状態） [source: track/items/review-json-per-group-review-2026-03-29/spec.md §Acceptance Criteria] [tasks: T002]
- [ ] check-approved が review.json の cycle/group state から承認判定を行う [source: track/items/review-json-per-group-review-2026-03-29/spec.md §Acceptance Criteria] [tasks: T003]
- [ ] metadata.json に review セクションが書き込まれない [source: track/items/review-json-per-group-review-2026-03-29/spec.md §Acceptance Criteria] [tasks: T004]
- [ ] planner/reviewer の Codex 呼び出しに --full-auto が含まれず、--sandbox read-only が使用される [source: .claude/rules/02-codex-delegation.md §Sandbox and Hook Coverage Warning] [tasks: T001]
- [ ] cargo make ci が全チェック通過する [source: convention — .claude/rules/10-guardrails.md] [tasks: T004]
- [ ] record-round が group の frozen scope ファイル群から計算した per-group hash を review.json に記録する [source: knowledge/adr/2026-03-29-0947-review-json-per-group-review-state.md §5] [tasks: T005]
- [ ] check-approved が各 group の latest round hash を current scope hash と照合し、stale な場合にブロックする [source: knowledge/adr/2026-03-29-0947-review-json-per-group-review-state.md §5] [tasks: T006]

## Related Conventions (Required Reading)
- project-docs/conventions/hexagonal-architecture.md
- .claude/rules/05-testing.md
- .claude/rules/10-guardrails.md
- .claude/rules/02-codex-delegation.md

## Signal Summary

### Stage 1: Spec Signals
🔵 21  🟡 0  🔴 0

