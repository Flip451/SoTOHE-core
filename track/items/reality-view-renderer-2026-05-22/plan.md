<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# reality-view renderer の rustdoc_types::Crate 入力対応 (v3 schema 移行)

## Tasks (1/14 resolved)

### S1 — Domain layer: 3 port + 3 error type + BaselineDocument

> BaselineDocument wrapper struct + 3 secondary port (BaselineGraphLoader / BaselineGraphRenderer / BaselineGraphWriter) + 各 error type (BaselineGraphLoaderError / BaselineGraphRendererError / BaselineGraphWriterError) を domain crate に新設する。
> Contract Map の CatalogueDocument / CatalogueLoader / ContractMapRenderer / ContractMapWriter と symmetric な設計。domain → infrastructure 依存なし (hexagonal 分離)。
> syn 依存は追加しない (rustdoc が Type を構造化済み)。

- [x] **T001**: domain layer: BaselineDocument wrapper struct + 3 secondary port (BaselineGraphLoader / BaselineGraphRenderer / BaselineGraphWriter) + 3 error type (BaselineGraphLoaderError / BaselineGraphRendererError / BaselineGraphWriterError) を libs/domain に新設する。各 port は Send + Sync bound を持つ。error type は Display / Error / Debug を impl する (IN-01, IN-02, IN-19, AC-01, AC-02)

### S2 — Usecase layer: interactor + Command / Error (3-port compose)

> RenderBaselineGraph primary port + RenderBaselineGraphInteractor<L,R,W> + Command / Output / Error を usecase crate に新設する。
> interactor は L: BaselineGraphLoader / R: BaselineGraphRenderer / W: BaselineGraphWriter の 3 generic で compose される (IN-19)。
> load → render → write の 3 段パイプラインを hexagonal purity (CN-03) に従って usecase 内に実装する。
> Command の identity 系フィールド (track_id / layer_filter) は validated domain value object (TrackId / LayerId) で型付けする (CN-12)。

- [ ] **T002**: usecase layer: RenderBaselineGraph primary port + RenderBaselineGraphInteractor<L,R,W> (L: BaselineGraphLoader, R: BaselineGraphRenderer, W: BaselineGraphWriter の 3 generic compose) + RenderBaselineGraphCommand / RenderBaselineGraphOutput / RenderBaselineGraphError を libs/usecase に新設する。interactor は load → render → write の 3 段パイプラインを compose する (IN-02, IN-19, AC-02, CN-12)

### S3 — Sibling modify: RenderContractMapCommand / Error の型厳格化

> CN-12 (DD decision) の波及として、既存の RenderContractMapCommand / RenderContractMapError の identity 系フィールドを raw String から TrackId / LayerId に変更する。
> 変更は usecase 層 (struct 定義) + CLI (呼び出し側) + 既存テストに及ぶ。独立した task として影響範囲を明示する。
> InvalidTrackId variant を削除し、Command 構築時の validation を CLI 境界に移動する。

- [ ] **T003**: sibling modify (CN-12 波及): RenderContractMapCommand / RenderContractMapError の identity 系フィールドを raw String から TrackId / LayerId に変更し、呼び出し側 CLI および既存テストを更新する (CN-12, AC-18)

### S4 — Infrastructure layer: 3 adapter 新設

> T004: BaselineGraphRendererAdapter — .harness/config/baseline-graph-style.toml 読み込み (fail-closed) + BaselineGraphRenderer port impl。render_overview / render_cluster の骨格を実装する。
> T013: BaselineGraphLoaderAdapter — architecture-rules.json で tddd.enabled layers を列挙、各 layer の rustdoc JSON をロード。symlink 拒否 (trusted_root 外)。Symmetric to FsCatalogueLoader。
> T014: BaselineGraphWriterAdapter — atomic write で depth 1 / depth 2 ファイルを書き出す。symlink 拒否。Symmetric to FsContractMapWriter。
> 3 adapter は責務が明確に独立しているため独立 task に分割する (各 200〜400 行の目安)。

