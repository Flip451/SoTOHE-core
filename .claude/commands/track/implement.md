---
description: Run parallel interactive implementation for the current track.
---

Canonical command for interactive parallel implementation.

Arguments:
- Use `$ARGUMENTS` as optional scope notes (target module, constraints, priority).

Execution:
- Read the latest active track's `spec.md`, `plan.md`, and `metadata.json` before implementation.
- Read every convention file listed in the `## Related Conventions (Required Reading)` section of `plan.md` before writing code.
- For exact type signatures, trait definitions, module trees, and Mermaid diagrams, prefer `## Canonical Blocks` in `plan.md` and `.claude/docs/DESIGN.md` over surrounding prose.
- Identify the target task(s) from the approved plan. If `$ARGUMENTS` is provided, map it to the relevant plan scope.
- Use `cargo make track-transition <track_dir> <task_id> in_progress` to mark selected tasks as `in_progress` in `metadata.json` and auto-render `plan.md` + `registry.md`. Do NOT edit `plan.md` directly — it is a read-only view rendered from metadata.json (SSoT).
- Before using `cargo make *-exec` commands or Agent Teams fast loops, confirm `cargo make tools-up` has already started `tools-daemon`. If not, either start it first or fall back to `run --rm` tasks.
- Run Agent Teams based parallel implementation for the current approved plan.
- Use any auto-injected external guide summaries from `docs/external-guides.json` before opening cached raw guide documents.
- If `$ARGUMENTS` is provided, treat it as implementation scope.
- Do not modify dependencies or rewrite `Cargo.lock` from multiple workers at once. Serialize `cargo add`, `cargo update`, and any `Cargo.lock`-changing step through a single worker, then resume parallel work.
- Parallel workers should prefer `cargo make test-one-exec {test_name}` for single-test validation. Reserve full-suite commands (`test-exec`, `check-exec`) for integration phases or a single worker to avoid `target/` build lock contention.
- Before reporting completion, require `cargo make ci` equivalent validation.
- After CI passes, update `verification.md` in the current track directory:
  - Record which manual verification steps were performed and their results.
  - Note any open issues or areas requiring further review.
  - Set `verified_at` to the current date.
- Use `cargo make track-transition <track_dir> <task_id> done --commit-hash <hash>` to mark completed tasks as `done` (auto-renders `plan.md` + `registry.md`). If work remains blocked, keep tasks in `in_progress` and report why.

Behavior:
- This command is the canonical replacement for legacy team-implement style flow.
- After execution, summarize:
  1. Implemented scope
  2. Updated `metadata.json` task states (todo → in_progress → done, or blocked in_progress)
  3. Remaining tasks
  4. Recommended next command (`/track:review`, `/track:commit <message>`, or `/track:full-cycle <task>`)
