---
adr_id: "2026-07-28-1024-catalogue-scope-is-implementation-delta"
decisions:
  - id: D1
    user_decision_ref: "chat:2026-07-28:revoke-d6-catalogue-scope-obligation"
    status: accepted
---
# track 単位 feature 宣言 ADR の D6 を撤回する

## Context

`2026-07-27-0039-tddd-track-scoped-feature-declaration.md` D6 は、ある cargo feature を最初に宣言する track に対し、その有効化で可視化された既存 public 要素を catalogue に整備する責任を課した。

これは、`2026-04-11-0001-baseline-reverse-signals.md` が既に定めた、catalogue は per-track であり、reverse check はその track で変化した型を対象とするという scope に反する。可視性はどの cargo feature が有効かで決まり、新規性は変更が何を追加または変更するかで決まる。

## Decision

### D1: `2026-07-27-0039-tddd-track-scoped-feature-declaration.md` D6 を撤回する

`2026-07-27-0039-tddd-track-scoped-feature-declaration.md` D6 を撤回する。代替の義務は導入しない。catalogue の scope を定める判断は `2026-04-11-0001-baseline-reverse-signals.md` に既に存在するためである。

## Consequences

- 一度も観測されていなかった既存コードの drift は、catalogue が所有しない別の関心として残る。

## Reassess When

- `2026-04-11-0001-baseline-reverse-signals.md` の catalogue scope が変更されたとき。

## Related

- `knowledge/adr/2026-07-27-0039-tddd-track-scoped-feature-declaration.md` — D6 を撤回する。
- `knowledge/adr/2026-04-11-0001-baseline-reverse-signals.md` — D6 が逸脱していた catalogue scope の判断を復元する。
