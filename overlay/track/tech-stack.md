# 技術スタック

> このファイルは技術的な決定の「真実の源泉」です。
> 実装前にユーザーと対話して `TODO:` 項目を埋めてください。
> `TODO:` が残っている間は実装を開始してはいけません。

## コア言語・実行環境

- **言語**: Rust (stable, 最新安定版)
- **Rust Edition**: 2024
- **非同期ランタイム**: TODO: なし（同期） / `tokio` / `async-std` のいずれかを決定
- **MSRV**: TODO: プロジェクトの下限を決定（Edition 2024 の下限は 1.85）

## アーキテクチャ

- **パターン**: `Workspace + Layered Architecture`（固定）
- **ドメインモデリング**: Rust Enum/Struct + Newtype パターン
- **エラー型**: `thiserror` (`#[derive(Error)]`) でドメインエラーを定義
- **Repository 契約**: 現行テンプレートでは同期トレイトを前提とする
  理由: async DB を採用する場合は async runtime の決定に加えて `domain/usecase/infrastructure` の repository 契約変更が必要になるため、採用時にまとめて見直す
- **機械可読 SoT**: `architecture-rules.json`

### Workspace 構成（標準）

```text
libs/domain          # 最下層（外部依存を最小化）
libs/usecase         # domain を利用
libs/infrastructure  # domain を利用（実装詳細）
apps/cli             # CLI エントリーポイント（bin のみ。DI 配線は apps/cli-composition に委譲）
apps/cli-composition # CLI composition root（domain / usecase / infrastructure を束ねる）
apps/cli-driver      # CLI primary adapter（invoke + render、usecase のみに依存）
```

### 依存ルール（強制）

- `domain` は `usecase/infrastructure/cli` へ依存してはいけない
- `usecase` は `infrastructure/cli` へ依存してはいけない
- `infrastructure` は `domain` と `usecase` に依存可能（usecase port trait を実装するため）。`cli` へは依存してはいけない
- `deny.toml` と `sotp verify layers` は `architecture-rules.json` と同期させる
- ルール検証: `cargo make check-layers` と `cargo make deny`
- 詳細: `knowledge/conventions/hexagonal-architecture.md`

## Web レイヤー

- **フレームワーク**: TODO: なし（CLI のみ） / `axum` / `actix-web` 等を決定
- **HTTP クライアント**: TODO: 不要 / `reqwest`（`blocking` feature か async か）を決定

## 永続化レイヤー

- **DB ライブラリ**: TODO: なし（ファイルベース） / `sqlx` / `diesel` 等を決定
- **マイグレーション**: TODO: 不要 / ツールを決定
- **DB**: TODO: なし / PostgreSQL / SQLite 等を決定

## オブザーバビリティ

- **ロギング**: TODO: `tracing` + `tracing-subscriber` を採用するか決定
- **メトリクス**: TODO: なし / 採用ツールを決定

## ビルド・ツール

- **タスクランナー**: `cargo-make` (Makefile.toml)
- **テスト**: `cargo nextest`
- **静的解析**: `cargo make clippy`
- **フォーマット**: `rustfmt` (rustfmt.toml で設定)
- **依存関係監査**: `cargo-deny` (deny.toml), `cargo-machete`
- **カバレッジ**: `cargo-llvm-cov`

## ユーティリティ

- **UUID**: TODO: 必要なら `uuid` (`features = ["v4", "serde"]`) を決定
- **時刻**: TODO: 必要なら `chrono` (`features = ["serde"]`) を決定（domain 層でも newtype wrap すれば I/O なしの純粋ユーティリティとして使用可）
- **シリアライゼーション**: `serde` + `serde_json`
- **設定管理**: TODO: 必要なら `config` 等を決定
- **モック**: `mockall`（dev-dependency）
- **パラメータ化テスト**: `rstest`（dev-dependency — `#[rstest]` + `#[case]` でパラメータ化テスト、`#[fixture]` で共通セットアップ注入）
- **外部連携**: TODO: 外部サービス / SDK 連携があれば決定

## 認証・セキュリティ

- **パスワードハッシュ**: TODO: なし / `argon2` 等を決定
- **トークン**: TODO: なし / JWT 等を決定

## Dev-only Tooling (nightly)

- **rustdoc JSON**: `cargo +nightly rustdoc -- -Z unstable-options --output-format json`
  - 用途: `cargo make export-schema` — 対象 crate の pub API を JSON 抽出（TDDD type-signals 用）
  - crate 自体は stable のまま。nightly は rustdoc JSON 生成のみに使用
  - `rustdoc-types`: rustdoc JSON の公式 Rust 型定義（infrastructure 依存）
  - nightly 不在時は fail-closed
  - CI: nightly が必要なテストは `#[ignore]` + nightly 専用タスクで分離（将来）

## Version Baseline

- **最新調査日**: TODO: プロジェクト開始時に調査して記入
- **調査ログ**: TODO: `knowledge/research/version-baseline-YYYY-MM-DD.md` を作成して記入
- **反映対象**: `Cargo.toml`, `Dockerfile`, `Makefile.toml`（ツールバージョン指定がある場合）

## 変更履歴

| 日付 | 変更内容 | 理由 |
|------|---------|------|
| TODO | テンプレート初期化 | プロジェクト開始時に技術選定を合意形成するため |
