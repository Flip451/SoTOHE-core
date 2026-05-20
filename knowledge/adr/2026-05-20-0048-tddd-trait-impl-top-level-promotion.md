---
adr_id: 2026-05-20-0048-tddd-trait-impl-top-level-promotion
decisions:
  - id: D1
    user_decision_ref: "chat_segment:make-catalogue-schema-permissive-2026-05-19:2026-05-20"
    candidate_selection: "from:[typeentry-trait-impls-field,top-level-trait-impls-vec,traitentry-external-self-impls] chose:top-level-trait-impls-vec"
    status: accepted
  - id: D2
    user_decision_ref: "chat_segment:make-catalogue-schema-permissive-2026-05-19:2026-05-20"
    status: accepted
  - id: D3
    user_decision_ref: "chat_segment:make-catalogue-schema-permissive-2026-05-19:2026-05-20"
    status: accepted
  - id: D4
    user_decision_ref: "chat_segment:make-catalogue-schema-permissive-2026-05-19:2026-05-20"
    status: accepted
---
# TDDD: `TraitImplDeclV2` を `CatalogueDocument` の top-level コレクションに並列化し、impl block を独立 entry として扱う

## Context

ADR `2026-05-08-0248-tddd-catalogue-layer-schema-axis-separation.md` の D7 (illustrative block) で `TypeEntry` 構造の一部として `trait_impls: Vec<TraitImplDeclV2>` フィールドが導入された。この設計では、TypeEntry に attach された TraitImplDeclV2 は「その親 TypeEntry の型に対する trait impl」を表現する。つまり「impl の対象型 (`for_`) = 親 TypeEntry の型」という対応が暗黙に組み込まれていた。さらに TypeEntry は自 catalogue の型 (= 自 crate 型) のみを保持するため、結果として impl の対象型は **必ず自 crate の型** になるという制約が発生していた。

しかし Rust の impl block (`impl<G> Trait for Type`) は trait と for_type が対等な関係を持つ独立した entity であり、片方を「親」とするのは概念的に不正確である。さらに、D6 (1 catalogue = 1 crate) との組み合わせで以下の構造的欠陥が露呈する。

- ケース A (`impl From<external_crate::ExtType> for SelfType`): trait 外部 + for_ 自 crate → 自 crate TypeEntry に attach 可能、declare 可能
- ケース B (`impl MyTrait for std::vec::Vec<i32>`): trait 自 crate + for_ 外部 → 自 crate TypeEntry が catalogue に存在しないため attach 先がなく declare 不可能 (orphan rule では正当な Rust 表現)
- Add action のケース B では false-positive Red (CMinusSUnionD) を不可避に生む

旧 ADR `2026-05-18-1223-make-catalogue-schema-permissive.md` の D4 (cross-crate impl filter 撤廃) で、既存コードベースの cross-crate impl false-positive Red は filter 撤廃で対称化 (Blue) として解消したが、Add action の新規 cross-crate impl は依然 declare 手段が欠如しているため D4 のスコープから外していた。本 ADR はこの構造的欠陥を解消し、impl block を catalogue の独立 entry として扱う設計に移行する。

旧 ADR `2026-05-18-1223-make-catalogue-schema-permissive.md` D2 (`InherentImplDeclV2` を `CatalogueDocument.inherent_impls` top-level Vec として導入) の対称展開でもあり、catalogue の構造的一貫性を保つ。

## Decision

### D1: `CatalogueDocument.trait_impls` を top-level Vec として導入し、`TypeEntry.trait_impls` を廃止する

```rust
// <!-- illustrative, non-canonical -->
struct CatalogueDocument {
    schema_version: u32,
    crate_name: CrateName,
    layer: LayerId,
    types: BTreeMap<TypeName, TypeEntry>,
    traits: BTreeMap<TraitName, TraitEntry>,
    functions: BTreeMap<FunctionPath, FunctionEntry>,
    inherent_impls: Vec<InherentImplDeclV2>,   // 旧 ADR D2 で導入
    trait_impls: Vec<TraitImplDeclV2>,         // 本 ADR D1 で top-level 化
}

struct TypeEntry {
    action: ItemAction,
    role: DataRole,
    kind: TypeKindV2,
    methods: Vec<MethodDeclaration>,
    // trait_impls: Vec<TraitImplDeclV2> ← 廃止
    module_path: ModulePath,
    docs: Option<String>,
    spec_refs: Vec<SpecRef>,
    informal_grounds: Vec<InformalGroundRef>,
}
```

