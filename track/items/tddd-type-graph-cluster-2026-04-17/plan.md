<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# TDDD Type Graph View Phase 2 — cluster + port/adapter 可視化 + TDDD debt 返済

ADR 2026-04-16-2200 の Phase 2 を scope (K) で最小実装する: cluster + trait impl edges + debt 返済
Phase 1 実測 (110 types flat 不読) + 本 track 計画中の追加実測 (domain に typestate パターン 0 件) を受け、state machine view の期待は撤回し、aggregate composition + port/adapter view に再定義する
TDDD-BUG-02 (check_type_signals ハードコード) と TDDD-Q01 (SECTIONS 網羅テスト) を同時修正し、将来の ripple を予防する
Deferred scope: entry point detection / DRIFT-01 (現 may_depend_on 粒度では verify layers 重複) / auto-render 統合。別 ADR または module 粒度拡張後に再評価

## S001 — TDDD-BUG-02: check_type_signals に catalogue_file 引数を追加

check_type_signals のエラーメッセージに含まれるハードコード文字列 "domain-types.json" を引数化する (TDDD-BUG-02)
signature 変更: check_type_signals(doc, strict, catalogue_file: &str)
3 crate への ripple: libs/domain/src/tddd/consistency.rs (定義 + 8 test_check_type_signals_* tests) + libs/usecase/src/merge_gate.rs:220 + libs/infrastructure/src/verify/spec_states.rs:231
merge_gate.rs は format!("{layer_id}-types.json") を渡す; spec_states.rs は binding.catalogue_file() を渡す
エラー文言の catalogue_file 埋め込み検証テストを 2 件追加する

- [x] TDDD-BUG-02: check_type_signals(doc, strict) に catalogue_file: &str 引数を追加。libs/domain/src/tddd/consistency.rs の定義と以下 8 tests を更新: test_check_type_signals_empty_entries_returns_error / test_check_type_signals_none_signals_returns_error / test_check_type_signals_coverage_gap_returns_error / test_check_type_signals_red_is_error_regardless_of_mode / test_check_type_signals_yellow_is_warning_in_interim_mode / test_check_type_signals_yellow_is_error_in_strict_mode / test_check_type_signals_all_blue_passes_in_both_modes / test_check_type_signals_undeclared_yellow_is_not_blocked。caller 2 箇所 (libs/usecase/src/merge_gate.rs:220 は format!("{layer_id}-types.json") を渡す、libs/infrastructure/src/verify/spec_states.rs:231 は binding.catalogue_file() を渡す) を更新。エラー文言内の catalogue_file 表示を検証する test を 2 件追加。backward compat なし clean break (feedback_no_backward_compat 準拠)。 f49c9f8a35643cf198ccc7ee63e88e2b56801462

## S002 — TDDD-Q01: SECTIONS exhaustive coverage test

libs/infrastructure/src/type_catalogue_render.rs の SECTIONS が TypeDefinitionKind::kind_tag() の全 variant を網羅していることを保証する exhaustive test を追加する (TDDD-Q01)
test_sections_covers_all_kind_tags: HashSet<&str> of 13 variants vs SECTIONS.iter().map(|s| s.kind_tag) を assert_eq!
将来の variant 追加時に SECTIONS への追記漏れをコンパイル時ではなく nextest 時に検出可能にする

- [x] TDDD-Q01: libs/infrastructure/src/type_catalogue_render.rs に test_sections_covers_all_kind_tags を追加する。TypeDefinitionKind::kind_tag() の全 13 variants の HashSet<&str> と SECTIONS.iter().map(|s| s.kind_tag).collect::<HashSet<_>>() を assert_eq! で比較。将来 variant 追加時の SECTIONS 更新漏れを nextest で検出する exhaustive test。 76e8ad11b7979faf2a1404310990ffe79f550095

## S003 — ClusterPlan module (type_graph_cluster.rs)

新規モジュール libs/infrastructure/src/tddd/type_graph_cluster.rs を作成する
ClusterKey = String 型エイリアス、UNRESOLVED_CLUSTER = "unresolved" 定数
ClusterPlan { depth, assignments: HashMap<ClusterKey, Vec<String>>, cross_edges: Vec<CrossEdge> } struct
CrossEdge { source_type, source_cluster, target_type, target_cluster, label, edge_kind } struct
classify_types(graph, depth, edges) -> ClusterPlan 純粋関数: module_path prefix depth 段までで cluster 分類、module_path=None は UNRESOLVED_CLUSTER へ
6 unit tests (I/O なし): depth 0/1/2 の分類、unresolved フォールバック、cross_edges 検出、単一 cluster 時の cross_edges 空検証

- [x] ClusterPlan module: libs/infrastructure/src/tddd/type_graph_cluster.rs を新設。ClusterPlan struct (depth, assignments, cross_edges) + CrossEdge struct + UNRESOLVED_CLUSTER 定数 + classify_types pure function。module_path prefix depth 段で cluster 分類、None は UNRESOLVED_CLUSTER へ。I/O なし、6 unit tests (depth 0/1/2 分類、unresolved フォールバック、cross_edges 検出)。mod.rs に登録。 55d180b520247072348eb925d2f8ad1ad70ee0da

## S004 — Cluster directory layout + stale cleanup

