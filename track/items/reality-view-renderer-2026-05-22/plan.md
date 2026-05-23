<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# reality-view renderer の rustdoc_types::Crate 入力対応 (v3 schema 移行)

## Tasks (16/17 resolved)

### S1 — Domain layer: 3 port + 3 error type + BaselineDocument

> BaselineDocument wrapper struct + 3 secondary port (BaselineGraphLoader / BaselineGraphRenderer / BaselineGraphWriter) + 各 error type (BaselineGraphLoaderError / BaselineGraphRendererError / BaselineGraphWriterError) を domain crate に新設する。
> Contract Map の CatalogueDocument / CatalogueLoader / ContractMapRenderer / ContractMapWriter と symmetric な設計。domain → infrastructure 依存なし (hexagonal 分離)。
> syn 依存は追加しない (rustdoc が Type を構造化済み)。

- [x] **T001**: domain layer: BaselineDocument wrapper struct + 3 secondary port (BaselineGraphLoader / BaselineGraphRenderer / BaselineGraphWriter) + 3 error type (BaselineGraphLoaderError / BaselineGraphRendererError / BaselineGraphWriterError) を libs/domain に新設する。各 port は Send + Sync bound を持つ。error type は Display / Error / Debug を impl する (IN-01, IN-02, IN-19, AC-01, AC-02) (`cf2caa3c2175bcc835b53f1e4f6e9696c642ee89`)

### S2 — Usecase layer: interactor + Command / Error (3-port compose)

> RenderBaselineGraph primary port + RenderBaselineGraphInteractor<L,R,W> + Command / Output / Error を usecase crate に新設する。
> interactor は L: BaselineGraphLoader / R: BaselineGraphRenderer / W: BaselineGraphWriter の 3 generic で compose される (IN-19)。
> load → render → write の 3 段パイプラインを hexagonal purity (CN-03) に従って usecase 内に実装する。
> Command の identity 系フィールド (track_id / layer_filter) は validated domain value object (TrackId / LayerId) で型付けする (CN-12)。

- [x] **T002**: usecase layer: RenderBaselineGraph primary port + RenderBaselineGraphInteractor<L,R,W> (L: BaselineGraphLoader, R: BaselineGraphRenderer, W: BaselineGraphWriter の 3 generic compose) + RenderBaselineGraphCommand / RenderBaselineGraphOutput / RenderBaselineGraphError を libs/usecase に新設する。interactor は load → render → write の 3 段パイプラインを compose する (IN-02, IN-19, AC-02, CN-12) (`246ef29b9107c9ca04bd50602905a273c6de66f9`)

### S3 — Sibling modify: RenderContractMapCommand / Error の型厳格化

> CN-12 (DD decision) の波及として、既存の RenderContractMapCommand / RenderContractMapError の identity 系フィールドを raw String から TrackId / LayerId に変更する。
> 変更は usecase 層 (struct 定義) + CLI (呼び出し側) + 既存テストに及ぶ。独立した task として影響範囲を明示する。
> InvalidTrackId variant を削除し、Command 構築時の validation を CLI 境界に移動する。

- [x] **T003**: sibling modify (CN-12 波及): RenderContractMapCommand / RenderContractMapError の identity 系フィールドを raw String から TrackId / LayerId に変更し、呼び出し側 CLI および既存テストを更新する (CN-12, AC-18) (`246ef29b9107c9ca04bd50602905a273c6de66f9`)

### S4 — Infrastructure layer: 3 adapter 新設

> T004: BaselineGraphRendererAdapter — .harness/config/baseline-graph-style.toml 読み込み (fail-closed) + BaselineGraphRenderer port impl。render_overview / render_clusters の骨格を実装する。
> T013: BaselineGraphLoaderAdapter — architecture-rules.json で tddd.enabled layers を列挙、各 layer の rustdoc JSON をロード。symlink 拒否 (trusted_root 外)。Symmetric to FsCatalogueLoader。
> T014: BaselineGraphWriterAdapter — atomic write で depth 1 / depth 2 ファイルを書き出す。symlink 拒否。Symmetric to FsContractMapWriter。
> 3 adapter は責務が明確に独立しているため独立 task に分割する (各 200〜400 行の目安)。

