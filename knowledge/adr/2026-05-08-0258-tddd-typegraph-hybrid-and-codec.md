---
adr_id: 2026-05-08-0258-tddd-typegraph-hybrid-and-codec
decisions:
  - id: D1
    user_decision_ref: "chat_segment:tddd-v2-typegraph-codec-design:2026-05-08"
    candidate_selection: "from:[A-wrapping-only,B-full-custom-schema,C-hybrid] chose:C-hybrid"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:tddd-v2-typegraph-codec-design:2026-05-08"
    candidate_selection: "from:[single-ExtendedCrate,two-type-split] chose:two-type-split"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:tddd-v2-typegraph-codec-design:2026-05-08"
    candidate_selection: "from:[full-path-key,short-name-plus-incremental-id,short-name-plus-per-graph-id-with-rebuild] chose:short-name-plus-per-graph-id-with-rebuild"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:tddd-v2-typegraph-codec-design:2026-05-08"
    candidate_selection: "from:[flat-path,module-path-included] chose:module-path-included"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:tddd-v2-typegraph-codec-design:2026-05-08"
    status: proposed
  - id: D6
    user_decision_ref: "chat_segment:tddd-v2-typegraph-codec-design:2026-05-08"
    status: proposed
  - id: D7
    user_decision_ref: "chat_segment:tddd-v2-typegraph-codec-design:2026-05-08"
    status: proposed
  - id: D8
    user_decision_ref: "chat_segment:tddd-v2-typegraph-codec-design:2026-05-08"
    candidate_selection: "from:[keep-inline,convert-to-id-ref] chose:convert-to-id-ref"
    status: proposed
  - id: D9
    user_decision_ref: "chat_segment:tddd-v2-typegraph-codec-design:2026-05-08"
    candidate_selection: "from:[keep-string,parse-to-resolved-path] chose:parse-to-resolved-path"
    status: proposed
  - id: D10
    user_decision_ref: "chat_segment:tddd-v2-typegraph-codec-design:2026-05-08"
    candidate_selection: "from:[warning-plus-dummy-id,error-reject-codec,open-world-A-closed-world-S] chose:open-world-A-closed-world-S"
    status: proposed
  - id: D11
    user_decision_ref: "chat_segment:tddd-v2-typegraph-codec-design:2026-05-08"
    status: proposed
  - id: D12
    user_decision_ref: "chat_segment:tddd-v2-typegraph-codec-design:2026-05-08"
    candidate_selection: "from:[declare-required,auto-derive-workspace,identity-only-external,identity-only-all] chose:identity-only-all"
    status: proposed
  - id: D13
    user_decision_ref: "chat_segment:tddd-v2-cross-crate-typeref-matching:2026-05-13"
    status: proposed
---
# TDDD TypeGraph hybrid 構造と Catalogue → TypeGraph codec

## Context

### §1 TDDD-01 / TDDD-02 による型カタログ多層化の経緯

ADR `2026-04-11-0002-tddd-multilayer-extension.md` (TDDD-01) は型カタログを per-layer + 多層化し、TypeGraph 構造を独自 schema (HashMap-based: types / traits / functions の 3 分離 + inline 展開) で定義した。

ADR `2026-04-11-0001-baseline-reverse-signals.md` (TDDD-02) は baseline-aware reverse signal を導入し、4 グループ評価 (A\B / A∩B / B\A / ∁(A∪B)∩C) で signal を出すようにした。baseline は TypeGraph 構造のスナップショットとして保存される。

### §2 TypeGraph の internal / external 表現 hybrid 化の方針

上記 ADR が確定した後、TypeGraph の内部表現を `rustdoc_types::Crate` (rustdoc-types crate v0.57.3、`libs/infrastructure/src/schema_export.rs` が既に使用中) ベースに根本再設計する方針が決まった。

「内部表現 = `rustdoc_types::Crate` / 外部 (catalogue) = 独自軽量 schema」の hybrid 構造を採用することで、3 種の TypeGraph (A: Catalogue 由来 / B: Baseline / C: Current) を共通の rustdoc_types::Crate 形式で統一し、schema レベルの 3-way 突合を可能にする。

### §3 Catalogue layer schema との分担

