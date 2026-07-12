---
description: Diagnose a phase-rollback target for an impl-phase or later structural inconsistency, returning a structured routing decision the orchestrator dispatches.
---

> Operational SSoT: `.harness/workflows/track/diagnose.md` — provider 非依存 workflow logic はそちらを参照。本ファイルは Claude Code 固有 adapter として、起動形態 / Tool 制約 / 報告形式のみを残す。

## Invocation

User (or the orchestrator's main loop) invokes this command as `/track:diagnose`.

- `$ARGUMENTS`: the diagnostic input — a PreReviewGate Blocked summary, a review finding on an
  SoT scope, or a free-form reviewer comment. May be passed inline or via a
  `--briefing-file <path>` reference. If empty, ask the user for the diagnostic input and stop.

## Claude Code invocation constraints

- Provider resolution: read `.harness/config/agent-profiles.json`
  (`capabilities.rollback-diagnoser.provider` / `model`).
  - **provider: claude** — invoke the Claude subagent via the Agent tool with
    `subagent_type: "rollback-diagnoser"`. The subagent reads the operational SSoT
    (`.harness/capabilities/rollback-diagnoser.md`) and executes the routing judgment.
  - **provider: codex** — write the diagnostic briefing first, then run
    `bin/sotp capability exec rollback-diagnoser --host claude --briefing-file <path>`. The
    dispatcher resolves the profile model and validates the Codex skill's declared read-only
    sandbox before invoking it. The codex skill (`.agents/skills/rollback-diagnoser/SKILL.md`)
    reads the same operational SSoT.
- This command performs no writes: no SoT artifact edits, no staging/commits, no writer
  subagent invocation, no mutating `bin/sotp` subcommands (see the workflow SSoT's
  Constraints section).

## Report format

After execution, return the structured routing decision verbatim to the caller:

```
{
  "routing_target": "adr" | "spec" | "type" | "impl_plan" | "impl",
  "reason": "<japanese diagnostic citing element ids>",
  "recommended_next_action": "<japanese concrete next step>"
}
```

The orchestrator then dispatches per the workflow SSoT's "Step 4" table and may override the
suggested target if it judges `reason` insufficiently convincing.

## References

- `.harness/workflows/track/diagnose.md` — provider-agnostic workflow SSoT
- `.harness/capabilities/rollback-diagnoser.md` — capability operational contract
- `.claude/agents/rollback-diagnoser.md` — Claude subagent wrapper
- `.agents/skills/rollback-diagnoser/SKILL.md` — Codex skill wrapper
- `.harness/config/agent-profiles.json` — `capabilities.rollback-diagnoser` provider routing
