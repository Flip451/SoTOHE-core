---
description: Stage review is complete and create a guarded commit, then attach a git note.
---

Canonical command for final commit in the track workflow.

Arguments:

- Use `$ARGUMENTS` as the commit message.
- Guarded commit execution requires being on a `track/<id>` branch; commits from non-track branches are rejected (fail-closed).
- If empty, do not commit immediately. Inspect the current track (branch-bound if on a `track/<id>` branch, otherwise latest by timestamp), current diff, and recent task context, then propose 2-3 concrete commit message candidates and ask the user to choose one or edit one.

Execution:

## Step 0: Fill missing commit message when omitted

If `$ARGUMENTS` is empty:

1. Resolve the current track: if the current git branch matches `track/<id>`, use that track. Otherwise (read-only message proposal only), fall back to the latest active track by `updated_at`.
2. Inspect the current track under `track/items/` when present.
3. Read available context from `spec.md` and `plan.md`, current changed files, and `observations.md` if it exists (optional source — absent is normal).
4. Propose 2-3 commit message candidates that follow the repository's current style and reflect the actual change scope.
5. Stop after presenting the candidates. Do not run `cargo make track-commit-message` yet.

## Step 1: Pre-commit review

**Staging order (canonical flow)**: `implement → /track:review → stage → /track:commit`. Review rounds append to `review.json`; staging **before** review silently omits that delta from the commit, producing an unstaged `review.json` left in the worktree after commit. Always run `cargo make add-all` (or selective `track-add-paths`) **after** the final review round, not before.

Before committing:

1. Run `git diff --cached --stat` and verify the staged scope matches the intended commit.
2. If the staged diff is empty or includes unrelated changes, stop and fix staging before continuing. If `review.json` shows as modified-but-unstaged in `git status`, the caller staged before the final review round — re-stage now before proceeding.
3. Resolve the current track from the current `track/<id>` branch. A guarded commit from a non-track branch is rejected (fail-closed) — switch to the track branch first.
4. `track/registry.md` is a generated view (gitignored, not version-controlled). Do NOT attempt to stage or commit it. If the current track's `plan.md` or `spec.md` views appear stale, run `bin/sotp track views sync` to regenerate them. Note: from a `track/<id>` branch, this command refreshes `track/registry.md` and the current track's rendered views. When staging for commit, only include the current track's versioned files and intended review artifacts. This is purely about commit scope; staging does not affect review hash computation (hashes are worktree-based).

## Step 2: Guarded commit

Write the chosen commit message to `tmp/track-commit/commit-message.txt`.
If you are on a non-track branch, stop instead of running the wrapper (guarded commits require a `track/<id>` branch).
Then run:

```bash
cargo make track-commit-message
```

This exact wrapper performs `ci + git commit -F tmp/track-commit/commit-message.txt` and deletes the scratch file on success.
If the commit fails, report the error and stop. Do not proceed to note generation.

## Step 3: Attach git note

After a successful commit, attach a structured git note to HEAD.

### 3a. Use generated scratch note (normal path)

Generate the note from current track context unless the user explicitly provided some other
traceability source:

1. Find the current track under `track/items/` (if any exist).
2. Read `spec.md` and `plan.md` from that track; also read `observations.md` if it exists (optional source).
3. Run `git show HEAD --stat` to get the list of changed files.
4. Generate the note text using the format below and write it to `tmp/track-commit/note.md`.
5. Run: `cargo make track-note`

### 3b. Skip note (no track and no generated scratch note)

If no track directory exists and no generated scratch note is available, skip note generation
and mention this in the summary.

## Note format

```markdown
## Task Summary: <brief description>

**Track:** <track-id or "no-track">
**Task:** <task description from metadata.json done task, or commit subject>
**Date:** <YYYY-MM-DD>

### Changes

- <filename>: <what changed — one line per key file>

### Why

<1–3 sentences from spec.md/plan.md or commit context>
```

## Summary output

After execution, report:

1. Commit result (success/failure) and commit hash
2. Commit message used
3. `track/registry.md` status: regenerated locally via `bin/sotp track views sync` / already current / skipped (gitignored, not committed)
4. Git note status: applied (source: generated tmp scratch file) or skipped (reason)
5. Next recommended action
