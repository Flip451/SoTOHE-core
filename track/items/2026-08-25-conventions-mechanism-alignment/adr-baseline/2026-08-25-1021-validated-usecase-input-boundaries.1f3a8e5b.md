---
adr_id: "2026-08-25-1021-validated-usecase-input-boundaries"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:claude-session-01JAppYbUq3yZwAfDVLnqf56:2026-08-26 merge-stage adoption of the three delta drafts"
    status: proposed
---
# usecase 入力境界を Command と Query の検証済み型に統一する

## Context

`2026-08-20-1043-conventions-mechanism-alignment.md` D4 は、cli と usecase の境界から未検証の string primitive を除き、usecase 所有の boundary 型で一度だけパースすることを定めた。しかし D4 の「Command 型のみ」という表現は、CQRS の query usecase が検証済み Query 型を入力に取る規則と両立しない。

入力境界で守るべき対象は、Command という役割名ではなく、未検証の primitive を渡さないこと、境界語彙を usecase が所有すること、及び cli が domain を知らないことである。

## Decision

### D1: usecase の入力は検証済み Command または Query 型を 1 個だけ受け取る

command usecase の入力境界は検証済み Command 型を 1 個だけ受け取り、query usecase の入力境界は検証済み Query 型を 1 個だけ受け取る。いずれも位置引数で未検証の primitive を並べてはならない。

string から Command または Query へのパースは usecase 所有の boundary 型が担い、cli はそのパースを一度だけ呼び出してから対応する入力境界を呼び出す。domain enum の鏡像を cli 側に定義せず、境界語彙は usecase 所有の boundary 型に一本化する。cli が domain を知らない原則は維持する。

## Rejected Alternatives

- **query usecase も Command 型だけを受け取る**: CQRS の Query 役割を否定し、読み取り専用 usecase の入力規則と矛盾する。
- **query usecase に未検証 primitive を許す**: パース点が分散し、境界語彙の所有者と cli の責務分離を失う。
- **cli に domain enum の鏡像を置く**: cli が domain 語彙を知ることになり、既存の層境界を破る。

## Consequences

- Good: command と query の双方で、入力検証とパース点が usecase 境界に一意に定まる。
- Good: CQRS の役割分離を保ったまま、cli と domain の分離を維持できる。
- Bad: boundary 型は Command と Query を役割に応じて明示的に区別する必要がある。

## Reassess When

- Command と Query のいずれにも属さない入力役割が必要になり、同じ検証済み境界規則で表現できなくなったとき。

## Related

- `knowledge/adr/2026-08-20-1043-conventions-mechanism-alignment.md` D4 — 本決定が refine する現在の関係連鎖の対象。D4 の Command-only 表現を、役割に応じた検証済み Command / Query 入力へ明確化する。
- `knowledge/adr/2026-08-15-1302-composition-root-pure-di-port-granularity.md` D2 — Command 文脈の未検証 primitive 排除は維持する。D2 の Command-only 表現もこの明確化の影響を受けるが、本決定が直接 refine する対象は一般規則へ昇格した D4 である。
