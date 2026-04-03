<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: draft
version: "1.0"
signals: { blue: 8, yellow: 17, red: 0 }
---

# BRIDGE-01: sotp export-schema — pub シグネチャ抽出 CLI

## Goal

生成プロジェクトの Rust ソースから pub アイテムのシグネチャを syn AST で抽出し、AI テスト生成向けのコンパクトな型コンテキストを出力する sotp export-schema コマンドを実装する。
Phase 3 テスト生成パイプライン（3-2 spec→テスト、3-3 proptest、3-12 spec↔code 整合性）の基盤となる。

## Scope

### In Scope
- Domain 型定義: ExportedSchema, SchemaCrate, SchemaModule, SchemaItem enum (Struct/Enum/Trait/TypeAlias/Constant/InherentImpl) [source: inference — extracted schema aggregate/domain model naming for BRIDGE-01 v1] [tasks: T001]
- domain_scanner.rs から共通パース基盤の抽出（ソース収集、ファイルパース、pub 判定） [source: inference — domain_scanner.rs との重複排除] [tasks: T002]
- syn AST による pub アイテム抽出: struct (フィールド), enum (バリアント), trait (メソッド), type alias, const, InherentImpl メソッド [source: inference — v1 extractor scope for public signature export] [tasks: T003]
- 複数パス指定: positional paths + --layer によるレイヤー名解決 (architecture-rules.json) [source: discussion] [tasks: T004, T006]
- 出力形式: text (Rust 風シグネチャ、モジュール階層保持) と json (schema_version: 1、安定ソート) [source: inference — AI コンテキスト注入 + 3-12 機械処理の両方に対応] [tasks: T005]
- CLI サブコマンド: sotp export-schema (positional paths, --layer, --format text|json, --project-root) [source: discussion] [tasks: T006]
- pub(crate) および non-pub アイテムの除外 [source: knowledge/strategy/vision.md §7] [tasks: T003]
- 戦略ドキュメント更新: vision.md / TODO-PLAN.md のコマンド名を sotp export-schema に更新し、TODO.md の Phase 3 roadmap を BRIDGE-01 に整合させる [source: discussion] [tasks: T007]

### Out of Scope
- trait impl のメソッドエクスポート（v1 ではノイズが多い。InherentImpl と trait 定義で十分） [source: inference — v1 スコープ制限]
- #[path = ...] 属性によるカスタムモジュールパス解決 [source: inference — v1 では file path + inline mod で十分]
- CodeScanResult の変更（既存信号機への影響ゼロを保証） [source: inference — Phase 2 安定性]
- spec ↔ code 整合性チェック（3-12 は BRIDGE-01 完了後に別トラックで実装） [source: knowledge/adr/2026-03-23-2130-spec-code-consistency-deferred.md]

## Constraints
- syn は infrastructure 層のみで使用。Domain 型は syn に依存しない [source: convention — knowledge/conventions/hexagonal-architecture.md]
- モジュールサイズ上限: 400行 warning / 700行 max [source: architecture-rules.json]
- TDD: テストを先に書く（Red → Green → Refactor） [source: convention — .claude/rules/05-testing.md]
- パニック禁止（unwrap/expect/panic/todo は非テストコードで使用不可） [source: convention — .claude/rules/04-coding-principles.md]
- 同期のみ（async なし） [source: track/tech-stack.md]

## Domain States

| State | Description |
|-------|-------------|
| SchemaItem | 抽出された pub アイテムの種類。Struct, Enum, Trait, TypeAlias, Constant, InherentImpl の6バリアント |
| ExportedSchema | 全クレートの抽出結果を保持するトップレベル集約 |
| SchemaCrate | 1クレート分の抽出結果。crate_name, root_path, modules を保持 |
| SchemaModule | 1モジュール内の pub アイテム集合。ModulePath でモジュール階層を表現 |

## Acceptance Criteria
- [ ] sotp export-schema libs/domain/src で pub struct/enum/trait/InherentImpl/type alias/const のシグネチャがモジュール階層を保持した text 形式で出力される [source: inference — BRIDGE-01 text output contract for public signature export] [tasks: T003, T005, T006]
- [ ] --format json で schema_version: 1 の構造化 JSON が出力される（crate/module/item 順で安定ソート） [source: inference — 3-12 機械処理用] [tasks: T005, T006]
- [ ] pub(crate) および non-pub アイテムが出力に含まれない [source: knowledge/strategy/vision.md §7] [tasks: T003]
- [ ] 複数パス指定で複数クレートの schema が統合出力される [source: discussion] [tasks: T004, T006]
- [ ] --layer で architecture-rules.json からパスが自動解決される [source: discussion] [tasks: T004, T006]
- [ ] domain_scanner.rs の既存テストが全てパスする（behavior 変更なし） [source: inference — Phase 2 安定性] [tasks: T002]
- [ ] --project-root オプションで architecture-rules.json の検索ルートを指定できる [source: inference — 外部プロジェクトからの利用] [tasks: T006]
- [ ] vision.md / TODO-PLAN.md のコマンド名が sotp export-schema に更新され、TODO.md の Phase 3 roadmap と最終更新日が今回の戦略更新に整合している [source: discussion] [tasks: T007]

## Related Conventions (Required Reading)
- knowledge/conventions/hexagonal-architecture.md
- knowledge/conventions/source-attribution.md

## Signal Summary

### Stage 1: Spec Signals
🔵 8  🟡 17  🔴 0

