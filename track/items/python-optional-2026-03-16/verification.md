# Verification: STRAT-03 Phase 6 — 残留 Python の optional utility 化

## Scope Verified

- [ ] `.venv` 未構築で `cargo make ci` が成功する
- [ ] `cargo make ci-python` が Python 品質ゲートを実行する
- [ ] 孤立 verify スクリプト 3 件が削除されている
- [ ] 孤立テストファイル 2 件が削除されている（`test_track_resolution.py` は保持・修正済み）
- [ ] `test_track_resolution.py` の削除済みスクリプト参照が修正されている
- [ ] advisory hook が Python 不在時に graceful skip する
- [ ] Docker コンテナ内で `scripts-selftest` / `hooks-selftest` が引き続き動作する
- [ ] 以下の 4 ドキュメントが更新されている:
  - `track/workflow.md`
  - `DEVELOPER_AI_WORKFLOW.md`
  - `.claude/rules/07-dev-environment.md`
  - `CLAUDE.md`

## Manual Verification Steps

1. `.venv` を一時的に退避（`mv .venv .venv.bak`）して `cargo make ci` を実行 — pass
2. `.venv` 復帰後に `cargo make ci-python` を実行 — pass
3. 孤立 verify スクリプト 3 件（`verify_plan_progress.py`, `verify_track_metadata.py`, `verify_track_registry.py`）が `scripts/` に存在しないことを確認
4. 孤立テストファイル 2 件（`test_verify_scripts.py`, `test_verify_latest_track_files.py`）が `scripts/` に存在しないことを確認
5. `test_track_resolution.py` が存在し、`pytest scripts/test_track_resolution.py` が pass することを確認
6. advisory hook が `.venv` 退避状態で Claude Code セッション内でクラッシュしないことを確認
7. Docker コンテナ内で `cargo make scripts-selftest` と `cargo make hooks-selftest` が pass することを確認
8. 4 ドキュメント（`track/workflow.md`, `DEVELOPER_AI_WORKFLOW.md`, `.claude/rules/07-dev-environment.md`, `CLAUDE.md`）に新しい CI パス構造が反映されていることを確認

## Result

(未検証)

## Open Issues

(なし)

## verified_at

(未検証)
