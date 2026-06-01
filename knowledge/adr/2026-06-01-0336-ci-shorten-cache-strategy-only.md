---
adr_id: 2026-06-01-0336-ci-shorten-cache-strategy-only
decisions:
  - id: D1
    user_decision_ref: "chat:2026-06-01:ci-shorten-cache-only-no-source-change"
    status: proposed
---
# 長くなった CI の短縮 — ソースを変えずキャッシュ戦略のみ見直す

## Context

重量級ネイティブ依存が `libs/infrastructure` に加わって以降、CI の所要時間が従来の約 3〜4 分から約 14〜16 分に伸びている。これを短縮したい。所要時間が伸びた具体的な原因は未確定であり、調査・試行錯誤の中で変わりうる。

## Decision

CI の所要時間を短縮する調整を行う。その際、ソースコード(機能実装・依存構成)は変更せず、キャッシュ戦略の見直しのみで対応する。原因の特定と具体的な手段の選定は track での試行錯誤を通じて行い、本 ADR では確定させない。
