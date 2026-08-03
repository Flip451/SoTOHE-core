# Recover Workflow SSoT

> Provider-agnostic workflow SSoT for recovery after a conflicted guarded base merge. Provider-specific adapters reference this file; they own only invocation framing, tool constraints, and reporting.

## Mission

Recover an active track after `bin/sotp track merge-base` reports a conflict. The orchestrator coordinates resolution through each artifact's designated owner, then drives the normal review and guarded-commit lanes. A conflict never runs clean-merge cleanup; that cleanup is exclusive to the successful guarded merge command.

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
invoke the same writer in normal mode. If the remaining failure is base-induced drift without a
conflict hunk, use the affected chain's normal writer/implementer reconciliation and the guarded,
chain-limited review route below before demanding the global pre-gates. If neither route can make
all pre-gates pass, retain the conflict and fail closed.

Whenever direct hunk selection or a `conflict-preparation` dispatch touches an ADR, immediately
dispatch `adr-diagnoser` for the lifecycle-specific two-box verdict required by
`.harness/policies/pre-track-adr-authoring.md`; follow that verdict before any downstream
re-entry, review, or commit.

For that D2-only, base-induced-drift route, run the existing guarded CLI surfaces directly for the
affected chain and scope (without claiming global pre-gate convergence):

Before invoking the raw review command, the orchestrator must write or refresh
`tmp/reviewer-runtime/briefing-<affected-scope>.md` from the current post-merge SoT by following
Step 3 of `.harness/workflows/track/review.md`: recompute the design intent, scope checklist,
architecture constraints and verification checklist from the current ADR/spec/plan/source, and
replace any prior briefing rather than reusing it. The canonical scope-specific severity policy is
still injected by `bin/sotp review local`.

```
bin/sotp ref-verify run --track-id <id>
bin/sotp ref-verify results --track-id <id> --chain <affected-chain>
bin/sotp adr-baseline check-review --track-id <id>
bin/sotp review local --track-id <id> --round-type <fast|final> --group <affected-scope> \
  --briefing-file tmp/reviewer-runtime/briefing-<affected-scope>.md
```

Inspect the chain-scoped result after every run. If the full run aborts while unrelated or stale
downstream artifacts prevent enumeration, do not treat that abort as an affected-chain failure;
regenerate the affected chain, rerun the full command, and inspect the chain result as soon as it is
enumerable. Run the applicable chain signal check before the review when its upstream artefact was
regenerated. This pre-gate-free review route is permitted only for base-induced drift with no
conflict hunk; once the affected chain is reconciled, rerun the normal global pre-gates and
canonical review workflow.

**Step 3: Verify and review**

Run the applicable implementation verification, then invoke the canonical provider-neutral review
workflow until every required scope reports `zero_findings`; the active adapter supplies the
provider-specific invocation.

If verification or review fails, repair the resolution and repeat this step. Do not run clean-merge cleanup from this workflow.

**Step 4: Guarded commit (staging and commit)**

After review is clean, stage the intended recovery scope through a guarded staging surface:

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
| 2 | `task-contract-check`, signal, `track-active-gate`, and scoped-review pre-gates pass (or the D2-only chain-limited route above is recorded while reconverging) | pass / stop |
| 3 | Required implementation verification | pass / fail |
| 3 | Canonical review reaches `zero_findings` | pass / fail |
| 4 | Guarded staged diff matches the recovery scope | pass / fix-staging |
| 4 | Guarded commit succeeds | pass / fail |

## Failure / recovery

- **No conflicted guarded-merge context**: stop and report that recovery is not authorized for the current worktree.
- **Parseability preparation failure**: retain the conflict; do not launch a scoped review or descend to a downstream writer.
- **Resolution or verification failure**: retain the conflict for correction; do not claim completion and do not run clean-merge cleanup.
- **Designated writer or regeneration failure**: retain the conflict and use that surface's failure route; do not substitute an orchestrator edit.
- **Review finding**: repair through the normal review/fix loop, then repeat review.
- **Staging or guarded commit failure**: follow the commit workflow's failure handling and retry only through its guarded staging and commit surfaces.

## Outputs

- A conflict resolution that has passed the normal verification and review gates.
- A guarded commit and its normal repository note.
- No clean-merge views, baseline, or sync-base cleanup from this workflow.