Catalogue layer schema は ADR `2026-05-08-0248-tddd-catalogue-layer-schema-axis-separation.md` (以下「ADR 1」) で確定している。ADR 1 は `CatalogueDocument` のフィールド構成 (types / traits / functions の 3 分離、Role / Pattern / Action の軸分離、厳密 payload-encoded schema) を定義した。

本 ADR は ADR 1 で確定した `CatalogueDocument` を入力として、TypeGraph 内部表現と Catalogue → TypeGraph codec の設計を決定する。

### §4 現状 TypeGraph 実装との関係

現状の `libs/domain/src/schema.rs` は TypeNode / TraitNode / FunctionNode / TraitImplEntry を HashMap で管理する独自 schema であり、rustdoc_types::Crate の subset を独自命名で再編成したものに相当する。本 ADR の採用により、この内部表現を `rustdoc_types::Crate` ベースに置き換える必要がある。

## Decision

### D1: TypeGraph hybrid 構造の採用

TypeGraph の internal / external 表現を以下のように分担する。

| 層 | schema | TDDD-specific 情報 |
|---|---|---|
| TypeGraph (内部) | `rustdoc_types::Crate` ベース (B / C / D は純粋、A / S は action 拡張) | B / C / D は持たない。A / S は action のみ (docs は Item.docs に encode 済み、突合対象外) |
| Catalogue (外部) | ADR 1 で確定した独自軽量 schema (`CatalogueDocument`) | role / pattern / action / docs を declare |
| Codec | Catalogue → TypeGraph の一方向変換 | bridge |

B (Baseline) と C (Current) は rustdoc 出力をそのまま `rustdoc_types::Crate` として保持する。A (Catalogue 由来) と S (突合中間表現) は `rustdoc_types::Crate` を action のみで薄く拡張した `ExtendedCrate` として保持する。**D (削除予定の中間表現、ADR 3 D2 で構築) は各 Item が暗黙 action=Delete のため `rustdoc_types::Crate` 純粋とする** (item_actions field は不要)。role / pattern は Catalogue 側に残置し、linter のみに使用する。docs は Catalogue 側に declare し、codec が rustdoc `Item.docs` フィールドに encode する (突合対象外。render / 文書化のみに使用、D7 参照)。

### D2: TypeGraph schema 2 種への分離

突合アルゴリズムが必要とする action のみを TypeGraph に attach し、role / pattern は `CatalogueDocument` 側に残置する。docs は `CatalogueDocument` で declare し codec が rustdoc `Item.docs` に encode する (突合対象外、D7 参照)。schema を以下の 2 種に分離する。

```
<!-- illustrative, non-canonical -->
// A, S 用 (Catalogue 由来 + 突合アルゴリズム中間表現)
pub struct ExtendedCrate {
    krate: rustdoc_types::Crate,
    item_actions: BTreeMap<Id, ItemAction>,
}
pub enum ItemAction { Add, Modify, Reference, Delete }
// S では Delete は現れない (S 構築時に D へ移動)

// B, C 用 (rustdoc 出力そのまま)
pub type Baseline = rustdoc_types::Crate;
pub type Current  = rustdoc_types::Crate;

// D 用 (Delete 予定中間表現、暗黙 action=Delete のため item_actions 不要)
pub type DeleteSet = rustdoc_types::Crate;
```

TypeGraph (A / B / C) と Phase 1 中間表現 (S / D) のうち、**A と S は `ExtendedCrate`** (action 拡張)、**B / C / D は `rustdoc_types::Crate` 純粋** とする。

| Component | 入力 | 役割 |
|---|---|---|
| Linter | `CatalogueDocument` | role / pattern × 構造制約の enforcement |
| Catalogue → A codec | `CatalogueDocument` | `ExtendedCrate` (A) を build |
| Signal evaluator | A + B + C | S, D 構築 → 3-way 評価 |

B は突合時に動的に action を付与して S 構築の素材とする (B 自体は不変のまま保持する)。

### D3: Type identity mapping = name / path で identity / Id は ExtendedCrate スコープ

