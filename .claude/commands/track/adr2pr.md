---
description: Drive a prepared ADR all the way to a reviewed PR — init → review → commit → plan → review → commit → full-cycle → pr-review, autonomously (no merge).
---

> Operational SSoT: `.harness/workflows/track/adr2pr.md` — provider 非依存 workflow logic はそちらを参照。本ファイルは Claude Code 固有 adapter として、起動形態 / Tool 制約 / 報告形式のみを残す。

## Invocation

User invokes this command as
`/track:adr2pr <feature> --primary-adr <primary-adr-file>.md`. The feature name and selected
pre-track ADR's direct filename under `knowledge/adr/` are both required when the track needs
initialization. Pass them unchanged to `/track:init <feature> --primary-adr <file>`; the filename
is recorded as that track's init baseline.

## Claude Code invocation constraints

- **Progress tracking**: use `TaskCreate` to register the workflow steps as tasks; mark each `in_progress` before starting and `completed` after its gate passes.
- **Sub-command execution**: drive each sub-step by reading its `.claude/commands/track/<name>.md` definition and executing it. Do not re-state sub-command logic here.
- **Phase 0 input forwarding**: when Step 1 invokes
  `/track:init <feature> --primary-adr <file>`, pass both command inputs explicitly. Do not
  re-select or derive either value in `init`.
- **Phase writer capabilities** (`spec-designer` / `type-designer` / `impl-planner` /
  `adr-editor` for back-and-forth escalation) — dispatch every invocation through
  `bin/sotp capability exec <capability> --host claude --briefing-file <path>`. The dispatcher
  resolves `capabilities.<name>.provider` internally from `.harness/config/agent-profiles.json`
  and either completes the provider dispatch or returns
  `CAPABILITY_EXEC_OUTCOME: delegate-in-host`. Only on that outcome invoke the Agent tool
  (`subagent_type: "<capability>"`, `run_in_background: true`) with the briefing path and
  discipline body as the task prompt. Never invoke a capability's Agent-tool subagent without
  that delegation outcome; this adapter must not resolve or assume the provider itself. Pass
  `--resume` only when continuing the same assignment (`.claude/rules/08-orchestration.md`).
- **Autonomy boundary (Phase 0 user approval)**: the workflow SSoT's fully-autonomous
  constraint carries one mandated exception per
  `knowledge/conventions/pre-track-adr-authoring.md` §In-track 意味変更の裁定権 — when the
  Phase 0 ADR-baseline review reaches `zero_findings`, stop and escalate to the user with the
  init-stamp diff and any guardian-withheld proposals; only after user approval may the
  post-approval stamp and ADR-baseline commit proceed. No other step pauses for confirmation.
- **Staging**: `bin/sotp git add-all`
- **Commit**: write to `tmp/track-commit/commit-message.txt`, then `cargo make track-commit-message`

## Report format

After execution, summarize:

1. Each step's gate verdict and the commits produced.
2. PR URL and the final `/track:pr-review` result (confirming no merge was performed).
3. Any per-scope ceiling batch split decisions made during full-cycle.
4. Confirmation that all 🔴/🟡 signals are resolved.
