<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# TDDD Contract Map Phase 1 (MVP) — 全層カタログ入力統合 mermaid view

ADR 2026-04-17-1528 の Phase 1 (MVP) を実装する: 全層カタログを入力とする単一 mermaid ファイル `contract-map.md` の生成
スコープ: D1 render API / D2 subgraph 配置 / D3 13 kinds shape mapping / D4 method + trait impl edge
除外 (Phase 2/3 送り): action overlay / signal overlay / spec_source edge / baseline diff view / AI briefing 自動添付
usecase 層に orchestration (RenderContractMapInteractor) を置く。既存 type-graph の CLI 直結パターンとは非対称になるが、本 track ではリファクタしない (別 track 扱い)
layer-agnostic 不変条件 (ADR §4.5) を 2 層 / 3 層 / 独自層名の fixture で機械的に検証する

## S001 — ADR 整合性訂正 (13 variants + Reality View 補記)

ADR 2026-04-17-1528 の D3 shape mapping 表を 12 variants から 13 variants に訂正する (SecondaryAdapter 追加、ADR 2026-04-15-1636 で導入済み)
ADR 2026-04-16-2200 に §D10 "Reality View as drill-down to Contract Map" を新設し、Contract Map ADR の Open Question Q6 を解消する (D7 は既存の「段階的実装」節が使用済みのため D10 を使用)
2 ADR の整合を先に取ることで、実装 task (T003 以降) が参照する仕様を固定する
docs のみ、production code 変更なし

- [ ] ADR 整合性訂正: ADR 2026-04-17-1528 の D3 shape mapping 表に SecondaryAdapter (13 番目) を追加し、12 variants 表記を 13 に訂正する。ADR 2026-04-16-2200 に §D10 "Reality View as drill-down to Contract Map" を新設して Reality View を Contract Map の drill-down と位置付け直す (Open Q6 対応。D7 は既存の「段階的実装」節が使用済みのため D10 を使用)。Docs のみ、production code 変更なし。

## S002 — Infrastructure helpers (bulk loader + topo sort + extract_type_names 公開化)

libs/infrastructure/src/tddd/ に複数層カタログ一括 loader と共通 helper を追加する
load_all_catalogues(track_dir, rules_path, trusted_root) -> Result<(Vec<LayerId>, BTreeMap<LayerId, TypeCatalogueDocument>), LoadAllCataloguesError>: tddd.enabled 層のみを load_tddd_layers_from_path で列挙し、binding ごとに catalogue_codec::decode を呼ぶ。Vec<LayerId> は may_depend_on のトポロジカルソート順を保持 (FsCatalogueLoader が CatalogueLoader::load_all の契約を満たすために必要)。不在カタログの扱いは domain エラーで明示区別 (skip せず error)
layer_order 決定: load_tddd_layers_from_path の返す順序でトポロジカル成立を確認、不成立なら may_depend_on から topo_sort helper を追加
extract_type_names を libs/infrastructure/src/tddd/type_graph_render.rs から pub 昇格 (infrastructure crate 内部での再利用のため。domain の render_contract_map は独自型名抽出ロジックを domain 内に実装し、infrastructure への逆依存を作らない)
reject_symlinks_below ガードを load_all_catalogues の各 read 前に適用 (既存 type_graph と同じパターン)
unit tests: 3 層正常 load / 1 層不在エラー / symlink 拒否 / topo_sort の正当性 / extract_type_names 公開化の visibility test

- [ ] Infrastructure helpers: load_all_catalogues(track_dir, rules_path, trusted_root) -> Result<(Vec<LayerId>, BTreeMap<LayerId, TypeCatalogueDocument>), _> を libs/infrastructure/src/tddd/catalogue_bulk_loader.rs に新設。Vec<LayerId> は may_depend_on のトポロジカルソート順を保持 (FsCatalogueLoader が CatalogueLoader::load_all の契約を満たすために必要)。load_tddd_layers_from_path の返す binding を iterate し、binding ごとに reject_symlinks_below + catalogue_codec::decode。不在カタログは明示エラー (skip しない)。layer_order はバインディング列挙順が may_depend_on のトポロジカル順であることを確認、不成立なら topo_sort helper を追加。extract_type_names を type_graph_render.rs から infrastructure crate 内 pub 昇格 (domain からは使用しない、domain → infrastructure 逆依存禁止)。unit tests: 正常 load / 不在エラー / symlink 拒否 / topo_sort / visibility。

## S003 — Contract Map pure render module (13 kinds + method + trait impl edge)

