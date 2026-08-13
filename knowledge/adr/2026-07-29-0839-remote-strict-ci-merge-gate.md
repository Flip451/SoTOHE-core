---
adr_id: "2026-07-29-0839-remote-strict-ci-merge-gate"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:track-adr2pr-phase0-approval:2026-07-31"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:track-adr2pr-phase0-approval:2026-07-31"
    status: proposed
---
# merge 強制を remote CI に移し、/track:merge を無確認直行にする

## Context

merge gate は現在ローカル実行に依存しており、GitHub 側には「gate を通っていない merge」を機械的に拒否する仕組みがない。また `/track:merge` はユーザー確認を挟む段が残っており、merge 判断の実体（ユーザーが `/track:merge` を打つこと）と重複した承認になっている。

## Decision

### D1: strict merge gate を GitHub Actions CI + branch protection で強制する

remote CI で `sotp signal check --gate merge`（strict）を含む track-aware gate を実行し、branch protection により CI 失敗時はマージ不可とする。track 解決は checkout ref から行う（アクティブ track 不明時に graceful skip はしない — 解決失敗は workflow 側の ref 指定で対処する既定方針に従う）。ローカルの merge gate 実行は早期検知用として残るが、強制の実体は remote に移る。

### D2: /track:merge はユーザー確認なしで CI green 待ち → merge に直行する

`/track:merge` の発行自体を merge 承認とみなし、以降の確認プロンプトを workflow / adapter から除去する。安全性は D1 の remote strict CI が担保する。

## Rejected Alternatives

### A: ローカル merge gate のみの現状維持

gate を通さない merge を機械的に防げず、承認段の重複も残るため却下。

## Consequences

- 良: 「CI green = merge 可」に一本化され、merge までの対話段数が減る。gate バイパスが構造的に不可能になる。
- 負: remote CI の実行時間が merge のクリティカルパスに入る。CI インフラ障害時は merge が止まる。

## Reassess When

- remote CI の所要時間が merge 待ちの主要因になったとき（ローカル gate 先行実行との役割分担を見直す）。
