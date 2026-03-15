# Verification: ERR-09 全層エラーハンドリング改善

## 自動検証

- [ ] `cargo make ci` 通過
- [ ] `cargo make test` でテスト数が変換前と同等以上（517 tests baseline）

## 手動検証

### infrastructure 層
- [ ] `libs/infrastructure/src/git_cli.rs` に `GitError` が定義され、trait メソッドが型付きエラーを返すこと
- [ ] `libs/infrastructure/src/gh_cli.rs` に `GhError` が定義され、trait メソッドが型付きエラーを返すこと
- [ ] `libs/infrastructure/src/track/fs_store.rs` の `read_track_metadata` が型付きエラーを返すこと

### domain port 層
- [ ] `libs/domain/src/repository.rs` の `WorktreeReader::porcelain_status` が `Result<_, String>` でないこと

### usecase 層
- [ ] `libs/usecase/src/track_resolution.rs` の public 関数が `Result<_, String>` でないこと
- [ ] `libs/usecase/src/git_workflow.rs` の public 関数が `Result<_, String>` でないこと
- [ ] `libs/usecase/src/pr_workflow.rs` の public 関数が `Result<_, String>` でないこと
- [ ] `libs/usecase/src/review_workflow.rs` の public 関数が `Result<_, String>` でないこと
- [ ] `libs/usecase/src/worktree_guard.rs` の public 関数が `Result<_, String>` でないこと

### CLI 層 — CliError 統合対象
- [ ] `apps/cli/src/` に `CliError` enum が定義されていること
- [ ] `commands/track/activate.rs` の `eprintln!` + `ExitCode::FAILURE` パターンが `?` 演算子に置換されていること
- [ ] `commands/track/transition.rs` の同パターンが `?` に置換されていること
- [ ] `commands/track/resolve.rs` の同パターンが `?` に置換されていること
- [ ] `commands/track/views.rs` の同パターンが `?` に置換されていること
- [ ] `commands/git.rs` の同パターンが `?` に置換されていること
- [ ] `commands/pr.rs` の同パターンが `?` に置換されていること
- [ ] `commands/file.rs` の同パターンが `?` に置換されていること
- [ ] `commands/review.rs` の内部ヘルパーの `Result<_, String>` が型付きエラーに変更されていること
- [ ] `main.rs` の同パターンが `?` に置換されていること

### CLI 層 — CliError 統合対象外（特殊ケース維持）
- [ ] `commands/lock.rs` の JSON stderr 出力パターンが維持されていること
- [ ] `commands/hook.rs` の exit code プロトコル (0/1/2) が維持されていること

## 結果

- verified_at: (未実施)
- 結果: (未実施)
