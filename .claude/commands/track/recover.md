---
description: Recover an active track after a conflicted guarded base merge.
---

> Operational SSoT: `.harness/workflows/track/recover.md` — provider-neutral recovery semantics live there. This file is a Claude Code adapter and owns only invocation, tool constraints, and reporting.

## Invocation

User invokes this command as `/track:recover`. `$ARGUMENTS` may provide concise conflict-resolution context.

## Claude Code invocation constraints

- Use the guarded `bin/sotp track merge-base` surface for base merges; never invoke direct VCS commands.
- Use the canonical `/track:review` and `/track:commit` workflows for the review and commit stages.
- Do not invoke direct staging, commit, push, or filesystem cleanup commands.
- Keep the adapter free of recovery sequence, gate, and failure-handling definitions; defer them to the operational SSoT.

## Report format

After execution, summarize:

1. The recovery-context check.
2. Verification and review outcome.
3. Guarded commit outcome.
4. Remaining blocker, if any.
