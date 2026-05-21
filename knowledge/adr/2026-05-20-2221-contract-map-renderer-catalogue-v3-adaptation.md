---
adr_id: 2026-05-20-2221-contract-map-renderer-catalogue-v3-adaptation
decisions:
  - id: A
    user_decision_ref: "chat_segment:tddd-v2-contract-map-renderer-design:2026-05-13"
    candidate_selection: "from:[A-3',A-1',A-2'] chose:A-3'"
    status: proposed
  - id: B
    user_decision_ref: "chat_segment:tddd-v2-contract-map-renderer-design:2026-05-13"
    candidate_selection: "from:[B-1,B-2,B-3] chose:B-1"
    status: proposed
  - id: C
    user_decision_ref: "chat_segment:tddd-v2-contract-map-renderer-design:2026-05-13"
    candidate_selection: "from:[C-1,C-2,C-Minimal,C-3,config-file] chose:config-file"
    status: proposed
  - id: D
    user_decision_ref: "chat_segment:tddd-v2-contract-map-renderer-design:2026-05-13"
    candidate_selection: "from:[D-1,D-2,D-3,D-4] chose:D-2"
    status: proposed
  - id: E
    user_decision_ref: "chat_segment:tddd-v2-contract-map-renderer-design:2026-05-13"
    candidate_selection: "from:[E-1+,E-3a,E-3b,E-3c,E-3d] chose:E-3c"
    status: proposed
  - id: F
    user_decision_ref: "chat_segment:tddd-v2-contract-map-renderer-design:2026-05-13"
    candidate_selection: "from:[F-1,F-2+b1,F-2+b2-i,F-2+b2-ii,F-2+b2-iii,F-2+b3,F-2+d1,F-2+d2,F-3] chose:F-2+b2-ii+F-2+d1"
    status: proposed
  - id: G
    user_decision_ref: "chat_segment:tddd-v2-contract-map-renderer-design:2026-05-13"
    candidate_selection: "from:[G-1,G-2'a,G-2'b,G-2'c,G-2'd] chose:G-2'b"
    status: proposed
  - id: H
    user_decision_ref: "chat_segment:tddd-v2-contract-map-renderer-design:2026-05-13"
    candidate_selection: "from:[H-1,H-2,H-3] chose:H-3"
    status: proposed
  - id: H_prime
    user_decision_ref: "chat_segment:tddd-v2-contract-map-renderer-design:2026-05-13"
    candidate_selection: "from:[H'-1,H'-2,H'-3,H'-4,H'-5] chose:H'-1"
    status: proposed
  - id: I
    user_decision_ref: "chat_segment:tddd-v2-contract-map-renderer-design:2026-05-13"
    candidate_selection: "from:[I-1,I-2,I-3] chose:I-1"
    status: proposed
  - id: J
    user_decision_ref: "chat_segment:tddd-v2-contract-map-renderer-design:2026-05-13"
    candidate_selection: "from:[J-1,J-2,J-3] chose:J-2"
    status: proposed
  - id: K
    user_decision_ref: "chat_segment:tddd-v2-contract-map-renderer-design:2026-05-13"
    candidate_selection: "from:[K-1,K-2,K-3,K-4] chose:K-2+(d)+Newtype-1"
    status: proposed
  - id: L
    user_decision_ref: "chat_segment:tddd-v2-contract-map-renderer-design:2026-05-13"
    candidate_selection: "from:[L-1,L-2,L-7,L-8,L-9,L-10] chose:L-1+L-8+L-10"
    status: proposed
  - id: M
    user_decision_ref: "chat_segment:tddd-v2-contract-map-renderer-design:2026-05-13"
    candidate_selection: "from:[M-A1,M-A2,M-A3,YAGNI] chose:YAGNI"
    status: proposed
  - id: N
    user_decision_ref: "chat_segment:tddd-v2-contract-map-renderer-design:2026-05-13"
    candidate_selection: "from:[N-1,N-1',N-2,N-3,N-4] chose:N-1'"
    status: proposed
  - id: O
    user_decision_ref: "chat_segment:tddd-v2-contract-map-renderer-design:2026-05-13"
    candidate_selection: "from:[O-1,O-2,O-3,O-a,O-b] chose:O-2+O-3+O-a"
    status: proposed
  - id: U
    user_decision_ref: "chat_segment:tddd-v2-contract-map-renderer-design:2026-05-13"
    candidate_selection: "from:[U-1,U-2,U-3,U-4,U-5,U-6a',U-6b',U-6c',U-6d-iii] chose:U-6d-iii"
    status: proposed
