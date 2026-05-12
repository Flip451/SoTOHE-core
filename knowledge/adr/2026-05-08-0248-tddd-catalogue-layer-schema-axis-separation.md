---
adr_id: 2026-05-08-0248-tddd-catalogue-layer-schema-axis-separation
decisions:
  - id: D1
    user_decision_ref: "chat_segment:tddd-v2-axis-separation-design:2026-05-08"
    candidate_selection: "from:[single-enum,3-axis-separation,3-axis-with-entry-separation-encoding] chose:3-axis-with-entry-separation-encoding"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:tddd-v2-axis-separation-design:2026-05-08"
    candidate_selection: "from:[A-single-role-enum,B-3-enum-split] chose:B-3-enum-split"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:tddd-v2-axis-separation-design:2026-05-08"
    candidate_selection: "from:[B-flat-pattern-field,Q4.1-strict-payload-encoded,transitions_to-via-type-names,transition_methods-via-method-names] chose:Q4.1-strict-payload-encoded+transition_methods-via-method-names"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:tddd-v2-axis-separation-design:2026-05-08"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:tddd-v2-axis-separation-design:2026-05-08"
    status: proposed
  - id: D6
    user_decision_ref: "chat_segment:tddd-v2-axis-separation-design:2026-05-08"
    candidate_selection: "from:[C-i-1to1,C-ii-p-crate-per-file,C-ii-q-layer-per-file] chose:C-ii-p-crate-per-file"
    status: proposed
  - id: D7
    user_decision_ref: "chat_segment:tddd-v2-axis-separation-design:2026-05-08"
    candidate_selection: "from:[module-path-optional,module-path-required-with-default,module-path-required-no-default] chose:module-path-required-with-default"
    status: proposed
  - id: D8
    user_decision_ref: "chat_segment:tddd-v2-axis-separation-design:2026-05-08"
    status: proposed
  - id: D9
    user_decision_ref: "chat_segment:tddd-v2-axis-separation-design:2026-05-08"
    status: proposed
  - id: D10
    user_decision_ref: "chat_segment:tddd-v2-axis-separation-design:2026-05-08"
    candidate_selection: "from:[required-with-methods,optional-with-methods,identity-only-no-methods] chose:identity-only-no-methods"
    status: proposed
  - id: D11
    user_decision_ref: "chat_segment:tddd-v2-axis-separation-design:2026-05-08"
    status: proposed
  - id: D12
    user_decision_ref: "chat_segment:tddd-v2-axis-separation-design:2026-05-08"
    candidate_selection: "from:[payload_types-flat,VariantPayload-3-variants] chose:VariantPayload-3-variants"
    status: proposed
---
# TDDD 型カタログ: kind / role / pattern / action 軸分離と厳密 payload-encoded schema

## Context

### §1 発端: TypeDefinitionKind の軸混在問題

`TypeDefinitionKind` enum は Language 要素 (struct / enum / trait / function)、Role (DDD/Clean Architecture 上の役割)、Pattern (実装パターン) を 1 つの enum に flatten していた。その結果、`Typestate` (実装パターン) と `Entity` / `AggregateRoot` (役割ラベル) を同じ enum で並置するという axis 混在が生じていた。

この問題は、ADR `2026-04-29-1653-aggregate-entity-kind-representation.md` (spin-off ADR) で集約 / entity 用 kind の新設を議論した過程で表面化した。「軸を整える前に役割を増やすのは順序が逆」という判断から、spin-off ADR を採否保留に戻し、本 ADR で軸分離を先行して決定することとした (Path 4 採用)。

### §2 parent ADR の M1 / S1 との連続性

parent ADR `2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md` は以下を決定した:

- M1: 全 struct 系 kind に `expected_methods` を均質化する
- S1: `domain_service` kind を新設する
- S2: contract-map renderer の `methods_of()` を全 struct kind 対応に拡張する
- S3: type catalogue linter framework の導入を決定する

本 ADR はこれらを継承しつつ、TypeDefinitionKind の単一 enum 構造を 3 軸 (Language / Role / Layer) に再設計する。M1 の「common fields (expected_methods)」は TypeEntry の top-level field として継承し、S1 の `domain_service` は DataRole の値として継承する。

### §3 Catalogue schema の設計選択

「内部表現 = rustdoc_types::Crate / Catalogue = 書きやすさ重視の軽量独自 schema」という方針のもと、既存 TypeGraph スタイル (Type / Trait / Function の 3 分離 + inline 展開) を Catalogue schema の出発点として採用した。これは既存資産の活用、書きやすさ、実装コスト最小の 3 点から選択した (α 案採用)。

なお TypeGraph (内部表現) は別 ADR の対象とする。本 ADR は Catalogue layer schema のみを決定する。

## Decision

### D1: 3 軸分離 (Language by entry-separation / Role / Layer)

型カタログの記述軸を以下の 3 つに分離する。

**Language 軸 (3 値: DataType / Contract / Function)**:

Language 軸の 3 値は Rust 4 構成要素 (struct / enum / trait / function) を 3 値に圧縮した Rust 固有の設計である。actual schema には `Language` enum も `DataKind` enum も導入しない。代わりに、D6 で定義する `CatalogueDocument` の 3 つの BTreeMap によって entry 種別を分離し、どの map に格納されるかで Language 値を parse-time に一意決定する:

