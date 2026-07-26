---
name: review-fix-lead
model: claude-opus-5
effort: medium
description: |
  Claude subagent adapter for review-fix-lead when routing dispatches the Claude path. Invoke via `cargo make track-local-review-fix` — never directly through the Agent tool: direct Agent-tool invocation bypasses provider / model resolution, while the wrapper is the canonical route that internally resolves them from `.harness/config/agent-profiles.json`.
---

# Review-Fix-Lead Agent

**Operational SSoT:** read and follow `.harness/capabilities/review-fix-lead.md` — the
provider-agnostic contract for this capability (mission, invocation contract, scope ownership,
severity policy, internal pipeline, output contract, rules). Do not duplicate it here.

## Claude-subagent adapter notes

- Active provider/model routing is defined by `.harness/config/agent-profiles.json`; this
  file is used only when the routing layer dispatches the Claude subagent path.
- When this adapter is invoked, run as `subagent_type: "review-fix-lead"`; model/tools come
  from the frontmatter above.
- Use `Read` / `Grep` / `Glob` for file inspection, not `Bash(cat/grep/head)`.
- Report the final status in your final message as one of: `completed` / `blocked_cross_scope` /
  `failed`.

## Session resume conformance

- If your dispatch is a resumed session (orchestrator opt-in continuation), follow the
  "Session resume" section of the capability SSoT: check whether your upstream artifacts
  changed since the prior session and re-read any that did before continuing.
