# Verification: rstest 導入

## 自動検証

- [ ] `cargo make ci` 通過
- [ ] `cargo make test` でテスト数が変換前と同等以上（469 tests baseline）

## 手動検証

- [ ] rstest が各 crate の `[dev-dependencies]` に追加されていること
- [ ] tech-stack.md に rstest が記載されていること
- [ ] `guard/policy.rs` の blocked/allowed テストがパラメータ化されていること
- [ ] `guard/parser.rs` の該当テストがパラメータ化されていること
- [ ] `libs/domain/src/track_phase.rs` の status/branch マトリクステストがパラメータ化されていること
- [ ] `libs/domain/src/lib.rs` の繰り返しパターンがパラメータ化されていること
- [ ] `track_resolution.rs` の reject_branchless 系がパラメータ化されていること
- [ ] `libs/usecase/src/hook.rs` の resolve_lock_mode テストがパラメータ化されていること
- [ ] `worktree_guard.rs` の parse_dirty_worktree_paths テストがパラメータ化されていること
- [ ] infrastructure crate の繰り返しテストがパラメータ化されていること（`track/render.rs`, `lock/fs_lock_manager.rs`, `gh_cli.rs`, `git_cli.rs`, `track/codec.rs`）
- [ ] cli crate の繰り返しテストがパラメータ化されていること（`commands/track/activate.rs`, `commands/git.rs`, `commands/review.rs`, `commands/pr.rs`）

## 結果

- verified_at: (未実施)
- 結果: (未実施)