<!-- illustrative, non-canonical -->

旧設計では `TypeEntry.trait_impls` が「親 TypeEntry の自 crate 型に対する trait impl」を暗黙に表現していたが、新設計では impl block を catalogue の独立 entry として top-level に配置し、TypeEntry / TraitEntry と同列に扱う。

### D2: `TraitImplDeclV2` schema を `trait_ref: TypeRef` + `for_type: TypeRef` の 2 軸対称構造に統一し、独立 entry として `action: ItemAction` フィールドを持たせる

```rust
// <!-- illustrative, non-canonical -->
struct TraitImplDeclV2 {
    action: ItemAction,           // Add / Modify / Reference / Delete。top-level 化で親を持たないため自身が action を宣言する。serde default は Add (他の entry と共通)
    trait_ref: TypeRef,           // trait の参照 (例: "core::convert::From<MyError>"、"std::fmt::Display"、"FnOnce<(A,), B>")
    for_type: TypeRef,            // 自 crate / 外部 crate どちらも対称 (例: "SelfType"、"std::vec::Vec<i32>")
    impl_generics: Vec<MethodGenericParam>,
    impl_where_predicates: Vec<WherePredicateDecl>,
}
```

<!-- illustrative, non-canonical -->

`action` フィールドを `TraitImplDeclV2` に追加する理由は trait impl と inherent impl の非対称性に起因する。`InherentImplDeclV2` は action を持たない — inherent impl は定義上ただ 1 つの自 crate 型に属し、その型の `TypeEntry.action` から action を継承するため、自身で宣言する必要がない。一方 `TraitImplDeclV2` は D1 の top-level 化により親 `TypeEntry` を持たない独立 entry となった。action を継承できる親が存在しないため、`TypeEntry` / `TraitEntry` / `FunctionEntry` と同様に自身が `action: ItemAction` を宣言する必要がある。

`trait_ref` / `for_type` は両方とも TypeRef 表現 (旧 ADR `2026-05-08-0258-tddd-typegraph-hybrid-and-codec.md` D5 / D11 の crate prefix 慣習に従う):

- `trait_ref`: trait の参照を fully-qualified path で記述する。自 crate trait は短名 (例: `"MyTrait"`)、外部 crate trait は crate prefix 付き fully-qualified path (例: `"core::convert::From<MyError>"`、`"std::fmt::Display"`)。trait の generic argument 列 (`<...>` 内) も同じ TypeRef 文字列に含める。
- `for_type`: impl の対象型を TypeRef で記述する。自 crate type は短名 (例: `"SelfType"`)、外部 crate type は crate prefix 付き fully-qualified path (例: `"std::vec::Vec<i32>"`)。

A-codec は `trait_ref` と `for_type` の両方を `syn::parse_str::<syn::Type>` で parse し、D5 自動 build により `external_crates` に登録 (外部 crate の場合) または自 catalogue 内 lookup (自 crate の場合) を行う。trait identity (旧 ADR `2026-05-08-0248` D10) は `trait_ref` の 1 フィールドで完全に表現される (旧 schema の `trait_name + origin_crate` の 2 フィールド分割は廃止)。`generic_args` フィールドも `trait_ref` 内に統合されるため廃止。

ケース A (`impl From<external_crate::ExtType> for SelfType`) / ケース B (`impl MyTrait for std::vec::Vec<i32>`) / 自 crate type への impl のすべてが、同じ `trait_ref` + `for_type` の 2 軸対称構造で declare 可能になる。

### D3: 信号評価器の `for_is_external` filter を C 側で撤廃し、cross-crate impl を対称扱いする

