<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# STRAT-03 Phase 3: Git workflow Python scripts cleanup

STRAT-03 Phase 3 のクリーンアップトラック。
git_ops.py / branch_switch.py / pr_merge.py の Rust 移行は既に完了済み（sotp git/pr サブコマンド）。
残留するデッドコード Python スクリプトの削除とドキュメントの stale 参照修正を行う。

## Python スクリプト削除

呼び出し元がなくなった Python スクリプトとそのテストを削除し、Makefile.toml の selftest リストを更新する。

- [x] Delete dead Python scripts: git_ops.py, branch_switch.py, pr_merge.py
- [x] Delete test_git_ops.py and remove from Makefile.toml scripts-selftest list
- [x] Update test_make_wrappers.py: remove git_ops.py fixture/stub references

## ドキュメント修正

Python スクリプト参照が残っているドキュメントを Rust CLI (bin/sotp) 参照に更新する。

- [x] Fix stale docs: .claude/commands/track/merge.md and track/workflow.md Python references

## 完了処理

TODO.md の進捗更新と CI 通過確認。

- [x] Update tmp/TODO.md: mark STRAT-03 Phase 3 as complete
- [x] Verify cargo make ci passes
