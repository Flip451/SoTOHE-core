# Spec: STRAT-03 Phase 6 — 残留 Python の optional utility 化

## Feature Goal

ホスト上で `.venv` 未構築でも advisory hook がクラッシュしない状態を達成する。CI (`cargo make ci`) は Docker コンテナ内で実行され Python は常に利用可能なため、CI パス自体の変更は不要。

## Scope

### In Scope

- 孤立 Python verify スクリプトの削除（`verify_plan_progress.py`, `verify_track_metadata.py`, `verify_track_registry.py`）
- テストファイル整理:
  - `test_verify_latest_track_files.py` を削除（対象スクリプト削除済み）
  - `test_verify_scripts.py` から全削除済みスクリプト（Phase 5 + Phase 6）のテストケースを除去（生存テスト維持）
  - `test_track_resolution.py` / `test_track_registry.py` / `test_track_schema.py` の削除済みスクリプト参照を修正
  - `test_track_resolution.py` を `scripts-selftest-local` に追加
- advisory hook の Python 不在時 graceful skip（`.claude/settings.json` の hook command にランチャーレベルのガード追加）
- ドキュメント更新（6ファイル）

### Out of Scope

- CI パスの変更（`ci-local` / `ci-container` は Docker 内で Python 常在のため変更不要）
- advisory hook の Rust 化（Hook は Python のまま維持）
- `external_guides.py`, `convention_docs.py`, `architecture_rules.py` の Rust 化（optional utility として継続）
- `track_schema.py`, `track_state_machine.py` 等の Python ライブラリ群の削除（テストスイートが依存）

## Constraints

- `cargo make ci` / `ci-rust` は変更なし
- advisory hook は Python 不在時に exit 0 で graceful skip（`command -v python3 || exit 0`）
- `scripts-selftest` / `hooks-selftest` は Docker コンテナ内で引き続き動作すること

## Acceptance Criteria

- [ ] 孤立 Python verify スクリプト 3 ファイルが削除されている
- [ ] `test_verify_latest_track_files.py` が削除されている
- [ ] `test_verify_scripts.py` から削除済みスクリプトのテストケースが除去され、`pytest scripts/test_verify_scripts.py` が pass する
- [ ] `test_track_resolution.py` の参照が修正され、`scripts-selftest-local` に追加されている
- [ ] advisory hook が `python3` 不在時にクラッシュせず graceful に skip する
- [ ] Docker コンテナ内で `cargo make scripts-selftest` / `hooks-selftest` が pass する
- [ ] 以下の 6 ドキュメントが更新されている:
  - `track/workflow.md`
  - `DEVELOPER_AI_WORKFLOW.md`
  - `.claude/rules/07-dev-environment.md`
  - `.claude/rules/09-maintainer-checklist.md`
  - `CLAUDE.md`
  - `LOCAL_DEVELOPMENT.md`
