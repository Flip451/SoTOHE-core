# Recover Workflow SSoT

> Provider-agnostic workflow SSoT for recovery after a conflicted guarded base merge. Provider-specific adapters reference this file; they own only invocation framing, tool constraints, and reporting.

## Mission

Recover an active track after `bin/sotp track merge-base` reports a conflict. The orchestrator coordinates resolution through each artifact's designated owner, then drives the normal review and guarded-commit lanes. A conflicted merge already replaced the type baselines from the exact merged base commit and regenerated the derived views (cleanup-state ADR D3); this workflow does not repeat either cleanup stage.

**Cleanup-failure entry.** When the merge instead reported `cleanup failed` (`ConflictedCleanupFailed`), the cleanup stages did NOT complete, and every statement in this workflow that assumes replaced baselines or regenerated views does not yet hold. Per cleanup-state ADR D3 the failure is reported only and recovery is manual and operator-owned: the orchestrator stops here and reports the failure to the user; it does not attempt filesystem or VCS recovery itself, and no agent-invocable surface performs it (the baseline-capture surface is first-write-wins and does not replace retained stale files). The operator completes both stages by hand — replacing the type baselines from the exact merged base commit in a separate checkout and regenerating the views with `bin/sotp track views sync` — and confirms completion; only then does the rest of this workflow, pre-gates included, apply.

## Inputs

- **Current branch** — must be the active `track/<id>` branch with a merge conflict left by `bin/sotp track merge-base`.
- **Conflict resolution** — the source changes needed to resolve the reported conflict.

## Sequence

**Step 1: Confirm the recovery context**

Confirm that the current branch is the active track branch and that the preceding guarded base-merge operation ended in a conflict. If either condition is absent, stop without changing the worktree. Start a base merge only through `bin/sotp track merge-base`; do not substitute direct VCS merge or filesystem orchestration.

**Step 2: Resolve the conflict**

Re-check the intended behavior against the track's current source-of-truth chain, then resolve only
the conflicted artifacts needed for a correct resolution. Treat every SoT conflict as a sequential
re-entry: process ADR → spec → catalogues → implementation plan, and keep each downstream writer
halted until its direct upstream has re-converged. Before each writer dispatch and each descent,
confirm the applicable signal, chain-limited semantic verification, and scoped `zero_findings`
review required by `.harness/policies/sot-reentry-sequencing.md`; a newly discovered upstream
change immediately returns the recovery to that upstream surface rather than being deferred to
Step 3.

Route every conflict through its designated writer or regeneration surface. For an ADR, dispatch
`adr-editor` in `conflict-preparation` mode only after selecting the existing hunks has failed to
make the required pre-gates parseable; provide the affected path, those hunks, the originating
input verbatim, the effective `merge_target`, and explicit instructions both to add no new meaning
and to edit the working tree only (do not commit or snapshot). Do not dispatch `adr-editor` in
normal mode until upstream reconvergence. Route `spec.json` through `spec-designer`; `*-types.json` through
`type-designer`; `impl-plan.json`, `task-coverage.json`, `task-contract.json`, and
`batch-plan.json` through `impl-planner`; and generated views through `bin/sotp track views sync`.
Resolve production-source and test conflicts through the normal implementation-delegation surface.
Do not hand-edit a sole-writer artifact or a generated view outside the documented
`conflict-preparation` source-repair boundary, and do not invoke direct VCS operations.

**Pre-gate blocker and conflict-edit exception.** Apply the narrowly scoped source-repair exception
in `.harness/policies/sot-reentry-sequencing.md`: if conflict hunks prevent the actual
`task-contract-check`, signal, `track-active-gate`, or scoped-review gate from passing, the
orchestrator first selects only existing hunks and records affected paths. If the actual pre-gates
still do not pass, invoke each affected designated writer in `conflict-preparation` mode; that
mode may resolve existing hunks and regenerate derived artifacts only, never meaning or
placeholders. Do not claim SoT convergence during preparation. After upstream reconvergence,
invoke the same writer in normal mode. Base-induced signal drift is not a gate-bypass case: the
conflicted guarded merge itself already replaced the type baselines from the exact merged base
commit (cleanup-state ADR D3) — or, after a reported cleanup failure, the operator has completed
that replacement per the cleanup-failure entry — so the normal pre-gates and the canonical review
workflow apply throughout the recovery. If the pre-gates still cannot pass, retain the conflict and fail closed.

