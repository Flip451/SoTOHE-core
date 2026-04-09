# Verification: RV2-17 Python hooks 全廃止 (Phase 1)

## Scope Verified

- [x] `.claude/hooks/` ディレクトリが完全削除されている (T02)
- [x] `.claude/settings.json` から Python hook entry (PreToolUse 2 + PostToolUse 7 = 9 entries) が全削除されている (T03)
- [x] `.claude/settings.json` permissions.allow から `Bash(cargo make hooks-selftest)` が削除されている (T01 ペア変更で実施)
- [x] Rust hook (`skill-compliance` / `block-direct-git-ops` / `block-test-file-deletion`) は `.claude/settings.json` に維持されている (T03 で確認)
- [x] `libs/infrastructure/src/verify/orchestra.rs` の `EXPECTED_HOOK_PATHS` から削除対象 9 hook が除去されている (T01)
- [x] `Makefile.toml` から `[tasks.hooks-selftest]` と `[tasks.hooks-selftest-local]` が削除されている (T04)
- [x] `Makefile.toml` の `python-lint-local` / `python-lint` ruff 対象が `scripts/` のみになっている (T05)
- [x] CLAUDE.md / .claude/rules/09-maintainer-checklist.md / DEVELOPER_AI_WORKFLOW.md / knowledge/WORKFLOW.md / LOCAL_DEVELOPMENT.md / START_HERE_HUMAN.md / knowledge/DESIGN.md / track/workflow.md から Python hook 言及が整理されている (T06)
- [x] TRACK_TRACEABILITY.md の enforcement task list から `hooks-selftest-local` が削除されている (T06)
- [x] `libs/infrastructure/src/verify/doc_patterns.rs` から hooks-selftest 関連の `RequireLine` 3 entries が削除されている (T06)
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

### T05 (2026-04-10) — 実装順序を T02 と入れ替え

- 順序入れ替えの理由: T04 と同じ。T02 (Python hook ファイル削除) を先にやると `python-lint-local` の `ruff check scripts/ .claude/hooks/` が削除済みディレクトリを参照して E902 で失敗する。T05 を先にすることで CI を一貫した状態で維持
- `Makefile.toml`:
  - `[tasks.python-lint-local]` の script を `'ruff check scripts/'` に変更し、description を "Run ruff lint on Python helper scripts under scripts/" に更新
  - `[tasks.python-lint]` (host wrapper) の script から `.claude/hooks/` を削除
  - `python-lint` task 自体は `scripts/` 配下の Python ヘルパー (architecture_rules.py, atomic_write.py, convention_docs.py, external_guides.py, track_*.py 等) の lint 用に維持
- 検証: `cargo make ci` 全 PASS

### T02 (2026-04-10) — Python hook ファイル削除 (実行順序: T03→T04→T05 完了後)

- 全 16 個の `.claude/hooks/*.py` ファイルを `__pycache__` / `.pytest_cache` を含めて削除し、`.claude/hooks/` ディレクトリ自体も削除した:
  - advisory hooks (9): `check-codex-before-write.py`, `check-codex-after-plan.py`, `error-to-codex.py`, `post-implementation-review.py`, `post-test-analysis.py`, `suggest-gemini-research.py`, `lint-on-save.py`, `python-lint-on-save.py`, `log-cli-tools.py`
  - libraries (2): `_agent_profiles.py`, `_shared.py`
  - tests (5): `test_agent_profiles.py`, `test_helpers.py`, `test_post_tool_hooks.py`, `test_pre_tool_hooks.py`, `test_shared_hook_utils.py`
- 安全な削除のための前提条件 (T03 settings.json hook entry 削除、T04 hooks-selftest task 削除、T05 python-lint scripts/ 限定) は全て先行完了済み
- 検証: `cargo make ci` 全 PASS、`cargo make verify-orchestra` PASS、`.claude/hooks/` ディレクトリが filesystem 上に存在しないことを確認

### T06 (2026-04-10) — ドキュメント更新

- `.claude/rules/09-maintainer-checklist.md`:
  - "Host prerequisite" の Python 言及を更新 ("python3 is optional on host" → "required inside Docker for scripts/ helpers")
  - "enforcement" 列から `.claude/hooks/` を削除し、Rust hook entries (skill-compliance / block-direct-git-ops / block-test-file-deletion) を `.claude/settings.json` の項目に併記
- `DEVELOPER_AI_WORKFLOW.md`:
  - Python test 説明から `hooks-selftest` を削除
  - `cargo make hooks-selftest` のコマンド行を削除
  - `cargo make ci` の補足リストから `hooks-selftest` を除外
- `LOCAL_DEVELOPMENT.md`:
  - Python test 説明から `cargo make hooks-selftest` を削除
  - "Claude hooks in `.claude/hooks/` run via `python3`..." の行を Rust hook (`bin/sotp hook dispatch`) 説明に置換
  - "lint-on-save を有効にする" 節を "tools-daemon コンテナを使う" 節に書き換え (lint-on-save hook 自体が削除されたため)
- `START_HERE_HUMAN.md`: 編集対象リストから `.claude/hooks/**` を削除
- `track/workflow.md`: Definition of Done のチェックリストから `cargo make hooks-selftest` 行を削除
- `knowledge/DESIGN.md`: "Security Hardening: Rust Migration" 節を更新し、Python advisory hooks 表を Rust-only 表に書き換え。RV2-17 への参照を追記
- `TRACK_TRACEABILITY.md`: enforcement task list から `hooks-selftest-local` を削除し、`python-lint-local` の説明を `scripts/` のみに修正
- `libs/infrastructure/src/verify/doc_patterns.rs`: docs 整合性チェックの 3 entries (track/workflow.md hooks selftest gate / TRACK_TRACEABILITY.md hooks selftest gate / DEVELOPER_AI_WORKFLOW.md hooks selftest gate) を削除
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
