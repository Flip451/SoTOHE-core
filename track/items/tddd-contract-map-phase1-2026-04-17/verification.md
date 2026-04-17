# Verification: tddd-contract-map-phase1-2026-04-17

## Scope Verified

- [ ] In-scope items match ADR 2026-04-17-1528 Phase 1 (MVP — method + trait impl edge, layer-agnostic)
- [ ] Out-of-scope items correctly deferred (action overlay / signal overlay / spec_source edge / baseline diff / AI briefing auto-injection / type-graph usecase refactor)
- [ ] layer-agnostic 不変条件 (ADR §4.5) が 3 fixture で機械的に検証されている

## Task Verification

### T001: ADR 整合性訂正

- [ ] ADR 2026-04-17-1528 §D3 shape mapping 表に SecondaryAdapter 行が追加されている
- [ ] ADR 2026-04-17-1528 本文の \"12 variants\" が \"13 variants\" に訂正されている
- [ ] ADR 2026-04-16-2200 に §D10 \"Reality View as drill-down to Contract Map\" が新設されている (D7 は既存の「段階的実装」節が使用済みのため D10 を使用)
- [ ] cargo make ci が通る (verify-doc-links / verify-arch-docs を含む — spec.json 最終 acceptance criterion に対応)

### T002: Infrastructure helpers

- [ ] `libs/infrastructure/src/tddd/catalogue_bulk_loader.rs` が存在し `load_all_catalogues` 関数が提供されている
- [ ] 不在カタログに対して明示エラー variant が返される
- [ ] symlink injection が fail-closed で拒否される (test で確認)
- [ ] `extract_type_names` が pub 昇格されている
- [ ] layer_order が `may_depend_on` のトポロジカル順 (依存なし層が左端) であることが assert される
- [ ] unit tests (正常 load / 不在エラー / symlink 拒否 / topo_sort / visibility) が pass

### T003: Contract Map pure render module

- [ ] `libs/domain/src/tddd/contract_map_render.rs` が存在し `render_contract_map` が pure function (I/O なし) として domain 内に実装されている
- [ ] 13 kind variants すべてに shape/classDef が定義されている (snapshot test で確認)
- [ ] Method call edge (`A -->|method| B`) が描画される
- [ ] Trait impl edge (`A -.impl.-> Trait`) が SecondaryAdapter.implements から生成される
- [ ] `ContractMapRenderOptions` の 5 フィールドが public で提供されている
- [ ] `kind_filter` が全 variant を除外した場合、空 subgraph の mermaid が返される (error ではない)
- [ ] Phase 2/3 スタブフィールド (signal_overlay / action_overlay / include_spec_source_edges) は Phase 1 で渡されても出力が変わらない

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
| T001 | Pending |  |  |
| T002 | Pending |  |  |
| T003 | Pending |  |  |
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
