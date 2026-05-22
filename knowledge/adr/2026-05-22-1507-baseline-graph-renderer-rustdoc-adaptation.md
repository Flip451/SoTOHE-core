---
adr_id: 2026-05-22-1507-baseline-graph-renderer-rustdoc-adaptation
decisions:
  - id: A
    user_decision_ref: "chat_segment:tddd-v2-baseline-graph-renderer-design:2026-05-13"
    candidate_selection: "from:[A-r1,A-r2,A-r3] chose:A-r3"
    status: proposed
  - id: B
    user_decision_ref: "chat_segment:tddd-v2-baseline-graph-renderer-design:2026-05-13"
    candidate_selection: "from:[B-r1,B-r2,B-r4] chose:B-r1"
    status: proposed
  - id: F
    user_decision_ref: "chat_segment:tddd-v2-baseline-graph-renderer-design:2026-05-13"
    candidate_selection: "from:[F-r1,F-r2,F-r3] chose:F-r1"
    status: proposed
  - id: O
    user_decision_ref: "chat_segment:tddd-v2-baseline-graph-renderer-design:2026-05-13"
    candidate_selection: "from:[O-r1,O-r2,O-r3] chose:O-r1"
    status: proposed
  - id: U
    user_decision_ref: "chat_segment:tddd-v2-baseline-graph-renderer-design:2026-05-13"
    candidate_selection: "from:[U-r1,U-r2,U-r3] chose:U-r3"
    status: proposed
  - id: X
    user_decision_ref: "chat_segment:tddd-v2-baseline-graph-renderer-design:2026-05-13"
    candidate_selection: "from:[X-1,X-2,X-3] chose:X-1"
    status: proposed
  - id: Y
    user_decision_ref: "chat_segment:tddd-v2-baseline-graph-renderer-design:2026-05-13"
    candidate_selection: "from:[Y-1,Y-2,Y-3] chose:Y-2"
    status: proposed
  - id: Z
    user_decision_ref: "chat_segment:tddd-v2-baseline-graph-renderer-design:2026-05-13"
    candidate_selection: "from:[Z-1,Z-2,Z-3,Z-4] chose:Z-4"
    status: proposed
  - id: BB
    user_decision_ref: "chat_segment:tddd-v2-baseline-graph-renderer-design:2026-05-13"
    candidate_selection: "from:[BB-1,BB-2,BB-3,BB-4-fix1] chose:BB-4-fix1+blanket-a"
    status: proposed
  - id: CC
    user_decision_ref: "chat_segment:tddd-v2-baseline-graph-renderer-design:2026-05-13"
    candidate_selection: "from:[CC-1,CC-2,CC-3] chose:CC-1"
    status: proposed
  - id: config_location
    user_decision_ref: "chat_segment:tddd-v2-baseline-graph-renderer-design:2026-05-13"
    candidate_selection: "from:[shared-file,separate-file,unified-rename] chose:separate-file"
    status: proposed
  - id: inherited_from_contract_map
    user_decision_ref: "chat_segment:contract-map-sibling-promotion-followup:2026-05-22"
    candidate_selection: "inherits:[C,D,E,H,H_prime,I,J,K,L,M,N] from:knowledge/adr/2026-05-20-2221-contract-map-renderer-catalogue-v3-adaptation.md"
    status: proposed
  - id: usecase_command_field_typing
    user_decision_ref: "chat_segment:r9-strict-command-typing:2026-05-22"
    candidate_selection: "from:[string-raw,typed-value-object] chose:typed-value-object"
    status: accepted
---
# Reality View Renderer: rustdoc_types::Crate 入力への対応設計 (v3 schema 移行)

## Context

### §1 旧 ADR が依拠した Reality View renderer の構造

ADR `2026-04-16-2200-tddd-type-graph-view` (Accepted、grandfathered) で導入された Reality View renderer (`<layer>-graph/index.md` + `<cluster>.md` 群) は、旧 TypeGraph 独自 schema (HashMap-based: TypeNode / TraitNode / FunctionNode / TraitImplEntry の inline 展開 + `TypeDefinitionKind` 単一 enum 13 variants) を前提として設計されていた。

旧 renderer は以下の構造で実装されていた:

- `libs/infrastructure/src/tddd/type_graph_render.rs` — overview renderer
- `libs/infrastructure/src/tddd/type_graph_cluster.rs` — cluster renderer
- `sotp track type-graph` CLI サブコマンド
- 入力: `&TypeGraph` (旧独自 schema)
- 出力: `<layer>-graph/index.md` + `<cluster>.md` 群 (`--cluster-depth N` で depth 制御)

### §2 TDDD v3 schema 移行による旧 renderer の廃止

ADR `2026-05-08-0258-tddd-typegraph-hybrid-and-codec` D2 で TypeGraph schema が 2 種に分離された:

- B / C / D: `rustdoc_types::Crate` 純粋 (rustdoc 出力そのまま)
- A / S: `ExtendedCrate` (action のみ薄く拡張)

この移行により:

- baseline 表現が `rustdoc_types::Crate` 純粋型に変更された (ADR 2026-05-08-0258 D2)
- 旧独自 TypeGraph schema (`TypeNode` / `TraitNode` 等) は廃止対象となった
- 既存の Reality View renderer (`type_graph_render.rs` / `type_graph_cluster.rs`) は入力 schema 不一致のため機能しなくなり、対象 track で削除された (stub のみ残存)
- `sotp track type-graph` CLI サブコマンドも同様に削除された

本 ADR は `rustdoc_types::Crate` 入力に対応する新規 Reality View renderer の設計を確定する。

### §3 renderer の役割 (旧 ADR §D10 継承)