- `types: BTreeMap<TypeName, TypeEntry>` — Language = DataType (struct / enum / type alias)
- `traits: BTreeMap<TraitName, TraitEntry>` — Language = Contract
- `functions: BTreeMap<FunctionPath, FunctionEntry>` — Language = Function (Catalogue layer のみで first-class 保持)

`TypeEntry` 等に `language` フィールドは持たない。Language 値の取得は「どの map に入っているか」によって決まる。

DataType 内の Composite / Sum 区別 (旧 DataKind) は、`TypeEntry.kind: TypeKindV2` の variant 構造で完全に encode される:
- `TypeKindV2::UnitStruct` — フィールドなし unit struct (`pub struct Foo;`)
- `TypeKindV2::TupleStruct { fields: Vec<TypeRef>, has_stripped_fields }` — tuple/newtype struct
- `TypeKindV2::PlainStruct { fields: Vec<FieldDecl>, has_stripped_fields, typestate }` — named-field struct (typestate marker 付き可)
- `TypeKindV2::Enum { variants }` — Sum (enum 系)
- `TypeKindV2::TypeAlias { target }` — 型の別名

`TypeKindV2` の詳細は D3 / D7 で定義する。

Pattern × Kind 制約は `TypeKindV2` の 5 flat variant 構造で encode される。UnitStruct/TupleStruct に named fields を、Enum/TypeAlias に struct fields を declare しようとすると parse 段階で reject される (D3 で詳述)。

**Role 軸** — D2 で定義する 3 enum (DataRole / ContractRole / FunctionRole) に分離する。全 Role が Language を一意決定する。

**Layer 軸** — `architecture-rules.json` の `layers[].crate` フィールドと対応する `LayerId` newtype (`libs/domain/src/tddd/layer_id.rs`、ADR `2026-04-17-1528-tddd-contract-map.md` §D1 で定義済み)。`CatalogueDocument` 内に `layer: LayerId` フィールドを持ち (D6 で定義)、1 ファイル = 1 crate = 1 layer の対応を schema 上で明示する。ファイル名 (`<crate_name>-types.json`) との整合は validation で検証する。layer 値が workspace 内に実在する layer か (`architecture-rules.json` との整合性) は use-site validation (CatalogueLoader / SignalEvaluatorV2 等) で検証する。

Rust 特化を本 ADR で明示する。Language 軸の 3 値 (DataType / Contract / Function) は Rust 固有の設計であり、多言語化が必要になった場合は framework ごと fork することを前提とする (本 ADR が Rust 特化前提として確定するため、段階的な schema 開放は行わない)。

### D2: Role 軸 = 3 enum 分離

Role × Entry 種別の制約を型レベルで encode するため、Role を以下の 3 enum に分離する。

**DataRole (13 値)** — TypeEntry に attach:
- `ValueObject` / `Entity` / `AggregateRoot` / `DomainService` / `Specification`
- `Factory` / `UseCase` / `Interactor` / `Command` / `Query` / `Dto`
- `ErrorType` / `SecondaryAdapter`

**ContractRole (3 値)** — TraitEntry に attach:
- `SpecificationPort` / `ApplicationService` / `SecondaryPort`

**FunctionRole (2 値)** — FunctionEntry に attach:
- `FreeFunction` / `UseCaseFunction`

TypeEntry に ContractRole を declare しようとするとスキーマ parse 段階で reject される。Role × Entry 種別の制約はこの型分離で schema 構造により encode される。

全 Role が Language を一意決定する (Role が決まれば Language が一意に決まる)。Function に関する役割 (`FreeFunction` / `UseCaseFunction`) は FunctionRole でのみ表現し、DataRole には含めない。

### D3: Pattern 軸 = TypeKindV2 の 5 flat variant で厳密 payload-encoded

D3 の設計原則 (Q4.1 厳密 payload-encoded: Pattern × Kind 制約を schema 構造で encode する) を採用しつつ、最終的な実装では `TypeKind::Struct { pattern: CompositePattern }` ではなく `TypeKindV2` の 5 flat variant 構造を採用した。5 variant にすることで「不正な状態が型として表現不可能」になる:

```
<!-- illustrative, non-canonical -->
enum TypeKindV2 {
    UnitStruct,                                                                 // フィールドなし
    TupleStruct { fields: Vec<TypeRef>, has_stripped_fields: bool },            // 位置フィールドのみ
    PlainStruct { fields: Vec<FieldDecl>, has_stripped_fields: bool,
                  typestate: Option<TypestateMarker> },                         // named フィールド + optional typestate
    Enum { variants: Vec<VariantDecl> },
    TypeAlias { target: TypeRef },
}
```

各 kind が持てるフィールドは variant 構造で完全に制約される: UnitStruct にフィールドは存在できず、TupleStruct には `Vec<TypeRef>` のみ (named フィールド不可)、PlainStruct には `Vec<FieldDecl>` (positional フィールド不可)。旧設計で `CompositePattern` が担っていた Plain / Typestate / Newtype の区別は以下に再マッピングされる:
- `CompositePattern::Plain` (fields あり) → `PlainStruct`
- `CompositePattern::TypestateState` → `PlainStruct { typestate: Some(TypestateMarker { state_name, transitions }) }`
- `CompositePattern::Newtype` / tuple 系 → `TupleStruct`
- フィールドなし unit struct → `UnitStruct`

