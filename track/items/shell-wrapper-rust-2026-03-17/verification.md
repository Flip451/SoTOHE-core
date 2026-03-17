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

### Phase 2 (T003–T006): 高優先度タスク移行 — Done

- 10 タスクを `script_runner = "@shell"` → `command = "bin/sotp"` + `args = ["make", "<task>", "${@}"]` に移行:
  - `commit`, `track-commit-message`, `note`, `track-note`
  - `track-transition`, `track-add-task`, `track-next-task`, `track-task-counts`
  - `track-set-override`, `track-sync-views`
- `scripts/test_make_wrappers.py` の 4 テストを新フォーマットに対応するよう更新
- `cargo make ci` グリーン (336 passed scripts, 245 passed hooks, 全 verify パス)
- レビュー済み P1 修正 (Phase 1 中):
  - `raw_args_to_words()` quoting 保持
  - `dispatch_track_commit_message()` stderr キャプチャ
  - CI exit code 伝播 (`dispatch_commit`, `dispatch_track_commit_message`)
  - `dispatch_exec()` 残余引数転送
  - `dispatch_track_pr_push/ensure()` 残余引数転送

## verified_at

- Phase 1: 2026-03-17
- Phase 2: 2026-03-17
