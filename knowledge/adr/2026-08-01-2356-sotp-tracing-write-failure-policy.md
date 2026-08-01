---
adr_id: "2026-08-01-2356-sotp-tracing-write-failure-policy"
decisions:
  - id: D1
    review_finding_ref: "spec_adr_verification:CN-03; rollback_diagnoser:separate-narrow-adr"
    status: proposed
---
# sotp tracing の書き込み失敗を型付き Result で caller へ伝搬する

## Context

全 sotp コマンドの tracing 記録をローカル JSONL に保存する決定では、記録する内容と保存先は定められていたが、記録処理が失敗した場合の caller-visible な失敗境界は定められていなかった。panic を避けるだけでは失敗を黙って握りつぶす実装も選べるため、usecase の service、port、および interactor が共有する feature 固有のエラー契約を一意に導けなかった。

## Decision

### D1: trace 書き込み失敗を CommandTraceWriteError として Result で伝搬する

`knowledge/adr/2026-07-29-0839-sotp-tracing-instrumentation.md` D1 を、コマンド trace の書き込み失敗境界に限って **refines** する。trace 記録処理が失敗した場合は panic せず、失敗を黙って握りつぶさず、型付き `CommandTraceWriteError` として `Result` の error 側から caller へ伝搬する。

usecase の service、書き込み port、および interactor はこの feature 固有の失敗型を境界契約として扱う。下位の書き込み失敗は `CommandTraceWriteError` へ型を保って変換し、正常終了へ置き換えない。

この refinement は失敗時の伝搬だけを定める。書き込み成功時の JSONL 内容、ローテーション、telemetry 集計、外部送信の有無、および CLI への統合方法に関する決定は変更しない。

## Rejected Alternatives

- 書き込み失敗時に panic する: library code の panic 禁止に反し、caller が失敗を型として扱えない。
- 書き込み失敗を無視して正常終了する: 観測記録の欠落を caller から検出できず、失敗を黙って握りつぶすことになる。
- 生の文字列または feature 非固有の汎用エラーへ潰す: service、port、interactor 間の失敗契約が型で固定されず、trace 書き込み失敗を caller が識別できない。

## Consequences

- Good: trace 書き込み失敗を caller が型安全に検出し、伝搬または処理できる。
- Good: service、port、interactor の失敗境界が `CommandTraceWriteError` に統一され、panic と silent failure の双方を避けられる。
- Bad: 各境界で下位エラーを `CommandTraceWriteError` へ変換し、`Result` を扱う実装とテストが必要になる。
- Neutral: 成功時の記録内容と周辺機能の振る舞いは変わらない。

## Reassess When

- trace 記録を best-effort として扱い、記録失敗とコマンド本体の結果を同時に caller へ返す別の型付き契約が必要になったとき。
- 複数の trace sink を導入し、sink ごとの部分失敗を表現する必要が生じたとき。

## Related

- `knowledge/adr/2026-07-29-0839-sotp-tracing-instrumentation.md` D1 — 全 sotp コマンドを tracing 計装し、ローカル JSONL に記録する元決定。本 ADR D1 は書き込み失敗境界だけを refines する。
- `knowledge/adr/2026-08-01-0902-sotp-tracing-rotation-policy.md` D1 — JSONL のローテーションと保持ライフサイクル。本 ADR はその成功時の処理とローテーション判断を変更しない。
- `knowledge/conventions/coding-principles.md` §Error Handling: Result and ? Operator — `Result` と `?` によるエラー伝搬の規約。
- `knowledge/conventions/coding-principles.md` §No Panics in Library Code — library code で panic を避け、エラーを返す規約。
