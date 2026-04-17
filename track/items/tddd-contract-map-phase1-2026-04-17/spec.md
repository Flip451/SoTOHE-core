<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: approved
approved_at: "2026-04-17T17:05:10Z"
version: "1.0"
signals: { blue: 45, yellow: 0, red: 0 }
---

# TDDD Contract Map Phase 1 (MVP) — 全層カタログ入力統合 mermaid view

## Goal

ADR 2026-04-17-1528 の Phase 1 (MVP) を実装し、全 tddd.enabled 層のカタログを入力とする 1 枚の mermaid 図 (contract-map.md) を生成する
SoT chain の 型カタログ層を単一 artifact で俯瞰可能にし、designer の設計意図を視覚化する初めての手段を提供する
layer-agnostic 不変条件 (ADR §4.5) を保ち、任意のアーキテクチャ構成 (2 層 / 3 層 / 独自層名) で動作する
usecase 層に orchestration (RenderContractMapInteractor) を配置し、hexagonal 分離を保った実装パターンを確立する
Phase 2 (signal / action overlay) および Phase 3 (spec_source edge / baseline diff / AI briefing 統合) の土台となる render API を確定する

## Scope

### In Scope
- ADR 2026-04-17-1528 の D3 shape mapping 表を 12 variants から 13 variants に訂正し、SecondaryAdapter (ADR 2026-04-15-1636 で導入) を追加する [source: knowledge/adr/2026-04-17-1528-tddd-contract-map.md §D3, knowledge/adr/2026-04-15-1636-tddd-05-secondary-adapter.md] [tasks: T001]
- ADR 2026-04-16-2200 に §D10 "Reality View as drill-down to Contract Map" を新設し、Contract Map ADR の Open Question Q6 を解消する (D7 は既存の「段階的実装」節が使用済みのため D10 を使用) [source: knowledge/adr/2026-04-17-1528-tddd-contract-map.md §Open Questions §Q6, knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md] [tasks: T001]
- libs/infrastructure/src/tddd/catalogue_bulk_loader.rs を新設し load_all_catalogues(track_dir, rules_path, trusted_root) -> Result<(Vec<LayerId>, BTreeMap<LayerId, TypeCatalogueDocument>), _> を実装する。Vec<LayerId> は may_depend_on のトポロジカルソート順を保持する。load_tddd_layers_from_path の返す binding を iterate し、binding ごとに reject_symlinks_below + catalogue_codec::decode を呼ぶ。不在カタログは明示エラー (skip しない) [source: knowledge/adr/2026-04-17-1528-tddd-contract-map.md §D1, §4.5, libs/infrastructure/src/verify/tddd_layers.rs §load_tddd_layers_from_path, libs/infrastructure/src/tddd/catalogue_codec.rs §decode] [tasks: T002]
- extract_type_names を libs/infrastructure/src/tddd/type_graph_render.rs から infrastructure crate 内で pub に昇格し、catalogue_bulk_loader.rs など infrastructure 内の他モジュールから参照可能にする。domain 層の render_contract_map は domain 内で独自の型名抽出ロジックを実装し、infrastructure の extract_type_names に直接依存しない (domain → infrastructure 依存は架橋禁止) [source: libs/infrastructure/src/tddd/type_graph_render.rs §extract_type_names, convention — knowledge/conventions/hexagonal-architecture.md §Layer Dependency Direction] [tasks: T002]
- libs/domain/src/tddd/contract_map_render.rs を新設し render_contract_map(catalogues: &BTreeMap<LayerId, TypeCatalogueDocument>, layer_order: &[LayerId], opts: &ContractMapRenderOptions) -> ContractMapContent を pure function として実装する。domain 配置 (無 I/O の純粋変換、usecase 層から直接呼ぶため hexagonal 上適切)。subgraph per tddd.enabled 層、13 kind variants の shape/classDef、method call edge、trait impl edge を生成する [source: knowledge/adr/2026-04-17-1528-tddd-contract-map.md §D1, §D2, §D3, §D4, knowledge/adr/2026-04-14-1531-domain-serde-ripout.md §D1] [tasks: T003]
- nutype pattern を最大適用し生 String を排除する: LayerId (value_object wrapping architecture-rules.json layer crate name) と ContractMapContent (value_object wrapping rendered mermaid markdown) を libs/domain/src/tddd/ に追加する [source: knowledge/adr/2026-04-17-1528-tddd-contract-map.md §D1, convention — .claude/rules/04-coding-principles.md §Newtype パターン, libs/domain/src/ids.rs §TrackId] [tasks: T004]
- ContractMapRenderOptions に 5 フィールド (layers / kind_filter / signal_overlay / action_overlay / include_spec_source_edges) を public API に確保する。Phase 1 では後 3 者は false 固定のスタブとして、signature のみ確定させる [source: knowledge/adr/2026-04-17-1528-tddd-contract-map.md §D1, §Implementation Phases] [tasks: T003]
- libs/domain/src/tddd/catalogue_ports.rs を新設し CatalogueLoader / ContractMapWriter trait を定義する。serde 不使用 (ADR 2026-04-14-1531 準拠)。CatalogueLoader::load_all は (Vec<LayerId>, BTreeMap<LayerId, TypeCatalogueDocument>) のタプルを返し、topological 順序を保持する (BTreeMap は key 昇順のため layer order を喪失するため) [source: knowledge/adr/2026-04-17-1528-tddd-contract-map.md §D1, §Notes for track planning, knowledge/adr/2026-04-14-1531-domain-serde-ripout.md §D1] [tasks: T004]
- libs/infrastructure/src/tddd/contract_map_adapter.rs を新設し FsCatalogueLoader (T002 の load_all_catalogues をラップ) と FsContractMapWriter (atomic_write_file + reject_symlinks_below、書き出しパス track_dir/contract-map.md) を実装する [source: knowledge/adr/2026-04-17-1528-tddd-contract-map.md §D1, convention — knowledge/conventions/security.md §Symlink Rejection in Infrastructure Adapters] [tasks: T004]
- libs/usecase/src/contract_map_workflow.rs を新設し、RenderContractMap trait (application_service primary port、execute(&self, &RenderContractMapCommand) -> Result<RenderContractMapOutput, RenderContractMapError>) を定義する。RenderContractMapInteractor<L: CatalogueLoader, W: ContractMapWriter> がこの trait を実装する。フロー: loader.load_all → kind_filter/layer_filter 適用 → domain::tddd::render_contract_map (free function) → writer.write [source: knowledge/adr/2026-04-17-1528-tddd-contract-map.md §D1, convention — knowledge/conventions/hexagonal-architecture.md, knowledge/adr/2026-04-13-1813-tddd-taxonomy-expansion.md §application_service] [tasks: T005]
- apps/cli/src/commands/track/tddd/contract_map.rs を新設し sotp track contract-map <track-id> [--kind-filter k1,k2] [--layers l1,l2] を実装する。clap の引数定義、parse_kind_filter/parse_layer_filter helper、RenderContractMap trait (application_service primary port) 経由での dispatch (CLI は concrete RenderContractMapInteractor に直接依存しない) [source: knowledge/adr/2026-04-17-1528-tddd-contract-map.md §Implementation Phases Phase 1, apps/cli/src/commands/track/tddd/graph.rs §parse_edge_set] [tasks: T006]
- libs/infrastructure/tests/fixtures/architecture_rules/ に 3 種類の fixture (fixture_2layers.json / fixture_3layers_default.json / fixture_custom_names.json) を配置し、render_contract_map が layer-agnostic に動作することを機械的に検証する [source: knowledge/adr/2026-04-17-1528-tddd-contract-map.md §4.5 layer-agnostic 不変条件, §Notes for track planning §5] [tasks: T007]
- TDD red→green: 各 task で unit tests を先に書き red 確認後に実装で green 化する。各 commit diff は 500 LOC 以下に保つ [source: convention — .claude/rules/05-testing.md §Core Principles, convention — .claude/rules/10-guardrails.md §Small task commits] [tasks: T002, T003, T004, T005, T006, T007]

