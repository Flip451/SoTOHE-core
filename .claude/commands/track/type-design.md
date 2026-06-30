---
description: Author per-layer type catalogues via the type-designer subagent (Phase 2).
---

> Operational SSoT: `.harness/workflows/track/type-design.md` — provider 非依存 workflow logic はそちらを参照。本ファイルは Claude Code 固有 adapter として、起動形態 / Tool 制約 / 報告形式のみを残す。

## Invocation

User invokes this command as `/track:type-design`. No arguments.

## Claude Code invocation constraints

Invoke the type-designer via the Agent tool (`subagent_type: "type-designer"`, `run_in_background: true`). Briefing must include:

- Track id and `track/items/<track-id>/spec.json` path
- `architecture-rules.json` path (source of truth for TDDD-enabled layers)
- Paths to the related ADR(s) under `knowledge/adr/` and conventions under `knowledge/conventions/`

The subagent owns: baseline capture, each `<layer>-types.json` write, all rendered views, and the type → spec signal evaluation (🔵🟡🔴). No direct CLI calls from this adapter body.

## Report format

Report: track id, processed layers and their catalogue file paths, signal counts per layer (blue / yellow / red).
