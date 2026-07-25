---
description: Set up the development environment and catch up on project state.
---

> Operational SSoT: `.harness/workflows/track/catchup.md` — provider-independent workflow
> behavior lives there. This file is the Claude Code adapter and defines only invocation,
> Claude-specific tool constraints, and report presentation.

## Invocation

User invokes this command as `/track:catchup`. No arguments are required.

## Claude Code invocation constraints

- Execute the sequence in the workflow SSoT with Claude Code tools.
- When the workflow directs track-workflow setup, invoke `/track:setup`.
- Do not start implementation work, stage files, create commits, or push branches.

## Report format

1. Environment command selected and its pass/fail status.
2. Track workflow setup status: initialized / already set up.
3. Project briefing: active tracks, current tech-stack decisions, active conventions, and recent commit history.
4. Suggested next actions for the contributor.
