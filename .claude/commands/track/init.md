---
description: Initialize a new track directory and its branch (Phase 0).
---

> Operational SSoT: `.harness/workflows/track/init.md` — provider 非依存 workflow logic はそちらを参照。本ファイルは Claude Code 固有 adapter として、起動形態 / Tool 制約 / 報告形式のみを残す。

## Invocation

User invokes this command as `/track:init`. Use `$ARGUMENTS` as the feature name (or a slug-ready phrase). If empty, ask for a feature name and stop.

## Claude Code invocation constraints

This command runs directly — no subagents. Key Bash wrappers used:

- `git branch --show-current`, `git status --short` (read-only pre-flight)
- `cargo make track-branch-create '<track-id>'`
- `bin/sotp track views sync`
- `cargo make verify-track-metadata`

## Report format

Report: track id, track directory, branch name, `verify-track-metadata` result.