libs/infrastructure/src/tddd/type_graph_render.rs に render_type_graph_clustered と render_type_graph_overview を追加する
write_type_graph_dir(graph, layer_id, track_dir, trusted_root, opts) -> Result<Vec<String>, io::Error> を新設: <layer>-graph/index.md + <cluster>.md 群を出力
cluster ファイル名は module_path の :: を _ に置換 (例: domain::review_v2 → domain_review_v2.md)
TypeGraphRenderOptions::cluster_depth: usize (default=2) フィールド追加、Default::default() を更新
Stale file cleanup: flat→cluster 切り替え時は <layer>-graph.md を削除、cluster→flat 切り替え時は <layer>-graph/ を remove_dir_all。削除前に reject_symlinks_below で二重ガード
CLI graph.rs に --cluster-depth N arg を追加し、cluster_depth>0 なら write_type_graph_dir、=0 なら既存 write_type_graph_file を呼び分け
3 層 (domain/usecase/infrastructure) 全対応の統合テストを追加 (resolve_layers による既存 iteration)
6 tests: dir 作成検証 / 片方向ガード / stale cleanup flat→cluster / stale cleanup cluster→flat / overview cluster nodes / clustered intra-cluster edges only

- [x] Cluster directory layout: libs/infrastructure/src/tddd/type_graph_render.rs に render_type_graph_clustered / render_type_graph_overview / write_type_graph_dir を追加。出力は <layer>-graph/index.md + <cluster>.md (cluster 名は :: を _ に置換)。TypeGraphRenderOptions::cluster_depth: usize (default=2) 追加。Stale file cleanup: flat→cluster で .md を削除、cluster→flat で remove_dir_all。削除前に reject_symlinks_below で二重ガード。CLI --cluster-depth N flag を追加し、>0 なら write_type_graph_dir、=0 なら既存 write_type_graph_file に dispatch。3 層 (domain/usecase/infrastructure) 全対応。6 tests 追加。

## S005 — Fields + Impls edges (hexagonal port/adapter 可視化)

libs/infrastructure/src/tddd/type_graph_render.rs の EdgeSet::Fields と EdgeSet::Impls を実装する (Phase 1 では stub)
Fields: TypeNode::members() の Field / Variant から A --- B (実線矢印なし) edge 生成
Impls: TypeNode::trait_impls() (tddd-05 で利用可能) から A -.-> Trait (破線矢印あり) edge 生成。trait ノードは classDef traitNode (stadium shape ([TraitName]))
All: Methods + Fields + Impls の union
CLI --edges methods|fields|impls|all フラグを追加し opts.edge_set に反映
mermaid classDef 追加: traitNode fill:#e8f5e9,stroke:#388e3c
既存 flat と cluster 両方のレンダリングパスで fields/impls を対応 (S004 の context で S005)
4 tests: field edges 描画 / trait impl 破線描画 / All edge set の union / trait node stadium shape

- [ ] Fields + Impls edges: EdgeSet::Fields (TypeNode::members の Field/Variant から A --- B) と EdgeSet::Impls (TypeNode::trait_impls から A -.-> Trait 破線) を実装。Trait ノードは classDef traitNode (stadium shape)。EdgeSet::All は 3 種の union。CLI --edges methods|fields|impls|all flag 追加。flat + cluster 両パスで edge を描画。4 tests (field edges / impl dashed / All union / trait stadium shape)。tddd-05 で利用可能な TypeNode::trait_impls を消費する。

## S006 — Multi-layer readability verification + ADR 実測補強

3 層 (domain / usecase / infrastructure) で実際に sotp track type-graph --cluster-depth {1,2} --edges all を実行し cluster 出力を生成する
可読性の実測結果を verification.md に記録する: (a) 層ごとのノード数・エッジ数・cluster 数、(b) cluster_depth 1/2 の可読性比較、(c) trait impl 破線が hexagonal port/adapter 可視化として機能するか
S004 Open Question (deduplicate_typestate_edges default) の実測判断を ADR Open Questions §4 への実測解答として記録する
ADR 2026-04-16-2200 §Phase 2 Scope Update §S4 / §S5 に実測データを補強追記する (具体的な node/edge/cluster 数、dedup 結論、scope (K) 延期の正当性の裏付け)。§S1-§S5 の本体構造は planning 時に追加済み
このタスクは production code 変更なし、verification.md + ADR 補強追記の docs のみ

- [ ] Multi-layer readability verification + ADR 実測補強: 3 層 (domain/usecase/infrastructure) で sotp track type-graph --cluster-depth {1,2} --edges all を実行して生成物を目視確認。verification.md に (a) 層別 node/edge/cluster 数、(b) depth 1 vs 2 可読性比較、(c) trait impl 破線の hexagonal port/adapter 可視化の有効性、(d) deduplicate_typestate_edges default 判断 (ADR Open Questions §4 への実測解答) を記録。ADR 2026-04-16-2200 §Phase 2 Scope Update §S4 / §S5 に実測データ (具体的な node/edge/cluster 数、dedup 結論) を補強追記し、scope (K) 延期の正当性を実測で裏付ける。§Phase 2 Scope Update §S1-§S5 の構造は planning 時に既に追加済みなので、T006 の役割は実測 data による補強のみ。production code 変更なし、docs のみ。
