---
adr_id: "2026-07-29-0839-track-id-date-prefix"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:sotohe-issues-discussion:2026-07-29; chat_segment:codex-task-019fb7e8-7803-7b92-b378-dc0d33ae591b:2026-07-31:phase0-boundary-approval「承認します」"
    status: proposed
---
# track id を日付プレフィックス形式に変更する

## Context

track id は現在 `<slug>-<date>` のサフィックス形式（例: `scope-diff-ceiling-admission-enforcement-2026-07-29`）で生成される。一方 ADR ファイル名は `YYYY-MM-DD-HHMM-<slug>` のプレフィックス形式であり、両者の並び順規則が揃っていない。サフィックス形式は `ls` / 一覧表示で時系列に並ばず、track の一覧性・grep 性が低い。

branch 名 `track/<id>`、`track/items/<id>/`、branch-bound な track 解決ロジックはすべて id 文字列に依存しており、命名規則の変更は id 生成箇所（init）に閉じるか、解決ロジックへの波及があるかの確認が必要である。

## Decision

### D1: track id を `<YYYY-MM-DD>-<slug>` プレフィックス形式に変更する

`/track:init` の id 生成を日付プレフィックス形式に変更し、ADR ファイル名と並び順規則を揃える。既存 track の id は改名しない（新規 track からの適用）。

## Rejected Alternatives

### A: 現行サフィックス形式の維持

時系列ソートが効かず、ADR 命名との非対称も残るため却下。

## Consequences

- 良: `track/items/` と branch 一覧が時系列に並ぶ。ADR と同じ規則で grep できる。
- 中立: 新旧形式の track が混在する期間が生じる（解決ロジックは id を不透明文字列として扱っているため動作影響はない想定。確認は track 実装時）。

## Reassess When

- track id に依存する新しい機械処理（並び順に意味を持たせる等）を導入するとき。
