<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# track:activate gitignore 修正 + planner 移譲 cargo make コマンド追加

track:activate が gitignored な track/registry.md を staging しようとして失敗するバグの修正。
planner capability 移譲用の cargo make track-local-plan コマンドを追加し、briefing file 経由で Codex planner を呼び出せるようにする。

## Phase 1: activation gitignore 修正

GITIGNORED_RENDERED_VIEWS 定数と is_gitignored_rendered_view() ヘルパーを activate.rs に追加。
activation_commit_paths() を抽出し persist_activation_commit() で gitignored パスをフィルタ。
activation_artifact_paths() から registry.md を削除し spec.md を追加。
既存テスト更新と回帰テスト追加。
sync_rendered_views() のドキュメントコメントに VCS フィルタ責任を明示。

- [x] GITIGNORED_RENDERED_VIEWS 定数 + is_gitignored_rendered_view() ヘルパーを追加
- [x] activation_commit_paths() を抽出し、persist_activation_commit() でフィルタ適用
- [x] activation_artifact_paths() から registry.md 削除、spec.md 追加
- [x] 既存テスト更新 + 回帰テスト追加（gitignored path 除外の確認）
- [x] sync_rendered_views() にドキュメントコメント追加（VCS フィルタは呼び出し側の責任）

## Phase 2: planner 移譲インフラ

sotp plan codex-local サブコマンドを review codex-local と同じパターンで実装。
Makefile.toml に track-local-plan タスクを追加し permissions.allow に登録。
codex 移譲ドキュメント（02-codex-delegation.md, 10-guardrails.md）を更新し briefing file パターンを正式化。
scripts/test_make_wrappers.py に track-local-plan ラッパーの回帰テストケースを追加。

- [x] sotp plan codex-local サブコマンド実装（CLI に Plan サブコマンド追加 + --briefing-file / --prompt、codex subprocess spawn、stdout 出力）
- [x] Makefile.toml に track-local-plan タスク追加 + permissions.allow 登録
- [x] .claude/rules/02-codex-delegation.md と 10-guardrails.md のドキュメント更新（briefing file パターン正式化）
- [x] scripts/test_make_wrappers.py に track-local-plan ラッパーの回帰テストケースを追加
