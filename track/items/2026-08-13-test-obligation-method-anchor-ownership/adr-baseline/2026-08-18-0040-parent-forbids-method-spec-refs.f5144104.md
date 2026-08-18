---
adr_id: "2026-08-18-0040-parent-forbids-method-spec-refs"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:grok-session:2026-08-18:user-forbid-method-spec-refs-on-reference-delete-parent"
    status: proposed
---
# 親が reference / delete のとき子 Method の spec_refs を禁止する

## Context

`MethodDeclaration` は独立した `action` を持ち、親の trait / type / function entry の `action` を継承しない（[`2026-08-17-0340-method-declaration-action.md`](2026-08-17-0340-method-declaration-action.md) D1）。
Add / Modify method の `spec_refs` は非空である（同 ADR D2）。
義務導出は Add / Modify のみ、Reference は変更面ではないため edge 宇宙の外、Delete は 0 件である（[`2026-07-02-0359-test-obligation-and-fulfillment-gate.md`](2026-07-02-0359-test-obligation-and-fulfillment-gate.md) D13）。

親が `reference` / `delete` のまま子 method に `add` / `modify` を付けると、親は変更面の外なのに子の `spec_refs` だけが残る。この組み合わせを構造的に捨てる。

## Decision

### D1: 親が reference / delete のとき子 Method の spec_refs は空でなければならない

親が `reference` / `delete` のとき、子 `MethodDeclaration` の `spec_refs` は空でなければならない。非空なら棄却する。

親とは次の immediate container の `action` である:

- `TraitEntry.methods` → その `TraitEntry.action`
- `TypeEntry.methods` → その `TypeEntry.action`
- `inherent_impls[].methods` → コマンドが指名する同一 catalogue 内の所属 `TypeEntry.action`（`type_name` で解決。見つからなければ fail-closed）

0340 D2 により Add / Modify method は非空 `spec_refs` が必要なので、この規則の結果として親が `reference` / `delete` のとき Add / Modify method は宣言できない。

0340 D1（独立 `action`・省略時 `add`・親を継承しない）は変えない。変えるのは合法な組み合わせだけ。
適用範囲は 0340 D3 と同じ（コマンドが指名する catalogue だけ）。

## Rejected Alternatives

- **子の Add / Modify `spec_refs` を edge 宇宙に入れ、親と子の混在 action を残す** — 親はなお「変更面ではない」と主張する。子 method の約束だけが edge 宇宙に残るため却下。

## Consequences

- 良: 親が変更面外なのに子 method の `spec_refs` だけが残る不整合を、宣言時点で棄却できる。
- 負: 親を `reference` / `delete` にしたまま子 method を Add / Modify できない。

## Reassess When

- 参照または削除対象の親に対して、子 method の追加・変更を正当に宣言する必要が繰り返し現れたとき。

## Related

- [`2026-08-17-0340-method-declaration-action.md`](2026-08-17-0340-method-declaration-action.md) D1 / D2 — 本 ADR はこれらを refine する。組み合わせ制約であり、親 `action` の継承ではない。D1 の独立 `action` は維持する。
- [`2026-07-02-0359-test-obligation-and-fulfillment-gate.md`](2026-07-02-0359-test-obligation-and-fulfillment-gate.md) D13 — 本 ADR は D13 に method 粒度で従う。Add / Modify のみ導出、Reference は edge 宇宙の外、Delete は 0 件、を再決定しない。
