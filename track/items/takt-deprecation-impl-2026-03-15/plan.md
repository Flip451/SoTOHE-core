<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# takt 廃止実装

`takt-removal-2026-03-13` で固定した計画に基づき、takt runtime / wrapper / テスト / ドキュメントをリポジトリから完全に除去する。
Phase A（ドキュメント + profile/hook 互換エイリアス除去）→ Phase B+C+D（ランタイム＋failure-report＋テスト/CI を atomic に一括削除）→ transient path 清掃の順で進める。
各タスクは削除 → `cargo make ci` 実行 → 失敗テスト修正 → CI green のサイクルで進める。公開 UI `/track:*` は維持する。

## Related Conventions (Required Reading)

project-docs/conventions/README.md
project-docs/conventions/security.md
track/items/takt-removal-2026-03-13/takt-runtime-removal-sequence.md
track/items/takt-removal-2026-03-13/takt-removal-definition-of-done.md
track/items/takt-removal-2026-03-13/pending-artifact-cutover.md
track/items/takt-removal-2026-03-13/takt-touchpoint-inventory.md


## Phase A: Document and profile/hook cleanup (M1 preconditions)

- [x] ドキュメント清書 — `.claude/rules/`、`.claude/commands/track/`、`.claude/docs/WORKFLOW.md`、`.claude/skills/gemini-system/SKILL.md`、`track/workflow.md`、`DEVELOPER_AI_WORKFLOW.md`、`LOCAL_DEVELOPMENT.md`、`TRACK_TRACEABILITY.md`（旧 `TAKT_TRACK_TRACEABILITY.md` からリネーム）、`START_HERE_HUMAN.md`、`CLAUDE.md` から takt の言及を完全除去し、Claude Code + Rust CLI 前提の記述に統一する。`cargo make ci` 通過を確認する f42a79c
- [ ] Profile/Hook 互換エイリアス除去（Phase B 前提条件）— `.claude/hooks/_agent_profiles.py` の `takt_host_*` 互換エイリアスと公開関数を除去する。`.claude/hooks/agent-router.py` の `takt` keyword ルーティングを除去する。全 hook から `TAKT_SESSION` silence guard を除去する。関連する hook selftest を更新する。`cargo make ci` 通過を確認する

## Phase B+C+D: Runtime, failure-report, and test/CI removal (atomic)

- [ ] ランタイム・failure-report・テスト/CI を atomic に一括削除する。`scripts/takt_profile.py`、`scripts/takt_failure_report.py`、`.takt/pieces/`、`.takt/personas/`、`.takt/runtime/`、`.takt/tasks*`、`.takt/runs/`、`.takt/persona_sessions.json` を削除。`Makefile.toml` から `takt-*` wrapper と `TAKT_PYTHON` を除去。`scripts/test_takt_*.py` を削除し `scripts-selftest-local` から除去。`cargo make ci` を実行し、失敗するテスト（`test_make_wrappers.py`、`test_verify_scripts.py`、`verify_orchestra_guardrails.py` 等）を修正して CI green にする

## Transient path cleanup and final sweep

- [ ] Rust/Python transient path 清掃 — `git_workflow.rs` から `.takt/pending-*` / `.takt/handoffs` を除去。`git_ops.py` の `LEGACY_TAKT_PENDING_FILES` を除去。`Makefile.toml` の `add-pending-paths`、`commit-pending-message`、`note-pending` legacy wrapper を除去。`cargo make ci` を実行し、失敗するテストを修正して CI green にする
- [ ] 最終清掃と検証 — `.takt/config.yaml`、`.takt/.gitignore`、残余ファイルを削除し `.takt/` ディレクトリ自体を除去する。`cargo make ci` で全ゲート通過を最終確認する。M1〜M4 の exit criteria を verification.md に記録する
