<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: draft
version: "1.0"
signals: { blue: 36, yellow: 0, red: 0 }
---

# domain-types.json — typed domain type registry (spec.json から分離)

## Goal

spec.json から domain_states を切り出し、独立ファイル domain-types.json として新設する。
DomainTypeKind enum で型カテゴリ (typestate/enum/value_object/error_type/trait_port) を表現し、各カテゴリに固有の検証データを持たせる。
信号評価を Blue/Red 2値に厳格化し、spec と code の完全一致のみを Blue とする。
spec (要件) と domaintypes (型宣言) のライフサイクルを分離し、それぞれ独立に更新・凍結可能にする。

## Scope

### In Scope
- DomainTypeKind enum 定義: Typestate/Enum/ValueObject/ErrorType/TraitPort の5カテゴリ [source: knowledge/adr/2026-04-07-0045-domain-types-separation.md §Decision.2] [tasks: T001]
- DomainTypeEntry: name + description + kind [source: knowledge/adr/2026-04-07-0045-domain-types-separation.md §Decision.2] [tasks: T001]
- DomainTypeSignal: Blue/Red 2値 + found/missing/extra_items [source: knowledge/adr/2026-04-07-0045-domain-types-separation.md §Decision.3] [tasks: T001]
- DomainTypesDocument: domain-types.json の domain 表現 (schema_version + entries + signals) [source: knowledge/adr/2026-04-07-0045-domain-types-separation.md §Decision.1] [tasks: T002]
- evaluate_domain_type_signals(): kind ごとの評価ロジック。CodeScanResult + Optional SchemaExport [source: knowledge/adr/2026-04-07-0045-domain-types-separation.md §Decision.3] [tasks: T003]
- SpecDocument から domain_states / domain_state_signals を削除 [source: knowledge/adr/2026-04-07-0045-domain-types-separation.md §Decision.1] [tasks: T004]
- domain-types.json 用 codec 新設: DomainTypeKindDto + schema_version 1 [source: knowledge/adr/2026-04-07-0045-domain-types-separation.md §Decision.1] [tasks: T005]
- domain-types.md renderer 新設: Domain Types テーブル + kind ごとの Details 列 [source: knowledge/adr/2026-04-07-0045-domain-types-separation.md §Decision.1] [tasks: T006]
- spec.json codec / renderer から domain_states 関連を削除 [source: knowledge/adr/2026-04-07-0045-domain-types-separation.md §Decision.1] [tasks: T007, T008]
- verify: domain-types.json 読み込み + Blue/Red ゲート [source: knowledge/adr/2026-04-07-0045-domain-types-separation.md §Decision.3] [tasks: T009]
- CLI: domain-type-signals コマンド + views sync に domain-types.md 追加 [source: knowledge/adr/2026-04-07-0045-domain-types-separation.md §Decision.1] [tasks: T010, T011]
- 既存 track の spec.json から domain-types.json へのマイグレーション [source: knowledge/adr/2026-04-07-0045-domain-types-separation.md §Consequences] [tasks: T012]
- DESIGN.md + ADR 更新 [source: knowledge/adr/2026-04-07-0045-domain-types-separation.md] [tasks: T013]

### Out of Scope
- spec.json schema_version 変更 (domain_states 削除のみ、spec スキーマ自体の v2 化は不要) [source: knowledge/adr/2026-04-07-0045-domain-types-separation.md §Decision.1]
- spec ↔ code 双方向整合性チェック CLI コマンド (3-12 本体, 次 track) [source: knowledge/adr/2026-03-23-2130-spec-code-consistency-deferred.md]
- 信号自動降格ループ (SPEC-01, Phase 3) [source: knowledge/strategy/TODO-PLAN.md §Phase3 3-9]

## Constraints
- domain 層は I/O を含まない (hexagonal purity) [source: convention — knowledge/conventions/hexagonal-architecture.md]
- DomainTypeKind は enum-first パターンで設計 (variant ごとに異なるデータ) [source: convention — .claude/rules/04-coding-principles.md, knowledge/adr/2026-04-07-0045-domain-types-separation.md §Decision.2]
- 信号は Blue/Red 2値。Yellow は domain-types の評価で使用しない [source: knowledge/adr/2026-04-07-0045-domain-types-separation.md §Decision.3]
- domain-types.json は track ごとに独立ファイル (spec.json と同じディレクトリ) [source: knowledge/adr/2026-04-07-0045-domain-types-separation.md §Decision.1]
- SchemaExport は optional 入力。nightly 未インストール環境では CodeScanResult のみで部分検証 [source: knowledge/adr/2026-04-07-0045-domain-types-separation.md §Context, track/tech-stack.md §Dev-only Tooling]
- transitions_to 参照整合性チェックは Typestate kind のみに適用 [source: knowledge/adr/2026-04-07-0045-domain-types-separation.md §Decision.2]

## Domain States

| State | Description |
|-------|-------------|
| DomainTypeKind | 型カテゴリ enum: Typestate{transitions_to} / Enum{expected_variants} / ValueObject / ErrorType{expected_variants} / TraitPort{expected_methods} |
| DomainTypeEntry | domain-types.json の各型宣言エントリ: name + description + kind (DomainTypeKind) + approved (bool, default true) |
| DomainTypesDocument | domain-types.json の domain 表現: schema_version + entries (Vec<DomainTypeEntry>) + signals (Option<Vec<DomainTypeSignal>>) |
| CodeProfile | crate の pub API を pre-indexed した評価用ビュー: types (HashMap<String, CodeType>) + traits (HashMap<String, CodeTrait>)。Infrastructure 層が SchemaExport から構築 |
| CodeType | pub 型の pre-indexed 情報: kind (TypeKind) + members (Vec<String>) + method_return_types (HashSet<String>) |
| CodeTrait | pub trait の pre-indexed 情報: method_names (Vec<String>) |
| TypestateTransitions | Typestate の遷移宣言: Terminal (終端) / To(Vec<String>) (遷移先リスト) |
| DomainTypeSignal | per-type 信号評価結果: type_name + kind_tag + signal (Blue/Red) + found_type + found_items + missing_items + extra_items |