旧 ADR §D10 の役割分担を v3 schema 用語で引き継ぐ。Contract Map renderer (sibling ADR `knowledge/adr/2026-05-20-2221-contract-map-renderer-catalogue-v3-adaptation.md`、renderer 実装済み) と complementary な位置付け:

| 観点 | Contract Map (sibling ADR) | Reality View (本 ADR) |
|---|---|---|
| 入力 | `&[CatalogueDocument]` (designer 宣言) | `&[BaselineDocument]` (rustdoc 出力 wrapper) |
| 出力 | 1 mermaid (`contract-map.md`、全層統合) | per-layer + per-cluster (`<layer>-graph-dN/index.md` + `<cluster>.md`) |
| 表す内容 | designer が宣言した contract 関係 | コンパイル後の実装状態 |
| scope | cross-layer 統合 | per-layer (layer 単位でクラスタ分割) |
| 主目的 | 設計意図の俯瞰 (primary artifact) | 実装状態の検証ドリルダウン |
| 入力 node 数 | 30〜70 (catalogue 由来) | 100+ (rustdoc 全乗せ) |

### §4 layer-agnostic 不変条件の継承

Contract Map ADR `2026-04-17-1528-tddd-contract-map` §4.5 の layer-agnostic 不変条件 (層名ハードコード禁止 / 層リストは `architecture-rules.json` 由来 / layer_order はトポロジカルソート) を本 ADR でも継承する。`LayerId` 型を採用し、layer 名の文字列リテラルは renderer 内に埋め込まない。

### §5 本 ADR の位置付け

本 ADR は `rustdoc_types::Crate` 入力に対応する Reality View renderer の設計を確定する。実装は将来の独立した track で行う。

## Decision

### A: renderer の入力 type (採択: A-r3)

renderer 関数のシグネチャ:

```rust
<!-- illustrative, non-canonical -->
pub struct BaselineDocument {
    pub layer: LayerId,
    pub crate_name: CrateName,
    pub krate: rustdoc_types::Crate,
}

pub fn render_baseline_graph_overview(
    baselines: &[BaselineDocument],
    layer: &LayerId,
    opts: &BaselineGraphRenderOptions,
) -> BaselineGraphOverviewContent;

pub fn render_baseline_graph_cluster(
    baselines: &[BaselineDocument],
    layer: &LayerId,
    cluster_module: &ModulePath,
    opts: &BaselineGraphRenderOptions,
) -> BaselineGraphClusterContent;
```

