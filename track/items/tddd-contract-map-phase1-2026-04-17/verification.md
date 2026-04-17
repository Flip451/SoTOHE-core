# Verification: tddd-contract-map-phase1-2026-04-17

## Scope Verified

- [ ] In-scope items match ADR 2026-04-17-1528 Phase 1 (MVP — method + trait impl edge, layer-agnostic)
- [ ] Out-of-scope items correctly deferred (action overlay / signal overlay / spec_source edge / baseline diff / AI briefing auto-injection / type-graph usecase refactor)
- [ ] layer-agnostic 不変条件 (ADR §4.5) が 3 fixture で機械的に検証されている

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
- [x] `LayerId` nutype VO が `libs/domain/src/tddd/layer_id.rs` に追加された (T004 acceptance criterion の一部を本 task で先行導入 — `Vec<LayerId>` 戻り値の型要件ため。残りの VOs (`ContractMapContent` / `ContractMapRenderOptions`) + port traits は T004 で追加)
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

- [ ] `LayerId` / `ContractMapContent` が `libs/domain/src/tddd/` に newtype (単一フィールド wrapper) として定義されている (生 String を排除)
- [ ] `ContractMapRenderOptions` が `libs/domain/src/tddd/` に multi-field value_object として定義されている (5 フィールド: layers / kind_filter / signal_overlay / action_overlay / include_spec_source_edges)
- [ ] `CatalogueLoaderError` / `ContractMapWriterError` が `libs/domain/src/tddd/` に error_type として定義されている
- [ ] これらの型が `domain-types.json` の宣言と一致している
- [ ] `libs/domain/src/tddd/catalogue_ports.rs` が存在し `CatalogueLoader` / `ContractMapWriter` trait が定義されている
- [ ] domain 側の port 定義は serde 非依存である (ADR 2026-04-14-1531 準拠)
- [ ] `libs/infrastructure/src/tddd/contract_map_adapter.rs` が存在し `FsCatalogueLoader` / `FsContractMapWriter` が port 契約を実装している
- [ ] 書き出しパスが `track_dir/contract-map.md` になっている
- [ ] atomic_write_file + reject_symlinks_below が全ての write パスで使われている
- [ ] adapter tests (port 契約充足 / atomic 書き出し / symlink 拒否) が pass

### T005: usecase interactor

- [ ] `libs/usecase/src/contract_map_workflow.rs` が存在し `RenderContractMap` trait が application_service primary port として定義されている (シグネチャ: `execute(&self, &RenderContractMapCommand) -> Result<RenderContractMapOutput, RenderContractMapError>`)
- [ ] CLI は concrete `RenderContractMapInteractor` に直接依存せず、`RenderContractMap` trait を介して dispatch する
- [ ] `RenderContractMapInteractor` が `RenderContractMap` trait を実装している
- [ ] `RenderContractMapCommand` / `RenderContractMapOutput` / `RenderContractMapError` が定義されている
- [ ] execute のフロー (loader → filter → render → writer) が正しく実装されている
- [ ] `EmptyCatalogue` は `loader.load_all` が空の catalogue set を返した場合 (tddd.enabled 層が 0 件) に発火することが unit test で確認されている
- [ ] `LayerNotFound` は `layer_filter` に指定した `LayerId` が `load_all` の結果に存在しない場合に発火することが unit test で確認されている
- [ ] mockall を使った 5 tests (happy / loader_err / writer_err / kind_filter 動作 / 全除外時 empty mermaid) が pass
- [ ] cargo make ci が通る (verify-usecase-purity を含む — spec.json 最終 acceptance criterion に対応)

### T006: CLI subcommand

- [ ] `apps/cli/src/commands/track/tddd/contract_map.rs` が存在する
- [ ] `sotp track contract-map <track-id> [--kind-filter k1,k2] [--layers l1,l2]` が動作する
- [ ] `apps/cli/src/commands/track/tddd/mod.rs` および `apps/cli/src/commands/track/mod.rs` にサブコマンドが登録されている
- [ ] 既存 type-graph の動作に影響がない (既存 tests が pass)
- [ ] --help 出力が spec / ADR と一致する

### T007: Layer-agnostic fixture tests

- [ ] `libs/infrastructure/tests/fixtures/architecture_rules/fixture_2layers.json` が存在する (core / adapter)
- [ ] `libs/infrastructure/tests/fixtures/architecture_rules/fixture_3layers_default.json` が存在する (domain / usecase / infrastructure)
- [ ] `libs/infrastructure/tests/fixtures/architecture_rules/fixture_custom_names.json` が存在する (application / port / gateway)
- [ ] 各 fixture に対し subgraph 数が `tddd.enabled` 層数と一致する
- [ ] 各 fixture の subgraph ラベルが fixture の `layers[].crate` と一致する
- [ ] subgraph の出現順序が fixture の may_depend_on から算出したトポロジカル順 (may_depend_on なし層が先頭) に一致することが assert される
- [ ] 他 fixture の層名が出力に混入しないこと (layer-agnostic 違反検出) が assert される
- [ ] 3 fixture × 基本 render 検証 = 最低 6 test が pass

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
| T002 | Done (in_progress until batch transition) | `7741e7a1a673fcc0ff640415b80103b46bf15d52` | `libs/domain/src/tddd/layer_id.rs` (LayerId nutype) + `libs/infrastructure/src/tddd/catalogue_bulk_loader.rs` (load_all_catalogues + topological_sort + parse_may_depend_on) + `extract_type_names` pub(crate) 昇格。clippy / nextest (2062 tests) pass。state は batch flow (task-completion-flow.md 正式フロー) 準拠で T007 後に一括 done 遷移予定 |
| T003 | Implemented (pre-commit, in_progress) |  | `libs/domain/src/tddd/contract_map_render.rs` (pure function, 13 kind shape + method/trait edge) + `contract_map_content.rs` (validation-free newtype) + `contract_map_options.rs` (5-field options with 3 Phase 2/3 stubs)。T004 acceptance criterion 2 件 (`ContractMapContent`/`ContractMapRenderOptions`) を戻り値・引数の型要件上先行導入。clippy / nextest (2078 tests) pass |
| T004 | Pending |  |  |
| T005 | Pending |  |  |
| T006 | Pending |  |  |
| T007 | Pending |  |  |

## Open Issues

- (実装中に発覚した論点はここに追記)

## Phase 1 Scope Acknowledgement

本 track は ADR 2026-04-17-1528 の **Phase 1 (MVP) のみ** を扱う。以下は Phase 2/3 として意図的に延期:

- Phase 2: action overlay / signal overlay (ADR §D5)
- Phase 3: spec_source edge / baseline diff view / AI briefing 自動添付 (ADR §Implementation Phases Phase 3)

また、既存 type-graph CLI の usecase 層介在へのリファクタは scope 爆発防止のため別 track 扱いとする (本 track の out_of_scope に明記)。

## Verified At

(T001–T007 完了後に日付記入)
