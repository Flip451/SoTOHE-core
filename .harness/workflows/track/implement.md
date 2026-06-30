# Implement Workflow SSoT

> Provider-agnostic workflow SSoT for the `implement` track workflow. Both the Claude adapter
> (`.claude/commands/track/implement.md`) and the Codex skill adapter
> (`.agents/skills/track-implement/SKILL.md`) reference this file. Provider-specific
> invocation framing lives in those adapters; the full workflow contract lives here.

## Mission

Run parallel interactive implementation for the current track. The workflow reads the approved
implementation plan, marks selected tasks `in_progress`, implements them using the available
parallelism of the execution environment, runs CI to verify correctness, and marks completed
tasks `done`. Implementation requires being on a `track/<id>` branch. No commit is created
by this workflow — the `commit` workflow (`review` → `commit`) follows.

## Inputs

- **Current branch** — must match `track/<id>`. The track id is resolved from this branch.
  If the branch does not match this pattern, stop immediately and report the situation.
- **Track context** — `spec.md`, `plan.md`, `metadata.json`, and all conventions listed in
  `## Related Conventions (Required Reading)` in `spec.md` (or `plan.md` for legacy tracks).
  For exact type signatures, trait definitions, module trees, and Mermaid diagrams, prefer
  `## Canonical Blocks` in `plan.md` over surrounding prose.
- **ADR pre-check** — if `spec.md` or `plan.md` references an ADR under `knowledge/adr/`,
  read the ADR and verify that the target task's description is consistent with the ADR's
  design (layer placement, error types, behavioral contracts). Fix `metadata.json` (then
  `bin/sotp track views sync`) before writing code if discrepancies are found. The ADR is the
  source of truth for design decisions.
- **Optional scope notes** — caller-supplied hints (target module, constraints, priority) that
  narrow the set of tasks to implement.

## Sequence

**Step 1: Resolve track and validate context**

1. Resolve the current track:
   - If the current git branch matches `track/<id>`, use that track.
   - Otherwise, use the latest materialized active track (non-archived, non-done, `branch != null`).
   - If no materialized active track is found on a `track/<id>` branch, stop immediately and
     report the situation. Do not transition tasks or write implementation code.
2. Read `spec.md`, `plan.md`, and `metadata.json`. Read every convention file listed in
   `## Related Conventions (Required Reading)`.
3. Identify the target task(s) from the approved plan. If scope notes are provided, map them
   to the relevant plan scope.

**Step 2: Mark tasks in_progress**

Use `bin/sotp track transition <task_id> in_progress` to mark selected tasks as `in_progress`
in `metadata.json`. This auto-renders `plan.md` + `registry.md`. The active track is resolved
from the current branch; pass `--track-id <id>` explicitly only when targeting a different track.
Do NOT edit `plan.md` directly — it is a read-only view rendered from `metadata.json`.

**Step 3: Parallel implementation**

Implement the selected tasks in dependency order (lower-layer first, then upper layers that
consume the new lower-layer surface). The order is encoded in the impl-plan sections.

Parallelism rules:

- Tasks touching independent files may be implemented in parallel.
- Serialize `cargo add`, `cargo update`, and any `Cargo.lock`-changing step through a single
  worker to avoid lock contention, then resume parallel work.
- Parallel workers should prefer `cargo make test` for test validation. Reserve full-suite
  commands and full CI for the integration phase or a single worker to avoid build lock contention.
  To isolate a single test: `cargo nextest run <test_name>` inside the tools container.

**Step 4: CI validation**

Before reporting completion, require `cargo make ci` equivalent validation.

**Step 5: Record observations (conditional)**

After CI passes, create or append to `track/items/<id>/observations.md` only when one of the
following holds:

- (a) The task produced machine-non-verifiable observations (wall-time measurements, UX
  confirmation, dogfooding results) worth recording.
- (b) `spec.json`'s `acceptance_criteria` explicitly mandates recording to `observations.md`.

The file is free-form markdown with no required scaffold. Otherwise, skip this step
(file absence = no observations).

**Step 6: Mark tasks done**

Use `bin/sotp track transition <task_id> done` to mark completed tasks as `done` (auto-renders
`plan.md` + `registry.md`). After the subsequent `commit` workflow creates the actual commit,
the commit hash is recorded separately with
`bin/sotp track transition <task_id> done --commit-hash <hash>`. If work remains blocked,
keep tasks in `in_progress` and report why.

## Gates

| Step | Gate | Verdict |
|------|------|---------|
| 1 | Active `track/<id>` branch found | OK / stop |
| 4 | `cargo make ci` exits 0 | pass / fail |

## Failure / recovery

- **No track branch**: stop immediately. Do not transition tasks or write code. Report the
  situation to the caller.
- **CI failure**: fix the failing gate (fmt, clippy, test, deny, layers, verify-*), re-run
  `cargo make ci`, and continue. Do not mark tasks done until CI passes.
- **Blocked task**: keep the task in `in_progress`. Report the blocker and the remaining work.
  The `review` + `commit` cycle may proceed for any tasks that did reach `done`.
- **Cargo.lock contention** (parallel workers): serialize the lockfile-changing step through
  one worker, then resume parallel work.

## Outputs

- Source code changes in the working tree (not committed)
- Updated `metadata.json` task states (todo → in_progress → done, or blocked in_progress)
- Optional `track/items/<id>/observations.md` (appended if conditions are met)
- `plan.md` and `registry.md` regenerated as side effects of task state transitions
- Implemented scope summary and remaining tasks (reported to caller)
- No commit is created by this workflow