旧 ADR `2026-05-18-1223-make-catalogue-schema-permissive.md` の D4 で削除予定だった signal_evaluator_v2 の `for_is_external` filter 撤廃は本 ADR の D3 に統合する。Top-level 化された trait_impls は S 側で全て扱われ、C 側も filter 撤廃により対称な状態となる。これにより既存コードベースの cross-crate impl も新規 (Add action) の cross-crate impl も同じパスで Blue 化する。

なお、filter 撤廃自体は本 ADR より前に先行実装済みであり、本 D3 はその decision を明示する。新 ADR の実装段階で top-level 化と一体の設計として記録する。

### D4: A-codec / signal evaluator の orphan impl 経路を統合し、各 entry の宣言 action に従って S へ挿入する

旧設計では trait impl の処理経路が以下の 2 つに分岐していた。

- `TypeEntry.trait_impls` 経由 (親 TypeEntry の context で処理、parent_id を伝播)
- `phase1/builder.rs` の orphan pass (parent を持たない impl は B 側 baseline からのみ insert)

新設計では `CatalogueDocument.trait_impls` を独立 entry として処理し、orphan pass と統一する。

- A-codec は `CatalogueDocument.trait_impls` を loop で処理し、各 impl の `trait_ref` と `for_type` を syn で parse して resolve する (自 crate / 外部 crate の判別含む)
- trait impl が top-level entry になることで、現状 A 側で必要だった orphan-impl 検出処理 (`TypeAlias` など `impls` フィールドを持たない型の impl を、親型を辿る通常の traversal とは別に拾い上げる pass) は不要になる。すべての trait impl が同一の top-level loop で統合スコープ S に挿入される
- 外部 crate trait / 外部 crate type は ADR `2026-05-08-0258-tddd-typegraph-hybrid-and-codec.md` D5 の自動 build により external_crates に登録される
- catalogue (A) と baseline (B) を統合スコープ S に取り込む際、両者の独立した id 空間が衝突しないように A の全 item に fresh な S id を pre-allocate し、各 item 内の型参照 id を一括で付け替える id 再割り当て機構 (旧 ADR `2026-05-18-1223-make-catalogue-schema-permissive.md` D3 で確立) は新設計でも維持する。impl block の `for_` / `trait_` の id 解決もこの単一機構を経由する

**action-driven 挿入 (D2 の action フィールドに対応する Phase 1 処理):** top-level loop は各 `TraitImplDeclV2` を `entry.action` に従って S に挿入する。`TypeEntry` 等の他 entry と同一のセマンティクスを適用する。

- `Add`: S にまだ存在しない identity (`trait_ref + for_type`) として挿入する。baseline (B) に同一 identity が既に存在する場合は action contradiction となり、S への挿入を行わず報告する (`Add` を宣言した impl が B 側に存在するということは既存 impl を `Add` しようとしていることを意味する)。
- `Modify`: baseline (B) に同一 identity が存在することを前提に挿入する。B 側に存在しない場合は action contradiction。
- `Reference`: catalogue 側から B の impl を参照する形で S に挿入する。shape 差分があれば Red の根拠となる。
- `Delete`: A 側エントリは S へ挿入しない。代わりに B 側の同一 identity エントリを S から D (削除セット) へ移動する。B 側に存在しない場合は action contradiction。

A-codec が `entry.action` を参照せず `ItemAction::Add` を固定で使用することは禁止する。旧設計の「A-codec は常に Add として挿入」という暗黙の動作は本 D4 により明示的に廃止される。

## Rejected Alternatives

### A. `TraitImplDeclV2` に `for_` field を追加 (旧 ADR D4 草案の初期形)

`TraitImplDeclV2` に `for_: Option<String>` を追加し、None 時は parent TypeEntry の型、Some 時は fully-qualified path で declare する案。

却下理由: parent TypeEntry の暗黙依存を残しつつ Option で external case を補完する hybrid 設計。schema の非直交性が高く、A-codec の処理経路も二重 (parent attach vs override) になる。本 ADR D1 のように impl block を独立 entry として扱う方が概念的に整理されている。

