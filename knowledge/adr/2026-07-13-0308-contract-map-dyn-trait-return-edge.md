---
adr_id: 2026-07-13-0308-contract-map-dyn-trait-return-edge
decisions:
  - id: D1
    user_decision_ref: "chat_segment:adr-add:2026-07-13:contract-map-dyn-trait-return-edge-promotion"
    candidate_selection: "from:[node-index-trait-merge,ghost-node,do-nothing,two-track-candidate-collection] chose:two-track-candidate-collection"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:adr-add:2026-07-13:contract-map-dyn-trait-return-edge-promotion"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:adr-add:2026-07-13:contract-map-dyn-trait-return-edge-promotion"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:adr-add:2026-07-13:contract-map-dyn-trait-return-edge-promotion"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:adr-add:2026-07-13:contract-map-dyn-trait-return-edge-promotion"
    status: proposed
---
# contract-map renderer: `dyn Trait` return/param edge の解決

## Context

contract-map renderer は TypeRef 文字列を syn で parse し、抽出した型名を
`NodeIndex`（`libs/infrastructure/src/tddd/contract_map_renderer_adapter/render/node_index.rs`）
で解決して edge を張る。この `NodeIndex` は **TypeEntry のみ**を登録し、TraitEntry を
意図的に除外している（node_index.rs の設計コメント: 型と trait の同名衝突で TypeRef が
誤って trait subgraph に link する事故を防ぐため。trait_impl edge の解決は別経路
`build_trait_index` + `resolve_trait_subgraph` が担う）。

このため、method / function の returns / params や struct の field など、TypeRef 位置に現れる trait object 型
（例: factory port の `fn build(&self, ...) -> Arc<dyn SomePort>`）は edge の張り先と
して解決されず、**黙って読み飛ばされる**（silent skip）。hexagonal 設計では「factory port が
`Arc<dyn Port>` を返す」「interactor が `Arc<dyn Port>` を注入される」形が頻出するため、
port 間の依存 edge が contract-map 上で系統的に欠落する。catalogue の宣言自体は正しく
検証も通るのに rendered view にだけ依存が現れない、という SoT 整合性の死角になる。

なお `collect_type_names_from_syn`（render/type_ref.rs）は `Type::TraitObject` を
意図的に無視している（ignore コメントに列挙）。抽出を単純に追加しても `NodeIndex` に
trait が登録されていないため解決に失敗する — 抽出（D1）と解決の受け皿（D2）の両方が必要。

## Decision

### D1: TraitObject 由来の候補を通常の型候補と分離して収集する

`collect_type_names_from_syn` の出力を「型候補」と「trait 候補」の 2 系統に分ける
（tagged enum または 2 本の `Vec<String>`）。`Type::TraitObject` では、各
`TypeParamBound::Trait` の bound path 自身を trait 候補にのみ積む。lifetime bound は
候補にしない。既存の型候補の抽出経路は不変である。

trait bound path の generic 引数および associated-type binding の**値側**は、通常の
型候補として再帰走査する。一方、trait bound path 自身（例: `Iterator` / `Port`）を
型候補に混ぜない。したがって `dyn Iterator<Item = DeclaredType>` と
`dyn Port<DeclaredType>` は trait edge に加え、内部の `DeclaredType` が catalogue に
宣言されていれば従来どおり type edge も張る。`dyn Port + Send` のような複数 bound は
各 trait 候補を独立に解決し、宣言済みのものすべてに edge を張る。未宣言の `Send` 等は
既存方針どおり黙ってスキップする。

これにより node_index.rs の設計コメントが懸念する「plain TypeRef が同名 trait に
誤 link する」「型と trait の同名で曖昧化する」事故を構造的に排除する:
plain な path 名は従来どおり NodeIndex（型のみ）で解決し、`dyn` 文脈で現れた名前
だけが trait 解決を受ける。

### D2: `resolve_type_ref_node_ids` に trait_index fallback を追加する

シグネチャに `trait_index: &BTreeMap<(String, String), String>` を追加する。通常の
型候補は従来どおり `NodeIndex` のみで解決し、trait 候補だけを `trait_index` fallback
で以下の順に解決する。通常の型候補を trait に fallback させないため、type と trait の
同名衝突時にも plain TypeRef の意味は変わらない。

1. 非修飾名（bare name）→ `(current_crate, name)` で trait_index を引く（自 crate の trait）
2. `crate::` / `self::` / `super::` prefix を持つ修飾付きパス → prefix を除いた末尾
   segment を trait 名として `(current_crate, name)` で引く（catalogue TypeRef 規約どおり
   自 crate 参照として扱う。中間 segment は解決に使わない）
3. それ以外の修飾付きパス（qualified path）→ 先頭 segment を crate、末尾 segment を trait 名として
   `(crate, name)` で引く（他 crate の trait）
4. どちらでも引けない → 黙ってスキップ（workspace 外部の trait は edge を張らないという既存実装の方針を踏襲）

この解決規則を `resolve_trait_subgraph` にも適用して共通ロジックを関数抽出し、
`dyn Trait` edge と trait_impl edge の両方で再利用する。したがって既存の trait_impl
`trait_ref` も `crate::` / `self::` / `super::` prefix を上記 2 の自 crate 解決へ正規化する。
これは catalogue TypeRef 規約との整合を取る意図的な既存 resolver の意味論拡張である。
bare name の自 crate 限定、prefix を持たない修飾付きパスの他 crate 解決、未宣言・
workspace 外 trait の黙ってスキップという trait_impl の既存挙動は不変とする。edge の張り先は
`build_trait_index` が既に格納している trait の代表ノード id
（`trait_rep_node_id`）であり、subgraph id ではない（クラスタ境界のレイアウト破壊回避も既存方針を踏襲）。
戻り値は両系統で解決した代表ノード id の和集合とし、同じ id は 1 回だけ返す。