libs/domain/src/tddd/contract_map_render.rs を新設 (domain 配置: I/O なし純粋変換、usecase から直接呼ぶため hexagonal 上適切)
render_contract_map(catalogues, layer_order, opts) -> ContractMapContent: pure function、I/O なし、snapshot-style unit test
subgraph per tddd.enabled 層 (layer_order で左→右配置、subgraph ラベルは layers[].crate をそのまま使用、層名ハードコードなし)
13 kind variants の shape + classDef 定義 (typestate stadium / enum hexagon / value_object round / error_type flag / secondary_port subroutine / application_service 平行四辺形 / use_case / interactor / dto rect / command / query / factory / secondary_adapter)
method call edge (実線): MethodDeclaration.returns から extract_type_names で参照型名を抽出し、同じ catalogues に含まれる型への edge を生成
trait impl edge (破線): SecondaryAdapter.implements の TraitImplDecl から対応する secondary_port kind ノードへの edge を生成。層跨ぎ edge が自然に現れる
ContractMapRenderOptions { layers: Vec<LayerId>, kind_filter: Option<Vec<TypeDefinitionKind>>, signal_overlay: bool (Phase1 では false 固定・将来フィールド)、action_overlay: bool (同)、include_spec_source_edges: bool (同) }
Phase 1 では signal/action/spec_source overlay の実装は空スタブ。フィールドだけ public API に確保し Phase 2/3 で実装
kind_filter が全 variant を除外した場合は空 subgraph の mermaid を返す (error ではない、CLI 側で警告)
snapshot unit tests: 3 層 fixture、各 kind の shape 出現、method edge 描画、trait impl edge 描画、kind_filter 効き、layer subset 効き

- [ ] Contract Map pure render function: libs/domain/src/tddd/contract_map_render.rs を新設 (domain free function として実装)。render_contract_map(catalogues: &BTreeMap<LayerId, TypeCatalogueDocument>, layer_order: &[LayerId], opts: &ContractMapRenderOptions) -> ContractMapContent (pure, infallible)。subgraph per tddd.enabled 層 (LayerId をラベル、layer_order で左→右)。13 kind variants の shape/classDef 実装。method call edge (returns から参照型名抽出、catalogues 内の型への edge のみ)。trait impl edge (SecondaryAdapter.implements から secondary_port ノードへの破線)。ContractMapRenderOptions の 5 フィールドを public API に確保 (signal_overlay/action_overlay/include_spec_source_edges は Phase1 では false 固定のスタブ)。kind_filter 全除外時は空 subgraph 返却。snapshot tests: shape 出現検証、method/trait impl edge 描画、filter 効き。domain 配置により usecase から直接呼び出せる。

## S004 — Domain ports + Infrastructure adapters

hexagonal 分離: usecase 層の orchestration が依存する domain port を追加する
libs/domain/src/tddd/catalogue_ports.rs 新規: CatalogueLoader trait (fn load_all(&self, &TrackId) -> Result<(Vec<LayerId>, BTreeMap<LayerId, TypeCatalogueDocument>), CatalogueLoaderError>、Vec は may_depend_on のトポロジカルソート順を保持) と ContractMapWriter trait (fn write(&self, &TrackId, &ContractMapContent) -> Result<(), ContractMapWriterError>)
ADR 2026-04-14-1531 (domain serde 依存除去) 準拠: port 定義は serde 不使用、TypeCatalogueDocument は既に domain 型
libs/infrastructure/src/tddd/contract_map_adapter.rs 新規: FsCatalogueLoader (T002 の load_all_catalogues をラップ、CatalogueLoader 実装) と FsContractMapWriter (atomic_write_file + reject_symlinks_below、書き出しパス track_dir/contract-map.md、ContractMapWriter 実装)
unit tests: FsCatalogueLoader の port 契約 / FsContractMapWriter の atomic 書き出し / symlink 拒否

- [ ] Domain ports + infrastructure adapters + nutype value objects: libs/domain/src/tddd/ に LayerId / ContractMapContent (nutype value_objects) / ContractMapRenderOptions (value_object) / CatalogueLoader trait / ContractMapWriter trait / CatalogueLoaderError / ContractMapWriterError を新設。serde 不使用 (ADR 2026-04-14-1531 準拠)。port 方法シグネチャ: CatalogueLoader::load_all(&self, &TrackId) -> Result<(Vec<LayerId>, BTreeMap<LayerId, TypeCatalogueDocument>), CatalogueLoaderError> (Vec は topological 順序を保持、BTreeMap は検索に使用)、ContractMapWriter::write(&self, &TrackId, &ContractMapContent) -> Result<(), ContractMapWriterError>。libs/infrastructure/src/tddd/ に FsCatalogueLoader (T002 の load_all_catalogues をラップ、CatalogueLoader 実装) と FsContractMapWriter (atomic_write_file + reject_symlinks_below、書き出しパス track_dir/contract-map.md、ContractMapWriter 実装)。adapter 側 tests: port 契約充足 / atomic 書き出し / symlink 拒否。

## S005 — usecase interactor (RenderContractMapInteractor)

