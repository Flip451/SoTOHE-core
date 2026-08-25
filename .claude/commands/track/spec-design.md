---
description: Author the track's spec.json via the spec-designer subagent (Phase 1).
---

> Operational SSoT: `.harness/workflows/track/spec-design.md` — provider 非依存 workflow logic はそちらを参照。本ファイルは Claude Code 固有 adapter として、起動形態 / Tool 制約 / 報告形式のみを残す。

## Invocation

User invokes this command as `/track:spec-design`. No arguments.

## Claude Code invocation constraints

- **Context intake**: follow the spec-design workflow SSoT's `Summary-first context intake`.
  Use CLI summaries as the primary context; do not bulk-read `spec.json`, review or binding
  JSON, catalogues, full sub-workflow texts, or a `Related Conventions` list. Open only a
  targeted diff or the artifact body named by a blocker; the dispatcher supplies exact
  convention paths to the delegated capability.

Write a briefing to `tmp/spec-designer-briefing.md` containing:

- Track id and `track/items/<track-id>/metadata.json` path
- Paths to the referenced ADR(s) under `knowledge/adr/`

Do not put convention paths in the briefing: the capability dispatcher resolves the
`spec-designer` convention set and delivers it with the dispatch, and that resolution is the
complete convention input (workflow SSoT § Inputs).

Then run `bin/sotp phase enter spec-design`. Phase entry runs its declared convergence checks
and launches the configured writer only after they pass. Do not launch the writer from this
adapter.

The capability owns: writing `spec.json`, rendering `spec.md`, and evaluating the spec → ADR signal (🔵🟡🔴).

## Report format

Report: track id, `spec.json` path, signal counts (blue / yellow / red).
