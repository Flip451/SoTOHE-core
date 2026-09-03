---
name: track-adr2pr
description: Use when Codex is asked to drive a prepared ADR all the way to a reviewed PR (init → review → commit → plan phases → review → commit → full-cycle → pr-review), autonomously without merging.
---

# Track-Adr2pr (Codex skill)

**Operational SSoT:** read and follow `.harness/workflows/track/adr2pr.md` — the provider-agnostic
workflow contract for this skill. Do not duplicate step sequence, gate conditions, state transitions,
or failure-recovery procedures here.

## Codex-skill notes

### (1) Invocation surface

- Triggered via `$track-adr2pr` in a Codex skill mention surface.
- Can also be force-loaded with `codex exec` by referencing this skill file.
- Before any input acquisition, check whether the current branch is an initialized
  `track/<id>` with `metadata.json`. On that re-invocation path (including any resumed run,
  skill note 5) skip feature / ADR resolution, user confirmation, and
  `$track-init` forwarding entirely; the workflow SSoT's Step 1 derives the first incomplete
  lifecycle boundary from persisted state and resumes there.
- Only when the track needs initialization are the feature name and primary ADR filename
  acquired: an explicitly supplied value always takes precedence, and any missing value is
  resolved and user-confirmed per the workflow SSoT's input-acquisition contract (conversation
  context resolution, one confirmation of the completed pair, candidate selection when
  resolution is not unique) before `$track-init` receives both values explicitly.

### (2) Sandbox constraint

- Requires `--sandbox workspace-write`: the workflow orchestrates commits, PR creation, and
  file writes across multiple sub-workflows.
- Do not run `git push` under any circumstance. PR operations are handled via `bin/sotp pr` wrappers,
  with one exception: the workflow SSoT's all-protected-source terminal audit comment step calls
  `gh pr view --json author` (read-only lookup) directly and posts through the argv-validating
  `cargo make pr-audit-comment -- tmp/pr-audit/<body-file>` wrapper (body file must live under
  `tmp/pr-audit/`; never direct `gh pr comment`). Branch pushes, PR creation, and review-cycle
  triggers remain `bin/sotp pr` wrapper-only.

### (3) Sub-workflow and capability invocation

- Sub-workflows are invoked by their Codex skill name (e.g. `$track-init`, `$track-review`, etc.).
- Phase writers enter only through `bin/sotp phase enter spec-design`,
  `bin/sotp phase enter type-design`, or `bin/sotp phase enter impl-plan` after their configured
  briefings are prepared. Do not launch phase writers from this skill; phase entry owns the
  configured writer launch. For back-and-forth escalation, invoke
  `bin/sotp capability exec adr-editor --host codex --briefing-file <path>` or
  `bin/sotp capability exec adr-diagnoser --host codex --briefing-file <path>`. Invoke the
  matching `.codex/agents/<capability>.toml` in-host only on
  `CAPABILITY_EXEC_OUTCOME: delegate-in-host`.
  `review-fix-lead` keeps its typed-pipeline route (`cargo make track-local-review-fix`), which
  resolves the provider internally.

### (4) Autonomy boundary (Phase 0 user approval)

- The workflow SSoT's autonomy constraint yields to the Phase 0 interaction boundary governed
  by `.harness/policies/pre-track-adr-authoring.md` §In-track 意味変更の裁定権. That
  convention is the sole normative source for Phase 0; this skill states no procedure of its
  own for that phase. Exactly two unconditional approval pauses are sanctioned after Phase 0:
  (1) that Phase 0 boundary; (2) the pause inherited from the delegated `$track-pr-review`
  workflow — recording Accepted Deviations at its terminal state requires that workflow's
  explicit user approval. One conditional interaction is additionally permitted: the
  parent-session refresh request in skill note 5, allowed only on a host without automatic
  context management and never on a host that has it. No other step pauses for user
  confirmation; the invocation-time input acquisition (skill note 1) happens before Step 1
  begins and is outside this pause accounting.

### (5) Parent-session refresh points

- The workflow SSoT fixes the parent-session refresh boundaries (after the plan-artifacts
  commit, after the first implementation batch, at PR-lane start) and what may be discarded
  there; this skill adds no boundary of its own.
- The boundaries are resume points, not stops. A Codex root with automatic context management
  continues past them without asking the user. Only when the host lacks automatic context
  management may the root ask the user in plain prose to start a fresh Codex session that
  re-invokes `$track-adr2pr` on the same `track/<id>` branch; the re-invoked run resumes at the
  step the workflow SSoT derives from the persisted state (commits, plan artifacts, task states)
  rather than replaying earlier steps. Do not add host-specific backgrounding,
  notification-format, or compaction handling here.

### (6) Reporting format

- On successful completion (only when the final `$track-pr-review` step reaches a terminal
  state per `.harness/workflows/track/adr2pr.md` — machine PASS, or Accepted Deviations
  recorded with the user approval that workflow requires), print:
  `ADR2PR_STATUS: completed — PR <url> reviewed, no merge performed`
- After that line, report the all-protected-source terminal audit comment result on one line: the
  posted comment URL, or its per-source empty-diff/provenance-fallback outcome, or the reported
  non-fatal posting failure.
- On failure or block, print: `ADR2PR_STATUS: blocked — <reason>`