- [ ] **T004**: infrastructure layer: BaselineGraphRendererAdapter を libs/infrastructure に新設し、BaselineGraphRenderer port を impl する。.harness/config/baseline-graph-style.toml を読み込む (fail-closed: ファイル不在は StyleConfigNotFound エラー)。style config section 構造: [node.*] + [pattern.*] + [class.*] + [edge.*] + [filter] ([role.*] なし) (IN-02, IN-04, AC-02, AC-15, CN-02, CN-06)
- [ ] **T013**: infrastructure layer: BaselineGraphLoaderAdapter を libs/infrastructure に新設し、BaselineGraphLoader port を impl する。architecture-rules.json を rules_path から読み込んで tddd.enabled layers を列挙し、各 layer の rustdoc JSON baseline を track_root 下からロードして Vec<BaselineDocument> を返す。symlink は trusted_root 外で拒否 (SymlinkRejected)。ファイル不在は NotFound エラー (fail-closed)。Symmetric to FsCatalogueLoader (IN-02, IN-19, AC-02, CN-03)
- [ ] **T014**: infrastructure layer: BaselineGraphWriterAdapter を libs/infrastructure に新設し、BaselineGraphWriter port を impl する。write_overview は track_root/<track_id>/<layer>-graph-d1/index.md に atomic write、write_cluster は track_root/<track_id>/<layer>-graph-d2/<cluster_key>.md に atomic write する。symlink は trusted_root 外で拒否 (SymlinkRejected)。Symmetric to FsContractMapWriter (IN-02, IN-19, AC-02, CN-03)

### S5 — Adapter: node 抽出 + node_id 生成

> T005: rustdoc ItemEnum から 5 種 (Struct / Enum / TypeAlias / Trait / Function) を抽出するロジックと visibility filter (CC-1: Public only、Trait method / Enum variant は Default 例外) を実装する。
> T006: node_id 生成スキーム (D decision: T/R/F prefix + length-prefix + sanitized_module_path) を実装する。同一 crate 内の同名 Type / Trait 衝突を防ぐ。
> これら 2 タスクは密結合のため同じセクションに配置するが、各実装範囲が 500 行以内に収まるよう分割する。

- [ ] **T005**: adapter 内: node 抽出ロジック (B-r1: 5 種固定 Struct / Enum / TypeAlias / Trait / Function) + visibility filter (CC-1: Public only / Default 例外) + Function 列挙範囲 (I decision) を実装する (IN-03, IN-17, IN-18, AC-03, AC-12)
- [ ] **T006**: adapter 内: node_id 生成スキーム (D decision: T/R/F prefix + length-prefix + sanitized_module_path) を実装し、同一 crate 内の同名 Type / Trait の衝突を防ぐ (IN-05, AC-11)

### S6 — Adapter: entry subgraph + edge (F / H / H' / K / N decisions)