---
# Contract Map Renderer: catalogue schema v3 対応設計

## Context

### §1 先行 ADR (2026-04-17-1528) の renderer が依拠した旧 catalogue schema

ADR `2026-04-17-1528-tddd-contract-map` で導入された contract map renderer は、旧 catalogue schema を前提として設計されていた:

- **`TypeDefinitionKind` 単一 enum** (13 variants) で kind / role / pattern を 1 軸に混在
- **1 ファイル = 1 layer** の catalogue 構成 (per-layer `<layer>-types.json`)
- `render_contract_map(catalogues: &BTreeMap<LayerId, TypeCatalogueDocument>, ...)` — caller が事前に layer keyed map を構築して渡す
- shape/color は 13 kind ごとの固定テーブル (旧 ADR §D3)

### §2 catalogue schema v3 への移行 (ADR 2026-05-08-0248 + make-catalogue-schema-permissive)

TDDD v2 では catalogue schema を v3 (`schema_version: 3`) に全面更新した。主要変更:

1. **3 軸分離**: Language × Role × Layer の直交分離
2. **TypeKindV2 5 variants**: `UnitStruct` / `TupleStruct { fields, has_stripped_fields }` / `PlainStruct { fields, has_stripped_fields, typestate: Option<TypestateMarker> }` / `Enum { variants }` / `TypeAlias { target }`
3. **Role 3 enum 分離**: `DataRole` (13 値) / `ContractRole` (3 値) / `FunctionRole` (2 値)
4. **`LayerId` 採用**: `architecture-rules.json` 由来の layer-agnostic 不変識別子 (ADR 2026-04-17-1528 §4.5 と整合)
5. **1 ファイル = 1 crate**: crate 単位の catalogue document (`CatalogueDocument`) に変更。1 layer に N crate を持てる
6. **top-level `inherent_impls` + `trait_impls` コレクション**: `CatalogueDocument` に `inherent_impls: Vec<InherentImplDeclV2>` と `trait_impls: Vec<TraitImplDeclV2>` が追加された (ADR 2026-05-18-1223 D2 / ADR 2026-05-20-0048 D1)。旧 `TypeEntry.trait_impls` フィールドはこの変更で削除された
7. **`action: ItemAction` 必須化**: 全 entry (`TypeEntry` / `TraitEntry` / `FunctionEntry`) および `TraitImplDeclV2` に `action: ItemAction` (Add/Modify/Reference/Delete) が必須フィールドとして追加された
8. **`InherentImplDeclV2` の構造**: `type_name: TypeName` + `impl_generics` + `impl_where_predicates` + `methods` で構成される。同一 type に複数 impl block が存在する場合は複数エントリで表現する (ADR 2026-05-18-1223 D2)
9. **`TraitImplDeclV2` は identity-only**: `action: ItemAction` + `trait_ref: TypeRef` + `for_type: TypeRef` + `impl_generics` + `impl_where_predicates` で構成され、**methods を持たない** (ADR 2026-05-20-0048 D2)
10. **where 句の表現**: `WherePredicateDecl` = lhs/rhs/operator + BoundOp で表現し、HRTB-on-TraitBound もサポートされる (ADR 2026-05-18-1223 D1/D5)
11. **`TypeEntry.methods` は維持**: `TypeEntry` は `action` + `role` + `kind`(TypeKindV2) + `methods` + `module_path` + `docs` + `spec_refs` + `informal_grounds` を持つ。`trait_impls` は削除されたが `methods` は健在

これらの変更により旧 renderer は入力 schema 不一致で機能しなくなった。renderer の全面再設計が必要。

### §3 renderer の役割

Contract Map renderer は:

- 全 tddd.enabled layer の `CatalogueDocument` 群を入力として受け取り
- 宣言された型 / trait / function の contract 関係を mermaid flowchart として render し
- `contract-map.md` として出力する

rustdoc 入力 (Reality View、ADR 2026-04-16-2200) とは役割を補完する位置付け:

| 観点 | Contract Map (本 ADR) | Reality View |
|---|---|---|
| 入力 | catalogue (`CatalogueDocument` 群) | rustdoc JSON |
| 表す内容 | designer が宣言した contract 関係 | コンパイル後の実装状態 |
| 更新タイミング | catalogue 更新時 | 実装進捗に応じて随時 |

### §4 layer-agnostic 不変条件の継承

