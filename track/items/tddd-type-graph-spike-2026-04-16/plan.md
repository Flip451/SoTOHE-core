<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# TDDD Type Graph View Phase 1 — domain 層 mermaid 最小スパイク

ADR 2026-04-16-2200-tddd-type-graph-view の Phase 1 (minimum spike) を実装する
domain 層に対してクラスタなし・メソッドエッジのみの mermaid 図を 1 枚出すレンダラーと CLI コマンドを追加する
目的は可読性の実測と mermaid ノード数限界の確認であり Phase 2 以降の設計判断に入力を提供する

## Infrastructure mermaid renderer

libs/infrastructure/src/tddd/type_graph_render.rs を新設し TypeGraphRenderOptions / EdgeSet / render_type_graph_flat を実装する
TypeGraph から method edge (self-receiver メソッドの return type) を抽出し mermaid flowchart LR を生成する
type ノードは struct=[rect] / enum={diamond} で表現し凡例を図先頭に配置する
既存 type_catalogue_render.rs のパターン (Generated from ヘッダー) を踏襲する

- [x] type_graph_render.rs — mermaid renderer (TypeGraphRenderOptions / EdgeSet / render_type_graph_flat) + unit tests. libs/infrastructure/src/tddd/type_graph_render.rs を新設し mod.rs に登録。TDD: empty graph / single edge / multi-edge / enum shape テスト 88f63f72937edb04d4d61a50b23d0739d360b325

## CLI command sotp track type-graph + wiring

apps/cli/src/commands/track/tddd/graph.rs を新設し execute_type_graph 関数を実装する
signals.rs の execute_type_signals パターンを踏襲: resolve_layers → ensure_active_track → RustdocSchemaExporter → build_type_graph → render → atomic_write_file
出力先は track/items/<id>/<layer>-graph.md (Phase 1 はフラット 1 ファイル)
CLI dispatch に type-graph サブコマンドを 2 箇所に登録: (1) apps/cli/src/commands/track/tddd/mod.rs に graph モジュールを追加、(2) apps/cli/src/commands/track/mod.rs の TrackCommand enum に TypeGraph variant を追加し execute() match arm を実装する

- [ ] graph.rs — CLI command sotp track type-graph + wiring. apps/cli/src/commands/track/tddd/graph.rs を新設。resolve_layers / ensure_active_track / RustdocSchemaExporter / build_type_graph / atomic_write_file を使い <layer>-graph.md を出力。2 箇所に登録: (1) apps/cli/src/commands/track/tddd/mod.rs に graph モジュールを追加、(2) apps/cli/src/commands/track/mod.rs の TrackCommand enum に TypeGraph variant を追加し execute() match arm を実装。TDD: invalid track / missing layer / dispatch テスト

## CI gate + 生成物可読性検証

cargo make ci 全通過を確認する
domain 層で実際に domain-graph.md を生成し mermaid ノード数と可読性を実測する
verification.md に Phase 2 への判断材料 (ノード数限界・クラスタ粒度の推奨) を記録する

- [ ] CI gate + 生成物可読性検証。cargo make ci 全通過確認。domain-graph.md の生成物を目視確認 (integration test #[ignore])。verification.md に domain 層ノード数・可読性所感・Phase 2 判断材料を記録