### Out of Scope
- Action overlay (add/modify/delete/reference の視覚化) は ADR §D5 前半に該当し Phase 2 に送る [source: knowledge/adr/2026-04-17-1528-tddd-contract-map.md §Implementation Phases Phase 2]
- Signal overlay (Blue/Yellow/Red のノード塗り) は ADR §D5 後半に該当し Phase 2 に送る。type-signals 出力の読み取りロジックは Phase 2 で追加 [source: knowledge/adr/2026-04-17-1528-tddd-contract-map.md §Implementation Phases Phase 2]
- spec_source edge (ノード → spec セクションの外向きリンク) は spec_source フィールドがカタログ schema に未実装 (TDDD-04 proposed 段階) のため Phase 3 に送る [source: knowledge/adr/2026-04-17-1528-tddd-contract-map.md §Implementation Phases Phase 3, inference — libs/domain/src/tddd/catalogue.rs の TypeCatalogueEntry に spec_source フィールド未実装]
- Baseline diff view (4 グループ差分の色分け) は Phase 3 に送る。現 track では「現在のカタログ」のみを対象とする [source: knowledge/adr/2026-04-17-1528-tddd-contract-map.md §Open Questions §Q4, §Implementation Phases Phase 3]
- /track:review / /track:plan briefing への自動添付は Phase 3 に送る [source: knowledge/adr/2026-04-17-1528-tddd-contract-map.md §Implementation Phases Phase 3]
- 既存 type-graph CLI (ADR 2026-04-16-2200 Phase 2 実装) の usecase 層介在へのリファクタは本 track では扱わない。Contract Map だけが usecase 経由で、type-graph は現状の CLI 直結パターンのまま。非対称は別 track 扱い [source: apps/cli/src/commands/track/tddd/graph.rs, inference — 既存 type-graph は CLI → infrastructure 直接。本 track では Contract Map のみ usecase 層を介し、type-graph のリファクタは scope 爆発を避けるため切り離す]
- Contract Map の living document 化 (sotp track type-signals 成功時の auto-render 統合) は本 track では扱わない。手動 sotp track contract-map 実行で十分 [source: knowledge/adr/2026-04-17-1528-tddd-contract-map.md §Implementation Phases Phase 3, knowledge/adr/2026-04-16-2200-tddd-type-graph-view.md §Phase 2 Scope Update §S5.3]

