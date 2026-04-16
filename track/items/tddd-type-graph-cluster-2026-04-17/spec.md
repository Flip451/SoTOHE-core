<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: approved
approved_at: "2026-04-16T17:19:43Z"
version: "1.0"
signals: { blue: 32, yellow: 5, red: 0 }
---

# TDDD Type Graph View Phase 2 — cluster + port/adapter 可視化 + TDDD debt 返済

## Goal

ADR 2026-04-16-2200 の Phase 2 を scope (K) aggregate + port/adapter view として実装する
Phase 1 の flat 50 ノード限界を cluster 分割で突破し、3 層全て (domain / usecase / infrastructure) で cluster directory を生成する
Trait impl 破線エッジで hexagonal port/adapter の関係を可視化し、検査観点での有用性を高める
TDDD-BUG-02 (check_type_signals ハードコード) と TDDD-Q01 (SECTIONS 網羅テスト) の既知 debt を同時返済する
DRIFT-01 / entry-point / auto-render は現 codebase (enum-first 設計 / typestate 0 件) では ROI 限定的と実測判断し、別 ADR または module 粒度ルール拡張後に再評価する

## Scope

### In Scope
- libs/infrastructure/src/tddd/type_graph_cluster.rs を新設し ClusterPlan / CrossEdge / classify_types を実装する。module_path prefix depth N 段までで cluster 分類、module_path=None は UNRESOLVED_CLUSTER に集約する [source: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §D4, §D7 Phase 2, libs/domain/src/schema.rs §TypeNode.module_path] [tasks: T003]
- libs/infrastructure/src/tddd/type_graph_render.rs に write_type_graph_dir / render_type_graph_clustered / render_type_graph_overview を追加する。出力は <layer>-graph/index.md + <cluster>.md (cluster 名は :: を _ に置換)。cluster_depth=0 は既存 write_type_graph_file の flat 出力を維持する [source: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §D5, §D7 Phase 2] [tasks: T004]
- TypeGraphRenderOptions::cluster_depth: usize (Default=2) フィールドを追加し CLI --cluster-depth N フラグを配線する。cluster_depth>0 なら write_type_graph_dir、=0 なら write_type_graph_file に dispatch する [source: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §D6, track/items/tddd-type-graph-spike-2026-04-16/verification.md §Phase 2 Handoff] [tasks: T004]
- Stale file cleanup を実装する: flat→cluster 切り替え時は <layer>-graph.md を削除、cluster→flat 切り替え時は <layer>-graph/ を remove_dir_all する。削除前に reject_symlinks_below で二重ガードを通し symlink 経由の意図しない削除を fail-closed で防ぐ [source: convention — knowledge/conventions/security.md §Symlink Rejection in Infrastructure Adapters, libs/infrastructure/src/tddd/type_graph_render.rs §write_type_graph_file (Phase 1 symlink guard pattern), inference — mode switch でのファイル残留は視覚的混乱を生むため、削除を自動化する必要がある] [tasks: T004]
- EdgeSet::Fields (TypeNode::members の Field/Variant から A --- B 実線矢印なし) と EdgeSet::Impls (TypeNode::trait_impls から A -.-> Trait 破線) を実装する。Trait ノードは classDef traitNode (stadium shape)。EdgeSet::All は 3 種の union [source: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §D2(b), §D2(c), knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md, libs/domain/src/schema.rs §TypeNode.trait_impls] [tasks: T005]
- CLI --edges methods|fields|impls|all フラグを追加し opts.edge_set に反映する。flat / cluster 両方のレンダリングパスで 3 種の edge 描画を対応する [source: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §D2, §D6] [tasks: T005]
- check_type_signals(doc, strict) の signature を (doc, strict, catalogue_file: &str) に変更する。libs/domain/src/tddd/consistency.rs の定義 + 8 test_check_type_signals_* tests (empty_entries / none_signals / coverage_gap / red_is_error / yellow_warning_interim / yellow_error_strict / all_blue_passes / undeclared_yellow_not_blocked) を更新、libs/usecase/src/merge_gate.rs:220 は format!("{layer_id}-types.json") を渡し、libs/infrastructure/src/verify/spec_states.rs:231 は binding.catalogue_file() を渡す。エラー文言内の catalogue_file 表示を検証する新テスト 2 件を追加する [source: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §D9 TDDD-BUG-02, knowledge/strategy/TODO.md §TDDD-BUG-02, libs/domain/src/tddd/consistency.rs:372-441] [tasks: T001]
- libs/infrastructure/src/type_catalogue_render.rs に test_sections_covers_all_kind_tags を追加する。TypeDefinitionKind::kind_tag() の全 13 variants を HashSet 化し SECTIONS.iter().map(|s| s.kind_tag) と assert_eq! で比較する [source: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §D9 TDDD-Q01, knowledge/strategy/TODO.md §TDDD-Q01, libs/infrastructure/src/type_catalogue_render.rs §SECTIONS] [tasks: T002]
- 3 層 (domain / usecase / infrastructure) 全対応の統合テストを追加する。resolve_layers による既存 iteration pattern を踏襲し、各 layer で cluster directory 出力が生成されることを確認する [source: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §D7 Phase 2 Acceptance, apps/cli/src/commands/track/tddd/signals.rs §resolve_layers] [tasks: T004]
- TDD red→green: 各 task で unit tests を先に書き red 確認後に実装で green 化する。各 commit diff は 500 LOC 以下に保つ [source: convention — .claude/rules/05-testing.md §Core Principles, convention — .claude/rules/10-guardrails.md §Small task commits] [tasks: T001, T002, T003, T004, T005]
- 3 層で sotp track type-graph --cluster-depth {1,2} --edges all を実行し verification.md に層別 node/edge/cluster 数・可読性比較・trait impl 破線の hexagonal 可視化有効性を記録する。ADR 2026-04-16-2200 に DRIFT-01 / entry-point / auto-render を延期する note を追加する [source: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §D7 Phase 2 Acceptance, §D8, inference — enum-first codebase 実測 (typestate 0 件) に基づくスコープ縮小判断] [tasks: T006]

