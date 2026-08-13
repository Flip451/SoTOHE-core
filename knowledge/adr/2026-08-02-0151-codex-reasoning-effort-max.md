---
adr_id: "2026-08-02-0151-codex-reasoning-effort-max"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:sotohe-issues-discussion:2026-08-02"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:luna-max-profile-scope:2026-08-02"
    status: proposed
---
# reasoning effort に max 段を追加し、限定レーンを Luna Max へ移行する

## Context

Codex CLI の GPT-5.6 ファミリー（Sol / Terra / Luna）は reasoning effort として Low / Medium / High / Extra high / **Max** / Ultra を提供しており、max は xhigh の上位段として CLI で利用可能である（公式 Learn ドキュメントおよび CLI モデルセレクタで確認。macOS アプリのピッカーに max が出ないのは既知の表示バグで CLI の制約ではない）。

一方 sotp の `ReasoningEffort` は xhigh までしか表現できず、capability profile（`.harness/config/agent-profiles.json`）で max を宣言できない。最終 verdict 系レーン（reviewer final 等）が利用できる最上位推論段が実際のプロバイダー提供段より一段低い状態にある。

GPT-5.6 Luna は GPT-5.6 Terra と同じ context window・最大出力長・tool capability を持ち、公開 coding benchmark では Terra に近い一方、API 単価と Codex credits の消費係数はいずれも Terra の 10 分の 1 である。ただし、長文脈・調査・セキュリティ・高度な抽象推論では Terra との性能差が大きく、公開値だけで Luna Max が Terra の既定 effort と同等以上であるとは判断できない。

既存 track には Luna Max の運用実績がないため、過去 track 同士による品質・credits・所要時間・再試行回数の比較は成立しない。過去 track の再実行も、入力状態や外部条件を完全には再現できず、採用前の必須条件としない。最初の限定運用で実測し、失敗時に Terra へ戻せる範囲から導入する。

## Decision

### D1: `ReasoningEffort` に `Max` を追加し、provider × effort の妥当性検証を維持する

`ReasoningEffort` enum に `Max` variant を追加し、`model_reasoning_effort="max"` として発行する。全 match site（capability exec、agent profiles 検証、review 系 runner）を追従させる。プロバイダーが当該 effort を受理しない構成は既存の `UnsupportedEffort(ProviderName, ReasoningEffort)` の fail-closed 検証で拒否する — max の受理可否はプロバイダー宣言側の知識とし、enum 追加自体はプロバイダー非依存に保つ。

Ultra 段は本 ADR の範囲外とする（従量課金・提供条件が別枠であり、採否は別途の判断）。D1 の enum 拡張はプロバイダー非依存とし、既定 profile の限定的な変更は D2 で扱う。

### D2: 実装・修正レーンのみ Luna Max へ移行する

既定 capability profile のうち、`implementer`、`review-fix-lead`、`dry-fix-lead` を `gpt-5.6-luna` + `max` へ変更する。これらは成果物を機械検証または後続レビューで評価でき、失敗時の再実行範囲も限定しやすいため、最初の運用観測対象とする。

設計・計画・調査・最終 verdict の品質を優先し、`spec-designer`、`impl-planner`、`researcher`、`dry-checker` の final、`obligation-fulfillment-verifier`、`waiver-verifier` は現時点では Terra の既定構成を維持する。

runtime による自動フォールバックは実装しない。Luna Max の出力が不完全、timeout、または gate failure になった場合は、同じ割り当てを従来の Terra 構成で 1 回再実行する。Terra で成功した場合は model regression の候補として記録し、そのレーンを Terra に戻す判断材料とする。

## Rejected Alternatives

### A: 効力値を文字列 pass-through にして enum 検証を捨てる

typo が実行時までエラーにならず、provider × effort の fail-closed 検証も失われるため却下。

### B: 既定の Terra レーンを一括して Luna Max へ置き換える

公開 benchmark では長文脈・調査・セキュリティ・高度な抽象推論の差が大きく、設計・調査・最終 verdict まで一括移行すると品質低下を検知しにくいため却下。

### C: 過去 track との実測比較を導入の前提にする

過去 track に Luna Max の実績がなく、同条件の比較データは存在しない。過去 track の再実行も厳密な再現試験にならず、採用判断を不必要に遅らせるため却下。

## Consequences

- 良: `Max` は schema / enum の能力として、各 provider declaration が対応状況に応じて列挙できる。既定 profile で Luna Max へ移すのは `implementer`、`review-fix-lead`、`dry-fix-lead` だけであり、最終 verdict 系レーンは既存の model / effort を維持する。
- 良: 実装・修正レーンの token 単価と credits 消費係数を Terra の 10 分の 1へ下げられる。
- 負: enum variant 追加は全 match site の追従を強制する（網羅 match ゆえコンパイルで検出される — 漏れは起きない）。
- 負: Luna の単価が低くても、max effort による reasoning token と所要時間の増加で、実際のコストパフォーマンスが期待どおり改善しない可能性がある。
- 中立: 本 ADR を起点とする限定運用が Luna Max の最初の観測となる。track 完了時に、取得可能な範囲で品質・credits・所要時間・再試行回数を報告する。

## Reassess When

- Ultra 段の採否を判断するとき。
- Luna Max では失敗し、同じ割り当てを Terra で再実行すると成功する事例が生じたとき。
- 実測した credits・所要時間・再試行回数が Terra の既定構成より悪化したとき。
- Luna Max の適用を設計・計画・調査・最終 verdict のレーンへ拡大するとき。
- GPT-5.6 ファミリーの価格、性能、または利用可能な reasoning effort が変わったとき。
- プロバイダー多様化（別 ADR）で effort 語彙がプロバイダー間で発散し、対応表の宣言化が必要になったとき。
