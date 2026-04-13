<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: approved
approved_at: "2026-04-12T22:18:13Z"
version: "1.0.0"
signals: { blue: 49, yellow: 0, red: 0 }
---

# TDDD-01: 型カタログ多層化 + L1 シグネチャ検証

## Goal

TDDD を domain 層以外 (usecase 等) でも利用可能にする。`architecture-rules.json` の `layers[].tddd` を SSoT とし、CLI / verify / merge gate / `/track:design` が動的に層を発見する。
`TraitPort::expected_methods` を `Vec<String>` から `Vec<MethodDeclaration>` (name + receiver + params + returns + is_async) に拡張し、L1 解像度でメソッドシグネチャを構造的に検証する (primitive obsession の検出)。
`libs/domain/src/tddd/catalogue.rs` (2088 行) の一括リネームと DM-06 (3 モジュール分割: catalogue / signals / consistency) を同時実施し、以降のメンテナンス負荷を低減する。

## Scope

### In Scope
- `libs/domain/src/tddd/catalogue.rs` の `Domain*` シンボル一括リネーム (DomainTypeKind → TypeDefinitionKind, DomainTypeEntry → TypeCatalogueEntry, DomainTypesDocument → TypeCatalogueDocument, DomainTypeSignal → TypeSignal, evaluate_domain_type_signals → evaluate_type_signals, check_domain_types_signals → check_type_signals) と 3 モジュール分割 (DM-06 同時解消) [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md D3, knowledge/strategy/TODO.md DM-06] [tasks: T001]
- infrastructure 層の catalogue_codec / baseline_codec / baseline_builder / domain_types_render (section header を `## Domain Types` → `## Type Declarations` に改題、関数名を render_type_catalogue に変更) / verify/spec_states / merge_gate_adapter のリネーム [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md D3] [tasks: T002]
- usecase (merge_gate, task_completion) / CLI (subcommand `domain-type-signals` → `type-signals`, baseline capture) / Makefile.toml のリネーム [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md D3] [tasks: T003]
- `ParamDeclaration` (struct) / `MethodDeclaration` (struct) / `MemberDeclaration` (enum-first: Variant/Field) を domain 層 catalogue.rs に追加 [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md D2, knowledge/research/2026-04-12-0709-planner-tddd-01.md Type Design] [tasks: T004]
- `FunctionInfo` に構造化フィールド (params / returns / receiver / is_async) 追加。既存 `signature: String` は削除し、`MethodDeclaration::signature_string()` で都度生成 (C1) [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md Phase 1-4, knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md Consequences C1] [tasks: T004]
- `TypeInfo::members` を `Vec<MemberDeclaration>` に、`TypeNode::{members, methods}` を新シェイプに、`TraitNode::methods: Vec<MethodDeclaration>` に拡張 [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md Phase 1-4] [tasks: T004]
- `RustdocSchemaExporter` に recursive `format_type` / `format_args` ヘルパーを追加して `FunctionInfo` の 4 構造化フィールド (`params` / `returns` / `receiver` / `is_async`) を抽出する。`build_type_graph` の FunctionInfo → MethodDeclaration 変換。注: CLI が `schema_export.targets` を読み複数 crate をループする wiring は T007 の `architecture-rules.json` 拡張後に実装する — T004 では `SchemaExporter::export(&self, crate_name)` の per-crate 抽出ロジックのみを実装する [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md D1, D6, knowledge/research/2026-04-12-0709-rustdoc-types-extraction.md] [tasks: T004]
- `TypeBaselineEntry` / `TraitBaselineEntry` を MemberDeclaration + MethodDeclaration ベースに拡張、`baseline_codec` の schema_version を 2 にバンプ、v1 検出時に明示エラーメッセージ + 再実行手順。`check_consistency` の TypeBaselineEntry 構築を新シェイプに更新 [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md Phase 1-4 + Baseline schema migration] [tasks: T005]
- `TypeDefinitionKind::TraitPort { expected_methods: Vec<MethodDeclaration> }` に拡張し、`catalogue_codec` の schema_version を 2 にバンプ、top-level key を `domain_types` → `type_definitions` に変更、ty/returns の `::` 含有は codec で reject [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md D2, D3] [tasks: T006]
- `evaluate_trait_port` に L1 forward check (ステップ 1-6: name, receiver, params 数, params 型, returns, async) と reverse check (ステップ 7: undeclared method → Red) を実装 [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md D2] [tasks: T006]
- `architecture-rules.json` の `layers[]` に optional な `tddd { enabled, catalogue_file, schema_export: { method, targets } }` ブロックを追加 [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md D1] [tasks: T007]
- `sotp track type-signals` / `baseline-capture` に `--layer <layer_id>` flag を追加 (未指定時は全 enabled 層を loop、`enabled=false` 層指定は fail-closed) [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md D1, Phase 1 step 5] [tasks: T007]
- `verify_from_spec_json` で全 `tddd.enabled` 層の catalogue を per-layer symlink-guarded で読み、AND 集約 [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md Phase 1 step 6, knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md D4.3] [tasks: T007]
- `TrackBlobReader` に `read_type_catalogue(branch, track_id, layer_id)` port method を追加、`check_strict_merge_gate` で per-layer loop、U19-U26 新規テスト (2-layer 組み合わせ) [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md Phase 1 step 6, knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md U1-U18] [tasks: T007]
- merge gate が `architecture-rules.json` を PR ブランチの blob から読み込むこと (`read_architecture_rules(branch)` port method 経由、ローカル `workspace_root` ファイルシステムコピーではない)。これにより `architecture-rules.json` 自体を変更するトラック PR でも正しいルールセットで評価できる [source: knowledge/research/2026-04-12-0709-planner-tddd-01.md Q6] [tasks: T007]
- 全 `tddd.enabled` 層の `catalogue_file` 値はユニークでなければならない — 起動時に重複を検出した場合は fail-closed でエラーを返すこと。各層の生成物は `catalogue_file` の stem を使用すること: catalogue JSON = `<stem>.json`、rendered MD = `<stem>.md`、baseline JSON = `<stem>-baseline.json` (例: `usecase-types.json` → `usecase-types.md` / `usecase-types-baseline.json`) [source: knowledge/research/2026-04-12-0709-planner-tddd-01.md Q8] [tasks: T007]
- `.claude/commands/track/design.md` を多層 loop 対応 (layer 順に design → type-signals --layer → 集約) [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md Phase 1 step 7] [tasks: T007]
- `knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md` の Status を `Proposed` → `Accepted (implemented in track tddd-01-multilayer-2026-04-12, 2026-04-12)` に更新 [source: knowledge/conventions/adr.md] [tasks: T007]