旧 ADR §4.5 の layer-agnostic 不変条件 (層名ハードコード禁止 / 層リストは `architecture-rules.json` 由来 / layer_order はトポロジカルソート) を本 ADR でも継承する。

## Decision

### A: renderer の入力 type (採択: A-3')

renderer 関数のシグネチャ:

```rust
<!-- illustrative, non-canonical -->
pub fn render_contract_map(
    catalogues: &[CatalogueDocument],
    layer_order: &[LayerId],
    opts: &ContractMapRenderOptions,
) -> ContractMapContent;
```

- 各 `CatalogueDocument` は自己記述的 (`crate_name: CrateName` + `layer: LayerId`)
- subgraph 集約は renderer 内で `doc.layer` で group_by
- 1 layer N crate 構成でも `Vec` に並列格納可能 (ADR 2026-05-08-0248 D6 の 1 catalogue = 1 crate 原則と整合)

### B: Node 列挙の表現 (採択: B-1)

entry 種別を enum で区別する:

```rust
<!-- illustrative, non-canonical -->
enum CatalogueNode<'a> {
    Type { layer: &'a LayerId, doc: &'a CatalogueDocument, name: &'a TypeName, entry: &'a TypeEntry },
    Trait { layer: &'a LayerId, doc: &'a CatalogueDocument, name: &'a TraitName, entry: &'a TraitEntry },
    Function { layer: &'a LayerId, doc: &'a CatalogueDocument, path: &'a FunctionPath, entry: &'a FunctionEntry },
}
```

`match` で entry 種別ごとの shape/edge ロジックを分岐する (`.claude/rules/04-coding-principles.md` § Enum-first パターン)。

**schema-delta 注記 (make-catalogue-schema-permissive)**: `TraitImplDeclV2` / `InherentImplDeclV2` は `CatalogueNode` の variant にはならない。これらは top-level コレクション (`doc.trait_impls` / `doc.inherent_impls`) として別途走査し、edge 生成や method 付加の source として扱う (詳細は Decision J / F を参照)。

### C: shape / 色 / 線種は設定ファイル化 (採択)

- **設定ファイル位置**: `.harness/config/contract-map-style.toml` (`.harness/config/agent-profiles.json` と同じ階層)
- TypeEntry / TraitEntry の shape は subgraph 矩形に固定 (Decision F の帰結として TypeEntry / TraitEntry が subgraph 化されるため、shape の表現力は実質的に消失する)。FunctionEntry は Decision F で subgraph 化されず standalone callable node として扱われるため、この subgraph 矩形固定の制約から外れる — FunctionEntry の node shape は設定ファイル `[node.Function]` の `shape` で別途指定する
- Role 区別は classDef (色 / 線種 / 太さ) のみで表現
- 旧 ADR 2026-04-17-1528 §D3 の 13 kind × shape mapping は本 ADR で廃止

### D: node_id 命名 (採択: D-2)

prefix で entry 種別を識別し、length-prefix で injective 性を維持する:

- Type: `T<len>_<sanitized_layer>_<sanitized_crate>_<sanitized_name>`
- Trait: `R<len>_<sanitized_layer>_<sanitized_crate>_<sanitized_name>` (R for tRait)
- Function: `F<len>_<sanitized_layer>_<sanitized_crate>_<sanitized_full_path>` (FunctionPath 全体を sanitize)

`<len>` の定義は entry 種別で異なる:
- Type / Trait: `sanitized_layer` + `_` + `sanitized_crate` + `_` + `sanitized_name` の文字数
- Function: `sanitized_layer` + `_` + `sanitized_crate` + `_` + `sanitized_full_path` の文字数

1 layer に N crate が存在する構成 (ADR 2026-05-08-0248 D6 の 1 catalogue = 1 crate 原則) では同一 layer 内で同名の Type / Trait が異なる crate に宣言される可能性があるため、`sanitized_crate` を含めることで injective 性を保証する。`sanitized_crate` は `doc.crate_name` を mermaid node_id 用に sanitize した値。Function は `sanitized_full_path` (FunctionPath 全体の sanitized 値) を `sanitized_name` に代えて使用するため、`<len>` もその全体長で計算する。

### E: renderer の配置層 (採択: E-3c)

hexagonal port + adapter pattern で配置する:

- `ContractMapRenderer` **port** を domain 層に新設 (`libs/domain/src/tddd/contract_map_renderer.rs` 等)
- **Adapter** は infrastructure 層 (`libs/infrastructure/src/tddd/contract_map_renderer_adapter.rs` 等)。syn crate を使用して TypeRef を精密 parse
- usecase 層の `RenderContractMapInteractor` が port を inject される
- 既存の `ContractMapWriter` port + `FsContractMapWriter` adapter (旧 ADR 2026-04-17-1528 §D1 由来) と同じ pattern を踏襲

