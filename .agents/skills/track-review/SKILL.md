---
name: track-review
description: Use when Codex is asked to run the review cycle for the current track — driving each required scope through fast and final rounds until every scope reaches zero_findings.
---

# Track-Review (Codex skill)

**Operational SSoT:** read and follow `.harness/workflows/track/review.md` — the provider-agnostic
workflow contract for this skill. Do not duplicate step sequence, gate conditions, state transitions,
or failure-recovery procedures here.

## Codex-skill notes

### (1) Invocation surface

- Triggered via `$track-review` in a Codex skill mention surface.
- Can also be force-loaded with `codex exec` by referencing this skill file.

### (2) Sandbox constraint

- Requires `--sandbox workspace-write`: the review-fix-lead capability writes source fixes
  to the working tree during the fix phase.
- The reviewer subprocess itself (`bin/sotp review local`) runs read-only internally;
  only the fix phase writes files.

### (3) Sub-workflow and capability invocation

- The review-fix loop per scope is delegated to the `review-fix-lead` capability via
  `$review-fix-lead` (`.codex/agents/review-fix-lead.toml`).
- Scope discovery and briefing preparation are handled by the workflow orchestrator (this skill).

### (4) Reporting format

- On successful completion, print: `REVIEW_STATUS: completed — all scopes zero_findings`
- On failure or block, print: `REVIEW_STATUS: blocked — <scope>: <reason>`