### Out of Scope
- Entry point 検出 (classDef entry marking) は現 codebase (enum-first 設計) では navigation 価値が実証困難なため見送る。ADR Open Questions §2 / §5 の wrapper 型処理も含めて別 track で再評価する [source: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §D3, §Open Questions, inference — domain に typestate 0 件・consuming-self 1 件 (collection unwrap 除けば 0) の実測結果]
- DRIFT-01 基盤 (--check-drift flag / DriftReport / cross-cluster may_depend_on 違反検出) は現 architecture-rules.json の crate 粒度では verify layers と重複するため見送る。module 粒度ルール拡張の新 ADR 後に再評価する [source: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §D8, libs/infrastructure/src/verify/layers.rs, inference — 現 may_depend_on は crate 粒度のみ、DRIFT-01 は同粒度で検出すると既存 verify layers と redundant]
- Orphan type 検出 (classDef orphan marking) は DRIFT-01 と同じトラック扱いで見送る [source: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §D8]
- sotp track type-signals 成功時の auto-render 統合 (evaluate_and_write_signals から write_type_graph_dir を呼ぶ) は見送る。現 codebase で graph view の living document 化が ROI を正当化する可視化価値を持つか未実証のため [source: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §D6, inference — Phase 2 (K) scope の可読性実測 (T006) で ROI を判断してから auto-render を別 track で追加する]
- Phase 3 (path query CLI: sotp track type-path --from A --to B) は ADR Phase 3 スコープとして Phase 2 完了後に別 track で扱う [source: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §D6, §D7 Phase 3]
- State machine view (stateDiagram-v2 で enum state machine を描画する機能) は TypeGraph とは別のデータソースを必要とするため本 track では扱わない。別 ADR 案件として将来的に検討する [source: inference — 現 codebase は enum-first 設計で state 遷移は enum メソッド内部、TypeGraph (型と型の関係) では表現できない]
- architecture-rules.json の module 粒度拡張 (例: domain::guard may_depend_on domain::shared) は別 ADR 案件として扱う [source: inference — DRIFT-01 の真の価値を引き出す前提条件だが、スキーマ変更 + 既存 verify layers/deny.toml/check_layers との整合設計が必要]

## Constraints
- TDDD 不変条件 (ADR 0002 §D6) を侵さない: layer-agnostic / rustdoc JSON 唯一基盤 / last-segment short name / libs/domain には serde を戻さない [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md §D6, knowledge/adr/2026-04-14-1531-domain-serde-ripout.md §D1]
- 既存の TypeGraph / TypeNode / TraitNode / TypeCatalogueDocument の schema に変更を加えない。view 拡張と既存関数 signature 変更のみ [source: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §D1]
- hexagonal 原則を維持する: cluster / render / drift 関連の全コードは libs/infrastructure に配置し、libs/domain は layer 無名で保つ。CLI (apps/cli) は thin composition layer のみ [source: convention — knowledge/conventions/hexagonal-architecture.md, knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md §D6]
- Symlink safety: 全ての write / remove パスで reject_symlinks_below + atomic_write_file を通す。directory 削除 (remove_dir_all) の前には二重 symlink guard を通す [source: convention — knowledge/conventions/security.md §Symlink Rejection in Infrastructure Adapters, libs/infrastructure/src/tddd/type_graph_render.rs §write_type_graph_file]
- 小さい task commits: 各 task の diff は 500 LOC 未満を目標とする [source: convention — .claude/rules/10-guardrails.md §Small task commits]
- Backward compat なし clean break (TDDD-BUG-02): feedback_no_backward_compat に従い check_type_signals の signature 変更は破壊的に行う [source: feedback — user memory feedback_no_backward_compat.md]
- TDD red→green の順序を守る。各 task で unit tests を先に書き red 確認後に実装する [source: convention — .claude/rules/05-testing.md §Core Principles]
- enum-first を維持する (typestate は使わない)。ClusterPlan / CrossEdge は struct、EdgeSet / edge_kind は enum または const &'static str の有限集合とする [source: convention — .claude/rules/04-coding-principles.md §Enum-first]

