---
description: Stage review is complete and create a guarded commit, then attach a git note.
---

> Operational SSoT: `.harness/workflows/track/commit.md` — provider 非依存 workflow logic はそちらを参照。本ファイルは Claude Code 固有 adapter として、起動形態 / Tool 制約 / 報告形式のみを残す。

## Invocation

User invokes this command as `/track:commit`. Use `$ARGUMENTS` as the commit message. If empty, inspect the current track and propose 2–3 commit message candidates, then stop.

## Claude Code invocation constraints

This command runs directly — no subagents. Key wrappers used:

- `git diff --cached --stat` (read-only — verify staged scope)
- `git show HEAD --stat` (read-only — changed files for git note)
- Write commit message to `tmp/track-commit/commit-message.txt` (Read + Edit preferred)
- `cargo make track-commit-message` — guarded commit (CI + commit)
- Write note to `tmp/track-commit/note.md`, then `cargo make track-note`

`track/registry.md` is gitignored — do NOT stage or commit it.

## Report format

After execution, report:

1. Commit result (success/failure) and commit hash.
2. Commit message used.
3. `track/registry.md` status: regenerated locally / already current / skipped (gitignored).
4. Git note status: applied or skipped (reason).
5. Next recommended action.
