# Guardrails

Core guardrails:

- Prefer `/track:*` in user-facing guidance
- Do not use direct `git add` / `git commit`
- Do not tell users to run `*-local` tasks directly
- Keep `track/tech-stack.md` free of blocking `TODO:` before implementation
- Keep `track/registry.md`, `spec.md`, `plan.md`, and `verification.md` synchronized
- Keep `cargo make ci`, `cargo make deny`, and `cargo make verify-*` as reproducible final gates (`run --rm`)
- Before committing code changes, run the `reviewer` capability review cycle
  (review -> fix -> review -> ... -> no findings). Do not commit until the reviewer
  reports zero findings. The reviewer provider is resolved via `.claude/agent-profiles.json`.

## Permission Guardrails

`scripts/verify_orchestra_guardrails.py` の `FORBIDDEN_ALLOW` リストにより、以下のシェルコマンドは `.claude/settings.json` の `permissions.allow` に追加できない:

- `Bash(ls:*)`, `Bash(cat:*)`, `Bash(find:*)`, `Bash(grep:*)`, `Bash(head:*)`, `Bash(tail:*)` など — 対応する専用ツール（`Glob`, `Read`, `Grep`）を使うこと
- `Bash(cd:*)` — 各ツールの `path` パラメータを使うこと
- `Bash(echo:*)`, `Bash(pwd:*)` — テキスト出力は直接応答、パス確認は `Glob` を使うこと
- `Bash(git add:*)`, `Bash(git commit:*)` 等 — `cargo make` wrapper を使うこと

ユーザーからこれらの許可追加を求められた場合は、`FORBIDDEN_ALLOW` で禁止されている旨を説明し、代替ツールを案内すること。
プロジェクト固有の拡張が必要な場合は `.claude/permission-extensions.json` の `extra_allow` に追加するが、`FORBIDDEN_ALLOW` に該当するエントリは拒否される。

## Hook Constraint

The `sotp hook dispatch block-direct-git-ops` guard scans the entire Bash command string for protected git-operation keywords.
This includes string literals, prompt text, and heredocs.

To avoid unnecessary retries:

- `python3 -c`: do not embed code containing protected git keywords. Write a `.py` file, then run it.
- `codex exec` / `gemini -p`: do not embed prompts containing protected git keywords. Write the prompt to a file first.
- heredoc / `cat >`: also scanned. Use the Write/Edit tool instead.
- **新規ファイル作成**: `Write` ツールは未読ファイルへの書き込みを拒否するため、まず `Read` で対象パスを読む（存在しない場合はエラーが返る）。その後 `Write` で作成できる。`touch` は `FORBIDDEN_ALLOW` に含まれるため `allow` に追加してはならない。
- Fallback: when Codex review is blocked by the hook, write the prompt to a file and retry with `--briefing-file`.

## Reviewer Capability Constraint

`reviewer` capability は `.claude/agent-profiles.json` で定義された外部 provider に委譲する。
Claude Code の主コンテキスト内でのインラインレビュー（self-review）は `reviewer` capability の代替にならない。

- 外部 reviewer（Codex CLI 等）が verdict を返さなかった場合 → **リトライする**（最大2回）
- リトライしても失敗する場合 → **ユーザーに報告して判断を仰ぐ**
- 主コンテキスト内のインラインレビューで `zero_findings` を達成したと判断してコミットしてはならない
- Claude Code subagent (`subagent_type: "Explore"`) による reviewer 代替は `claude-heavy` profile（`reviewer: "claude"`）の場合のみ有効
- Hook によるブロックとは区別する: Hook ブロックはプロンプトの書き方の問題（ファイル経由で回避可能）、verdict 抽出失敗は外部 provider の実行問題（リトライで対応）

Operational details live in:

- `track/workflow.md`
- `.claude/docs/WORKFLOW.md`
- `.claude/settings.json`
- `.claude/hooks/`