### F: method / variant の node 化 + entry の subgraph 化 (採択: F-2+b2-ii + F-2+d1)

- 全 TypeEntry / TraitEntry を **subgraph 化** (methods 0 個でも空 subgraph を生成)
- `TypeEntry.methods` / `TraitEntry.methods` の各 method を node 化し、entry subgraph 内に配置
- **`InherentImplDeclV2.methods` も method ソース**: `doc.inherent_impls` の各エントリが持つ `methods` は `type_name` で対応する Type subgraph に紐付け、そのサブグラフ内に同様に node 化する。1 type に複数 `InherentImplDeclV2` が存在する場合はすべてのエントリの methods を集約して配置する
- method node から `--o` で param type へ、`-->` で returns type へ edge
- `FunctionEntry` は自身が callable node (subgraph 化はせず、entry subgraph と並んで配置)
- shape は subgraph 矩形固定。Role 区別は classDef のみ (Decision C の帰結)

旧 `SecondaryAdapter.trait_impl_methods` 経由の edge (旧 ADR 2026-04-17-1528 §D4 (2)) は ADR 2026-05-08-0248 D10 (TraitImplDeclV2 identity-only) により廃止されたため、本 ADR では新 schema に基づく edge 設計に置き換える。`TraitImplDeclV2` は methods を持たず identity-only であるため (ADR 2026-05-20-0048 D2)、trait impl 由来の method node 化は行わない。

### G: typestate transition edge (採択: G-2'b)

`TypeKindV2::PlainStruct.typestate: Some(TypestateMarker { state_name, transitions })` のとき、`transitions.transition_methods()` の各 method の returns edge を専用 style で描画する:

- edge syntax: `methodNode ==>|transitions_to| returnsTypeSubgraph`
- 通常の returns edge (`-->`) と視覚的に区別し、typestate machine の遷移構造を強調
- style は設定ファイル `[edge.transition]` で調整可能 (default arrow `==>`, label `transitions_to`)
- 旧 ADR 2026-04-29-0241 の `TypestateTransitions::To(names)` 由来の transition edge 設計を継承し、v3 schema (`typestate: Option<TypestateMarker>` field) に対応させる

### H: enum variant node 化 + payload edge (採択: H-3)

`TypeKindV2::Enum.variants` の各 `VariantDecl` を node 化し、entry subgraph 内に配置する:

- `VariantPayload::Tuple(Vec<TypeRef>)`: 各 TypeRef へ無 label edge (`--o`)
- `VariantPayload::Struct(Vec<FieldDecl>)`: 各 FieldDecl の name を label に (`--o|field_name|`)
- `VariantPayload::Unit`: edge なし

### H': Trait method の表現 (採択: H'-1)

Decision F と同じ表現で Trait method を扱う: Trait subgraph 内に method node を内包する。

### I: FunctionEntry の列挙範囲 (採択: I-1 + 将来 filter)

- default: 全 FunctionEntry を render (filter なし)
- filter は将来 `[filter] include_function_roles = ["UseCaseFunction"]` などで設定ファイル経由で対応

### J: trait impl edge の target 解決 (採択: J-2)

全 ContractRole で統一 edge style `-.impl.->` を使用する:

- edge の source/target は **top-level `doc.trait_impls`** の各 `TraitImplDeclV2` から導出する: `for_type`(実装型) → `trait_ref`(trait) の方向で edge を引く
- 旧 `TypeEntry.trait_impls` は schema から削除されたため、trait impl edge の source は `doc.trait_impls` コレクションに一本化される (ADR 2026-05-20-0048 D1)
- Adapter→Port / Interactor→ApplicationService / SecondaryPort impl のいずれも同 style
- ADR 2026-05-08-0248 D10 (TraitImplDeclV2 identity-only) と整合
- Role 別 edge style は将来の設定ファイル拡張 (`[edge.trait_impl.<TraitRole>]`) で対応可能

### K: struct fields の表現 (採択: K-2 + (d) + Newtype-1)

- `PlainStruct.fields` は node 化せず、entry subgraph から field type subgraph へ直接 edge `--o|field_name|`
- `TupleStruct.fields`: entry → field type へ edge、label は positional index (`.0` / `.1` / ...)
- `has_stripped_fields: true` の場合: render しない (catalogue data は保持するが render 出力には反映しない)
- Newtype 検出 (TupleStruct + 1 field): lint 層に委譲 (renderer は通常 TupleStruct と同扱い)