`TypestateMarker` は `state_name: TypeName` (cluster を特定する型名) と `transitions: TypestateTransitions` (遷移メソッド名のリスト) を持つ。linter による typestate 整合性検証ルール (receiver 制約 / generics unwrap 範囲等) の詳細は Linter ADR で決定する。本 ADR は schema 構造のみを決定する。

Enum に fields を書いた場合、TypeAlias に variants を書いた場合はいずれも parse 段階で reject される。TupleStruct に named FieldDecl を書いた場合も同様に reject される。

### D4: Action 軸 = ItemAction を entry-level に attach

ItemAction を TypeEntry / TraitEntry / FunctionEntry の各 entry に付加する (現行 catalogue 仕様踏襲)。ADR `2026-04-11-0003-type-action-declarations.md` (TDDD-03) で決定された 4 種を継承する:

- `Add` — 型を新規追加する (省略時のデフォルト)
- `Modify` — 既存型を変更する
- `Reference` — 既存型をそのまま参照目的で declare する。**明示 declare は任意** であり、catalogue で declare されない baseline 由来型は Signal evaluator の Phase 1 (S 構築時) が暗黙 Reference として auto-resolve する (A codec は open-world、詳細は ADR `2026-05-08-0258-tddd-typegraph-hybrid-and-codec.md` D10 / ADR `2026-05-08-0305-tddd-signal-evaluator-three-way-diff.md` D2 を参照)。明示 declare は文書化 / 意図表明 / 構造変更検出 (Reference 契約のはずなのに変更されているケースを 🔴 で捕捉、ADR 3 D3) のために行う
- `Delete` — 型を意図的に削除する

action × 領域 × C (Current 実装) の状態で signal が決まる semantics は TDDD-03 から継承し、ADR `2026-05-08-0305-tddd-signal-evaluator-three-way-diff.md` (本 ADR シリーズの Signal evaluator ADR) で「11 領域 × signal table」に refine する。「領域」の正確な定義は同 ADR D3 を参照。

### D5: γ-1 newtype 系 — Identifier base + 各種 newtype wrap

「Make Illegal States Unrepresentable」原則に従い、String を直接使う箇所を newtype で包む:

```
<!-- illustrative, non-canonical -->
pub struct Identifier(String);         // Rust identifier validation 共通 base
pub struct TypeName(Identifier);
pub struct TraitName(Identifier);
pub struct FieldName(Identifier);
pub struct MethodName(Identifier);
pub struct ParamName(Identifier);
pub struct VariantName(Identifier);
pub struct CrateName(Identifier);
pub struct FunctionName(Identifier);
pub struct ModulePath(Vec<Identifier>); // module path の segment 列
pub struct TypeRef(String);             // generics を含む L1 文字列、別系統 validation
pub struct FunctionPath {
    crate_name: CrateName,
    module_path: ModulePath,            // serde default で省略可 (空 = crate root)
    name: FunctionName,
}
```

`Identifier` は共通 base として Rust identifier の validation (空文字禁止、先頭数字禁止等) を担う。`TypeRef` は generics 含む型文字列 (`"Result<Option<User>, DomainError>"` 等) を表すため別系統の validation を持つ。これにより `TypeName` を `MethodName` フィールドに渡す操作はコンパイルエラーになる。

### D6: Catalogue ファイル = `<crate_name>-types.json` (crate 単位 1 ファイル)

```
<!-- illustrative, non-canonical -->
// ファイル名: domain_core-types.json
pub struct CatalogueDocument {
    schema_version: u32,
    crate_name: CrateName,   // ファイル名と一致することを validation で検証
    layer: LayerId,           // この crate が属する layer を宣言 (architecture-rules.json 駆動)
    types: BTreeMap<TypeName, TypeEntry>,
    traits: BTreeMap<TraitName, TraitEntry>,
    functions: BTreeMap<FunctionPath, FunctionEntry>,
}
```

- 1 ファイル = 1 crate = 1 layer
- 内部に `crate_name` (ファイル名との一致検証) と `layer` (所属 layer の宣言) を持つ
- 同じ layer に複数ファイルが存在することを許容する (1 layer N crate の workspace 構成に対応)

rustdoc は crate 単位で 1 JSON を出力するため、Catalogue (TypeGraph A) も crate 単位にすると codec と突合が 1:1 比較になる。commit / diff / PR の粒度も crate と一致する。layer 全体の俯瞰は render 層 (`contract-map.md` / `<layer>-graph-dN`) で集約する。

### D7: TypeKindV2 厳密 payload-encoded — entry struct 定義

D3 で示した `TypeKindV2` の構造を改めて明示する。各 kind が持てるフィールドは variant 構造で完全に制約される:

```
<!-- illustrative, non-canonical -->
pub struct TypeEntry {
    action: ItemAction,
    role: DataRole,
    kind: TypeKindV2,                  // UnitStruct / TupleStruct / PlainStruct / Enum / TypeAlias の 5 flat variant
    methods: Vec<MethodDeclaration>,
    trait_impls: Vec<TraitImplDeclV2>,
    module_path: ModulePath,           // serde default で省略可 (空 = crate root)
    docs: Option<String>,
    spec_refs: Vec<SpecRef>,           // SoT Chain ② spec.json リンク (ADR 2026-05-11-1257 D1)
    informal_grounds: Vec<InformalGroundRef>, // 非公式根拠引用 (ADR 2026-05-11-1257 D1)
}

// TypeKindV2 は D3 に記述 (5 flat variant)

pub struct FieldDecl { name: FieldName, ty: TypeRef }

pub struct VariantDecl {
    name: VariantName,
    payload: VariantPayload,    // serde default で省略可能 (Unit)
}

pub enum VariantPayload {
    Unit,                              // default (省略時)
    Tuple(Vec<TypeRef>),
    Struct(Vec<FieldDecl>),            // FieldDecl を再利用 (DRY)
}
```

