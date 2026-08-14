---
adr_id: "2026-08-13-1756-adr-open-set-depth-check"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:claude-session-01498BG434ep3fe1BuyqfDtc:2026-08-14; issue-registration:2026-08-11"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:claude-session-01498BG434ep3fe1BuyqfDtc:2026-08-14"
    status: proposed
---
# ADR 起草とレビューに開集合検査を追加する

## Context

ADR の一句（「後始末まで一括実行」「実装入力の hash」）が、厳密に機構化すると数千行の実装と複数回のレビュー非収束に展開した実例が続いた。行数見積りのゲートはあるが、**決定が開集合（正確な列挙・完全な追跡を要求する意味論）を含むかを起草時に問う関門が無い**。検出は実装後のレビュー滞留まで遅れる。

## Decision

### D1: ADR 起草ヒアリングと ADR 意味論レビューに開集合検査を追加する

起草ヒアリング（Full モード）と ADR レビュー prompt に次の検査を加える: 決定文面に「正確な列挙」「完全な追跡」「すべての X を Y する」型の句があれば、(a) 既存の権威（コンパイラ・cargo・rustdoc・git・domain 型）へ委譲可能か、(b) 保守的な過大近似で足りるか、(c) 厳密実装の深さ見積り、の三択を確認してから決定を確定する。

### D2: impl-plan / types の reviewer briefing に同観点を追加する

実装方針が「開集合をヒューリスティックで覆う」形になった時点で指摘する観点（手作りのパーサ・リソース管理・ビルドシステム模倣の検出）を briefing に加える。起草をすり抜けた開集合の二段目の網。

## Rejected Alternatives

- **機械 lint 化（句のパターン検出を CI に置く）**: 自然言語の意味判定であり決定論検査に乗らない。誤検出の摩擦が価値を上回る。
- **事後検出のみ（現行の艦隊監視の継続）**: レビュー 2 桁ラウンド滞留まで検出が遅れ、請求書が実装後に届く。

## Consequences

- 良: 開集合の混入が起草時（D1）と計画時（D2）の 2 点で捕まる。変更は文書のみで Rust 実装なし。
- 負: ヒアリングの質問が 1 種増える。briefing の維持対象が増える。

## Reassess When

- 検査を通過した ADR で開集合起因のレビュー滞留が再発したとき（検査の語彙・観点の見直し）。
