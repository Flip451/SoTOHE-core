# Verification: tddd-type-graph-cluster-2026-04-17

## Scope Verified

- [ ] In-scope items match ADR 2026-04-16-2200 Phase 2 (scope K — aggregate + port/adapter view)
- [ ] Out-of-scope items correctly deferred (entry-point / DRIFT-01 / auto-render / state machine view / Phase 3 path query)

## Task Verification

### T001: check_type_signals catalogue_file 引数追加 (TDDD-BUG-02)

- [ ] `libs/domain/src/tddd/consistency.rs` の signature が `(doc, strict, catalogue_file: &str)` に変更された
- [ ] 8 既存 tests + 2 新規 tests (catalogue_file error message 検証) が pass
- [ ] `libs/usecase/src/merge_gate.rs:220` が `format!("{layer_id}-types.json")` を渡している
- [ ] `libs/infrastructure/src/verify/spec_states.rs:231` が `binding.catalogue_file()` を渡している
- [ ] usecase-types.json / infrastructure-types.json に対するエラーで catalogue_file が正しく表示される

### T002: SECTIONS exhaustive coverage test (TDDD-Q01)

- [ ] `test_sections_covers_all_kind_tags` が `libs/infrastructure/src/type_catalogue_render.rs` に存在する
- [ ] `TypeDefinitionKind` 全 13 variants の kind_tag が SECTIONS に含まれる
- [ ] テストが variant 追加漏れを検出することを手動で確認 (一時的に SECTIONS 項目を削除して red 確認)

### T003: ClusterPlan module

- [ ] `libs/infrastructure/src/tddd/type_graph_cluster.rs` が存在する
- [ ] `ClusterPlan` / `CrossEdge` struct が定義されている
- [ ] `classify_types(graph, depth, edges) -> ClusterPlan` が pure function として実装されている
- [ ] 6 unit tests (depth 0/1/2 分類、unresolved フォールバック、cross_edges 検出、単一 cluster 時の cross_edges 空検証) が pass
- [ ] `mod.rs` に `pub mod type_graph_cluster;` が登録されている

### T004: Cluster directory layout + stale cleanup

- [ ] `TypeGraphRenderOptions::cluster_depth: usize` (Default=2) が追加されている
- [ ] `write_type_graph_dir` が `<layer>-graph/index.md` + `<cluster>.md` 群を出力する
- [ ] cluster ファイル名が `module_path` の `::` を `_` に置換している (例: `domain_review_v2.md`)
- [ ] CLI `--cluster-depth N` flag が動作し、cluster_depth>0 / =0 で dispatch が分かれる
- [ ] Stale cleanup: flat→cluster 切り替え時に `.md` が削除される
- [ ] Stale cleanup: cluster→flat 切り替え時に dir が `remove_dir_all` される
- [ ] 削除前の `reject_symlinks_below` 二重ガードが symlink injection を fail-closed で拒否する (test で確認)
- [ ] 6 tests (dir 作成 / path traversal ガード / stale flat→cluster / stale cluster→flat / overview nodes / clustered intra-cluster only) が pass

### T005: Fields + Impls edges

- [ ] `EdgeSet::Fields` が `TypeNode::members` から `A --- B` (実線矢印なし) edge を生成する
- [ ] `EdgeSet::Impls` が `TypeNode::trait_impls` から `A -.-> Trait` (破線) edge を生成する
- [ ] Trait ノードが stadium shape `([TraitName])` + `classDef traitNode` で描画される
- [ ] `EdgeSet::All` が methods + fields + impls の union を正しく出力する
- [ ] CLI `--edges methods|fields|impls|all` flag が動作する
- [ ] flat と cluster 両方のパスで field / impl edges が描画される
- [ ] 4 tests (field edges / impl dashed / All union / trait stadium) が pass

### T006: Multi-layer readability verification + ADR 実測補強

