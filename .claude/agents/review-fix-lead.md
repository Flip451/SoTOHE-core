---
name: review-fix-lead
model: opus
description: Claude subagent adapter for review-fix-lead when routing dispatches the Claude path.
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
