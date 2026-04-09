# Verification: RV2-17 Python hooks 全廃止 (Phase 1)

## Scope Verified

- [ ] `.claude/hooks/` ディレクトリが完全削除されている
- [ ] `.claude/settings.json` から Python hook entry (PreToolUse 2 + PostToolUse 7 = 9 entries) が全削除されている
- [ ] `.claude/settings.json` permissions.allow から `Bash(cargo make hooks-selftest)` が削除されている
- [ ] Rust hook (`skill-compliance` / `block-direct-git-ops` / `block-test-file-deletion`) は `.claude/settings.json` に維持されている
- [ ] `libs/infrastructure/src/verify/orchestra.rs` の `EXPECTED_HOOK_PATHS` から削除対象 9 hook が除去されている
- [ ] `Makefile.toml` から `[tasks.hooks-selftest]` と `[tasks.hooks-selftest-local]` が削除されている
- [ ] `Makefile.toml` の `python-lint-local` / `python-lint` ruff 対象が `scripts/` のみになっている
- [ ] CLAUDE.md / .claude/rules/09-maintainer-checklist.md / DEVELOPER_AI_WORKFLOW.md / knowledge/WORKFLOW.md / LOCAL_DEVELOPMENT.md / START_HERE_HUMAN.md / knowledge/DESIGN.md / track/workflow.md から Python hook 言及が整理されている
- [ ] knowledge/adr/2026-04-09-{2047,2235,2323}*.md と knowledge/strategy/TODO.md がトラック計画 commit に含まれている
- [ ] cargo make ci 全チェック通過

## Manual Verification Steps

1. `ls .claude/` で hooks/ ディレクトリが存在しないことを確認
2. `cargo make ci` を実行し全ゲート通過を確認 (verify-orchestra で hook path 整合性チェック)
3. `cargo make python-lint` を実行し scripts/ のみが対象で成功することを確認
4. `cargo make hooks-selftest` がエラーになる (task 未定義) ことを確認
5. Claude Code 内で `Edit`/`Write`/`Bash`/`WebSearch` ツールを実行し、Python hook 由来の advisory メッセージが出ないことを確認 (Rust hook の skill-compliance / block-direct-git-ops は引き続き機能することも確認)
6. `git log --oneline -1` で計画 commit に 3 ADR + TODO.md + track artifacts が含まれていることを確認

## Result

（実装完了後に記入）

## Open Issues

（実装中に発見された問題を記録）

### 既知の Out-of-Scope (本トラックでは未対応)

- Dockerfile の python3 / python3-yaml / python3-pytest 削除 → scripts/ Python 残存により不可、別トラックで対応
- requirements-python.txt 削除 → 同上
- scripts/ 配下 25 Python ファイルの削除 → 別トラック (ADR §5 段階的除去方針)
- docker compose 設定から Python 関連ボリュームマウント / 環境変数の整理 (ADR §5 言及) → compose.yml / compose.dev.yml に Python 関連エントリが存在しないため変更不要 (no-op)
- .claude/rules/02-codex-delegation.md の debugger capability 言及削除 → Phase 2 (agent-profiles redesign) トラックで対応

## Verified At

（検証完了日時を記入）
