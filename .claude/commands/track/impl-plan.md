---
description: Author the track's impl-plan.json + task-coverage.json + task-contract.json + batch-plan.json via the impl-planner subagent (Phase 3).
---

> Operational SSoT: `.harness/workflows/track/impl-plan.md` — provider 非依存 workflow logic はそちらを参照。本ファイルは Claude Code 固有 adapter として、起動形態 / Tool 制約 / 報告形式のみを残す。

## Invocation

User invokes this command as `/track:impl-plan`. No arguments.

## Claude Code invocation constraints

Write a briefing containing the track id, paths to `spec.json` and each `<layer>-types.json`,
and paths to the related ADR(s). Do not put convention paths in the briefing: the capability
dispatcher resolves the `impl-planner` convention set and delivers it with the dispatch, and
that resolution is the complete convention input (workflow SSoT § Inputs). Then run
`bin/sotp phase enter impl-plan`. Phase entry runs its declared convergence checks and launches
the configured writer only after they pass. Do not launch the writer from this adapter.

The subagent owns: writing `impl-plan.json`, `task-coverage.json`, `task-contract.json`, and `batch-plan.json` (sole writer of all four), and evaluating both binary gates — task-coverage (`bin/sotp verify plan-artifact-refs`) and batch-plan structural (`bin/sotp batch-plan check`); Phase 3 passes only when both are OK.

## Report format

Report: track id, `impl-plan.json`, `task-coverage.json`, `task-contract.json`, and `batch-plan.json` paths, task count, both gate verdicts (OK / ERROR each).
