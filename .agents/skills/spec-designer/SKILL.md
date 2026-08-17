---
name: spec-designer
sandbox: workspace-write
grok-sandbox: workspace
description: Use when Codex is assigned the SoTOHE Phase 1 spec-designer capability. Writes the behavioral contract spec.json from ADRs and conventions, regenerates the rendered spec view through the SoTOHE CLI, and reports signal counts.
---

# Spec-Designer (Codex skill)

**Operational SSoT:** read and follow `.harness/capabilities/spec-designer.md` — the provider-agnostic
contract for this capability. Do not duplicate it here.

## Codex-skill notes
- Invoked when Codex is assigned the `spec-designer` capability (`.codex/agents/spec-designer.toml`).

## Session resume conformance

- If your dispatch is a resumed session (orchestrator opt-in continuation), follow the
  "Session resume" section of the capability SSoT: check whether your upstream artifacts
  changed since the prior session and re-read any that did before continuing.
