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
- Feature name and primary ADR filename are optional at invocation: an explicitly supplied
  value always takes precedence, and any missing value is resolved and user-confirmed per the
  workflow SSoT's input-acquisition contract (conversation context resolution, one
  confirmation of the completed pair, candidate selection when resolution is not unique)
  before `$track-init` receives both values explicitly.

### (2) Sandbox constraint

- Requires `--sandbox workspace-write`: the workflow orchestrates commits, PR creation, and
  file writes across multiple sub-workflows.
- Do not run `git push` under any circumstance. PR operations are handled via `bin/sotp pr` wrappers,
  with one exception: the workflow SSoT's terminal primary-ADR diff-comment step calls
  `gh pr view --json author` (read-only lookup) directly and posts through the argv-validating
  `cargo make pr-audit-comment -- tmp/pr-audit/<body-file>` wrapper (body file must live under
  `tmp/pr-audit/`; never direct `gh pr comment`). Branch pushes, PR creation, and review-cycle
  triggers remain `bin/sotp pr` wrapper-only.

### (3) Sub-workflow and capability invocation

- Sub-workflows are invoked by their Codex skill name (e.g. `$track-init`, `$track-review`, etc.).
- Capabilities (spec-designer, type-designer, impl-planner, adr-editor) are dispatched through
  `bin/sotp capability exec <capability> --host codex --briefing-file <path>`; the dispatcher
  resolves `capabilities.<name>.provider` from `.harness/config/agent-profiles.json`. Only when
  it returns `CAPABILITY_EXEC_OUTCOME: delegate-in-host` invoke the matching
  `.codex/agents/<name>.toml` agent in-host. Never invoke a `.toml` agent without that
  delegation outcome; this skill must not resolve or assume the provider itself.
  `review-fix-lead` keeps its typed-pipeline route (`cargo make track-local-review-fix`), which
  resolves the provider internally.

### (4) Autonomy boundary (Phase 0 user approval)

- The workflow SSoT's fully-autonomous constraint carries one mandated exception per
  `knowledge/conventions/pre-track-adr-authoring.md` §In-track 意味変更の裁定権: when the
  Phase 0 ADR-baseline review reaches `zero_findings`, STOP and escalate to the user with the
  init-stamp diff and any guardian-withheld proposals for approval. Only after the user
  approves may the post-approval stamp and the ADR-baseline commit proceed. The only other
  pause is inherited from the delegated `$track-pr-review` workflow: recording Accepted
  Deviations at its terminal state requires that workflow's explicit user approval. No other
  step pauses for user confirmation; the invocation-time input acquisition (skill note 1)
  happens before Step 1 begins and is outside this pause accounting.

### (5) Reporting format

- On successful completion (only when the final `$track-pr-review` step reaches a terminal
  state per `.harness/workflows/track/adr2pr.md` — machine PASS, or Accepted Deviations
  recorded with the user approval that workflow requires), print:
  `ADR2PR_STATUS: completed — PR <url> reviewed, no merge performed`
- After that line, report the terminal primary-ADR diff-comment result on one line: the posted
  comment URL, or its empty-diff/provenance-fallback outcome, or the reported non-fatal
  posting failure.
- On failure or block, print: `ADR2PR_STATUS: blocked — <reason>`
