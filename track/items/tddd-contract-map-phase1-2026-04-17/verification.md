# Verification: tddd-contract-map-phase1-2026-04-17

## Scope Verified

- [x] In-scope items match ADR 2026-04-17-1528 Phase 1 (MVP — method + trait impl edge, layer-agnostic)
- [x] Out-of-scope items correctly deferred (action overlay / signal overlay / spec_source edge / baseline diff / AI briefing auto-injection / type-graph usecase refactor)
- [x] layer-agnostic 不変条件 (ADR §4.5) が 3 fixture で機械的に検証されている

## Task Verification

### T001: ADR 整合性訂正

- [x] ADR 2026-04-17-1528 §D3 shape mapping 表に SecondaryAdapter 行が追加されている (計画時点で既に 13 行整合済み。本 task 実行時に追加編集は不要だったことを確認)
- [x] ADR 2026-04-17-1528 本文の \"12 variants\" が \"13 variants\" に訂正されている (line 11 / line 200 の「12 → 13 variants」変遷注記以外に \"12 variants\" 表記は残存せず、既に整合。本 task ではさらに §Q6 を "Resolved (2026-04-17)" に更新)
- [x] ADR 2026-04-16-2200 に §D10 \"Reality View as drill-down to Contract Map\" が新設されている (D7 は既存の「段階的実装」節が使用済みのため D10 を使用)。§D9 の末尾と \"## Rejected Alternatives\" の間に挿入。Contract Map を primary artifact、Reality View を drill-down と位置付け、役割分担表 / Phase 計画への影響 / Open Q6 解消を記載
- [x] `cargo make verify-arch-docs` が通る (docs 整合性確認)
- [x] `cargo make verify-doc-links` が通る (ADR 間相互リンク確認)
- [ ] `cargo make ci` が通る (commit 時に `track-commit-message` ラッパーが自動実行)

### T002: Infrastructure helpers

- [x] `libs/infrastructure/src/tddd/catalogue_bulk_loader.rs` が存在し `load_all_catalogues(track_dir, rules_path, trusted_root) -> Result<(Vec<LayerId>, BTreeMap<LayerId, TypeCatalogueDocument>), LoadAllCataloguesError>` が提供されている
- [x] 不在カタログに対して `LoadAllCataloguesError::CatalogueNotFound { layer_id, path }` が返される (`test_load_all_catalogues_missing_catalogue_returns_error_not_skip`)
- [x] symlink injection が fail-closed で拒否される (`test_load_all_catalogues_rejects_symlink_catalogue`, `#[cfg(unix)]`)
- [x] `extract_type_names` が `pub(crate)` 昇格されている (infrastructure 内の他モジュール / tests から参照可能。`test_extract_type_names_is_reusable_from_this_module` で visibility を確認)
- [x] layer_order が `may_depend_on` のトポロジカル順 (依存なし層が左端) であることが assert される (`test_load_all_catalogues_happy_path_sorts_topologically` — fixture は `infrastructure → usecase → domain` の逆順で並べ、`load_all_catalogues` が `[domain, usecase, infrastructure]` を返すことを確認)
- [x] unit tests (正常 load / 不在エラー / symlink 拒否 / topo_sort / visibility + cycle 検出 / tie-order / deps-outside-enabled) 合計 7 tests が pass
- [x] `LayerId` VO が `libs/domain/src/tddd/layer_id.rs` に追加された (T004 acceptance criterion の一部を本 task で先行導入 — `Vec<LayerId>` 戻り値の型要件ため。残りの VOs (`ContractMapContent` / `ContractMapRenderOptions`) + port traits は T004 で追加。当初 nutype で実装したが T007 にて素 struct に書き換え — deviation 詳細は `## Verified At` の deviation セクションを参照)
- [x] domain `ValidationError` に `InvalidLayerId(String)` variant を追加
- [x] Dead code として設計段階に書いた `LoadAllCataloguesError::UnknownDependency` variant は実装レビュー時に削除 (topological_sort が enabled set 外の deps を silently ignore するため到達不能)

### T003: Contract Map pure render module