- [x] **T004**: infrastructure layer: BaselineGraphRendererAdapter を libs/infrastructure に新設し、BaselineGraphRenderer port を impl する。.harness/config/baseline-graph-style.toml を読み込む (fail-closed: ファイル不在は StyleConfigNotFound エラー)。style config section 構造: [node.*] + [pattern.*] + [class.*] + [edge.*] + [filter] ([role.*] なし) (IN-02, IN-04, AC-02, AC-15, CN-02, CN-06) (`40fc5c1227b3a1343b8fed3e9e335b096b2a8e55`)
- [x] **T013**: infrastructure layer: BaselineGraphLoaderAdapter を libs/infrastructure に新設し、BaselineGraphLoader port を impl する。architecture-rules.json を rules_path から読み込んで tddd.enabled layers を列挙し、各 layer の rustdoc JSON baseline を track_root 下からロードして Vec<BaselineDocument> を返す。symlink は trusted_root 外で拒否 (SymlinkRejected)。ファイル不在は NotFound エラー (fail-closed)。Symmetric to FsCatalogueLoader (IN-02, IN-19, AC-02, CN-03) (`31026b2f108a24dfd3e9981ae06f2a38f5d35d39`)
- [x] **T014**: infrastructure layer: BaselineGraphWriterAdapter を libs/infrastructure に新設し、BaselineGraphWriter port を impl する。write_overview は track_root/<track_id>/<layer>-graph-d1/index.md に atomic write、write_cluster は track_root/<track_id>/<layer>-graph-d2/<cluster_key>.md に atomic write する。symlink は trusted_root 外で拒否 (SymlinkRejected)。Symmetric to FsContractMapWriter (IN-02, IN-19, AC-02, CN-03) (`2a8cb94c0b358371c47ddd0f92fbae50ad275444`)

### S5 — Adapter: node 抽出 + node_id 生成

> T005: rustdoc ItemEnum から 5 種 (Struct / Enum / TypeAlias / Trait / Function) を抽出するロジックと visibility filter (CC-1: Public only、Trait method / Enum variant は Default 例外) を実装する。
> T006: node_id 生成スキーム (D decision: T/R/F prefix + length-prefix + sanitized_module_path) を実装する。同一 crate 内の同名 Type / Trait 衝突を防ぐ。
> これら 2 タスクは密結合のため同じセクションに配置するが、各実装範囲が 500 行以内に収まるよう分割する。

- [x] **T005**: adapter 内: node 抽出ロジック (B-r1: 5 種固定 Struct / Enum / TypeAlias / Trait / Function) + visibility filter (CC-1: Public only / Default 例外) + Function 列挙範囲 (I decision) を実装する (IN-03, IN-17, IN-18, AC-03, AC-12) (`6ab06b3f218d4c479813383835e8c4ecef30ea5d`)
- [x] **T006**: adapter 内: node_id 生成スキーム (D decision: T/R/F prefix + length-prefix + sanitized_module_path) を実装し、同一 crate 内の同名 Type / Trait の衝突を防ぐ (IN-05, AC-11) (`4f6f00955804259461863f812e11275dc69cd9e1`)

### S6 — Adapter: entry subgraph + edge (F / H / H' / K / N decisions)

