---
adr_id: 2026-04-09-2323-python-hooks-removal
decisions:
  - id: 2026-04-09-2323-python-hooks-removal_grandfathered
    status: accepted
    grandfathered: true
---
# Python Hooks Removal (RV2-17)

## Status

Proposed

## Context

このプロジェクトの `.claude/hooks/` 配下には複数の Python hook スクリプトが存在し、`.claude/settings.json` の `PreToolUse` / `PostToolUse` エントリから起動されている。歴史的経緯として、Claude Code の hook システムは Python スクリプトを実行する方式から始まり、段階的に Rust (`sotp hook dispatch <name>`) に移行してきた。

現時点で以下の 3 hook は既に Rust 化済み:

- `skill-compliance` (`sotp hook dispatch skill-compliance`)
- `block-direct-git-ops` (`sotp hook dispatch block-direct-git-ops`)
- `block-test-file-deletion` (`sotp hook dispatch block-test-file-deletion`)

一方、以下の Python hook は `.claude/hooks/*.py` にファイルとして残存している:

| ファイル | 役割 | 分類 |
|----------|------|------|
| `check-codex-before-write.py` | 書き込み前に Codex 相談を advisory 的に促す | advisory |
| `check-codex-after-plan.py` | plan 後に Codex 相談を促す | advisory |
| `error-to-codex.py` | エラー発生時に debugger capability を提示 | advisory (debugger 廃止済み) |
| `post-implementation-review.py` | 実装後に reviewer 呼び出しを促す | advisory |
| `post-test-analysis.py` | テスト解析を促す | advisory |
| `suggest-gemini-research.py` | researcher capability を提示 | advisory |
| `lint-on-save.py` | 保存時の lint 結果を表示 | informational |
| `python-lint-on-save.py` | Python lint 結果を表示 | informational |
| `log-cli-tools.py` | CLI tool 呼び出しをログ出力 | informational |
| `_agent_profiles.py` | agent profiles config の Python 側 loader (内部) | library |
| `_shared.py` | Python hook 共通ユーティリティ | library |
| `test_agent_profiles.py` | `_agent_profiles.py` のテスト | test |
| `test_post_tool_hooks.py` | post-tool hook のテスト | test |
| `test_pre_tool_hooks.py` | pre-tool hook のテスト | test |
| `test_helpers.py` | テスト共通ヘルパー | test |
| `test_shared_hook_utils.py` | `_shared.py` のテスト | test |

### 問題点

#### 1. Python 依存の残存

このプロジェクトは Rust への移行を進めてきたが、Python hook が残る限り以下が必要:

- `python3` ランタイムの必要性 (`.claude/settings.json` の各エントリで `command -v python3 >/dev/null 2>&1 || exit 0` で graceful skip しているが、実質的に依存)
- Python の lint / test 基盤の維持 (`cargo make python-lint`, `cargo make hooks-selftest`)
- Python hook と Rust コードの二重メンテナンス負荷
- CI 環境 (Docker コンテナ) に Python をインストールする必要性

#### 2. advisory hook の価値低下

残存する Python hook のほとんどは **advisory (提案を表示するだけ)** で、以下の観点から価値が下がっている:

- `/track:*` コマンドが十分自己完結しており、各ステップで次に実行すべきコマンドを明示する
- Skill システム (`.claude/skills/`) やコマンドドキュメント (`.claude/commands/track/*.md`) が advisory 的な指示を吸収できる
- Claude Code 自身が track workflow を理解しているため、runtime hook で advisory する必要性が低い

fail-closed 系 (既に Rust 化済み) とは異なり、advisory 系 hook は「実行時にリマインド」する程度の役割であり、ドキュメント/skill への吸収で代替可能。

#### 3. `_agent_profiles.py` の二重実装

`libs/infrastructure/src/agent_profiles.rs` (Rust) と `.claude/hooks/_agent_profiles.py` (Python) の **2 つの loader 実装** が存在する。スキーマ変更 (後続 ADR `2026-04-09-2235-agent-profiles-redesign.md` で予定) 時に両方を更新する必要があり、不整合のリスクがある。

Python hook を全削除すれば `_agent_profiles.py` も不要になり、Rust loader のみが SoT となる。

#### 4. debugger capability 廃止による即時的な問題

後続 ADR `2026-04-09-2235-agent-profiles-redesign.md` で `debugger` capability の廃止が予定されている。`error-to-codex.py` はこの capability を前提とした advisory を出すため、debugger 廃止と同時に hook が論理的に壊れる。

この壊れた hook を個別に修正するより、本 ADR で Python hook 全削除を先に行うことで根本解決する。

### 実装順序上の位置付け

本 ADR は以下 2 つの後続 ADR の **prerequisite** として位置付ける:

1. **本 ADR** (Python hooks removal): 既存 Python hook を全削除
2. **ADR `2026-04-09-2235-agent-profiles-redesign.md`** (`.harness/config/` migration + agent-profiles v2): Python hook がもう存在しないため、Python 側の参照更新が不要となり、migration scope が縮小する
3. **ADR `2026-04-09-2047-planning-review-phase-separation.md`** (RV2-16): 新しい agent-profiles スキーマを使って `sotp review plan` を実装

この順序で実装することで、各 ADR のスコープが最小化され、移行時の不整合リスクも減る。

## Decision

### 1. `.claude/hooks/` 配下の Python hook を全削除

以下のファイルを全て削除する:

**Advisory / Informational hooks** (機能は skill/docs に吸収):
- `check-codex-before-write.py`
- `check-codex-after-plan.py`
- `error-to-codex.py`
- `post-implementation-review.py`
- `post-test-analysis.py`
- `suggest-gemini-research.py`
- `lint-on-save.py`
- `python-lint-on-save.py`
- `log-cli-tools.py`

**Libraries** (他に参照がない):
- `_agent_profiles.py`
- `_shared.py`

**Tests** (対象がなくなるため不要):
- `test_agent_profiles.py`
- `test_post_tool_hooks.py`
- `test_pre_tool_hooks.py`
- `test_helpers.py`
- `test_shared_hook_utils.py`

**最終的に `.claude/hooks/` ディレクトリは空になる → ディレクトリ自体も削除する**。

### 2. `.claude/settings.json` から Python hook エントリを削除

以下のコマンドを起動する `PreToolUse` / `PostToolUse` エントリを削除:

- `python3 "$CLAUDE_PROJECT_DIR/.claude/hooks/check-codex-before-write.py"`
- `python3 "$CLAUDE_PROJECT_DIR/.claude/hooks/suggest-gemini-research.py"`
- `python3 "$CLAUDE_PROJECT_DIR/.claude/hooks/check-codex-after-plan.py"`
- `python3 "$CLAUDE_PROJECT_DIR/.claude/hooks/error-to-codex.py"`
- `python3 "$CLAUDE_PROJECT_DIR/.claude/hooks/post-test-analysis.py"`
- `python3 "$CLAUDE_PROJECT_DIR/.claude/hooks/post-implementation-review.py"`
- `python3 "$CLAUDE_PROJECT_DIR/.claude/hooks/lint-on-save.py"`
- `python3 "$CLAUDE_PROJECT_DIR/.claude/hooks/python-lint-on-save.py"`
- `python3 "$CLAUDE_PROJECT_DIR/.claude/hooks/log-cli-tools.py"`

既存の Rust hook エントリ (`sotp hook dispatch`) は維持する。

### 3. advisory 機能の吸収先

削除する advisory hook が提供していた機能は、以下の方法で代替する:

| 削除する Python hook | 代替先 |
|---------------------|-------|
| `check-codex-before-write.py` | `.claude/rules/02-codex-delegation.md` で利用方針を明記 (既に存在) |
| `check-codex-after-plan.py` | `.claude/commands/track/plan.md` の SKILL で planner capability を明示 (既に存在) |
| `error-to-codex.py` | 廃止 (debugger capability 自体を削除するため対応不要) |
| `post-implementation-review.py` | `.claude/commands/track/implement.md` / `full-cycle.md` で review step を明示 (既に存在) |
| `post-test-analysis.py` | `.claude/commands/track/review.md` で test 失敗時のフローを明示 |
| `suggest-gemini-research.py` | `.claude/rules/03-gemini-delegation.md` で researcher 利用を明記 (既に存在) |
| `lint-on-save.py` | `cargo make ci` で CI 時に lint を実行 (既に存在) |
| `python-lint-on-save.py` | Python を全廃するため不要 |
| `log-cli-tools.py` | ログ取得は開発者の必要に応じて手動で実行 (default 不要) |

多くの hook が提供していた情報は既に他の場所 (rules, commands, skills) に存在するため、**実質的な機能損失はほぼない**。`log-cli-tools.py` のような debugging 用 hook は、必要になった時点で再実装 (Rust 化) すればよい。

### 4. Makefile.toml タスクの整理

以下のタスクを削除する:

- `hooks-selftest`: Python hook の pytest を実行するタスク (対象がなくなる)
- `python-lint`: Python hook / スクリプトの ruff lint を実行するタスク (Python hook がなくなるため不要)

ただし、`scripts/` 配下に他の Python スクリプトが残る場合は `python-lint` を部分的に残すか検討する (本 ADR の調査対象)。

### 5. CI / Docker の整理

- `cargo make ci` から Python hook 関連のステップを削除
- Dockerfile に `python3` インストールが残っているか確認し、他に依存がなければ削除
- `docker compose` の設定から Python 関連のボリュームマウント / 環境変数を整理

`scripts/` 配下に Python ファイルが残る場合や、ユーザー環境で `python3` が使われる他の箇所がある場合は、それらとの整合性を確認した上で段階的に除去する。

