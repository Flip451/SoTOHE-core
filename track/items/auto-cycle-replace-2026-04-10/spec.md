<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: approved
approved_at: "2026-04-10T14:17:24Z"
version: "1.0.0"
---

# full-cycle をタスクごとの implement → review → commit ループに書き換え

## Goal

/track:full-cycle の動作を「全タスク一括実装 + まとめてレビュー」から「タスクごとの implement → review → commit ループ」に変更する。
レビュー差分を小さく保ち、失敗時のロールバックを容易にする。

## Scope

### In Scope
- full-cycle.md のコマンド定義をタスクループ方式に書き換え [source: feedback — user request 2026-04-08] [tasks: T001]
- SKILL.md 内の full-cycle 説明を新セマンティクスに更新 [source: feedback — user request 2026-04-08] [tasks: T002]
- Makefile.toml に cargo make track-signals ラッパーを追加し、track:plan コマンドに track-signals → spec-approve の手順を明記 [source: discussion — planning session 2026-04-10] [tasks: T003]

### Out of Scope
- コマンド名の変更（full-cycle のまま維持） [source: feedback — user feedback 2026-04-10]
- Rust コードの変更 [source: feedback — user request 2026-04-08]
- DEVELOPER_AI_WORKFLOW.md 等の参照更新（既存参照がそのまま有効） [source: discussion — planning session 2026-04-10]

## Constraints
- 既存の activation guard を継承する（branchless planning-only track では実行不可） [source: .claude/commands/track/full-cycle.md]
- transitional compatibility の位置づけを解除し、正式コマンドとする [source: feedback — user request 2026-04-08]

## Acceptance Criteria
- [ ] full-cycle.md がタスクごとの implement（CI・done transition 含む）→ review (zero_findings) → commit ループを定義しており、コミットメッセージはタスク説明から自動生成され、失敗時はループを停止して報告し、transitional compatibility の位置づけが削除され正式コマンドとして記述されている [source: feedback — user request 2026-04-08] [tasks: T001]
- [ ] SKILL.md の full-cycle 参照が新セマンティクスと整合している [source: feedback — user request 2026-04-08] [tasks: T002]
- [ ] Makefile.toml に cargo make track-signals ラッパーが追加され、track:plan コマンドに track-signals → spec-approve の手順が明記されている [source: discussion — planning session 2026-04-10] [tasks: T003]
- [ ] cargo make ci が通過する [source: track/workflow.md] [tasks: T004]