## Acceptance Criteria
- [ ] DomainTypeKind が 5 variant (Typestate/Enum/ValueObject/ErrorType/TraitPort) を持ち、各 variant が固有データのみを保持する [source: knowledge/adr/2026-04-07-0045-domain-types-separation.md §Decision.2] [tasks: T001]
- [ ] Typestate: 型存在 + transitions_to 全遷移発見 → Blue, それ以外 → Red [source: knowledge/adr/2026-04-07-0045-domain-types-separation.md §Decision.3] [tasks: T003]
- [ ] Enum: 型存在 + expected_variants 完全一致 → Blue, 過不足あり → Red [source: knowledge/adr/2026-04-07-0045-domain-types-separation.md §Decision.3] [tasks: T003]
- [ ] ValueObject: 型存在 → Blue, 型なし → Red [source: knowledge/adr/2026-04-07-0045-domain-types-separation.md §Decision.3] [tasks: T003]
- [ ] ErrorType: 型存在 + expected_variants 全カバー → Blue, variant 不足 → Red [source: knowledge/adr/2026-04-07-0045-domain-types-separation.md §Decision.3] [tasks: T003]
- [ ] TraitPort: trait 存在 + expected_methods 全発見 → Blue, メソッド不足 → Red [source: knowledge/adr/2026-04-07-0045-domain-types-separation.md §Decision.3] [tasks: T003]
- [ ] domain-types.json が独立ファイルとしてエンコード/デコードされる (spec.json とは別) [source: knowledge/adr/2026-04-07-0045-domain-types-separation.md §Decision.1] [tasks: T005]
- [ ] DomainTypeKind の serde tag (kind フィールド) で各 variant が正しく JSON 直列化される [source: knowledge/adr/2026-04-07-0045-domain-types-separation.md §Decision.2] [tasks: T005]
- [ ] domain-types.md に Domain Types テーブル + Kind + Details + Signal 列が表示される [source: knowledge/adr/2026-04-07-0045-domain-types-separation.md §Decision.1] [tasks: T006]
- [ ] spec.json から domain_states / domain_state_signals が完全に除去されている [source: knowledge/adr/2026-04-07-0045-domain-types-separation.md §Decision.1] [tasks: T004, T007, T008]
- [ ] sotp verify spec-states が domain-types.json を読み Blue/Red ゲートで動作する [source: knowledge/adr/2026-04-07-0045-domain-types-separation.md §Decision.3] [tasks: T009]
- [ ] cargo make track-sync-views が domain-types.md も生成する [source: knowledge/adr/2026-04-07-0045-domain-types-separation.md §Decision.1] [tasks: T011]
- [ ] 既存 track の spec.json から domain-types.json が生成されている [source: knowledge/adr/2026-04-07-0045-domain-types-separation.md §Consequences] [tasks: T012]
- [ ] cargo make ci が全テスト通過する [source: convention — knowledge/conventions/hexagonal-architecture.md] [tasks: T001, T002, T003, T004, T005, T006, T007, T008, T009, T010, T011]

## Canonical Blocks

## Block 1 — DomainTypeKind enum (domain layer)

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainTypeKind {
    Typestate {
        transitions_to: Vec<String>,
    },
    Enum {
        expected_variants: Vec<String>,
    },
    ValueObject,
    ErrorType {
        expected_variants: Vec<String>,
    },
    TraitPort {
        expected_methods: Vec<String>,
    },
}
```

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainTypeEntry {
    name: String,
    description: String,
    kind: DomainTypeKind,
    approved: bool,  // default: true (manual), false for AI auto-added
}
```

## Block 2 — domain-types.json schema

```json
{
  "schema_version": 1,
  "domain_types": [
    { "name": "Draft", "kind": "typestate", "description": "...", "transitions_to": ["Published"], "approved": true },
    { "name": "TrackStatus", "kind": "enum", "description": "...", "expected_variants": ["Planned", "Done"], "approved": true },
    { "name": "TrackId", "kind": "value_object", "description": "...", "approved": true },
    { "name": "SchemaExportError", "kind": "error_type", "description": "...", "expected_variants": ["NightlyNotFound"], "approved": true },
    { "name": "SchemaExporter", "kind": "trait_port", "description": "...", "expected_methods": ["export"], "approved": true }
  ]
}
```

## Block 3 — Signal rules (Blue/Red binary)

| kind | Blue | Red |
|------|------|-----|
| typestate | type exists + all transitions found | type not found / transitions missing |
| enum | type exists + variants exact match | type not found / variant mismatch |
| value_object | type exists | type not found |
| error_type | type exists + all expected_variants covered | type not found / variant missing |
| trait_port | trait exists + all expected_methods present | trait not found / method missing |

## Related Conventions (Required Reading)
- knowledge/conventions/hexagonal-architecture.md
- knowledge/conventions/source-attribution.md

## Signal Summary

### Stage 1: Spec Signals
🔵 36  🟡 0  🔴 0

