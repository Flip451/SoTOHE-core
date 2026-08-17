---
adr_id: "2026-08-14-0428-review-yield-measurement"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:claude-session-01498BG434ep3fe1BuyqfDtc:2026-08-14"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:claude-session-01498BG434ep3fe1BuyqfDtc:2026-08-14"
    status: proposed
---
# review-yield を計測する

## Context

どの検査がどの文脈で欠陥を検出しているかは計測されておらず、検証量を調整する判断にも、レビュアー構成（provider / model / effort）の変更の影響評価にも使える根拠が無い。reviewer verdict は構造化済みで、track 単位 telemetry の基盤も既にある。

## Decision

### D1: review 実行ごとに検出結果と割り当ての実値を telemetry に記録し、集計コマンドを設ける

記録する軸は、検査文脈（scope、round type）と、その実行を担ったレビュアー割り当ての実値（provider、model、reasoning effort）、および findings 件数とする。割り当ては設定から解決された実値を記録し、設定ファイルの参照では代替しない（設定は後から変わるため、過去の実行の再現に使えない）。

集計コマンドは読み取り専用とし、上記の任意の軸で実行回数と検出率を集計できるようにする。

### D2: 検証量の変更はデータを根拠とする裁定でのみ行う

検査の削減・増強は、十分な標本（目安 50 実行以上）の検出率を根拠に ADR 裁定で行う。本 ADR は検査を一つも変更しない。

## Rejected Alternatives

- **検出率データなしの検査調整**: 削減の根拠が残らない。
- **track クラスによる軽量レーンとの同時導入**: 検証量を減らす判断は本 ADR が作る根拠の後に行う。分類軸の設計は独立の問題。

## Consequences

- 良: 検証量の議論と、レビュアー構成変更（provider / model / effort）の影響評価に実測が使える。検査は変わらないため保証は不変。
- 負: telemetry スキーマと集計コマンドが保守対象に加わる。
- 中立: 検出率の低さは「無駄な検査」とも「抑止が効いている」とも解釈できる。削減裁定ではこの両義性を明示的に扱う。
- 中立: 検出率の比較は、対象 diff の難易度が実行ごとに異なるため厳密な統制比較にはならない。標本数で均す前提の指標である。

## Reassess When

- 標本が蓄積し、最初の検証量調整を裁定するとき。
