# Product Guidelines

> このファイルは `/track:setup` 時に設定します。
> 開発判断の基準として使用します。

## Design Principles

1. **SoT + CQRS**: 限られた SoT からビューとなる md ファイルを生成する。ビューへの直接書き込みは禁じる
2. **CLI ファースト**: 操作はすべて CLI コマンドで完結する
3. **AST ベースの厳格なガードレール**: hooks で AST を用いた検証を提供する

## API Design Guidelines

- 出力は基本 JSON 形式とする
- md ファイルなどのビュー用ファイル出力も行う

## Error Handling Guidelines

- ユーザー向けエラーは明確で行動可能なメッセージを含める
- 内部エラーの詳細はログに記録し、ユーザーには抽象化されたメッセージを返す
- すべての公開エラーは `# Errors` セクション付きでドキュメント化する

## Performance Guidelines

- CLI コマンドの応答は 500ms 以内

## Security Guidelines

- シークレットはハードコードしない
- すべての外部入力をドメイン型で検証する
- SQL クエリはパラメータバインドを使用する
- 詳細なエラーをユーザーに露出しない

## Code Quality Standards

- `cargo make clippy` がクリーンであること
- `cargo make fmt-check` を通過すること
- 新規コードのカバレッジ 80% 以上を目標とする
- すべての `pub` 項目に `///` ドキュメントを付ける
