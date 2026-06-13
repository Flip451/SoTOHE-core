---
name: adr-editor
description: Use when Codex is assigned the SoTOHE ADR editor capability during a back-and-forth planning loop. Edits a target ADR only when a downstream SoT signal needs a persistent decision clarification.
---

# ADR-Editor (Codex skill)

**Operational SSoT:** read and follow `.harness/capabilities/adr-editor.md` — the provider-agnostic
contract for this capability. Do not duplicate it here.

## Codex-skill notes

- Invoked when Codex is assigned the `adr-editor` capability (`.codex/agents/adr-editor.toml`).