### D3: trait_index を呼び出し経路に引き回す

`emit.rs` の `resolve_type_ref_node_ids` / `resolve_method_type_refs` 呼び出し
（field / variant / alias / fn param / fn return / method param / method return の
約 8 箇所）に trait_index を渡す。trait_index は render 本体
（`render/mod.rs`）で trait_impl edge 用に既に構築済みのため、追加コストはゼロ
（参照を配るだけ）。この配線により、returns / params に限らず field / variant / alias 位置に現れる trait object も同じ規則で解決される。

### D4: edge style key は出現位置の既存 key を流用する

`dyn Trait` への edge に新しい矢印種を導入しない。解決済み trait target は、その
TypeRef が置かれた既存の emit 経路と同じ style key を使う: fn / method の param は
`[edge.method_param]`、通常の return は `[edge.method_returns]`、transition method の
return は既存どおり `[edge.transition]`、struct field は `[edge.field]`、variant payload
は `[edge.variant_payload]`、type alias は `[edge.alias]` である。style config の追加は不要で、
当該 key が欠ける場合の fail-closed 挙動も不変とする。

### D5: 回帰テスト

renderer のテスト（`contract_map_renderer_adapter/mod.rs` の `#[cfg(test)] mod tests`）に
以下の 7 ケースをすべて追加する:

1. 正常系: `fn build(&self) -> Arc<dyn DeclaredPort>` を持つ trait entry +
   同 catalogue に DeclaredPort trait entry → `build --> DeclaredPort_rep` edge が
   描画される
2. 外部 trait: `Arc<dyn std::fmt::Debug>` 等 catalogue 外 trait → edge なし
   （黙ってスキップし、panic しない）
3. 同名衝突: type `Foo` と trait `Foo` が併存する catalogue で、plain TypeRef `Foo`
   は型に、`Arc<dyn Foo>` は trait に、それぞれ正しく link する
4. 複数 bound: `Arc<dyn DeclaredPort + DeclaredMarker>` で両方の宣言済み trait に
   edge を張り、同じ位置に未宣言の `Send` を加えても追加 edge を出さない
5. generic / associated type: `Arc<dyn DeclaredPort<DeclaredType, Item = DeclaredType>>`
   で `DeclaredPort` への trait edge と `DeclaredType` への通常の type edge の両方を張る
6. 自 crate prefix: `Arc<dyn crate::port::DeclaredPort>`、`Arc<dyn self::port::DeclaredPort>`、
   `Arc<dyn super::port::DeclaredPort>` はいずれも `(current_crate, DeclaredPort)` として
   解決し、`DeclaredPort_rep` への trait edge を張る
7. 共通 resolver: trait_impl の `trait_ref` に上記 3 種の自 crate prefix を使っても、
   `DeclaredPort_rep` への trait_impl edge を張る。bare name、prefix を持たない他 crate
   path、および外部 trait の既存解決結果も不変である

## Rejected Alternatives

### A. NodeIndex に TraitEntry を混載する

最小 diff だが、node_index.rs の設計コメントが明示的に禁止する同名曖昧化を再導入する。
plain TypeRef が trait subgraph に誤 link する退行が既存 catalogue でも
起こり得るため却下。

### B. dyn Trait 出現時のみ「ghost node」を生成する

未宣言 trait への edge を点線の ghost node として描く案。「edge は宣言済み型の間のみ」という
renderer の既存原則（catalogue-declared entries only）に反するため却下。

### C. 何もしない（catalogue docs で補足）

可視化の欠落は「catalogue は正しいのに rendered view が矛盾する」状態を恒常化させ、
types review のたびに SoT 整合性 findings を誘発する。恒久的に「容認済みの逸脱」と
注記し続けるコストの方が高いため却下。

## Consequences

- Positive: hexagonal 頻出パターン（factory port / `Arc<dyn Port>` 注入）の依存が
  contract-map に正しく現れ、types review の誤検出が消える
- Positive: 解決規則を trait_impl edge と共通化することで renderer 内の二重実装を防ぐ
- Negative: `resolve_type_ref_node_ids` のシグネチャ変更が render 系の呼び出し箇所 8 箇所 +
  テストに波及（見積り約 100–150 行 + テスト）
- Neutral: style config / mermaid 出力形式は不変

## Reassess When

- `impl Trait` 戻り値など TraitObject 以外の trait 出現位置も可視化したくなったとき
- renderer が mermaid 以外の出力形式を持つとき（解決規則の共有方法を再設計）

## 実装メモ

- 対象ファイル: render/type_ref.rs（D1/D2）、render/emit.rs（D3）、
  render/node_index.rs（共通解決関数の抽出先候補）、
  `contract_map_renderer_adapter/mod.rs` の test module（D5）
- 実装規模の見積り: 約 100–150 行 + テスト。単一のモジュール群
  （contract_map_renderer_adapter）に閉じる
- catalogue 変更は不要の見込み: `resolve_type_ref_node_ids` は pub(crate) のため
  公開 API 面に影響しない — 着手時に要再確認
