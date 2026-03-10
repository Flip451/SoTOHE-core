# Takt / Track Traceability Rules

This document defines the mapping rules between takt execution logs and `track/items/<id>/plan.md` updates.

## 1. Responsibility Split (Fixed)

- `track`:
  - Single source of truth for specs (`spec.md`) and progress state
  - **`metadata.json` is the SSoT (Single Source of Truth)**: task state and track status are managed in `metadata.json`
  - `plan.md` is a **read-only view** generated from `metadata.json` via `render_plan()` (direct editing forbidden)
- `takt`:
  - Execution guardrails (progression control via piece/movement)
- `Makefile/CI`:
  - Final enforcement of quality gates

## 2. Mapping Rules

`metadata.json` is the SSoT for task state. State transitions go through `scripts/track_state_machine.py` API (`transition_task()`, `add_task()`, `set_track_override()`).

1. When implementation starts in takt, update state with `transition_task(track_dir, task_id, "in_progress")`
2. Completed tasks: `transition_task(track_dir, task_id, "done", commit_hash=hash)`
3. Unnecessary tasks: `transition_task(track_dir, task_id, "skipped")` (`[-]` in `plan.md`)
4. Regenerate `plan.md` via `render_plan()` (never edit directly)
5. `metadata.json` is always the SSoT for plan data:
   - **Initial creation** (`/track:plan` after approval): create `metadata.json` first, then generate `plan.md` via `render_plan()`
   - **Subsequent updates**: update `metadata.json` via state machine API, regenerate `plan.md` via `render_plan()` (direct editing forbidden)
   - If spec changes are needed during takt execution, update `spec.md` and `metadata.json` in the same turn and regenerate `plan.md`
6. Before commit, `verify-plan-progress` validates that `plan.md` and `metadata.json` are in sync
7. Track status is auto-derived from task states (`effective_track_status()`). Both `done` and `skipped` are treated as "resolved"; when all tasks are resolved, the track becomes `done`
8. Use `set_track_override()` for blocking/cancelling
9. Before commit, verify that the latest track's `spec.md` / `plan.md` / `verification.md` (selected by `metadata.json.updated_at`) are filled without placeholders
10. User-facing commits use `/track:commit <message>` (direct `git commit` is forbidden). `cargo make commit` is the low-level terminal alternative only

This document covers state transitions and verification rules only.
For day-to-day workflow and quality gate overview, see `track/workflow.md`.

## 3. Enforcement Points

- Gates executed by `cargo make ci` (via `ci-local`):
  - `fmt-check-local` — Rust formatter diff detection
  - `clippy-local` — warnings-deny static analysis
  - `test-local` — nextest test execution
  - `test-doc-local` — doctest execution
  - `deny-local` — forbidden dependency / license verification
  - `scripts-selftest-local` — verify script regression tests
  - `hooks-selftest-local` — Claude hook regression tests
  - `check-layers-local` — layer dependency rule verification (including transitive)
  - `verify-arch-docs-local` — architecture document sync verification
  - `verify-plan-progress-local` — plan.md and metadata.json sync verification
  - `verify-track-metadata-local` — metadata.json schema, task graph, status consistency
  - `verify-tech-stack-local` — tech-stack.md blocking `TODO:` verification
  - `verify-track-registry-local` — registry.md and metadata.json sync verification
  - `verify-orchestra-local` — hooks, permissions, agent definition verification
  - `verify-latest-track-local` — latest track spec.md / plan.md / verification.md completeness
- `/track:commit <message>` = recommended path; applies pending-note if present
- `cargo make commit` = `cargo make ci` + `git commit` low-level alternative (no extra gates, no auto note)
- Exact automation wrappers:
  - `cargo make add-all` = stage entire worktree (excludes `.takt/pending-*` transient files)
  - `cargo make commit-pending-message` = use `.takt/pending-commit-message.txt` as commit message, delete on success
  - `cargo make note-pending` = apply `.takt/pending-note.md` as git note, delete on success
  - `cargo make track-commit-message` = use `tmp/track-commit/commit-message.txt` as commit message, delete on success
  - `cargo make track-note` = apply `tmp/track-commit/note.md` as git note, delete on success
