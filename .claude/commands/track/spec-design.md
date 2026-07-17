---
description: Author the track's spec.json via the spec-designer subagent (Phase 1).
---

> Operational SSoT: `.harness/workflows/track/spec-design.md` — provider 非依存 workflow logic はそちらを参照。本ファイルは Claude Code 固有 adapter として、起動形態 / Tool 制約 / 報告形式のみを残す。

## Invocation

User invokes this command as `/track:spec-design`. No arguments.

## Claude Code invocation constraints

Write a briefing to `tmp/spec-designer-briefing.md` containing:

- Track id and `track/items/<track-id>/metadata.json` path
- Paths to the referenced ADR(s) under `knowledge/adr/`
- Paths to the related conventions under `knowledge/conventions/`

Then run `bin/sotp capability exec spec-designer --host claude --briefing-file tmp/spec-designer-briefing.md`.
The dispatcher resolves `capabilities.spec-designer` internally from
`.harness/config/agent-profiles.json` and either completes the provider dispatch or returns
`CAPABILITY_EXEC_OUTCOME: delegate-in-host`; only on that outcome invoke the Agent tool
(`subagent_type: "spec-designer"`, `run_in_background: true`) with the briefing path and
discipline body as the task prompt. Never invoke the Agent-tool subagent without that
delegation outcome; this adapter must not resolve or assume the provider itself.

The capability owns: writing `spec.json`, rendering `spec.md`, and evaluating the spec → ADR signal (🔵🟡🔴).

## Report format

Report: track id, `spec.json` path, signal counts (blue / yellow / red).