## Acceptance Criteria
- [ ] libs/infrastructure/src/tddd/type_graph_cluster.rs が存在し classify_types(graph, depth, edges) が ClusterPlan を返す。6 unit tests (depth 0/1/2 分類、unresolved フォールバック、cross_edges 検出、単一 cluster 時の cross_edges 空検証) が pass する [source: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §D4, §D7 Phase 2] [tasks: T003]
- [ ] sotp track type-graph <id> --cluster-depth 2 --edges all が 3 層 (domain / usecase / infrastructure) それぞれで <layer>-graph/index.md + <cluster>.md 群を書き出す [source: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §D5, §D7 Phase 2 Acceptance] [tasks: T004]
- [ ] FsReviewStore が ReviewReader / ReviewWriter に破線エッジ (A -.-> Trait) を持ち stadium shape で描画される。生成された mermaid に classDef traitNode が存在する [source: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §D2(c), §D7 Phase 2 Acceptance, knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md] [tasks: T005]
- [ ] check_type_signals のエラーメッセージが呼び出し元から渡された catalogue_file 引数を表示する。usecase-types.json に対するエラーで domain-types.json がハードコード表示されない [source: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §D9 TDDD-BUG-02, knowledge/strategy/TODO.md §TDDD-BUG-02] [tasks: T001]
- [ ] test_sections_covers_all_kind_tags が TypeDefinitionKind 全 13 variants と SECTIONS の kind_tag 集合の一致を assert する。variant 追加漏れが cargo nextest で検出される [source: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §D9 TDDD-Q01, knowledge/strategy/TODO.md §TDDD-Q01] [tasks: T002]
- [ ] 既存 sotp track type-signals の動作が壊れていない。check_type_signals caller 2 箇所 (merge_gate.rs / spec_states.rs) の既存テストが全て pass する [source: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §D7 Phase 2 Acceptance, libs/usecase/src/merge_gate.rs, libs/infrastructure/src/verify/spec_states.rs] [tasks: T001]
- [ ] Stale file cleanup: flat↔cluster 切り替え時に古い出力が残らない。symlink 経由の削除試行は fail-closed で拒否される (unit test で injection verified) [source: convention — knowledge/conventions/security.md §Symlink Rejection in Infrastructure Adapters] [tasks: T004]
- [ ] verification.md に 3 層 (domain / usecase / infrastructure) の node/edge/cluster 数、cluster_depth 1 vs 2 の可読性比較、trait impl 破線の hexagonal port/adapter 可視化有効性が記録されている [source: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §D7 Phase 2 Acceptance] [tasks: T006]
- [ ] verification.md に deduplicate_typestate_edges default の実測判断が記録されている (T004 実装中に生じた Open Question の解決を docs として残す) [source: inference — T004 S004 Open Question として metadata.json に記載; verification.md チェックリストに対応項目あり] [tasks: T006]
- [ ] ADR 2026-04-16-2200 に scope (K) 縮小と DRIFT-01 / entry-point / auto-render 延期を記録する note が追加されている [source: inference — ADR との scope 齟齬を記録し将来の再評価に備える] [tasks: T006]
- [ ] cargo make ci (fmt-check + clippy + nextest + test-doc + deny + python-lint + scripts-selftest + check-layers + verify-canonical-modules + verify-arch-docs + verify-doc-links + verify-plan-progress + verify-track-metadata + verify-track-registry + verify-tech-stack + verify-orchestra + verify-latest-track + verify-module-size + verify-domain-strings + verify-domain-purity + verify-usecase-purity + verify-view-freshness + verify-spec-coverage + verify-spec-states-current) が全通過する [source: convention — .claude/rules/07-dev-environment.md §Pre-commit Checklist, inference — full ci-local task dependency list は Makefile.toml [tasks.ci-local] が authoritative] [tasks: T001, T002, T003, T004, T005, T006]

## Related Conventions (Required Reading)
- knowledge/conventions/source-attribution.md
- knowledge/conventions/hexagonal-architecture.md
- knowledge/conventions/nightly-dev-tool.md

## Signal Summary

### Stage 1: Spec Signals
🔵 32  🟡 5  🔴 0