### L: 設定ファイル schema (採択: L-1 + L-8 + L-10)

- 位置: `.harness/config/contract-map-style.toml` (repo commit 対象、default として同梱)
- section 構造:
  - `[role.<RoleName>]` — RoleName は DataRole 13 値 / ContractRole 3 値 / FunctionRole 2 値、各 `class: String` のみ
  - `[node.<NodeCategory>]` — NodeCategory は `{Method, Variant, Field, Function}` など、`shape` と `class` フィールド (Function は FunctionEntry standalone callable node のスタイル定義)
  - `[pattern.<PatternName>]` — 現状 `Typestate` のみ、`overlay_class`
  - `[class.<ClassName>]` — `fill` / `stroke` / `stroke_width` / `stroke_dasharray` (mermaid classDef 構文と直接対応)
  - `[edge.<EdgeKind>]` — EdgeKind は `{method_param, method_returns, transition, trait_impl, variant_payload, field, alias}`、`arrow` と optional `label`
  - `[filter]` — `include_function_roles: Vec<FunctionRole>` / `kind_filter` / `include_fields` 等
- 設定ファイル不在は **fail-closed エラー**
- default は repo 同梱 `.harness/config/contract-map-style.toml` を commit、user は同 file を直接編集

### M: action / signal overlay (採択: YAGNI で不採用)

旧 ADR 2026-04-17-1528 §D5 の action overlay と signal overlay は本 ADR では採用しない。必要性が顕在化した際に別 ADR で追加検討する。

**schema-delta 補記**: `action: ItemAction` は schema 上 entry の core 必須フィールドとなったが (§2 変更点 7)、renderer への overlay 適用は引き続き YAGNI と判断する。`action` 値を renderer が読むことは technically 可能だが、本 ADR の scope では行わない。将来 overlay が必要になった場合の再評価は「Reassess When」の項目として記載済み。

### N: TypeAlias.target の表現 (採択: N-1')

`TypeAlias` は空 subgraph + target type subgraph への **無向 edge** `---|alias_of|` で表現する。`TypeAlias.target` の方向性 (alias → target) はスキーマ上は有向だが、contract-map の可視化目的では「AとBが alias 関係にある」という存在を示すことが主であり、読者が alias か target かを edge 向きで読み取る必要は低い。方向の明示が必要な場合は Rejected Alternative N-1 の有向 `--o|alias_of|` に戻すことを検討する。

### O: cross-catalogue trait_impl 解決 (採択: O-2 + O-3 + O-a)

renderer 関数内でローカル cache として global trait index を 1 回構築する:

```rust
<!-- illustrative, non-canonical -->
// BTreeMap<(CrateName, TraitName), trait_subgraph_id: String>
```

- 入力 `catalogues` (= `&[CatalogueDocument]`) を全走査して 1 回だけ構築
- **index の構築源は `doc.traits` (各 `CatalogueDocument` の `TraitEntry` BTreeMap)**: 全 catalogue の `traits` エントリを走査し `(doc.crate_name, trait_name) → trait_subgraph_id` の写像を構築する。edge の描画 (どの型がどの trait を impl するか) は `doc.trait_impls` の各 `TraitImplDeclV2` から導出し (`for_type` → `trait_ref` の方向)、上記 index を用いて `trait_ref` の subgraph_id を解決する。top-level trait_impls コレクションに移行したことで `for_type` (実装型) が明示的に格納されるようになり、external self-type による cross-crate impl (Case B) が first-class で扱える。`for_type` が workspace 外の type を指す場合は **silent skip** (edge を引かない) の方針は維持する
- workspace 外の std / 外部 crate は **silent skip** (edge を引かない)
- catalogue 単独完結性 (旧 ADR 2026-04-17-1528 §D1) と整合

### U: node 配置順序 + module subgraph (採択: U-6d-iii)

**最上位 module 1 段 subgraph** で entry を group する:

- `(crate_name, module_path[0])` の組が同じ entry を 1 subgraph に集約
- subgraph id: `<sanitized_layer>_<sanitized_crate>_module_<sanitized_module_path_first_segment>`
- subgraph label: `<crate_name>::<module_path_first_segment>` (例: `domain::review`)
- sub-module 階層 (`module_path[1..]`) は entry subgraph の label に含める (例: `team::manager::TeamManager`)
- crate root entry (`module_path = []`) は module subgraph 外、layer subgraph 直下に配置
- nesting 深さは固定: layer → top-module → entry → method = 4 重
- 各 collection の iter 順: BTreeMap は alphabetical、Vec は declaration order

