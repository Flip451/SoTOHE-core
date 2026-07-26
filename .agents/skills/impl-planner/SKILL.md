---
name: impl-planner
sandbox: workspace-write
description: Use when Codex is assigned the SoTOHE Phase 3 impl-planner capability. Writes impl-plan.json, task-coverage.json, and task-contract.json from the spec and type catalogues, then verifies plan artifact coverage.
---

# Impl-Planner (Codex skill)

**Operational SSoT:** read and follow `.harness/capabilities/impl-planner.md` — the provider-agnostic
contract for this capability. Do not duplicate it here.

## Codex-skill notes

- Invoked when Codex is assigned the `impl-planner` capability (`.codex/agents/impl-planner.toml`).

## Session resume conformance

- If your dispatch is a resumed session (orchestrator opt-in continuation), follow the
  "Session resume" section of the capability SSoT: check whether your upstream artifacts
  changed since the prior session and re-read any that did before continuing.
