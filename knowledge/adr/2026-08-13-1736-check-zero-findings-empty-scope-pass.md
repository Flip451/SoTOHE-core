---
adr_id: "2026-08-13-1736-check-zero-findings-empty-scope-pass"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:claude-session-01498BG434ep3fe1BuyqfDtc:2026-08-14; empty-scope operator adjudication:2026-08-13"
    candidate_selection: "from:[A,B,C,D] chose:A"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:claude-session-01498BG434ep3fe1BuyqfDtc:2026-08-14"
    status: proposed
---
# check-zero-findings は空 scope を pass させる

## Context

正規順序では guarded commit 直後に次 phase へ入るため、pre-entry の `check-zero-findings --scope <s> --round final` の時点で対象 scope の diff は空になる。現実装は空 scope（`NotRequired(Empty)`）を fail に写し、空 scope に verdict を記録する経路も無い — 正規手順を踏むほど検査が通らない。実 track で operator 裁定により当該検査を phase 宣言 config から一時除去して回避中（2026-08-13）。

空 scope の内容は guarded commit gate のレビュー承認を通過したものだけで構成され、未レビューのまま空になる経路は構造的に存在しない。よって空は「整合的な不在」である。

## Decision

### D1: `NotRequired(Empty)` を pass に写す

非空 scope に対する fail は不変。pass 出力には empty scope である旨を明示する。

### D2: 一時除去した phase 宣言エントリを復旧する

D1 実装後、除去済みの pre-entry エントリを `.harness/config/phase-commands.json` に戻す。除去状態を恒久化しない。

## Rejected Alternatives

- **空 + 最新 round が final zero_findings なら pass**: track 中一度も触れていない scope（round 記録なしの整合的な不在）を偽 fail にする。commit gate の保証の下では履歴要求は冗長。
- **phase gate 用に diff base を再定義**: base 意味論の恒久的複雑化に見合わない。
- **検査エントリ除去の恒久化**: 非空 scope（未収束の編集残り）の検出まで失う。

## Consequences

- 良: 正規順序が宣言どおりの検査を通過する。変更は 1 variant の写しのみ。
- 中立: 健全性は「commit は guarded 経路のみ + commit gate がレビュー承認を要求する」ことに依存（git hooks が担保）。

## Reassess When

- unguarded な commit 経路が導入される、または commit gate からレビュー承認要求が外れたとき。
