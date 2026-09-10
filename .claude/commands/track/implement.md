---
description: Run parallel interactive implementation for the current track.
---

> Operational SSoT: `.harness/workflows/track/implement.md` — provider 非依存 workflow logic はそちらを参照。本ファイルは Claude Code 固有 adapter として、起動形態 / Tool 制約 / 報告形式のみを残す。

## Invocation

User invokes this command as `/track:implement`. Use `$ARGUMENTS` as optional scope notes (target module, constraints, priority).

## Claude Code invocation constraints

- Shared context intake, task ordering and state, phase/rollback routing, obligation/CI gates,
  and recovery behavior are defined by `.harness/workflows/track/implement.md`; this adapter
  does not restate them.
- **Implementation dispatch**: for each selected task, prepare the workflow SSoT's briefing and
  invoke the configured capability through:
  `bin/sotp capability exec implementer --host claude --briefing-file <path>`. The dispatcher
  resolves the provider and model from `.harness/config/agent-profiles.json`. If it returns
  `CAPABILITY_EXEC_OUTCOME: delegate-in-host`, invoke the returned Claude subagent with its
  briefing path and discipline body; otherwise let the dispatcher complete the provider
  subprocess. Do not bypass this resolution with a direct Agent or hand-assembled provider
  command.
- **Parallel implementation**: independent tasks may use Agent Teams only after each task has
  passed the capability-dispatch step above. The workflow SSoT owns dependency ordering and
  serialization boundaries.

## Report format

After execution, summarize:

1. Implemented scope.
2. Implementation handoff: implemented task IDs and their verification results.
3. Remaining tasks.
4. Recommended next command: `/track:full-cycle`.