- catalogue の String key (TypeName / TraitName) は rustdoc `Path.path` の最後セグメント (短名) と一致させる
- 同名異モジュール衝突は catalogue 側で禁止する (declare 型名がユニーク前提)
- function の key (FunctionPath = crate_name + module_path + name) は、ADR 1 D11 で `new` / `build` 等の短名重複が日常的なため full path key を採用する。突合時は FunctionPath の全セグメントで identity する
- 突合は types / traits は **short name で identity**、functions は **FunctionPath (full path) で identity** する。`Id` (rustdoc 流の `u32` wrapper) は **ExtendedCrate スコープに閉じる per-graph 値** であり、graph 横断で同じ値が同じ Item を指すことは保証しない
- 各 graph (A 単独 codec = ExtendedCrate / S 構築 = ExtendedCrate / D 構築 = `rustdoc_types::Crate`) は **per-graph に flat incremental 発番** する (Id 0 = root module 予約、Id 1, 2, ... を順次割当)
- B (rustdoc 生出力) と A (codec 出力) の Id 空間は独立であり衝突しうる。S 構築時に Item を取り込む際は name / path → 新 Id mapping を再構築して **S 内で Id を rebuild** し、衝突を解消する。具体操作は ADR `2026-05-08-0305-tddd-signal-evaluator-three-way-diff.md` の D2 (Phase 1) を参照
- catalogue 変更によって id がシフトしても、突合は name / path で identity するため影響しない

### D4: `Crate.paths` の path 形式 = module path 込み

各 in-crate item に対して `paths: HashMap<Id, ItemSummary>` の entry を生成する。

- `path = [crate_name, ...module_path, item_name]` (例: `module_path = ["review"]` のとき `["domain", "review", "Draft"]`)
- `module_path` は ADR 1 D5 の `ModulePath` 型 (`Vec<Identifier>`)。catalogue では `#[serde(default)]` で省略可、空 vec が default で「crate root 直下」を意味する (ADR 1 D7)。空 vec の場合 path は `[crate_name, item_name]` (module path セグメントなし) になる
- catalogue で `module_path` を declare し、rustdoc 出力 (TypeGraph B / C) と path 構造を一致させる
- crate_name は `CatalogueDocument` の `crate_name: CrateName` フィールドを使用する (ADR 1 D6 で確定)

path 構造を一致させることで、突合アルゴリズムが cross-crate 参照や突合精度を安全に確保できる。

### D5: `external_crates` の自動 build = TraitImplDecl.origin_crate + TypeRef crate prefix + std prelude allowlist から抽出

- 自 catalogue の `crate_name` を crate_id 0 に予約する (rustdoc 流儀: crate_id == 0 は current crate)
- catalogue の `TraitImplDecl.origin_crate` と `TypeRef` 中の crate prefix を集めて unique な外部 crate name を抽出する
- D11 で定義する std prelude allowlist (Vec / Option / Result / String / Box 等) を prefix なしで使った TypeRef も暗黙的に `"std"` 外部 crate 参照として抽出し、`external_crates` に `"std"` エントリを追加する (crate_id を発番する)
- 各外部 crate name に crate_id 1, 2, ... を incremental 発番する
- workspace 内別 crate も catalogue 単位では `external_crates` 扱いで統一する (rustdoc 流儀と一致)
- `html_root_url` / `path` は空 / None 固定とする (catalogue にない情報。突合アルゴリズムは読まない)

なお `crate_id` は **graph スコープに閉じる per-graph 値**であり (D3 の Id と同じ原則)、A 単独 codec / S 構築 / D 構築 で別空間として再発番される (A / S は ExtendedCrate、D は `rustdoc_types::Crate` 純粋)。S 構築時には S 内の残存 Item (Delete 処理後) が参照する外部 crate を抽出し、S スコープで crate_id 1, 2, ... を再発番する (具体操作は ADR `2026-05-08-0305-tddd-signal-evaluator-three-way-diff.md` の D2 (Phase 1) を参照)。重複は merge され、S 内に同 crate name の entry が複数現れることはない。Delete 宣言によって外部 crate 依存が S 側で消えるケースでは、その外部 crate は D.external_crates 側にのみ残る。

### D6: catalogue にない rustdoc フィールドの固定値処理