## Constraints
- layer-agnostic 不変条件: 層名 (domain/usecase/infrastructure 等) を一切ハードコードしない。層リスト・描画順序・catalogue_file はすべて architecture-rules.json 駆動 [source: knowledge/adr/2026-04-17-1528-tddd-contract-map.md §4.5, knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md §D6]
- カタログ schema 変更を伴わない: TypeCatalogueDocument / TypeCatalogueEntry / TypeDefinitionKind の既存 schema は不変。view 拡張と新規 port/interactor/adapter/CLI のみ [source: knowledge/adr/2026-04-17-1528-tddd-contract-map.md §D1, libs/domain/src/tddd/catalogue.rs]
- hexagonal 原則: render (純粋関数) は domain (I/O なし純粋変換、usecase から直接呼ぶため hexagonal 上適切)、port trait は domain、orchestration は usecase、adapter / I/O は infrastructure、composition は apps/cli。libs/domain は serde 非依存 (ADR 2026-04-14-1531) [source: convention — knowledge/conventions/hexagonal-architecture.md, knowledge/adr/2026-04-14-1531-domain-serde-ripout.md §D1]
- Symlink safety: 全ての write / read パスで reject_symlinks_below を通す。書き出しは atomic_write_file 経由 [source: convention — knowledge/conventions/security.md §Symlink Rejection in Infrastructure Adapters, libs/infrastructure/src/tddd/type_graph_render.rs §write_type_graph_file]
- 小さい task commits: 各 task の diff は 500 LOC 未満を目標とする [source: convention — .claude/rules/10-guardrails.md §Small task commits]
- TDD red→green の順序を守る。各 task で unit tests を先に書き red 確認後に実装する [source: convention — .claude/rules/05-testing.md §Core Principles]
- enum-first を維持する (typestate は不要)。ContractMapRenderOptions の Phase 2/3 用スタブフィールドは bool、kind_filter は Option<Vec<TypeDefinitionKind>> [source: convention — .claude/rules/04-coding-principles.md §Enum-first]
- 既存 type-graph 実装 (ADR 2026-04-16-2200 Phase 2) は変更しない。extract_type_names の pub 昇格のみ例外 (非破壊変更) [source: knowledge/adr/2026-04-17-1528-tddd-contract-map.md §Notes for track planning §1]
- mermaid 出力は GitHub の mermaid renderer で正常に描画可能な記法のみ使用する。shape は mermaid 公式サポート内に限定する [source: knowledge/adr/2026-04-17-1528-tddd-contract-map.md §D3, inference — GitHub レンダラー互換性を保たないと contract-map.md が PR で見えなくなる]

