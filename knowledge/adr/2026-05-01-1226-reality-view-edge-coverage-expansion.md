---
adr_id: 2026-05-01-1226-reality-view-edge-coverage-expansion
decisions:
  - id: D1
    user_decision_ref: "chat_segment:reality-view-edge-coverage-expansion:2026-05-01"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:reality-view-edge-coverage-expansion:2026-05-01"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:reality-view-edge-coverage-expansion:2026-05-01"
    status: proposed
---
# Reality View renderer の edge カバレッジ拡張 — receiver-less method / trait-method incoming + 起源別視覚区別

## Context

TDDD の Reality View (rustdoc 入力ベースの type-graph view、ADR 2026-04-16-2200) では、直近トラック (`track/items/cli-via-usecase-only-2026-04-30/`) の `domain-graph-d2/domain_tddd.md` を観察すると 34 ノード中 6 ノードが孤立している (`CatalogueLinterError` / `CatalogueLinterRuleError` / `CatalogueLoaderError` / `ContractMapWriterError` / `ContractMapContent` / `TypestateTransitions`)。同様の傾向は他のクラスタにも見られる。

孤立の構造的原因は `libs/infrastructure/src/tddd/type_graph_render.rs::collect_edges()` の絞り込み方針にある。具体的には次のギャップが重畳している:

1. **Variant payload 無視** (line 404 `MemberDeclaration::Variant(_) => {}`) — enum variant が抱える workspace 型を edge 化しない。
2. **Receiver-less method の skip** (line 359 `if method.receiver().is_none() { continue; }`) — `pub fn try_new(...) -> Result<Self, FooError>` のような associated function は edge 起源にならず、`FooError` への edge が消える。
3. **Method 引数の無視** — 戻り値型 `method.returns()` のみを `extract_type_names` にかけており、引数型 (`method.params()`) は edge 起源として走査されない。
4. **Trait map の不走査** — `graph.type_names()` のみをループしており、trait の method (例: `CatalogueLinter::run -> Result<_, CatalogueLinterError>`) からの incoming edge は出ない。
5. **Trait impl edge は trait short name のみ** — `impl From<DomainError> for FooError` の `DomainError` のような generic 引数は edge にならない。

このうち項目 1 (variant payload) は schema 拡張 (catalogue / TypeGraph / baseline 3 輪、ADR 2026-04-26-0855 の Core invariant) を伴うため、**別 ADR に切り出す**。本 ADR は renderer 側のみで完結する項目 2 / 3 / 4 を扱う。項目 5 は本 ADR の D1 / D2 と独立した別軸の改善であり、ここでは scope 外とする。

Catalogue ベースの Contract Map (ADR 2026-04-17-1528) は kind ごとの `expected_methods` を `methods_of()` で一律に拾えるため、本 ADR が解決しようとするギャップは Contract Map 側では構造的に発生しない。Reality View 固有の改善である。

## Decision

### D1: receiver-less method を edge 起源に含める (戻り値 + 引数の両方)

`collect_edges()` の `EdgeSet::Methods` 分岐において、`method.receiver().is_none()` による early-continue を撤廃する。associated function (constructor / factory / parser) も edge 起源として走査する。

edge を派生させる入力は、receiver の有無に関わらず以下の 2 系統:

- **戻り値型** — `method.returns()` を `extract_type_names` にかけて得られる workspace 型名
- **引数型** — `method.params()` の各 `ParamDeclaration::ty()` を `extract_type_names` にかけて得られる workspace 型名

これにより、`fn try_new(rule_kind: CatalogueLinterRuleKind, ...) -> Result<Self, CatalogueLinterRuleError>` のような associated function から `CatalogueLinterRuleKind` および `CatalogueLinterRuleError` の双方への edge が発生する。

#### 実装上の補足 (D1)

