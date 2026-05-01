---
adr_id: 2026-05-02-0316-enum-variant-payload-schema
decisions:
  - id: D1
    user_decision_ref: "chat_segment:enum-variant-payload-schema-adr-promotion:2026-05-02"
    status: proposed
---
# enum variant の payload 型を schema レベルで宣言可能にする

## Context

現行の schema は `MemberDeclaration::Variant(String)` で variant 名のみを保持する
(`libs/domain/src/tddd/catalogue.rs` の `MemberDeclaration` 定義)。
同箇所の doc comment には次の記載がある:

> "An enum variant: only a name is tracked at L1 (payload types are out of scope until L2)."

つまり「L2 で扱う」と明示的に先送りされたままの未解消事項である。
`Enum` / `ErrorType` の `expected_variants` も名前文字列のみを保持する構造は変わらない。

影響: `AdrDecisionEntry { Proposed(ProposedDecision), Accepted(AcceptedDecision), ... }` の
ように variant が他の catalogue 登録型を payload に持つ場合、その関係を catalogue に
記述できない。enum 型と payload 型の双方が graph 上で孤立する。
`DecisionGrounds { UserDecisionRef(...), ReviewFindingRef(...) }` 等の
payload 持ち `ErrorType` / `Enum` 全般に同じ構造が存在する。

この変更は ADR `2026-04-26-0855` の Core invariant を発動させる:
「catalogue schema 拡張 / TypeGraph schema 拡張 / baseline schema 拡張は同時に決め、
同時に実装する」。schema 4 点 (catalogue schema / TypeGraph / baseline / serde codec)
の同時更新が必要な重い拡張である。

## Decision

### D1: `MemberDeclaration::Variant` の構造変更と schema 4 点同時更新

#### schema 拡張 — `MemberDeclaration::Variant` の構造変更

`MemberDeclaration::Variant(String)` を構造体 variant に拡張する:

```rust
// <!-- illustrative, non-canonical -->
// 変更前
MemberDeclaration::Variant(String)  // variant 名のみ

// 変更後 (EnumVariantDeclaration を導入)
pub struct EnumVariantDeclaration {
    pub name: String,
    pub payload_types: Vec<String>,  // unit variant は空 Vec
}
MemberDeclaration::Variant(EnumVariantDeclaration)
```

- `Enum { expected_variants: Vec<EnumVariantDeclaration> }` に移行する。
- `ErrorType { expected_variants: Vec<EnumVariantDeclaration> }` も同様に移行する。
- payload を持たない unit variant は `payload_types: []` で表現する。
- payload を持つ variant は `payload_types: ["ProposedDecision", "AdrDecisionMetadata"]`
  のように完全な型文字列のリストで宣言する (generic 引数を含む完全形を要求)。
- tuple variant の複数 payload および struct variant のフィールド型はすべて
  `payload_types` に列挙する。

宣言例:

```
<!-- illustrative, non-canonical -->
{
  "name": "AdrDecisionEntry",
  "kind": "enum",
  "expected_variants": [
    { "name": "Proposed",     "payload_types": ["ProposedDecision"] },
    { "name": "Accepted",     "payload_types": ["AcceptedDecision"] },
    { "name": "Implemented",  "payload_types": ["ImplementedDecision"] },
    { "name": "Withdrawn",    "payload_types": [] }
  ]
}
```

#### TypeGraph schema 拡張

TypeGraph 側の `TypeNode::members` は `MemberDeclaration` の変更に追従し、
variant payload 情報を保持できるよう拡張する。

#### baseline schema 拡張

`TypeBaselineEntry::members` は `MemberDeclaration` 拡張に追従する。
新 schema は本 ADR 適用後に authored される新規 track の catalogue にのみ適用する。
過去 track の既存 catalogue は旧 schema のまま歴史的記録として保持する
(backward compat はプロジェクト方針として持たないため、一括変換作業は行わない)。

#### codec (serde) 拡張

`MemberDeclaration::Variant` の serde 表現を
`{ "name": "...", "payload_types": [...] }` 形式に変更する。
serde codec は新 schema 専用とし、旧 schema (`String` 形式) を読む経路は持たない。
既存 catalogue JSON の書き換えは行わない。

#### renderer 拡張

本 ADR の schema 拡張に対して、2 つの renderer がそれぞれ以下の方針で
enum → payload type edge の描画に対応する。

##### Contract Map renderer (catalogue 入力)

