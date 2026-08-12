---
adr_id: "2026-08-05-1035-type-signals-authority-availability-boundary"
decisions:
  - id: D1
    user_decision_ref: "chat:2026-08-12-strip-excess-decisions"
    status: proposed
---
# type-signals cache は clean な同一 HEAD commit でのみ再利用する

## Context

型シグナルは catalogue 宣言と実装の一致判定結果であり、作業ツリーが変わった後に古い評価 cache を再利用すると現在の実装について誤った結果を返し得る。

cache の再利用可否は、作業ツリーに差分がなく、HEAD commit が記録時と一致し、かつ権威を読み取れることの三条件で判定する。

## Decision

### D1: cache は clean な同一 HEAD commit でのみ再利用する

type-signals の評価 cache は、作業ツリーに差分がなく、かつ HEAD commit が記録時と一致する場合に限り再利用する。

作業ツリーに差分がある場合、HEAD commit が一致しない場合、または権威を読み取れない場合は常に再計算する。

### Existing decision relationship

本 ADR の D1 は `2026-07-29-0839-base-merge-and-conflict-recovery.md` D4 の cache 再利用条件を **modifies** する。

## Rejected Alternatives

- **局所入力 hash により差分がある作業ツリーでも cache を再利用する**: 複雑性に見合わないため採用しない。

## Consequences

- Good: 変更後の作業ツリーに古い評価結果を返さない。
- Good: cache の再利用条件が clean な作業ツリー、同一の HEAD commit、読取可能な権威の三条件になる。
- Bad: 作業ツリーに差分がある間は、入力が実質的に同じでも再計算する。

## Reassess When

- 差分がある作業ツリーでの再計算費用が常態的に問題となった場合。

## Related

- `knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md` D4 — D1 が modifies する対象。