rustdoc 出力 (TypeGraph B / C) には実値が入るが、Catalogue (TypeGraph A) は手書きのため持たない。突合アルゴリズムはこれらを比較対象としない。

- `format_version` = codec で固定値 (rustdoc-types crate のバージョンに対応するコンパイル時定数、例: 45 for v0.57.3)
- `crate_version` = `None` 固定
- `target` = 空 / template 固定
- `includes_private` = `false` 固定 (catalogue は public API のみを declare する)
- `root` = Id 0 を予約し、root module item を生成する。root Module の `children` フィールドに catalogue で declare した全 type / trait / function の Id を集約する

### D7: Item の trivial フィールドの扱い

以下のフィールドは突合対象外として固定値で処理する。

- `span` (ファイル位置) = `None` 固定
- `links` (intra-doc link) = 空 HashMap 固定
- `attrs` (`#[attr]` 列) = 空 Vec 固定 (`#[derive(...)]` 等の derive macro は catalogue の `trait_impls` で declare し、attrs では扱わない)
- `deprecation` = `None` 固定 (現状 TDDD では未扱い)
- `docs` (`///` doc comment) = catalogue の docs フィールド (`TypeEntry` / `TraitEntry` / `FunctionEntry` / `MethodDeclaration`、ADR 1 D9) から rustdoc `Item.docs` に encode する (突合対象外。render / 文書化のみに使用)

### D8: inline → id 参照変換

catalogue は inline 表現 (`FieldDecl` / `VariantDecl` が `TypeKind` の payload に inline) を取るが、rustdoc は id 参照表現 (`StructField` / `Variant` が別 Item として `index` に id で登録) を取る。codec が field / variant ごとに Id を発番して `index` に追加し、親 (Struct / Enum) からは `Vec<Id>` で参照する。

inherent method の grouping は 1 type = 1 Inherent Impl block とする (catalogue で declare した全 methods を 1 つの impl block に集約する)。

```
<!-- illustrative, non-canonical -->
// User struct + 2 inherent methods の例
Id 0: root module
Id 1: User struct
Id 2: User の Inherent Impl block (trait_: None, items: [Id 3, Id 4])
Id 3: User::new method (Function)
Id 4: User::email method (Function)
```

### D9: TypeRef の generics parse

catalogue の `TypeRef(String)` は generics を含む L1 文字列 (例: `"Result<Option<User>, DomainError>"`) として保持される。codec がこれを parse して各 identifier (`Result`, `Option`, `User`, `DomainError`) に id を解決し、rustdoc 流の `Type::ResolvedPath(Path { id, args: Some(Box<GenericArgs>) })` 構造に変換する。

各 identifier の id 解決は **catalogue 内 closed** で行う。catalogue の types / traits / `external_crates` (D5 の自動抽出結果) のいずれかに登録されていれば id を発番、いずれにも該当しない identifier は **未解決マーカー** で保持する (A は open-world)。未解決マーカーの schema 表現 (sentinel id / 別 BTreeMap / 仮想 `<unresolved>` crate 等) は実装段階で決定する。closed-world 検証は S 構築時に行う (D10 / ADR 3 D2 を参照)。

string → 構造変換の parser は **`syn` crate** (Rust syntax 解析で広く使われる) を活用する想定であり、tokenizer / generics nesting handler を自前実装する必要はない。codec が書くのは `syn::parse_str::<syn::Type>(...)` で得た構造化 AST を `rustdoc_types::Type` の各 variant (`ResolvedPath` / `Primitive` / `Tuple` / `Slice` 等) にマッピングする変換ロジックのみとなる。`syn` 依存は実装段階で確定するが、本 ADR は前提として `syn` 利用を想定して悪影響を評価する。

### D10: A は open-world / S は closed-world (closed-world 検証は S 構築時)

A codec は **open-world** で TypeRef parse を進める。catalogue 内 declare 型のみ id 発番し、未 declare 型は未解決マーカーで保持する (D9)。catalogue 単独で完結することは要求しない (catalogue は変更宣言 ≒ 差分宣言であり、baseline 由来型を芋づる式に Reference 宣言する必要がない)。

