<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# STRAT-03 Phase 6: 残留 Python の optional utility 化

STRAT-03 Phase 6: .venv 未構築でも CI 必須経路が動作するよう、
Python タスクを optional utility に降格する。

## 孤立ファイル削除

孤立した Python verify スクリプトとテストファイルを削除し、
scripts-selftest の引数リストを更新する。

- [ ] 孤立 verify スクリプト削除: verify_plan_progress.py, verify_track_metadata.py, verify_track_registry.py
- [ ] 孤立テストファイル削除: test_verify_scripts.py, test_verify_latest_track_files.py + test_track_resolution.py の参照修正 + scripts-selftest 更新

## CI パス分離

ci-local の依存チェーンから Python タスクを分離し、
新タスク ci-python-local を追加する。
.venv 不在時の graceful skip を実装する。

- [ ] Makefile.toml: ci-local/ci-container から python-lint-local, scripts-selftest-local, hooks-selftest-local を分離
- [ ] Makefile.toml: ci-python-local/ci-python タスク追加（Python lint + selftest + hooks-selftest の optional gate）
- [ ] cargo make ci の compose wrapper 更新: .venv 存在チェック付き conditional Python gate

## Hook graceful degradation + ドキュメント

advisory hook が Python 不在時にクラッシュしない仕組みを追加し、
ドキュメントを更新する。

- [ ] advisory hook の Python 不在時 graceful skip: .claude/hooks/ の全 .py hook に python3 存在チェック追加
- [ ] ドキュメント更新: track/workflow.md, DEVELOPER_AI_WORKFLOW.md, .claude/rules/07-dev-environment.md, CLAUDE.md
