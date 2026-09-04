---
adr_id: "2026-08-25-0425-gate-output-summary-contract"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:claude-session-01WeFPmkvji5CWNP5T5A8q1G:2026-08-25 Phase 0 adjudication approval (D1 scope-membership delegation)"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:claude-session-01KjrLiixdHPZAezqFdVwGSS:2026-08-25 orchestrator-token adjudication"
    status: proposed
---
# ゲートの標準出力をサマリ契約にする

## Context

ゲート・検証コマンドは成功時も全文を標準出力に流す: テスト実行は成功 PASS 行を全件(実測 330 行)、義務評価の失敗は内部レコードの巨大 Debug 表現、コミットゲートはその全てを親に返す。オーケストレーターの課金ログ実測(2026-08-22)で、これがコミットごとに数万トークンの固定費として観測された。診断の全文が要るのは失敗の調査時だけであり、呼び出し側が毎回摂取する理由はない。

## Decision

### D1: ゲートの stdout は「結果 + ログパス + 失敗抜粋」に限る

ゲート・検証系タスク(テスト実行・義務評価・コミットゲート等)の標準出力の契約を、判定(PASS/FAIL)・フルログのファイルパス・失敗時のみの抜粋(失敗項目と短い理由)に統一する。対象タスクの所属は、`Makefile.toml` と `bin/sotp` の定義でテスト実行・義務評価・コミット前の集約ゲートとして定義されるものにより決める。フルログは `tmp/gate/` 配下に残し、調査時に開く。成功時に個別 PASS 行・内部レコードの Debug 表現を stdout に出さない。

### D2: 機械可読の判定は既存の exit code と検査コマンドが担う

本契約は人間・オーケストレーター向けの表示の変更であり、機械判定の面は変えない: 合否は exit code、状態照会は既存の check 系コマンドが正。stdout の文面をパースする新経路を作らない。

## Rejected Alternatives

- **呼び出し側の規律(「最終 N 行以外読むな」)だけで済ませる**: 手順文書の規律は別 ADR で導入済みだが、prompt-level であり全文が流れる事実は変わらない。出力側で契約にするのが構造的。
- **verbosity フラグで全文/サマリを切り替え可能にする**: 既定が全文のままなら固定費は消えず、既定をサマリにするなら本決定と同じ。切り替えの複雑性だけ増える。

## Consequences

- 良: コミット・レビューのたびの成功ログ摂取が消える。ログはファイルに全量残るため診断情報の損失はない。
- 負: CI・ゲートの人間向け見え方が変わる(慣れた全文出力が消える)。既存の出力を前提にしたテストの改訂が必要。
- 中立: 効果は課金ログの同一指標(コミットあたりの親入力トークン)で事後検証できる。

## Reassess When

- ログファイルの肥大が別の問題(ディスク・清掃)を起こしたとき(計測除外 config と同じ扱いへ)。
- 失敗抜粋の情報量が診断に不足し、フルログを開く頻度が常態化したとき(抜粋の設計を見直す)。
