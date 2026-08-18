---
adr_id: "2026-08-11-0910-guarded-stash-pop-pairing"
decisions:
  - id: D1
    user_decision_ref: "chat:2026-08-12-strip-excess-decisions"
    status: proposed
---
# ガード付き stash pop は同じ push が作成した stash commit と対にする

## Context

ガード付き stash push は作業ツリーに差分がなければ stash entry を作らないため、後続の無指定 pop は無関係な既存 entry を適用して削除し得る。

`2026-07-29-0839-base-merge-and-conflict-recovery.md` D3 はガード付き stash 操作を定めたが、push と pop の対応は定めていない。

## Decision

### D1: pop は同じ push が作成した stash だけを復元する

ガード付き stash push は作成した stash の commit OID、または stash を作成しなかったことを記録する。

ガード付き stash pop は記録された OID を指定して stash を適用し、成功後に記録を消す。

stash を作成しなかった記録なら、pop は何も適用せず記録を消す。

記録がない場合または OID が見つからない場合は失敗を報告して停止し、無関係な stash entry には触れない。

手動回復では operator が `git stash list` を確認して操作する。

### Existing decision relationship

本 ADR の D1 は `2026-07-29-0839-base-merge-and-conflict-recovery.md` D3 を **refines** し、ガード付き stash の push と pop を commit OID で対応させる。

## Rejected Alternatives

- **stash stack の番号で push と pop を対応させる**: 無関係な stash entry の追加で番号が変わるため採用しない。
- **失敗後の手動回復まで機構化する**: 失敗は報告され、`git stash list` から回復できるため採用しない。

## Consequences

- Good: 無関係な stash entry を誤って適用または削除しない。
- Good: stash stack の番号が変わっても同じ OID を復元できる。
- Bad: 記録または stash が失われた場合は operator の手動回復が必要になる。

## Reassess When

- git stash が push と pop を直接対応付ける不変の識別子を提供した場合。

## Related

- `knowledge/adr/2026-07-29-0839-base-merge-and-conflict-recovery.md` D3 — ガード付き stash 操作の refines 対象。
