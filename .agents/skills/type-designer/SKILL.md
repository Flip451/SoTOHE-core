---
name: type-designer
description: Use when Codex is assigned the SoTOHE Phase 2 type-designer capability. Writes per-layer TDDD type catalogues from the spec, ADRs, baselines, and type-design conventions, then verifies type and catalogue-spec signals.
---

# Type-Designer (Codex skill)

**Operational SSoT:** read and follow `.harness/capabilities/type-designer.md` — the provider-agnostic
contract for this capability. Do not duplicate it here.

## Codex-skill notes
- Invoked when Codex is assigned the `type-designer` capability (`.codex/agents/type-designer.toml`).
- Run the canonical pipeline and the 12a/12b/12c self-verification gates from the shared SSoT before returning.
