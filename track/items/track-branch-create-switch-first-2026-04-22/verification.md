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

### T001 実装結果 (commit 34d0214c)

- `execute_branch(BranchAction::Create)` を `execute_activate` forward から切り離し、`execute_branch_create` 独立関数として実装。`git switch -c track/<id> main` のみを実行し、`git add` / `git commit` は一切呼ばない。
- `BranchMode` enum から `Create` variant を削除 (enum-first 原則で型レベルで到達不可能にする)。`execute_activate` は `Switch` / `Auto` のみ受け付ける。
- 既に retire 済だった `execute_legacy_branch_mode` / `uses_legacy_branch_mode` と関連 rstest を dead code として削除 (user 明示承認)。
- 新 regression test `branch_create_execute_runs_only_switch_c_main_and_no_commit` が `RecordingRepo::status_calls` で `git add` / `git commit` が呼ばれないことを検証。
- 既存 `BranchMode::Switch` / `BranchMode::Auto` のテスト群は全 pass 維持。

### T002 結果 (cargo make ci 回帰ゲート)

- `cargo make ci` (fmt-check + clippy + nextest + deny + check-layers + verify-* 一式) 全項目 PASS。
- nextest: 2348 tests run, 2348 passed, 7 skipped (regression ゼロ)。
- verify-orchestra / verify-domain-purity / verify-usecase-purity / verify-view-freshness / verify-plan-artifact-refs / verify-spec-states / verify-arch-docs / verify-plan-progress / verify-track-metadata / verify-tech-stack / verify-latest-track / verify-track-registry 全 PASS。

### 付随 commit (ad74a8ab) — spec 外 micro change

- `FORBIDDEN_ALLOW` から `grep` / `diff` / `jq` / `pwd` / `uniq` を allow に移行 (user 明示承認)。
- `find` (`-exec`/`-execdir`) / `sort` (`--compress-program`) / `env` / `xargs` は wrap-execute 脆弱性のため FORBIDDEN 維持。
- reviewer (gpt-5.4) の P0 finding を 2 度受けて最終方針確定。3 SSoT (orchestra.rs / settings.json / 10-guardrails.md) 同期済。

### 未解決事項

なし。AC-01 〜 AC-04 全て満たす。

## verified_at

2026-04-23