- [x] `libs/domain/src/tddd/contract_map_render.rs` が存在し `render_contract_map(&BTreeMap<LayerId, TypeCatalogueDocument>, &[LayerId], &ContractMapRenderOptions) -> ContractMapContent` が pure function (I/O なし、`#[must_use]`) として domain 内に実装されている
- [x] 13 kind variants すべてに shape/classDef が定義されている (`test_render_contract_map_emits_13_shape_variants_correctly` が 13 substring pattern を逐一 assert)。spec.md:62 は "snapshot tests が pass する" と記載しているが、実装では `insta` スナップショットではなく inline substring assertion を採用した (実装決定による意図的な deviation)。13 variants の形状が正しく出力されることは同テストで検証済み
- [x] Method call edge (`A -->|method| B`) が描画される (`test_render_contract_map_draws_method_call_edges_across_layers`: 同層 / 層跨ぎを両方検証)
- [x] Trait impl edge (`A -.impl.-> Trait`) が SecondaryAdapter.implements から生成される (`test_render_contract_map_draws_trait_impl_edges_as_dashed`)
- [x] `ContractMapRenderOptions` の 5 フィールド (`layers` / `kind_filter` / `signal_overlay` / `action_overlay` / `include_spec_source_edges`) が public で提供されている (`contract_map_options.rs`)
- [x] `kind_filter` が `Some(vec![])` (全 variant 除外) の場合、subgraph scaffold のみの空 mermaid が返される (error ではない、`test_render_contract_map_kind_filter_empty_vec_returns_empty_subgraphs`)
- [x] Phase 2/3 スタブフィールド (`signal_overlay` / `action_overlay` / `include_spec_source_edges`) は Phase 1 で渡されても出力が変わらない (`test_render_contract_map_phase_2_3_stub_fields_do_not_alter_output`: overlays on/off で出力 byte-equal)
- [x] `ContractMapContent` は libs/domain/src/tddd/contract_map_content.rs に validation-free newtype (`new` / `into_string` / `AsRef<str>`) として追加 (T004 acceptance criterion を先行導入 — render 戻り値型要件ため)
- [x] `ContractMapRenderOptions` も libs/domain/src/tddd/contract_map_options.rs に追加 (T004 acceptance criterion を先行導入 — render 引数型要件ため)
- [x] Layer-agnostic 挙動: hyphen 含む layer id (`my-gateway`) が mermaid node id で `my_gateway_*` に sanitize される (`test_render_contract_map_hyphenated_layer_id_sanitized_in_ids`)
- [x] Determinism: 同一入力で 2 回呼び出しても output が byte-equal (`test_render_contract_map_is_pure_and_deterministic`)
- [x] Library code に panic-prone pattern (unwrap / expect / panic! / unreachable!) なし (domain `extract_type_names` は infrastructure の重複実装、hexagonal 逆依存禁止ゆえ domain 独自実装)
- [x] T003 unit tests 11 件 + ContractMapContent 3 件 + ContractMapRenderOptions 2 件 = 計 16 件追加、nextest 2078 tests all pass

### T004: Domain ports + Infrastructure adapters

