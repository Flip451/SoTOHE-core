---
name: track-plan
description: |
  Plan a new Rust feature via the canonical track planning workflow.
  Orchestrates Phase 0 (init) → Phase 1 (spec) → Phase 2 (design) →
  Phase 3 (impl-plan), delegating each phase to its assigned writer capability.
  Triggers automatically when the user invokes /track:plan.
metadata:
  short-description: Rust feature kickoff — pre-track ADR + Phase 0-3 planning
---

# `/track:plan` Backing Skill

**The implementation details for this skill have moved to `.claude/commands/track/plan.md`.**

This file is retained as the skill registry entry so that `/track:plan` continues to be
discoverable as a named skill. The full workflow specification (SoT Chain, phase lifecycle,
provider routing, ADR pre-check, back-and-forth loop) lives in the command doc.

See `.claude/commands/track/plan.md` for the complete orchestration contract.