**closed-world 検証は S 構築時に行う**。ADR 3 D2 (Phase 1) で A の未解決マーカーを **Delete 処理後の S** の name match で resolve し、resolve 不能なら Phase 1 Error として reject する。S は Phase 2 (3-way 評価) に進む時点で完全 resolve 状態である (closed-world)。S には B 由来の Reference 型および A 由来の Add/Modify 型が残存しており、Delete 済み型は S に存在しない。

declare 漏れの早期検出は失われるが、Phase 1 が同等の検出粒度で carry する (Delete 処理後の S が事実上の closed universe set として機能する)。setup ミスの典型的な症状 (catalogue の typo / 名前不一致) は Phase 1 Error で検出される。

### D11: TypeRef の crate prefix で参照先を判別

- prefix なし (例: `"UserId"`) → 同 catalogue 内 declare 型を参照。ただし以下の 2 ケースは自動解決する: (1) `bool` / `u32` / `i64` / `usize` 等の Rust primitive は `Type::Primitive` に encode する (Phase 1 Error にしない); (2) `Vec` / `Option` / `Result` / `String` / `Box` 等の std prelude 型は prefix なしで書くことが慣用的であり、codec は固定の std prelude allowlist を持ち、その型名を自動的に `"std"` 外部 crate 参照 (`Type::ResolvedPath`) として解決する (Phase 1 Error にしない)。allowlist 外の未解決マーカーのみ Phase 1 で検証する
- `"std::HashMap"` → 外部 crate (std) の型。`external_crates` 経由で id 発番する
- `"domain_core::UserId"` → workspace 内別 crate の型。同様に `external_crates` 扱いとする

### D12: trait impl の codec 処理 — impl identity のみ encode (signature 比較は不要)

ADR 1 D10 で `TraitImplDecl` が trait identity (`trait_name + origin_crate`) のみを持つ schema になったことを踏まえ、codec は trait impl の methods を空 Vec として encode する。同 catalogue trait / 別 catalogue trait / 外部 crate trait の区別なく、すべての trait impl は trait identity のみで encode され、methods 比較は突合の対象外となる。

trait def と impl の signature 整合は Rust コンパイラが保証する範囲であり、TDDD signal evaluator が二重チェックする必要はない。突合 algorithm は trait impl identity の一致のみを判定する: declare されたが impl されていない / declare されないが impl されている のような impl identity の差を signal 化し、methods signature の差はコンパイル時に Rust コンパイラが検出する。

A codec は trait impl の signature を導出するために cross-catalogue を読み込む必要がなくなる。ADR 1 D6 (1 catalogue = 1 crate の独立性) が codec レベルで完全に成立する。

### D13: 外部クレート修飾 TypeRef は C と同形の triple-structure で表現する

外部クレートを参照する TypeRef について、A (`ExtendedCrate`) の内部表現を C (`rustdoc_types::Crate`) が foreign 参照を表すときに使う triple-structure と同形で持つ。

**triple-structure の定義**

(i) `Crate::external_crates: HashMap<u32, ExternalCrate>` に対象外部 crate のエントリが登録されていること。このエントリは D5 の自動 build 範囲に含まれる (TraitImplDecl.origin_crate + TypeRef crate prefix + std prelude allowlist からの抽出)。

(ii) `Crate::paths: HashMap<Id, ItemSummary>` に対象外部 crate の参照対象 item のエントリが登録されていること。当該エントリの `crate_id` は (i) の `external_crates` キーと一致する 0 以外の値である。

(iii) 当該 TypeRef は `Type::ResolvedPath { id, path, .. }` として保持し、`id` は (ii) の `paths` エントリを指す。

この 3 つが揃った形が C 側で foreign 参照を表す形式であり、A もこの形式で外部クレート修飾 TypeRef を表現する。

**id 値と突合の関係**

id 値は graph スコープに閉じる per-graph 値であり (D3 / D5)、A の id と C の id が等しい必要はない。S 構築時に S スコープで id が再発番される。突合は **shape ベース** — (i)(ii)(iii) の構造 + `paths` の path 値 + `external_crates` の crate name — で行い、id 値そのものを比較しない。

**既存決定との関係**