## Render Output 構造

Decision 群の集約として、生成する mermaid 出力の構造:

```
1. classDef 定義 (.harness/config/contract-map-style.toml の [class.*] 由来、alphabetical) — [edge.*] は arrow/label 定義であり classDef には含まない
2. layer subgraph 群 (layer_order 順)
   2.1. top-module subgraph 群 ((crate_name, module_path[0]) per pair、crate_name alphabetical 内で module_path[0] alphabetical)
        2.1.1. entry subgraph 群 (TypeEntry / TraitEntry、BTreeMap iter = alphabetical by name)
               - label に sub-module path 含む (例: team::manager::TeamManager)
               2.1.1.1. method / variant node (Vec 順 = declaration order)
        2.1.2. FunctionEntry callable node 群 (同 top-module subgraph 内、entry subgraph と並列配置、alphabetical by FunctionPath)
   2.2. crate root entry / crate root FunctionEntry (subgraph 外、alphabetical by crate_name then name)
3. edge 定義群 (構造 walk 順)
4. class attach 群 (`class <id> <className>` を別行で適用、subgraph も含む)
```

mermaid syntax 注: subgraph には inline `:::className` が使えない (parse error)。`class <subgraphId> <className>` を別行で記述する必要がある。

## Rejected Alternatives

### A: renderer の入力 type

- **A-1': `BTreeMap<LayerId, Vec<CatalogueDocument>>`** — caller が事前 group する負担が増える。LayerId と `doc.layer` の二重保持になる
- **A-2': `BTreeMap<CrateName, CatalogueDocument>`** — layer 集約と crate 順序の 2 重 order 制御が必要になる

### B: Node 列挙の表現

- **B-2: trait `RenderableNode` polymorphism** — entry 種別ごとの shape/edge ロジックが大きく異なるため trait API が痩せて実用性がない
- **B-3: NormalizedNode 中間表現** — variant payload 等の context が normalize 時に decompose されて情報が失われる

### C: shape / 色 / 線種

- **C-1: Role × Pattern 両軸を shape に encode** — mermaid の shape vocabulary が不足して実現不可
- **C-2: Role を shape、Pattern を classDef** — Decision F で entry が subgraph 化されるため shape 表現力が消失する
- **C-Minimal: 旧 shape 流用 + 新 Role 5 種を rect** — 設計意図が失われる
- **C-3: label 注記** — shape の「読まずに伝わる」価値が損なわれる

### D: node_id 命名

- **D-1: 旧 scheme `L<len>_<layer>_<name>` 維持** — 同名 Type / Trait の id 衝突を回避できない
- **D-3: suffix `_t` / `_r` / `_f`** — prefix の方が可視性が高い
- **D-4: namespace-aware `<layer>::types::<name>`** — node_id が冗長になる

### E: renderer の配置層

- **E-1+: light-weight `extract_type_names` 拡張** — syn 精密 parse を採用するため domain のみでは対応できない
- **E-3a: syn を domain crate に追加 + renderer を domain 配置** — ADR 2026-05-08-0258 D9 が syn を codec (infrastructure 層) に置く設計を前提とするため、syn を domain crate に追加することは hexagonal 分離原則 (`knowledge/conventions/hexagonal-architecture.md`) に反する
- **E-3b: renderer を infrastructure に移動 (port なし)** — usecase → infrastructure 直接依存で hexagonal 違反
- **E-3d: renderer 入力を `ExtendedCrate` に変更** — ADR 2026-04-17-1528 §D1 の「catalogue 単独完結」契約を破る

### F: method / variant の node 化 + entry の subgraph 化

- **F-1: TypeEntry.methods のみから edge** — 旧 SecondaryAdapter trait_impl methods 経由の経路は ADR 2026-05-08-0248 D10 で廃止済み
- **F-3: F-2 + Trait method を Adapter にも複製** — edge / node 重複でグラフが肥大する
- **F-2+b1: parent → method edge** — 含有関係を edge で表現するのは冗長
- **F-2+b2-i: methods を持つ entry のみ subgraph 化** — shape 表現が混在して視覚的に不一貫
- **F-2+b2-iii: entry node + 内包 subgraph 二重** — label 重複表示になる
- **F-2+b3: 配置のみ (近接配置を auto-layout 任せ)** — 近接保証がない
- **F-2+d2: FunctionEntry に method node を付随** — 冗長

