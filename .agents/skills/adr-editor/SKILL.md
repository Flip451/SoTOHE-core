---
name: adr-editor
sandbox: workspace-write
description: Use when Codex is assigned the SoTOHE ADR editor capability — the single in-track writer for knowledge/adr/*.md under the two-box model. Applies Phase 0 convergence edits on input-box ADRs, authors / revises / deletes Phase 1+ delta candidates (track-born draft ADRs), applies non-semantic in-place fixes, and implements explicit user adjudications (grounds promotion, rejection deletion or revision, corrective restoration). Every applied edit is judged or re-audited afterwards by adr-diagnoser; this skill edits the working tree only and never commits or snapshots.
---

# ADR-Editor (Codex skill)

**Operational SSoT:** read and follow `.harness/capabilities/adr-editor.md` — the provider-agnostic
contract for this capability. Do not duplicate it here.

## Codex-skill notes

- Invoked when Codex is assigned the `adr-editor` capability (`.codex/agents/adr-editor.toml`).

## Session resume conformance

- If your dispatch is a resumed session (orchestrator opt-in continuation), follow the
  "Session resume" section of the capability SSoT: check whether your upstream artifacts
  changed since the prior session and re-read any that did before continuing.
