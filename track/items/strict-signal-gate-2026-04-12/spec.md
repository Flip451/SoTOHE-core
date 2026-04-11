<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: approved
approved_at: "2026-04-11T22:08:44Z"
version: "1.0.0"
signals: { blue: 1, yellow: 4, red: 0 }
---

# Strict spec signal gate — Yellow blocks merge

## Goal

verify spec-states --strict を track-pr-merge に配線し、spec signals に Yellow が残る PR のマージを阻止する。

## Scope

### In Scope
- wait-and-merge のタスク完了ガード直後に verify spec-states --strict を呼び出す配線追加 [source: discussion] [tasks: T001]

### Out of Scope
- verify spec-states --strict の実装自体 (既に完了済み) [source: inference — libs/infrastructure/src/verify/spec_states.rs に実装済み]

## Constraints
- 既存の --strict 実装をそのまま使い、新規ロジックは追加しない [source: discussion]

## Acceptance Criteria
- [ ] spec signals に Yellow がある状態で track-pr-merge を実行するとマージが阻止されること [source: discussion] [tasks: T001]
- [ ] cargo make ci が通ること [source: convention — .claude/rules/07-dev-environment.md] [tasks: T001]

## Signal Summary

### Stage 1: Spec Signals
🔵 1  🟡 4  🔴 0