## Acceptance Criteria
- [ ] ADR 2026-04-17-1528 の D3 表に SecondaryAdapter 行が追加され、全 13 variants の shape/classDef マッピングが記載されている。ADR 本文の "12 variants" 表記が "13 variants" に訂正されている [source: knowledge/adr/2026-04-17-1528-tddd-contract-map.md §D3, libs/domain/src/tddd/catalogue.rs §TypeDefinitionKind] [tasks: T001]
- [ ] ADR 2026-04-16-2200 に §D10 "Reality View as drill-down to Contract Map" が新設され、両 ADR の役割分担が文書化されている (D7 は既存節のため D10 を使用) [source: knowledge/adr/2026-04-17-1528-tddd-contract-map.md §Open Questions §Q6] [tasks: T001]
- [ ] load_all_catalogues が tddd.enabled 層の全カタログを (Vec<LayerId>, BTreeMap<LayerId, TypeCatalogueDocument>) のタプルとして返す。Vec は may_depend_on のトポロジカルソート順を保持する。不在カタログに対して明示エラー variant を返す。symlink injection は fail-closed で拒否される [source: knowledge/adr/2026-04-17-1528-tddd-contract-map.md §D1, §4.5, convention — knowledge/conventions/security.md §Symlink Rejection in Infrastructure Adapters] [tasks: T002]
- [ ] extract_type_names が pub として公開され、libs/infrastructure/src/tddd/ 外部からも参照可能になる。可視性変更後も既存 type_graph_render の tests が pass する [source: libs/infrastructure/src/tddd/type_graph_render.rs §extract_type_names] [tasks: T002]
- [ ] layer_order が may_depend_on のトポロジカルソート順 (依存なし層が左端) になっていることが unit test で検証される。ソート不成立時は明示エラーを返す [source: knowledge/adr/2026-04-17-1528-tddd-contract-map.md §D2, §4.5, knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md §D6] [tasks: T002]
- [ ] render_contract_map が 13 kind variants 全てについて適切な mermaid shape/classDef を出力する。method call edge と trait impl edge が描画される。kind_filter で絞り込んだ subset が描画される。snapshot tests が pass する [source: knowledge/adr/2026-04-17-1528-tddd-contract-map.md §D2, §D3, §D4] [tasks: T003]
- [ ] ContractMapRenderOptions の 5 フィールドが public API に存在する。Phase 1 では signal_overlay / action_overlay / include_spec_source_edges が渡されても出力が変わらない (将来拡張用の stub 挙動) [source: knowledge/adr/2026-04-17-1528-tddd-contract-map.md §D1, §Implementation Phases] [tasks: T003]
- [ ] kind_filter が全 kind variant を除外した場合、render_contract_map は error ではなく空 subgraph の mermaid を返す。この挙動が unit test で検証される [source: knowledge/adr/2026-04-17-1528-tddd-contract-map.md §D1, inference — kind_filter が全除外でも error でなく空 mermaid を返す仕様は CLI 側の警告責任と整合する] [tasks: T003]
- [ ] LayerId / ContractMapContent が libs/domain/src/tddd/ に newtype wrapper として定義され生 String を排除する。ContractMapRenderOptions が multi-field value_object として定義される (5 フィールド)。CatalogueLoaderError / ContractMapWriterError が error_type として定義される。これらの型が domain-types.json の宣言と一致することが確認される [source: knowledge/adr/2026-04-17-1528-tddd-contract-map.md §D1, convention — .claude/rules/04-coding-principles.md §Newtype パターン] [tasks: T004]
- [ ] CatalogueLoader / ContractMapWriter trait が libs/domain/src/tddd/catalogue_ports.rs に定義され、serde 依存を持たない。FsCatalogueLoader / FsContractMapWriter が port 契約を実装し unit test で契約充足が検証される [source: knowledge/adr/2026-04-17-1528-tddd-contract-map.md §D1, knowledge/adr/2026-04-14-1531-domain-serde-ripout.md §D1] [tasks: T004]
- [ ] RenderContractMap trait が libs/usecase/src/contract_map_workflow.rs に application_service primary port として定義されている。シグネチャ: execute(&self, &RenderContractMapCommand) -> Result<RenderContractMapOutput, RenderContractMapError>。CLI は concrete RenderContractMapInteractor に直接依存せず、この trait を介して dispatch する [source: track/items/tddd-contract-map-phase1-2026-04-17/usecase-types.json, knowledge/adr/2026-04-17-1528-tddd-contract-map.md §D1, convention — knowledge/conventions/hexagonal-architecture.md] [tasks: T005]
- [ ] RenderContractMapInteractor::execute が Command を受け取り Output または Error を返す。loader / writer / render の失敗が対応する Error variant に正しく伝播する。mockall を使った 5 tests (happy / loader_err / writer_err / kind_filter / 全除外) が pass する [source: knowledge/adr/2026-04-17-1528-tddd-contract-map.md §D1, convention — knowledge/conventions/hexagonal-architecture.md] [tasks: T005]
- [ ] RenderContractMapError の EmptyCatalogue と LayerNotFound の発火条件が明確に定義されていること: EmptyCatalogue は loader.load_all が空の catalogue set を返した場合 (tddd.enabled 層が 0 件)、LayerNotFound は layer_filter に指定した LayerId が load_all の結果に存在しない場合。それぞれ unit test で検証される [source: knowledge/adr/2026-04-17-1528-tddd-contract-map.md §D1 RenderContractMap::execute の失敗条件] [tasks: T005]
- [ ] sotp track contract-map <track-id> が track/items/<id>/contract-map.md を生成する。--kind-filter / --layers が反映される。--help が spec と一致する [source: knowledge/adr/2026-04-17-1528-tddd-contract-map.md §Implementation Phases Phase 1] [tasks: T006]
- [ ] 3 fixture (2層 / 3層デフォルト / 独自層名) に対して render が成功する。subgraph 数と subgraph ラベルが fixture に一致する。subgraph の出現順序が fixture の may_depend_on から算出したトポロジカル順 (may_depend_on なし層が先頭) に一致することを assert する。他 fixture の層名 (例: custom_names の出力に domain が混入) が出現しないことが assert される [source: knowledge/adr/2026-04-17-1528-tddd-contract-map.md §4.5] [tasks: T007]
- [ ] cargo make ci が全通過する (fmt-check + clippy + nextest + test-doc + deny + python-lint + scripts-selftest + check-layers + verify-* 一式)。T001 については verify-doc-links / verify-arch-docs の通過が特に重要 [source: convention — .claude/rules/07-dev-environment.md §Pre-commit Checklist] [tasks: T001, T002, T003, T004, T005, T006, T007]

## Related Conventions (Required Reading)
- knowledge/conventions/source-attribution.md
- knowledge/conventions/hexagonal-architecture.md
- knowledge/conventions/security.md

## Signal Summary

### Stage 1: Spec Signals
🔵 45  🟡 0  🔴 0