### 6. ドキュメント更新

- `CLAUDE.md`: Python 依存についての記述を削除または更新 (`python3 は optional` という表現を含め、Python 依存そのものを削除)
- `.claude/rules/`: hook による advisory の言及を削除
- `DEVELOPER_AI_WORKFLOW.md` / `knowledge/WORKFLOW.md`: Python hook の言及があれば削除
- `knowledge/conventions/`: Python hook 関連 convention があれば削除または更新
- `LOCAL_DEVELOPMENT.md`: Python セットアップ手順があれば削除

### 7. 後方互換性はサポートしない

本 ADR の変更は breaking change として扱う:

- `.claude/hooks/*.py` → 全削除 (legacy として残さない)
- `.claude/settings.json` の Python hook エントリ → 全削除
- Python 依存の graceful skip (`command -v python3 || exit 0`) の仕組み → 削除 (Python 自体を使わないため不要)
- Rollback パスは提供しない (git revert のみ)

理由: 個人開発 / 小規模運用で、Python hook に依存している外部ユーザーが想定されないため。

## Rejected Alternatives

### A. Python hook を Rust に全移行 (sotp hook dispatch 化)

全 10 個の advisory hook を Rust 化することも可能だが、以下の理由で却下:

- advisory hook の価値が既に低下しており、Rust 化しても維持コストが釣り合わない
- 各 advisory hook の機能は rules/skills/commands に吸収できる
- Rust 化は Python 削除より実装コストが高い (各 hook のロジックを Rust に書き直す必要)
- advisory は「忘れがちなことを思い出させる」ものであり、skill/docs に移せば runtime hook は不要

### B. 一部の Python hook を残す (例: lint-on-save.py だけ残す)

「どれを残すか」の線引きが曖昧になり、将来の判断が難しくなる。一度に全削除する方がクリーン。また、個別に残した hook も `_shared.py` や `_agent_profiles.py` に依存しているため、library ファイルを残す必要があり、全廃の意義が薄れる。

### C. `.claude/hooks/` ディレクトリは残して中身のみ空にする

空ディレクトリを残しても意味がない。git でも tracked できない (`.gitkeep` を置く必要がある)。完全削除が妥当。

### D. Python hook を別リポジトリに退避してアーカイブ

オーバーエンジニアリング。git history から過去の実装は参照可能であり、別リポジトリは管理コストが増えるだけ。

## Consequences

### Good

- **hook 層の Python 依存消滅**: `.claude/hooks/` に起因する Python runtime 依存が消える (本 ADR 単独で達成。`scripts/` Python と `requirements-python.txt` は後続の段階的除去トラックで対応)
- **後続 ADR のスコープ縮小**: `2026-04-09-2235-agent-profiles-redesign.md` が Python 側の参照更新を気にしなくてよくなる
- **二重実装の解消**: `_agent_profiles.py` と `agent_profiles.rs` の重複がなくなり、Rust loader が SoT となる
- **CI 高速化**: `hooks-selftest` タスクが削除され、`python-lint` の対象が `scripts/` のみに縮小されることで CI 時間が短縮
- **Docker image 軽量化**: Python 削除により image サイズが減る可能性

### Bad

- **advisory の喪失**: 一部の advisory メッセージが runtime で表示されなくなる (skills/docs に吸収されるが、リマインド頻度は下がる可能性)
- **debugging 機能の一時的な喪失**: `log-cli-tools.py` のような debug 用 hook を後で再実装する必要が発生するかもしれない
- **移行作業の広範囲化**: CI / Docker / ドキュメント等の更新が必要で、一時的に作業量が増える
- **git history の scatter**: 削除するファイルが多いため、本 ADR 実装の commit が大きくなる

## Reassess When

- 削除後、特定の advisory が不足しているとユーザーから feedback があった場合: 該当機能を rules/skills/commands に明示的に吸収するか、必要なら Rust hook として再実装
- Python スクリプトを他の用途 (例: データ解析、プロトタイプ) で復活させたい場合: `.harness/` や `scripts/` 配下で独立管理し、`.claude/hooks/` の runtime hook 用途とは切り離す
- Claude Code が将来 hook system を拡張し、Python 以外の hook 実装方法を提供した場合: 本 ADR の判断を再評価
- `scripts/` 配下の Python スクリプト状況によって `python-lint` や `python3` 依存を部分的に残す必要がある場合: 本 ADR 実装時に調査結果を反映

## Related

- **ADR `2026-04-09-2235-agent-profiles-redesign.md`** (本 ADR の後続): agent-profiles スキーマ再設計。本 ADR 完了後に実装することで、`_agent_profiles.py` を気にせず Rust loader のみを更新すれば済む
- **ADR `2026-04-09-2047-planning-review-phase-separation.md`** (RV2-16): planning review 機能の追加。上記 agent-profiles 再設計完了後に実装する
- TODO.md の RV2-17 エントリ