### B. `TraitEntry.external_self_impls: Vec<ExternalSelfImplDecl>` を導入

自 crate の trait を外部 type に impl するケースを `TraitEntry` に専用フィールドで保持する案。

却下理由: 自 crate trait + 自 crate type の impl は `TypeEntry.trait_impls`、自 crate trait + 外部 type は `TraitEntry.external_self_impls`、外部 trait + 自 crate type は `TypeEntry.trait_impls` と、impl が 3 つの異なる場所に分散する。catalogue 構造が複雑化し、`inherent_impls` (D2) の top-level Vec 設計との対称性も崩れる。

## Consequences

### Positive

- impl block が catalogue の独立 entry として `inherent_impls` (D2) と対称な構造になり、catalogue 設計の一貫性が向上する。
- ケース A / ケース B / 自 crate type への impl のすべてが同じ schema (`TraitImplDeclV2`) で declare 可能になり、cross-crate impl の Add action も自然に表現できる。
- TypeEntry が trait_impls を持たないため、TypeEntry の責務が「型定義 (kind + methods)」に純粋化される。
- A-codec / signal evaluator の trait_impl 処理経路が単一化され (top-level loop)、parent TypeEntry の context 伝播が不要になる。

### Negative

- catalogue ファイル (domain / usecase / infrastructure-types.json) を新 schema に基づいて記述し直す必要がある。旧 schema (`TypeEntry.trait_impls` を持つ既存 catalogue) を migration する経路は実装しない。既存 catalogue は git history 上に残るが、新 commit 以降は新 schema 形式のみ有効。各 catalogue ファイルの `trait_impls` エントリには `action` フィールドの記載が必要になる (serde default を活用して省略も可能だが、明示推奨)。
- DTO / codec / A-codec / signal evaluator / linter など、`TypeEntry.trait_impls` を参照する全コード経路の修正が必要。
- A-codec の top-level trait_impl 処理ループは `entry.action` を読み取り、`TypeEntry` 等と同一の action セマンティクスで TypeGraph A へのエンコードを行うよう更新する必要がある。`ItemAction::Add` の固定使用箇所を削除する。実際の action-driven Phase-1 S-insertion (Add / Modify / Reference / Delete セマンティクスの実行) は signal evaluator Phase 1 (`phase1/builder.rs`) が担う。
- 旧 ADR `2026-05-08-0248-tddd-catalogue-layer-schema-axis-separation.md` の D7 illustrative block で示された `TypeEntry` 構造が更新される。

## Reassess When

- 新 schema 移行後、cross-crate impl の Add action / Modify action / Reference action のすべてのケースで signal 評価が期待通り Blue 化することを test で確認できなかった場合。
- catalogue 設計者が新 schema を活用して cross-crate impl を declare した際、A-codec の `for_type` parse / D5 自動 build の動作が誤判定を生む場合。

## Related

- `knowledge/adr/2026-05-08-0248-tddd-catalogue-layer-schema-axis-separation.md` — D7 (TypeEntry struct 構造) を本 ADR D1 で更新 (TypeEntry.trait_impls フィールド廃止)。D10 (TraitImplDecl の trait identity 表現) を本 ADR D2 で更新 (trait_name + origin_crate の 2 フィールド分割を撤廃し、`trait_ref: TypeRef` の 1 フィールドに統合)。
- `knowledge/adr/2026-05-08-0258-tddd-typegraph-hybrid-and-codec.md` — D11 (TypeRef crate prefix) が本 ADR D2 の `trait_ref` / `for_type` 慣習の根拠。D5 (external_crates 自動 build) を本 ADR D2 で更新 (自動 build の source field を `TraitImplDecl.origin_crate` から `TraitImplDecl.trait_ref` / `TraitImplDecl.for_type` を含む全 TypeRef の crate prefix 集約に変更)。
- `knowledge/adr/2026-05-18-1223-make-catalogue-schema-permissive.md` — 本 ADR の母体。旧 D4 (cross-crate impl filter 撤廃) を破棄し本 ADR で再構築。D1 / D2 / D3 はそのまま有効。