### G: typestate transition edge

- **G-2'a: 通常 `-->` と同じ** — transition と他 returns の区別がなくなる
- **G-2'c: method node に classDef** — 遷移「方向」は edge 依存なので node 色付けでは表現が弱い
- **G-2'd: method node + edge 両方強調** — 表現過剰
- **G-1: scope 外として除外** — transition は contract map の主要な表現価値なので scope に含めるべき

### H: enum variant node 化 + payload edge

- **H-1: 旧 entry subgraph → payload type 直接 edge** — F-2+b2-ii で method を node 化した設計と非対称になる
- **H-2: variant node 化 + label 統一** — Struct field 名の視覚化価値を捨てることになる

### H': Trait method の表現

- **H'-2: Trait method を Trait subgraph 外 + 専用 contract edge** — method 配置が分散する
- **H'-3: Trait と各 Adapter に method 複製** — node 重複でグラフが肥大する
- **H'-4: 独立 contract subgraph** — レイアウトが複雑になる
- **H'-5: Adapter subgraph 内に Trait method の実装 node** — 同 method が 2 node で表現される

### I: FunctionEntry の列挙範囲

- **I-2: UseCaseFunction のみ default** — free function の contract 説明が抑制される
- **I-3: Rust API に `include_function_roles` 引数追加** — 設定ファイル化方針と統合できない

### J: trait impl edge target 解決

- **J-1: SecondaryPort 限定** — Interactor→ApplicationService 等が描けず、新 schema の表現力が低下する
- **J-3: Role 別 edge style hard-code** — 複雑性が増し、設定ファイル化と統合できない

### K: struct fields の表現

- **K-1: field を node 化** — node 数が爆発し、method との対称性は生まれるが情報密度が過剰になる
- **K-3: 完全 render 対象外** — field 由来の依存関係が不可視になる
- **K-4: default 非表示 + option 切替** — K-2 の方が user 選択として適切と判断
- **has_stripped_fields (a): 省略記号表示** / **(b): 警告 note 付与** / **(c): classDef で色変え** — いずれも (d) 表現なしより情報ノイズが増える
- **Newtype-2: renderer 内検出 + `:::newtype` overlay** — renderer に lint logic が混入する

### L: 設定ファイル schema

- **L-2: 統合 `[style.*]`** — Role と Node category が同 namespace に混在して意味的区別がつかない
- **L-7: code 内 hard-coded fallback** — ファイル不在をエラーで明示する方が運用上安全
- **L-9: code 内 default** — repo 同梱 file との二重管理になり SoT がぶれる

### M: action / signal overlay

- **M-A1: default OFF + option 切替** / **M-A2: default ON** / **M-A3: mixed** — いずれも YAGNI と判断

### N: TypeAlias.target の表現

