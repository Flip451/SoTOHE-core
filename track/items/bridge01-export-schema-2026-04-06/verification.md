# Verification: BRIDGE-01 sotp domain export-schema

## Scope Verified

- [x] T001: tech-stack.md 更新 + rustdoc-types 依存追加
- [x] T002: domain SchemaExport 型定義
- [x] T003: domain SchemaExportPort trait
- [x] T004: infrastructure RustdocSchemaExporter
- [x] T005: CLI sotp domain export-schema コマンド
- [x] T006: cargo make タスク + Docker nightly 対応
- [x] T007: 統合テスト

## Manual Verification Steps

1. `rustup toolchain install nightly` を実行
2. `bin/sotp domain export-schema --crate domain --format pretty` を実行し、JSON 出力を目視確認
3. 出力に `TrackStatus`, `TaskStatus`, `TrackId` 等の既知型が含まれることを確認
4. `cargo make export-schema -- --crate domain` で Docker 経由実行を確認
5. nightly をアンインストールして `bin/sotp domain export-schema --crate domain` が NightlyNotFound エラーを返すことを確認

## Result

(未実施)

## Open Issues

(なし)

## verified_at

(未記入)