`VariantPayload` の 3 variant により、unit / tuple / struct それぞれの意味的違いが schema 構造で encode される。`VariantPayload::Unit` は serde の `#[serde(default)]` により省略可能とし、unit variant を宣言する際に `payload` フィールドを書かなくてよい。`VariantPayload::Struct` は `FieldDecl` を再利用するため、struct variant の field 名を declare できる。

宣言例 (Verdict enum):

```
<!-- illustrative, non-canonical -->
// Rust 実装
enum Verdict {
    ZeroFindings,
    FindingsRemain(Vec<StoredFinding>),
    Custom { findings: Vec<StoredFinding>, reason: String },
}

// catalogue declare (JSON)
"variants": [
  { "name": "ZeroFindings" },
  { "name": "FindingsRemain", "payload": { "Tuple": ["Vec<StoredFinding>"] } },
  {
    "name": "Custom",
    "payload": {
      "Struct": [
        { "name": "findings", "ty": "Vec<StoredFinding>" },
        { "name": "reason",   "ty": "String" }
      ]
    }
  }
]
```

Enum に `fields` を書いた場合、TypeAlias に `variants` を書いた場合はいずれも parse 段階で reject される。TypeEntry の `fields` は `kind` payload に内包されており、`methods` は kind に依らず宣言可能である。これは parent ADR M1 の「全 struct 系 kind に expected_methods を均質化」の継承形である。

TraitEntry と FunctionEntry は以下の通り:

```
<!-- illustrative, non-canonical -->
pub struct TraitEntry {
    action: ItemAction,
    role: ContractRole,
    methods: Vec<MethodDeclaration>,
    supertrait_bounds: Vec<TypeRef>,   // スーパートレイト境界 (serde default = 空 Vec)
    module_path: ModulePath,           // serde default で省略可 (空 = crate root)
    docs: Option<String>,
    spec_refs: Vec<SpecRef>,           // SoT Chain ② spec.json リンク (ADR 2026-05-11-1257 D1)
    informal_grounds: Vec<InformalGroundRef>, // 非公式根拠引用 (ADR 2026-05-11-1257 D1)
}

pub struct FunctionEntry {
    action: ItemAction,
    role: FunctionRole,
    params: Vec<ParamDeclaration>,
    returns: TypeRef,
    is_async: bool,
    docs: Option<String>,
    spec_refs: Vec<SpecRef>,           // SoT Chain ② spec.json リンク (ADR 2026-05-11-1257 D1)
    informal_grounds: Vec<InformalGroundRef>, // 非公式根拠引用 (ADR 2026-05-11-1257 D1)
}

pub struct MethodDeclaration {
    name: MethodName,
    receiver: Option<SelfReceiver>,
    params: Vec<ParamDeclaration>,
    returns: TypeRef,
    is_async: bool,
    docs: Option<String>,
}

pub struct ParamDeclaration { name: ParamName, ty: TypeRef }
```

### D8: SelfReceiver enum でメソッドの receiver を表現

MethodDeclaration の `receiver` は固定 3 値のため、enum で表現する (`.claude/rules/04-coding-principles.md` の Enum-first パターンと整合):

```
<!-- illustrative, non-canonical -->
pub enum SelfReceiver {
    Owned,        // "self"
    SharedRef,    // "&self"
    ExclusiveRef, // "&mut self"
}
```

receiver が `None` の場合は associated function (static method 相当、`Self` を引数に取らない)。

### D9: docs フィールド — top-level entry と MethodDeclaration に宣言可能

`TypeEntry` / `TraitEntry` / `FunctionEntry` / `MethodDeclaration` に `docs: Option<String>` フィールドを追加する。docs は型の意図や制約を文書化する目的で宣言可能だが、突合アルゴリズムの比較対象とはしない。

`FieldDecl` / `VariantDecl` / `ParamDeclaration` には docs を付けない (簡素化。Rust 慣行でも param doc は珍しい)。

### D10: TraitImplDecl は trait identity のみ (methods field を持たない)

TraitImplDecl は trait identity (`trait_name + origin_crate`) のみで encode し、`methods` field を持たない:

```
<!-- illustrative, non-canonical -->
pub struct TraitImplDecl {
    trait_name: TraitName,
    origin_crate: CrateName,
}
```

trait def と impl の signature 整合は Rust コンパイラが保証する範囲であり、TDDD signal evaluator が二重チェックする必要はない。突合 algorithm は trait impl の identity 一致 (catalogue declare vs current 実装) のみを判定する: declare されたが impl されていない / declare されないが impl されている のような impl identity の差のみを signal 化し、method signature の差はコンパイラに任せる。

これにより catalogue の declare コストは impl identity のみとなり、derive trait (`#[derive(Debug)]` 等) や workspace 内別 crate の trait に対する impl の宣言が最小化される。

### D11: BTreeMap key の形式