> Struct / Enum / Trait / TypeAlias の entry subgraph 化 (F-r1)。
> Enum variant node 化 + payload edge (H decision: Tuple / Struct / Plain の 3 形態)。
> Trait method 内包 (H' decision: Trait.items から Function 抽出)。
> struct fields edge (K decision: PlainStruct の --o|field_name|、TupleStruct の positional index、Unit / stripped は skip)。
> TypeAlias の無向 alias edge (N decision: ---|alias_of|)。

- [x] **T007**: adapter 内: entry subgraph 化 (F-r1: Struct / Enum / Trait / TypeAlias) + Enum variant node 化 + payload edge (H decision) + Trait method 内包 (H' decision) + struct fields edge (K decision) + TypeAlias alias edge (N decision) を実装する (IN-06, IN-07, IN-08, IN-10, IN-11, AC-04, AC-06, AC-07, AC-09, AC-10) (`8b29d5ee7178b9294f365ae6a9c0e2181a0b30e2`)

### S7 — Adapter: Impl 処理 + cross-baseline trait index (BB / O / J decisions)

> cross-baseline global trait index を render 関数内で 1 回構築 (O-r1)。index key は (CrateName, module_path, TraitName)。
> Impl Item 処理 (BB-4-fix1): inherent merge / trait impl edge / blanket 本体 a 案 / provided_method + negative + synthetic + blanket_impl:Some skip。
> trait impl edge `-.impl.->` (J decision)。workspace 外型は silent skip。
> cross-baseline での rustdoc Id 比較禁止 (CN-05)。lookup 失敗は silent skip (CN-10)。

- [x] **T008**: adapter 内: cross-baseline global trait index 構築 (O-r1) + Impl Item 処理 (BB-4-fix1: inherent merge / trait impl edge / blanket 本体 / skip rules) + trait impl edge `-.impl.->` (J decision) を実装する (IN-09, IN-12, IN-13, AC-05, AC-08, AC-17, CN-04, CN-05, CN-10, CN-11) (`ebe68a25b054281ea3d6b1a124f8ad20c08bf2e7`)

### S8 — Adapter: depth 1 overview renderer (U-r3)

> render_overview を完成させる。cluster (crate_name × top-level module) を 1 node に縮約 (subgraph 化しない)。
> cross-cluster edge group のみ表示。cluster node は :::cluster classDef。crate root entry は <crate_name> root cluster に集約。
> mermaid 出力構造: (1) classDef 定義群 (2) layer subgraph > cluster node 群 (alphabetical) (3) cross-cluster edge group (4) class attach 群。
> subgraph への inline :::className は parse error のため class <id> <className> を別行で記述。

- [x] **T009**: adapter 内: depth 1 overview renderer — cluster (crate_name × top-level module) 縮約 + cross-cluster edge 集約 + alphabetical ordering (U-r3) + mermaid 出力構造 (classDef / layer subgraph / cluster node / edge group / class attach) を実装し render_overview を完成させる (IN-14, IN-16, AC-13, CN-07, CN-08) (`d4336a52f7a48b25ae4e3476a6f5052e6eb1f7f8`)

### S9 — Adapter: depth 2 cluster detail renderer (U-r3)

> render_clusters を完成させる。各 cluster の識別キー (cluster_key) と描画コンテンツのペアを Vec<ClusterRender> で返す。ファイル名規則: cluster_key をそのまま stem として使用: top-level module cluster は <crate_name>_<module_seg1>.md、crate root cluster は <crate_name>_root.md (cluster_key がそのままファイル名 stem になる)。
> mermaid 出力構造: (1) classDef 定義群 (2) layer subgraph > top-module subgraph > entry subgraph 群 (alphabetical) > method/variant node (Vec 順) + FunctionEntry callable node 群 (alphabetical) (3) edge 定義群 (cluster 内 edge のみ) (4) class attach 群。
> sub-module path を entry subgraph label に含める (例: team::manager::TeamManager)。cross-cluster edge は深さ 1 に集約済みのため描画しない。

- [x] **T010**: adapter 内: depth 2 cluster detail renderer — top-module subgraph + entry subgraph + method/variant node + FunctionEntry callable node + cluster 内 edge + mermaid 出力構造 + ファイル名規則 (cluster_key をそのまま stem として使用: <crate_name>_<module_seg1>.md または <crate_name>_root.md) を実装し render_clusters を完成させる (IN-15, IN-16, AC-14, CN-07, CN-08) (`1c643301f15b5e14c2734cf33f00e641c3513750`)

### S10 — CLI 統合

> sotp track baseline-graph サブコマンドを追加する。
> BaselineGraphLoaderAdapter (T013) / BaselineGraphRendererAdapter (T004) / BaselineGraphWriterAdapter (T014) を composition root で compose し、RenderBaselineGraphInteractor<L,R,W> に inject する。
> depth 1 <layer>-graph-d1/index.md と depth 2 <layer>-graph-d2/<cluster>.md を書き出す。

- [x] **T011**: CLI 統合: sotp track baseline-graph サブコマンドを追加し RenderBaselineGraphInteractor を呼び出す。BaselineGraphLoaderAdapter (T013) / BaselineGraphRendererAdapter (T004) / BaselineGraphWriterAdapter (T014) を composition root で compose し RenderBaselineGraphInteractor<L,R,W> に inject する。depth 1 index.md + depth 2 cluster files を書き出す (IN-02, IN-19, AC-02, AC-18) (`3936c45df2e12c4c0037a15e645f16c864b42936`)

### S11 — layer-agnostic unit tests + CI gate

> 2 層 / 3 層 / 独自層名構成の rustdoc JSON fixture で renderer の正常動作を確認する。
> subgraph label に層名がハードコードされていないことを unit test で検証する (AC-16)。
> cargo make ci (fmt-check + clippy + nextest + deny + check-layers + verify-*) が全 pass することを確認する (AC-18)。

- [x] **T012**: layer-agnostic unit tests: 2 層構成 / 3 層構成 / 独自層名構成の rustdoc JSON fixture で renderer が正常動作することを確認し、subgraph label に層名がハードコードされていないことを検証する。cargo make ci (fmt-check + clippy + nextest + deny + check-layers + verify-*) が全 pass することを確認する (AC-16, AC-18, CN-01, CN-03, CN-09) (`8d253bbd3adb398ff50ca2b9418edc6132124bd3`)

### S12 — Adapter: method edge + ネスト型参照抽出 (AC-19 / AC-20)

> T015: method edge 新規実装 — inherent method (BB) および Trait method (H') の method node から method_param / method_returns edge を引く。型解決は Type::ResolvedPath.args 再帰走査、own crate 型のみを対象とする (AC-19 + AC-20 の method 経路)。
> T016: 既存 edge の型解決修正 — struct field (K) / enum variant payload (H) / TypeAlias target (N) の型解決を ResolvedPath.args 再帰に統一し、anonymous node (prim_* / generic_* / anon_*) 生成コードを削除する (AC-20 の既存 edge 修正)。
> T017: depth-1 overview の collect_entry_edge_pairs に method-signature edge walk (Pass 3) を追加する。inherent method および Trait method の FunctionSignature.inputs / output を collect_resolved_node_ids_from_type で own crate 型解決し cross-cluster pair を生成する。depth-2 側 (emit_method_signature_edges) のロジックを参照して再利用する (AC-19 depth-1 経路)。
> syn は使わない (rustdoc Type が構造化済みのため ADR E の制約通り)。catalogue 宣言型 (BaselineGraphRenderer) は不変のため type-design 不要。各 task が 500 行以内に収まるよう分割する。

- [x] **T015**: adapter 内: method edge 実装 — inherent method (BB: entry subgraph 内包) および Trait method (H': Trait subgraph 内包) の各 method node から、引数の型へ method_param edge、返り値の型へ method_returns edge を引く。FunctionSignature.inputs / output を走査し、own crate 型 (krate.paths lookup で crate_id == 0) のみ entry subgraph へ edge を引く。型解決は Type::ResolvedPath.args (GenericArgs) の再帰走査で行う (syn 不使用)。method node が引数・返り値を持たない場合は edge なし。edge スタイルは設定ファイル [edge.method_param] / [edge.method_returns] を参照する。修正対象: render/entry_subgraph.rs / render/impl_processor.rs (AC-19, AC-20 の method 経路) (`6fde3a59a49786e5779cb1b69bd2edc268d112fa`)
- [x] **T016**: adapter 内: 既存 edge の型解決を ResolvedPath.args 再帰に変更 — struct field (K decision) / enum variant payload (H decision) / TypeAlias target (N decision) の型解決ロジックを Type::ResolvedPath.args (GenericArgs) 再帰走査に統一し、own crate 型 (krate.paths lookup で crate_id == 0) のみ edge を引くように修正する。primitive (Type::Primitive) / generic 型パラメータ (Type::Generic) / 外部型への edge は生成しない。anonymous node (prim_* / generic_* / anon_*) 生成コードおよびそれらへの edge 生成コードを削除する (syn 不使用)。修正対象: render/entry_subgraph.rs (AC-20 の既存 edge 修正) (`1adc93a29cbcdfbec9c1ebff2cdd254de740f14c`)
- [ ] **T017**: adapter 内: depth-1 overview の edge collector `collect_entry_edge_pairs` (render/mod.rs) に method-signature edge walk (Pass 3) を追加する — inherent method (Impl items: trait_:None / blanket:None / non-negative / non-synthetic) および Trait method (Trait items の Function variant) の FunctionSignature.inputs / output を既存の `collect_resolved_node_ids_from_type` で own crate 型解決し、(src_rep_id, dst_type_rep_id) の cross-cluster pair を生成して既存の cross-cluster フィルタに流す。depth-2 側 (impl_processor の emit_method_signature_edges) のロジックを参照して再利用する。catalogue 宣言型 (BaselineGraphRenderer) は不変。修正対象: render/mod.rs のみ (AC-19 depth-1 経路)