- self-loop 抑制は現行 `target != source_name.as_str()` のロジックを引数側にも適用する。
- edge label は method name (`|method_name|`) を流用する。引数由来 / 戻り値由来の区別は label suffix ではなく edge 形状 + 色で表現する (詳細は D3)。
- edge_kind タグは戻り値由来を `"method_return"`、引数由来を `"method_param"` の 2 値に分離する (現行の `"method"` を 2 つに分割)。`render_edge_symbol` の dispatch は D3 で定義する。
- edge dedup は現行の `edges.sort(); edges.dedup();` で吸収できる (同一 (source, label, target, kind) 組はマージされる)。

```rust
// <!-- illustrative, non-canonical -->
for method in node.methods() {
    let return_targets = extract_type_names(method.returns());
    for target in return_targets {
        if graph_type_names.contains(target) && target != source_name.as_str() {
            edges.push((source_name.clone(), method.name().to_string(), target.to_string(), "method_return"));
        }
    }
    for param in method.params() {
        for target in extract_type_names(param.ty()) {
            if graph_type_names.contains(target) && target != source_name.as_str() {
                edges.push((source_name.clone(), method.name().to_string(), target.to_string(), "method_param"));
            }
        }
    }
}
```

### D2: trait の method から type への incoming edge を出す (戻り値 + 引数の両方)

`collect_edges()` に `graph.trait_names()` を走査する処理を追加する。各 trait について `trait_node.methods()` をループし、戻り値型 + 引数型から `extract_type_names` で抽出した workspace 型名を edge target とする。edge source は trait 自身のノードとする。

これにより、`CatalogueLinter::run(rules: &[CatalogueLinterRule], catalogue: &TypeCatalogueDocument, ...) -> Result<Vec<CatalogueLintViolation>, CatalogueLinterError>` のような trait method から、`CatalogueLinterRule` / `TypeCatalogueDocument` / `CatalogueLintViolation` / `CatalogueLinterError` への 4 本の edge が発生する。

#### 実装上の補足 (D2)

- trait ノードは現行 `EdgeSet::Impls` 経由で既にレンダー対象に上がっている (impl edge の target として stadium shape `([TraitName])` 表現)。本 D2 で edge source としても機能させる。D2 のコードブロックが `EdgeSet::Methods | EdgeSet::All` に対して走査するため、`EdgeSet::Methods` 単独で呼ばれる場合は impl edge が出力されず trait ノードがクラスタに存在しない可能性がある。この場合、D2 の edge source となる trait ノードをクラスタへ明示的に追加する（`graph.get_trait(trait_name)` で取得した trait ノードを、edge target が属するクラスタか、trait 自身の module-path 由来のクラスタへ割り当てる）。実運用上 `render_type_graph_clustered` は `EdgeSet::All` で呼ばれるため、`EdgeSet::Impls` 経由で trait ノードが既に挿入されているのが通常経路である。
- edge_kind タグは戻り値由来を `"trait_method_return"`、引数由来を `"trait_method_param"` の 2 値に分離する。impl edge (`-.impl.->`) との意味論差はそのまま維持する。
- 引数由来 / 戻り値由来の視覚区別 (mermaid symbol + 色) は D1 と同じく D3 で定義する。
- Trait は workspace に存在する trait のみを対象とする (`graph.trait_names()` がそもそも workspace 限定)。
- self-loop 抑制 (`target != trait_name`) を引数側 / 戻り値側ともに適用する。

```rust
// <!-- illustrative, non-canonical -->
if matches!(edge_set, EdgeSet::Methods | EdgeSet::All) {
    for trait_name in graph.trait_names() {
        if let Some(trait_node) = graph.get_trait(trait_name) {
            for method in trait_node.methods() {
                for target in extract_type_names(method.returns()) {
                    if graph_type_names.contains(target) && target != trait_name.as_str() {
                        edges.push((trait_name.clone(), method.name().to_string(), target.to_string(), "trait_method_return"));
                    }
                }
                for param in method.params() {
                    for target in extract_type_names(param.ty()) {
                        if graph_type_names.contains(target) && target != trait_name.as_str() {
                            edges.push((trait_name.clone(), method.name().to_string(), target.to_string(), "trait_method_param"));
                        }
                    }
                }
            }
        }
    }
}
```

