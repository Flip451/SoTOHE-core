---
name: adr-diagnoser
sandbox: read-only
description: Use when Codex is assigned the SoTOHE `adr-diagnoser` capability. It classifies an ADR-baseline mismatch as a non-semantic restamp, a deviation, or an unknown-editor case and returns a structured read-only verdict for the orchestrator.
---

# ADR-Diagnoser (Codex skill)

**Operational SSoT:** read and follow `.harness/capabilities/adr-diagnoser.md` — the
provider-agnostic contract for this capability. Do not duplicate it here.

## Codex-skill notes

- Invoked when Codex is assigned the `adr-diagnoser` capability.
- Read-only posture: inspect the briefing, current ADR, latest baseline, ledger, and diff with
  `cat` / `grep` / `rg`; do not write files or run mutating `bin/sotp` commands.
- The terminal output is only the structured `verdict`, `reason`, and
  `recommended_next_action` object required by the operational SSoT.
- Never run `bin/sotp adr-baseline snapshot` or `bin/sotp adr-baseline restore`; the
  orchestrator performs those actions after consuming the verdict.

## Session resume conformance

- If the dispatch is resumed, follow the operational SSoT's Session resume section before
  returning a verdict.