- [ ] `sotp track type-graph <id> --cluster-depth 2 --edges all` を 3 層で実行完了
- [ ] verification.md に以下を記録:
  - [ ] domain / usecase / infrastructure の node / edge / cluster 数
  - [ ] cluster_depth 1 vs 2 の可読性比較
  - [ ] trait impl 破線の hexagonal port/adapter 可視化有効性 (FsReviewStore -.-> ReviewReader 等の具体例)
  - [ ] deduplicate_typestate_edges default 判断の実測結論 (ADR Open Questions §4 への実測解答)
- [x] ADR 2026-04-16-2200 §Phase 2 Scope Update §S1-§S5 が存在する (planning 時に追加済み)
- [ ] §Phase 2 Scope Update §S4 / §S5 に実測データ (node/edge/cluster 数、dedup 結論、scope (K) 延期正当性) を補強追記した

## Manual Verification Steps

```bash
# 1. CI 全通過
cargo make ci

# 2. 3 層 cluster 生成 (depth 2)
cargo run --quiet -p cli -- track type-graph tddd-type-graph-cluster-2026-04-17 \
  --cluster-depth 2 --edges all

# 2b. 可読性比較用: depth 1 でも生成して比較
cargo run --quiet -p cli -- track type-graph tddd-type-graph-cluster-2026-04-17 \
  --cluster-depth 1 --edges all --force
# 期待: より粗い cluster 分割 (depth 1 は domain/usecase/infrastructure レベル)
# depth 1 vs depth 2 の可読性を比較し、T006 checklist 「cluster_depth 1 vs 2 の可読性比較」を記録する

# 3. 生成物確認
ls track/items/tddd-type-graph-cluster-2026-04-17/
# 期待: domain-graph/ usecase-graph/ infrastructure-graph/

cat track/items/tddd-type-graph-cluster-2026-04-17/domain-graph/index.md
# 期待: cluster 分割された overview + trait impl 破線

# 4. Stale cleanup 動作確認 (flat→cluster→flat 往復)
cargo run --quiet -p cli -- track type-graph tddd-type-graph-cluster-2026-04-17 \
  --cluster-depth 0  # flat に戻す
ls track/items/tddd-type-graph-cluster-2026-04-17/domain-graph* 2>/dev/null
# 期待: domain-graph.md のみ (dir 削除済み)

# 5. TDDD-BUG-02 修正確認
cargo run --quiet -p cli -- track type-signals <task> --layer usecase
# エラー時に usecase-types.json が表示されることを確認 (domain-types.json ではなく)
```

## Result

_To be completed after implementation._

| Task | Status | Notes |
|------|--------|-------|
| T001 | Pending | TDDD-BUG-02 — catalogue_file 引数追加 |
| T002 | Pending | TDDD-Q01 — SECTIONS 網羅テスト |
| T003 | Pending | ClusterPlan module |
| T004 | Pending | write_type_graph_dir + stale cleanup |
| T005 | Pending | Fields + Impls edges |
| T006 | Pending | 可読性検証 + ADR 実測補強 |

## Open Issues

_To be populated as issues surface during implementation._

## Phase 2 Scope Modifications Recorded

本 track では ADR 2026-04-16-2200 Phase 2 full scope に対し以下の縮小 (scope K) を実施:

- **延期**: Entry point detection (ADR §D3) — 現 codebase (enum-first / typestate 0 件) で navigation 価値の実証困難
- **延期**: DRIFT-01 foundation (ADR §D8) — 現 architecture-rules.json の crate 粒度では verify layers 重複
- **延期**: Orphan type detection (ADR §D8) — DRIFT-01 と同トラック扱い
- **延期**: Auto-render integration in sotp track type-signals (ADR §D6) — 可読性 ROI 未実証のため living document 化を先送り

延期理由と再評価条件は ADR 2026-04-16-2200 §Phase 2 Scope Update §S1-§S5 に planning 時に追記済み。T006 では §S4/§S5 に実測データ (node/edge/cluster 数、dedup 結論) を補強追記する。

## Verified At

_To be filled after completion._
