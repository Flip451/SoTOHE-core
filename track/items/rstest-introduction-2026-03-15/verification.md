# Verification: rstest 導入

## 自動検証

- [x] `cargo make ci` 通過
- [x] `cargo make test` でテスト数が変換前と同等以上（469 baseline → 517 after rstest expansion）

## 手動検証

- [x] rstest が各 crate の `[dev-dependencies]` に追加されていること
- [x] tech-stack.md に rstest が記載されていること
- [x] `guard/policy.rs` の blocked/allowed テストがパラメータ化されていること
- [x] `guard/parser.rs` の該当テストがパラメータ化されていること
- [x] `libs/domain/src/track_phase.rs` の status/branch マトリクステストがパラメータ化されていること
- [x] `libs/domain/src/lib.rs` の繰り返しパターンがパラメータ化されていること
- [x] `track_resolution.rs` の reject_branchless 系がパラメータ化されていること
- [x] `libs/usecase/src/hook.rs` の resolve_lock_mode テストがパラメータ化されていること
- [x] `worktree_guard.rs` の parse_dirty_worktree_paths テストがパラメータ化されていること
- [x] infrastructure crate の繰り返しテストがパラメータ化されていること（`gh_cli.rs`, `git_cli.rs`）
- [x] cli crate の繰り返しテストがパラメータ化されていること（`commands/track/activate.rs`）

## 結果

- verified_at: 2026-03-15
- 結果: 全項目パス。rstest 0.26.1 を全 4 crate に導入済み。469 → 517 tests（パラメータ展開による増加）。`cargo make ci` 全通過。
- 備考: infrastructure の `track/render.rs`, `lock/fs_lock_manager.rs`, `track/codec.rs` および cli の `commands/git.rs`, `commands/review.rs`, `commands/pr.rs` はテストパターンが十分にユニークであり、パラメータ化の恩恵が小さいため現状維持とした。
