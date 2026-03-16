<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# STRAT-03 Phase 6: 残留 Python の optional utility 化

STRAT-03 Phase 6: ホスト上で .venv 未構築でも advisory hook がクラッシュしない状態を達成する。
CI (Docker コンテナ内) は Python 常在のため変更不要。孤立 Python ファイルを削除し、hook のランチャーにガードを追加する。

## 孤立ファイル削除・テスト整理

孤立した Python verify スクリプトとテストファイルを削除し、
test_verify_scripts.py から削除済みスクリプトへの参照を除去する。

- [x] 孤立 verify スクリプト削除: verify_plan_progress.py, verify_track_metadata.py, verify_track_registry.py
- [x] テストファイル整理: test_verify_latest_track_files.py 削除 + test_verify_scripts.py から全削除済みスクリプト(Phase5+6)のテストケース除去(生存テスト維持) + test_track_resolution.py/test_track_registry.py/test_track_schema.py の参照修正 + test_track_resolution.py を scripts-selftest-local に追加

## Hook graceful degradation + ドキュメント

advisory hook が Python 不在時にクラッシュしない仕組みを追加し、
ドキュメントを更新する。
注: cargo make ci は Docker コンテナ内で実行され Python は常に利用可能。CI パス自体の変更は不要。

- [x] advisory hook の Python 不在時 graceful skip: .claude/settings.json の hook command を python3 存在チェック付きランチャーに変更 (command -v python3 || exit 0) + .claude/hooks/ 内の個別 hook にもガード追加
- [x] ドキュメント更新: track/workflow.md, DEVELOPER_AI_WORKFLOW.md, .claude/rules/07-dev-environment.md, .claude/rules/09-maintainer-checklist.md, CLAUDE.md, LOCAL_DEVELOPMENT.md + 削除済み Python verifier への参照修正
