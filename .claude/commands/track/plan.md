---
description: Plan a feature via the canonical track planning workflow (Phase 0-3 orchestrator).
---

> Operational SSoT: `.harness/workflows/track/plan.md` — provider 非依存 workflow logic はそちらを参照。本ファイルは Claude Code 固有 adapter として、起動形態 / Tool 制約 / 報告形式のみを残す。

## Invocation

User invokes this command as `/track:plan`. `$ARGUMENTS`:

- `<feature>`: feature name / slug for a new track
- `<integer>`: `max_retry` count (default 5; bare integer = `max_retry`)
- `<feature> <integer>`: both (space-separated)
- Empty: ask the user for a feature name and stop

## Claude Code invocation constraints

- **Context intake**: follow the plan workflow SSoT's `Summary-first context intake`. Use its
  CLI summaries as the primary context; do not bulk-read track artifacts, type catalogues,
  review or binding JSON, or full sub-workflow texts. Open artifact bodies only for a targeted
  diff or blocker investigation. Do not enumerate or read a `Related Conventions` list; the
  dispatcher supplies resolved convention paths to delegated capabilities.
- **Progress tracking**: when `TaskCreate` is available, use it to register Phase 0–3 steps
  and Termination as tasks. When it is unavailable, report the same phase boundaries and
  termination progress in text and continue the workflow.
- **Timestamps**: `date -u +"%Y-%m-%dT%H:%M:%SZ"` (ISO 8601 UTC) — manual input is forbidden.
- **Phase 0**: invoke `/track:init`, `/track:review`, then `/track:commit`; their transition
  rules and inputs are owned by the plan workflow SSoT.
- **Phase 0 governing convention**: apply
  `.harness/policies/pre-track-adr-authoring.md#In-track 意味変更の裁定権` as the sole
  normative source for Phase 0. This adapter states no procedure of its own for that phase.
- **Phase writer entry** — write the configured briefing, then enter the matching phase:
  `bin/sotp phase enter spec-design`, `bin/sotp phase enter type-design`, or
  `bin/sotp phase enter impl-plan`. Phase entry runs the declared convergence checks and
  launches the configured writer only after they pass. Parse its terminal status / gate verdict
  before advancing; on `blocked` / `failed` or a red signal, run the corresponding
  back-and-forth loop. Do not launch a phase writer from this adapter. Back-and-forth
  `adr-editor` / `adr-diagnoser` dispatch remains capability-specific: invoke
  `bin/sotp capability exec adr-editor --host claude --briefing-file <path>` or
  `bin/sotp capability exec adr-diagnoser --host claude --briefing-file <path>`. The
  dispatcher resolves each provider from `.harness/config/agent-profiles.json`; only on
  `CAPABILITY_EXEC_OUTCOME: delegate-in-host` invoke the matching Claude Agent tool with the
  briefing path and discipline body. Pass `--resume` only when continuing the same assignment.

- **Semantic review check**: `bin/sotp ref-verify run`

## Report format

On completion, present:

1. Per-phase gate results (🔵🟡🔴 / OK / ERROR) and final `max_retry` counters per loop.
2. Generated track artifact paths (`metadata.json` / `spec.json` / `<layer>-types.json` / `impl-plan.json` / `task-coverage.json` / `task-contract.json` / `batch-plan.json`).
3. Back-and-forth edits that occurred (target artifact and its writer).
4. Input-box divergence triage results (if any) and the admitted delta drafts left 🟡 for
   the merge-stage adjudication — no synchronous termination decision is requested.
5. Suggested next commands: standard lane (`/track:implement` → `/track:review` → `/track:commit`, or `/track:full-cycle`) or planning-review-first (`/track:review` → `/track:commit`).
