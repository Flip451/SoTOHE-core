---
description: Diagnose a phase-rollback target for an impl-phase or later structural inconsistency, returning a structured routing decision the orchestrator dispatches.
---

> Operational SSoT: `.harness/workflows/track/diagnose.md` — provider 非依存 workflow logic はそちらを参照。本ファイルは Claude Code 固有 adapter として、起動形態 / Tool 制約 / 報告形式のみを残す。

## Invocation

The orchestrator invokes this command as `/track:diagnose --briefing-file <path>`. The briefing
is prepared by the caller before this read-only command starts and must contain either an
ADR-baseline byte-mismatch report (including its source filename, active track id, latest-baseline
diff, and originating capability when known) or a PreReviewGate Blocked summary, a SoT-scope
review finding, or a free-form reviewer comment. If `--briefing-file` is absent or empty, stop
and ask the caller for a complete briefing path; inline diagnostic input is not supported.

## Claude Code invocation constraints

- For an ADR-baseline byte mismatch, dispatch `adr-diagnoser` with the caller-supplied briefing:
  `bin/sotp capability exec adr-diagnoser --host claude --briefing-file <path>`. For every other
  input, dispatch `rollback-diagnoser` with the same form. The dispatcher resolves provider and
  model internally from `.harness/config/agent-profiles.json`, validates the provider-native
  read-only definition, and either completes the dispatch or returns the in-host delegation
  instruction to follow.
- This command and both diagnosers are strictly read-only: do not create the briefing here, edit
  SoT artifacts, stage/commit, invoke writer subagents, or run mutating `bin/sotp` subcommands.
  After a verdict returns, only the calling orchestrator may run ADR-baseline snapshot or restore
  outside this diagnose workflow.

## Report format

After execution, return the matching structured verdict verbatim to the caller. For an
ADR-baseline mismatch:

```
{
  "verdict": "non-semantic-restamp" | "deviation" | "unknown-editor",
  "reason": "<Japanese explanation>",
  "recommended_next_action": "<Japanese orchestrator action>"
}
```

For every other diagnostic input:

```
{
  "routing_target": "adr" | "spec" | "type" | "impl_plan" | "impl",
  "reason": "<japanese diagnostic citing element ids>",
  "recommended_next_action": "<japanese concrete next step>"
}
```

The orchestrator then performs the workflow SSoT's post-verdict action outside this command:
ADR-baseline snapshot/restore for an ADR verdict, or routing dispatch for a rollback verdict.

## References

- `.harness/workflows/track/diagnose.md` — provider-agnostic workflow SSoT
- `.harness/capabilities/adr-diagnoser.md` — ADR-baseline verdict contract
- `.harness/capabilities/rollback-diagnoser.md` — capability operational contract
- `.claude/agents/rollback-diagnoser.md` — Claude subagent wrapper
- `.agents/skills/rollback-diagnoser/SKILL.md` — Codex skill wrapper
- `.harness/config/agent-profiles.json` — `capabilities.rollback-diagnoser` routing SSoT