- D5 (`external_crates` の自動 build): TraitImplDecl.origin_crate + TypeRef crate prefix + std prelude allowlist から外部 crate を抽出するルールはそのまま。D13 は build 結果が `paths` と `Type::ResolvedPath` まで一貫して流れる shape を要求するだけで、抽出ルール自体を変えない。
- D9 (TypeRef generics parse): catalogue 内 closed で id 解決し、catalogue に declare のない identifier は未解決マーカーで保持する原則はそのまま。D13 は **解決できた crate-prefixed 識別子について A が C と同形を取る** という追加制約である。
- D10 (A: open-world / S: closed-world): A は未 declare 型を未解決マーカーで保持し、closed-world 検証は S 構築時に行うという構造はそのまま。
- D11 (TypeRef crate prefix の参照先判別ルール): prefix あり / なし / primitive / std prelude の解釈はそのまま。
- D12 (trait impl identity-only encode): そのまま。

**`SignalEvaluatorPort::evaluate` の signature**

ADR `2026-05-08-0305-tddd-signal-evaluator-three-way-diff.md` D5 で確定した `evaluate(a, b, c)` の 3 入力 signature はそのまま。D13 は内部表現の shape を揃える decision であり、port の入力数・型を変えない。

**multi-crate rustdoc 読込の扱い**

D13 は multi-crate rustdoc 読込を要求しない。A 側で必要な triple-structure はすべて catalogue + D5 の自動 build から組み立てる。

**alignment 機構の選択**

A/S 構築時のみで shape を揃える方式と、C を rustdoc 出力から所定形に正規化する処理を 1 箇所追加する方式の双方が考えられる。具体的な機構選択は D9 の先例 ("未解決マーカーの schema 表現 ... は実装段階で決定する") に従い、**実装段階で決定する**。

## Rejected Alternatives

### A: TypeGraph = `rustdoc_types::Crate` の wrapping のみ

TDDD-specific 情報 (role / action / pattern) を別の並列 collection で保持し、TypeGraph 自体は `rustdoc_types::Crate` の thin wrapper とする案。TypeGraph の純粋性は保てるが、突合アルゴリズムが `CatalogueDocument` を別途読む必要が生じ、TypeGraph A の id と `CatalogueDocument` の name を対応付ける処理が複雑化する。A 構築時に action を落とすと S や D を構築できないという問題も生じる。action のみを薄く拡張する D2 採用で解決するため却下。

### B: TypeGraph = 完全独自 schema

Item / ItemData / ItemTrait / Inherit のような独自命名と独自構造で TypeGraph を設計する案。書きやすさを重視できるが rustdoc_types::Crate との codec 距離が遠く、突合アルゴリズムの 3-way diff が複雑になる。「内部表現 = rustdoc_types::Crate に振る」という方針で D1 採用となったため却下。

### C: `Crate.paths` の path に module path なし (crate_name + item_name のみ)

catalogue での `module_path` declare を不要にし、`paths` を flat (crate_name + item_name の 2 セグメント) にする案。catalogue の記述コストが下がるが、rustdoc 出力 (B / C) は実際の module 構造を反映した path を持つため、突合時に TypeGraph A と B / C で path 構造が一致しなくなる。cross-crate 参照や突合精度が低下するため却下。

### D: hash-based Id 発番

name + parent からハッシュで deterministic に Id を生成する案。catalogue が不変なら id も不変という利点があるが、collision risk とデバッグ困難の欠点がある。突合は name で identity するため flat incremental で id が変わっても影響しない。シンプルさを優先して flat incremental (D3) を採用し却下。

### E: TypeRef を文字列のまま保持

codec で parse せず、TypeRef 文字列のまま rustdoc 流構造に変換しない案。`rustdoc_types::Crate` の `Type` 構造が維持できなくなるため突合精度が低下し、外部 crate 参照や generics の id 解決が不可能になる。D9 の generics parse を採用したため却下。

### F: 未 declare 型を A codec で Error reject (closed-world for A)

catalogue の types に prefix なし TypeRef が未 declare の場合、A codec で即座に Error reject する案 (旧 D10)。setup ミスの早期検出ができる利点があるが、catalogue で baseline 由来型を芋づる式に Reference 宣言する負担が発生し、catalogue が「変更宣言」(差分宣言) であるという ADR 1 D4 Action 軸の方針と矛盾する。**A は open-world、S は closed-world** という責務分離 (現 D10) を採用したため却下。declare 漏れの検出は Phase 1 (S 構築時) の Error reject で carry される。

