# Project Conventions

このディレクトリは、このテンプレートから作られた各プロジェクト固有の実装規約を管理するための場所です。

## Purpose

- テンプレート共通の運用ルールと、プロジェクト固有の実装ルールを分離する
- 人間と AI が参照する一次資料を固定する
- レビュー時に「何がルールで、何が実装の判断か」を追いやすくする

## Read Order

1. `track/tech-stack.md`
2. この `README.md` の `Current Files` を上から順に読む
3. 実装対象に直接関係する個別ルール文書を優先して再確認する

## Scope

ここに置くもの:

- アーキテクチャ制約
- ドメインモデル、データモデル、変換規約などの設計方針
- 計装、監視、トレース、メトリクスの実装方針
- エラー処理と境界での変換ルール
- テスト戦略と必須テスト観点
- 命名規則、ディレクトリ規約、生成コード運用などのプロジェクト固有ルール

ここに置かないもの:

- テンプレート共通のワークフロー
- 一時的な設計メモ
- 作業途中の比較メモ

## Maintenance Rules

- 新しい実装規約を追加したら、必要に応じてこの `README.md` の読み順や補足説明を更新する
- 既存ルールを破る例外を認める場合は、理由と適用範囲を必ず明記する
- `track/tech-stack.md` と矛盾する場合は、先に tech stack の決定を更新する
- `private/` や `config/secrets/` のような機密ディレクトリを新設する場合は、この配下に対応する規約文書を追加し、同時に `.claude/settings.json` の `Read` / `Grep` deny へ明示的なパスを追加する

## Suggested Files

必要なものだけ自由に追加すること。以下は例であり、固定ではない。

- `architecture.md`
- `instrumentation.md`
- `error-handling.md`
- `testing.md`
- `api-design.md`
- `naming.md`
- `generated-code.md`

## Current Files

この一覧は `cargo make conventions-add ...` により自動更新される。既知の主要カテゴリは推奨読順で並び、それ以外はファイル名順で並ぶ。

<!-- convention-docs:start -->
- `adr.md`: Convention: Architecture Decision Records (ADR)
- `bash-write-guard.md`: Bash File-Write Guard (CON-07)
- `filesystem-persistence-guard.md`: Filesystem Persistence Guard Convention
- `hexagonal-architecture.md`: Hexagonal Architecture Convention
- `impl-delegation-arch-guard.md`: Implementation Delegation Architecture Guard
- `prefer-type-safe-abstractions.md`: Prefer Type-Safe Abstractions Convention
- `security.md`: Security Convention
- `shell-parsing.md`: Shell Parsing Convention
- `source-attribution.md`: Source Attribution Convention
- `task-completion-flow.md`: Task Completion Flow
- `typed-deserialization.md`: Typed Deserialization Convention
<!-- convention-docs:end -->
