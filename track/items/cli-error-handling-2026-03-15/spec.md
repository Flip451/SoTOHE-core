# Spec: ERR-09 全層エラーハンドリング改善

## 概要

全層で蔓延する `Result<_, String>` を型付きエラーに置換し、CLI 層の `eprintln!` + `ExitCode::FAILURE` ボイラープレート (94箇所) を `CliError` + `?` 演算子に統合する。

## 背景

- CLI 層: 94 箇所の `match ... Err(e) => { eprintln!(...); return ExitCode::FAILURE; }` パターン
- infrastructure 層: `GitRepository` (5+ メソッド), `GhClient` (4 メソッド), `read_track_metadata` が `Result<_, String>`
- domain port: `WorktreeReader::porcelain_status` が `Result<String, String>`
- usecase 層: `track_resolution`, `git_workflow`, `pr_workflow`, `review_workflow`, `worktree_guard` が `Result<_, String>`
- エラーの型情報が `map_err(|e| e.to_string())` で消失し、パターンマッチによる回復やログの構造化が不可能

## ゴール

- 全層の `Result<_, String>` を `thiserror` ベースの型付きエラーに置換
- CLI 層に `CliError` enum を導入し、94箇所のボイラープレートを `?` 演算子に統合
- エラーメッセージのフォーマットを統一
- 既存テストが全て通ること

## スコープ

### infrastructure 層

| ファイル | 変更内容 |
|----------|---------|
| `libs/infrastructure/src/git_cli.rs` | `GitError` 定義、trait メソッド戻り値変更 |
| `libs/infrastructure/src/gh_cli.rs` | `GhError` 定義、trait メソッド戻り値変更 |
| `libs/infrastructure/src/track/fs_store.rs` | `read_track_metadata` 型付きエラー化、`RepositoryError::Message` 構造化 |

### domain port 層

| ファイル | 変更内容 |
|----------|---------|
| `libs/domain/src/repository.rs` | `WorktreeReader::porcelain_status` の String エラー除去 |

### usecase 層

| ファイル | 変更内容 |
|----------|---------|
| `libs/usecase/src/track_resolution.rs` | `Result<_, String>` → 型付きエラー |
| `libs/usecase/src/git_workflow.rs` | `Result<_, String>` → 型付きエラー |
| `libs/usecase/src/pr_workflow.rs` | `Result<_, String>` → 型付きエラー |
| `libs/usecase/src/review_workflow.rs` | `Result<_, String>` → 型付きエラー |
| `libs/usecase/src/worktree_guard.rs` | `Result<_, String>` → 型付きエラー |

### CLI 層

| ファイル | eprintln 箇所数 |
|----------|----------------|
| `apps/cli/src/commands/track/activate.rs` | ~28 |
| `apps/cli/src/commands/git.rs` | 20 |
| `apps/cli/src/commands/lock.rs` | 11 |
| `apps/cli/src/commands/pr.rs` | 9 |
| `apps/cli/src/commands/track/transition.rs` | 9 |
| `apps/cli/src/commands/track/resolve.rs` | 6 |
| `apps/cli/src/commands/file.rs` | 3 |
| `apps/cli/src/commands/track/views.rs` | 2 |
| `apps/cli/src/commands/hook.rs` | 2 |
| `apps/cli/src/main.rs` | 4 |

### 対象外

- テストコード内の `unwrap()` / `expect()` — テスト内は許可済み
- `commands/hook.rs` の特殊な exit code ロジック (exit 0/1/2) — hook protocol に依存するため CliError 統合対象外
- `commands/lock.rs` の JSON stderr 出力フォーマット — machine-readable 出力として CliError 統合対象外
- `commands/review.rs` — 独自の `ReviewRunResult` ワークフローを持つため、CliError 統合は内部ヘルパーの `Result<_, String>` を型付きに変更する範囲に留める

## 制約

- 下層 (infrastructure → domain port → usecase) から先に変更し、CLI 層を最後にする
- `thiserror` は既に workspace dependency として利用可能
- アーキテクチャの依存方向 (domain ← usecase ← infrastructure ← cli) を維持する
- エラー型の `Display` 実装がユーザー向けメッセージとして適切であること

## 完了条件

- [ ] infrastructure 層に `Result<_, String>` が残っていないこと
- [ ] domain port に `Result<_, String>` が残っていないこと
- [ ] usecase 層に `Result<_, String>` が残っていないこと
- [ ] CLI 層に `CliError` が導入され、`eprintln!` + `ExitCode::FAILURE` パターンが大幅に削減されていること
- [ ] `cargo make ci` が通ること
