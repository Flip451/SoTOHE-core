# Verification: STRAT-09 shell wrapper / cargo make 依存の縮退

## Scope Verified

- [ ] `Makefile.toml` の `script_runner = "@shell"` タスク一覧の確認
- [ ] 各タスクの移行前後の動作等価性
- [ ] quoting 脆弱性 (commit, note) の解消

## Manual Verification Steps

1. `cargo make ci` がグリーンであること
2. `cargo make commit "test message with 'quotes' and \"double quotes\""` が安全に動作すること
3. `cargo make note "note with 'quotes' and special $chars"` が安全に動作すること
4. `cargo make track-transition` が正しく引数をパースすること
5. `cargo make track-branch-create '<id>'` が正しく動作すること
6. `-exec` 系タスク (`WORKER_ID=w1 cargo make test-exec`) が分離動作すること
7. `bin/sotp make --help` がサブコマンド一覧を表示すること

## Result / Open Issues

### Phase 1 (T001 + T002): sotp make サブコマンド基盤 — Done

- `cargo make ci-rust` グリーン (fmt, clippy, nextest 743 tests pass, deny ok, check-layers ok)
- `bin/sotp make --help` が全26サブコマンドを表示
- `bin/sotp make track-sync-views` が正常ディスパッチ
- `bin/sotp make track-task-counts shell-wrapper-rust-2026-03-17` が JSON を返す
- `raw_args_to_single` / `raw_args_to_words` のユニットテスト 10 件パス
- clippy `indexing_slicing` を `.get()` パターンに修正済み
- 残タスク: T003–T014 (Phase 2–5)

## verified_at

- Phase 1: 2026-03-17
