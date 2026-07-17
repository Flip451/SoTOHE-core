# Commit Workflow SSoT

> Provider-agnostic workflow SSoT for the `commit` track workflow. Both the Claude adapter
> (`.claude/commands/track/commit.md`) and the Codex skill adapter
> (`.agents/skills/track-commit/SKILL.md`) reference this file. Provider-specific invocation
> framing lives in those adapters; the full workflow contract lives here.

## Mission

Stage the working tree, create a guarded commit from the current track branch, and attach a
structured git note for traceability. The commit is protected by the `cargo make track-commit-message`
wrapper, which runs full CI (`cargo make ci`), track-aware gates (`cargo make ci-track`),
including the ADR-baseline commit check, review/ref-verify approvals, the test-obligation
fulfillment gate, and the DRY gate before
writing the commit. Commits from non-track branches are rejected (fail-closed). The canonical staging order is:
implement → review → stage → commit. Staging before the final review round silently omits the
`review.json` delta from the commit, producing a stale artifact on disk.

## Inputs

- **Commit message** — supplied by the caller. If absent, the workflow generates 2-3 candidates
  from current track context and presents them for the user to choose; no guarded commit is
  executed in the proposal phase.
- **Current branch** — must match `track/<id>`. Commits from non-track branches are rejected.
- **Staged diff** — must be non-empty and must cover the intended commit scope. If `review.json`
  shows modified-but-unstaged in `git status`, the caller staged before the final review round
  and must re-stage now.
- **`bin/sotp review check-approved` exit 0** — required before the guarded commit wrapper
  proceeds (the wrapper enforces this gate internally via `cargo make track-commit-message`).
- **`bin/sotp test-obligation check` exit 0** — required before the guarded commit wrapper
  proceeds; missing bindings, unresolved edges, or stale verdicts block the commit.
- **`bin/sotp dry check-approved` exit 0** — required before the guarded commit wrapper
  proceeds.
- **`bin/sotp adr-baseline check-commit` exit 0** — required through `cargo make ci-track`;
  every recorded ADR must match its latest baseline and every cited, non-draft ADR must be
  stamped.

## Sequence

**Step 0: Fill missing commit message (when omitted)**

If no commit message is supplied:

1. Resolve the current track from the current git branch (`track/<id>`). If not on a track
   branch, fall back to the latest active track by `updated_at` for proposal-only mode.
2. Read `spec.md`, `plan.md`, `observations.md` (optional), and current changed files for context.
3. Propose 2-3 commit message candidates following the repository's current commit style and
   reflecting the actual change scope.
4. Stop after presenting candidates. Do not execute the guarded commit until the user selects
   a message.

**Step 1: Pre-commit checks**

1. Run `git diff --cached --stat` and verify the staged scope matches the intended commit.
2. If the staged diff is empty or includes unrelated changes, stop and fix staging before
   continuing. If `review.json` appears modified-but-unstaged, re-stage now before proceeding.
3. Confirm the current branch matches `track/<id>`. A guarded commit from a non-track branch
   is rejected fail-closed — switch to the track branch first.
4. `track/registry.md` is a generated view (gitignored, not version-controlled). Do NOT stage
   or commit it. If `plan.md` or `spec.md` views appear stale, run `bin/sotp track views sync`
   to regenerate them before staging.

**Step 2: Guarded commit**

Write the commit message to `tmp/track-commit/commit-message.txt`. Then run:

```
cargo make track-commit-message
```

This wrapper executes `cargo make ci`, `cargo make ci-track` (including
`bin/sotp adr-baseline check-commit`), review/ref-verify approval checks,
`bin/sotp test-obligation check`, and `bin/sotp dry check-approved`, then commits
from `tmp/track-commit/commit-message.txt`. It deletes the scratch file on success.
If the commit fails (CI failure or git error), report the error and stop. Do not proceed to
note generation. A byte mismatch from `adr-baseline check-commit` or its CI path is recovered
through the ADR-baseline route in `.harness/workflows/track/diagnose.md`, not by retrying the
guarded commit unchanged.

**Step 3: Attach git note**

After a successful commit, generate and attach a structured git note to HEAD.

*Step 3a — Generate note (normal path)*:

1. Find the current track under `track/items/`.
2. Read `spec.md` and `plan.md`; also read `observations.md` if it exists.
3. Run `git show HEAD --stat` to get the changed file list.
4. Generate the note text using the format below and write it to `tmp/track-commit/note.md`.
5. Run `cargo make track-note`.

*Step 3b — Skip note*: if no track directory exists and no generated scratch note is available,
skip note generation and mention this in the summary.

**Note format:**

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

## Gates

| Step | Gate | Verdict |
|------|------|---------|
| 1 | Staged diff non-empty and matches intent | OK / fix-staging |
| 1 | Current branch matches `track/<id>` | OK / ERROR |
| 2 | `cargo make track-commit-message` exits 0 (CI + ADR-baseline-aware track gates + commit) | OK / ERROR |

## Failure / recovery

- **No commit message supplied**: generate proposals and stop. Do not execute commit.
- **Empty or wrong staged diff**: fix staging with `cargo make add-all` or selective
  `cargo make track-add-paths`, then re-run the workflow.
- **Non-track branch**: switch to the track branch and re-run.
- **`cargo make track-commit-message` failure**:
  - CI/gate failure (fmt, clippy, test, deny, layers, verify-*, ADR-baseline-aware track gates,
    test-obligation, DRY): fix the failing gate and re-run.
    Do not re-stage — the working tree is the same. Do not proceed to note generation.
  - ADR-baseline byte mismatch from `check-commit` or CI: stop the commit flow and enter the
    ADR-baseline recovery route in `diagnose.md`. The orchestrator dispatches `adr-diagnoser`;
    only `non-semantic-restamp` permits `snapshot --kind non-semantic-fix` and a retry.
    `deviation` restores the latest baseline and injects an amendment-proposal request into the
    originating capability briefing; `unknown-editor` restores and records the history in
    `observations.md`.
  - git commit error: diagnose (index state, branch protection) and resolve before retrying.
- **Note generation failure** (`cargo make track-note` non-zero): report the error. The commit
  itself already succeeded; note failure is non-fatal but should be investigated.

## Outputs

- Commit on the current `track/<id>` branch (commit hash reported)
- Commit message used (echoed in summary)
- Git note attached to HEAD (or skipped with reason)
- `track/registry.md` status: regenerated locally via `bin/sotp track views sync` if stale,
  or already current (gitignored; not committed)
- Recommended next action reported to the caller
