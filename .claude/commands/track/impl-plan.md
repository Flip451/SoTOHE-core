---
description: Author the track's impl-plan.json + task-coverage.json via the impl-planner subagent (Phase 3).
---

Canonical command for Phase 3 implementation-plan authoring.

Writer: impl-planner subagent. Per CN-10 / CN-13, the command body contains only
(a) the subagent invocation and (b) the receipt of its gate-evaluation result.
All file writes (impl-plan.json, task-coverage.json) and the task-coverage
binary gate run **inside** the impl-planner subagent.

Arguments:

- Use `$ARGUMENTS` as the track id if specified.
- If empty, resolve the track id from the current branch (`track/<id>`).

Pre-check:

- Confirm `track/items/<track-id>/spec.json` exists (Phase 1 output). If not, stop and instruct the user to run `/track:spec-design` first.
- Confirm at least one `track/items/<track-id>/<layer>-types.json` exists for every TDDD-enabled layer (Phase 2 output). If not, stop and instruct the user to run `/track:type-design` first.

Execution:

1. Invoke the impl-planner subagent via the Agent tool (`subagent_type: "impl-planner"`).
   Briefing must include:
   - Track id and paths to `track/items/<track-id>/spec.json` and each `<layer>-types.json`
   - Paths to the related ADR(s) under `knowledge/adr/` and conventions under `knowledge/conventions/`
   - The subagent owns writing `track/items/<track-id>/impl-plan.json` and `track/items/<track-id>/task-coverage.json`, and evaluating the task-coverage binary gate.
2. Receive the gate-evaluation result (OK / ERROR) from the subagent and surface it as the `/track:impl-plan` output.

Report:

- Track id
- `impl-plan.json` and `task-coverage.json` paths
- Task count and gate verdict (OK / ERROR)

Behavior:

- No direct CLI invocation from this command body — all CLI calls execute inside the impl-planner subagent.
- Single-shot: invoke once, receive the gate verdict, return.