### D3: 引数由来 edge と戻り値由来 edge を視覚的に区別する

D1 / D2 で発生する edge を、edge 起源 (引数 / 戻り値) に応じて mermaid 上で視覚的に区別する。**形状の差を主軸**にし、**色を補強**として与える (色のみでは モノクロ印刷 / カラーブラインド条件で識別困難なため)。

| 起源 | edge_kind | mermaid symbol | 線色 (linkStyle) | 視覚効果 |
|---|---|---|---|---|
| 戻り値由来 (D1 / D2) | `method_return` / `trait_method_return` | `-->|method_name|` | 既定 (テーマ色) | 実線 + 塗りつぶし矢印 (現行 method edge と同一) |
| 引数由来 (D1 / D2) | `method_param` / `trait_method_param` | `--o|method_name|` | 抑制色 (例: `#888` グレー) | 実線 + open circle 終端 (白抜き相当) |

mermaid flowchart の `--o` は終端を open circle で描画するため、塗りつぶし矢頭の `-->` と形状で即座に区別できる。`linkStyle` 指令を mermaid 出力末尾に追記し、引数由来 edge のインデックスをまとめてグレー化する。

field edge (`---`) と impl edge (`-.impl.->`) は本 D3 の影響範囲外であり、現行の表記をそのまま維持する。

#### 実装上の補足 (D3)

- `render_edge_symbol()` (`type_graph_render.rs:1077` 付近) の `kind` 引数 dispatch に 4 値を追加する。
  - `"method_return"` / `"trait_method_return"` → `format!("    {} -->|{}| {}", src, label, tgt)` (現行と同じ)
  - `"method_param"` / `"trait_method_param"` → `format!("    {} --o|{}| {}", src, label, tgt)`
- `linkStyle` 指令は mermaid block の末尾に集約する。`render_type_graph_clustered` / `render_type_graph_flat` の各関数で edge を出力した後、引数由来 edge の連番インデックスを収集して `linkStyle <i1>,<i2>,... stroke:#888;` を 1 行で append する (mermaid は edge の出現順に 0 から自動採番する)。`render_type_graph_overview` はクラスタ間 edge を集約して出力するため、個別 edge ごとのインデックスが存在せず、`linkStyle` による引数由来 / 戻り値由来の色分けは overview には適用しない。overview の edge はすべて既定色のままとする。
- 既定色は mermaid テーマに任せる (明示しない)。グレーは引数由来のみに適用し、戻り値由来 edge は既定値のままとする。
- legend (図例) は本 ADR では導入しない (`index.md` 上部の凡例ブロックは別 ADR で扱う) が、cluster ファイル先頭のコメントで `--o` が引数由来であることを 1 行で示す。

```rust
// <!-- illustrative, non-canonical -->
fn render_edge_symbol(src: &str, label: &str, tgt: &str, kind: &str) -> String {
    match kind {
        "method_return" | "trait_method_return" => format!("    {src} -->|{label}| {tgt}"),
        "method_param"  | "trait_method_param"  => format!("    {src} --o|{label}| {tgt}"),
        "field" => format!("    {src} ---|{label}| {tgt}"),
        "impl"  => format!("    {src} -.{label}.-> {tgt}"),
        _ => unreachable!(),
    }
}

// linkStyle aggregator (illustrative)
let param_indices: Vec<usize> = edges.iter().enumerate()
    .filter(|(_, (_, _, _, kind))| matches!(*kind, "method_param" | "trait_method_param"))
    .map(|(i, _)| i)
    .collect();
if !param_indices.is_empty() {
    let joined = param_indices.iter().map(usize::to_string).collect::<Vec<_>>().join(",");
    output.push_str(&format!("\n    linkStyle {} stroke:#888;\n", joined));
}
```