libs/usecase/src/contract_map_workflow.rs 新規
RenderContractMapCommand { track_id: TrackId, kind_filter: Option<Vec<TypeDefinitionKind>>, layer_filter: Option<Vec<LayerId>> }
RenderContractMapOutput { rendered_layer_count: usize, total_entry_count: usize }
RenderContractMapError: CatalogueLoaderFailed / ContractMapWriterFailed / EmptyCatalogue / LayerNotFound
RenderContractMapInteractor<L: CatalogueLoader, W: ContractMapWriter>: new(loader, writer) + execute(&self, cmd) -> Result<Output, Error>
実行フロー: loader.load_all → (kind_filter / layer_filter 適用) → contract_map_render::render_contract_map → writer.write
mock 実装 (mockall) を使った usecase-level tests: happy path / loader error 伝播 / writer error 伝播 / kind_filter 経由の subset 描画 / 全層 kind_filter で空に絞られた場合の empty mermaid

- [ ] usecase interactor + application_service trait: libs/usecase/src/contract_map_workflow.rs 新設。RenderContractMap trait (application_service primary port, execute(&self, &RenderContractMapCommand) -> Result<RenderContractMapOutput, RenderContractMapError>)、RenderContractMapCommand { track_id: TrackId, kind_filter, layer_filter: Option<Vec<LayerId>> }、RenderContractMapOutput { rendered_layer_count, total_entry_count }、RenderContractMapError (CatalogueLoaderFailed/ContractMapWriterFailed/EmptyCatalogue/LayerNotFound)、RenderContractMapInteractor<L: CatalogueLoader, W: ContractMapWriter> が RenderContractMap trait を実装。フロー: loader.load_all → filter 適用 → domain::tddd::render_contract_map (free function) → writer.write。mockall を使った usecase-level tests: happy / loader error / writer error / kind_filter 動作 / 全除外時の empty mermaid。

## S006 — CLI subcommand (sotp track contract-map)

apps/cli/src/commands/track/tddd/contract_map.rs 新規
CLI subcommand: sotp track contract-map <track-id> [--kind-filter k1,k2] [--layers l1,l2]
clap の引数定義: track_id (positional), --kind-filter (comma-separated kind_tag), --layers (comma-separated layer id)
parse_kind_filter / parse_layer_filter helper (既存 type-graph の parse_edge_set パターンを踏襲)
dispatch: CLI → RenderContractMap trait (application_service primary port) 経由 → RenderContractMapInteractor::execute → 出力 track/items/<id>/contract-map.md (CLI は concrete RenderContractMapInteractor に直接依存しない)
apps/cli/src/commands/track/tddd/mod.rs にサブコマンド登録
apps/cli/src/commands/track/mod.rs の既存 type-graph 登録箇所と同じ場所に contract-map を追加
既存 type-graph は CLI → infrastructure 直接パターンのまま維持 (本 track では変更しない、後追いリファクタは別 track)

- [ ] CLI subcommand: apps/cli/src/commands/track/tddd/contract_map.rs 新設。sotp track contract-map <track-id> [--kind-filter k1,k2] [--layers l1,l2] を clap で定義。parse_kind_filter / parse_layer_filter helper (既存 parse_edge_set パターン踏襲)。dispatch: CLI → RenderContractMap trait (application_service) 経由 → RenderContractMapInteractor::execute → 出力 track/items/<id>/contract-map.md。apps/cli/src/commands/track/tddd/mod.rs および apps/cli/src/commands/track/mod.rs に登録。既存 type-graph は変更しない。

## S007 — Layer-agnostic fixture tests (2 層 / 3 層 / 独自層名)

libs/infrastructure/tests/fixtures/architecture_rules/ に 3 種類の fixture を配置する
fixture_2layers.json: core / adapter の 2 層構成 (現 SoTOHE-core と異なる層名)
fixture_3layers_default.json: domain / usecase / infrastructure の現 SoTOHE-core 構成
fixture_custom_names.json: application / port / gateway の独自命名 3 層構成
各 fixture に対し render_contract_map 出力を検証: subgraph 数が tddd.enabled 層数と一致、subgraph ラベルが fixture の crate 名と一致、may_depend_on に沿った左→右配置
layer-agnostic 不変条件 (ADR §4.5) の違反検出テスト: expected に含まれない層名 (例: fixture_custom_names の subgraph に domain/usecase が混入しないこと) を assert_eq! で確認
fixture 用 TypeCatalogueDocument の最小サンプルも同ディレクトリに配置
tests: 3 fixture × 基本 render 検証 = 最低 6 test

- [ ] Layer-agnostic fixture tests: libs/infrastructure/tests/fixtures/architecture_rules/ に fixture_2layers.json (core/adapter) / fixture_3layers_default.json (domain/usecase/infrastructure) / fixture_custom_names.json (application/port/gateway) を配置。各 fixture 用の最小カタログサンプルも同梱。render_contract_map 出力を検証: subgraph 数 == tddd.enabled 層数、subgraph ラベル == fixture の crate 名、may_depend_on に沿った左→右配置、他 fixture の層名が出力に混入しないこと。3 fixture × 基本 render = 最低 6 test。
