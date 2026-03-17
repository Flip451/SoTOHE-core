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

### Phase 3 (T007–T010): 中優先度 arg フォワーディング移行 — Done

- 18 タスクを `script_runner = "@shell"` → `command + args` に移行:
  - `track-branch-create`, `track-branch-switch`, `track-activate` (T007)
  - `track-pr-push`, `track-pr-ensure`, `track-pr`, `track-pr-review`, `track-pr-merge`, `track-pr-status` (T008)
  - `track-plan-branch` (T009)
  - `track-resolve`, `track-switch-main`, `track-add-paths`, `add-all` (T010)
- `track-local-review` は shell `"$@"` wrapper 維持 (multi-word arg quoting 保護)
- Python テスト: 4 テスト更新 + 2 テスト新規追加
- `cargo make ci` グリーン
- `bin/sotp make track-resolve` スモークテスト確認

### Phase 4 (T011–T012): -exec daemon ラッパー統一 — Skipped

- **スキップ理由**: `-exec` 系タスクは `tools-daemon` 常駐前提だが、現在のワークフローでは `run --rm` (cargo make ci 等) で十分。daemon 常駐の投資対効果が低い:
  - `run --rm` オーバーヘッドは ~2-3秒/回で体感的に十分速い
  - daemon stale 状態やキャッシュ不整合のデバッグコストが増える
  - 並列ワーカーは Codex CLI に委譲しており、Claude Code 側の `-exec` 多重実行の場面が少ない
- **将来方針**: Agent Teams の並列度が上がって `run --rm` がボトルネックになったら改めて検討
- `dispatch_exec()` は Phase 1 で実装済み。Makefile.toml 側の移行のみ未実施

### Phase 5 (T013–T014): ドキュメント・CI 更新 — Done

- T013: `script_runner = "@shell"` 残存監査
  - 残存 27 件を分類: -exec 8件 (Phase 4 skipped), bootstrap 1件, -local 8件, shell "$@" wrapper 3件, host/internal 7件
  - 全て spec の out of scope または intentional shell wrapper として妥当
- T014: `.claude/rules/07-dev-environment.md` に `sotp make` dispatch セクション追加 (英語)
  - cargo make → bin/sotp make の委譲関係と multi-word arg 制約を記載

## verified_at

- Phase 1: 2026-03-17
- Phase 2: 2026-03-17
- Phase 3: 2026-03-17
- Phase 4: 2026-03-17 (skipped)
- Phase 5: 2026-03-17
