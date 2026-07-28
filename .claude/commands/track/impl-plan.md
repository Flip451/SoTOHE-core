---
description: Author the track's impl-plan.json + task-coverage.json + task-contract.json via the impl-planner subagent (Phase 3).
---

> Operational SSoT: `.harness/workflows/track/impl-plan.md` — provider 非依存 workflow logic はそちらを参照。本ファイルは Claude Code 固有 adapter として、起動形態 / Tool 制約 / 報告形式のみを残す。

## Invocation

User invokes this command as `/track:impl-plan`. No arguments.

## Claude Code invocation constraints

Write a briefing containing the track id, paths to `spec.json` and each `<layer>-types.json`,
and paths to the related ADR(s). Do not put convention paths in the briefing: the capability
dispatcher resolves the `impl-planner` convention set and delivers it with the dispatch, and
that resolution is the complete convention input (workflow SSoT § Inputs). Then run
`bin/sotp capability exec impl-planner --host claude --briefing-file tmp/impl-planner-briefing.md`.
The dispatcher resolves `capabilities.impl-planner` internally from
`.harness/config/agent-profiles.json` and either completes the dispatch or returns the
in-host delegation instruction to follow.

The subagent owns: writing `impl-plan.json`, `task-coverage.json`, and `task-contract.json`, and evaluating the task-coverage binary gate (OK / ERROR).

## Report format

Report: track id, `impl-plan.json`, `task-coverage.json`, and `task-contract.json` paths, task count, gate verdict (OK / ERROR).
