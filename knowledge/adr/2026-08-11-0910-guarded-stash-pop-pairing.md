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

guarded stash の push は、作成した stash の immutable な commit OID、または明示的な nothing-to-stash outcome のいずれかを返す。可変な `stash@{n}` index を guarded path の identity に用いない。

guarded stash surface は push outcome を、repository の local operational state 配下にある machine-owned、gitignored、repository-scoped な single-slot record へ永続化する。record の exact path は implementation detail とし、その ownership と gitignore は guarded stash surface が担う。書き込みは temporary file と rename により原子的に行う。record が存在する間の次の guarded push は fail-closed とし、pending pop の解決を先に要求する。

pop はこの record を読み取る。record がなければ recovery guidance を伴って fail-closed とする。nothing-to-stash outcome なら no-op の後に record を clear する。commit OID が記録されている場合、適用直前にその OID の実在を再検証し、OID を指定して対象 stash だけを適用する。適用成功後の削除では、その時点の reflog reference を OID から再解決し、reference が同じ OID を指すことを確認してから削除する。記録された OID が stash list に存在しない場合、または再解決した identity が一致しない場合は fail-closed として record を保持する。その他の失敗でも record を暗黙に clear せず、無関係な既存 stash entry には触れない。削除成功後にだけ record を clear する。

stash 作成成功から record 永続化までの crash window は transactionally には除去しない。この window で停止した結果は「stash は存在するが record は不在」となり、absent-record fail-closed recovery lane に入る。guidance はこの状況を明示し、`git stash list` による確認手順と、処理を続けるための guarded recovery route を示す。

threat model は次で閉じる。guarded path は adapter lock で直列化し、unguarded な stash mutation は repository の `reference-transaction` hook layer が拒否する。token を保持する operator の manual intervention は operator responsibility とし、適用直前と削除直前の OID 再検証を誤適用に対する最後の防御とする。

この record は guarded stash surface 固有の状態であり、`.sync-base.json` とは別である。本 decision は repository-local record 全般の policy を定めない。

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
