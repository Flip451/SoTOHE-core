# Spec: STRAT-03 Phase 6 — 残留 Python の optional utility 化

## Feature Goal

`.venv` 未構築でも track workflow と CI の必須経路（`cargo make ci`）が壊れない状態を達成する。Python は advisory hook と optional utility に限定する。

## Scope

### In Scope

- 孤立 Python verify スクリプトの削除（`verify_plan_progress.py`, `verify_track_metadata.py`, `verify_track_registry.py`）
- テストファイル整理:
  - `test_verify_latest_track_files.py` を削除（対象スクリプト削除済み）
  - `test_verify_scripts.py` から削除済みスクリプト（Phase 5 + Phase 6 の全削除対象）のテストケースを除去（生存する verifier テストは維持）。対象: `verify_latest_track_files.py`, `verify_tech_stack_ready.py`, `verify_architecture_docs.py`, `verify_orchestra_guardrails.py`, `verify_plan_progress.py`, `verify_track_metadata.py`, `verify_track_registry.py` への参照
  - `test_track_resolution.py` は `track_resolution.py`（共有ライブラリ）の回帰テストを含むため保持。削除済みスクリプトへの参照のみ修正。
- CI パスの整理:
  - `ci-local`/`ci-container` は Python タスクを維持（Docker コンテナ内は Python 常在）
  - ホスト用に `ci-no-python-local` を追加（`.venv` 不在時のフォールバック）
  - `cargo make ci` compose wrapper に `.venv` 存在チェック付き条件分岐を追加
- 新タスク `ci-python-local`/`ci-python` の追加（ホスト向け optional Python gate）
- `test_verify_scripts.py` は削除ではなく、削除済みスクリプトのテストケースのみ除去（生存テスト維持）
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
- [ ] `test_verify_latest_track_files.py` が削除されている
- [ ] `test_verify_scripts.py` から削除済みスクリプトのテストケースが除去されている（生存テスト維持）
- [ ] advisory hook が Python 不在時にクラッシュせず graceful に skip する
- [ ] Docker コンテナ内で `scripts-selftest` / `hooks-selftest` が引き続き動作する
- [ ] `test_track_resolution.py` の削除済みスクリプト参照が修正され、`scripts-selftest-local` に追加されている
- [ ] `.claude/settings.json` permissions.allow に `ci-python` 関連タスクが追加されている
- [ ] `cargo make bootstrap` が venv 構築後に `ci-python-local` を実行する（依存ではなくスクリプト内呼び出し）
- [ ] 以下のドキュメントが更新されている:
  - `track/workflow.md`
  - `DEVELOPER_AI_WORKFLOW.md`
  - `.claude/rules/07-dev-environment.md`
  - `.claude/rules/09-maintainer-checklist.md`
  - `CLAUDE.md`
  - `LOCAL_DEVELOPMENT.md`
