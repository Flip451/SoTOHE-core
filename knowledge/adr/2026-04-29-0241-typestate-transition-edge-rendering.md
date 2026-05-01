---
adr_id: 2026-04-29-0241-typestate-transition-edge-rendering
decisions:
  - id: D1
    user_decision_ref: "chat_segment:tddd-struct-kind-spinoff:2026-04-29"
    status: proposed
---
# typestate 遷移を contract-map に描画する renderer 拡張

## Context

catalogue schema には `TypestateTransitions` 型が存在し、
`Typestate { transitions: TypestateTransitions, expected_members: ... }` の形で宣言される
(`libs/domain/src/tddd/catalogue.rs` の `TypestateTransitions` 定義)。

`TypestateTransitions` の variant は次の 2 種:

- `To(Vec<String>)` — 遷移先となる typestate 名前のリスト
- `Terminal` — 終端状態 (遷移先なし)

ところが main の `libs/domain/src/tddd/contract_map_render.rs` の `methods_of()` は
`Typestate` に空の Vec を返し、`transitions: TypestateTransitions::To(...)` を一度も読まない。

結果として、typestate 同士の遷移関係 (例:
`ProposedDecision -> AcceptedDecision -> ImplementedDecision`) がカタログで正しく
宣言されているのに、contract-map 上では遷移先へのリンクが出ず typestate ノードが
孤立して並ぶ状態になる。

これは「schema は存在するが renderer が未対応」という構造で、ADR `2026-04-26-0855`
§D2 (expected_members フィールド宣言の必須化) と同型のギャップである。

renderer のみの変更で解消できるため、schema 拡張・codec 追加・baseline 拡張は不要。

## Decision

### D1: `TypestateTransitions::To(names)` から transition edge を生成する renderer 拡張

#### edge の mermaid 表現

typestate transition edge は既存の method-call edge (`-->`) と impl edge (`-.impl.->`)
と視覚的に区別できる表記を用いる。太線 `==>` を選択する:

```
<!-- illustrative, non-canonical -->
%% method edge (既存)
Foo -->|.some_method()| Bar

%% impl edge (既存)
Foo -.impl.-> Bar

%% typestate transition edge (本 ADR 決定)
ProposedDecision ==>|transitions_to| AcceptedDecision
```

3 種の edge が視覚的に明確に区別される。

#### 実装上の制約

- `TypestateTransitions::To(names)` の各 `name` を type_index で resolve できれば
  edge を描画する。catalogue 未登録の遷移先名前は silently skip する (CN-08 ノイズ抑制)。
- `TypestateTransitions::Terminal` の場合は edge を描画しない。
- self-loop は抑制する (現行 method edge と同じ self-loop 抑制ロジックを適用)。
- renderer 側のみの変更であり、schema 拡張・codec 追加・baseline 拡張は不要。

## Rejected Alternatives

### A1: typestate 遷移を contract-map に出さず Type Graph View に任せる

typestate 遷移の可視化を Type Graph View (Reality View) のみに担わせ、
contract-map では描画しない案。

**却下理由**: contract-map は「カタログに宣言された関係」を視覚化するビューである。
カタログ上に `transitions: To([...])` として存在する情報を contract-map に反映しないのは
情報損失である。Type Graph View は impl に基づく Reality View として別の目的を持ち、
catalogue-declared な関係の肩代わりにはならない。

## Consequences

### 良い影響

- typestate で設計された型の状態遷移が contract-map 上に可視化され、
  typestate 設計の構造が一目で確認できるようになる。
- renderer のみの変更で済むため実装コストが小さい。

### 悪い影響・トレードオフ

- typestate 遷移 edge が密になりすぎてグラフが読めなくなる場合がある。
  その際は kind フィルタや edge 種別フィルタによる表示制御の強化を検討する。

## Reassess When

- typestate 遷移 edge が密になりすぎてグラフが読めなくなった場合:
  kind フィルタや edge 種別フィルタによる表示制御の強化を検討する。

## Related

- `knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md` —
  本 ADR の主テーマである struct kind 均質化の元 ADR。typestate transition edge は
  そこから spin-off した独立 decision。
- `knowledge/adr/2026-04-26-0855-tddd-feature-extension-with-verification.md` —
  expected_members フィールド宣言の必須化と Core invariant を確立した前駆 ADR。本 ADR は
  同型のギャップ (schema あり / renderer 未対応) を renderer 側のみで解消する。
- `knowledge/adr/2026-04-17-1528-tddd-contract-map.md` —
  contract-map の edge 生成設計の元 ADR。
- `knowledge/adr/2026-04-13-1813-tddd-taxonomy-expansion.md` —
  `TypeDefinitionKind` / `TypestateTransitions` の taxonomy 元 ADR。
