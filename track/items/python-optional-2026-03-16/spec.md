# Spec: STRAT-03 Phase 6 — 残留 Python の optional utility 化

## Feature Goal

`.venv` 未構築でも track workflow と CI の必須経路（`cargo make ci`）が壊れない状態を達成する。Python は advisory hook と optional utility に限定する。

## Scope

### In Scope

- 孤立 Python verify スクリプトの削除（`verify_plan_progress.py`, `verify_track_metadata.py`, `verify_track_registry.py`）
- 孤立テストファイルの削除（`test_verify_scripts.py`, `test_verify_latest_track_files.py`）
  - 注: `test_track_resolution.py` は `track_resolution.py`（共有ライブラリ）の回帰テストを含むため保持。削除済みスクリプトへの参照のみ修正。
- `ci-local` から Python 必須タスクを分離:
  - `python-lint-local` を `ci-local` の必須依存から外す（`.venv` 存在時のみ実行）
  - `scripts-selftest-local` を `ci-local` の必須依存から外す
  - `hooks-selftest-local` を `ci-local` の必須依存から外す
- 新タスク `ci-python-local` の追加（Python lint + selftest + hooks-selftest をまとめた optional gate）
- `cargo make ci` compose wrapper の更新
- ドキュメント更新（`track/workflow.md`, `DEVELOPER_AI_WORKFLOW.md` 等）

### Out of Scope

- advisory hook の Rust 化（Hook は Python のまま維持。Phase 7 以降の課題）
- `external_guides.py`, `convention_docs.py`, `architecture_rules.py` の Rust 化（optional utility として継続）
- `track_schema.py`, `track_state_machine.py` 等の Python ライブラリ群の削除（テストスイートが依存）

## Constraints

- `cargo make ci-rust` は変更なし（Rust のみの CI パス）
- `cargo make ci` は `.venv` なしでも成功すること（Python タスクをスキップ）
- `.venv` がある環境では `cargo make ci-python` で Python 品質ゲートを実行可能
- advisory hook は `.venv` なしでもクラッシュしない（Python 不在時は graceful skip）
- `scripts-selftest` / `hooks-selftest` は引き続き Docker コンテナ内で実行可能

## Acceptance Criteria

- [ ] `cargo make ci` が `.venv` 未構築環境で成功する
- [ ] `cargo make ci-python` が Python lint + selftest + hooks-selftest を実行する
- [ ] 孤立 Python verify スクリプト 3 ファイルが削除されている
- [ ] 孤立テストファイル 2 ファイルが削除されている（`test_track_resolution.py` は保持・修正）
- [ ] advisory hook が Python 不在時にクラッシュせず graceful に skip する
- [ ] Docker コンテナ内で `scripts-selftest` / `hooks-selftest` が引き続き動作する
- [ ] `test_track_resolution.py` の削除済みスクリプト参照が修正され、`scripts-selftest-local` に追加されている
- [ ] `.claude/settings.json` permissions.allow に `ci-python` 関連タスクが追加されている
- [ ] `cargo make bootstrap` が `ci-python-local` を含む
- [ ] 以下のドキュメントが更新されている:
  - `track/workflow.md`
  - `DEVELOPER_AI_WORKFLOW.md`
  - `.claude/rules/07-dev-environment.md`
  - `.claude/rules/09-maintainer-checklist.md`
  - `CLAUDE.md`
  - `LOCAL_DEVELOPMENT.md`
