---
name: type-designer
sandbox: workspace-write
grok-sandbox: workspace
description: Use when Codex is assigned the SoTOHE Phase 2 type-designer capability. Produces per-layer TDDD type catalogues via the generate + annotate workflow (`sotp catalog`) from the spec, ADRs, baselines, and type-design conventions, then verifies type and catalogue-spec signals.
---

# Type-Designer (Codex skill)

**Operational SSoT:** read and follow `.harness/capabilities/type-designer.md` — the provider-agnostic
contract for this capability. Do not duplicate it here.

## Codex-skill notes
- Invoked when Codex is assigned the `type-designer` capability (`.codex/agents/type-designer.toml`).
- Run the canonical pipeline and the 12a/12b/12c self-verification gates from the shared SSoT before returning.
- Empty-layer 12a receipt: a layer with zero public items has no `<layer>-graph-d2/` output; its `<layer>-graph-d1/index.md` plus `baseline-graph` exit 0 satisfies 12a (see the SSoT's step 2 / 12a wording).

## Session resume conformance

- If your dispatch is a resumed session (orchestrator opt-in continuation), follow the
  "Session resume" section of the capability SSoT: check whether your upstream artifacts
  changed since the prior session and re-read any that did before continuing.