### G: workspace 内別 crate を `external_crates` と区別

workspace 内 crate を `external_crates` に入れず別の channel で保持する案。codec が workspace 内 / 外部の二重判定を必要とし複雑化する。rustdoc 流儀との不一致も生じるため却下。

### H: trait impl methods を declare 必須とする

`Vec<MethodDeclaration>` として宣言を必須にする案。`#[derive(Debug)]` のような derive trait で全 methods を declare するのは冗長であり、ADR 1 D10 で `methods` field 自体が撤廃 (identity-only) された。本 ADR D12 は identity-only の codec 処理を確定する形で発展するため、declare 必須化はそもそも対象外となった。却下。

### I: workspace 内全 trait を auto-derive (旧 D12 採用案)

catalogue の `TraitImplDecl.methods: None` のとき、codec が workspace 内全 catalogue を読み込んで trait def を resolve し、signature まで複製する案 (本 ADR の旧 D12)。利点: signal evaluator が signature 比較できる。欠点: codec が cross-catalogue を読み込む必要があり、D6 (1 catalogue = 1 crate の独立性) が codec レベルで崩れる。さらに本質的に、trait def と impl の signature 整合は Rust コンパイラが保証する範囲であり、TDDD が signature 比較する必要そのものがない。impl identity-only 案 (現 D12) を採用したため却下。

### J: 同 catalogue trait のみ auto-derive、別 catalogue trait は identity-only

同 catalogue 内の trait のみ signature を auto-derive し、別 catalogue / 外部 crate の trait は identity-only とする中間案。codec の cross-catalogue 解決は不要になるが、同 catalogue 内 trait のみ signature 比較する根拠が「コンパイラ保証範囲を一部 TDDD で重ねる」という中途半端な立場になる。コンパイラ保証範囲は同 catalogue / 別 catalogue / 外部 crate を区別しないため、論理的整合性を取るなら identity-only 案 (現 D12) に振り切る方が clean。

### K: Id を graph 横断で永続化する

TypeGraph A / B / C / S / D を通じて同じ Item は常に同じ Id を持つよう、Id を graph 横断で共有・永続化する案。catalogue 変更や rustdoc の再実行があっても Id が変わらないため、外部ツールや保存済みスナップショットとの連携が容易になる。しかし、rustdoc が発番する Id (B / C 側) はビルドごとに変わりうるため、catalogue 側との Id 同期を維持するには複雑な tracking 機構が必要になる。突合が name で identity する (D3) 以上、Id の永続性は突合精度に影響しない。S 構築時の rebuild (D3 採用案) で衝突を解消する方が実装がシンプルになるため却下。

## Consequences

### 良い影響

- rustdoc 出力との 1:1 対応が取れる。B / C は `rustdoc_types::Crate` 純粋なため rustdoc から直接 deserialize 可能で、追加 codec が不要になる。
- 突合 codec が明確になる。A (ExtendedCrate) と B / C (rustdoc_types::Crate) の構造比較が同じ `Crate` 形式で行えるため、3-way diff が直接的になる。
- TypeGraph の拡張が最小限で済む (action のみ)。role / pattern / docs は `CatalogueDocument` 側に残置するため、TypeGraph schema の rustdoc_types::Crate に対する追加がほぼない。
- Catalogue → TypeGraph 変換が一意に定まる。D3-D9 で codec の各処理が確定し、catalogue の各フィールドが rustdoc 構造のどの位置に encode されるかが明確になる。
- declare ミスの早期検出ができる。D10 では A codec は open-world で進めるが、Phase 1 (S 構築時、ADR `2026-05-08-0305-tddd-signal-evaluator-three-way-diff.md` D2) で closed-world 検証が行われ、未解決マーカーが Delete 処理後の S で resolve できなければ Error reject される。typo / 名前不一致は Phase 2 (突合) に到達する前に検出される。
- cross-workspace 対応が統一表現で取れる。D11 の crate prefix ルールで workspace 内別 crate と外部 crate の参照が同じ表現になる。
- A codec が単一 catalogue で完結する。D12 で trait impl を identity-only にしたため、cross-catalogue 解決が不要となり、D6 の crate 単位独立性が codec レベルで完全に成立する。
- TDDD と Rust コンパイラの責務分離が明確になる。trait def と impl の signature 整合性検証は Rust コンパイラに任せ、TDDD は declare と current 実装の差 (impl identity の有無) のみを signal 化する。

