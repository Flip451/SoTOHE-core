---
description: Drive a prepared ADR all the way to a reviewed PR — init → review → commit → plan → review → commit → full-cycle → pr-review, autonomously (no merge).
---

> Operational SSoT: `.harness/workflows/track/adr2pr.md` — provider 非依存 workflow logic はそちらを参照。本ファイルは Claude Code 固有 adapter として、起動形態 / Tool 制約 / 報告形式のみを残す。

## Invocation

User invokes this command as
`/track:adr2pr [<feature>] [--primary-adr <primary-adr-file>.md]`. Both arguments are
optional. Before any input acquisition, check whether the current branch is
an initialized `track/<id>` with `metadata.json`. On that re-invocation path, skip feature/ADR
resolution, user confirmation, and `/track:init` forwarding entirely; let Step 1 derive the first
incomplete lifecycle boundary from persisted state and resume there. Only when the track needs
initialization does an explicitly supplied value take precedence, with any missing value resolved
and user-confirmed per the workflow SSoT's input-acquisition contract (conversation context
resolution, one confirmation of the completed pair, candidate selection when resolution is not
unique). Use AskUserQuestion for that confirmation / selection. When Step 1 determines that the
track needs initialization, pass the resolved values unchanged to `/track:init <feature>
--primary-adr <file>`; the filename is recorded as that track's init baseline.

## Claude Code invocation constraints

- **Progress tracking**: when `TaskCreate` is available, use it to register the workflow steps
  as tasks, marking each `in_progress` before starting and `completed` after its gate passes.
  When it is unavailable, report those transitions in text and continue the workflow.
- **Sub-command execution**: drive each sub-step by reading its `.claude/commands/track/<name>.md` definition and executing it. Do not re-state sub-command logic here.
- **Phase 0 input forwarding**: when Step 1 invokes
  `/track:init <feature> --primary-adr <file>`, pass both resolved, user-confirmed inputs
  explicitly. Do not re-select or derive either value in `init`.
- **Phase writer entry** — after preparing its configured briefing, enter Phase 1–3 through
  `bin/sotp phase enter spec-design`, `bin/sotp phase enter type-design`, or
  `bin/sotp phase enter impl-plan`. Do not launch a phase writer from this adapter; phase
  entry owns the configured writer launch. Back-and-forth `adr-editor` / `adr-diagnoser`
  dispatch remains capability-specific: invoke
  `bin/sotp capability exec adr-editor --host claude --briefing-file <path>` or
  `bin/sotp capability exec adr-diagnoser --host claude --briefing-file <path>`. The
  dispatcher resolves each provider from `.harness/config/agent-profiles.json`; only on
  `CAPABILITY_EXEC_OUTCOME: delegate-in-host` invoke the matching Claude Agent tool with the
  briefing path and discipline body. Pass `--resume` only when continuing the same assignment
  (`.claude/rules/orchestration.md`).
- **Interaction boundaries**: honor the workflow SSoT's user-interaction and terminal-state
  rules; this adapter does not restate them.
- **Parent-session refresh points**: use the workflow SSoT's three fixed boundaries as resume
  points. Claude Code manages context automatically (compaction), so do not stop at a boundary
  and do not ask the user to run `/clear` or start a fresh session — continue the workflow. A
  re-invocation of `/track:adr2pr` on the same `track/<id>` branch (for any reason) still lets
  Step 1 inspect persisted state, skip `init`, and resume at the first incomplete boundary. Do
  not add host-specific backgrounding, notification-format, or compaction handling here.
- **Phase 0 governing convention**: apply
  `.harness/policies/pre-track-adr-authoring.md#In-track 意味変更の裁定権` as the sole
  normative source for Phase 0. This adapter states no procedure of its own for that phase.
- **Staging**: `bin/sotp git add-all`
- **Commit**: write to `tmp/track-commit/commit-message.txt`, then `cargo make track-commit-message`
- **Terminal audit comment**: use only the workflow SSoT's approved wrapper; do not invoke
  `gh pr comment` directly or route the audit through `bin/sotp pr review-cycle`.

## Report format

After execution, summarize:

1. Each step's gate verdict and the commits produced.
2. PR URL and the final `/track:pr-review` result (confirming no merge was performed).
3. Any per-scope ceiling batch split decisions made during full-cycle.
4. Confirmation that all 🔴 and actionable 🟡 signals are resolved, plus any admitted delta
   drafts intentionally left 🟡 for merge-stage adjudication.
5. The all-protected-source terminal-audit comment result: posted URL, per-source
   empty-diff/provenance fallback outcomes, or the reported non-fatal posting failure.
