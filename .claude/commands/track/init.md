---
description: Initialize a new track directory and its branch (Phase 0).
---

> Operational SSoT: `.harness/workflows/track/init.md` — provider 非依存 workflow logic はそちらを参照。本ファイルは Claude Code 固有 adapter として、起動形態 / Tool 制約 / 報告形式のみを残す。

## Invocation

User invokes this command as `/track:init <feature> --primary-adr <filename>.md`. Parse the
feature name (or slug-ready phrase) and require the direct primary ADR filename under
`knowledge/adr/`; if either input is absent, stop and ask for it. The caller selects and passes
this filename explicitly — do not derive it from the feature name — and the workflow snapshots it
after track creation to create the ledger init designation record; no separate primary pointer is
retained.

## Claude Code invocation constraints

This command runs directly — no subagents. Key Bash wrappers used:

- `git branch --show-current`, `git status --short` (read-only pre-flight)
- `bin/sotp track branch create --items-dir track/items '<track-id>'`
- `bin/sotp track views sync`
- `bin/sotp adr-baseline snapshot --source '<primary-adr-file>.md' --kind init`
- `cargo make verify-track-metadata`

## Report format

Report: track id, track directory, branch name, primary ADR baseline result, `verify-track-metadata` result.
