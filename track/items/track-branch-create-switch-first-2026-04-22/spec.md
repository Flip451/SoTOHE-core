<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 17, yellow: 0, red: 0 }
---

# sotp track branch create: main 上 activation commit bug fix (switch-before-commit)

## Goal

- [GO-01] `sotp track branch create` (`execute_branch(BranchAction::Create)`) を `execute_activate` への forward から切り離し、`git switch -c track/<id> main` のみを実行する独立 path に分離することで、main ブランチ上に activation commit が生成される regression を構造的に排除する [adr: knowledge/adr/2026-04-22-1432-branch-create-commit-ordering.md#D1, knowledge/adr/2026-04-22-0829-plan-command-structural-refinements.md#D3]
- [GO-02] `/track:init` が `cargo make track-branch-create '<track-id>'` を呼んだ際、branch 作成と switch のみが実行され、metadata persist / activation commit は後続 step (track ブランチ上) で行われる設計を確立し、main ブランチを汚染しない不変条件を保証する [adr: knowledge/adr/2026-04-22-1432-branch-create-commit-ordering.md#D3, knowledge/adr/2026-04-22-0829-plan-command-structural-refinements.md#D3]

## Scope

### In Scope
- [IN-01] `apps/cli/src/commands/track/activate.rs` の `execute_branch(BranchAction::Create)` を `execute_activate(BranchMode::Create)` への forward から切り離し、単純な `git switch -c track/<id> main` のみを実行する独立 path として実装する [adr: knowledge/adr/2026-04-22-1432-branch-create-commit-ordering.md#D1] [tasks: T001]
- [IN-02] `BranchMode::Create` を `execute_activate` から退役させる: `execute_activate` は `BranchMode::Switch` / `BranchMode::Auto` のみを受け付け、`BranchMode::Create` 経由の呼び出しは存在しないようにする。関連する rstest / unit test の caller を更新する [adr: knowledge/adr/2026-04-22-1432-branch-create-commit-ordering.md#D2] [tasks: T001]
- [IN-03] `execute_branch(BranchAction::Create)` の独立 path 実装と `BranchMode::Create` 退役に対応するユニットテスト: 新 path が `git switch -c` のみを実行し commit を生成しないこと、`BranchMode::Auto` / `BranchMode::Switch` の既存テストが引き続き pass することを確認する [adr: knowledge/adr/2026-04-22-1432-branch-create-commit-ordering.md#D1, knowledge/adr/2026-04-22-1432-branch-create-commit-ordering.md#D2] [tasks: T001]
- [IN-04] `.claude/commands/track/init.md` を 3 step 構成 (1. `cargo make track-branch-create` で branch 作成 + switch、2. `metadata.json` を `branch: "track/<track-id>"` で作成、3. `cargo make verify-track-metadata`) に整理し、`Makefile.toml` の `track-branch-create` description を branch 作成 + switch のみ (no metadata, no commit) に更新する。`execute_activate` / `sotp track activate` は無変更 [adr: knowledge/adr/2026-04-22-1432-branch-create-commit-ordering.md#D3, knowledge/adr/2026-04-22-1432-branch-create-commit-ordering.md#D1] [tasks: T003]

### Out of Scope
- [OS-01] `execute_branch` / `execute_activate` の上位 refactor (OS-08、ADR 2026-04-22-0829 で別 track に委譲): `BranchMode` のコードパス全体の統廃合・再設計は本 track では行わない [adr: knowledge/adr/2026-04-22-0829-plan-command-structural-refinements.md#Consequences, knowledge/adr/2026-04-22-1432-branch-create-commit-ordering.md#Reassess When]
- [OS-02] `BranchMode::Auto` / `BranchMode::Switch` の activation フローへの変更: `BranchMode::Create` が `execute_activate` から退役するため、残る `Auto` / `Switch` のコードパスは既存動作を維持する [adr: knowledge/adr/2026-04-22-1432-branch-create-commit-ordering.md#D2]
- [OS-03] activation resume flow の変更: resume marker / `already_materialized = true` パスのロジックは `BranchMode::Auto` 配下であり、`BranchMode::Create` 退役の影響を受けない。変更は行わない [adr: knowledge/adr/2026-04-22-1432-branch-create-commit-ordering.md#D2]

## Constraints
- [CN-01] no panics in library code: `activate.rs` の変更箇所でパニックしうる構文 (unwrap / expect / slice indexing / unreachable など) を使わない。`Result` / `?` 演算子で伝搬する [adr: knowledge/adr/2026-04-22-1432-branch-create-commit-ordering.md#D1] [conv: .claude/rules/04-coding-principles.md#No Panics in Library Code] [tasks: T001, T003]
- [CN-02] `execute_activate` に残る `BranchMode::Switch` / `BranchMode::Auto` のロジックは変更しない。`BranchMode::Create` 退役後も `BranchMode::Auto` の resume flow (already_materialized = true かつ resume_allowed = true) の動作を壊さないこと [adr: knowledge/adr/2026-04-22-1432-branch-create-commit-ordering.md#D2, knowledge/adr/2026-04-22-0829-plan-command-structural-refinements.md#D5] [tasks: T001, T003]
- [CN-03] `cargo make ci` (fmt / clippy / nextest / deny / verify-* 一式) が pass すること [adr: knowledge/adr/2026-04-22-1432-branch-create-commit-ordering.md#Consequences] [conv: .claude/rules/07-dev-environment.md#Pre-commit Checklist] [tasks: T002, T003]

## Acceptance Criteria
- [ ] [AC-01] `cargo make track-branch-create '<track-id>'` 実行後、`git log main --oneline` に activation commit が現れない。main ブランチは track ブランチ作成前後で HEAD が変化しない [adr: knowledge/adr/2026-04-22-1432-branch-create-commit-ordering.md#D3, knowledge/adr/2026-04-22-0829-plan-command-structural-refinements.md#D3] [tasks: T001, T002]
- [ ] [AC-02] `execute_branch(BranchAction::Create)` の独立 path ユニットテストが pass する: `git switch -c track/<id> main` のみが実行され、`git commit` が実行されないことを確認する [adr: knowledge/adr/2026-04-22-1432-branch-create-commit-ordering.md#D1] [tasks: T001]
- [ ] [AC-03] `BranchMode::Auto` / `BranchMode::Switch` の `execute_activate` 既存テストが `BranchMode::Create` 退役後も引き続き pass し、動作が変わっていないことを確認する [adr: knowledge/adr/2026-04-22-1432-branch-create-commit-ordering.md#D2] [tasks: T001, T003]
- [ ] [AC-04] `cargo make ci` (fmt / clippy / nextest / deny / verify-* 一式) が pass する [adr: knowledge/adr/2026-04-22-1432-branch-create-commit-ordering.md#Consequences] [conv: .claude/rules/07-dev-environment.md#Pre-commit Checklist] [tasks: T002, T003]
- [ ] [AC-05] `/track:init <feature>` が 3 step (branch 作成 + switch → metadata.json を `branch: "track/<track-id>"` で作成 → verify-track-metadata) で完走する [adr: knowledge/adr/2026-04-22-1432-branch-create-commit-ordering.md#D3] [tasks: T003]

## Related Conventions (Required Reading)
- .claude/rules/04-coding-principles.md#No Panics in Library Code
- .claude/rules/05-testing.md#Core Principles
- knowledge/conventions/pre-track-adr-authoring.md#Rules

## Signal Summary

### Stage 1: Spec Signals
🔵 17  🟡 0  🔴 0

