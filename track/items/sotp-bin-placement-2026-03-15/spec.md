# Spec: sotp バイナリを bin/sotp に配置し cargo run を排除

## 概要

sotp CLI バイナリのビルド成果物を `bin/sotp` に配置し、Makefile.toml と hook の全呼び出しを統一する。テンプレート利用者は `cargo make bootstrap` で自動ビルド+配置される。

## 背景

- 現状: Makefile.toml の約 25 箇所が `cargo run --quiet -p cli --` で毎回ビルドチェック付き実行
- hook は `${SOTP_CLI_BINARY:-sotp}` で PATH 上の `sotp` を期待するが、開発中は PATH に存在しない
- テンプレートとして配布する際、利用者が `./bin/sotp` で直接実行できるのが望ましい
- `cargo run` は毎回 rustc の最新チェックが走り、数百 ms のオーバーヘッドがある

## ゴール

- `bin/sotp` にビルド済みバイナリを配置
- Makefile.toml のホスト側 wrapper タスク（非 `-local`）の `cargo run --quiet -p cli --` を `bin/sotp` に置換（コンテナ内 `-local` タスクは対象外）
- hook の `SOTP_CLI_BINARY` デフォルトを `bin/sotp` に変更
- `cargo make bootstrap` で自動ビルド+配置（Docker イメージビルド後に実行）
- `bin/sotp` は `.gitignore` で git 管理外（バイナリをリポジトリに入れない）

## スコープ

| ファイル | 変更内容 |
|----------|---------|
| `bin/.gitkeep` | 新規作成（ディレクトリ構造維持） |
| `.gitignore` | `/bin/sotp` 追加 |
| `Makefile.toml` | `build-sotp` タスク追加、bootstrap 統合、全 cargo run 置換 |
| `.claude/settings.json` | hook コマンドに `bin/sotp` 優先 + `cargo run` フォールバック |
| `.claude/docs/DESIGN.md` | SOTP_CLI_BINARY 参照箇所の更新 |
| `scripts/verify_orchestra_guardrails.py` | スニペット期待値更新 |

## 対象外

- crate 名の変更（`cli` → `sotp` リネームは別トラック）
- GitHub Releases でのプリビルドバイナリ配布（将来拡張）
- Docker コンテナ内のバイナリ配置（コンテナは引き続き cargo build で対応）

## 制約

- テンプレート利用者はホスト側に Rust toolchain を持つ前提（Rust 開発用テンプレートのため妥当）
- 対象プラットフォーム: Linux / macOS (POSIX)。Windows は対象外（WSL2 経由を想定）
- `bin/sotp` が存在しない状態でも `cargo make bootstrap` → `cargo make ci` が通ること
- `build-sotp` はホスト側で `cargo build -p cli --release` を実行（Docker 経由ではない — クロスプラットフォーム対応のため）
- コンテナ内タスク（`-local` サフィックス）は引き続き `cargo build` 成果物を直接使用
- ホスト側タスク（wrapper）のみ `bin/sotp` を参照
- hook timeout: fresh clone で `cargo run` フォールバックが cold build を伴う場合、既存の hook timeout (10-15s) を超える可能性がある。これは既存の問題（`sotp` が PATH にない場合も同様）であり、このトラックの scope 外。必要に応じて hook timeout の引き上げを別途検討する

## 完了条件

- [x] `bin/sotp` にバイナリが配置されること
- [x] Makefile.toml のホスト側 wrapper タスク（非 `-local`）に `cargo run --quiet -p cli` が残っていないこと（コンテナ内 `-local` タスクは対象外）
- [x] `.claude/settings.json` の hook が `bin/sotp` 存在時に優先し、不在時に `cargo run` にフォールバックすること
- [x] `cargo make bootstrap && cargo make ci` が通ること
