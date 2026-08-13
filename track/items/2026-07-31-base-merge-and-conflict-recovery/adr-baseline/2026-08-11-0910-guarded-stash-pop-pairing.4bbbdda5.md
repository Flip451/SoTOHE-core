---
adr_id: "2026-08-11-0910-guarded-stash-pop-pairing"
decisions:
  - id: D1
    review_finding_ref: "pr-236-review:guarded-stash-pop-pairing:2026-08-11"
    status: proposed
---
# guarded stash pop は同じ push が作成した stash commit と対にする

## Context

guarded stash の push は、clean tree では成功しても stash entry を作らない。このとき `refs/stash` に無関係な既存 entry があると、後続の無指定 pop がその entry を適用して削除し得る。`2026-07-29-0839-base-merge-and-conflict-recovery.md` D3 は guarded stash surface を定めたが、push と pop の pairing semantics は定めていない。

## Decision

### D1: pop は同じ push の結果だけを復元する

guarded stash の push は、作成した stash の immutable な commit identity、または明示的な nothing-to-stash outcome のいずれかを返す。可変な `stash@{n}` index を pairing identity に用いない。

guarded stash surface は push outcome を、repository の local operational state 配下にある machine-owned、gitignored、repository-scoped な single-slot record へ永続化する。record の exact path は implementation detail とし、その ownership と gitignore は guarded stash surface が担う。書き込みは temporary file と rename により原子的に行う。record が存在する間の次の guarded push は fail-closed とし、pending pop の解決を先に要求する。

pop はこの record を読み取る。record がなければ recovery guidance を伴って fail-closed とする。nothing-to-stash outcome なら no-op の後に record を clear する。commit identity が記録されている場合、その identity に対応する stash だけを復元し、成功後にその entry と record を clear する。記録された stash が stash list に存在しない場合、または commit identity が一致しない場合は fail-closed として record を保持する。その他の失敗でも record を暗黙に clear せず、無関係な既存 stash entry には触れない。

この record は guarded stash surface 固有の状態であり、`.sync-base.json` とは別である。本 decision は repository-local record 全般の policy を定めない。

### Existing decision relationship

本 ADR の D1 は `2026-07-29-0839-base-merge-and-conflict-recovery.md` D3 を **refines** する。guarded stash surface の導入判断を維持したまま、push と pop の一対一対応を追加する decision-preserving refinement である。

## Rejected Alternatives

- **clean tree で guarded push を拒否する**: caller を事前の tree-state 判定に不必要に結合するため却下する。

## Consequences

- Good: clean tree の push 後に、無関係な既存 stash を誤って適用・削除しない。
- Good: stash stack の index 変動に依存せず、push と pop を immutable identity で対応付けられる。
- Bad: guarded stash surface は single-slot record の lifecycle と、失敗後の pending record を管理する必要がある。

## Reassess When

- git stash が push と pop を直接対応付ける immutable transaction handle を提供したとき。

## Related

- `knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md` D3 — guarded stash surface の refines 対象。
