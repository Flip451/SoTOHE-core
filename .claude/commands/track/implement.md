---
description: Run parallel interactive implementation for the current track.
---

Canonical command for interactive parallel implementation.

Arguments:
- Use `$ARGUMENTS` as optional scope notes (target module, constraints, priority).

Execution:
- Resolve the current track in this order:
  1. If the current git branch matches `track/<id>`, use that track.
  2. Otherwise, use the latest materialized active track (non-archived, non-done, `branch != null`).
  3. If no materialized active track exists, fall back to the latest branchless planning-only track (`status=planned`, `branch=null`).
- Read the current track's `spec.md`, `plan.md`, and `metadata.json` before implementation.
- If the resolved track is branchless planning-only (`status=planned`, `branch=null`), stop immediately and instruct the user to run `/track:activate <track-id>`. Do not transition tasks, do not write implementation code, and do not treat this as an implicit branch-creation step.
- Read every convention file listed in the `## Related Conventions (Required Reading)` section of `spec.md` (or `plan.md` for legacy tracks without `spec.json`) before writing code.
- For exact type signatures, trait definitions, module trees, and Mermaid diagrams, prefer `## Canonical Blocks` in `plan.md` and `knowledge/DESIGN.md` over surrounding prose.
- **ADR pre-check**: If `spec.md` or `plan.md` references an ADR (`knowledge/adr/*.md`), read the ADR and verify that the target task's description is consistent with the ADR's design (layer placement, error types, behavioral contracts). If discrepancies are found, fix the plan (`metadata.json` + `track-sync-views`) before writing code. ADR is the SSoT for design decisions — do not override ADR layer placement or omit ADR-specified types.
- Identify the target task(s) from the approved plan. If `$ARGUMENTS` is provided, map it to the relevant plan scope.
- Use `cargo make track-transition <track_dir> <task_id> in_progress` to mark selected tasks as `in_progress` in `metadata.json` and auto-render `plan.md` + `registry.md`. Do NOT edit `plan.md` directly — it is a read-only view rendered from metadata.json (SSoT).
- Before using `cargo make *-exec` commands or Agent Teams fast loops, confirm `cargo make tools-up` has already started `tools-daemon`. If not, either start it first or fall back to `run --rm` tasks.
- Run Agent Teams based parallel implementation for the current approved plan.
- If `$ARGUMENTS` is provided, treat it as implementation scope.
- Do not modify dependencies or rewrite `Cargo.lock` from multiple workers at once. Serialize `cargo add`, `cargo update`, and any `Cargo.lock`-changing step through a single worker, then resume parallel work.
- Parallel workers should prefer `cargo make test-one-exec {test_name}` for single-test validation. Reserve full-suite commands (`test-exec`, `check-exec`) for integration phases or a single worker to avoid `target/` build lock contention.
- Before reporting completion, require `cargo make ci` equivalent validation.
- After CI passes, create or append to `track/items/<id>/observations.md` **only** when one of the following holds:
  - (a) the task produced machine-non-verifiable observations (e.g., wall-time measurements, UX confirmation, dogfooding results) that the implementer judges worth recording, or
  - (b) `spec.json`'s `acceptance_criteria` explicitly mandates recording to `observations.md`.
  The file is free-form markdown with no scaffold / required fields / required sections — record the observation target, procedure, value, and date at the author's discretion. Otherwise, skip this step (file absence = no observations).
- Use `cargo make track-transition <track_dir> <task_id> done` to mark completed tasks as `done` (auto-renders `plan.md` + `registry.md`). After `/track:commit` creates the actual commit, run `cargo make track-transition <track_dir> <task_id> done --commit-hash <hash>` to record the commit hash. If work remains blocked, keep tasks in `in_progress` and report why.

Behavior:
- This command is the canonical replacement for legacy team-implement style flow.
- Planning-only tracks must pass through `/track:activate <track-id>` first.
- After execution, summarize:
  1. Implemented scope
  2. Updated `metadata.json` task states (todo → in_progress → done, or blocked in_progress)
  3. Remaining tasks
  4. Recommended next command (`/track:review`, `/track:commit <message>`, or `/track:full-cycle <task>`)
