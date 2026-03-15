# Verification: STRAT-03 Phase 2 — Track State Machine の Rust 化

## Scope Verified

- [ ] 全タスク (T001-T010) の実装が spec.md の Acceptance Criteria を満たしている

## Manual Verification Steps

### CLI サブコマンド直接確認
- [ ] `sotp track add-task` で `--section` / `--after` によるタスク配置を確認し、plan.md に反映されることを確認
- [ ] `sotp track add-task` が不一致ブランチでエラーを返すことを確認（ブランチガード）
- [ ] `sotp track set-override` / `clear-override` でオーバーライドの設定/解除を確認
- [ ] `sotp track set-override` / `clear-override` が不一致ブランチでエラーを返すことを確認（ブランチガード）
- [ ] `sotp track next-task` が plan section 順で in_progress 優先の JSON を出力することを確認
- [ ] `sotp track task-counts` が total/todo/in_progress/done/skipped を含む JSON を出力することを確認

### Makefile wrapper 確認
- [ ] `cargo make track-add-task` が sotp track add-task を正しく呼び出すことを確認
- [ ] `cargo make track-next-task` が sotp track next-task を正しく呼び出すことを確認
- [ ] `cargo make track-task-counts` が sotp track task-counts を正しく呼び出すことを確認
- [ ] `cargo make track-set-override` が sotp track set-override を正しく呼び出すことを確認

### Python 削減確認
- [ ] `track_state_machine.py` が sotp なしでは動作しないことを確認（Python フォールバック除去）
- [ ] `cargo make ci` が通ることを確認

## Result / Open Issues

（実装完了後に記入）

## verified_at

（検証完了後に記入）
