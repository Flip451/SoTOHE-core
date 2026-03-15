# Verification: rstest 導入

## 自動検証

- [ ] `cargo make ci` 通過
- [ ] `cargo make test` でテスト数が変換前と同等以上（469 tests baseline）

## 手動検証

- [ ] rstest が各 crate の `[dev-dependencies]` に追加されていること
- [ ] tech-stack.md に rstest が記載されていること
- [ ] `guard/policy.rs` の blocked/allowed テストがパラメータ化されていること
- [ ] `track_phase.rs` の status/branch マトリクステストがパラメータ化されていること
- [ ] `track_resolution.rs` の reject_branchless 系がパラメータ化されていること
- [ ] `hook.rs` の resolve_lock_mode テストがパラメータ化されていること
- [ ] infrastructure + cli crate の該当テストがパラメータ化されていること

## 結果

- verified_at: (未実施)
- 結果: (未実施)