## Rejected Alternatives

### A. variant payload type 抽出も本 ADR に含めて一括解決する

本 ADR の D1 / D2 と並行して、`MemberDeclaration::Variant` に payload 型を持たせ、`collect_edges()` の `EdgeSet::Fields` 分岐で variant 由来の edge を出す案。

**却下理由**: variant payload を edge 化するためには `MemberDeclaration::Variant` schema に payload 型情報を載せる必要があり、ADR 2026-04-26-0855 の Core invariant (catalogue / TypeGraph / baseline 3 者の同時拡張) を起動する。本 ADR のスコープを 2 倍以上に膨らませ、レビュー / 実装 / リスクが非対称に増える。renderer 側のみで閉じる D1 / D2 と、schema 3 輪拡張を伴う variant payload は実装フェーズの粒度が異なるため、別 ADR に切り出して順序を分けて進める方が運用上安全。

### B. 戻り値のみを edge 起源にし、引数の edge 化は除外する

D1 / D2 で `extract_type_names` を適用する範囲を `method.returns()` のみに限定し、`method.params()` の走査は将来 ADR に送る案。

**却下理由**: Contract Map の Phase 1.5 拡張 (ADR 2026-04-17-1528 §D4) で「method 引数も edge 起源にする」判断が既に確定しており、Reality View だけ戻り値限定とすると 2 つの view の意味論が非対称になる。同じ rustdoc / catalogue 由来情報を扱いながら片方だけ引数を捨てるのは利用者にとって混乱要因。引数を含めることで「実装フローを入力側からも追える」という俯瞰価値が両 view で揃い、Contract Map と Reality View が drill-down 関係 (ADR 2026-04-16-2200 §D10) で並ぶときも一貫した読み筋を提供できる。

### C. 引数由来 / 戻り値由来の区別を label suffix または色のみで表現する

D3 の代替として 2 案を検討:

- **label suffix 案**: `-->|method_name(arg)|` のように label に `(arg)` を付与して引数由来であることを示し、矢印形状はすべて `-->` に統一する。
- **色のみ案**: 矢印形状は `-->` で統一し、`linkStyle` での色差分のみで引数 / 戻り値を区別する。

**却下理由**:

- label suffix 案は label 文字列が冗長化し、複数 edge 間で同名 method が並ぶ際 (例: `Foo -->|new(arg)| Bar` と `Foo -->|new| Bar`) の grep 性 / 視覚的整列が損なわれる。また label をキーとする dedup ロジックや既存テスト fixture も書き換えが広範囲に及ぶ。
- 色のみ案はモノクロ印刷 / ダークモード環境 / 一部のカラーブラインド条件で識別困難になる。形状による差異 (`-->` vs `--o`) はカラー独立して区別できる。色を補強として併用する方針 (D3 採用案) であれば色差分の利点も享受しつつカラー独立性も担保できるため、形状を主軸にする選択が優位。

## Consequences

### Positive

- **孤立ノードの大幅削減** — Reality View で観測されている孤立ノードのうち、constructor / associated function 経由でしか参照されない error 型 (`CatalogueLinterRuleError` 等) や、trait method 経由でしか参照されない型 (`CatalogueLinterError` / `CatalogueLoaderError` / `ContractMapWriterError` 等) は D1 / D2 で edge が発生し、視覚上の孤立から脱する。
- **receiver 以外の含意の可視化** — associated function (factory / parser) や trait method の引数 / 戻り値が edge として現れることで、「実装の入力 / 出力の依存関係」が `&self` 受けの inherent method に閉じない形で俯瞰できる。
- **Contract Map との対称性** — D1 / D2 はいずれも引数 + 戻り値を edge 起源とするため、Contract Map (catalogue 入力) との semantics が揃い、両 view を読み比べる際の認知コストが下がる。
- **引数由来 / 戻り値由来の視覚分離** — D3 により mermaid 上で形状 (`--o` vs `-->`) + 色 (グレー vs 既定) で起源が即座に区別でき、edge 数が増えても「入力依存 (argument)」と「出力依存 (return)」の読み分けが可能になる。Contract Map 側にもこのスキームを将来移植する余地が生じる。

