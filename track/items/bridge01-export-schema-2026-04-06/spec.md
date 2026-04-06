<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: draft
version: "1.0.0"
signals: { blue: 10, yellow: 9, red: 0 }
---

# BRIDGE-01: sotp domain export-schema (rustdoc JSON)

## Goal

生成プロジェクトの domain crate から公開型・関数シグネチャを rustdoc JSON 経由で抽出し、テスト生成 AI の型コンテキストとして JSON 出力する。
syn ベースではなく rustdoc JSON を採用することで、コンパイラの型解決済み情報を利用し、パース精度への指摘が構造的に発生しない設計とする。

## Scope

### In Scope
- sotp domain export-schema CLI コマンドの実装 [source: knowledge/strategy/TODO-PLAN.md §Phase 3 item 3-1] [tasks: T005]
- rustdoc JSON の生成（nightly toolchain 経由）とパース [source: discussion] [tasks: T004]
- pub types / functions / traits / impls の抽出と domain 型への変換 [source: knowledge/strategy/TODO-PLAN.md §Phase 3 item 3-1] [tasks: T002, T003, T004]
- JSON 出力フォーマット（AI テスト生成の入力用） [source: knowledge/strategy/vision.md §1] [tasks: T005]
- nightly toolchain の dev-tool 導入と tech-stack 更新 [source: discussion] [tasks: T001]
- cargo make タスク + Docker nightly 対応 [source: discussion] [tasks: T006]

### Out of Scope
- テスト生成（Phase 3 items 3-2, 3-3, 3-4） [source: knowledge/strategy/TODO-PLAN.md §Phase 3]
- spec ↔ code 整合性チェック（Phase 3 item 3-12） [source: knowledge/strategy/TODO-PLAN.md §Phase 3]
- 既存 domain_scanner.rs (syn ベース) の置換 [source: discussion]
- proptest / usecase テストテンプレート [source: knowledge/strategy/TODO-PLAN.md §Phase 3]

## Constraints
- crate 自体は stable Rust のまま維持。nightly は rustdoc JSON 生成のみに使用する dev-tool [source: discussion]
- rustdoc-types crate の format version に依存するため、対応 nightly バージョンを固定する [source: inference — rustdoc JSON format は nightly 間で破壊的変更あり]
- hexagonal architecture に従い、rustdoc JSON パース (I/O) は infrastructure 層、出力型定義は domain 層に配置 [source: knowledge/conventions/hexagonal-architecture.md]
- nightly 不在時は fail-closed (SchemaExportError::NightlyNotFound) [source: convention — knowledge/conventions/security.md]

## Domain States

| State | Description |
|-------|-------------|
| SchemaExport | export-schema の出力トップレベル型。crate_name + types + functions + traits + impls を保持 |
| TypeInfo | 公開型の情報。name / kind (Struct/Enum/TypeAlias) / fields or variants / docs |
| FunctionInfo | 公開関数の情報。name / signature / docs / receiver type |
| TraitInfo | 公開 trait の情報。name / required methods / docs |
| ImplInfo | impl ブロックの情報。target_type / trait_name (Option) / methods |
| SchemaExportError | export-schema のエラー型。NightlyNotFound / RustdocFailed / ParseFailed / CrateNotFound |

## Acceptance Criteria
- [ ] sotp domain export-schema --crate domain が SoTOHE-core の domain crate に対して JSON を出力する [source: knowledge/strategy/TODO-PLAN.md §Phase 3 item 3-1] [tasks: T005, T007]
- [ ] 出力 JSON に TrackStatus, TaskStatus, TrackId 等の既知の pub 型が含まれる [source: discussion] [tasks: T007]
- [ ] 出力 JSON が SchemaExport 型として serde roundtrip 可能 [source: discussion] [tasks: T002, T007]
- [ ] nightly 不在時に NightlyNotFound エラーで fail-closed する [source: convention — knowledge/conventions/security.md] [tasks: T004, T007]
- [ ] cargo make export-schema -- --crate domain で Docker 経由実行可能 [source: discussion] [tasks: T006]

## Related Conventions (Required Reading)
- knowledge/conventions/hexagonal-architecture.md
- knowledge/conventions/typed-deserialization.md
- knowledge/conventions/source-attribution.md

## Signal Summary

### Stage 1: Spec Signals
🔵 10  🟡 9  🔴 0

