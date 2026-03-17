# Spec: STRAT-09 shell wrapper / cargo make 依存の縮退

## Goal

`Makefile.toml` の `script_runner = "@shell"` で記述されている安全性・状態管理系の shell wrapper を、`sotp` CLI (Rust) のサブコマンドに集約する。shell 文字列組み立てに起因する quoting 脆弱性・追跡困難性・条件分岐の脆さを根本解決する。

## Background

- `Makefile.toml` には 46 個の `script_runner = "@shell"` タスクが存在
- `$CARGO_MAKE_TASK_ARGS` の展開が unquoted な箇所で injection リスク
- 複雑な positional arg パース (`$1`, `$2`, `shift`) が shell で行われ、エラー診断が困難
- shell 文字列内のロジックが grep/LSP で追跡不能
- 関連項目: STRAT-09 (`tmp/TODO.md`), SEC-11, CON-08

## Scope

### In scope

- `bin/sotp` への薄い shell ラッパー (22件) の Rust CLI 直接呼び出し化
- オーケストレーション shell (`commit`, `note`, `track-commit-message`) の Rust 化
- `-exec` daemon ラッパー (8件) の `WORKER_ID` 処理の Rust 統一
- `sotp make` サブコマンドの新設（Makefile.toml からの呼び出しインターフェース）
- ドキュメント更新 (`track/workflow.md`, `.claude/rules/07-dev-environment.md`, `DESIGN.md`)

### Out of scope

- Docker compose ラッパー（shell ロジックなし、現状で安全）
- Python スクリプトラッパー (`guides-*`, `conventions-*`, `architecture-rules-*`)
- `bootstrap` スクリプト（一回限りの初期化、投資対効果低い）
- `-local` タスク（shell 不使用、`command` + `args` 形式）

## Constraints

- 既存の `cargo make <task>` インターフェースは維持する（後方互換）
- `bin/sotp` が未ビルドでもエラーメッセージが明確であること
- TDD: テストを先に書く（Red → Green → Refactor）
- `cargo make ci` が通ること

## Acceptance Criteria

1. `Makefile.toml` 内の `script_runner = "@shell"` タスクが Phase 1-4 のスコープ内で `command` + `args` 形式に置換されている
2. `sotp make` サブコマンドが clap derive で定義され、既存の sotp サブコマンドに内部ディスパッチできる
3. `commit` / `note` タスクの quoting 脆弱性が解消されている
4. 全タスクの動作が移行前と等価であること（手動検証）
5. `cargo make ci` がグリーン
6. ドキュメントが更新されている

## Related Conventions (Required Reading)

- `project-docs/conventions/security.md`
