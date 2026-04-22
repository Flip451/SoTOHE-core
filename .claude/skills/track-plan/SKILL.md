---
name: track-plan
description: |
  Plan a new Rust feature via the canonical track planning workflow.
  `/track:plan` is a state-machine orchestrator that invokes the four
  independent phase commands in order: /track:init (Phase 0) →
  /track:spec-design (Phase 1) → /track:type-design (Phase 2) →
  /track:impl-plan (Phase 3). Each phase command delegates to its writer
  capability. Triggers automatically when the user invokes /track:plan.
metadata:
  short-description: Rust feature kickoff — pre-track ADR + /track:init /spec-design /type-design /impl-plan
---

# `/track:plan` Backing Skill

**The implementation details for this skill have moved to `.claude/commands/track/plan.md`.**

This file is retained as the skill registry entry so that `/track:plan` continues to be
discoverable as a named skill. The full workflow specification (SoT Chain, phase lifecycle,
provider routing, ADR pre-check, back-and-forth loop) lives in the command doc.

See `.claude/commands/track/plan.md` for the complete orchestration contract.