### Out of Scope
- L2 generics/bounds フィールド (generics: {type_params: [{name, bounds}]}) — Phase 2 以降 [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md Phase 2]
- Cross-layer 型参照の catalogue 明示と評価 — Phase 2 以降 (Phase 1 では cargo + L1 が検証をカバー) [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md D5, R4, Phase 2]
- 層ごとの Kind 制限 lint (forbidden_kinds 等) — TDDD コアに持たず、別 lint として実装可能 [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md D4, R3]
- 多言語対応 (schema_export.method を rust 以外に拡張) — Phase 3 以降 [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md Phase 3, Reassess When]
- v1 baseline / catalogue の自動マイグレーション — 後方互換性を持たず、既存 baseline は削除して baseline-capture で再生成する [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md Phase 1-4 + Baseline schema migration, discussion — planner Q7]
- async-trait proc-macro で desugar されたメソッドの is_async 検出 — rustdoc JSON レベルでは is_async=false になる既知の制約。Catalog 側も `async: false` を使う [source: discussion — planner EC-D, knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md Consequences C2]

## Constraints
- 後方互換性は対応しない。一括リネーム、v1 codec alias なし、既存 baseline/catalogue ファイルは再生成が必要 [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md D3 + Baseline schema migration] [tasks: T001, T002, T003, T005, T006]
- `architecture-rules.json` の `tddd` ブロックは optional フィールド追加 (version 2 維持)。未指定層は TDDD 対象外として従来通り動作 [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md D1] [tasks: T007]
- `catalogue_file` は optional。省略時のデフォルトは `<layers[].crate>-types.json` (当該 `layers[]` エントリの `crate` フィールド値を使用)。`schema_export.targets` は配列 (1 層 = 複数 crate 可)。`targets` が複数の場合も catalogue_file は `layers[].crate` ベースの単一ファイル名 [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md D1] [tasks: T004, T007]
- 型表現はモジュールパスを最終セグメント (短縮名) に正規化、ジェネリクス構造は完全保持 (例: `Result<Option<User>, DomainError>`)。`::` を含む型文字列は codec で reject [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md D2 型表現の解像度] [tasks: T004, T006]
- `FunctionInfo::signature: String` を削除し、表示が必要な場面は `MethodDeclaration::signature_string()` (レンダリング時に構造化フィールドから都度生成) に置換する。冗長フィールドを残さず構造化フィールドのみを source of truth とする。BRIDGE-01 (`sotp domain export-schema`) の JSON 出力から `signature` キーが消える breaking change を受容する [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md Consequences C1] [tasks: T004]
- `MemberDeclaration` は enum-first (Variant / Field) で設計。`struct + Option<ty>` の illegal state (struct field で ty=None) を構造的に排除 [source: .claude/rules/04-coding-principles.md (Enum-first パターン), knowledge/research/2026-04-12-0709-planner-tddd-01.md Type Design Q3] [tasks: T004]
- `evaluate_trait_port` の L1 forward check は完全マッチ (ファジーマッチルールなし)。宣言と実装の params 順序も厳密に比較 [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md D2 完全マッチの利点] [tasks: T006]
- 多層化で全 `tddd.enabled` 層の catalogue を AND 集約。symlink guard は per-layer に適用 (strict-signal-gate-v2 §D4.3 を維持) [source: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md D4.3] [tasks: T007]
- merge gate は全 layer の finding を AND 集約 (per-layer short-circuit せず、全層の finding を union 返却) し、一度の診断で全問題を開発者に見せる [source: knowledge/research/2026-04-12-0709-planner-tddd-01.md Edge Cases #7] [tasks: T007]
- `cargo make ci` / `cargo make deny` / `cargo make verify-spec-states` が通ること [source: .claude/rules/07-dev-environment.md] [tasks: T001, T002, T003, T004, T005, T006, T007]

## Acceptance Criteria
- [ ] `libs/domain/src/tddd/catalogue.rs` が 3 モジュール (catalogue.rs / signals.rs / consistency.rs) に分割され、各モジュールが 400 行以下の warn threshold を下回ること (`cargo make check-layers` でモジュール行数確認) [source: knowledge/strategy/TODO.md DM-06, architecture-rules.json module_limits] [tasks: T001]
- [ ] `domain::tddd::catalogue` / `usecase::merge_gate` / CLI / Makefile など全レイヤーで `Domain*` シンボル / `domain-type-signals` サブコマンドが残存しないこと。検証 grep は `libs apps .claude Makefile.toml knowledge/adr knowledge/strategy` に path scoped (履歴 track 配下 `track/items/**` と frozen research snapshot `knowledge/research/**` は除外) — `rg 'DomainType|domain-type-signals|domain_types' libs apps .claude Makefile.toml knowledge/adr knowledge/strategy` で新名に置換確認 [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md D3] [tasks: T001, T002, T003]
- [ ] [T006 v2 wire-format 検証] `catalogue_codec` が `schema_version: 2` のみを decode 成功とし、v1 (`schema_version: 1`) および v1 時代の top-level key `domain_types` を含むドキュメントを `UnsupportedSchemaVersion(1)` または `InvalidEntry` でリジェクトすること。encode 側は常に `schema_version: 2` と top-level key `type_definitions` を emit し、旧 key `domain_types` を生成しないこと。round-trip (v2 JSON → TypeCatalogueDocument → v2 JSON) でキーと構造が保存されること [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md D2, knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md D3] [tasks: T006]
- [ ] `MethodDeclaration` / `ParamDeclaration` / `MemberDeclaration` が domain 層 `catalogue.rs` に定義され、`FunctionInfo` / `TypeNode` / `TraitNode` / `TypeBaselineEntry` / `TraitBaselineEntry` の 5 箇所で共有されていること [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md Phase 1-4 MethodDeclaration 共有] [tasks: T004, T005, T006]
- [ ] [T007 検証] CLI が `architecture-rules.json` の `schema_export.targets: Vec<String>` を読み、各 crate に対して `SchemaExporter::export(crate_name)` を順次呼び出して結果を統合できること (既存 `SchemaExporter` trait の `export(&self, crate_name: &str)` シグネチャは変更しない — BRIDGE-01 互換を維持)。この wiring は T007 (`architecture-rules.json` への `tddd` ブロック追加後) で実装 — T004 対象外 [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md D1 targets は配列] [tasks: T007]
- [ ] C1 breaking change が実際に適用されていること: `rg 'pub fn signature|fn signature\(|format_sig|FunctionInfo.*signature' libs/` がゼロ件を返し、`FunctionInfo::signature: String` フィールド・`pub fn signature()` アクセサ・`format_sig` 関数が削除されていること [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md C1] [tasks: T004]
- [ ] TraitPort の L1 forward check が「名前一致・receiver 不一致」「params 数不一致」「params 型順序不一致」「returns 不一致」「async 不一致」の各ケースで Yellow を返し、全一致時のみ Blue を返すこと [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md D2 Forward check] [tasks: T006]
- [ ] trait にカタログ未宣言のメソッドがあれば reverse check で Red を返すこと [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md D2 Reverse check] [tasks: T006]
- [ ] `catalogue_codec` が `ty` / `returns` に `::` を含む文字列を `InvalidEntry` でリジェクトし、last-segment 強制を codec レベルで保証すること [source: knowledge/research/2026-04-12-0709-planner-tddd-01.md Edge Cases EC-E] [tasks: T006]
- [ ] `baseline_codec` が v1 baseline を検出した際に `UnsupportedSchemaVersion(1)` を返し、エラーメッセージに `baseline-capture --layer <layer>` の再実行手順を含むこと [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md Baseline schema migration, knowledge/research/2026-04-12-0709-planner-tddd-01.md Q7] [tasks: T005]
- [ ] `sotp track type-signals <id>` が `--layer` 未指定時に全 `tddd.enabled` 層を順次処理し、`--layer` に `enabled=false` の層を指定した場合は fail-closed でエラーを返すこと [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md D1 + Phase 1 step 5, knowledge/research/2026-04-12-0709-planner-tddd-01.md Q9] [tasks: T007]
- [ ] `verify_from_spec_json` が `architecture-rules.json` を読み、全 `tddd.enabled` 層の catalogue を per-layer で symlink-guarded + decode + evaluate し、一つでも違反があれば BLOCKED を返すこと (AND 集約) [source: knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md Phase 1 step 6] [tasks: T007]
- [ ] merge gate の U19-U26 新規テストが全て pass し、既存 U1-U18 (single-layer) が変更なく pass すること (2-layer combinations: NotFound+NotFound, NotFound+Blue, Blue+Yellow, Yellow+NotFound, Blue+Red, FetchError+Blue, NotFound+FetchError, Blue+Blue) [source: knowledge/research/2026-04-12-0709-planner-tddd-01.md Q12] [tasks: T007]
- [ ] `knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md` の Status が `Accepted (implemented in track tddd-01-multilayer-2026-04-12, 2026-04-12)` になっていること [source: knowledge/conventions/adr.md] [tasks: T007]
- [ ] `cargo make ci` (fmt-check + clippy + test + deny + check-layers + verify-arch-docs + verify-spec-states) が通ること [source: .claude/rules/07-dev-environment.md] [tasks: T001, T002, T003, T004, T005, T006, T007]

## Related Conventions (Required Reading)
- knowledge/conventions/hexagonal-architecture.md
- knowledge/conventions/prefer-type-safe-abstractions.md
- knowledge/conventions/typed-deserialization.md
- knowledge/conventions/source-attribution.md
- knowledge/conventions/nightly-dev-tool.md
- knowledge/conventions/adr.md

## Signal Summary

### Stage 1: Spec Signals
🔵 49  🟡 0  🔴 0