Whenever direct hunk selection or a `conflict-preparation` dispatch touches an ADR, immediately
dispatch `adr-diagnoser` for the lifecycle-specific two-box verdict required by
`.harness/policies/pre-track-adr-authoring.md`; follow that verdict before any downstream
re-entry, review, or commit.

**Step 3: Verify and review**

Run the applicable implementation verification, then invoke the canonical provider-neutral review
workflow until every required scope reports `zero_findings`; the active adapter supplies the
provider-specific invocation.

If verification or review fails, repair the resolution and repeat this step. Do not run the merge cleanup stages from this workflow.

**Step 4: Guarded commit (staging and commit)**

Staging uses the repository-wide guarded surface, so its scope is only correct when the working
tree contains recovery changes alone. Two obligations bound this:

- **Precondition (at merge start):** `bin/sotp track merge-base` mechanically enforces a clean
  worktree before attempting the merge, rejecting tracked or non-ignored untracked changes.
  Begin it only from a clean working tree; unrelated local edits that Git would tolerate (because
  they do not overlap the merge) must be committed, stashed through the guarded stash surface, or
  otherwise cleared first. Starting a guarded merge over unrelated dirt forfeits the staging scope
  guarantee below.
- **Verification (before staging):** after review is clean and before staging, confirm the
  working tree holds only the recovery scope (conflict resolution, regenerated artifacts, and
  recovery records). If unrelated changes are present, stop and clear them through their own
  lane instead of staging; do not bundle them into the recovery commit.

Then stage the recovery scope through the guarded staging surface:

```
bin/sotp git add-all
```

Generate an explicit recovery commit message that describes the resolved conflict, then invoke
the canonical provider-neutral commit workflow with that message; the active adapter supplies the
provider-specific invocation. Always use `bin/sotp git add-all` after the final
review; the commit workflow owns the complete staged scope.

The commit workflow validates the staged scope, creates the commit, and attaches the repository
note. On success, report that conflict recovery is complete and the track remains on its track
branch.

## Gates

| Step | Gate | Verdict |
| --- | --- | --- |
| 1 | Active track branch and conflicted guarded-merge context | pass / stop |
| 2 | `task-contract-check`, signal, `track-active-gate`, and scoped-review pre-gates pass | pass / stop |
| 3 | Required implementation verification | pass / fail |
| 3 | Canonical review reaches `zero_findings` | pass / fail |
| 4 | Guarded staged diff matches the recovery scope | pass / fix-staging |
| 4 | Guarded commit succeeds | pass / fail |

## Failure / recovery

- **No conflicted guarded-merge context**: stop and report that recovery is not authorized for the current worktree.
- **Merge reported `cleanup failed`**: stop and escalate to the user per the cleanup-failure entry above; the operator completes both cleanup stages manually before any pre-gate or Step 2 work runs, and the recovery report records that the baselines and views were manually recovered.
- **Parseability preparation failure**: retain the conflict; do not launch a scoped review or descend to a downstream writer.
- **Resolution or verification failure**: retain the conflict for correction; do not claim completion and do not run the merge cleanup stages.
- **Designated writer or regeneration failure**: retain the conflict and use that surface's failure route; do not substitute an orchestrator edit.
- **Review finding**: repair through the normal review/fix loop, then repeat review.
- **Staging or guarded commit failure**: follow the commit workflow's failure handling and retry only through its guarded staging and commit surfaces.

## Outputs

- A conflict resolution that has passed the normal verification and review gates.
- A guarded commit and its normal repository note.
- No cleanup from this workflow; the type baselines and derived views were already refreshed by the conflicted merge itself — or, after a reported cleanup failure, by the operator's manual recovery, which the recovery report records.