### 悪い影響

- TypeRef の string → 構造変換が必要になる (D9)。ただし parser は `syn` crate を活用するため自前 parser 実装は不要で、codec が必要なのは `syn::Type` → `rustdoc_types::Type` の variant 変換ロジックのみとなる。実装コストは tokenizer / parser を自前実装する場合に比べて大幅に低い。
- `format_version` 等の固定値管理が発生する。D6 で codec が固定値を埋めるが、rustdoc-types crate のバージョンアップ時に `format_version` の値を更新する必要がある。
- inline → id 参照変換のコストが増す (D8)。各 field / variant / method に Id を発番して `index` に追加する変換ロジックの複雑性が上がる。catalogue 上の inline declare が rustdoc では分散表現になるため、デバッグ時に対応関係を辿りにくくなる。
- 既存 TypeGraph 実装からの移行コストがかかる。既存 `libs/domain/src/schema.rs` の TypeNode / TraitNode / FunctionNode / TraitImplEntry を `rustdoc_types::Crate` ベースに置き換える必要があり、TypeGraph を読む既存コード (consistency / signals / contract_map_render 等) の書き換えが必要になる。

## Reassess When

- rustdoc-types crate のメジャーバージョンアップで Item / Type / Path 構造が破壊的に変わった場合
- `format_version` 固定値管理のコストが増した場合 (rustdoc-types crate のバージョン更新頻度が上がる場合)
- コンパイラ非対応の signature 検証要求が出た場合 (例: macro-generated trait impl の signature をユーザー declare で検証したい場合 / proc-macro により展開される method signature の TDDD レイヤでの可視化要求): D12 / ADR 1 D10 で `methods` field の復活を検討
- 既存 TDDD-01 / TDDD-02 で構築された baseline-aware reverse signal の運用と本 ADR の TypeGraph 構造に齟齬が生じた場合
- nightly 必須の rustdoc JSON が stable に移行した場合 (CI 構成の簡素化と `format_version` の安定性向上が期待できる)

## Related

- 本 ADR の前提:
  - `knowledge/adr/2026-05-08-0248-tddd-catalogue-layer-schema-axis-separation.md` — ADR 1 (Catalogue layer schema)。本 ADR は ADR 1 で確定した `CatalogueDocument` schema を入力として codec を設計する
- 既存 ADR との関係:
  - `knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md` — TDDD-01 (型カタログ多層化、TypeGraph 構造を独自 schema で定義)。本 ADR で TypeGraph 構造を `rustdoc_types::Crate` ベースに移行する形で進化させる
  - `knowledge/adr/2026-04-11-0001-baseline-reverse-signals.md` — TDDD-02 (baseline-aware reverse signal、4 グループ評価)。baseline 概念を継承する。ただし baseline schema は本 ADR で `rustdoc_types::Crate` 純粋に変更される
  - `knowledge/adr/2026-04-11-0003-type-action-declarations.md` — TDDD-03 (action フィールド)。本 ADR の `ItemAction` (D2) として継承し、`ExtendedCrate.item_actions` に attach する
  - `knowledge/adr/2026-04-26-0855-tddd-feature-extension-with-verification.md` — Core invariant ADR (catalogue / TypeGraph / baseline / serde codec の 4 点同時更新)。本 ADR は TypeGraph と codec 部分を再定義する形で Core invariant を維持する
- 後続 ADR:
  - `knowledge/adr/2026-05-08-0305-tddd-signal-evaluator-three-way-diff.md` — Signal evaluator ADR (S 構築 + D 構築 + 3-way 評価)。本 ADR の TypeGraph schema (D2) を入力として signal evaluator を設計する。
- `knowledge/adr/README.md` — ADR 索引
