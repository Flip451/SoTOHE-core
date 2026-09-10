---
name: track-review
description: Use when Codex is asked to run the canonical review workflow for the current track.
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

- The review-fix loop per scope is dispatched through the typed-pipeline wrapper
  `cargo make track-local-review-fix -- --scope <scope> --briefing-file <path> --round-type <fast|final>`,
  which resolves `capabilities.review-fix-lead` internally from
  `.harness/config/agent-profiles.json`; never invoke `$review-fix-lead`
  (`.codex/agents/review-fix-lead.toml`) directly on an assumed provider.

The workflow SSoT owns scope discovery, context intake, review rounds, gates, and recovery.

### (4) Reporting format

- On successful completion, print: `REVIEW_STATUS: completed — all scopes zero_findings`
- On failure or block, print: `REVIEW_STATUS: blocked — <scope>: <reason>`
