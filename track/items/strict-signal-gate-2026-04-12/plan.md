<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# Strict spec signal gate — Yellow blocks merge

既に実装済みの verify spec-states --strict (Yellow ブロック) を track-pr-merge の merge 前ガードに配線する。
spec signals の Yellow (inference/discussion ソース) が残っている PR のマージを阻止し、ADR/feedback/convention による根拠確定を促す。

## wait-and-merge への strict gate 配線

pr.rs の wait_and_merge (または dispatch_track_pr_merge) でタスク完了ガード直後に verify spec-states --strict を呼び出す。
strict gate が失敗した場合はマージを中止してエラーメッセージを表示する。

- [~] wait-and-merge のタスク完了ガード直後に verify spec-states --strict を呼び出す配線追加
