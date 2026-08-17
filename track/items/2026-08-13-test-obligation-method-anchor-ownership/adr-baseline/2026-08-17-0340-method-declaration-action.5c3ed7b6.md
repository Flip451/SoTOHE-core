---
adr_id: "2026-08-17-0340-method-declaration-action"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:grok-session:2026-08-17:user-adopted-2026-08-17-0340-method-declaration-action"
    candidate_selection: "from:[A,B,C,D,E] chose:A"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:grok-session:2026-08-17:user-adopted-2026-08-17-0340-method-declaration-action"
    candidate_selection: "from:[A,B,C,D,E] chose:A"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:grok-session:2026-08-17:user-adopted-2026-08-17-0340-method-declaration-action"
    status: proposed
---
# MethodDeclaration に action を導入し、義務対象 method を明示する

## Context

`MethodDeclaration` には `spec_refs` を書ける（`TraitEntry.methods` / `TypeEntry.methods` / `inherent_impls[].methods`）。
同じ method に `action` を書く欄はない。
`spec_refs` で grounding できるのに意図（`add` / `modify` / `reference` / `delete`）を宣言できないのは不合理であるため、method には独立した `action` が必要である。

## Decision

### D1: MethodDeclaration は独立した action を持つ

`MethodDeclaration.action` は `add` / `modify` / `reference` / `delete` のいずれかとする。
method の `action` の義務ゲート上の意味は、[`2026-07-02-0359-test-obligation-and-fulfillment-gate.md`](2026-07-02-0359-test-obligation-and-fulfillment-gate.md) D13 の ItemAction と同じであり、粒度だけが method になる。
この規則は共有型 `MethodDeclaration` を使う `TraitEntry.methods`、`TypeEntry.methods`、`inherent_impls[].methods` のすべての method に適用する。
method の `action` を省略した場合は、entry の `action` と同じく `add` とする。
親の trait・type・function entry の `action` は継承しない。

- `add`: method の追加。
- `modify`: 既存 method の変更。
- `reference`: method の形状参照。
- `delete`: method の削除。

### D2: Add/Modify method の spec_refs は非空とする

- D3 が適用対象とする catalogue では、Add/Modify method の `spec_refs` を非空とし、空なら棄却する。
- `2026-08-13-1720-test-obligation-method-anchor-ownership.md` D1 の「兄弟 method が全 entry anchor を持てば `spec_refs` を省略できる」という規則を廃止する。

### D3: 後方互換は要求しない

後方互換は要求しない。
新しい schema（method `action`、Add/Modify の非空 `spec_refs`）は、この決定を実装した作業およびそれ以降の作業が書く catalogue にだけ課す。
コマンドが指名しない既存 catalogue を直したり、失敗させたりしない。

## Rejected Alternatives

- **B: 省略を許したまま、anchor のない義務を skip-missing で通す** — method を未 grounding のまま検証から隠せる。
- **C: 省略を許したまま、自発的な結び付けまたは waiver を必須化する** — 仕様の欠落を運用記録で補うことになる。
- **D: 省略を許したまま、空な `spec_refs` から義務を導出しない** — 意図を明示せず method を gate から隠せる。
- **E: method の `action` を inherent impl に限る、または親から継承する** — 同じ entry 内の method を個別に分類できない。

## Consequences

- 良: 作業対象と形状参照の意図が明確になる。
- 負: schema と検証系の変更が必要になる。

## Reassess When

- method の変更意図を source diff から安定して導出できるとき。
- `delete` method に削除前契約の検証義務が必要になるとき。

## Related

- [`2026-07-02-0359-test-obligation-and-fulfillment-gate.md`](2026-07-02-0359-test-obligation-and-fulfillment-gate.md) D13 — この ADR は method に `action` を載せる。Add/Modify のみ導出、Reference は edge 宇宙の外、Delete は 0 件、は D13 に従う。再決定しない。
- [`2026-08-13-1720-test-obligation-method-anchor-ownership.md`](2026-08-13-1720-test-obligation-method-anchor-ownership.md) D1 — method 単位の anchor ownership と entry 参照の全体 coverage を維持する。
- [`2026-08-13-1720-test-obligation-method-anchor-ownership.md`](2026-08-13-1720-test-obligation-method-anchor-ownership.md) D2 — fulfillment を義務が所有する anchor に限る規則を維持する。
