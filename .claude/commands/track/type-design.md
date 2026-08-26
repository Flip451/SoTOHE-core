---
description: Author per-layer type catalogues via the type-designer subagent (Phase 2).
---

> Operational SSoT: `.harness/workflows/track/type-design.md` — provider 非依存 workflow logic はそちらを参照。本ファイルは Claude Code 固有 adapter として、起動形態 / Tool 制約 / 報告形式のみを残す。

## Invocation

User invokes this command as `/track:type-design`. No arguments.

## Claude Code invocation constraints

- **Context intake**: follow the type-design workflow SSoT's `Summary-first context intake`.
  Use CLI summaries as the primary context; do not bulk-read `*-types.json`, review or binding
  JSON, full sub-workflow texts, or a `Related Conventions` list. Open only a targeted diff or
  the body named by a blocker; the dispatcher supplies exact catalogue and convention paths to
  the delegated capability.

Write a briefing to `tmp/type-designer-briefing.md` containing:

- Track id and `track/items/<track-id>/spec.json` path
- `architecture-rules.json` path (source of truth for TDDD-enabled layers)
- Paths to the related ADR(s) under `knowledge/adr/`

Do not put convention paths in the briefing: the capability dispatcher resolves the
`type-designer` convention set and delivers it with the dispatch, and that resolution is the
complete convention input (workflow SSoT § Inputs).

Then run `bin/sotp phase enter type-design`. Phase entry runs its declared convergence checks
and launches the configured writer only after they pass. Do not launch the writer from this
adapter.

The capability owns: baseline capture, each `<layer>-types.json` write, all rendered views, and the type → spec signal evaluation (🔵🟡🔴).

## Report format

Report: track id, processed layers and their catalogue file paths, signal counts per layer (blue / yellow / red).
