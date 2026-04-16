<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: approved
approved_at: "2026-04-16T11:48:10Z"
version: "1.0"
signals: { blue: 20, yellow: 0, red: 0 }
---

# TDDD Type Graph View Phase 1 — domain 層 mermaid 最小スパイク

## Goal

TypeGraph から mermaid 図を生成する最小レンダラーを実装し domain 層で可読性を検証する (ADR Phase 1 spike)
sotp track type-graph CLI コマンドを追加し domain 層に対して <layer>-graph.md を 1 枚出力する
mermaid のノード数限界を実測し Phase 2 (cluster + multi-layer) の設計判断に入力を提供する

## Scope

### In Scope
- libs/infrastructure/src/tddd/type_graph_render.rs を新設し TypeGraphRenderOptions / EdgeSet / render_type_graph_flat を実装する。TypeGraph から method edge (self-receiver の return type) を抽出し mermaid flowchart LR を生成する [source: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §D1, §D2(a), libs/domain/src/schema.rs §TypeGraph, §TypeNode] [tasks: T001]
- apps/cli/src/commands/track/tddd/graph.rs を新設し sotp track type-graph <track-id> --layer <layer_id> コマンドを実装する。signals.rs の execute_type_signals パターンを踏襲する [source: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §D6, apps/cli/src/commands/track/tddd/signals.rs §execute_type_signals] [tasks: T002]
- CLI dispatch に type-graph サブコマンドを追加し 2 箇所に登録する: (1) apps/cli/src/commands/track/tddd/mod.rs に graph モジュールを追加、(2) apps/cli/src/commands/track/mod.rs の TrackCommand enum に TypeGraph variant を追加し execute() の match arm を実装する [source: apps/cli/src/commands/track/tddd/mod.rs, apps/cli/src/commands/track/mod.rs §TrackCommand, §execute] [tasks: T002]
- TDD red→green: unit tests (empty graph / single edge / multi-edge / enum shape) を先に追加し red 確認後に実装で green 化する [source: convention — .claude/rules/05-testing.md §Core Principles, inference — render function requires boundary cases covering empty/single/multi edges and enum node shapes to validate mermaid output correctness] [tasks: T001, T002]
- cargo make ci 全通過と domain-graph.md 生成物の可読性検証を実施する [source: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §D7 Phase 1 Acceptance] [tasks: T003]

### Out of Scope
- クラスタリング (--cluster-depth) は Phase 2 で実装する [source: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §D4, §D7 Phase 2]
- フィールド/variant エッジと trait impl 破線エッジは Phase 2 で実装する (TypeNode::trait_impls は tddd-05 merge 済みで既存; Phase 2 は実装スコープの選択として延期) [source: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §D2(b), §D2(c), libs/domain/src/schema.rs §TypeNode::trait_impls]
- Entry point 検出は Phase 2 で実装する [source: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §D3]
- 経路クエリ CLI (sotp track type-path) は Phase 3 で実装する [source: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §D6, §D7 Phase 3]
- DRIFT-01/02 統合と TDDD-BUG-02/TDDD-Q01 修正は Phase 2 で実装する [source: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §D8, §D9]
- sotp track type-signals からの自動レンダー統合は Phase 2 で実装する [source: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §D6]

## Constraints
- TDDD 不変条件 (layer-agnostic / rustdoc-only / domain serde なし) を侵さない純粋な view 拡張であること [source: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §5, knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md §D6]
- 既存の TypeCatalogueDocument / TypeGraph の schema に変更を加えない [source: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §D1]
- small task commits: 各 task の diff は 500 行未満を目標とする [source: convention — .claude/rules/10-guardrails.md §Small task commits]
- TDD red→green の順序を守る [source: convention — .claude/rules/05-testing.md §Core Principles]

## Acceptance Criteria
- [ ] libs/infrastructure/src/tddd/type_graph_render.rs が存在し render_type_graph_flat 関数が TypeGraph から mermaid flowchart LR を生成する [source: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §D1] [tasks: T001]
- [ ] sotp track type-graph <id> --layer domain が <layer>-graph.md を書き出す [source: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §D6, §D7 Phase 1] [tasks: T002]
- [ ] 生成された mermaid 図で method edge (self→return type) が正しく表現されている [source: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §D2(a)] [tasks: T001]
- [ ] cargo make ci (full suite: fmt-check + clippy + nextest + test-doc + deny + python-lint + scripts-selftest + check-layers + verify-canonical-modules + verify-arch-docs + verify-doc-links + verify-plan-progress + verify-track-metadata + verify-track-registry + verify-tech-stack + verify-orchestra + verify-latest-track + verify-module-size + verify-domain-strings + verify-domain-purity + verify-usecase-purity + verify-view-freshness + verify-spec-coverage + verify-spec-states-current) が全通過する [source: convention — .claude/rules/07-dev-environment.md §Pre-commit Checklist, inference — full ci-local task dependency list is defined in Makefile.toml [tasks.ci-local] and is the authoritative commit gate] [tasks: T003]
- [ ] domain 層の TypeGraph ノード数と mermaid 可読性の実測結果が verification.md に記録されている [source: knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §D7 Phase 1 Acceptance] [tasks: T003]

## Related Conventions (Required Reading)
- knowledge/conventions/source-attribution.md
- knowledge/conventions/nightly-dev-tool.md
- knowledge/conventions/hexagonal-architecture.md

## Signal Summary

### Stage 1: Spec Signals
🔵 20  🟡 0  🔴 0

