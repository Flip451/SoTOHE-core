---
adr_id: 2026-04-29-0240-method-type-full-generic-declaration
decisions:
  - id: D1
    user_decision_ref: "chat_segment:tddd-struct-kind-spinoff:2026-04-29"
    status: proposed
---
# method / param 型宣言で generic 引数を含む完全な型文字列を強制する

## Context

`TypeDefinitionKind` の各 kind が持つ method / param の型フィールド:

- `expected_methods[].returns`
- `expected_methods[].params[].ty`
- `FreeFunction.expected_params[].ty`
- `FreeFunction.expected_returns`

これらに generic 引数を省略した wrapper 名のみで型を宣言するケースが発生している。
たとえば戻り値型を `"Result<AdrFrontMatter, AdrFrontMatterCodecError>"` と書くべきところを
`"Result"` のみで宣言する、`"Arc<dyn AdrFilePort>"` と書くべきところを `"Arc"` のみで
宣言する、といったパターンである。

contract-map renderer (`libs/domain/src/tddd/contract_map_render.rs`) の
`extract_type_names()` は型文字列を非英数字で split し PascalCase token を抽出する。
`"Result<AdrFrontMatter, AdrFrontMatterCodecError>"` であれば
`["Result", "AdrFrontMatter", "AdrFrontMatterCodecError"]` を返し、
catalogue 登録型 (`AdrFrontMatter` / `AdrFrontMatterCodecError`) への edge が出る。

しかし `"Result"` のみを渡すと `["Result"]` しか返らず、`Result` という名前の
catalogue エントリは存在しないため edge は一切生まれない。内部の具象型への edge が
消えることで、これらの型が graph 上で孤立ノードになる。

同じ問題が `Option` / `Vec` / `Box` / `Arc` / `Rc` / `Cow` / `BTreeMap` / `HashMap` /
`HashSet` / `BTreeSet` などの wrapper 型すべてで繰り返し発生する。

`extract_type_names` 自体は generic 引数内の PascalCase token も取り出す実装に
なっているため、宣言側で完全な型文字列を書けば edge は出る。問題は宣言の慣行にある。

## Decision

### D1: generic 引数を含む完全な型文字列で宣言する

#### 規範

以下のフィールドでは、generic 引数を省略した bare wrapper 名のみの宣言を禁止する:

- `expected_methods[].returns`
- `expected_methods[].params[].ty`
- `FreeFunction.expected_params[].ty`
- `FreeFunction.expected_returns`

禁止対象の「bare wrapper 名」:
`Result` / `Option` / `Vec` / `Box` / `Arc` / `Rc` / `Cow` /
`BTreeMap` / `HashMap` / `HashSet` / `BTreeSet`

これらが具象型を伴わず単独で宣言された場合、`extract_type_names` は wrapper 名 token
しか返さず、内部具象型への edge が生まれない。

良い例:

```
<!-- illustrative, non-canonical -->
returns: "Result<AdrFrontMatter, AdrFrontMatterCodecError>"
ty:      "Arc<dyn AdrFilePort>"
ty:      "Vec<AdrDecisionEntry>"
```

悪い例:

```
<!-- illustrative, non-canonical -->
returns: "Result"
ty:      "Arc"
ty:      "Vec"
```

#### lint ゲート

bare wrapper 名のみの宣言を catalogue の codec / verify CLI が schema validation で
reject する lint を後続作業として組み込む。実装前は設計レビューで確認する。

## Rejected Alternatives

### A1: `extract_type_names` の parser を強化して wrapper 内部を推論する

`"Result"` という宣言を受け取っても、内部型を何らかの方法で補完する実装を追加する案。

**却下理由**: 宣言が `"Result"` のままである限り parser が推論できる情報はゼロ。
改善すべき対象は parser ではなく宣言の正確性である。「型情報を推論で補う」設計は、
宣言が契約になるという TDDD の原則と矛盾する。

## Consequences

### 良い影響

- 型情報を完全に宣言することで、contract-map 上の method edge 漏れが減る。
- type-designer が型情報を省略したときに edge が消えるという問題を、
  設計レビューで早期に検出できるようになる (lint 実装後はスキーマレベルで検出可能)。

### 悪い影響・トレードオフ

- lint 実装前は人間レビューに依存する過渡期間が存在する。
- 既存 catalogue でこのルールを満たしていないエントリは書き直しが必要になる。

## Reassess When

- generic 完全型宣言 lint が誤検出を多発させる場合: 規範の緩和や例外リストの導入を
  検討する。
- `extract_type_names` の string ベース tokenize が generic 内部の token 抽出で
  edge case を起こした場合: parser の構造化を検討する。

## Related

- `knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md` —
  本 ADR の主テーマである struct kind 均質化の元 ADR。method 型宣言規範は
  そこから spin-off した独立 decision。
- `knowledge/adr/2026-04-26-0855-tddd-feature-extension-with-verification.md` —
  `expected_members` 必須化と Core invariant を確立した前駆 ADR。
- `knowledge/adr/2026-04-17-1528-tddd-contract-map.md` —
  contract-map の `extract_type_names` / edge 生成の元設計。
