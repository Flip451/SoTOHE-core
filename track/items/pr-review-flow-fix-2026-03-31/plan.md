<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# Phase D: PR review flow fix (WF-66/ERR-08)

push() と review_cycle() から check_task_completion_guard を削除し、wait_and_merge_with() のみで実行するよう移動する (WF-66)。
review_cycle() の trigger state を JSON ファイルに永続化し --resume で再開可能にする (ERR-08)。

## タスク完了ガードの移動 (WF-66)

pr.rs の push() (line 170-175) と review_cycle() (line 657-662) から check_task_completion_guard 呼び出しを削除。
wait_and_merge_with() の冒頭 (merge 前) に同ガードを追加。
これにより中間 push と PR review が未完了タスクでもブロックされなくなり、merge のみがタスク完了を要求する。

- [x] check_task_completion_guard を push() と review_cycle() から削除し wait_and_merge_with() に移動
- [x] track-pr-merge のみがタスク完了を要求するよう SKILL.md (pr-review.md, merge.md) を更新
- [x] check_approved: review未開始(NotStarted/no review.json)はコミット許可

## PR review 中断耐性 (ERR-08)

review_cycle() で @codex review をポストした直後に trigger state (PR番号, comment ID, trigger_timestamp, head_hash) を tmp/pr-review-state/<track-id>.json に保存。
新しい --resume フラグで既存の state ファイルからポーリングを再開可能にする。
state ファイルが存在しない場合や古い場合は通常フローにフォールバック。

- [x] review_cycle() の trigger state (PR番号, comment ID, timestamp) を tmp/pr-review-state/<track-id>.json に永続化
- [x] track-pr-review --resume サブコマンド追加 (永続化 state から poll を再開)

## テスト

push() が未完了タスクでも成功すること、wait_and_merge_with() が未完了タスクでブロックすることを検証。
trigger state の永続化と復元の往復テスト。

- [x] テスト: 未完了タスクがある状態で push + PR review が通ること、wait_and_merge で未完了タスクがブロックされること
