# Verification — scripts/ Python ヘルパー削減 フェーズ1

## Scope Verified

### 削除確認 (10 ファイル)
- [x] `scripts/track_branch_guard.py` / `scripts/test_track_branch_guard.py` が削除されている (T003, commit ae3578f)
- [x] `scripts/test_track_schema.py` が削除されている (impl の `track_schema.py` は Phase 3 まで残す) (T007, commit cd14994)
- [x] `scripts/track_markdown.py` / `scripts/test_track_markdown.py` が削除されている (T010, commit c4c10eb)
- [x] `scripts/track_registry.py` / `scripts/test_track_registry.py` が削除されている (T013, commit a86ff57)
- [x] `scripts/track_state_machine.py` / `scripts/test_track_state_machine.py` が削除されている (T015, commit be8adef)
- [x] `scripts/test_check_layers.py` が削除されている (T016, commit 2a73bfd)

### scripts/ 残存ファイル確認 (15 ファイル)
- [x] `scripts/` に残るファイルが以下の 15 ファイルと完全一致する: `__init__.py`, `conftest.py`, `architecture_rules.py`, `test_architecture_rules.py`, `atomic_write.py`, `test_atomic_write.py`, `convention_docs.py`, `test_convention_docs.py`, `external_guides.py`, `test_external_guides.py`, `track_schema.py`, `track_resolution.py`, `test_track_resolution.py`, `test_make_wrappers.py`, `test_verify_scripts.py` (T017)

### Makefile.toml 更新確認
- [x] `Makefile.toml` の `scripts-selftest-local` args リストが 5 ファイル (`test_track_branch_guard.py` / `test_track_schema.py` / `test_track_markdown.py` / `test_track_registry.py` / `test_track_state_machine.py`) を除外した状態になっている (T003, T007, T010, T013, T015)

### Rust テスト追加確認
- [x] `apps/cli/src/commands/track/transition.rs::tests` に `verify_branch_guard_with_branch` の 5 観点テスト + `verify_branch_guard` の branchless skip test (計 6 件) が追加されている (T001, T002, commit 43ddb54)
- [x] `libs/infrastructure/src/track/render.rs::tests` に validate_track_document の 5 観点確認テストが追加されている (T004, T005, commit ffc6842)。新規ロジック追加なし
- [x] `libs/domain/src/track.rs::tests` に `TrackMetadata::status()` の 10 導出観点テストが追加されている (T006, commit 2d89505)
- [x] `libs/infrastructure/src/track/render.rs::tests` に `render_plan()` の個別マーカーテスト 7 件が追加されている (T008, T009, commit 3d50b2d)
- [x] `libs/infrastructure/src/track/render.rs::tests` に `render_registry()` / `collect_track_snapshots()` / `sync_rendered_views()` の境界テスト 3 件 + `collect_track_snapshots()` の updated_at tie-break が追加されている (T011, T012, commit 762fe57)
- [x] `apps/cli/tests/transition_integration.rs` が新規作成され、5 つの CLI integration test (transition_subcommand_success / _invalid_transition / _missing_dir / _with_commit_hash / sync_views_subcommand) が pass している (T014, commit 49ba07a)

### 後方互換性確認
- [x] `scripts/test_track_resolution.py:47` の `test_package_style_imports_work_from_repo_root` から `scripts.track_registry` import が除外されている (T013, commit a86ff57)
- [x] `scripts/track_schema.py` が `scripts/track_resolution.py` および `scripts/external_guides.py` から引き続き import されている (Phase 3 まで残存) (T007 以降)

## Verification Results

- 全 13 commit (C1–C13) で `cargo make ci` が pass (fmt-check / clippy / test / deny / check-layers / verify-arch-docs / python-lint / scripts-selftest / verify-doc-links / verify-track-metadata / verify-view-freshness 全 gate 通過)
- `cargo make scripts-selftest`: T003 後 278 → T007 後 204 → T010 後 153 → T013 後 120 → T015/T016 後 120 (削除時の pass 数推移。残存 6 test ファイル分)
- Rust 側新規テスト合計: 6 (verify_branch_guard) + 5 (validate_track_document) + 10 (TrackMetadata::status) + 7 (render_plan marker) + 3 (registry/snapshot boundary) + 5 (CLI integration) = 36 件、全て pass
- `scripts/` 残存ファイル確認: `ls scripts/*.py` が 15 ファイルを返し、削除対象 10 件 + 残存 15 件 + 想定外 extra file なしの 3 条件を満たす
- ドキュメント更新: `track/workflow.md`, `.claude/rules/09-maintainer-checklist.md` から削除済ファイルへの参照を除外

## Manual Verification Steps

- 各 task commit 後に `cargo make ci` を実行して `ci-local` gate 全サブタスクが pass することを確認する (実施済)
- 各 cleanup task 実行後に `cargo make scripts-selftest` を実行し、削除対象外の Python テスト (`test_architecture_rules.py` / `test_atomic_write.py` / `test_convention_docs.py` / `test_external_guides.py` / `test_make_wrappers.py` / `test_track_resolution.py`) が引き続き pass することを確認する (実施済)
- T005 の validate_track_document テスト追加後に `cargo make track-sync-views` を実行し、plan.md と registry.md が正常に更新されることを確認する (実施済、新規ロジック追加なし)
- T014 の CLI integration test 実装後に `cargo make test-one-exec transition_subcommand` で 5 件全てが pass することを確認する (実施済)
- T017 の最終確認で `scripts/` 配下の tracked file 集合が 15 ファイルと完全一致することを手動確認する (実施済、`ls scripts/*.py` で 15 件)
- `cargo make track-signals scripts-phase1-rustify-2026-04-13` で spec signals が Blue 維持であることを確認する (実施済、本 track は track artifact のみ変更のため signals に影響なし)

## Result / Open Issues

- すべての受け入れ基準 (10 件) を達成
- Phase 2 (atomic_write.py / architecture_rules.py) と Phase 3 (external_guides.py / convention_docs.py / track_schema.py / track_resolution.py) は引き続き残存
- commit_hash backfill は C13 (本 commit) で batch 実施

## verified_at

2026-04-13T14:00:00Z
