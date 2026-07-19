---
name: adr-diagnoser
sandbox: read-only
description: Use when Codex is assigned the SoTOHE `adr-diagnoser` capability — the guardian of ADR decisions. In edit-judgment mode it judges whether an ADR edit inside the Phase 0 baseline-review loop preserves or breaks the recorded decisions (supplying a decision-preserving alternative or a no-change rationale when breaking; Phase 1+ expected escalation edits stamp directly and are not edit-judgment inputs); in mismatch mode it classifies an unexpected ADR-baseline byte mismatch as non-semantic restamp, deviation, or unknown-editor. Always returns a structured read-only verdict for the orchestrator.
---

# ADR-Diagnoser (Codex skill)

**Operational SSoT:** read and follow `.harness/capabilities/adr-diagnoser.md` — the
provider-agnostic contract for this capability. Do not duplicate it here.

## Codex-skill notes

- Invoked when Codex is assigned the `adr-diagnoser` capability.
- Read-only posture: inspect the briefing, current ADR, latest baseline, ledger, and diff with
  `cat` / `grep` / `rg`; do not write files or run mutating `bin/sotp` commands.
- The terminal output is only the structured verdict object the operational SSoT requires for
  the invoked mode: edit judgment returns `verdict` / `reason` (plus exactly one of
  `alternative` / `no_change_rationale` when the verdict is `decision-breaking`); mismatch
  classification returns `verdict` / `reason` / `recommended_next_action`.
- Never run `bin/sotp adr-baseline snapshot` or `bin/sotp adr-baseline restore`; the
  orchestrator performs those actions after consuming the verdict.

## Session resume conformance

- If the dispatch is resumed, follow the operational SSoT's Session resume section before
  returning a verdict.
