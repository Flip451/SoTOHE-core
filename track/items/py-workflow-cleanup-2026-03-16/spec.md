# STRAT-03 Phase 3: Git Workflow Python Scripts Cleanup

## Feature Goal

Rust 移行済みで呼び出し元がなくなった Python git workflow スクリプト群を削除し、ドキュメントの stale 参照を修正する。

## Scope

### In Scope

- `scripts/git_ops.py` の削除（`sotp git` に完全移行済み）
- `scripts/branch_switch.py` の削除（`sotp git switch-and-pull` に完全移行済み）
- `scripts/pr_merge.py` の削除（`sotp pr` に完全移行済み）
- `scripts/test_git_ops.py` の削除と `Makefile.toml` selftest リスト更新
- `scripts/test_make_wrappers.py` の `git_ops.py` 関連 fixture/stub 除去
- `.claude/commands/track/merge.md` の `scripts/pr_merge.py` 参照修正
- `track/workflow.md` の `git_ops.py` 参照修正

### Out of Scope

- `scripts/pr_review.py` の Rust 化（Phase 4 で対応）
- 完了済みトラックの成果物内の歴史的参照（`track/items/*/spec.md` 等）は修正しない
- Rust 側の機能追加・変更

## Constraints

- `cargo make ci` が通ること
- 既存の Rust テスト 517+ が全て通ること
- `scripts-selftest` が通ること（`test_git_ops.py` 除去後）

## Acceptance Criteria

- [ ] `scripts/git_ops.py`, `scripts/branch_switch.py`, `scripts/pr_merge.py` がリポジトリから削除されている
- [ ] `scripts/test_git_ops.py` が削除され、`Makefile.toml` の selftest リストから除去されている
- [ ] `scripts/test_make_wrappers.py` が `git_ops.py` に依存せず動作する
- [ ] `.claude/commands/track/merge.md` が `bin/sotp pr wait-and-merge` を参照している
- [ ] `track/workflow.md` が `sotp git note-from-file` を参照している
- [ ] `cargo make ci` が通る
