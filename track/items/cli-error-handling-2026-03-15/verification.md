# Verification: ERR-09 全層エラーハンドリング改善

## 自動検証

- [x] `cargo make ci` 通過
- [x] `cargo make test` でテスト数が変換前と同等以上（517 tests baseline → 517 tests）

## 手動検証

### infrastructure 層
- [x] `libs/infrastructure/src/git_cli.rs` に `GitError` が定義され、trait メソッドが型付きエラーを返すこと
- [x] `libs/infrastructure/src/gh_cli.rs` に `GhError` が定義され、trait メソッドが型付きエラーを返すこと
- [x] `libs/infrastructure/src/track/fs_store.rs` の `read_track_metadata` が型付きエラーを返すこと

### domain port 層
- [x] `libs/domain/src/repository.rs` の `WorktreeReader::porcelain_status` が `Result<_, String>` でないこと

### usecase 層
- [x] `libs/usecase/src/track_resolution.rs` の public 関数が `Result<_, String>` でないこと
- [x] `libs/usecase/src/git_workflow.rs` の public 関数が `Result<_, String>` でないこと
- [x] `libs/usecase/src/pr_workflow.rs` の public 関数が `Result<_, String>` でないこと
- [x] `libs/usecase/src/review_workflow.rs` の public 関数が `Result<_, String>` でないこと
- [x] `libs/usecase/src/worktree_guard.rs` の public 関数が `Result<_, String>` でないこと

### CLI 層 — CliError 統合対象
- [x] `apps/cli/src/` に `CliError` enum が定義されていること
- [x] `commands/track/activate.rs` の `eprintln!` + `ExitCode::FAILURE` パターンが `?` 演算子に置換されていること
- [x] `commands/track/transition.rs` の同パターンが `?` に置換されていること
- [x] `commands/track/resolve.rs` の同パターンが `?` に置換されていること
- [x] `commands/track/views.rs` の同パターンが `?` に置換されていること
- [x] `commands/git.rs` の同パターンが `?` に置換されていること
- [x] `commands/pr.rs` の同パターンが `?` に置換されていること
- [x] `commands/file.rs` の同パターンが `?` に置換されていること
- [x] `commands/review.rs` の内部ヘルパーの `Result<_, String>` が型付きエラーに変更されていること — review.rs は ReviewRunResult パイプラインを維持。render_review_payload は ReviewWorkflowError を使用。
- [x] `main.rs` の同パターンが `?` に置換されていること

### CLI 層 — CliError 統合対象外（特殊ケース維持）
- [x] `commands/lock.rs` の JSON stderr 出力パターンが維持されていること
- [x] `commands/hook.rs` の exit code プロトコル (0/1/2) が維持されていること

## 結果

- verified_at: 2026-03-15
- 結果: 全検証項目パス。cargo make ci 通過、全 517 テスト通過。
