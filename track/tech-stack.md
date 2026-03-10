# 技術スタック

> このファイルは技術的な決定の「真実の源泉」です。
> 実装前にユーザーと対話して `TODO:` 項目を埋めてください。
> `TODO:` が残っている間は実装を開始してはいけません。

## コア言語・実行環境

- **言語**: Rust (stable, 最新安定版)
- **Rust Edition**: 2024
- **非同期ランタイム**: なし（同期）
- **MSRV**: 1.85

## アーキテクチャ

- **パターン**: `Workspace + Layered Architecture`（固定）
- **ドメインモデリング**: Rust Enum/Struct + Newtype パターン
- **エラー型**: `thiserror` (`#[derive(Error)]`) でドメインエラーを定義
- **Repository 契約**: 現行テンプレートでは同期トレイトを前提とする
  理由: async DB を採用する場合は async runtime の決定に加えて `domain/usecase/infrastructure` の repository 契約変更が必要になるため、採用時にまとめて見直す
- **機械可読 SoT**: `docs/architecture-rules.json`

### Workspace 構成（標準）

```text
libs/domain          # 最下層（外部依存を最小化）
libs/usecase         # domain を利用
libs/infrastructure  # domain を利用（実装詳細）
apps/api             # usecase を利用
apps/server          # composition root（api/usecase/infrastructure/domain を束ねる）
```

### 依存ルール（強制）

- `domain` は `usecase/infrastructure/api/server` へ依存してはいけない
- `usecase` は `infrastructure/api/server` へ依存してはいけない
- `api` は `infrastructure/server/domain` へ依存してはいけない
- `infrastructure` は `api/server` へ依存してはいけない
- `deny.toml` と `scripts/check_layers.py` は `docs/architecture-rules.json` と同期させる
- ルール検証: `cargo make check-layers` と `cargo make deny`

## Web レイヤー

- **フレームワーク**: なし（CLI ツールのため不要。`clap` 4.5 を使用）
- **HTTP クライアント**: `reqwest` 0.13（`blocking` feature）

## 永続化レイヤー

- **DB ライブラリ**: なし（JSON ファイルベースで管理）
- **マイグレーション**: なし
- **DB**: なし

## オブザーバビリティ

- **ロギング**: `tracing` + `tracing-subscriber`
- **メトリクス**: なし

## ビルド・ツール

- **タスクランナー**: `cargo-make` (Makefile.toml)
- **テスト**: `cargo nextest`
- **静的解析**: `cargo make clippy`
- **フォーマット**: `rustfmt` (rustfmt.toml で設定)
- **依存関係監査**: `cargo-deny` (deny.toml), `cargo-machete`
- **カバレッジ**: `cargo-llvm-cov`

## ユーティリティ

- **UUID**: `uuid` (`features = ["v4", "serde"]`)
- **時刻**: `chrono` (`features = ["serde"]`)
- **シリアライゼーション**: `serde` + `serde_json`
- **設定管理**: `config` 0.15
- **モック**: `mockall` 0.14（dev-dependency）

## 認証・セキュリティ

- **パスワードハッシュ**: なし
- **トークン**: なし

## Version Baseline

- **最新調査日**: 2026-03-11
- **調査ログ**: `.claude/docs/research/version-baseline-2026-03-11.md`
- **反映対象**: `Cargo.toml`, `Dockerfile`, `Makefile.toml`（ツールバージョン指定がある場合）

## 変更履歴

| 日付 | 変更内容 | 理由 |
|------|---------|------|
| 2026-02-28 | テンプレート初期化（対話入力型に変更） | 固定値ではなくプロジェクト開始時に合意形成するため |
| 2026-03-11 | 技術選定完了（同期CLI, clap, reqwest, config, mockall） | SoTOHE-core プロジェクト開始 |
