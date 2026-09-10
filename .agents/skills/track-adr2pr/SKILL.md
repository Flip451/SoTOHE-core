---
name: track-adr2pr
description: Use when Codex is asked to drive a prepared ADR through the canonical track-to-PR workflow without merging.
---

# Track-Adr2pr (Codex skill)

**Operational SSoT:** read and follow `.harness/workflows/track/adr2pr.md` — the provider-agnostic
workflow contract for this skill. Do not duplicate step sequence, gate conditions, state transitions,
or failure-recovery procedures here.

## Codex-skill notes

### (1) Invocation surface

- Triggered via `$track-adr2pr` in a Codex skill mention surface.
- Can also be force-loaded with `codex exec` by referencing this skill file.

### (2) Sandbox constraint

- Requires `--sandbox workspace-write`: the workflow orchestrates commits, PR creation, and
  file writes across multiple sub-workflows.
- Do not run `git push` under any circumstance. PR operations are handled via `bin/sotp pr` wrappers,
  with one exception: the workflow SSoT's all-protected-source terminal audit comment step calls
  `gh pr view --json author` (read-only lookup) directly and posts through the argv-validating
  `cargo make pr-audit-comment -- tmp/pr-audit/<body-file>` wrapper (body file must live under
  `tmp/pr-audit/`; never direct `gh pr comment`). Branch pushes, PR creation, and review-cycle
  triggers remain `bin/sotp pr` wrapper-only.

### (3) Codex invocation mapping

The workflow SSoT owns input acquisition, phase ordering, approval boundaries, resume derivation,
gates, and recovery. Codex invokes its sub-workflows through the corresponding skill adapters:

| Workflow surface | Codex adapter |
|---|---|
| `init` | `$track-init` |
| `review` | `$track-review` |
| `commit` | `$track-commit` |
| `spec-design` | `$track-spec-design` |
| `type-design` | `$track-type-design` |
| `impl-plan` | `$track-impl-plan` |
| `full-cycle` | `$track-full-cycle` |
| `pr-review` | `$track-pr-review` |

For ADR escalation, invoke `bin/sotp capability exec adr-editor --briefing-file <path>` or
`bin/sotp capability exec adr-diagnoser --briefing-file <path>` with `--host` omitted. The
dispatcher resolves the configured provider and runs its subprocess; do not force the Codex
root's provider or hand-assemble a provider command. The typed-pipeline
`cargo make track-local-review-fix` route remains the review-fix-lead mapping.

### (4) Reporting format

- On successful completion (only when the final `$track-pr-review` step reaches a terminal
  state per `.harness/workflows/track/adr2pr.md` — machine PASS, or Accepted Deviations
  recorded with the user approval that workflow requires), print:
  `ADR2PR_STATUS: completed — PR <url> reviewed, no merge performed`
- After that line, report the all-protected-source terminal audit comment result on one line: the
  posted comment URL, or its per-source empty-diff/provenance-fallback outcome, or the reported
  non-fatal posting failure.
- On failure or block, print: `ADR2PR_STATUS: blocked — <reason>`
