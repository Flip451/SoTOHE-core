# Project Conventions

このディレクトリは、このテンプレートから作られたプロジェクト固有の実装規約を管理するための場所です。

## Ownership

ここに置かれる規約はすべて **このプロジェクトが所有します**。テンプレートは初期値として `Current Files` に並ぶ規約を供給しますが、以後の改稿・改名・削除はプロジェクトの裁量です。ハーネスが所有するのは、規約を capability へ届ける解決機構 (`required_for` frontmatter とその resolver) と、規約本文が案内する検査コマンドの実装だけであり、**どう書くか** という方針そのものは各文書にあります。

供給された規約を全面的に破棄しても構いません。詳しくは各文書冒頭の「この文書の所有権」節を参照してください。

> **強制先**: review 観点 — harness-policy scope

## Purpose

- テンプレート共通の運用ルールと、プロジェクト固有の実装ルールを分離する
- 人間と AI が参照する一次資料を固定する
- レビュー時に「何がルールで、何が実装の判断か」を追いやすくする

## Read Order

1. `knowledge/adr/README.md` (pre-track ADR 索引 — 技術選定・製品方針の決定)
2. この `README.md` の `Current Files` を上から順に読む
3. 実装対象に直接関係する個別ルール文書を優先して再確認する

> **強制先**: review 観点 — harness-policy scope

## Scope

ここに置くもの:

- アーキテクチャ制約
- ドメインモデル、データモデル、変換規約などの設計方針
- 計装、監視、トレース、メトリクスの実装方針
- エラー処理と境界での変換ルール
- テスト戦略と必須テスト観点
- 命名規則、ディレクトリ規約、生成コード運用などのプロジェクト固有ルール

> **強制先**: review 観点 — harness-policy scope

ここに置かないもの:

- テンプレート共通のワークフロー
- 一時的な設計メモ
- 作業途中の比較メモ

> **強制先**: review 観点 — harness-policy scope

## Maintenance Rules

- 新しい実装規約を追加したら、必要に応じてこの `README.md` の読み順や補足説明を更新する
  > **強制先**: review 観点 — harness-policy scope
- 既存ルールを破る例外を認める場合は、理由と適用範囲を必ず明記する
  > **強制先**: review 観点 — harness-policy scope
- 技術選定の決定 (ADR) と矛盾する場合は、先に ADR を supersede してから規約を更新する
  > **強制先**: review 観点 — adr / harness-policy scope
- `private/` や `config/secrets/` のような機密ディレクトリを新設する場合は、この配下に対応する規約文書を追加し、同時に `.claude/settings.json` の `Read` / `Grep` deny へ明示的なパスを追加する
  > **強制先**: review 観点 — harness-policy scope

## Suggested Files

必要なものだけ自由に追加すること。以下は例であり、固定ではない。

> **強制先**: review 観点 — harness-policy scope

- `architecture.md`
- `instrumentation.md`
- `error-handling.md`
- `api-design.md`
- `naming.md`
- `generated-code.md`

## Current Files

この一覧は `bin/sotp conventions add ...` / `bin/sotp conventions update-index` により自動更新される。既知の主要カテゴリは推奨読順で並び、それ以外はファイル名順で並ぶ。

<!-- convention-docs:start -->
- `testing.md`: Testing Convention
- `coding-principles.md`: Coding Principles Convention
- `enforce-by-mechanism.md`: Enforce by Mechanism Convention
- `environment-assumptions.md`: Environment Assumptions Convention
- `prefer-type-safe-abstractions.md`: Prefer Type-Safe Abstractions Convention
- `security.md`: Security Convention
- `type-designer-kind-selection.md`: Type-Designer Kind Selection Convention
<!-- convention-docs:end -->

> **強制先**: 機械 lint — bin/sotp conventions verify-index
