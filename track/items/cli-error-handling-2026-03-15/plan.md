<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# ERR-09: 全層エラーハンドリング改善 — String エラー撲滅と CliError 導入

全層で蔓延する Result<_, String> を型付きエラーに置換し、CLI 層の eprintln! + ExitCode::FAILURE ボイラープレート (94箇所) を CliError + ? 演算子に統合する。
下層 (infrastructure/domain port/usecase) から型付きエラーを導入し、CLI 層で CliError として集約する。

## infrastructure 層の型付きエラー導入

GitRepository, GhClient, read_track_metadata の String エラーを thiserror ベースの型付きエラーに置換する。

- [ ] libs/infrastructure/src/git_cli.rs に GitError、gh_cli.rs に GhError を thiserror で定義。trait メソッドの戻り値を Result<_, String> から型付きエラーに変更。全アダプタ・呼び出し元を更新。
- [ ] libs/infrastructure/src/track/fs_store.rs の read_track_metadata を型付きエラーに変更。RepositoryError::Message catch-all を可能な限り構造化バリアントに置換。

## domain port + usecase 層の型付きエラー導入

WorktreeReader の String エラーを型付きに変更し、usecase 層の track_resolution, git_workflow, pr_workflow, review_workflow, worktree_guard を型付きエラーに移行する。

- [ ] libs/domain/src/repository.rs の WorktreeReader::porcelain_status を Result<String, String> から型付きエラーに変更。domain 層にエラー型を追加し、infrastructure アダプタを更新。
- [ ] track_resolution.rs, git_workflow.rs, pr_workflow.rs, review_workflow.rs, worktree_guard.rs の public 関数を型付きエラーに移行。map_err(|e| e.to_string()) による型情報消失を解消。

## CLI 層の CliError + ? 演算子統合

CliError enum を定義し、各コマンドの eprintln! + ExitCode::FAILURE パターンを ? 演算子に置換する。

- [ ] apps/cli/src/ に CliError enum を thiserror で定義。domain, usecase, infrastructure の各エラー型からの From impl を提供。ExitCode への変換メソッドを実装。
- [ ] commands/track/activate.rs (~28箇所), commands/track/transition.rs (9箇所), commands/track/resolve.rs (6箇所), commands/track/views.rs (2箇所) を CliError + ? に変換。注意: commands/lock.rs は JSON stderr 出力を維持するため CliError 統合対象外。commands/hook.rs は exit code プロトコル (0/1/2) を維持するため CliError 統合対象外。
- [ ] commands/git.rs (20箇所), commands/pr.rs (9箇所), commands/file.rs (3箇所), commands/review.rs (Result<_, String> パス), main.rs (4箇所) を CliError + ? に変換。commands/review.rs は独自の ReviewRunResult ワークフローを持つため、CliError 統合は内部ヘルパーの String エラーを型付きに変更する範囲に留める。

## CI 検証

- [ ] cargo make ci が通ることを確認。テスト数が変わらないことを検証。