- [x] `LayerId` / `ContractMapContent` が `libs/domain/src/tddd/` に newtype (単一フィールド wrapper) として定義されている (生 String を排除) — T002/T003 で先行導入、本 task では `CatalogueLoader::load_all` 戻り値 / `ContractMapWriter::write` 引数で実際に使用
- [x] `ContractMapRenderOptions` が `libs/domain/src/tddd/` に multi-field value_object として定義されている (5 フィールド) — T003 で先行導入済み
- [x] `CatalogueLoaderError` / `ContractMapWriterError` が `libs/domain/src/tddd/catalogue_ports.rs` に error_type (`thiserror::Error`) として定義されている。variant inventory: `CatalogueLoaderError` = CatalogueNotFound / LayerDiscoveryFailed / DecodeFailed / SymlinkRejected / IoError / TopologicalSortFailed (6); `ContractMapWriterError` = IoError / SymlinkRejected / TrackNotFound (3)
- [x] これらの型が `domain-types.json` の宣言と一致している (variants / methods / kinds すべて対応)
- [x] `libs/domain/src/tddd/catalogue_ports.rs` が存在し `CatalogueLoader` / `ContractMapWriter` trait (両方 `Send + Sync`、load_all/write methods) が定義されている
- [x] domain 側の port 定義は serde 非依存である (ADR 2026-04-14-1531 準拠、新規 `use` に `serde::` / `serde_json::` なし、derive に `Serialize`/`Deserialize` なし)
- [x] `libs/infrastructure/src/tddd/contract_map_adapter.rs` が存在し `FsCatalogueLoader` (T002 `load_all_catalogues` をラップ) / `FsContractMapWriter` (atomic_write_file + reject_symlinks_below) が port 契約を実装している
- [x] 書き出しパスが `track_root/<track_id>/contract-map.md` になっている (`FsContractMapWriter::write` + `contract_map_path` helper で検証)
- [x] `atomic_write_file` + `reject_symlinks_below` が書き込み経路で使われている。`FsCatalogueLoader` 側は `track_dir` 自体の symlink を adapter-level で先行 check してから `catalogue_bulk_loader::load_all_catalogues` (内部でも symlink guard) に委譲
- [x] adapter tests 9 件が pass: happy path / missing catalogue → `CatalogueNotFound` / symlinked track_dir → `SymlinkRejected` / writer happy / missing track_dir → `TrackNotFound` / symlinked target → `SymlinkRejected` / overwrite existing non-symlink file / `contract_map_path` helper path formatting / io error on unwritable track_dir (`#[cfg(unix)]` gating on symlink tests)
- [x] nextest 2087 tests all pass、clippy `-D warnings` pass

### T005: usecase interactor

- [x] `libs/usecase/src/contract_map_workflow.rs` が存在し `RenderContractMap` trait が application_service primary port として定義されている (シグネチャ: `execute(&self, &RenderContractMapCommand) -> Result<RenderContractMapOutput, RenderContractMapError>`)
- [x] CLI は concrete `RenderContractMapInteractor` に直接依存せず、`RenderContractMap` trait を介して dispatch する設計になっている (T006 で CLI 側実装、trait bound は `Box<dyn RenderContractMap>` または generic 経由)
- [x] `RenderContractMapInteractor<L: CatalogueLoader, W: ContractMapWriter>` が `RenderContractMap` trait を実装している
- [x] `RenderContractMapCommand { track_id, kind_filter, layer_filter }` / `RenderContractMapOutput { rendered_layer_count, total_entry_count }` / `RenderContractMapError (4 variants: CatalogueLoaderFailed/ContractMapWriterFailed/EmptyCatalogue/LayerNotFound)` が定義されている
- [x] execute のフロー (loader → empty check → layer_filter validation → options build → render → writer → metrics) が正しく実装されている
- [x] `EmptyCatalogue` は `loader.load_all` が空の catalogue set (Vec<LayerId> is_empty) を返した場合に発火 — `test_execute_empty_catalogue_returns_empty_catalogue_error` で検証
- [x] `LayerNotFound` は `layer_filter` に指定した `LayerId` が `load_all` の結果に存在しない場合に発火 — `test_execute_layer_filter_with_unknown_layer_returns_layer_not_found` で検証
- [x] mockall を使った 7 tests (happy / loader_err / writer_err / empty_catalogue / layer_not_found / kind_filter / kind_filter_empty) が pass。mockall は本 track で workspace dependency として追加 (`Cargo.toml [workspace.dependencies] mockall = "0.13"` + `libs/usecase/Cargo.toml dev-dependencies`)。テンプレート全体で推奨される testing crate を導入する機会として扱う
- [x] usecase 層の純粋性を保つ (`std::fs` / `chrono::Utc::now` / `println!` / `std::env` 未使用)
- [x] nextest 2094 tests all pass、clippy `-D warnings` pass
- [x] `cargo make ci` は commit 時に `track-commit-message` ラッパーが自動実行し、`verify-usecase-purity` を含む全 verifier を走らせる

### T006: CLI subcommand

