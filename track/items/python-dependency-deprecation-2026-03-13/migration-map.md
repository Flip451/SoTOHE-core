# Python Dependency Migration Map

作成日: 2026-03-13
対象戦略: `STRAT-03` Python 依存からの脱却

## 目的

Python エントリポイントを分類し、各要素を以下のどれに移すかを固定する。

- `Rustへ移行`: 必須経路のため Rust 実装へ置換
- `Rustへ集約`: 近縁責務をまとめて Rust CLI に吸収
- `当面維持`: 補助用途のため当面 Python のまま
- `廃止候補`: `takt` 縮退とともに消す

## A. Security-Critical Hooks

| Entry point | 現在の役割 | 方針 | 移行先 / 備考 |
|---|---|---|---|
| `.claude/hooks/block-direct-git-ops.py` | direct git block pre-hook | Rustへ移行 | 既存 `sotp hook dispatch` を launcher なしで直接呼ぶ |
| `.claude/hooks/file-lock-acquire.py` | pre-hook lock acquire | Rustへ移行 | lock daemon / Rust hook 直結 |
| `.claude/hooks/file-lock-release.py` | post-hook lock release | Rustへ移行 | lock daemon / Rust hook 直結 |

## B. Workflow Core

主に影響するコマンド:

- `/track:plan`
- `/track:commit`
- `/track:archive`
- `/track:pr-review`

| Entry point | 現在の役割 | 方針 | 移行先 / 備考 |
|---|---|---|---|
| `scripts/track_state_machine.py` | task transition / sync-views | Rustへ集約 | `sotp track transition`, `sotp track sync-views` |
| `scripts/track_schema.py` | metadata parse/validate shared model | Rustへ集約 | domain + infrastructure codec/validation へ統合 |
| `scripts/track_markdown.py` | rendered view generation (`/track:plan` の `plan.md` 生成を含む) | Rustへ集約 | `sotp track render-*` |
| `scripts/git_ops.py` | add/commit/note wrapper | Rustへ移行 | `sotp git add-from-file`, `commit-from-file`, `note-from-file` 相当 |
| `scripts/branch_switch.py` | branch switch / pull helper | Rustへ移行 | branch workflow subcommand へ統合 |
| `scripts/pr_review.py` | PR review orchestration | Rustへ移行 | async state 永続化込みで実装 |
| `scripts/pr_merge.py` | merge wait/status helper | Rustへ移行 | PR workflow subcommand 群へ統合 |

## C. Verification / CI Gate

| Entry point | 現在の役割 | 方針 | 移行先 / 備考 |
|---|---|---|---|
| `scripts/verify_plan_progress.py` | metadata / plan 整合検証 | Rustへ移行 | track validation subcommand |
| `scripts/verify_track_metadata.py` | metadata validation | Rustへ移行 | track validation subcommand |
| `scripts/verify_track_registry.py` | registry sync 検証 | Rustへ移行 | registry 非 Git 化後は責務再定義 |
| `scripts/verify_latest_track_files.py` | latest track artifact 検証 | Rustへ移行 | branch-aware latest selection と合わせて再設計 |
| `scripts/verify_tech_stack_ready.py` | planning completeness gate | Rustへ移行 | CI gate か preflight に再配置 |
| `scripts/check_layers.py` | layer dependency check | Rustへ集約 or 当面維持 | JSON SSoT 側の責務整理次第 |
| `scripts/verify_architecture_docs.py` | architecture doc / task sync | 当面維持 | docs/JSON SSoT 整理後に再評価 |
| `scripts/verify_orchestra_guardrails.py` | settings/guardrail allowlist 検証 | Rustへ移行 | guardrail SSoT 再設計とセット |

## D. Takt / Queue

| Entry point | 現在の役割 | 方針 | 移行先 / 備考 |
|---|---|---|---|
| `scripts/takt_profile.py` | queue / personas / runtime orchestration | 廃止候補 | `STRAT-01` に従い廃止。後継 orchestrator は作らず、必要機能のみ Rust/Claude Code ネイティブへ移す |
| `scripts/takt_failure_report.py` | failure summary generation | 廃止候補 | debugger workflow へ吸収、または Rust utility 化 |

## E. Auxiliary Utilities

| Entry point | 現在の役割 | 方針 | 移行先 / 備考 |
|---|---|---|---|
| `scripts/external_guides.py` | guide registry/cache utility | 当面維持 | security-critical ではない。後で必要なら Rust 化 |
| `scripts/convention_docs.py` | convention index utility | 当面維持 | docs utility のため優先度低 |
| `scripts/architecture_rules.py` | architecture rules utility | 当面維持 | SSoT 整理後に判断 |

## F. Non-Critical Hooks

| Entry point | 現在の役割 | 方針 | 移行先 / 備考 |
|---|---|---|---|
| `.claude/hooks/agent-router.py` | agent routing | 当面維持 | `RTR-*` 改善後に再評価 |
| `.claude/hooks/check-codex-before-write.py` | advisory hook | 当面維持 | 失敗しても致命ではない |
| `.claude/hooks/check-codex-after-plan.py` | advisory hook | 当面維持 | 同上 |
| `.claude/hooks/suggest-gemini-research.py` | advisory hook | 当面維持 | 同上 |
| `.claude/hooks/error-to-codex.py` | post-hook error suggestion | 当面維持 | debugger 戦略見直し後に再評価 |
| `.claude/hooks/post-test-analysis.py` | post-hook test failure suggestion | 当面維持 | structured diagnostics 化後に再評価 |
| `.claude/hooks/log-cli-tools.py` | CLI tool logging | 当面維持 | Rust logger に寄せる余地あり |
| `.claude/hooks/lint-on-save.py` | Rust save hook | 当面維持 | optional utility |
| `.claude/hooks/python-lint-on-save.py` | Python save hook | 当面維持 | Python 補助用途 |
| `.claude/hooks/post-implementation-review.py` | advisory review hook | 当面維持 | workflow 見直し後に再評価 |
| `.claude/hooks/_shared.py` | hook shared helper | 当面維持 | 上記 Python hook の縮退に従属 |
| `.claude/hooks/_agent_profiles.py` | hook helper | 当面維持 | 同上 |

## G. Makefile / Settings Dependencies To Remove

| Current dependency | 問題 | 目標 |
|---|---|---|
| `TAKT_PYTHON` in `Makefile.toml` | `.venv` / `python3` 依存の中心 | 削除 |
| `python3 ...` wrappers in `Makefile.toml` | 必須経路が Python 前提 | Rust CLI 呼び出しへ置換 |
| `.claude/settings.json` hook commands using `python3` | bootstrap 前に壊れる | Rust binary 直接呼び出し |
| `scripts-selftest-local` / `hooks-selftest-local` を CI 必須 | Python が CI 必須依存 | Rust 移行後は optional test suite へ格下げ |

## フェーズ別完了条件

### M1

- security-critical hooks が Python なしで動く
- `.venv` 未作成でも hook fail-closed が成立する

### M2

- track transition / sync-views / git workflow wrapper が Python なしで動く
- `Makefile.toml` の主要 wrapper から `python3` 依存が消える

### M3

- PR review / merge 系 workflow が Rust 実装へ移る
- CI の必須 verify path が Python 不要になる

### M4

- `.venv` は optional utility のみ
- Python は docs utility や補助 hook に限定され、track workflow の必須条件ではなくなる
