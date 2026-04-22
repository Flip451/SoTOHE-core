---
description: Author per-layer type catalogues via the type-designer subagent (Phase 2).
---

Canonical command for Phase 2 type catalogue authoring (TDDD workflow).

Writer: type-designer subagent. The command body contains only (a) the subagent invocation and (b) the receipt of its signal-evaluation result. All file writes (each `<layer>-types.json` and baseline), rendered views (type-graph md, contract-map.md, `<layer>-type-signals.md`), baseline capture, and the type → spec signal evaluation run **inside** the type-designer subagent.

Resolve the track id from the current branch (`track/<id>`). Every TDDD-enabled layer in `architecture-rules.json` order is processed; the subagent handles per-layer selection internally.

Pre-check:

- Confirm `track/items/<track-id>/spec.json` exists (Phase 1 output). If not, stop and instruct the user to run `/track:spec-design` first.

Execution:

1. Invoke the type-designer subagent via the Agent tool (`subagent_type: "type-designer"`).
   Briefing must include:
   - Track id and `track/items/<track-id>/spec.json` path
   - `architecture-rules.json` path (source of truth for TDDD-enabled layers)
   - Paths to the related ADR(s) under `knowledge/adr/` and conventions under `knowledge/conventions/`
   - The subagent owns baseline capture, each `<layer>-types.json` write, all rendered views, and the type → spec signal evaluation (blue / yellow / red).
2. Receive the per-layer signal-evaluation result from the subagent and surface it as the `/track:type-design` output.

Report:

- Track id
- Processed layers and their catalogue file paths
- Signal counts per layer (blue / yellow / red)

Behavior:

- No direct CLI invocation from this command body — all CLI calls execute inside the type-designer subagent.
- Single-shot: invoke once, receive the per-layer signal result, return.