catalogue の `expected_variants[].payload_types` の各 type token を
`type_index` で resolve し、enum → payload type への edge を描画する。

##### Reality View renderer (TypeGraph / baseline 入力)

ADR `2026-04-16-2200-tddd-type-graph-view.md` D2 (b) は
「variant payload 情報は L1 時点では不足 — L2 で variant fields が入るまでは
variant 名のみ」と定めていた。本 ADR の schema 拡張により
`TypeNode::members` および `TypeBaselineEntry::members` の
`MemberDeclaration::Variant` が `payload_types` を保持するようになるため、
この制約は本 ADR によって上書きされる。

Reality View renderer は `TypeNode::members` の `MemberDeclaration::Variant` を
走査し、`payload_types` に列挙された各 type token を resolve して
enum → payload type への edge を描画する。これにより TypeGraph / baseline 由来の
可視化でも Contract Map renderer と同じ意味論の edge が生成される。

##### 両 renderer 共通の mermaid 表現

```
<!-- illustrative, non-canonical -->
%% variant payload edge (Contract Map / Reality View 共通)
AdrDecisionEntry -->|::Proposed| ProposedDecision
AdrDecisionEntry -->|::Accepted| AcceptedDecision
```

`::VariantName` を label に含めることで、通常の field edge (`-->|.field_name|`) と区別する。

## Rejected Alternatives

### A1: enum variant の payload は宣言せず field edge で代用する

`AdrFrontMatter.decisions: Vec<AdrDecisionEntry>` のような外部参照フィールドから
間接的に関係を示す案。

**却下理由**: field edge は「ある型がある型をフィールドに持つ」という関係のみを表現する。
enum 自身が「どの payload 型を選択肢として保持するか」という構造情報は field edge では
表現できない。variant の選択肢関係は field の包含関係とは意味論が異なるため代用にならない。

### A2: `payload_types` を `Vec<String>` ではなく `Option<String>` にして単一 payload のみ対応する

variant が保持できる payload を 1 型のみに制限する案。

**却下理由**: Rust の enum variant は tuple variant (`Foo(A, B, C)`) や
struct variant (`Foo { a: A, b: B }`) で複数の型を payload に持てる。
単一 payload に絞ると tuple variant が schema で表現できない。
「完全な型情報を宣言する」という方針とも矛盾する。

## Consequences

### 良い影響

- enum-first パターンの「どの variant がどの型を選択肢として保持するか」という構造情報が
  catalogue で完結するようになり、enum 型と payload 型の孤立が解消される。

### 悪い影響・トレードオフ

- schema 4 点 (catalogue schema / TypeGraph / baseline / serde codec) の同時更新が必要で
  実装コストが大きい。これが実装コストの主因であり、既存 catalogue の書き換えは含まない。
- 過去 track の既存 catalogue は旧 schema (`expected_variants: ["Proposed", ...]` 形式)
  のまま歴史的記録として残る。新 schema の serde codec では読めないため、旧 catalogue を
  参照する場合は手動での読み替えが必要になる。

## Reassess When

- `payload_types` 宣言が tuple variant / struct variant の複雑性で混乱を招いた場合:
  表記の簡略化や別フォーマットへの再検討を行う。
- payload 型を持たない unit variant のみで構成される enum が大半の場合:
  schema 拡張コスト (4 点同時更新) に見合う可視化効果があるか実績をもとに再評価する。

## Related

- `knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md` —
  本 ADR の主テーマである struct kind 均質化の元 ADR。enum variant payload schema は
  そこから spin-off した独立 decision。
- `knowledge/adr/2026-04-26-0855-tddd-feature-extension-with-verification.md` —
  Core invariant (schema 4 点同時更新) を確立した前駆 ADR。本 ADR はその invariant を
  発動させる重い拡張として位置づけられる。
- `knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md` —
  TypeGraph (Reality View) の元 ADR。本 ADR による TypeGraph schema 拡張
  (`TypeNode::members` の variant payload 対応) の根拠コンテキスト。
  同 ADR D2 (b) の「variant 名のみ」制約は本 ADR によって上書きされ、
  Reality View renderer も variant payload に基づく edge 描画が可能になる。
- `knowledge/adr/2026-04-13-1813-tddd-taxonomy-expansion.md` —
  `TypeDefinitionKind` taxonomy の ADR。`Enum` / `ErrorType` の元定義。