- `cargo make machete` = auxiliary audit for dependency cleanup; not included in standard `ci`

## 4. Interactive Implementation Contract

`/track:implement` maintains the same state transitions as `/track:full-cycle`.

1. On implementation start: `transition_task(track_dir, task_id, "in_progress")`
2. On implementation complete: `transition_task(track_dir, task_id, "done", commit_hash=hash)`
3. On block: keep `in_progress`, document the reason
4. Completion report must include updated `metadata.json` task entries
5. Regenerate `plan.md` via `render_plan()`
6. Pass `cargo make ci` equivalent quality gates before marking as done

## 5. `track/registry.md` Update Rules

| Trigger | Required updates |
| ------- | ---------------- |
| `/track:plan <feature>` approved | Add or refresh the active track row, set `Current Focus`, set `Next recommended command` to `/track:full-cycle <task>` or `/track:implement` (automated renderer default for `planned` status), update `Last updated` |
| `/track:commit <message>` | Refresh the registry status/result for the current track, move finished work to `Completed Tracks` when appropriate, update `Last updated` |
| `/track:archive <id>` | Set track status to `archived`, move from Completed to `Archived Tracks`, update `Last updated`. Only resolved tracks (all tasks `done` or `skipped`) can be archived |

## 6. Git Notes (Implementation Traceability)

All takt pieces (`full-cycle`, `spec-to-impl`, `impl-review`, `tdd-cycle`) generate `.takt/pending-note.md` via the `note-writer` persona in the final step.

In the `/track:commit` flow, if pending-note.md exists, its content is applied as a git note.

When running `cargo make commit` directly from the terminal, follow up with `cargo make note ...` as needed:

```bash
cargo make note "$(cat .takt/pending-note.md)"   # from takt output (preferred)
cargo make note "note text here"                 # inline text
cargo make note-pending                          # apply .takt/pending-note.md and delete it
```

`cargo make note` uses `git notes add -f -m "$CARGO_MAKE_TASK_ARGS" HEAD` to pass text directly.
Both inline text and `$(cat .takt/pending-note.md)` use the same path.
Automation flows may create `.takt/pending-commit-message.txt` and use `cargo make commit-pending-message`.
The `/track:commit` non-takt path uses `tmp/track-commit/commit-message.txt` / `tmp/track-commit/note.md` as scratch files.

### Sharing Git Notes Across Machines

Git notes are **not** fetched or pushed by default. To share notes across clones or team members:

```bash
# Fetch notes from remote (add to .git/config or run once per clone)
git config --add remote.origin.fetch "+refs/notes/*:refs/notes/*"

# Push notes to remote
git push origin "refs/notes/*"
```

After configuring the fetch refspec, `git fetch` will automatically pull notes.
Notes are supplemental traceability data — losing them does not break the template workflow.

Note format follows the "Git Notes" section in `track/workflow.md`.

## 7. Reference Commands

```bash
# Direct piece execution
cargo make takt-full-cycle "task summary"
cargo make takt-spec-to-impl "task summary"
cargo make takt-impl-review "review scope"
cargo make takt-tdd-cycle "target scope"

# Queue-based execution
cargo make takt-add "task summary"
cargo make takt-run

# Traceability / consistency checks
cargo make ci
cargo make verify-plan-progress
cargo make verify-latest-track

# Git notes
cargo make note "$(cat .takt/pending-note.md)"  # from takt output (preferred)
cargo make note "note text here"                # inline text
cargo make note-pending                         # apply .takt/pending-note.md and delete it
cargo make track-note                           # apply tmp/track-commit/note.md and delete it
git notes show HEAD
git notes list
```

`cargo make takt-add` saves an active profile snapshot in the task entry.
`cargo make takt-run` reproduces host/provider from that snapshot and aborts if pending tasks have mixed snapshots (to avoid provider drift).