> Struct / Enum / Trait / TypeAlias の entry subgraph 化 (F-r1)。
> Enum variant node 化 + payload edge (H decision: Tuple / Struct / Plain の 3 形態)。
> Trait method 内包 (H' decision: Trait.items から Function 抽出)。
> struct fields edge (K decision: PlainStruct の --o|field_name|、TupleStruct の positional index、Unit / stripped は skip)。
> TypeAlias の無向 alias edge (N decision: ---|alias_of|)。

- [ ] **T007**: adapter 内: entry subgraph 化 (F-r1: Struct / Enum / Trait / TypeAlias) + Enum variant node 化 + payload edge (H decision) + Trait method 内包 (H' decision) + struct fields edge (K decision) + TypeAlias alias edge (N decision) を実装する (IN-06, IN-07, IN-08, IN-10, IN-11, AC-04, AC-06, AC-07, AC-09, AC-10)

### S7 — Adapter: Impl 処理 + cross-baseline trait index (BB / O / J decisions)

> cross-baseline global trait index を render 関数内で 1 回構築 (O-r1)。index key は (CrateName, module_path, TraitName)。
> Impl Item 処理 (BB-4-fix1): inherent merge / trait impl edge / blanket 本体 a 案 / provided_method + negative + synthetic + blanket_impl:Some skip。
> trait impl edge `-.impl.->` (J decision)。workspace 外型は silent skip。
> cross-baseline での rustdoc Id 比較禁止 (CN-05)。lookup 失敗は silent skip (CN-10)。

- [ ] **T008**: adapter 内: cross-baseline global trait index 構築 (O-r1) + Impl Item 処理 (BB-4-fix1: inherent merge / trait impl edge / blanket 本体 / skip rules) + trait impl edge `-.impl.->` (J decision) を実装する (IN-09, IN-12, IN-13, AC-05, AC-08, AC-17, CN-04, CN-05, CN-10, CN-11)

### S8 — Adapter: depth 1 overview renderer (U-r3)

> render_overview を完成させる。cluster (crate_name × top-level module) を 1 node に縮約 (subgraph 化しない)。
> cross-cluster edge group のみ表示。cluster node は :::cluster classDef。crate root entry は <crate_name> root cluster に集約。
> mermaid 出力構造: (1) classDef 定義群 (2) layer subgraph > cluster node 群 (alphabetical) (3) cross-cluster edge group (4) class attach 群。
> subgraph への inline :::className は parse error のため class <id> <className> を別行で記述。

- [ ] **T009**: adapter 内: depth 1 overview renderer — cluster (crate_name × top-level module) 縮約 + cross-cluster edge 集約 + alphabetical ordering (U-r3) + mermaid 出力構造 (classDef / layer subgraph / cluster node / edge group / class attach) を実装し render_overview を完成させる (IN-14, IN-16, AC-13, CN-07, CN-08)

### S9 — Adapter: depth 2 cluster detail renderer (U-r3)

> render_cluster を完成させる。ファイル名は <crate_name>_<module_seg1>.md 形式。
> mermaid 出力構造: (1) classDef 定義群 (2) layer subgraph > top-module subgraph > entry subgraph 群 (alphabetical) > method/variant node (Vec 順) + FunctionEntry callable node 群 (alphabetical) (3) edge 定義群 (cluster 内 edge のみ) (4) class attach 群。
> sub-module path を entry subgraph label に含める (例: team::manager::TeamManager)。cross-cluster edge は深さ 1 に集約済みのため描画しない。

- [ ] **T010**: adapter 内: depth 2 cluster detail renderer — top-module subgraph + entry subgraph + method/variant node + FunctionEntry callable node + cluster 内 edge + mermaid 出力構造 + ファイル名規則 (<crate_name>_<module_seg1>.md) を実装し render_cluster を完成させる (IN-15, IN-16, AC-14, CN-07, CN-08)

### S10 — CLI 統合

> sotp track baseline-graph サブコマンドを追加する。
> BaselineGraphLoaderAdapter (T013) / BaselineGraphRendererAdapter (T004) / BaselineGraphWriterAdapter (T014) を composition root で compose し、RenderBaselineGraphInteractor<L,R,W> に inject する。
> depth 1 <layer>-graph-d1/index.md と depth 2 <layer>-graph-d2/<cluster>.md を書き出す。

- [ ] **T011**: CLI 統合: sotp track baseline-graph サブコマンドを追加し RenderBaselineGraphInteractor を呼び出す。BaselineGraphLoaderAdapter (T013) / BaselineGraphRendererAdapter (T004) / BaselineGraphWriterAdapter (T014) を composition root で compose し RenderBaselineGraphInteractor<L,R,W> に inject する。depth 1 index.md + depth 2 cluster files を書き出す (IN-02, IN-19, AC-02, AC-18)

### S11 — layer-agnostic unit tests + CI gate

> 2 層 / 3 層 / 独自層名構成の rustdoc JSON fixture で renderer の正常動作を確認する。
> subgraph label に層名がハードコードされていないことを unit test で検証する (AC-16)。
> cargo make ci (fmt-check + clippy + nextest + deny + check-layers + verify-*) が全 pass することを確認する (AC-18)。

- [ ] **T012**: layer-agnostic unit tests: 2 層構成 / 3 層構成 / 独自層名構成の rustdoc JSON fixture で renderer が正常動作することを確認し、subgraph label に層名がハードコードされていないことを検証する。cargo make ci (fmt-check + clippy + nextest + deny + check-layers + verify-*) が全 pass することを確認する (AC-16, AC-18, CN-01, CN-03, CN-09)
