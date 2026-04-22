---
description: Author the track's spec.json via the spec-designer subagent (Phase 1).
---

Canonical command for Phase 1 spec authoring.

Writer: spec-designer subagent. The command body contains only (a) the subagent invocation and (b) the receipt of its signal-evaluation result. All file writes (spec.json), rendered views (spec.md), and spec → ADR signal evaluation run **inside** the spec-designer subagent.

Resolve the track id from the current branch (`track/<id>`).

Pre-check:

- Confirm `track/items/<track-id>/metadata.json` exists. If not, stop and instruct the user to run `/track:init <feature>` first.

Execution:

1. Invoke the spec-designer subagent via the Agent tool (`subagent_type: "spec-designer"`).
   Briefing must include:
   - Track id and `track/items/<track-id>/metadata.json` path
   - Paths to the referenced ADR(s) under `knowledge/adr/`
   - Paths to the related conventions under `knowledge/conventions/`
   - The subagent owns writing `track/items/<track-id>/spec.json`, rendering `spec.md`, and evaluating the spec → ADR signal (🔵🟡🔴).
2. Receive the signal-evaluation result from the subagent and surface it as the `/track:spec-design` output.

Report:

- Track id
- `spec.json` path
- Signal counts (blue / yellow / red)

Behavior:

- No direct CLI invocation from this command body — all CLI calls execute inside the spec-designer subagent.
- Single-shot: invoke once, receive the signal result, return.