型と trait は短名を key とし、関数は FunctionPath を key とする:

```
<!-- illustrative, non-canonical -->
types:     BTreeMap<TypeName, TypeEntry>       // 短名 key + entry 内 module_path
traits:    BTreeMap<TraitName, TraitEntry>      // 短名 key + entry 内 module_path
functions: BTreeMap<FunctionPath, FunctionEntry> // full path key
```

- types / traits の短名重複は domain 概念の衝突を意味し、catalogue 内で禁止しても問題ない
- functions は `new` / `build` / `from_str` のような短名重複が日常的なため、FunctionPath で曖昧さを解消する必要がある

types/traits は短名 key + entry 内 `module_path` field、functions は full path key (FunctionPath) を採用するという declare 方式の差は、上記の短名重複頻度差を反映した合理的トレードオフである。両者を統一しようとすると、(a) 短名 key で functions が `new` / `build` 等で衝突する、または (b) types/traits も full path key にして短名 key の検索性を失う、のいずれかになる。declare 表記の見た目の非対称性を許容してでも各 entry 種別の本質に合った設計を選択する。

crate root の関数は crate name を prefix とする (`"domain_core::register_user"`)。`"crate::"` prefix (自 crate のみを指す Rust 構文) や `"::"` leading (外部 crate も含む absolute path) ではなく crate name を直接 prefix する。これにより TypeRef の cross-crate 参照表現 (`"domain_core::UserId"` 等) と統一でき、catalogue 内の path 表現が一貫する。

#### D11.1: function path key は自 catalogue の crate prefix のみを受け入れる

catalogue 内の functions map のキー (function path) は、その catalogue 自身の `crate_name` で始まる path のみが許容される。他の crate の function を catalogue で宣言することは禁止する。

例:
- `domain-types.json` の functions map: `"domain::tddd::signals::evaluate"` は allow、`"infrastructure::foo"` は reject
- `infrastructure-types.json` の functions map: `"infrastructure::baseline::capture"` は allow、`"domain::bar"` は reject

この制約は v3 catalogue schema の不可侵的な前提として codec の decode 段階で enforce する。違反を silent drop せず explicit な decode error として報告する。

本制約は ADR `2026-05-11-1257-tddd-v2-catalogue-spec-link-restoration.md` D4 で確定した。

### D12: VariantPayload による enum variant の payload 構造化

`VariantDecl` の payload 表現を `payload_types: Vec<TypeRef>` 一律から `payload: VariantPayload` (Unit / Tuple / Struct の 3 variant) に変更する。

Rust の enum variant は unit / tuple / struct の 3 種があり、それぞれ以下の特徴を持つ:

- unit variant: payload なし (`ZeroFindings`)
- tuple variant: 名前なし型の列 (`FindingsRemain(Vec<StoredFinding>)`)
- struct variant: 名前付き field の集合 (`Custom { findings: Vec<StoredFinding>, reason: String }`)

`payload_types: Vec<TypeRef>` 一律の表現では unit / tuple / struct の意味的違いが schema 構造で encode されず、struct variant の field 名を declare できない。`VariantPayload` enum でこれを解決する:

```
<!-- illustrative, non-canonical -->
pub struct VariantDecl {
    name: VariantName,
    payload: VariantPayload,
}

pub enum VariantPayload {
    Unit,
    Tuple(Vec<TypeRef>),
    Struct(Vec<FieldDecl>),
}
```

`VariantPayload::Struct` の field は `FieldDecl` を再利用する (DRY)。serde の `#[serde(default)]` により payload 省略時は `Unit` として解釈する。

この決定により Q4.1 厳密 payload-encoded 原則の適用範囲が enum variant に拡張される。sibling ADR `2026-05-02-0316-enum-variant-payload-schema.md` の `EnumVariantDeclaration { name, payload_types: Vec<String> }` は本決定で **supersede** される。

## Rejected Alternatives

### A: 単一 Role enum (Role × Entry 種別の制約は validation 段階のみ)

`DataRole` / `ContractRole` / `FunctionRole` を統合した単一の `Role` enum として扱う案。

却下理由: TypeEntry に ContractRole 相当の値を書く操作が parse 段階で reject されず、validation 段階まで到達してから弾かれる。「軸間制約を可能な限り schema 構造で encode する」原則に反する。3 enum 分離により Role × Entry 種別の制約が型レベルで encode される。

### B: flat pattern field (`pattern: Option<CompositePattern>` を TypeEntry の top-level に配置)

`TypeEntry { kind: TypeKind, pattern: Option<CompositePattern>, ... }` として Pattern と Kind を並列に置く案。Pattern × Kind 制約は validation 段階で確認する。

却下理由: D1 / D3 で採用した「軸間制約を schema 構造で encode する」原則 (Q4.1 厳密 payload-encoded) に反する。Enum や TypeAlias に pattern を declare できてしまい、parse 段階での reject ができない。TypeKind::Struct の payload に内包することで Composite 以外は Pattern を declare できない構造となる。

### C: layer 単位 1 ファイル + entry 内 crate_name field (ii-q 案)

`<layer>-types.json` を維持し、TypeEntry / TraitEntry / FunctionEntry に `crate_name` フィールドを持たせる案。

