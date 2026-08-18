---
name: adr-diagnoser
sandbox: read-only
grok-sandbox: read-only
description: >-
  Use when Codex is assigned the SoTOHE `adr-diagnoser` capability — the guardian of ADR
  decisions under the two-box model. Four read-only verdict modes; (1) Phase 0 edit judgment:
  does an applied in-place edit preserve or break the recorded decisions (supplying a
  preserving alternative or a no-change rationale when breaking); (2) Phase 1+ delta admission:
  the three-way admit / bounce-with-resolution / modification-proposal judgment on track-born
  draft candidates; (3) classification & conformance: semantic vs non-semantic classification
  of applied input-box fixes, and conformance re-audits of user-decision implementation edits
  (adoption / rejection / restoration); (4) mismatch classification: non-semantic restamp,
  deviation, or unknown-editor for unexpected baseline divergence. Always returns a structured
  read-only verdict for the orchestrator.
---

# ADR-Diagnoser (Codex skill)

**Operational SSoT:** read and follow `.harness/capabilities/adr-diagnoser.md` — the
provider-agnostic contract for this capability. Do not duplicate it here.

## Codex-skill notes

- Invoked when Codex is assigned the `adr-diagnoser` capability.
- Read-only posture: inspect the briefing, current ADR, latest baseline, ledger, and diff with
  `cat` / `grep` / `rg`; do not write files or run mutating `bin/sotp` commands.
- The terminal output is only the structured verdict object the operational SSoT requires for
  the invoked mode (edit judgment / delta admission / classification & conformance /
  mismatch classification). Uncertainty always takes each mode's fail-closed branch.
- Never run `bin/sotp adr-baseline snapshot` or `bin/sotp adr-baseline restore`; the
  orchestrator performs those actions after consuming the verdict.

## Session resume conformance

- If the dispatch is resumed, follow the operational SSoT's Session resume section before
  returning a verdict.