### Negative

- **グラフ密度の上昇** — edge 起源が広がるため、特に大きなクラスタ (`domain::review_v2` 等) では edge 数が現状から数十パーセント増える可能性がある。mermaid の可読性閾値 (経験的に 50 ノード / 80 edges 付近) に近いクラスタでは追加の cluster 分割や filter が必要になる場合がある。
- **edge ノイズの可能性** — associated function には DTO 構築 (`Foo::new(Bar)`) など意味の軽い依存も含まれる。これらが構造的依存と同列に edge 化されることで、本来重要な依存が見えにくくなる懸念がある。

### Neutral

- **edge_kind 値の細分化に伴う render dispatch の拡張** — `"method"` の単一タグを `"method_return"` / `"method_param"` に分け、追加で `"trait_method_return"` / `"trait_method_param"` を新設するため、`render_edge_symbol` および snapshot テスト fixture の更新が必要になる。dispatch ロジック自体は match arm の追加で済むため複雑度は上がらない。
- **mermaid `--o` / `linkStyle` の互換性** — `--o` 終端と `linkStyle` 指令は mermaid v9 以降で安定動作するが、GitHub の mermaid renderer のバージョン依存があるため、レンダー結果が運用環境と PR diff 上で同一に見えるかは初回実装時に確認する。差異が観測された場合は線色のみのフォールバックを検討する余地を残す。

## Reassess When

- edge 密度の上昇が可読性を実用上損ねていると観察された場合 — cluster 分割粒度のデフォルト変更 (`--cluster-depth` の引き上げ)、edge filter の追加、または label 表記の差別化 (引数由来 / 戻り値由来の区別) を検討する。
- variant payload 拡張の別 ADR が実装され、`MemberDeclaration::Variant` から edge が出るようになった場合 — 本 ADR の D1 / D2 で発生した edge と variant payload edge の重複や干渉が発生していないかを再評価する。
- Reality View の位置付けが Contract Map drill-down (ADR 2026-04-16-2200 §D10) として確定し、独立 artifact として維持する必要が薄れた場合 — D1 / D2 の追加 edge が drill-down 用途として過剰でないかを再評価し、必要に応じてフィルタ等で縮小する。
- D3 の視覚スキーム (`--o` 終端 / `linkStyle` グレー) が GitHub mermaid renderer / IDE preview で意図通り表示されないことが観察された場合 — 形状差を別の native primitive (例: `==>` 太線で戻り値、`-->` 通常で引数) や stroke-dasharray 指定にフォールバックすることを検討する。
- 引数由来 / 戻り値由来の区別が運用上「あえて区別しない方が読みやすい」と判断された場合 — D3 の dispatch を解除し全 edge を `-->` に統一するロールバックを検討する。

## Related

- `knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md` — Reality View の元設計。本 ADR の対象である `collect_edges()` / `render_type_graph_clustered` のレンダリングパイプライン定義元。
- `knowledge/adr/2026-04-17-1528-tddd-contract-map.md` — Contract Map の元設計。本 ADR の D1 / D2 が引数を edge 起源に含めるのは Phase 1.5 (§D4) の判断との対称性を意図したもの。
- `knowledge/adr/2026-04-26-0855-tddd-feature-extension-with-verification.md` — Core invariant (catalogue / TypeGraph / baseline 3 輪同時拡張)。本 ADR が variant payload を別 ADR に切り出した根拠。
- `knowledge/adr/2026-04-28-0135-tddd-struct-kind-uniformization-and-catalogue-linter.md` — struct kind 均質化。Contract Map 側で同根の問題 (`methods_of()` が 3 種特例分岐) を扱った ADR。Reality View では同問題が `collect_edges()` の絞り込みとして異なる形で表面化していることを示す対比。
