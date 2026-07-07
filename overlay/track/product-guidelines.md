# Product Guidelines

> このファイルは `/track:setup` 時に設定します。
> 開発判断の基準として使用します。
> `TODO:` はプロジェクトに合わせて埋めてください。汎用的な品質基準はそのまま利用できます。

## Design Principles

1. TODO: プロダクト固有の設計原則その1
2. TODO: プロダクト固有の設計原則その2
3. TODO: プロダクト固有の設計原則その3

## API Design Guidelines

- TODO: 出力形式・API 設計の方針を記入

## Error Handling Guidelines

- ユーザー向けエラーは明確で行動可能なメッセージを含める
- 内部エラーの詳細はログに記録し、ユーザーには抽象化されたメッセージを返す
- すべての公開エラーは `# Errors` セクション付きでドキュメント化する

## Performance Guidelines

- TODO: 応答時間・スループット等の性能目標を記入

## Security Guidelines

- シークレットはハードコードしない
- すべての外部入力をドメイン型で検証する
- 詳細なエラーをユーザーに露出しない
- TODO: プロダクト固有のセキュリティ要件があれば追記

## Code Quality Standards

> 以下は汎用的な Rust 品質基準です。プロジェクトに合わせて調整してください。

- `cargo make clippy` がクリーンであること
- `cargo make fmt-check` を通過すること
- 新規コードのカバレッジ 80% 以上を目標とする
- すべての `pub` 項目に `///` ドキュメントを付ける
