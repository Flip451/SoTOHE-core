<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# BRIDGE-01: sotp domain export-schema (rustdoc JSON)

BRIDGE-01: rustdoc JSON ベースのドメインスキーマ抽出コマンド sotp domain export-schema を実装する。
syn ベースの前回実装はコンパイラ再発明によりレビュー収束しなかった教訓を踏まえ、rustdoc JSON（コンパイラ解決済み情報）を採用。
nightly toolchain を dev-tool として導入し、crate 自体は stable のまま維持する。
Phase 3 テスト生成パイプラインの起点となる型コンテキスト提供基盤。

## Foundation: tech-stack 更新 + 依存追加

tech-stack.md にnightly toolchain を dev-tool として追加（crate 自体は stable のまま）。
rustdoc-types crate を infrastructure の依存に追加。
knowledge/conventions/ に nightly-dev-tool convention を新規作成（適用範囲・CI 方針を明記）。

- [x] tech-stack.md 更新 + rustdoc-types 依存追加 + nightly dev-tool convention

## Domain: スキーマ型定義 + port trait

domain 層に schema モジュールを新設。
SchemaExport: crate_name + types + functions + traits + impls を保持するトップレベル型。
TypeInfo: name, kind (Struct/Enum/TypeAlias), visibility, fields/variants, docs。
FunctionInfo: name, signature (文字列), docs, receiver (self/&self/none)。
TraitInfo: name, methods (Vec<FunctionInfo>), docs。
ImplInfo: target_type, trait_name (Option), methods (Vec<FunctionInfo>)。
SchemaExportPort: rustdoc JSON パースの port trait。export(&self, crate_name: &str) -> Result<SchemaExport, SchemaExportError>。
SchemaExportError: NightlyNotFound, RustdocFailed, ParseFailed, CrateNotFound。
全型は Serialize を derive（JSON 出力用）。

- [x] domain 層: SchemaExport / TypeInfo / FunctionInfo / TraitInfo / ImplInfo 型定義
- [x] domain 層: SchemaExportPort trait + SchemaExportError

## Infrastructure: rustdoc JSON 生成 + パース

infrastructure 層に schema_export モジュールを新設。
RustdocSchemaExporter: SchemaExportPort の実装。
Step 1: cargo +nightly rustdoc -p <crate> -- -Z unstable-options --output-format json を subprocess で実行。
Step 2: target/doc/<crate>.json を読み込み、rustdoc_types::Crate にデシリアライズ。
Step 3: Crate.index を走査し、pub な Struct/Enum/Function/Trait/Impl を domain 型に変換。
型変換は rustdoc_types::ItemEnum の match で行う。シグネチャ文字列は rustdoc_types の情報から再構築。
nightly 不在時は SchemaExportError::NightlyNotFound を返す（fail-closed）。

- [x] infrastructure 層: RustdocSchemaExporter (rustdoc JSON 生成 + パース + 変換)

## CLI: sotp domain export-schema コマンド

apps/cli に domain サブコマンドグループを新設。
sotp domain export-schema --crate <name> [--format json|pretty] [--output <path>]。
--crate: workspace 内の crate 名（必須）。
--format: json (default, compact) or pretty (indented)。
--output: ファイルパス（省略時は stdout）。
composition root で RustdocSchemaExporter を組み立て、port 経由で実行。

- [x] CLI 層: sotp domain export-schema コマンド

## Build: cargo make タスク + Docker nightly 対応

Makefile.toml に export-schema タスクを追加。
Docker コンテナ内で nightly を利用可能にする Dockerfile 更新。
cargo make export-schema -- --crate domain で実行可能にする。

- [ ] cargo make タスク + Docker nightly 対応

## Integration Test: 自己検証

SoTOHE-core 自身の domain crate に対して export-schema を実行。
出力 JSON に TrackStatus, TaskStatus, TrackId 等の既知の pub 型が含まれることを検証。
出力 JSON が valid な SchemaExport 構造であることを serde roundtrip で検証。
nightly 不在時に NightlyNotFound エラーが返ることを検証。

- [ ] 統合テスト: SoTOHE-core 自身の domain crate で export-schema 実行・出力検証
