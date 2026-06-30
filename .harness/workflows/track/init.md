# Init Workflow SSoT

> Provider-agnostic workflow SSoT for the `init` track workflow. Both the Claude adapter
> (`.claude/commands/track/init.md`) and the Codex skill adapter
> (`.agents/skills/track-init/SKILL.md`) reference this file. Provider-specific invocation
> framing lives in those adapters; the full workflow contract lives here.

## Mission

Initialize a new track directory and its branch (Phase 0). Creates the minimal identity
artifacts — `track/items/<track-id>/metadata.json` and its rendered views — and materializes
the branch from `main`. Phase 0 is the precondition for every subsequent phase; no planning
or implementation may proceed until this workflow completes with OK.

## Inputs

- **Feature name** — a slug-ready phrase or descriptive string; the caller supplies this as the
  primary argument. If absent, the caller must ask the user for a feature name and stop.
- **Current branch = `main`** — the workflow requires the working tree to be on `main` before
  branch creation. Any other starting branch is a hard prerequisite failure.
- **Git status** — expected clean, or containing only ADR / convention files that belong to
  this new track and will be committed inside it. Any unrelated in-progress changes must be
  resolved by the user before the workflow proceeds.
- **`track/tech-stack.md`** — must be free of blocking `TODO:` markers before implementation
  begins (not enforced at Phase 0 itself, but the convention applies to the track as a whole).

## Sequence

**Step 1: Pre-flight check**

Verify the current git branch is `main`. If not, stop and present the situation to the
user — do not auto-switch. Check `git status --short`:

- ADR / convention / other `knowledge/` baseline files staged for the new track: already
  resolved — they will be committed inside the new track. No user action required.
- Other unrelated in-progress changes: present the list and ask the user whether to commit,
  stash, discard, or split into a separate track. Do not auto-act.

Proceed to Step 2 once the branch is `main` and any unrelated changes are resolved.

**Step 2: Create track branch**

Derive `<track-id>` from the feature name: kebab-case ASCII + date suffix `YYYY-MM-DD` from
`date -u +"%Y-%m-%d"`. Then create and switch to the track branch:

```
cargo make track-branch-create '<track-id>'
```

**Step 3: Create metadata.json**

Write `track/items/<track-id>/metadata.json` with the following fields:

- `schema_version`: 5
- `id`: `<track-id>`
- `title`: human-readable title derived from the feature name
- `branch`: `track/<track-id>`
- `created_at` / `updated_at`: `date -u +"%Y-%m-%dT%H:%M:%SZ"` (no manual input)

**Step 4: Render views**

Regenerate `plan.md` and `track/registry.md` from `metadata.json`:

```
bin/sotp track views sync
```

A warning about `contract-map.md` skipping the new track (because no `domain-types.json`
exists yet) is expected at this phase and is not an error.

**Step 5: Verify identity schema**

```
cargo make verify-track-metadata
```

This gate must pass (exit 0) before the workflow reports success.

**Step 6: (Optional) ADR baseline commit**

When ADR / convention files prepared for this track are present in the working tree without
commit history, the recommended flow is to commit them as the first commit of the new track
immediately after Step 5, via the `review` workflow followed by the `commit` workflow
(see `.harness/workflows/track/review.md` and `.harness/workflows/track/commit.md`). At this
point `metadata.json` (Step 3) and `plan.md` (Step 4) exist; `spec.md` is created later in
Phase 1 and is not required for this first commit.

## Gates

| Step | Gate | Verdict |
|------|------|---------|
| 1 | Current branch is `main`; no unrelated dirty state | ERROR → stop |
| 5 | `cargo make verify-track-metadata` exits 0 | OK / ERROR |

The workflow completes with **OK** when `verify-track-metadata` passes.
It completes with **ERROR** on any hard failure (non-main branch, branch creation failure,
metadata write failure, or gate failure). On ERROR, stop and report to the caller.

## Failure / recovery

- **Non-main branch**: report the current branch and available options (switch to main manually,
  or abort). Do not auto-switch.
- **Unrelated dirty state**: list the modified files, classify them, and ask the user for a
  resolution action.
- **Branch creation failure** (`cargo make track-branch-create` non-zero): report the error.
  A pre-existing branch with the same name is the most common cause; adjust the track-id slug
  or rename the existing branch.
- **verify-track-metadata failure**: report the schema validation errors from the command output.
  Fix `metadata.json` fields accordingly and re-run the gate.

## Outputs

- `track/items/<track-id>/` directory (created)
- `track/items/<track-id>/metadata.json` (written, schema_version 5)
- `track/items/<track-id>/plan.md` (rendered view; do not edit directly)
- `track/registry.md` (regenerated; gitignored, not committed)
- Branch `track/<track-id>` (created and checked out)
- Gate verdict reported to the caller: **OK** or **ERROR** + error details
- No commit is created by this workflow (Step 6 is optional and delegates to other workflows)
