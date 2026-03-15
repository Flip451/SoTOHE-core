<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# takt 廃止実装

`takt-removal-2026-03-13` で固定した計画に基づき、takt runtime / wrapper / テスト / ドキュメントをリポジトリから完全に除去する。
Phase A（ドキュメント清書）→ Phase B+C（ランタイム＋failure-report 削除）→ Phase D（テスト/CI 整理＋hook/profile）→ transient path 清掃の順で進める。
公開 UI `/track:*` は維持し、`cargo make ci` を常に green に保つ。

## Related Conventions (Required Reading)

project-docs/conventions/README.md
project-docs/conventions/security.md
track/items/takt-removal-2026-03-13/takt-runtime-removal-sequence.md
track/items/takt-removal-2026-03-13/takt-removal-definition-of-done.md
track/items/takt-removal-2026-03-13/pending-artifact-cutover.md
track/items/takt-removal-2026-03-13/takt-touchpoint-inventory.md


## Phase A: Document cleanup

- [ ] ドキュメント清書 — `.claude/rules/02-codex-delegation.md`、`.claude/rules/03-gemini-delegation.md`、`.claude/rules/07-dev-environment.md`、`.claude/rules/08-orchestration.md`、`.claude/rules/09-maintainer-checklist.md`、`track/workflow.md`、`DEVELOPER_AI_WORKFLOW.md`、`TAKT_TRACK_TRACEABILITY.md` から takt の migration-only 言及を完全除去し、Claude Code + Rust CLI 前提の記述に統一する。`CLAUDE.md` の takt definitions 参照も除去する

## Phase B+C: Runtime and failure-report removal

- [ ] ランタイム削除 — `scripts/takt_profile.py`、`.takt/pieces/`、`.takt/personas/`、`.takt/runtime/`、`.takt/tasks.yaml`、`.takt/tasks/`、`.takt/runs/`、`.takt/persona_sessions.json` を削除する。`Makefile.toml` から `takt-add`、`takt-run`、`takt-render-personas`、`takt-full-cycle`、`takt-spec-to-impl`、`takt-impl-review`、`takt-tdd-cycle`、`takt-clean-queue` wrapper と `TAKT_PYTHON` env 変数を除去する。`cargo make ci` 通過を確認する
- [ ] failure-report 削除 — `scripts/takt_failure_report.py`、`scripts/test_takt_failure_report.py`、`Makefile.toml` の `[tasks.takt-failure-report]` を削除する。`.takt/debug-report.md`、`.takt/last-failure.log` への言及も docs から除去する

## Phase D: Test/CI and profile/hook cleanup

- [ ] テスト/CI 整理 — `scripts/test_takt_profile.py`、`scripts/test_takt_personas.py` を削除する。`Makefile.toml` の `scripts-selftest-local` テストリストから除去する。`scripts/test_make_wrappers.py` の takt-* wrapper アサーション削除。`scripts/test_verify_scripts.py` の関連チェック更新。`cargo make ci` 通過を確認する
- [ ] Profile/Hook 整理 — `.claude/hooks/_agent_profiles.py` の `takt_host_*` 互換エイリアスと公開関数を除去する。`.claude/hooks/agent-router.py` の `takt` keyword ルーティングを除去する。hook の `TAKT_SESSION` silence guard を全 hook から除去する。`.claude/settings.json` の `add-pending-paths`、`commit-pending-message`、`note-pending` permissions を除去する。`scripts/verify_orchestra_guardrails.py` の関連 expected entries を更新する。`.claude/hooks/test_agent_router.py`、`.claude/hooks/test_agent_profiles.py` の takt テストを更新する

## Transient path cleanup and final sweep

- [ ] Rust/Python transient path 清掃 — `libs/usecase/src/git_workflow.rs` の `TRANSIENT_AUTOMATION_FILES` から `.takt/pending-*` を除去し、`TRANSIENT_AUTOMATION_DIRS` から `.takt/handoffs` を除去する。`scripts/git_ops.py` の `LEGACY_TAKT_PENDING_FILES` 定数と関連ロジックを除去する。`scripts/test_git_ops.py` の `.takt/pending-*` テストケースを更新する。`apps/cli/src/commands/git.rs` と `libs/infrastructure/src/git_cli.rs` のテスト内 `.takt/pending-note.md` パス参照を除去する。`Makefile.toml` の `add-pending-paths`、`commit-pending-message`、`note-pending` wrapper を除去する
- [ ] 最終清掃と検証 — `.takt/config.yaml`、`.takt/.gitignore`、残余ファイルを削除する。`.takt/` ディレクトリが空になったことを確認し、ディレクトリ自体を削除する。`cargo make ci` で全ゲート通過を最終確認する。M1〜M4 の exit criteria を verification.md に記録する
