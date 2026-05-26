<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 23, yellow: 0, red: 0 }
---

# typestate は struct 形状と直交配置する — 全 struct 形状を typestate 状態にできるよう修正する

## Goal

- [GO-01] TDDD catalogue_v2 の `TypeKindV2` スキーマにおいて、typestate membership marker (`TypestateMarker`) を struct 形状（unit / tuple / plain）と直交する位置に配置し直すことで、unit struct・tuple struct を含む任意の struct 形状を typestate クラスタの状態型として catalogue 宣言できるようにする。旧設計で達成した『構造的な形状と DDD 意味論的なパターンの分離』（カテゴリーエラーの解消）はそのまま維持する [adr: knowledge/adr/2026-05-26-1002-typestate-struct-kind-orthogonal.md#D1]

## Scope

### In Scope
- [IN-01] `libs/domain/src/tddd/catalogue_v2/composite.rs` の `TypeKindV2` スキーマを変更する: 3 つの struct 形状（UnitStruct / TupleStruct / PlainStruct）を同じグループとして扱い、typestate marker をそのグループに対して 1 回だけ付与する構造にする。具体的な Rust 型構造（shape enum の名前・フィールド構成）は type-designer が Phase 2 で確定する [adr: knowledge/adr/2026-05-26-1002-typestate-struct-kind-orthogonal.md#D1] [tasks: T001]
- [IN-02] `libs/infrastructure/src/tddd/` 配下の codec を新 `TypeKindV2` スキーマに対応させる: `rustdoc_types::StructKind::Unit` および `StructKind::Tuple` を受け取ったときに typestate marker を付与できるよう encode/decode ロジックを更新する [adr: knowledge/adr/2026-05-26-1002-typestate-struct-kind-orthogonal.md#D1] [tasks: T002]
- [IN-03] アクティブなトラックの catalogue ファイル（`*-types.json`）を新スキーマへ移行する: 新スキーマ導入により既存の PlainStruct 宣言の JSON 表現が変わる場合、アクティブトラックのカタログ宣言を新しい構造に書き換える [adr: knowledge/adr/2026-05-26-1002-typestate-struct-kind-orthogonal.md#D1] [tasks: T004]
- [IN-04] linter・signal evaluator など `TypeKindV2` を消費するコンポーネントを新スキーマに対応させる: typestate marker の配置変更にともない、typestate 整合性チェックや signal 評価のパターンマッチを新しい構造に合わせて更新する [adr: knowledge/adr/2026-05-26-1002-typestate-struct-kind-orthogonal.md#D1] [tasks: T003]
- [IN-05] unit struct・tuple struct な typestate 状態型（例: `struct Locked;`, `struct Pending(Uuid)`）を新スキーマで正しく宣言・評価できることを確認するテストを追加する [adr: knowledge/adr/2026-05-26-1002-typestate-struct-kind-orthogonal.md#D1] [tasks: T005]

### Out of Scope
- [OS-01] Enum / TypeAlias への typestate marker 拡張: ADR Reassess When に記載の通り、Enum や TypeAlias を typestate 状態として使うパターンは本 track のスコープ外とする。typestate marker の scope は struct 形状に限定する [adr: knowledge/adr/2026-05-26-1002-typestate-struct-kind-orthogonal.md#D1]
- [OS-02] 非アクティブトラックのカタログファイルへの遡及移行: archive 済み・completed track の `*-types.json` は保護対象であり、新スキーマへの移行は行わない [adr: knowledge/adr/2026-05-26-1002-typestate-struct-kind-orthogonal.md#D1]
- [OS-03] typestate marker の意味論拡張（`TypestateMarker` の `state_name` / `transitions` の役割変更）: 本 track は typestate marker の配置を修正するのみであり、その内部意味論の変更は別 ADR の対象とする [adr: knowledge/adr/2026-05-26-1002-typestate-struct-kind-orthogonal.md#D1]
- [OS-04] linter による typestate 整合性検証ルール（receiver 制約 / generics unwrap 範囲等）の意味論的な拡張・変更: 本 track は構造的配置の修正のみを対象とし、linter の検証ロジックの意味論的拡張は別 ADR の対象とする [adr: knowledge/adr/2026-05-26-1002-typestate-struct-kind-orthogonal.md#D1]

## Constraints
- [CN-01] typestate marker を共通位置に移しても、各 struct 形状固有の制約（UnitStruct はフィールドを持てない / TupleStruct には named field がない）は引き続き保証されなければならない。typestate marker の共通化は形状固有の制約を弱めることを意味しない [adr: knowledge/adr/2026-05-26-1002-typestate-struct-kind-orthogonal.md#D1] [tasks: T001, T002, T003]
- [CN-02] 本変更はスキーマの破壊的変更を許容する。アクティブなトラックのカタログのみを新スキーマへ移行し、非アクティブなトラックのカタログファイルは変更しない [adr: knowledge/adr/2026-05-26-1002-typestate-struct-kind-orthogonal.md#D1] [tasks: T004]
- [CN-03] `TypeKindV2` の変更は domain 層（`libs/domain/src/tddd/catalogue_v2/`）で行い、codec の対応変更は infrastructure 層（`libs/infrastructure/src/tddd/`）で行う。hexagonal layer 依存方向を維持する [adr: knowledge/adr/2026-05-26-1002-typestate-struct-kind-orthogonal.md#D1, knowledge/adr/2026-05-08-0248-tddd-catalogue-layer-schema-axis-separation.md#D3] [tasks: T001, T002, T003]
- [CN-04] ADR `2026-05-08-0248` D3 が達成した『構造的な形状（kind）と DDD 意味論的なパターンの分離』（カテゴリーエラーの解消）は新スキーマでも維持する。typestate marker を struct グループに移すことは、形状と意味論の再混在ではなく、直交性の実現である [adr: knowledge/adr/2026-05-26-1002-typestate-struct-kind-orthogonal.md#D1, knowledge/adr/2026-05-08-0248-tddd-catalogue-layer-schema-axis-separation.md#D3] [tasks: T001]
- [CN-05] rejected alternative A（全 struct variant に typestate フィールドを個別追加）および rejected alternative B（unit/tuple 状態型を PlainStruct として宣言させる運用ルール）はいずれも採用しない [adr: knowledge/adr/2026-05-26-1002-typestate-struct-kind-orthogonal.md#D1] [tasks: T001]

## Acceptance Criteria
- [ ] [AC-01] unit struct な typestate 状態型（例: `struct Locked;`）を catalogue で `typestate: Some(TypestateMarker { ... })` 付きで宣言でき、signal evaluator が 🔵 Blue と評価する [adr: knowledge/adr/2026-05-26-1002-typestate-struct-kind-orthogonal.md#D1] [tasks: T003, T005]
- [ ] [AC-02] tuple struct な typestate 状態型（例: `struct Pending(Uuid)`）を catalogue で `typestate: Some(TypestateMarker { ... })` 付きで宣言でき、signal evaluator が 🔵 Blue と評価する [adr: knowledge/adr/2026-05-26-1002-typestate-struct-kind-orthogonal.md#D1] [tasks: T003, T005]
- [ ] [AC-03] CatalogueDocument JSON codec が unit struct + typestate marker の catalogue 宣言を decode / encode round-trip でき、A 側（catalogue/domain）は typestate marker を保持し、C 側（rustdoc 由来）は `rustdoc_types::StructKind::Unit` として shape-only に一致する [adr: knowledge/adr/2026-05-26-1002-typestate-struct-kind-orthogonal.md#D1] [tasks: T002, T005]
- [ ] [AC-04] CatalogueDocument JSON codec が tuple struct + typestate marker の catalogue 宣言を decode / encode round-trip でき、A 側（catalogue/domain）は typestate marker を保持し、C 側（rustdoc 由来）は `rustdoc_types::StructKind::Tuple` として shape-only に一致する [adr: knowledge/adr/2026-05-26-1002-typestate-struct-kind-orthogonal.md#D1] [tasks: T002, T005]
- [ ] [AC-05] UnitStruct はフィールドを保持できないという形状固有の制約が、新スキーマでも型レベルで保証される（フィールドを持つ UnitStruct の宣言が parse 段階で reject される） [adr: knowledge/adr/2026-05-26-1002-typestate-struct-kind-orthogonal.md#D1] [tasks: T001, T002, T005]
- [ ] [AC-06] TupleStruct が named field を保持できないという形状固有の制約が、新スキーマでも型レベルで保証される（named FieldDecl を持つ TupleStruct の宣言が parse 段階で reject される） [adr: knowledge/adr/2026-05-26-1002-typestate-struct-kind-orthogonal.md#D1] [tasks: T001, T002, T005]
- [ ] [AC-07] 既存の PlainStruct + typestate の宣言（修正前から有効だったパターン）が新スキーマでも引き続き動作し、signal evaluator が 🔵 Blue と評価する（リグレッションなし） [adr: knowledge/adr/2026-05-26-1002-typestate-struct-kind-orthogonal.md#D1] [tasks: T002, T003, T005]
- [ ] [AC-08] `cargo make ci`（fmt-check + clippy + nextest + deny + check-layers + verify-*）が pass する [adr: knowledge/adr/2026-05-26-1002-typestate-struct-kind-orthogonal.md#D1] [tasks: T005]

## Related Conventions (Required Reading)
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies
- knowledge/conventions/no-backward-compat.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule
- .claude/rules/04-coding-principles.md#No Panics in Library Code
- .claude/rules/04-coding-principles.md#Make Illegal States Unrepresentable
- .claude/rules/05-testing.md#Test Structure

## Signal Summary

### Stage 1: Spec Signals
🔵 23  🟡 0  🔴 0