- [x] `apps/cli/src/commands/track/tddd/contract_map.rs` が存在する
- [x] `sotp track contract-map <track-id> [--kind-filter k1,k2] [--layers l1,l2] [--items-dir PATH] [--workspace-root PATH]` が動作する (clap `ContractMap { items_dir, track_id, workspace_root, kind_filter, layers }` variant + dispatch)
- [x] `apps/cli/src/commands/track/tddd/mod.rs` (pub(crate) mod contract_map) および `apps/cli/src/commands/track/mod.rs` (TrackCommand::ContractMap variant + execute dispatch) にサブコマンドが登録されている
- [x] 既存 type-graph の動作に影響なし (type-graph tests 全て pass、nextest 2108 tests all green)
- [x] CLI は composition root で `RenderContractMapInteractor` を構築し、`RenderContractMap` primary port trait 経由で dispatch する (`let renderer: &dyn RenderContractMap = &interactor;`)
- [x] 13 kind_tag + layer id パース: `parse_kind_filter` 7 tests (round-trip / trim / case-insensitive / unknown-rejection / empty) / `parse_layer_filter` 4 tests (single / multiple / trim / invalid) で検証
- [x] Active-track guard (`ensure_active_track`) + track metadata 読み出し (`read_track_metadata`) を type-graph / type-signals と同じ順序で実行
- [x] clippy `-D warnings` pass、nextest 2108 tests all pass (T006 新規 14 tests 追加)

### T007: Layer-agnostic fixture tests

- [x] `libs/infrastructure/tests/fixtures/architecture_rules/fixture_2layers/` が存在する (core / adapter、`architecture-rules.json` + `track_dir/core-types.json` + `track_dir/adapter-types.json`)
- [x] `libs/infrastructure/tests/fixtures/architecture_rules/fixture_3layers_default/` が存在する (domain / usecase / infrastructure)
- [x] `libs/infrastructure/tests/fixtures/architecture_rules/fixture_custom_names/` が存在する (application / port / gateway、独自命名 3 層)
- [x] 各 fixture に対し subgraph 数が `tddd.enabled` 層数と一致する (`test_fixture_*_emits_subgraph_per_enabled_layer`: `out.matches("subgraph ").count()` を assert)
- [x] 各 fixture の subgraph ラベルが fixture の `layers[].crate` と一致する (`subgraph_position` helper が `subgraph <crate> [<crate>]` 形式を検索)
- [x] subgraph の出現順序が fixture の `may_depend_on` トポロジカル順 (no-deps 層が先頭) に一致する (`test_fixture_*_respects_may_depend_on_topological_order`: `find` の byte offset を比較)
- [x] 他 fixture の層名が出力に混入しないこと (layer-agnostic 違反検出) が `assert_no_foreign_layers` で検証される (例: `fixture_2layers` の output に `domain` / `usecase` / `infrastructure` / `application` / `port` / `gateway` のいずれも subgraph として出現しないこと)
- [x] 9 integration tests (3 fixture × 3 観点) が pass — spec 要求 (3 fixture × 基本 render 最低 6 test) を上回る粒度
- [x] fixtures は実 JSON ファイル (inline 文字列ではない) として `libs/infrastructure/tests/fixtures/architecture_rules/<fixture>/` 配下に配置、`load_all_catalogues` + `render_contract_map` で実経路を検証

## Scope Verified (再掲、全 task 完了時点)

- [x] In-scope items match ADR 2026-04-17-1528 Phase 1 (MVP — method + trait impl edge, layer-agnostic)
- [x] Out-of-scope items correctly deferred (action overlay / signal overlay / spec_source edge / baseline diff / AI briefing auto-injection / type-graph usecase refactor)
- [x] layer-agnostic 不変条件 (ADR §4.5) が 3 fixture で機械的に検証されている (T007 の 9 integration tests)

## Manual Verification Steps

```bash
# 1. CI 全通過
cargo make ci

# 2. Contract Map 生成 (現 track 自身に対して実行、ドッグフーディング)
cargo run --quiet -p cli -- track contract-map tddd-contract-map-phase1-2026-04-17

# 3. 生成物確認
ls track/items/tddd-contract-map-phase1-2026-04-17/contract-map.md
cat track/items/tddd-contract-map-phase1-2026-04-17/contract-map.md

# 4. mermaid のレンダリング確認 (GitHub / VS Code で開いて視覚確認)

# 5. kind-filter 動作確認
cargo run --quiet -p cli -- track contract-map tddd-contract-map-phase1-2026-04-17 --kind-filter secondary_port,use_case

# 6. layer filter 動作確認
cargo run --quiet -p cli -- track contract-map tddd-contract-map-phase1-2026-04-17 --layers domain,usecase

# 7. 既存 type-graph に影響なし確認
cargo run --quiet -p cli -- track type-graph tddd-contract-map-phase1-2026-04-17 --cluster-depth 2 --edges all
```