却下理由: codec が 1 catalogue ファイルから N 個の rustdoc_types::Crate を生成する複雑性が生じる。commit / diff の粒度が layer 単位で粗くなり、PR のスコープが広がりすぎる。rustdoc の出力単位 (crate 単位) とのミスマッチが突合 algorithm の複雑性を増す。

### D: crate root 関数 key に `"crate::"` prefix を使う

Rust の `crate::` 構文を関数 path key の prefix として使う案。

却下理由: `crate::` は自 crate のみを指す self-referential な Rust 構文である。1 catalogue = 1 crate (D6) であっても、TypeRef (D5) や TraitImplDecl の `origin_crate` (D10) は他 crate への参照を含むため、function path key だけ `crate::` 形式にすると、catalogue 内で「自 crate 内 path」と「他 crate 参照」の表現が分裂する。D11 で採用した crate name 直接 prefix 形式 (`"domain_core::register_user"`) は TypeRef の cross-crate 参照表現 (`"domain_core::UserId"`) と統一でき、catalogue 内表現が一貫する。

### E: crate root 関数 key に `"::"` leading prefix を使う

`"::register_user"` のように leading `"::"` を使う案。

却下理由: Rust では extern crate も含む absolute path を指し、慣用句的に特殊な意味を持つ。crate name を直接 prefix する (c) 案の方が意図が明確で TypeRef の cross-crate 参照形式と統一できる。

### F: MemberDeclaration 共通表現の維持 (field / variant を 1 enum で表現)

現行の `MemberDeclaration` enum (Field / Variant バリアント) を維持する案。

却下理由: D3 の厳密 payload-encoded (Q4.1) により TypeKind::Struct の payload に `fields: Vec<FieldDecl>`、TypeKind::Enum の payload に `variants: Vec<VariantDecl>` を分離して内包する必要がある。共通表現を維持すると Struct に variant を書く操作が parse 段階で reject できなくなる。

### G: TraitImplDecl に methods field を持たせる (Required または Optional を問わず)

`methods: Vec<MethodDeclaration>` として必須にする案、あるいは `methods: Option<Vec<MethodDeclaration>>` として Optional にする案。

却下理由 (必須化): `#[derive(Debug)]` のような derive trait で全メソッド宣言を書くのは冗長である。declare コストが大きく、trait impl のたびに全 method signature を catalogue に書き起こす必要がある。

却下理由 (Optional 化): `methods: Option<Vec<MethodDeclaration>>` で declare を任意化しても、trait def と impl の signature 整合は Rust コンパイラが保証する範囲である。TDDD signal evaluator が signature 比較する必要そのものがないため、field を持たせる実益がない。codec が `methods: None` 時に trait def を auto-derive する処理は cross-catalogue 解決を必要とし、D6 の crate 単位独立性に反する。`methods` field を持たない identity-only 案 (現 D10) を採用したため、Optional 化も同様に却下する。

### H: Language enum を CatalogueDocument に schema 化する案

`CatalogueDocument` を 1 つの BTreeMap と Language payload を持つ統合形 (例: `entries: BTreeMap<EntryKey, (Language, EntryBody)>`) に変更し、`Language` enum を schema として持たせる案。

却下理由: entry 種別ごとに具備する field が異なる (`TypeEntry` は `kind: TypeKind` を持ち、`TraitEntry` は `methods: Vec<MethodDeclaration>` を、`FunctionEntry` は `params / returns / is_async` を持つ)。Language enum を payload-encoded で記述するなら結局 entry 構造は種別ごとに分かれるため、統合の実益がない。1 BTreeMap 統合は parse-time の entry 種別分離を失い、種別ごとに異なる validation ルール (`TypeEntry` の `kind` 必須、`FunctionEntry` の `params` 必須等) を runtime check に追いやる。D6 の 3 BTreeMap 分離と D7 の TypeKind 内包設計で Language 軸は既に schema 構造で完全に encode されているため、`Language` enum は redundant である。

### I: TypestateState payload を transitions_to (型名直接列挙) で維持する

`TypestateState { of: TypeName, transitions_to: Vec<TypeName> }` の形で遷移先 TypeName を直接列挙する案。

却下理由: catalogue 上で `methods: [{ name: "approve", returns: "Approved" }]` と `transitions_to: ["Approved"]` の両方を書く必要があり、同じ遷移先を二重宣言することになる。catalogue 上の declare と methods の戻り値の一致が自動突合の対象外となり、人間がチェックしなければならない。`transition_methods` は method 名で declare し各 method の `returns` から遷移先を linter が導出するため、二重宣言を解消し自動整合性検証が可能になる。また transitions_to は遷移先の型名のみを保持するため「どの method が遷移を行うか」という typestate の本質的な対応関係が catalogue から読み取れない。

### J: Language enum を ADR 内の概念モデルとして残す案

actual schema には `Language` enum を導入しないが、ADR 本文では概念軸の説明として `Language { DataType(DataKind), Contract, Function }` の illustrative 表現を保持する案。

却下理由: 実装に存在しない概念モデルを illustrative block として ADR に残すと、読者が「schema 上に Language enum が存在する」と誤解するリスクがある。Entry 分離 (D6) + TypeKind (D7) が Language 軸を完全に encode しているため、独立した概念モデルとしての `Language` enum は redundant である。Language 軸の値域 (3 値: DataType / Contract / Function) は散文で示せば十分であり、illustrative block を維持するコストに見合わない。

### K: `payload_types: Vec<TypeRef>` 一律 (sibling ADR `2026-05-02-0316` の `EnumVariantDeclaration`)

