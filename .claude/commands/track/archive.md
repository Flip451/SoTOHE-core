---
description: Archive a track through the canonical workflow.
---

> Operational SSoT: `.harness/workflows/track/archive.md` — provider-independent workflow
> logic lives there. This file is the Claude Code adapter and contains only invocation,
> Claude-specific execution constraints, and reporting.

## Invocation

User invokes this command as `/track:archive`. Pass `$ARGUMENTS` as the track ID when supplied;
without it, the CLI resolves the current track from the branch.

## Claude Code invocation constraints

- Follow `.harness/workflows/track/archive.md` for the command sequence and gates.
- Execute its CLI commands directly; do not replace them with metadata edits, manual directory
  moves, or staging operations.

## Report format

After execution, report:

1. Archived track ID and the `sotp track archive` result.
2. `sotp track views sync` result.
3. Suggested next command: `/track:commit <message>`.
