---
name: impl-planner
model: claude-opus-4-7[1m]
effort: max
tools:
  - Read
  - Grep
  - Glob
  - Write
  - Edit
  - Bash
  - WebFetch
  - WebSearch
description: |
  Phase 3 writer for /track:impl-plan. Authors `impl-plan.json` (tasks + plan.sections) and `task-coverage.json` (spec element ↔ task mapping) from the existing `spec.json` and per-layer type catalogues, writes them directly, and evaluates the task-coverage binary gate internally. Does NOT re-open Phase 1 spec decisions or Phase 2 type decisions — if either is ambiguous, raise it as an open question so the orchestrator can run the back-and-forth loop. Mirrors the `impl-planner` capability in `.harness/config/agent-profiles.json` and declares explicit Opus routing via frontmatter.
---

# Impl-Planner Agent

**Operational SSoT:** read and follow `.harness/capabilities/impl-planner.md` — the provider-agnostic
contract for this capability (mission, contract, design principles, scope ownership, rules). Do not
duplicate it here.

## Claude-subagent notes

- You run as a Claude subagent (`subagent_type: "impl-planner"`); model/tools come from the frontmatter above.

## Session resume conformance

- If your dispatch is a resumed session (orchestrator opt-in continuation), follow the
  "Session resume" section of the capability SSoT: check whether your upstream artifacts
  changed since the prior session and re-read any that did before continuing.