sibling ADR `2026-05-02-0316-enum-variant-payload-schema.md` が採用した `EnumVariantDeclaration { name, payload_types: Vec<String> }` を維持する案。

却下理由: `payload_types: Vec<TypeRef>` 一律では struct variant の field 名 (例: `Custom { findings, reason }` の `"findings"` / `"reason"`) を declare できない。unit / tuple / struct の意味的違いが schema 構造で表現されず、parse 段階での variant kind 別 reject ができない。`VariantPayload` enum (Unit / Tuple / Struct の 3 variant) で表現することで Q4.1 厳密 payload-encoded 原則と整合し、struct variant の field 名 declare も可能になる。同 ADR の `EnumVariantDeclaration` は本 ADR D12 で **supersede** される。

### L: TraitImplDecl.methods を Optional で持たせる (旧 D10)

`methods: Option<Vec<MethodDeclaration>>` で declare を任意化する案 (本 ADR の旧 D10)。declare コストは Optional により低減できるが、trait def と impl の signature 整合は Rust コンパイラが保証する範囲であり、TDDD が signature 比較する必要そのものがない。codec が `methods: None` 時に trait def を auto-derive する処理 (旧 ADR 2 D12) は cross-catalogue 解決を必要とし、D6 の crate 単位独立性に反する。`methods` field を持たない identity-only 案 (現 D10) を採用したため却下。

### M: v3→v2 形式変換による既存資産の流用（v2 コーデック・v2 カタログ型・各消費パスを v3→v2 stub 変換で温存する案）

v3 `CatalogueDocument` を v2 形式の `TypeCatalogueDocument` へ変換する `v3_doc_to_stub` 関数（v3 role → 最も近い v2 `TypeDefinitionKind` へマッピング）を各消費パスに挟み込み、v2 コーデック (`infrastructure::tddd::catalogue_codec`)・v2 カタログ型 (`domain::TypeCatalogueDocument` 等)・コミット/マージゲート・レンダラー・`sotp track lint` を v2 のまま温存する案。クロスリファレンスエッジ (`expected_methods` / `implements` / variant payload) はマッピング対象外として落とす。

却下理由:

1. 変換は不可逆であり、クロスリファレンスエッジ（`expected_methods` / `implements` / variant payload）はスタブに復元されない。
2. タイプシグナル評価器の `kind_tag` 出力と `v3_doc_to_stub` の v2-kind マッピングが「同じ kind マッピング表に暗黙に従う」結合を持つため、片方が独立に変更されるとゲートが静かに fail-open に後退する（ADR `2026-04-12-1200-strict-spec-signal-gate-v2.md` の fail-closed 原則と相反）。
3. 全消費パスに漏れなく変換を挟む必要があり、適用漏れが起きると即座にゲートやレンダラーの動作不能を生む脆い方式である。

すべての `<crate_name>-types.json` 消費パスは `schema_version: 3` のカタログ（`domain::tddd::catalogue_v2` モジュール、モジュール名は第 2 世代設計の開発名）をネイティブにデコードし、非 v3 はフェイルクローズドエラーとして扱う。`v3_stub` モジュール・v2 コーデック・v2 カタログ型は削除し後方互換は維持しない（完了・アーカイブ済みトラックは凍結されており再検証されないため、削除の影響はない）。各消費パスの移行詳細は `spec.json` / `impl-plan.json` で管理する。

## Consequences

### 良い影響

- Role × Entry 種別、Pattern × Kind、members × kind の軸間制約がすべて schema 構造で encode される。parse 段階で制約違反を reject でき、validation 段階まで到達しない。
- γ-1 newtype 系により context 違反 (TypeName を MethodName フィールドに渡す等) がコンパイルエラーになる。
- `TraitImplDecl` が trait identity のみとなることで derive trait の宣言コストが impl identity のみに最小化され、catalogue がさらにシンプルになる。signature 整合性の検証はコンパイラに任せる。
- crate 単位 1 ファイル (D6) により rustdoc 出力との粒度が一致し、突合 codec が 1:1 比較できる。commit / diff / PR の粒度も crate と自然に一致する。
- crate name prefix (D11) により TypeRef の cross-crate 参照表現と統一でき、catalogue 内 path 表現が一貫する。functions map キーは自 catalogue の crate prefix のみを受け入れる (D11.1)。
- 既存 TypeGraph スタイル (Type / Trait / Function 3 分離 + inline 展開) を出発点として活用するため、実装コストが最小化される。
- typestate transition の declare で二重宣言が解消される。`transition_methods` により method の `returns` から遷移先を導出するため、methods と transitions_to の二重 declare が不要になる。
- linter による自動整合性検証が可能になる。遷移元 / 遷移先の cluster 一致を schema レベルの method 参照 + linter で強制できる。
- enum variant の 3 種 (unit / tuple / struct) が `VariantPayload` enum の構造で encode される。parse 段階で variant kind 別の制約違反を reject できる。
- struct variant の field 名 declare が可能になり、`Custom { findings: Vec<StoredFinding>, reason: String }` のような struct variant を catalogue で完全に表現できる。
- sibling ADR `2026-05-02-0316` の `payload_types` 一律表現の制限が解消され、`VariantPayload` 構造によって表現力が向上する。

### 悪い影響

