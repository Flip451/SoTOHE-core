---
description: Author per-layer type catalogues via the type-designer subagent (Phase 2).
---

> Operational SSoT: `.harness/workflows/track/type-design.md` — provider 非依存 workflow logic はそちらを参照。本ファイルは Claude Code 固有 adapter として、起動形態 / Tool 制約 / 報告形式のみを残す。

## Invocation

User invokes this command as `/track:type-design`. No arguments.

## Claude Code invocation constraints

Write a briefing to `tmp/type-designer-briefing.md` containing:

- Track id and `track/items/<track-id>/spec.json` path
- `architecture-rules.json` path (source of truth for TDDD-enabled layers)
- Paths to the related ADR(s) under `knowledge/adr/`

Do not put convention paths in the briefing: the capability dispatcher resolves the
`type-designer` convention set and delivers it with the dispatch, and that resolution is the
complete convention input (workflow SSoT § Inputs).

Then run `bin/sotp capability exec type-designer --host claude --briefing-file tmp/type-designer-briefing.md`.
The dispatcher resolves `capabilities.type-designer` internally from
`.harness/config/agent-profiles.json` and either completes the provider dispatch or returns
`CAPABILITY_EXEC_OUTCOME: delegate-in-host`; only on that outcome invoke the Agent tool
(`subagent_type: "type-designer"`, `run_in_background: true`) with the briefing path and
discipline body as the task prompt. Never invoke the Agent-tool subagent without that
delegation outcome; this adapter must not resolve or assume the provider itself.

The capability owns: baseline capture, each `<layer>-types.json` write, all rendered views, and the type → spec signal evaluation (🔵🟡🔴).

## Report format

Report: track id, processed layers and their catalogue file paths, signal counts per layer (blue / yellow / red).
