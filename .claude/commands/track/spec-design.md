---
description: Author the track's spec.json via the spec-designer subagent (Phase 1).
---

> Operational SSoT: `.harness/workflows/track/spec-design.md` — provider 非依存 workflow logic はそちらを参照。本ファイルは Claude Code 固有 adapter として、起動形態 / Tool 制約 / 報告形式のみを残す。

## Invocation

User invokes this command as `/track:spec-design`. No arguments.

## Claude Code invocation constraints

Invoke the spec-designer via the Agent tool (`subagent_type: "spec-designer"`, `run_in_background: true`). Briefing must include:

- Track id and `track/items/<track-id>/metadata.json` path
- Paths to the referenced ADR(s) under `knowledge/adr/`
- Paths to the related conventions under `knowledge/conventions/`

The subagent owns: writing `spec.json`, rendering `spec.md`, and evaluating the spec → ADR signal (🔵🟡🔴). No direct CLI calls from this adapter body.

## Report format

Report: track id, `spec.json` path, signal counts (blue / yellow / red).