- γ-1 newtype 系で 9+ 種の newtype が増える。`Display` / `FromStr` / `Serialize` / `Deserialize` の実装が各 newtype に必要になる (boilerplate 増加)。
- crate 単位 1 ファイル (D6) により workspace の crate 数だけ catalogue ファイルが増える。
- 既存 catalogue (V1: `<layer>-types.json` / TypeDefinitionKind ベース) からの migration コストが発生する。新規 track 以降で新 schema を採用する方針 (backward compat なし) のため、既存 catalogue の一括変換は対象外。
- TypeKindV2 の 5 flat variant により JSON serde 表現が variant ごとに異なる payload を持つ (UnitStruct / TupleStruct / PlainStruct / Enum / TypeAlias で異なる shape)。
- parent ADR M1 / S2 で決定した `methods_of()` renderer の均質化は、本 ADR の TypeKindV2 ベース schema 上で再実装が必要になる。

## Reassess When

- 多言語化要求が表面化した場合: Language 軸の値域変更と Rust 固有の desugar 仮定が崩れるため、framework ごとの fork を検討する。
- DataRole / ContractRole / FunctionRole のいずれかに新しい role の追加要求が来た場合: Role 値域の変更は型カタログ全体の影響を持つため、専用 ADR で判断する。
- TypeKindV2 に新しい kind variant (例: `UnsafeStruct` / `BitfieldStruct`) の追加要求が来た場合: Kind 軸の拡張は schema 互換性に影響するため要評価。または PlainStruct の `typestate` marker を拡張するような要求が来た場合も同様。
- ItemAction (D4) に `Rename` のような新しい action の要求が来た場合: TDDD-03 との関係で別途判断する。
- workspace の crate 分割方針が大きく変わる場合: 1 layer N crate の想定が崩れる (またはその逆) と D6 のファイル単位設計を見直す必要がある。
- rustdoc-types crate のメジャーバージョンアップで Item / Type 構造が破壊的に変わる場合: Codec が Catalogue schema との間で変換する中間 TypeGraph 表現を見直す必要がある。
- ADR `2026-04-13-1813` で決定された TypeDefinitionKind variants の使われ方が大きく変わる場合: DataRole の 13 値と ContractRole の 3 値は同 ADR の 12 variants を引き継いでいるため、値域の意味が変化すると本 ADR の再評価が必要。
- typestate transition pattern が self-consuming method 以外の表現 (例: `&mut self` での状態遷移、`From` / `Into` trait による暗黙変換、async 遷移メソッド等) を求められた場合: `transition_methods` の declare 方式 / linter 検証ルールを再評価する。
- `rustdoc_types::VariantKind` の構造が変わった場合 (Rust の enum variant 機能の拡張、例: discriminant 値の宣言): `VariantPayload` の値域を再評価する。
- `VariantPayload` の既存 3 variant では表現できない新たな declare 要求が来た場合 (例: anonymous struct payload の入れ子等): schema 互換性への影響を評価した上で専用 ADR で判断する。

## Related

- `knowledge/adr/2026-04-13-1813-tddd-taxonomy-expansion.md` — V1 元 ADR (12 variants 拡張)。本 ADR で DataRole / ContractRole として軸分離に再編成する。同 ADR の TypeDefinitionKind variants は Role 軸の値として継承される。
- `knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md` — parent ADR。M1 (struct kind 均質化) / S1 (`domain_service` 新設) / S2 (renderer 統合) / S3 (linter framework) を本 ADR の schema 上で継承・発展させる。
- `knowledge/adr/2026-04-29-1653-aggregate-entity-kind-representation.md` — spin-off ADR (deferred)。本 ADR の軸分離確定後、Entity / AggregateRoot を新 Role 軸の上で再起草可能になる。
- `knowledge/adr/2026-04-11-0003-type-action-declarations.md` — TDDD-03 (action フィールド導入元)。本 ADR の D4 として ItemAction 4 種を継承する。
- `knowledge/adr/2026-05-08-0305-tddd-signal-evaluator-three-way-diff.md` — sibling ADR (Signal evaluator)。本 ADR D4 で言及する「領域」の正確な定義 (11 領域) は同 ADR D3 が確定する。action × 領域 × C (Current 実装) の signal semantics をここで「11 領域 × signal table」に refine する。
- `knowledge/adr/2026-04-29-0240-method-type-full-generic-declaration.md` — sibling spin-off (MethodDeclaration の generic 拡張)。本 ADR では MethodDeclaration を継続使用する。
- `knowledge/adr/2026-04-29-0241-typestate-transition-edge-rendering.md` — sibling spin-off (typestate transition rendering)。本 ADR の D3 で `TypestateState` の payload を `transition_methods` に変更したことで、同 ADR の rendering logic が影響を受ける。`transitions_to` を読んで edge を抽出していた箇所を `transition_methods` 経由 (method の `returns` から遷移先を導出) に変更する必要があり、同 ADR の更新は別途実施する。
- `knowledge/adr/2026-05-02-0316-enum-variant-payload-schema.md` — sibling spin-off (EnumVariantDeclaration { name, payload_types })。本 ADR D12 で **supersede** され、`VariantDecl { name, payload: VariantPayload }` (Unit / Tuple / Struct の 3 variant) に置き換えられる。同 ADR の `payload_types` 一律表現は本 ADR で廃止。
- `knowledge/adr/README.md` — ADR 索引
