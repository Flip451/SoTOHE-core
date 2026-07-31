---
adr_id: "2026-07-31-2134-signal-report-occurrence-source"
decisions:
  - id: D1
    review_finding_ref: "phase2-upstream-bounce:type-designer-and-spec-designer:2026-07-31"
    status: proposed
---
# signal report の発生単位データ取得方針

## Context

`sotp signal report` が 4 chain の Yellow / Red を発生単位で列挙するには、entry id・参照文字列・判定理由・対象ファイル位置を取得できなければならない。しかし、chain ⓪ は signal を永続化せず、chain ① も発生単位の signal ではなく集計値だけを永続化するため、既存の signal 成果物を読むだけでは要求された内訳を構成できないことが型設計時に判明した。

## Decision

### D1: 発生単位データが永続化されない chain は report 実行時に導出する

`knowledge/adr/2026-07-29-0839-signal-report-command.md` D1 を、signal の取得元に限って **refines** する。発生単位の signal 成果物が存在する chain はその成果物を読み、存在しない chain ⓪・①は正規の入力成果物から既存の評価規則で発生単位データをメモリ上に導出する。

この導出は report の表示を構成するためだけに行い、signal や集計値を永続化しない。したがって `sotp signal report` は読み取り専用の view であり続ける。一方、元決定の「signals の再計算を行わず、既存の `calc-*` 成果物だけを読む」という制約は、発生単位データが永続化されている chain に限定する。4 chain を横断すること、Yellow / Red の発生箇所を所定の粒度で列挙すること、および絞り込みの契約は変更しない。

chain ③ は、構成上期待されるすべての TDDD layer について、永続化された implementation-signal 成果物を読む。期待される layer の成果物が一つでも存在しない場合、選択された source は提供不能であり、query は `SourceUnavailable(ImplCatalog)` を返す。この欠落を任意の空 contribution として扱わない。存在する成果物についても、不正な形式、安全に読み取れない状態、または許容上限を超える大きさを検出した場合は失敗とし、その layer を読み飛ばさない。

## Rejected Alternatives

- 全 chain の発生単位データを永続化する: report のために chain ⓪ の live-only 性質と chain ① の保存形式を変更し、永続化契約を不必要に拡大する。
- 対象 chain または出力粒度を狭める: 4 chain の原因箇所を 1 コマンドで特定するという元決定の目的を満たさない。

## Consequences

- 良: 永続化形式の違いにかかわらず、4 chain の Yellow / Red を同じ発生単位で列挙できる。
- 良: report はファイルを書き換えず、導出した signal も保存しない。
- トレードオフ: chain ⓪・①では report 実行時に評価処理が必要になり、永続済み成果物だけを読む場合より処理量が増える。

## Reassess When

- 全 chain が発生単位データを共通形式で永続化するようになったとき。
- report 実行時の導出コストまたは入力間の整合した読み取りが運用上の問題になったとき。

## Related

- `knowledge/adr/2026-07-29-0839-signal-report-command.md` D1 — 4 chain 横断の occurrence-level report と読み取り専用性を定めた元決定。本 ADR D1 は取得元の規則だけを refines する。
