---
description: Drive a prepared ADR all the way to a reviewed PR — init → review → commit → plan → review → commit → full-cycle → pr-review, autonomously (no merge).
---

Canonical command for taking a prepared ADR through the whole track lane to a reviewed PR.

This is a **thin orchestrator**: it sequences existing `/track:*` sub-commands and adds only the adr2pr-specific ordering and constraints. It does **not** duplicate sub-command internals — each step delegates to its canonical command, which owns its own gates / parallelism / writer ownership / back-and-forth escalation. Read those commands; do not re-state their logic here.

**Precondition**: a pre-track ADR already exists under `knowledge/adr/` (the track seed; see `knowledge/conventions/pre-track-adr-authoring.md`). If no relevant ADR exists, stop and ask the user to author one (`/adr:add <slug>`).

## Preamble: register the chain with TaskCreate

Use `TaskCreate` to register the following steps as tasks (in order), then execute them sequentially. Mark each `in_progress` before starting and `completed` after its gate passes. Append a sub-task for any back-and-forth loop a sub-command triggers (e.g. spec → ADR 🔴 escalation).

1. `/track:init` — initialize the track (auto-decide the slug). **Skip this step if the track is already initialized** (already on a `track/<id>` branch with `metadata.json`).
2. `/track:review` — review the ADR baseline. This follows the flow recommended in `/track:init` step 6 (`/track:review → /track:commit` immediately after init). `spec.md` does not exist yet (created in step 5, Phase 1); `/track:review` Step 0 reads available context files — at this stage that is `plan.md`, `metadata.json`, and the ADR files in the working tree. The absence of `spec.md` does not block the review; the reviewer scopes to what exists.
3. Stage: run `cargo make add-all` after the final review round (per the canonical staging-order rule in `/track:commit` Step 1).
4. `/track:commit <message>` — commit the ADR + metadata (first commit). Generate the commit message yourself per Constraint 2 (no candidate presentation).
5. Phase 1-3 — invoke `/track:spec-design`, `/track:type-design`, and `/track:impl-plan` directly in sequence. Each phase command is single-shot (invoke once, receive signal/gate result). Back-and-forth escalation after 🔴/ERROR is the caller's responsibility; apply the loop rules as defined in `/track:plan`'s "Phase 1 loop", "Phase 2 loop", and "Phase 3 loop" sections (read those sections; do not re-state them here). Do **not** invoke `/track:plan`: after step 1 the orchestrator is on a `track/<id>` branch, so `/track:plan` would fail at its Phase 0 (`/track:init` requires `main`); and when back-and-forth adr-editor edits produce a non-empty ADR working-tree diff, `/track:plan`'s Termination pauses for a user decision (violates Constraint 2). Do **not** commit here; plan artifacts are staged at step 7 and committed at step 8.
6. `/track:review` — review the plan artifacts.
7. Stage: run `cargo make add-all` after the final review round.
8. `/track:commit <message>` — commit the plan artifacts. Generate the commit message yourself per Constraint 2.
9. `/track:full-cycle` — per-task implement → review → commit.
10. `/track:pr-review` — final PR-based review (whole track branch vs `main`).

## Step 0 (before executing any step): build the execution plan

Read every referenced sub-command definition (`/track:init`, `/track:review`, `/track:commit`, `/track:spec-design`, `/track:type-design`, `/track:impl-plan`, `/track:full-cycle`, `/track:pr-review`) and extract their gates / decision points / parallelism rules into a concrete execution plan — the same discipline `/track:full-cycle` Step 0 mandates. Treat them as a state machine to execute, not background reading.

## Orchestration rules (adr2pr-specific only)

- Execute steps 1-10 in order; each step's gate must pass before the next begins. If a step fails irrecoverably, stop and report (the autonomy in Constraint 2 covers decision-making, not ignoring hard failures).
- **Step 1 is conditional**: if init is already done (the track exists), skip step 1 and start at step 2.
- **Step 5**: Phase 1-3 are driven by invoking `/track:spec-design`, `/track:type-design`, and `/track:impl-plan` directly in sequence (not via `/track:plan`). Each phase command is single-shot; this orchestrator runs the back-and-forth escalation loops per `/track:plan`'s "Phase 1 loop", "Phase 2 loop", and "Phase 3 loop" sections (authoritative; do not re-state). `/track:plan` is not invoked because Phase 0 (`/track:init`) requires `main` and the Termination section can pause for a user decision when ADR working-tree diffs are present (violates Constraint 2). Plan artifacts are staged at step 7 and committed at step 8.
- All gate semantics (🔵🟡🔴 signals, OK/ERROR binary gates, per-scope fast→final review, upstream-writer escalation) are owned by the sub-commands. Follow them as defined; do not weaken them.

## Constraints

1. **No merge.** The chain stops at `/track:pr-review`. Never invoke `/track:merge` / `cargo make track-pr-merge`. Leave the PR open for the user to merge.
2. **Fully autonomous.** Never pause for user confirmation at any step; run to the end. Decide all commit messages yourself (no candidate presentation).
3. **CI-driven bundling allowed.** `/track:full-cycle` is per-task by default; when tightly-coupled tasks cannot pass `cargo make ci` individually, bundling them into a single implement → review → commit cycle is permitted.
4. **Signals must be resolved — non-negotiable.** Resolve every 🔴 at the phase where it surfaces, following the escalation rules in `/track:plan`'s loop sections. For 🟡 (yellow): `/track:plan` allows 🟡 to advance with a warning, but this adr2pr chain requires 🟡 to be resolved before step 9 (`/track:full-cycle`) begins — any remaining 🟡 at step 10 (`/track:pr-review`) will surface as a merge blocker for the user, so clearing all 🟡 before full-cycle is mandatory. If Phase 1 or Phase 2 returns 🟡 after all reds are cleared, re-invoke the relevant writer to address the yellow before proceeding to the next step.

## Behavior

After execution, summarize:

1. Each step's gate verdict and the commits produced.
2. The PR URL and the final `/track:pr-review` result (confirming **no merge** was performed).
3. Any CI-driven task-bundling decisions made during full-cycle.
4. Confirmation that all 🔴/🟡 signals are resolved.