- `BaselineDocument` は新規 wrapper struct (3 フィールド: layer / crate_name / krate)。Contract Map の `CatalogueDocument` と symmetric な自己記述設計 (layer + crate_name フィールドを持ち、renderer 内で自律的に layer grouping が可能)
- ADR 2026-05-08-0258 D2 の「baseline = `rustdoc_types::Crate` 純粋」を維持する (`BaselineDocument` は wrapper に留まり、内部 `krate` は純粋型)
- 1 layer N crate 構成でも `Vec` に並列に格納するだけで対応できる (Contract Map A-3' と同じ設計思想)
- **注**: 上記シグネチャは illustrative であり正確な API は未確定。port method 化を含む API 詳細は Open Question P (type-designer 領分) で扱う。sibling Contract Map ADR では P が Decision として解決済みだが、本 ADR は実装前のため Open Question のまま据え置く

### B: Node 列挙対象 (採択: B-r1、5 種固定)

rustdoc `ItemEnum` から node 化する対象を enum で区別する:

```rust
<!-- illustrative, non-canonical -->
pub enum BaselineNode<'a> {
    Struct { doc: &'a BaselineDocument, id: Id, item: &'a Item },
    Enum   { doc: &'a BaselineDocument, id: Id, item: &'a Item },
    TypeAlias { doc: &'a BaselineDocument, id: Id, item: &'a Item },
    Trait  { doc: &'a BaselineDocument, id: Id, item: &'a Item },
    Function { doc: &'a BaselineDocument, id: Id, item: &'a Item },
}
```

対象 5 種の内訳:

- `ItemEnum::Struct` (UnitStruct / TupleStruct / PlainStruct の全 3 形態)
- `ItemEnum::Enum`
- `ItemEnum::TypeAlias`
- `ItemEnum::Trait`
- `ItemEnum::Function` (**standalone / top-level function のみ**。rustdoc では inherent impl method も `ItemEnum::Function` として表現されるが、inherent method は BB decision で entry subgraph に内包されるため standalone node 化の対象外。trait method (H' decision で Trait subgraph 内包) も同様に除外)

除外対象と理由:

- **Module**: subgraph 化 (U decision 経由)
- **Impl**: edge 化 (BB decision で詳述)
- **Variant / StructField / AssocType / AssocConst**: 親 Item の payload として処理 (H / K / H' から継承)
- **inherent method (Impl 内の Function)**: entry subgraph 内包 (BB decision)。B decision の Function 対象 = inherent impl や trait method ではなく top-level standalone function のみ
- **trait method (Trait 内の Function)**: Trait subgraph 内包 (H' decision)
- **Macro / ProcMacro / Use / ExternCrate / Primitive / ForeignType**: contract 表現の本質に薄い寄与のため除外

### C: shape / 色 / 線種は設定ファイル化 (Contract Map C を継承)

- **設定ファイル位置**: `.harness/config/baseline-graph-style.toml` (Reality View 専用)
- Contract Map (`.harness/config/contract-map-style.toml`) とは**別ファイル** (config_location decision を参照)
- section schema は Contract Map L decision から `[role.*]` を除いた構造: `[node.*]` + `[pattern.*]` + `[class.*]` + `[edge.*]` + `[filter]` (Reality View の入力は `rustdoc_types::Crate` のみであり catalogue role データを持たないため `[role.*]` は不要。詳細は L decision 参照)
- TypeEntry / TraitEntry の shape は subgraph 矩形に固定 (F decision の帰結として entry が subgraph 化されるため)。FunctionEntry は F decision でも subgraph 化されず standalone callable node として扱われるため、この subgraph 矩形固定の制約から外れる — FunctionEntry の node shape は設定ファイル `[node.Function]` の `shape` で別途指定する
- node category (`[node.*]`: Method/Variant/Field/Function 等) ベースの classDef で色 / 線種 / 太さを区別する。Reality View は catalogue role data を持たないため role ベースの classDef 区別は行わない

### D: node_id 命名 (Contract Map D を継承)

prefix で entry 種別を識別し、length-prefix で segment 境界を区別する (sanitize による同一化を除いた injective 性を維持する):

- Type (Struct / Enum / TypeAlias): `T<len>_<sanitized_layer>_<sanitized_crate>_<sanitized_module_path>_<sanitized_name>`
- Trait: `R<len>_<sanitized_layer>_<sanitized_crate>_<sanitized_module_path>_<sanitized_name>` (R for tRait)
- Function: `F<len>_<sanitized_layer>_<sanitized_crate>_<sanitized_full_path>`

`<len>` の定義は entry 種別で異なる:
- Type / Trait: `sanitized_layer` + `_` + `sanitized_crate` + `_` + `sanitized_module_path` + `_` + `sanitized_name` の文字数
- Function: `sanitized_layer` + `_` + `sanitized_crate` + `_` + `sanitized_full_path` の文字数

`sanitized_crate` は `doc.crate_name` を mermaid node_id 用に sanitize した値。`sanitized_module_path` は rustdoc `ItemSummary.path` (= `[crate_name, module_seg1, ..., item_name]`) から先頭の crate_name と末尾の item_name を除いた中間セグメント列を `_` で連結し sanitize した値 (crate root 直下の場合は中間セグメントなしのため空文字列とする)。

rustdoc 全乗せ (100+ types) を扱う Reality View では、同一 crate 内で同名の Type / Trait が異なる module に宣言される可能性があるため、`sanitized_module_path` を含めることで node_id の衝突を回避する (Contract Map D では catalogue の short name unique 前提があるため module path は不要だったが、Reality View では rustdoc 全出力が入力のため module path が必須)。

**注意**: sanitize は英数字・アンダースコア以外の文字を `_` に置換するため、非英数字文字のみで異なる名前が同一 sanitized 値となる可能性がある。`domain` / `usecase` / `infrastructure` のような実際のアーキテクチャ構成では問題が発生しない。この制限は実装 phase で必要に応じて hash suffix 付加等の追加衝突回避策を検討してよい。

### E: renderer の配置層 (Contract Map E を継承、ただし syn は不要)

hexagonal port + adapter pattern で配置する:

- `BaselineGraphRenderer` **port** を domain 層に新設
- **Adapter** は infrastructure 層。rustdoc が Type を既に構造化済みのため **syn は不要**
  - Contract Map adapter が syn を使用する (D9 の TypeRef parse) のに対し、Reality View adapter は rustdoc `Type` variant を直接操作するため syn 依存を追加しない
- usecase 層の interactor が port を inject される

### F: subgraph 化 + scale 対策の責務分離 (採択: F-r1)

- Struct / Enum / Trait / TypeAlias の entry を **subgraph 化** (Contract Map F decision を継承)
- **Function は subgraph 化しない** — standalone callable node として扱う (Contract Map F-2+d1 と同じ)
- Struct / Enum / Trait / TypeAlias entry は method node を subgraph 内に内包する
- **scale 対策 (rustdoc 全乗せの 100+ node) は U decision (depth 分割) で対応する**
- F 自体には depth 知識を埋め込まない (F = 構造の表現、U = 表示制御の分離)

### G: typestate transition edge (Reality View では適用しない)

typestate は catalogue 固有概念 (`CompositePattern::TypestateState` / `TypeKindV2::PlainStruct.typestate` field) であり、rustdoc には typestate marker が存在しない。

Reality View では typestate transition edge を描画しない。typestate 宣言のある method の returns edge は通常の method returns edge (`-->`) として処理する。

これは Contract Map G decision (typestate transition 専用 style `==>|transitions_to|`) と対称的な差分であり、両 renderer の役割分担 (設計意図 vs 実装状態) を反映する。

### H: enum variant node 化 + payload edge (Contract Map H を継承)

`rustdoc_types::ItemEnum::Enum.variants` の各 variant Item を node 化し、entry subgraph 内に配置する:

- variant payload が `VariantKind::Tuple(Vec<Option<Id>>)`: 各 Id を `krate.index` で lookup して `ItemEnum::StructField(Type)` から Type を取得し、各 Type へ無 label edge (`--o`)
- variant payload が `VariantKind::Struct { fields, has_stripped_fields }`: 各 field の field name を label に (`--o|field_name|`)
- variant payload が `VariantKind::Plain`: edge なし

### H': Trait method の表現 (Contract Map H' を継承)

Contract Map H'-1 と同じ: Trait subgraph 内に method node を内包する。

`rustdoc_types::ItemEnum::Trait.items` から method (Function variant) を抽出し、Trait subgraph 内に配置する。

### I: Function 列挙範囲 (Contract Map I を継承)

- default: visibility = Public の全 Function を render (filter なし)
- filter は将来 `[filter]` section 経由で設定ファイル対応

### J: trait impl edge の target 解決 (Contract Map J を継承)

全 trait impl で統一 edge style `-.impl.->` を使用する。

rustdoc `ItemEnum::Impl` の `trait_: Some(Path)` かつ具体型 (`blanket_impl: None` + `for_: Type::ResolvedPath`) の場合に edge を描画する。詳細は BB decision を参照。

### K: struct fields の表現 (Contract Map K を継承)

- `PlainStruct.fields` は node 化せず、entry subgraph から field type subgraph へ直接 edge `--o|field_name|`
- `TupleStruct.fields`: entry → field type へ edge、label は positional index (`.0` / `.1` / ...)
- `has_stripped_fields: true` / `None` slot の場合: render しない
- Newtype 検出 (TupleStruct + 1 field): lint 層に委譲 (renderer は通常 TupleStruct と同扱い)

rustdoc では `ItemEnum::Struct(rustdoc_types::Struct)` を通じて struct 種別に応じた field アクセスを行う:
- `StructKind::Plain { fields: Vec<Id>, has_stripped_fields }`: `has_stripped_fields` が `true` の場合は render しない。field は `Vec<Id>` で各 Id を `krate.index` で lookup して `ItemEnum::StructField(Type)` から型と field 名を取得する
- `StructKind::Tuple(Vec<Option<Id>>)`: `None` slot は stripped field (非表示) であり render しない。`Some(Id)` のみ lookup して positional index (`.0` / `.1` / ...) を label に使用する
- `StructKind::Unit`: field なし、edge を描画しない

### L: 設定ファイル schema (採択: Reality View 専用ファイル)

- 位置: `.harness/config/baseline-graph-style.toml` (Reality View 専用)
- repo commit 対象、default として同梱
- Reality View は `rustdoc_types::Crate` 入力で catalogue role data (DataRole/ContractRole/FunctionRole) を持たないため、`[role.*]` section は持たない
- section 構造 (Contract Map L から `[role.*]` / `include_function_roles` を除いた構成):
  - `[node.<NodeCategory>]` — NodeCategory は `{Method, Variant, Field, Function}` など、`shape` と `class` フィールド (Function は FunctionEntry standalone callable node のスタイル定義)
  - `[pattern.<PatternName>]` — `overlay_class`
  - `[class.<ClassName>]` — `fill` / `stroke` / `stroke_width` / `stroke_dasharray`
  - `[edge.<EdgeKind>]` — `arrow`, optional `label` (EdgeKind: method_param / method_returns / trait_impl / variant_payload / field / alias)
  - `[filter]` — `include_fields` 等
- 設定ファイル不在は **fail-closed エラー** (Contract Map L-8 と整合)

### M: action / signal overlay (Contract Map M を継承: YAGNI で不採用)

旧 ADR 2026-04-17-1528 §D5 の action overlay と signal overlay は本 ADR では採用しない。必要性が顕在化した際に別 ADR で追加検討する。

### N: TypeAlias.target の表現 (Contract Map N を継承)

`rustdoc_types::ItemEnum::TypeAlias.type_` (= `Type`) へ**無向 edge** `---|alias_of|` で表現する。

`TypeAlias.type_` の方向性はスキーマ上は有向だが、可視化目的では「A と B が alias 関係にある」という存在を示すことが主であり、読者が alias か target かを edge 向きで読み取る必要は低い。方向の明示が必要な場合は有向 `--o|alias_of|` に戻すことを検討する。

### O: cross-baseline trait_impl 解決 (採択: O-r1)

renderer 関数内でローカル cache として global trait index を 1 回構築する:

```rust
<!-- illustrative, non-canonical -->
// BTreeMap<(CrateName, ModulePath, TraitName), trait_subgraph_id: String>
// ModulePath = module path segments joined with "::" (empty string for crate root)
let trait_index: BTreeMap<(CrateName, String, TraitName), String> = build_trait_index(baselines);
```

構築手順:

1. 入力 `baselines: &[BaselineDocument]` を全走査 (1 回のみ)
2. 各 document の `krate.index` から `ItemEnum::Trait` の Item を抽出
3. `(doc.crate_name.clone(), module_path_from_paths, trait_short_name)` をキーに、trait_subgraph_id を値として格納 (`module_path_from_paths` は `krate.paths` の `ItemSummary.path` (= `[crate_name, module_seg1, ..., trait_name]`) から先頭の crate_name と末尾の trait_name を除いた中間セグメント列を `::` 連結した値; crate root 直下の場合は空文字列)
4. workspace 外の std / 外部 crate は **silent skip** (edge を引かない)

**理由**: Reality View は rustdoc 全乗せ (100+ types) を入力とするため、同一 crate 内で同名 Trait が異なる module に存在しうる。module path を含めることで trait identity の衝突を回避する (D decision の node_id scheme と整合)。

cross-baseline 解決時の trait name 抽出 algorithm (`Impl.trait_: Option<Path>` → `(CrateName, ModulePath, TraitName)`):

- `Path.id` を使用して `krate.paths.get(&path.id)` で `ItemSummary` を取得する
- `ItemSummary.crate_id` を `krate.external_crates` で解決して crate name を得る。`crate_id == 0` は自 crate (`impl_doc.crate_name`)
- `ItemSummary.path` の最後セグメントを trait short name とし、先頭セグメント (= crate name) を除いた中間セグメント列を `::` で連結して module path とする
- `(crate_name, module_path, trait_short_name)` でキーとして trait index を lookup する
- `Path.path` の先頭セグメントを crate name として使ってはならない (rustdoc の `Path.path` は use として書かれたパスであり、省略形・re-export・エイリアスを含む可能性がある)
- Id / crate_id が index / external_crates / paths に存在しない場合は silent skip (edge を引かない)
- lookup 失敗は silent skip (edge を引かない)

**ADR 2026-05-08-0258 D3 との整合**: Id は per-graph スコープに閉じる値であり (D3)、cross-baseline での Id 比較は行わない。trait_subgraph_id は renderer が独自に生成した文字列 id (D decision の node_id 規則に従う) であり、rustdoc の Id とは別物。

### U: cluster 構造 + depth 分割 (採択: U-r3)

rustdoc 全乗せの 100+ node に対する scale 対策として、**depth 1 (overview) + depth 2 (cluster detail)** の 2 段構成を採用する。旧 ADR §D4 の `--cluster-depth N` の自由度は捨て、cluster = 最上位 module で固定する。

**depth 1 (overview)**: `<layer>-graph-d1/index.md` 1 ファイル

- cluster (= `(crate_name, top-level module)` の組。1 layer N crate 構成で crate 間の同名 module 衝突を回避) を **1 つの node に縮約** (subgraph 化しない)
- cluster 間の edge group のみ表示 (cross-cluster edge を集約)
- cluster node は `:::cluster` 等の classDef で表現

**depth 2 (cluster detail)**: `<layer>-graph-d2/<cluster>.md` per cluster

- 該当 cluster 内の entry を Contract Map U-6d-iii と同じ subgraph 構造で表現
- 最上位 module 1 段 subgraph + sub-module path を entry subgraph label に含める (例: `team::manager::TeamManager`)
- cluster 内 edge のみ描画 (cross-cluster edge は depth 1 overview に集約)

**ファイル名規則**:

- cluster key は `(crate_name, module_seg1)` の組 (1 layer N crate 構成で異なる crate が同名 top-level module を持つ場合の衝突を回避)。`ItemSummary.path` = `[crate_name, module_seg1, ..., item_name]` における `(path[0], path[1])` に相当する
- ファイル名は cluster key を `<crate_name>_<module_seg1>.md` 形式で使用 (例: cluster key = `(domain, review)` → `domain_review.md`)
- crate root entry (module_seg1 が存在しない = `ItemSummary.path` が `[crate_name, item_name]` のみの場合) は cluster key `(crate_name, <root>)` として overview の `<crate_name> root` cluster および detail file `<crate_name>_root.md` に集約

**各 collection の iter 順**:

- cluster: alphabetical (BTreeMap)
- entry subgraph: alphabetical by name (BTreeMap)
- method / variant node: rustdoc `items` field の Vec 順 (declaration order)

**depth 1 mermaid 構造**:

```
1. classDef 定義 (.harness/config/baseline-graph-style.toml 由来)
2. layer subgraph (該当 layer 1 つ)
   2.1. cluster node 群 (cluster = (crate_name, top-level module)、alphabetical by cluster key)
        例: domain_review[domain::review]:::cluster  (cluster key = (domain, review))
        例: domain_user[domain::user]:::cluster      (cluster key = (domain, user))
3. cross-cluster edge group (cluster 間の edge を集約して表示)
4. class attach 群
```

**depth 2 mermaid 構造**:

```
1. classDef 定義
2. layer subgraph
   2.1. top-module subgraph (該当 cluster 1 つ、最上位 module)
        2.1.1. entry subgraph 群 (BTreeMap iter = alphabetical by name)
               - label に sub-module path 含む (例: team::manager::TeamManager)
               2.1.1.1. method / variant node (rustdoc Vec 順 = declaration order)
        2.1.2. FunctionEntry callable node 群 (同 top-module subgraph 内、entry subgraph と並列配置、alphabetical by FunctionPath)
3. edge 定義群 (cluster 内 edge のみ、cross-cluster は overview に集約済み)
4. class attach 群
```

mermaid syntax 注: subgraph には inline `:::className` が使えない (parse error)。`class <subgraphId> <className>` を別行で記述する必要がある (Contract Map U decision の Render Output 構造と同じ制約)。

### X: node 数爆発の追加対策 (採択: X-1 YAGNI)

U decision (cluster 分割) と CC decision (Public only) で実用上十分と判断する。

設定ファイルでの表示上限警告 / 自動 second-level cluster 展開は本 ADR では採用しない。必要性が顕在化した場合に別 ADR で追加する。

### Y: Entry Point 検出 (採択: Y-2 YAGNI)

旧 ADR §D3 の primitive + workspace 外引数による Entry Point 検出 + `:::entry` 強調は本 ADR では採用しない。

実装コストに対して必要性が顕在化しておらず、設定ファイル経由での on/off も schema の複雑化を招くため採用しない。必要性が顕在化したら別 ADR で追加する。

### Z: DRIFT detection (採択: Z-4 YAGNI)

旧 ADR §D8 の DRIFT-01 (architecture-rules.json 違反検出) / DRIFT-02 (orphan type マーク) は本 ADR では採用しない。

理由:

- DRIFT-01 は既存の `cargo make check-layers` (deny.toml + check_layers.py、crate-level) で carry されており重複する
- DRIFT-02 (orphan type) は false positive が多い (将来参照予定の declared 型 / 公開 API 等)
- Reality View の本質 (実装状態の俯瞰) に CI gate 機能は外挿であり関心が混在する

### BB: rustdoc Impl Item の処理 (採択: BB-4-fix1 + blanket 本体 a 案)

`rustdoc_types::ItemEnum::Impl` の各ケースの処理:

| ケース | 処理 |
|---|---|
| Inherent impl (`trait_: None`) | 対象 type の inherent method 集合として merge し、entry subgraph 内に node 化。複数 impl block は 1 entry にまとめる |
| Trait impl (`trait_: Some(Path)`、具体型: `blanket_impl: None` + `for_: Type::ResolvedPath`) | trait_impl edge (`-.impl.->`、J decision) を引く |
| `provided_trait_methods` (default 借用 method 名) | **skip** (Trait subgraph 側で declared 済み。H'-1 内包設計と整合し重複表示を回避) |
| `negative: true` (例: `impl !Send for T`) | **skip** (auto trait 関係、Reality View の本質から外れる) |
| `synthetic: true` (rustdoc auto-generated) | **skip** (user-declared でない) |
| `blanket_impl: Some(_)` (rustdoc が他型ページに展開した重複コピー) | **skip** (1 つの blanket 本体が N 型ページに展開される重複防止) |
| `blanket_impl: None` + `for_: Type::Generic` (blanket 本体、user 宣言) | **trait subgraph 内/近傍に表示** (a 案) |

**blanket 本体 (a 案) の処理詳細**:

`blanket_impl: None` かつ `for_` が `Type::Generic` の Impl は user が宣言した blanket impl の本体である。`for_` が generic 型であるため具体型 subgraph に紐づけられないため、対象 Trait の subgraph 内/近傍に表示する。これにより「この Trait に対する generic 実装」の semantic を視覚化できる。

**`blanket_impl: Option<Type>` の意味論**:

rustdoc は blanket impl (`impl<T: Bound> Trait for T`) の展開コピーを各具体型のページに生成する際に `blanket_impl: Some(concrete_type)` を設定する。user が宣言した blanket 本体は `blanket_impl: None` + `for_: Type::Generic` で表現される。この区別を正確に利用することで、user 宣言を無視せず、かつ重複表示を排除する。

### CC: visibility filter (採択: CC-1 Public only 固定)

- top-level entry (Struct / Enum / TypeAlias / Trait / Function) は `Item.visibility == Visibility::Public` のみを node 化対象とする
- **例外**: rustdoc は trait 関連 item (trait methods 等) と enum variant を `Visibility::Default` で表現する。これらは親 entry が Public であれば `Visibility::Default` でも node 化対象に含める (H decision の enum variant node 化 / H' decision の Trait method 内包と整合)
- private (`Visibility::Crate` / `Visibility::Restricted`) は除外
- 設定ファイルでの on/off 切替は不要 (シンプルさ優先)
- Contract Map と scope 整合 (ADR 2026-05-08-0258 D6 の `includes_private = false` 固定)

### config_location: 設定ファイル位置 (採択: Reality View 専用ファイル)

- 位置: `.harness/config/baseline-graph-style.toml` (Reality View 専用)
- Contract Map (`.harness/config/contract-map-style.toml`) とは**別ファイル**
- 役割分担 (Contract Map = 設計意図、Reality View = 実装状態) を設定ファイルの分離でも明示する
- section schema は Contract Map L から `[role.*]` / `include_function_roles` を除いた構造 (詳細は L decision 参照)

## Contract Map ADR から継承する Decisions

以下は Contract Map ADR (`knowledge/adr/2026-05-20-2221-contract-map-renderer-catalogue-v3-adaptation.md`、正式 ADR、renderer 実装済み) と完全同じ採用案を継承する。本 ADR では差分 (Reality View 固有の判断) のみ詳述し、共通部分は sibling ADR を参照する。

継承 decision 一覧:

- **C** (shape / 色 / 線種は設定ファイル化): 本 ADR では `.harness/config/baseline-graph-style.toml`。FunctionEntry は subgraph 化されず callable node のため node shape を `[node.Function]` の `shape` で指定する点は Contract Map C と同じ
- **D** (node_id prefix `T` / `R` / `F` + `sanitized_crate` 込み length-prefixed): `doc.crate_name` 由来の `sanitized_crate` を含む形で継承 (1 layer N crate での衝突回避)
- **E** (port + adapter pattern): そのまま継承。**ただし syn は不要** (rustdoc が Type を構造化済みのため)
- **H** (enum variant node 化 + payload edge): rustdoc `VariantKind` を使用する点で実装詳細は異なるが意味論は同じ
- **H'** (Trait method を Trait subgraph 内包): Trait items からの method 抽出を rustdoc Id 経由で行う点で実装詳細は異なる
- **I** (全 Function 列挙、filter は設定ファイル経由で将来対応): そのまま継承
- **J** (全 trait impl `-.impl.->` 統一 style): そのまま継承
- **K** (field は edge のみ、has_stripped_fields render なし、Newtype 検出は lint 委譲): rustdoc `StructField` Item 経由になる点で実装詳細は異なる
- **L** (設定ファイル section schema、ファイル不在は fail-closed、repo 同梱 default): 部分継承 (`[role.*]` / `include_function_roles` を除く — Reality View に catalogue role data なし)
- **M** (action / signal overlay は YAGNI): そのまま継承
- **N** (TypeAlias は無向 edge `---|alias_of|`): rustdoc `TypeAlias.type_` を使用。edge style (無向) は維持

## Rejected Alternatives

### A: renderer の入力 type

- **A-r1: `&[(LayerId, CrateName, rustdoc_types::Crate)]`** — tuple field に意味的ラベルがなく、呼び出し側・実装側のコードが読みにくい
- **A-r2: `&[CatalogueDocument] + BTreeMap<CrateName, &rustdoc_types::Crate>`** — Reality View が catalogue (designer 宣言) に依存することになり、「実装状態の検証」という本来の役割から外れる。CatalogueDocument と Baseline は独立に管理されるべき

### B: Node 列挙対象

- **B-r2: + Constant / Static** — Contract Map と非対称になり、contract 表現の本質への寄与が薄い
- **B-r4: 全 top-level Item** — node 数が爆発し、contract の価値が低下する

### F: subgraph 化 + scale 対策の責務分離

- **F-r2: depth 1/2 で subgraph 化方針を切り替え** — F 自体に depth 知識が混入し、F と U の関心事が分離できない
- **F-r3: 閾値超過時 fallback (node 数 > 50 で subgraph 化解除)** — 閾値 hard-code で柔軟性がなく、設定ファイルでの制御も困難になる

### O: cross-baseline trait_impl 解決

- **O-r2: rustdoc internal Id を活用** — ADR 2026-05-08-0258 D3 で Id は per-graph スコープに閉じる値とされており、cross-baseline での Id 比較は無効。name / path で identity する原則に反する
- **O-r3: external_crates も含めた拡張 index** — derive trait (Debug / Clone / PartialEq 等) での edge が大量発生し graph が肥大する。workspace 外 silent skip で十分

### U: cluster 構造 + depth 分割

- **U-r1: 旧 ADR §D4 の `--cluster-depth N` 維持** — Contract Map との非対称設計になる。また旧 ADR §Phase 2 Scope Update §S5.1 の実測 (depth=1 は実用性なし、depth=2 が適切) により depth 選択の自由度は実益が薄いことが判明している
- **U-r2: Contract Map U-6d-iii と同じ (d1/d2 とは別軸、cluster 縮約なし)** — 1 ファイルに全 cluster detail を詰め込むため node 数爆発。rustdoc 全乗せの 100+ node に対して有効な対策にならない

### X: node 数爆発の追加対策

- **X-2: `[filter] max_nodes_per_cluster` で警告** — 設定 schema が複雑化し、警告の対処法も定まらない
- **X-3: threshold 超過時 second-level cluster 自動展開** — 境界条件の semantic が複雑で、実装コストに対して必要性が顕在化していない

### Y: Entry Point 検出

- **Y-1: 旧 §D3 継承 (primitive allowlist + workspace 外型判定)** — 実装コストが高く、旧 ADR §Phase 2 Scope Update §S2 で延期となった理由 (現 codebase に typestate 0 件で entry point 強調の navigation 価値の実証が困難) が v3 schema 移行後も継続する
- **Y-3: 設定ファイル経由 on/off** — 機能そのものの必要性が顕在化していない段階で設定 schema を複雑化するのは YAGNI に反する

### Z: DRIFT detection

- **Z-1: DRIFT-01 + DRIFT-02 両方継承** — `cargo make check-layers` と重複し (DRIFT-01)、false positive リスクがある (DRIFT-02)
- **Z-2: DRIFT-01 のみ** — `cargo make check-layers` (crate-level deny.toml) との重複
- **Z-3: DRIFT-02 のみ** — orphan type の false positive リスクが高い (将来参照予定の型、公開 API など)

### BB: rustdoc Impl Item の処理

- **BB-1: 全 Impl variant を表示** — auto-generated (synthetic / negative / blanket コピー) がノイズとなり、graph が肥大する
- **BB-2: provided_trait_methods を Adapter 側で `:::default_impl` 装飾** — Trait subgraph 側との重複表示になる
- **BB-3: provided_trait_methods は両方表示 (Trait 側 + Adapter 側)** — 重複表示による情報ノイズ
- **BB-4 (修正前): blanket_impl 全部 skip** — user 宣言の blanket 本体まで除外する誤り。rustdoc の `blanket_impl: Option<Type>` は展開コピーを示すメタ情報であり、user が宣言した blanket 本体 (`blanket_impl: None` + `for_: Type::Generic`) は別物
- **blanket 本体 (b): layer subgraph 直下に独立配置** — どの module / type にも所属しないため位置が不定で、graph のレイアウトが不安定になる
- **blanket 本体 (c): 専用 `blanket_impls` subgraph に集約** — subgraph 追加で構造が肥大し、追加の classDef 定義が必要になる
- **blanket 本体 (d): render しない** — user が宣言した設計意図を視覚化から除外することになる

### CC: visibility filter

- **CC-2: 設定ファイル経由で on/off (default false)** — 柔軟性はあるが設定 schema が複雑化する。private を見たいケースは Reality View の主目的から外れる
- **CC-3: 常に private 含む** — node 数が爆発し、Reality View の本質 (public contract の実装状態) から外れる

### config_location: 設定ファイル位置

- **(i) Contract Map と共通ファイル** — 1 ファイルで両 renderer をカバーできるが、Contract Map と Reality View の役割分担をファイル分離で明示するという設計上の意図が薄れる
- **(iii) 統合ファイル (`.harness/config/tddd-graph-style.toml` 等に rename)** — 既存 Contract Map ADR のファイルパス参照を変更する必要があり、変更コストが高い

## Render Output 構造

Decision 群の集約として、生成する mermaid 出力の構造:

### depth 1 (overview): `<layer>-graph-d1/index.md`

```
1. classDef 定義 (.harness/config/baseline-graph-style.toml の [class.*] 由来、alphabetical) — [edge.*] は arrow/label 定義であり classDef には含まない
2. layer subgraph (該当 layer 1 つ)
   2.1. cluster node 群 (cluster = (crate_name, top-level module)、alphabetical by cluster key)
        例: domain_review[domain::review]:::cluster  (cluster key = (domain, review))
        例: domain_user[domain::user]:::cluster      (cluster key = (domain, user))
3. cross-cluster edge group (cluster 間の edge を集約して表示)
4. class attach 群 (`class <id> <className>` を別行で適用)
```

### depth 2 (cluster detail): `<layer>-graph-d2/<cluster>.md`

```
1. classDef 定義
2. layer subgraph
   2.1. top-module subgraph (該当 cluster 1 つ、最上位 module)
        2.1.1. entry subgraph 群 (BTreeMap iter = alphabetical by name)
               - label に sub-module path 含む (例: team::manager::TeamManager)
               2.1.1.1. method / variant node (rustdoc Vec 順 = declaration order)
        2.1.2. FunctionEntry callable node 群 (同 top-module subgraph 内、entry subgraph と並列配置、alphabetical by FunctionPath)
3. edge 定義群 (cluster 内 edge のみ、cross-cluster は depth 1 overview に集約済み)
4. class attach 群 (`class <subgraphId> <className>` を別行で適用)
```

## Consequences

### 良い影響

- baseline schema v3 (`rustdoc_types::Crate` 純粋、ADR 2026-05-08-0258 D2) と完全整合する
- Contract Map renderer (sibling ADR) と symmetric な設計 — port + adapter pattern / subgraph 構造 / node_id 規則 / 設定ファイル section schema を共有
- layer-agnostic 不変条件 (旧 ADR 2026-04-17-1528 §4.5) を遵守する (LayerId 採用)
- per-layer + per-cluster の depth 分割により node 数爆発を構造的に抑制する (U decision)
- Contract Map との役割分担が明確化される (Contract Map = 設計意図の俯瞰、Reality View = 実装状態のドリルダウン)
- syn 依存を追加しない (rustdoc が Type を既に parse 済みのため、Contract Map adapter との差分)
- typestate transition edge を持たないことで、catalogue にない情報を rustdoc から推測しようとする誤りを構造的に避けられる
- usecase Command / error の identity 系 field を validated domain value object で型付けすることで、構築時に不正な identity を排除できる (DD decision)。sibling Contract Map renderer の `RenderContractMapCommand` / `RenderContractMapError` も symmetric に同じ型付けへ揃える (modify)

### 悪い影響

- 旧 ADR 2026-04-16-2200 の `--cluster-depth N` の自由度を捨てた (cluster = 最上位 module 固定)
- 旧 ADR §D3 (Entry Point 検出) / §D8 (DRIFT detection) を YAGNI で不採用にしたため機能後退と見られる可能性がある
- 設定ファイル `.harness/config/baseline-graph-style.toml` 不在は fail-closed エラーとなるため、セットアップ要件が生じる
- `BaselineDocument` wrapper struct の導入で baseline 関連 type が増える
- cluster 跨ぎ edge を depth 1 overview に集約する処理ロジックの実装コストが生じる
- rustdoc の `Impl` Item の処理 (blanket / synthetic / negative の判別) で edge ケースが多く、実装時の注意が必要

## Reassess When

- rustdoc-types crate のメジャーバージョンアップで `Impl` / `Item` / `Type` 構造が破壊的に変更された場合
- Contract Map renderer (sibling ADR) が変更され symmetric 維持が破綻した場合
- Reality View の per-cluster ファイル数が極端に増えて GitHub の Markdown レンダリングが遅延する場合 (rough threshold: cluster 数 50 超)
- Entry Point 検出 / DRIFT detection の必要性が顕在化した場合 (Y / Z の YAGNI 判断を再評価)
- visibility = Public only 固定が運用上不便となり private 表示要求が出た場合 (CC decision 再評価)
- baseline 入力が `rustdoc_types::Crate` 以外に変わった場合 (例: 別 schema 採用、ADR 2026-05-08-0258 改訂)
- cluster 分割だけでは node 数が制御できないケースが頻発した場合 (X decision 再評価)

## DD: usecase Command / error の identity 系 field の型付け (採択: typed-value-object)

usecase 層の Command および error type が concept-bearing な identity 系 field (track_id / layer_id 等) を持つ場合、raw `String` ではなく validated domain value object (`TrackId` / `LayerId` 等) で型付けする。構築時に不正な identity が排除され、illegal state が表現不可能になる。

`RenderBaselineGraphCommand` / `RenderBaselineGraphError` が対象 (具体的な struct 定義は Open Question P の port + adapter API 詳細で確定するが、identity field の型付け方針は本 decision で先行確定する)。

## Open Questions (本 ADR scope 外、別 phase / 別 ADR で扱う)

以下は本 ADR では扱わない。将来の独立した type-designer / 実装 phase / 別 ADR で扱う:

- **P: port + adapter API 詳細** — `BaselineGraphRenderer` port の正確な method 署名、Adapter の構造、interactor との関係 (type-designer 領分)
- **Q: blanket impl の trait subgraph 配置の細部** — trait subgraph 内に直接配置 / trait subgraph に隣接する `:::blanket_impl` indicator node を置く等の意匠決定 (実装 phase)
- **R: エラー処理 / fallback** — rustdoc Path 解釈失敗、設定ファイル不正値の error type と fallback 挙動 (実装 phase)。cross-baseline lookup 失敗の挙動は O decision で silent skip と確定済みのため本 Open Question の対象外
- **S: テスト戦略 / fixture** — layer-agnostic fixture、snapshot test、rustdoc JSON fixture (実装 phase)
- **T: subgraph 間 edge mermaid syntax 確認** — Contract Map ADR と共通の課題、実装 phase で確認

## Related

- `knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md` — Reality View の元 ADR (Accepted、grandfathered)。本 ADR は v3 schema 対応として renderer 設計を全面更新
- `knowledge/adr/2026-04-17-1528-tddd-contract-map.md` — Contract Map の元 ADR (§D6 で Reality View との役割分担を規定)
- `knowledge/adr/2026-05-20-2221-contract-map-renderer-catalogue-v3-adaptation.md` — Contract Map renderer の v3 対応 sibling ADR (renderer 実装済み)。本 ADR と symmetric な設計
- `knowledge/adr/2026-05-08-0258-tddd-typegraph-hybrid-and-codec.md` — TypeGraph hybrid + codec。本 ADR の入力 (baseline = `rustdoc_types::Crate` 純粋) の根拠 (D2)
- `knowledge/conventions/hexagonal-architecture.md` — hexagonal port + adapter pattern の根拠
- `knowledge/conventions/no-backward-compat.md` — 旧 Reality View renderer との非互換 (skip migration、新規 v3 専用設計)
