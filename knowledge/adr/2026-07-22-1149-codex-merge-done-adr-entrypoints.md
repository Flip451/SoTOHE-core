---
adr_id: "2026-07-22-1149-codex-merge-done-adr-entrypoints"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:adr-add-hearing:2026-07-22"
    candidate_selection: "from:[D1,A,B] chose:D1"
    status: proposed
---
# Codex 正規入口の整備

## Context

`.agents/skills` に codex 向けのスキルが定義されているが、SoTOHE の運用の入口と出口を担う重要コマンドが定義されていないままであり、codex をホストに据えて SoTOHE のワークフローを正規 $ コマンドのみで走り切ることができない。これを解消する必要がある。

## Decision

### D1: .agents/skills に track-merge / track-done / adr-add の 3 skill を新設する

codex 上で `$track-merge`、`$track-done`、`$adr-add` を呼び出せる正規の入口として、`.agents/skills/` 配下に `track-merge` / `track-done` / `adr-add` の 3 skill を新設する。

## Rejected Alternatives

### A. 自然言語指示で代用する（skill を作らない）

codex 上では自然言語指示で merge / done / ADR 起草を依頼する運用で代用する案。手順の再現性がなく、guardrail（guarded コマンド経由の強制）を取りこぼすため却下。

### B. merge / done を adr2pr に統合して自動化する

track 終端処理を adr2pr パイプラインへ統合し、専用入口を不要にする案。merge は user の明示操作とする既存方針（adr2pr は PR 作成で停止する）に反するため却下。

## Consequences

### Positive

- codex ホストで SoTOHE ワークフローを入口（adr-add）から出口（merge / done）まで正規 $ コマンドのみで完走できる。
- Claude / Codex 間のホスト対等性が向上する。

### Negative

- ホストごとの薄い入口ファイル（adapter）は増えるが、workflow ロジックは `.harness` 配下の SSoT に集約されるためロジックの二重メンテナンスは発生しない。ただし workflow SSoT 未整備のコマンドについては SSoT 側の整備が先行して必要になる可能性がある。

## Reassess When

- `.harness/workflows/` に対応 workflow SSoT が新設・変更されたとき
- codex 側 skill 定義方式（`.agents/skills` の仕様）が変わったとき
- orchestrator provider 構成（`.harness/config/agent-profiles.json`）の前提が変わったとき

## Related

- `knowledge/adr/` — ADR 索引
- `.harness/workflows/track/` — workflow SSoT
- `.agents/skills/` — codex 向け skill 群
- `.harness/config/agent-profiles.json` — capability → provider routing SSoT
