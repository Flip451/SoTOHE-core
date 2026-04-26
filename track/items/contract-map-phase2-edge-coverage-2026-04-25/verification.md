# Verification — contract-map-phase2-edge-coverage-2026-04-25 (T010 dogfood)

**Date:** 2026-04-26
**Track:** `contract-map-phase2-edge-coverage-2026-04-25`
**Verification target:** ADR `knowledge/adr/2026-04-17-1528-tddd-contract-map.md` § Known Limitations の L1-L4 全 4 カテゴリが本トラックの dogfood で解消されたことの記録。

## Procedure

1. T001-T009 の commit を経た現状で、本トラック自身の catalogue を Phase 2 schema に update:
   - `domain-types.json::render_contract_map`: kind を `value_object` placeholder → `free_function`、`expected_params` (catalogues / layer_order / opts) + `expected_returns` (ContractMapContent) を declare
   - `domain-types.json::ContractMapRenderOptions`: action を `reference` → `modify` + `expected_members` (5 fields: `layers` / `kind_filter` / `signal_overlay` / `action_overlay` / `include_spec_source_edges`) を declare → field edge 描画の対象に
   - `infrastructure-types.json::load_all_catalogues`: 新規 entry 追加 (kind=`free_function`, action=`reference`, expected_returns=[`LoadAllCataloguesError`]) → L4 edge source の宣言基盤
2. `bin/sotp track type-signals contract-map-phase2-edge-coverage-2026-04-25` + `bin/sotp track catalogue-spec-signals contract-map-phase2-edge-coverage-2026-04-25` で signal を refresh
3. `bin/sotp track contract-map contract-map-phase2-edge-coverage-2026-04-25` で `contract-map.md` を再 render
4. 出力された `contract-map.md` を grep して、9 isolated nodes (Phase 1.5 dogfood 時点) の解消状況を category 別に確認

## Result — Phase 1.5 isolated nodes (合計 9 件) の解消

### L1: forward-reference placeholders (5 件)

`action=reference` で edge source / target いずれにも参加しない型 → T005 の `unused_reference` classDef で dashed border 視覚識別。

| node | rendered class application |
|---|---|
| `TaskId` | `class L6_domain_TaskId unused_reference` |
| `CommitHash` | `class L6_domain_CommitHash unused_reference` |
| `TrackBranch` | `class L6_domain_TrackBranch unused_reference` |
| `NonEmptyString` | `class L6_domain_NonEmptyString unused_reference` |
| `ReviewGroupName` | `class L6_domain_ReviewGroupName unused_reference` |

→ 5 件すべて意図的な declaration として視覚識別済み。✓

### L2: declaration-only / field 参照 (2 件)

#### L2a: `ValidationError` (action=modify、 expected_methods 空)

T005 の `declaration_only` classDef で dashed border 視覚識別。

```
class L6_domain_ValidationError declaration_only
```

→ 解消済み。✓

#### L2b: `ContractMapRenderOptions` (field 参照)

T006 の field edge 実装 + T010 dogfood で `expected_members` declare → field edges が本構造体から referenced 型に伸びる + declaration_only でも視覚識別。

```
L6_domain_ContractMapRenderOptions -->|".kind_filter"| L6_domain_TypeDefinitionKind
L6_domain_ContractMapRenderOptions -->|".layers"| L6_domain_LayerId
class L6_domain_ContractMapRenderOptions declaration_only
```

→ field edges 2 本描画 + dashed border 視覚識別済み。bool / Option / Vec の external types は CN-05 通り silently skipped。✓

### L3: Interactor → ApplicationService 実装関係 (1 件)

T003 の `Interactor.declares_application_service` field 拡張 + T004 codec 経由 + T010 dogfood で usecase-types.json `RenderContractMapInteractor` entry に `declares_application_service: "RenderContractMap"` 設定 (T003+T004 commit 時点で完了) → Contract Map renderer が `-.impl.->` edge を描画。

```
L7_usecase_RenderContractMapInteractor -.impl.-> L7_usecase_RenderContractMap
```

→ impl edge 描画済み。✓

### L4: free function の戻り値型 (1 件)

T001 の `TypeDefinitionKind::FreeFunction` variant 追加 + T002 codec 経由 + T010 dogfood で:
- (i) `LoadAllCataloguesError` (action=reference, no edges) → T005 の `unused_reference` dashed border で視覚識別
- (ii) `load_all_catalogues` を `infrastructure-types.json` に `kind=free_function` + `expected_returns=[LoadAllCataloguesError]` で declare → Phase 3 で return-type edge rendering を追加すれば自動的に edge が描画される基盤を確立
- (iii) `render_contract_map` (renderer 自身) も `domain-types.json` で `kind=free_function` + `expected_params` + `expected_returns` で declare → 同様に Phase 3 で edge rendering 基盤

```
class L14_infrastructure_LoadAllCataloguesError unused_reference
L14_infrastructure_load__all__catalogues[load_all_catalogues]:::free_function
class L14_infrastructure_load__all__catalogues unused_reference
L6_domain_render__contract__map[render_contract_map]:::free_function
class L6_domain_render__contract__map unused_reference
```

→ (i) 視覚識別 + (ii)(iii) FreeFunction kind nodes として描画 + edge source 宣言基盤確立。✓ (Phase 3 で param/return-type edge rendering を追加することで完全 edge 描画になる)

## Side-effects (Phase 2 で意図的に追加した型の dashed border 視覚識別)

T001-T009 で本 track が catalogue に追加した以下の型は、Phase 2 schema を体現する宣言として `declaration_only` / `unused_reference` で視覚識別される (期待挙動):

```
class L6_domain_MemberDeclaration unused_reference
class L6_domain_TypeCatalogueEntry declaration_only
class L6_domain_TypeDefinitionKind declaration_only
class L7_usecase_RenderContractMapInteractor declaration_only
class L14_infrastructure_TypeCatalogueCodecError declaration_only
```

これらは本トラックの「catalogue schema を Phase 2 化した」事実の structural な visual evidence。Phase 1 当時の 9 isolated nodes とは独立。

## Final Contract Map metrics

```
sotp track contract-map contract-map-phase2-edge-coverage-2026-04-25
[OK] contract-map: wrote .../contract-map.md (layers=3, entries=29)
```

| metric | Phase 1.5 | Phase 2 (T010 dogfood) |
|---|---|---|
| layers | 3 | 3 |
| entries | 23 | 29 |
| isolated nodes (no edge / no dashed border) | 9 | 0 |

## Conclusion

ADR `2026-04-17-1528-tddd-contract-map.md` § Known Limitations の **L1-L4 全 4 カテゴリ (Phase 1.5 当時の 9 isolated nodes)** が、Phase 2 の以下の機能組み合わせで完全に対処された:

- **edge 描画**: L3 (Interactor impl edge) + L2b (field edge)
- **dashed border 視覚識別**: L1 (unused_reference) + L2a (declaration_only) + L4 (i) (LoadAllCataloguesError)
- **edge source 宣言基盤**: L4 (ii)(iii) (load_all_catalogues / render_contract_map FreeFunction kind nodes)

L4 の完全 edge 描画 (return-type edge rendering) のみ Phase 3 のスコープに残るが、本トラックの目的「Phase 1.5 で残った 9 isolated nodes を edge coverage で解消する」は達成済み。
