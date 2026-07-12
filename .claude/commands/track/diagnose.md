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

- Write the diagnostic briefing, then run
  `bin/sotp capability exec rollback-diagnoser --host claude --briefing-file <path>`. The
  dispatcher resolves the provider and model internally from
  `.harness/config/agent-profiles.json`, validates the provider-native read-only definition,
  and either completes the dispatch or returns the in-host delegation instruction to follow.
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

The orchestrator then dispatches per the workflow SSoT's "Step 3" table and may override the
suggested target if it judges `reason` insufficiently convincing.

## References

- `.harness/workflows/track/diagnose.md` — provider-agnostic workflow SSoT
- `.harness/capabilities/rollback-diagnoser.md` — capability operational contract
- `.claude/agents/rollback-diagnoser.md` — Claude subagent wrapper
- `.agents/skills/rollback-diagnoser/SKILL.md` — Codex skill wrapper
- `.harness/config/agent-profiles.json` — `capabilities.rollback-diagnoser` routing SSoT
