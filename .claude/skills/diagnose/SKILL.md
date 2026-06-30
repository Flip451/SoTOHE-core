---
name: track-diagnose
description: |
  One-shot diagnostic skill that runs when the impl-phase or later surfaces a structural
  inconsistency the internal signal pipeline cannot localize on its own. Triggers
  automatically when `bin/sotp task-contract check` (PreReviewGate) returns
  `PreReviewGateOutcome::Blocked`, and may also be invoked when step 6
  `/track:review` findings on any SoT scope (adr/spec/types/impl-plan) or
  external PR-reviewer comments need
  back-and-forth routing. Reads the SoT chain (ADR → spec → catalogue → impl-plan →
  source), judges the most upstream phase where the root cause originates, and
  returns a structured routing decision the calling orchestrator dispatches. This
  skill never invokes a writer subagent itself.
metadata:
  short-description: Phase-rollback routing for impl-phase structural inconsistencies — returns routing_target + reason + recommended_next_action; orchestrator dispatches.
---

# `/track:diagnose` Backing Skill

**The implementation details for this skill have moved to `.claude/commands/track/diagnose.md`.**

This file is retained as the skill registry entry so that `/track:diagnose` continues to be
discoverable as a named skill. The full workflow specification (trigger inputs, mandatory
context-file pre-read, LLM-semantic routing taxonomy, output contract, orchestrator dispatch
boundary) lives in the command doc.

See `.claude/commands/track/diagnose.md` for the orchestrator-facing slash command spec and
`.harness/capabilities/rollback-diagnoser.md` for the provider-agnostic operational SSoT.
