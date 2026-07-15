---
name: adr-editor
sandbox: workspace-write
description: Use when Codex is assigned the SoTOHE ADR editor capability during a back-and-forth planning loop. Edits a target ADR only when a downstream SoT signal needs a persistent decision clarification.
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
