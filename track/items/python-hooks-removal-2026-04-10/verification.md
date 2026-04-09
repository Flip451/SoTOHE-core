# Verification: RV2-17 Python hooks 全廃止 (Phase 1)

## Scope Verified

- [ ] `.claude/hooks/` ディレクトリが完全削除されている
- [x] `.claude/settings.json` から Python hook entry (PreToolUse 2 + PostToolUse 7 = 9 entries) が全削除されている (T03)
- [x] `.claude/settings.json` permissions.allow から `Bash(cargo make hooks-selftest)` が削除されている (T01 ペア変更で実施)
- [x] Rust hook (`skill-compliance` / `block-direct-git-ops` / `block-test-file-deletion`) は `.claude/settings.json` に維持されている (T03 で確認)
- [x] `libs/infrastructure/src/verify/orchestra.rs` の `EXPECTED_HOOK_PATHS` から削除対象 9 hook が除去されている (T01)
- [x] `Makefile.toml` から `[tasks.hooks-selftest]` と `[tasks.hooks-selftest-local]` が削除されている (T04)
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

### T01 (2026-04-10)

- `libs/infrastructure/src/verify/orchestra.rs`:
  - `EXPECTED_HOOK_PATHS` を `&[]` (empty slice) に変更し、後続の Python hook 削除作業に備えた
  - `Bash(cargo make hooks-selftest)` を `EXPECTED_CARGO_MAKE_ALLOW` から削除
  - `test_verify_hook_paths_passes_when_all_fragments_present` を簡略化 (Python hook iteration を削除)
  - 新規テスト `test_verify_hook_paths_does_not_require_any_python_hook_scripts` を追加し、post-RV2-17 の不変条件を明示
- `.claude/settings.json`: orphan permission entry `Bash(cargo make hooks-selftest)` を削除 (T01 と T03 の中間状態で `verify-orchestra` が壊れるのを防ぐペア変更)
- 検証: `cargo make ci` 全 PASS、`cargo make verify-orchestra` PASS、`cargo make test-one-exec test_verify_hook_paths` 3 tests PASS

### T03 (2026-04-10) — 実装順序を T02 と入れ替え

- 順序入れ替えの理由: T02 (Python hook 削除) を先に実施すると、settings.json の PreToolUse hook が削除済みファイルを起動しようとして exit 2 (blocking deny) を返し、後続の Edit/Write/Bash ツール呼び出しがブロックされる危険があった。settings.json の Python hook entry を先に削除することでこのリスクを排除
- `.claude/settings.json` の hooks セクションから Python hook を全削除:
  - PreToolUse: `check-codex-before-write` (Edit|Write matcher 全体)、`suggest-gemini-research` (WebSearch|WebFetch matcher 全体) を削除
  - PostToolUse: `check-codex-after-plan` (Task matcher), `error-to-codex` / `post-test-analysis` / `log-cli-tools` (Bash matcher), `lint-on-save` / `python-lint-on-save` / `post-implementation-review` (Edit|Write matcher) — 全 3 matchers を含む `PostToolUse` セクション全体を削除
  - Rust hook (`block-direct-git-ops`, `block-test-file-deletion`, `skill-compliance`) は維持
- 検証: `cargo make ci` 全 PASS、`cargo make verify-orchestra` PASS

### T04 (2026-04-10) — 実装順序を T02 と入れ替え

- 順序入れ替えの理由: T02 (Python hook ファイル削除) を先に実施すると、Makefile.toml の `hooks-selftest-local` task が `pytest .claude/hooks` を実行しようとして CI が失敗する。T04 を先にすることで `cargo make ci` を一貫した状態で通過させられる
- `Makefile.toml`:
  - `[tasks.hooks-selftest-local]` (private pytest task) を削除
  - `[tasks.hooks-selftest]` (compose wrapper) を削除
  - `ci-local` の dependencies から `hooks-selftest-local` を削除
  - `ci-container` の dependencies からも `hooks-selftest-local` を削除
- `scripts/test_make_wrappers.py`:
  - `test_selftest_wrappers_smoke` の selftest task ループから `hooks-selftest-local` を削除
  - `test_docker_wrappers_smoke` の compose wrapper expectation から `hooks-selftest` エントリ全体を削除
- 検証: `cargo make ci` 全 PASS

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
