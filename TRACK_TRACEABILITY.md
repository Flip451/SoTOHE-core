# Track Workflow Traceability Rules

This document defines the mapping rules between track workflow state transitions and rendered track views.

## 1. Responsibility Split (Fixed)

- `track`:
  - Single source of truth for specs (`spec.md`) and progress state
  - **`metadata.json` is the SSoT (Single Source of Truth)**: task state and track status are managed in `metadata.json`
  - `plan.md` is a **read-only view** generated from `metadata.json` via `render_plan()` (direct editing forbidden)
- `Makefile/CI`:
  - Final enforcement of quality gates

## 2. Mapping Rules

`metadata.json` is the SSoT for task state. State transitions go through `scripts/track_state_machine.py` API (`transition_task()`, `add_task()`, `set_track_override()`).

1. When implementation starts, update state with `transition_task(track_dir, task_id, "in_progress")`
2. Completed tasks: `transition_task(track_dir, task_id, "done", commit_hash=hash)`
3. Unnecessary tasks: `transition_task(track_dir, task_id, "skipped")` (`[-]` in `plan.md`)
4. Regenerate `plan.md` via `render_plan()` (never edit directly)
5. `metadata.json` is always the SSoT for plan data:
   - **Initial creation** (`/track:plan` after approval): create `metadata.json` first, then generate `plan.md` via `render_plan()`
   - **Subsequent updates**: update `metadata.json` via state machine API, regenerate `plan.md` via `render_plan()` (direct editing forbidden)
   - If spec changes are needed during execution, update `spec.md` and `metadata.json` in the same turn and regenerate `plan.md`
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
- `/track:commit <message>` = recommended path; applies `tmp/track-commit/note.md` if present
- `cargo make commit` = `cargo make ci` + `git commit` low-level alternative (no extra gates, no auto note)
- Exact automation wrappers:
  - `cargo make add-all` = stage entire worktree (excludes `tmp/` dir)
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

The normal `/track:commit` flow generates `tmp/track-commit/note.md` from current track context and applies it via `cargo make track-note`.

When running `cargo make commit` directly from the terminal, follow up with `cargo make note ...` as needed:

```bash
cargo make track-note                            # apply tmp/track-commit/note.md and delete it
cargo make note "note text here"                 # inline text
```

`cargo make note` uses `git notes add -f -m "$CARGO_MAKE_TASK_ARGS" HEAD` to pass text directly.
Automation flows use `tmp/track-commit/commit-message.txt` / `tmp/track-commit/note.md` as the primary scratch files.

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
# Traceability / consistency checks
cargo make ci
cargo make verify-plan-progress
cargo make verify-latest-track

# Git notes
cargo make track-note                           # primary scratch note path
cargo make note "note text here"                # inline text
git notes show HEAD
git notes list
```
