---
adr_id: "2026-09-09-0916-query-only-reviewer-safety"
decisions:
  - id: D1
    user_decision_ref: "user message: 2026-09-10 「delta ADRについては承認します」"
    status: accepted
---
# クエリ専用 ReviewCycle における非実行 Reviewer の安全契約

## Context

`ReviewCycle` は review 実行と review 状態の照会を同じ型で扱うため、状態照会だけを構成する場合にも generic な `Reviewer` 依存を受け取る。`review results` の state-summary 経路は review 状態を `get_review_states` で読み、`review check-approved` は `evaluate_approval` 経由で同じ `get_review_states` を用いる。いずれも `Reviewer::review` も `fast_review` も実行しない。この用途には、実行を担わない `NullReviewer` が必要である。

一方で、`review` と `fast_review` は `Reviewer` port が定める実行経路である。特に `fast_review` は advisory な実行であって、状態照会そのものではない。誤ってこれらの経路が `NullReviewer` に到達したとき、成功 verdict を作ることや provider を起動することは、照会専用構成の失敗を隠してしまう。

既存の観測可能な非実行・失敗応答を維持する必要がある。ただし、既存の基準文面が現在の実行来歴 enum を既に使っていたとは扱わない。従来の振る舞いを、現在の型付き診断設計で明示して表現する。

## Decision

### D1: 状態照会専用の Reviewer は非実行 adapter として fail-closed にする

`NullReviewer` は infrastructure 層に置く非実行の secondary adapter とする。composition root はこの adapter を構築して `ReviewCycle` へ注入するだけであり、`Reviewer` port の実装を所有しない。

`review results` の state-summary 経路は `get_review_states` を用い、`review check-approved` は `evaluate_approval` 経由で同じ `get_review_states` を用いる。いずれも `NullReviewer` の `Reviewer::review` および `fast_review` を呼ばない。両 method が誤って呼ばれた場合は、成功 verdict や provider 実行を生成せず、確立済みの型付き `PreSpawn` / `Unavailable` 診断で失敗する。

この判断は `Reviewer` port の method 契約を変更しない。既存の非実行・失敗という観測可能な振る舞いを保持し、その非開始状態を現在の型付き診断として表す。

## Rejected Alternatives

### A: 非実行 adapter から成功 verdict を返す

状態照会専用の誤配線を成功として扱うと、実行されていない review を完了済みのように見せるため採用しない。

### B: `Reviewer` 依存を optional にし、port の method 契約を変える

状態照会のためだけに共有する `ReviewCycle` の契約を広く変更し、review 実行経路との境界を不明瞭にするため採用しない。

### C: 非実行 adapter を composition root に実装する

composition root が secondary adapter の実装を所有することになり、純 DI の責務境界を崩すため採用しない。

## Consequences

- 良: 状態照会は provider を起動せず、誤った実行呼び出しは非開始の理由を保ったまま失敗する。
- 良: `fast_review` を状態照会と混同せず、実行経路として扱える。
- 負: 状態照会だけの構成にも `Reviewer` の concrete adapter を注入する必要がある。

## Reassess When

- 状態照会と実行が同じ `ReviewCycle` の generic 依存を共有する必要がなくなったとき。
- 非開始を表す型付き診断契約が変更されるとき。

## Related

- [composition root 規範を純 DI に確定し、実践側の逸脱を解消する](2026-07-23-0111-composition-root-pure-di-realignment.md) — D1 の純 DI 境界を維持する。
- [共通ハーネスで観測された責務・復旧・検証契約を整える](2026-09-08-1610-upstream-handoff-hardening.md) — D5 の実行失敗診断と区別し、非開始を型付きで伝える。
