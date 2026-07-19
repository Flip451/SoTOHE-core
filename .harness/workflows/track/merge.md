# Merge Workflow SSoT

> Provider-agnostic workflow SSoT for the `merge` track workflow. Provider-specific adapters
> (e.g. `.claude/commands/track/merge.md`) reference this file. Provider-specific invocation
> framing lives in those adapters; the full workflow contract lives here.

## Mission

Wait for a PR's CI checks to pass, then merge it using the configured merge method. Fail closed
on any check failure or wait timeout. The merge method must be resolved from the PR's track
`branch_strategy_snapshot.merge_method` unless the caller explicitly overrides it — the workflow
must not substitute a hard-coded default (e.g. `squash`, `rebase`, `merge`) at any layer.

## Inputs

- **PR number** — required. When the caller invokes the workflow without an explicit PR number,
  the adapter is expected to resolve one from the current branch (`gh pr view --json number -q
  .number`).
- **Optional merge method** — one of `merge`, `squash`, or `rebase`. When omitted, the merge
  method is resolved from the PR's track `branch_strategy_snapshot.merge_method` (via
  `BranchStrategyPort::merge_method()`).

## Sequence

**Step 0: Resolve PR**

Determine the target PR number, either from the caller's explicit argument or (as an adapter
convenience) via `gh pr view --json number -q .number` for the current branch. Parse an optional
merge method appended to the argument (e.g. `123 squash`) only when supplied literally.

**Step 1: Wait and merge**

Invoke the merge wrapper. Omit `--method` unless the caller explicitly supplied one — passing an
empty or implicit default would bypass the configured merge method.

```
bin/sotp pr wait-and-merge <pr_number>                     # method resolved from configured default
bin/sotp pr wait-and-merge <pr_number> --method <method>    # explicit caller override
```

`bin/sotp pr wait-and-merge` performs:

1. **Task completion guard**: blocks merge if any tasks in the PR's track `metadata.json` are
   unresolved (not `done` or `skipped`). This is the only workflow that enforces task
   completion — push and PR review are allowed with unresolved tasks.
2. **Strict merge-signal gate**: evaluates the PR head's signal-gate configuration at merge
   strictness (🟡 also blocks) after the task guard and before polling. On failure,
   `wait-and-merge` exits directly with `[BLOCKED] strict spec signal gate failed`; it is not a
   polled PR check. An intentional 🟡 routes to the dedicated adjudication recovery below.
3. Polls `gh pr checks` every 15 seconds with a 10 minute timeout.
4. **Method resolution**: when `--method` is omitted, resolves the merge method from the PR's
   track `branch_strategy_snapshot.merge_method`; an explicit `--method` always overrides it.
5. On all checks passed: merges via `gh pr merge --<method>`.
6. On any check failed: stops and reports the failing checks.
7. On timeout: stops and reports the pending checks.

**Step 2: Post-merge**

After a successful merge:

1. Report the merge result (PR URL, merge method, resulting commit).
2. Recommend the next action:
   - `/track:done` to switch to the configured base branch.
   - `/track:plan <feature>` to start the next piece of work.

## Gates

| Step | Gate | Verdict |
|------|------|---------|
| 1    | `bin/sotp pr wait-and-merge` exits 0 | pass / fail |
| 1    | Task completion guard passes | pass / fail |
| 1    | Strict merge-signal gate passes before polling | pass / fail (intentional-🟡 failure → adjudication recovery) |
| 1    | Polled PR checks all green | pass / fail |

All guards and checks are enforced inside `bin/sotp pr wait-and-merge`. A non-zero exit code
ends the workflow immediately — the workflow does not retry, and does not proceed to Step 2.

## Failure / recovery

- **Task completion guard failure**: resolve the unresolved tasks (`bin/sotp track transition
  <task_id> done|skipped`), then re-invoke the workflow.
- **Failing PR checks**: fix the underlying failure (source change / infra flake / config), push
  a new commit, and re-invoke.
- **Strict merge gate blocked on an intentional 🟡** (a track-born ADR draft or amendment
  proposal carrying only non-user grounds, reported directly by the wrapper before PR-check
  polling): this block is the designed adjudication point — the user is present at this
  workflow's invocation, so obtain their adjudication here. Before any recovery mutation, resolve the
  PR's head branch (`gh pr view <pr_number> --json headRefName`) and establish that checkout
  (`bin/sotp track branch switch <track-id>` when not already on it) so every edit, snapshot,
  and commit lands in the PR's own track context. Then: on adoption, dispatch `adr-editor` to
  promote the draft's grounds to `user_decision_ref`, run
  `bin/sotp adr-baseline snapshot --source <file> --kind new-adr --reason <text>`, commit the
  promoted ADR through the guarded review → commit flow, push via `bin/sotp pr push`, re-run
  the canonical `pr-review` workflow to a terminal state (the prior terminal verdict applied
  to the old HEAD), and only then re-invoke this workflow. On rejection, dispatch
  `adr-editor` to withdraw or revise the draft per the user's decision, then follow the same
  commit → push → `pr-review` → re-invoke sequence. Do not bypass or weaken the gate.
- **Wait timeout**: inspect the pending checks (`bin/sotp pr status <pr_number>`) and either wait
  longer (re-invoke) or diagnose the blocked check.

## Outputs

- Merged PR (or an explicit failure report — the workflow does not merge on any error path).
- A short post-merge summary (PR URL, merge method, resulting commit, next recommended
  command).
- The normal successful merge path creates no local commits. A recovery that follows user
  adjudication of an intentional 🟡 may create a guarded commit before the workflow is
  re-invoked.
