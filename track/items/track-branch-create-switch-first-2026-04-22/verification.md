# Verification — track-branch-create-switch-first-2026-04-22

> **Track**: `track-branch-create-switch-first-2026-04-22`
> **ADR**: `knowledge/adr/2026-04-22-1432-branch-create-commit-ordering.md` (D1-D3)
> **Scope**: T001 (apps/cli/src/commands/track/activate.rs path separation + BranchMode::Create retirement + tests) + T002 (cargo make ci)

## 検証範囲

本 track は `sotp track branch create` が main 上で activation commit を作成する regression を解消する CLI-layer bug fix。実装および検証は Phase 4 以降の `/track:implement` / `/track:full-cycle` で T001 + T002 として実施する。

## 手動検証手順

### T001 — execute_branch 独立 path + BranchMode::Create 退役

1. `apps/cli/src/commands/track/activate.rs::execute_branch(BranchAction::Create)` が `execute_activate(BranchMode::Create)` への forward から切り離され、独立実装になっている
2. 新 execute_branch path が `git switch -c track/<id> main` のみ実行し、**activation commit を作らない**
3. `execute_activate` の match arms から `BranchMode::Create` が退役している (enum variant 削除 or Err 返しで deny。CN-01 により `unreachable!` / panic は禁止)
4. 関連 rstest (既存で `BranchMode::Create` を execute_activate 経由で検証していたもの) が新独立 path を対象にする形で更新されている
5. 新規 regression test: `sotp track branch create` 実行前後で main の commit count が不変であることを確認
6. 既存の `BranchMode::Switch` / `BranchMode::Auto` テスト群が引き続き pass

### T002 — cargo make ci 回帰ゲート

1. `cargo make ci` 全項目 (fmt-check + clippy + nextest + deny + check-layers + verify-*) pass
2. 本 track の変更が他テストに regression を引き起こしていない

## 共通検証

1. Phase 1-3 の gate 評価が通過:
   - spec-signals: blue=16 / yellow=0 / red=0
   - type-signals: 全 3 layer 0 entries / 0 findings (ADR 2026-04-19-1242 §D6.4 空カタログ許容)
   - task-coverage: `sotp verify plan-artifact-refs` PASSED

## 結果 / 未解決事項

(実装完了時に記録)

## verified_at

(実装完了時に記録)
