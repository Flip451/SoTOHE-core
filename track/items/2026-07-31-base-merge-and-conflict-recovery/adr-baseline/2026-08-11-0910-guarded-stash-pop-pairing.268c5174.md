---
adr_id: "2026-08-11-0910-guarded-stash-pop-pairing"
decisions:
  - id: D1
    review_finding_ref: "pr-236-review:guarded-stash-pop-pairing:2026-08-11; fleet-prescription:2026-08-11-stash-concurrency"
    status: proposed
---
# guarded stash pop は同じ push が作成した stash commit と対にする

## Context

guarded stash の push は、clean tree では成功しても stash entry を作らない。このとき `refs/stash` に無関係な既存 entry があると、後続の無指定 pop がその entry を適用して削除し得る。`2026-07-29-0839-base-merge-and-conflict-recovery.md` D3 は guarded stash surface を定めたが、push と pop の pairing semantics は定めていない。

## Decision

### D1: pop は同じ push の結果だけを復元する

guarded pop は、対応する push が作成した stash commit OID とだけ対にする。可変な `stash@{n}` index を identity に用いない。push は created OID または nothing-to-stash を machine-owned、gitignored、repository-scoped な single-slot record に原子的に永続化し、record がある間の再 push は fail-closed とする。

pop は record を唯一の権威とする。created OID は各操作直前に再検証して OID 指定で適用・除去し、成功後に record を clear する。record 不在は fail-closed、OID の不在・不一致やその他の失敗は record を保持して fail-closed、nothing-to-stash は no-op 後に clear とし、無関係な stash には触れない。

stash 作成成功から record 永続化までの crash window は、absent-record の fail-closed recovery lane に写像する。

threat model は、guarded path を adapter lock で直列化し、unguarded mutation を hook layer で拒否し、token 保持者の manual intervention を operator responsibility とする範囲で閉じる。OID 再検証を最後の防御とし、これを超える防御は採用しない。

### Existing decision relationship

本 ADR の D1 は `2026-07-29-0839-base-merge-and-conflict-recovery.md` D3 を **refines** する。guarded stash surface の導入判断を維持したまま、push と pop の一対一対応を追加する decision-preserving refinement である。

## Rejected Alternatives

- **clean tree で guarded push を拒否する**: caller を事前の tree-state 判定に不必要に結合するため却下する。
- **operator interference に対する追加の lock / fsync layer を設ける**: token を保持する operator の介入は repository guard の対象外であり、guard philosophy を超える防御となるため却下する。

## Consequences

- Good: clean tree の push 後に、無関係な既存 stash を誤って適用・削除しない。
- Good: stash stack の index 変動に依存せず、push と pop を immutable identity で対応付けられる。
- Bad: guarded stash surface は single-slot record の lifecycle と、失敗後の pending record を管理する必要がある。
- Bad: stash 作成成功から record 永続化までの crash window は残り、absent-record recovery lane での明示的な確認が必要になる。

## Reassess When

- git stash が push と pop を直接対応付ける immutable transaction handle を提供したとき。

## Related

- `knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md` D3 — guarded stash surface の refines 対象。
