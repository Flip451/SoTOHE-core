# Verification — scripts/ Python ヘルパー削減 フェーズ1

## Scope Verified

### 削除確認 (10 ファイル)
- [ ] `scripts/track_branch_guard.py` / `scripts/test_track_branch_guard.py` が削除されている (T003)
- [ ] `scripts/test_track_schema.py` が削除されている (impl の `track_schema.py` は Phase 3 まで残す) (T007)
- [ ] `scripts/track_markdown.py` / `scripts/test_track_markdown.py` が削除されている (T010)
- [ ] `scripts/track_registry.py` / `scripts/test_track_registry.py` が削除されている (T013)
- [ ] `scripts/track_state_machine.py` / `scripts/test_track_state_machine.py` が削除されている (T015)
- [ ] `scripts/test_check_layers.py` が削除されている (T016)

### scripts/ 残存ファイル確認 (15 ファイル)
- [ ] `scripts/` に残るファイルが以下の 15 ファイルと完全一致する: `__init__.py`, `conftest.py`, `architecture_rules.py`, `test_architecture_rules.py`, `atomic_write.py`, `test_atomic_write.py`, `convention_docs.py`, `test_convention_docs.py`, `external_guides.py`, `test_external_guides.py`, `track_schema.py`, `track_resolution.py`, `test_track_resolution.py`, `test_make_wrappers.py`, `test_verify_scripts.py` (T017)

### Makefile.toml 更新確認
- [ ] `Makefile.toml:109-128` の `scripts-selftest-local` args リストが 5 ファイル (`test_track_branch_guard.py` / `test_track_schema.py` / `test_track_markdown.py` / `test_track_registry.py` / `test_track_state_machine.py`) を除外した状態になっている (T003, T007, T010, T013, T015)

### Rust テスト追加確認
- [ ] `apps/cli/src/commands/track/transition.rs::tests` に `verify_branch_guard_with_branch` の 5 観点テストが追加されている (T001, T002)
- [ ] `libs/infrastructure/src/track/render.rs::tests` に validate_track_document の 5 観点確認テストが追加されている。新規ロジック追加なし、既存実装 (`validate_plan_invariants()` / `validate_track_document()`) で pass している (T004, T005)
- [ ] `libs/domain/src/track.rs::tests` に `TrackMetadata::status()` の 10 導出観点テストが追加されている (T006)
- [ ] `libs/infrastructure/src/track/render.rs::tests` に `render_plan()` の個別マーカーテスト 7 件以上が追加されている (T008, T009)
- [ ] `libs/infrastructure/src/track/render.rs::tests` に `render_registry()` / `collect_track_snapshots()` / `sync_rendered_views()` の境界テスト 3 件が追加されている (T011, T012)
- [ ] `apps/cli/tests/transition_integration.rs` が新規作成され、5 つの CLI integration test (transition_subcommand_success / _invalid_transition / _missing_dir / _with_commit_hash / sync_views_subcommand) が pass している (T014)

### 後方互換性確認
- [ ] `scripts/test_track_resolution.py:47` の `test_package_style_imports_work_from_repo_root` から `scripts.track_registry` import が除外されている (T013)
- [ ] `scripts/track_schema.py` が `scripts/track_resolution.py` および `scripts/external_guides.py` から引き続き import されている (Phase 3 まで残存) (T007)

## Verification Results

(未実施)

## Manual Verification Steps

- 各 task commit 後に `cargo make ci` を実行して `ci-local` gate 全サブタスク (Makefile.toml:595 が dispatch する fmt-check / clippy / test / deny / check-layers / verify-arch-docs / verify-spec-states-current-local / test-doc / python-lint / scripts-selftest / verify-doc-links / verify-track-metadata / verify-view-freshness 等) が pass することを確認する
- 各 cleanup task 実行後に `cargo make scripts-selftest` を実行し、削除対象外の Python テスト 6 件 (`test_architecture_rules.py` / `test_atomic_write.py` / `test_convention_docs.py` / `test_external_guides.py` / `test_make_wrappers.py` / `test_track_resolution.py`) が引き続き pass することを確認する
- T005 の validate_track_document テスト追加後に `cargo make track-sync-views` を実行し、現在のトラックの plan.md と registry.md が正常に更新されることを確認する (新規ロジック追加なしのため後方互換性リスクは低いが念のため確認。`track-sync-views` は `--track-id` を明示しない場合、現在のブランチのトラックと registry.md のみ更新するため全トラック一括更新ではない)
- T014 の CLI integration test 実装後に `cargo test --test transition_integration -p cli` で 5 件全てが pass することを確認する
- T017 の最終確認で `scripts/` 配下の tracked file 集合が 15 ファイル (`__init__.py`, `conftest.py`, `architecture_rules.py`, `test_architecture_rules.py`, `atomic_write.py`, `test_atomic_write.py`, `convention_docs.py`, `test_convention_docs.py`, `external_guides.py`, `test_external_guides.py`, `track_schema.py`, `track_resolution.py`, `test_track_resolution.py`, `test_make_wrappers.py`, `test_verify_scripts.py`) と **完全一致** することを手動確認する (削除対象 10 件なし + 残存 15 件あり + 想定外の extra file なし の 3 条件)。確認コマンドは `find` / `git ls-files` / `ls` を環境に応じて組み合わせる
- `cargo make track-signals scripts-phase1-rustify-2026-04-13` で spec signals が Blue 維持であることを確認する (`bin/sotp track type-signals` は domain-types.json 用、本 track には domain-types.json はないため使用しない)

## Result / Open Issues

(未実施)

## verified_at

(未実施)