## Result

| Task | Status | Commit | Notes |
|------|--------|--------|-------|
| T001 | Done | `9a69df07bd5d98adb3c8d9de935e9527010e9758` | ADR 2026-04-16-2200 §D10 新設 + ADR 2026-04-17-1528 §Q6 Resolved 注記。§D3 表は計画時点で既に整合済みだった。verify-arch-docs / verify-doc-links pass |
| T002 | Done (in_progress until batch transition) | `7741e7a1a673fcc0ff640415b80103b46bf15d52` | `libs/domain/src/tddd/layer_id.rs` (LayerId VO — 当初 nutype、T007 で素 struct に変更) + `libs/infrastructure/src/tddd/catalogue_bulk_loader.rs` (load_all_catalogues + topological_sort + parse_may_depend_on) + `extract_type_names` pub(crate) 昇格。clippy / nextest (2062 tests) pass。state は batch flow (task-completion-flow.md 正式フロー) 準拠で T007 後に一括 done 遷移予定 |
| T003 | Done (in_progress until batch transition) | `e65f8f30c849ad981c53ff61d4ac397188095031` | `libs/domain/src/tddd/contract_map_render.rs` (pure function, 13 kind shape + method/trait edge) + `contract_map_content.rs` (validation-free newtype) + `contract_map_options.rs` (5-field options with 3 Phase 2/3 stubs)。T004 acceptance criterion 2 件 (`ContractMapContent`/`ContractMapRenderOptions`) を戻り値・引数の型要件上先行導入。clippy / nextest (2078 tests) pass |
| T004 | Done (in_progress until batch transition) | `85823b9bfdba42b3d1f47006718b7e024303a8b5` | `libs/domain/src/tddd/catalogue_ports.rs` (CatalogueLoader / ContractMapWriter traits + 2 error enums) + `libs/infrastructure/src/tddd/contract_map_adapter.rs` (FsCatalogueLoader wraps `load_all_catalogues`, FsContractMapWriter uses atomic_write_file + reject_symlinks_below) + 9 adapter tests。nextest 2087 tests all pass |
| T005 | Done (in_progress until batch transition) | `0a3a540a318f47e9a0b14e6c8be976bf43460552` | `libs/usecase/src/contract_map_workflow.rs` 新設: `RenderContractMap` trait (primary port) + `RenderContractMapInteractor<L, W>` + `RenderContractMapCommand` / `RenderContractMapOutput` / `RenderContractMapError` (4 variants) + inline metrics 計算 + 7 mockall tests。mockall を workspace dependency として追加 (Cargo.toml + libs/usecase/Cargo.toml)。nextest 2094 tests all pass |
| T006 | Done (in_progress until batch transition) | `c0c97370cc4cb1b3082449d6df9e42018dd9c037` | `apps/cli/src/commands/track/tddd/contract_map.rs` (execute_contract_map + parse_kind_filter + parse_layer_filter + 14 tests) + `tddd/mod.rs` (pub(crate) mod 登録) + `track/mod.rs` (TrackCommand::ContractMap variant + dispatch)。CLI は composition root で concrete interactor を構築し、`&dyn RenderContractMap` trait object 経由で dispatch。nextest 2108 tests all pass |
| T007 | Implemented (pre-commit, in_progress) |  | `libs/infrastructure/tests/fixtures/architecture_rules/{fixture_2layers,fixture_3layers_default,fixture_custom_names}/` (architecture-rules.json + track_dir/*-types.json) + `libs/infrastructure/tests/contract_map_layer_agnostic.rs` (9 integration tests: subgraph count / topo order / foreign-layer non-leak × 3 fixtures)。layer-agnostic 不変条件を機械的に検証。**追加変更**: LayerId を nutype → 素 struct 書き換え (nutype + rustdoc 検出制約の回避、別 track 計画 TODO 記録)、3 layer の signals 全 Blue 化、contract-map.md dogfood 生成。nextest 2117 tests all pass |

## Open Issues

- (実装中に発覚した論点はここに追記)

## Phase 1 Scope Acknowledgement

本 track は ADR 2026-04-17-1528 の **Phase 1 (MVP) のみ** を扱う。以下は Phase 2/3 として意図的に延期:

- Phase 2: action overlay / signal overlay (ADR §D5)
- Phase 3: spec_source edge / baseline diff view / AI briefing 自動添付 (ADR §Implementation Phases Phase 3)

また、既存 type-graph CLI の usecase 層介在へのリファクタは scope 爆発防止のため別 track 扱いとする (本 track の out_of_scope に明記)。

## Verified At

2026-04-17 (Phase 1 MVP 完了)

- 全 7 tasks 実装完了、T001-T006 は commit 確定 (hash 記録済み)、T007 は本 commit 確定予定
- nextest: 2117 tests all pass
- clippy `-D warnings` pass
- ADR 2026-04-17-1528 §D1 / §D3 / §D4 / §4.5 の Phase 1 MVP スコープを完全実装
- Phase 2/3 (action overlay / signal overlay / spec_source edge / baseline diff / AI briefing 自動添付) は spec 通り out_of_scope で延期

### Stage 2 type-signals (dogfooding)

| Layer | Declared | Blue | Yellow | Red | Undeclared |
|---|---|---|---|---|---|
| domain | 8 (incl. ValidationError modify) | 8 | 0 | 0 | 0 |
| usecase | 5 | 5 | 0 | 0 | 0 |
| infrastructure | 3 (incl. LoadAllCataloguesError) | 3 | 0 | 0 | 0 |

**所感**: 全 16 entries Blue、Stage 2 merge gate 通過条件を満たす。Stage 1 spec signals の yellow 解消 + `TypeCatalogueDocument` reference 追加による追加 edge 可視化は本 track の別 commit で対応。

### Contract Map dogfooding (`sotp track contract-map tddd-contract-map-phase1-2026-04-17`)

生成先: `track/items/tddd-contract-map-phase1-2026-04-17/contract-map.md`

- 3 subgraphs (domain / usecase / infrastructure) が `may_depend_on` トポロジカル順で配置
- 16 entries が 8 kind shape (value_object / secondary_port / error_type / application_service / command / dto / interactor / secondary_adapter) で描画
- Method edges: `CatalogueLoader -->|load_all| LayerId`, `CatalogueLoader -->|load_all| CatalogueLoaderError`, `ContractMapWriter -->|write| ContractMapWriterError`, `RenderContractMap -->|execute| RenderContractMapError`, `RenderContractMap -->|execute| RenderContractMapOutput`
- Trait impl edges (dashed): `FsCatalogueLoader -.impl.-> CatalogueLoader`, `FsContractMapWriter -.impl.-> ContractMapWriter`

### Implementation deviation: LayerId を素 struct に書き換え

本 track 進行中、`sotp track type-signals` 評価で `LayerId` が `found_type=false` (yellow) となる現象を検出。調査の結果、`schema_export` が `nutype` 生成型 (`#[doc(hidden)]` module 内の struct + `pub use` alias) を rustdoc JSON から抽出できない既知制約と判明:

- 同様に検出漏れする既存 nutype 型: `TrackId` / `TaskId` / `CommitHash` / `TrackBranch` / `NonEmptyString` / `ReviewGroupName` (declared なし、undeclared red にもならない = rustdoc 非露出)
- 本 track で declare した `LayerId` のみ yellow として顕在化

scope 最小化のため、本 track では `libs/domain/src/tddd/layer_id.rs` を `#[nutype]` から**素 struct `pub struct LayerId(String)` に書き換え**、同等の validation + `try_new` API を保つ。同じ `Result<Self, ValidationError>` シグネチャで上流コードへの影響なし。domain-types.json の signal は blue に遷移、nextest 2117 tests all pass。

harness 側の根本修理 (`schema_export` に `Item::Use` alias resolution を追加し、既存 6 nutype 型を catalogue 宣言できるようにする) は別 track として `knowledge/strategy/TODO.md` の **harness-hardening-nutype-rustdoc-support (HIGH)** に記録。