- **N-1: 有向 `--o|alias_of|`** — schema 上の方向を忠実に表現できる一方、contract-map の可視化目的においては alias か target かを読者が edge 向きで判断する必要は低い (N-1' を選択)
- **N-2: edge なし** — aliasing 関係が読み取れない
- **N-3: 専用 thick edge `==>`** — transition edge (Decision G) と style 重複する
- **N-4: TypeAlias 自体を render しない** — declare 意図を無視することになる

### O: cross-catalogue trait_impl 解決

- **O-1: lookup ごとに全走査** — O(NM) となり大規模時に性能懸念がある
- **O-b: long-lived index (Adapter 内 Arc 等)** — stale 問題が生じる。関数内ローカル cache で十分

### U: node 配置順序 + module subgraph

- **U-1〜U-5: module 集約なし** — 同 module の近接配置が保証されない
- **U-6a' (フル nesting)**: multi-segment ModulePath で 6 重以上の深い nest が生じ unreadable になる
- **U-6b' (フラット化 1 段 + full path label)**: sub-module の siblings が別 subgraph に分離する
- **U-6c' (上限 2 段)**: 2 段超え部分のフラット化の境界条件で semantic が複雑になる

## Consequences

### 良い影響

- catalogue v3 schema (TypeKindV2 5 variants + LayerId + Role 軸分離 + top-level trait_impls/inherent_impls) と完全に整合する
- method / variant が node 化されることで entry の callable surface / sum type variant が視覚化される
- 設定ファイル化により組織ごとの style カスタマイズが可能になる (`.harness/config/contract-map-style.toml`)
- 最上位 module 1 段 subgraph により edge cross 削減効果が期待できる
- syn 精密 parse + codec 共通化 (ADR 2026-05-08-0258 D9) により TypeRef 解決の信頼性が向上する
- port + adapter pattern により hexagonal architecture との整合性が維持される (Decision E)
- ADR 2026-04-17-1528 §4.5 の layer-agnostic 不変条件を遵守する (LayerId 採用)
- `for_type` 明示化により external self-type の cross-crate impl が first-class で扱えるようになる (Decision O)

### 悪い影響

- shape の表現力が classDef のみに制限される (subgraph 矩形固定のため)
- method / variant 単位で node 数が増加し、大規模 catalogue でグラフが肥大する可能性がある
- 設定ファイル不在は fail-closed エラーとなるためセットアップ要件がある
- cross-catalogue trait index 構築で O(N) コストが発生する (per render call)
- 旧 ADR 2026-04-17-1528 §D3 の shape mapping table を完全に置き換えるため migration コストがある
- TypeAlias / Newtype の semantic は lint 層との協調が必要で renderer 単独では完結しない

## Reassess When

- catalogue schema v3 が更新された場合 (variant 追加、Role 値域変化等)
- mermaid layout engine の挙動が大きく変化した場合 (近接配置 / subgraph nesting 対応)
- `.harness/config/contract-map-style.toml` の schema が複雑化した場合 (edge style の組み合わせ表現が増えた場合等)
- subgraph nesting 深さの mermaid 制約が変わった場合 (現在 4 重を想定)
- action / signal overlay の必要性が顕在化した場合 (本 ADR で YAGNI 判断。`action` field が schema 上 core 必須になった事実はあるが renderer overlay の採否は別問題)

## Open Questions

以下は本 ADR では扱わない。将来の独立した type-designer / 実装 phase / 別 ADR で扱う:

- **P: port + adapter API 詳細** — `ContractMapRenderer` port の正確な method 署名、Adapter の構造、`RenderContractMapInteractor` との関係 (type-designer 領分、Phase 2 catalogue で正式化)
- **Q: spec_source edge** — catalogue entry から spec section への外向 edge (旧 ADR 2026-04-17-1528 §D4(3) 継承候補、本 ADR では採否未確定)
- **R: エラー処理 / fallback** — TypeRef parse 失敗、cross-catalogue lookup 失敗、設定ファイル不正値の error type と fallback 挙動 (実装 phase)
- **S: テスト戦略 / fixture** — layer-agnostic fixture (2-layer / 3-layer / 独自層名)、snapshot test (実装 phase で確認)
- **T: subgraph 間 edge mermaid syntax 確認** — entry subgraph → field type subgraph の mermaid 動作確認 (実装 phase で確認)

## Related

- `knowledge/adr/2026-04-17-1528-tddd-contract-map.md` — Contract Map の元 ADR。本 ADR で renderer 設計を全面更新 (catalogue v3 schema 対応)
- `knowledge/adr/2026-05-08-0248-tddd-catalogue-layer-schema-axis-separation.md` — catalogue v3 schema (3 軸分離、TypeKindV2 5 variants 等)。本 ADR の入力 contract
- `knowledge/adr/2026-05-08-0258-tddd-typegraph-hybrid-and-codec.md` — TypeGraph hybrid + codec。本 ADR は catalogue を入力に取り `ExtendedCrate` に依存しない
- `knowledge/adr/2026-05-08-0305-tddd-signal-evaluator-three-way-diff.md` — signal evaluator。action / signal は本 ADR で YAGNI 判断したが将来の連携可能性として noted
- `knowledge/adr/2026-04-29-0241-typestate-transition-edge-rendering.md` — typestate transition edge rendering。本 ADR Decision G で `transition_methods` 経由の遷移先導出 + 専用 style edge として継承
- `knowledge/adr/2026-05-18-1223-tddd-catalogue-inherent-impl-where-clause.md` — inherent_impls (InherentImplDeclV2) + where 句 (WherePredicateDecl) の導入。本 ADR Decision F で inherent_impls.methods を method ソースとして扱う根拠
- `knowledge/adr/2026-05-20-0048-tddd-catalogue-trait-impl-top-level.md` — TraitImplDeclV2 top-level 化 + TypeEntry.trait_impls 削除。本 ADR Decision J / O の trait impl edge 導出源の変更根拠
- `knowledge/conventions/hexagonal-architecture.md` — hexagonal 配置 (Decision E-3c の port + adapter pattern の根拠)
- `knowledge/conventions/no-backward-compat.md` — 旧 contract map renderer との非互換 (skip migration、新規 v3 専用設計)
