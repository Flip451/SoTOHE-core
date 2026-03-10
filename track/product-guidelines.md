# Product Guidelines

> このファイルは `/track:setup` 時に設定します。
> 開発判断の基準として使用します。

## Design Principles

1. **TODO: 原則 1**: TODO: 説明
2. **TODO: 原則 2**: TODO: 説明
3. **TODO: 原則 3**: TODO: 説明

## API Design Guidelines

- TODO: API設計のガイドライン 1
- TODO: API設計のガイドライン 2

## Error Handling Guidelines

- ユーザー向けエラーは明確で行動可能なメッセージを含める
- 内部エラーの詳細はログに記録し、ユーザーには抽象化されたメッセージを返す
- すべての公開エラーは `# Errors` セクション付きでドキュメント化する

## Performance Guidelines

- TODO: パフォーマンス基準 1
- TODO: パフォーマンス基準 2

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
