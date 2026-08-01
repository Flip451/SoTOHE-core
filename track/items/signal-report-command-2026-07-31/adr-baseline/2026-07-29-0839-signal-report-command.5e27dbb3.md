---
adr_id: "2026-07-29-0839-signal-report-command"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:sotohe-issues-discussion:2026-07-29"
    status: proposed
---
# 信号機 Yellow/Red 内訳を横断列挙する signal report コマンドを追加する

## Context

`sotp signal` は `calc-*`（signals の計算・persist）と `check-*` / `check`（gate 合否と strictness 解決）を提供するが、🟡 / 🔴 が**どこで**出ているかを列挙する手段がない。内訳を知るには persist された `*-signals.json` / `*-catalogue-spec-signals.json` を jq で直接読むしかなく、gate が block した際の原因特定が遅い。

## Decision

### D1: 読み取り専用の `sotp signal report` コマンドを追加する

4 chain（adr_user / spec_adr / catalog_spec / impl_catalog）を横断して Yellow / Red 信号の発生箇所を列挙する。出力粒度は entry id・参照文字列・判定理由・対象ファイル位置。`--chain <id>` / `--only yellow,red` などの絞り込みを備える。signals の再計算や persist は行わない（既存 `calc-*` の成果物を読むだけの view）。

## Consequences

- 良: gate block 時の原因特定が 1 コマンドで完結する。jq による手動解析への依存が消える。
- 中立: `sotp track resolve` への要約統合（blocker 表示に内訳を含める）は本決定の範囲外とし、必要になった時点で別途判断する。

## Reassess When

- signals の persist 形式が変わり、view 側の追従コストが問題になったとき。
