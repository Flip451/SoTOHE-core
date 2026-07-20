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
Before any audit or gate, resolve that PR's head branch and SHA (`gh pr view <pr_number> --json
headRefName,headRefOid`). The caller must already be checked out on that `track/<track-id>` branch
at that exact SHA; verify both the branch name and `HEAD` before continuing. If the branch is not
locally available or either value differs, stop without auditing or invoking the merge wrapper —
this workflow does not materialize or update a local PR branch. The head must resolve to a track
branch and its track metadata must be available. An explicit PR number never authorizes using the
caller’s current branch as a substitute for the PR head.

**Step 1: Terminal audit**

Before every merge attempt, present the merge-stage terminal audit required by
`knowledge/conventions/pre-track-adr-authoring.md#In-track 意味変更の裁定権` to the user. Audit
every protected source (every source with a ledger record), from its protection-start record to
its current terminal bytes, with per-record provenance and the adjacent diff for every
`non-semantic-fix` record. This audit is required even when every terminal diff is empty.

If the user adjudicates a record as a misclassified semantic change, complete the corrective
restoration route before continuing. Any audit-triggered mutation, including an adoption or
rejection recovery, invalidates the presentation: re-run the required review / commit / push /
PR-review work, then re-invoke this workflow so a fresh all-protected-source audit occurs before
the next merge attempt. Do not invoke the merge wrapper until this audit has completed without a
recovery.

**Step 2: Wait and merge**

Immediately before invoking the wrapper, re-resolve the PR head OID (`gh pr view <pr_number>
--json headRefOid`) and verify it still equals the Step 0-verified OID whose bytes the Step 1
audit presented. A mismatch means the audited bytes are no longer the merge candidate: abort
without invoking the wrapper and re-invoke this workflow from Step 0 for a fresh audit.

The current wrapper re-fetches the remote ref while polling and merging without binding the
audited OID, so a head change inside that window is not rejected mechanically. This residual
window is a user-adjudicated accepted operational trade-off, not an oversight: the workflow is
user-attended at invocation and the repository is single-writer, so a head change inside the
window can originate only from the operator's own concurrent push. The target state remains the
follow-up wrapper implementation that carries the audited OID into the native merge (e.g.
`gh pr merge --match-head-commit`); once it ships, the head-bound form becomes the only
authorized invocation and this residual window closes.

After every wrapper outcome, re-resolve the PR head OID and compare it with the audited OID; on
success, also verify that the merge result records that audited PR-head commit (rather than
comparing the generated merge commit itself). Any mismatch is a fail-closed audit-invalidating
incident for corrective adjudication, never an ordinary success or failure.

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
2. **Strict merge-signal gate**: evaluates the signal-gate configuration at merge strictness
   (🟡 also blocks) after the task guard and before polling. On failure, `wait-and-merge`
   exits directly with a blocked report; it is not a polled PR check. The blocked report is
   generic — it does not identify whether a blocking finding is an intentional 🟡 (an
   admitted delta draft awaiting the user's adjudication), and the orchestrator must not
   make that distinction by judging content. Route every strict-gate block through the
   block triage in Failure/recovery below; only a guardian-confirmed admitted draft enters
   the dedicated adjudication recovery.
3. Polls `gh pr checks` every 15 seconds with a 10 minute timeout.
4. **Method resolution**: when `--method` is omitted, resolves the merge method from the PR's
   track `branch_strategy_snapshot.merge_method`; an explicit `--method` always overrides it.
5. On all checks passed: merges via `gh pr merge --<method>`. (The follow-up implementation
   will additionally bind the audited expected-head OID to this merge call.)
6. On any check failed: stops and reports the failing checks.
7. On timeout: stops and reports the pending checks.

This workflow also hosts the merge-stage user adjudication of the two-box model
(`knowledge/conventions/pre-track-adr-authoring.md#In-track 意味変更の裁定権`): admitted
delta drafts and the terminal audit are decided here, with the user present at invocation.

**Step 3: Post-merge**

After a successful merge:

1. Report the merge result (PR URL, merge method, resulting commit).
2. Recommend the next action:
   - `/track:done` to switch to the configured base branch.
   - `/track:plan <feature>` to start the next piece of work.

## Gates

| Step | Gate | Verdict |
|------|------|---------|
| 1    | All-protected-source terminal audit completes without recovery | pass / fail |
| 2    | Pre-invocation and post-outcome PR-head checks match the audited OID | pass / fail (pre-invocation mismatch → re-invoke from Step 0; post-outcome mismatch → audit-invalidating incident to the user; the in-wrapper window is the user-adjudicated accepted residual until the head-bound wrapper ships) |
| 2    | `bin/sotp pr wait-and-merge` exits 0 | pass / fail |
| 2    | Task completion guard passes | pass / fail |
| 2    | Strict merge-signal gate passes before polling | pass / fail (blocked → block triage; guardian-confirmed intentional 🟡 → adjudication recovery) |
| 2    | Polled PR checks all green | pass / fail |

The Step 1 terminal audit and the Step 2 head-OID checks are orchestrator-enforced gates
outside `bin/sotp pr wait-and-merge`. The Step 2 task, signal, and PR-check guards are
enforced inside that wrapper.
An ordinary non-zero wrapper exit ends this invocation without proceeding to Step 3. The
exception is a strict merge-signal block that the block triage below confirms as an intentional
🟡 admitted delta draft: enter the adjudication recovery, complete its required work, and
re-invoke this workflow from Step 1 for a fresh terminal audit.

## Failure / recovery

- **Task completion guard failure**: resolve the unresolved tasks (`bin/sotp track transition
  <task_id> done|skipped`), then re-invoke the workflow.
- **Failing PR checks**: fix the underlying failure (source change / infra flake / config), push
  a new commit, and re-invoke.
- **Post-outcome head-OID mismatch (audit-invalidating incident)**: the merged or gated bytes
  were never presented in the required terminal audit. Stop immediately and surface the
  mismatch to the user for corrective adjudication; never report it as an ordinary success or
  failure.
- **Strict merge gate blocked (block triage first)**: the wrapper's blocked report does not
  distinguish an intentional 🟡 from an ordinary signal failure, and no machine admission
  record exists yet. The orchestrator must not make that distinction by judging content.
  Before triage, resolve the PR head branch and SHA again. If either differs from the Step 0
  verified checkout, stop and re-invoke this workflow from Step 0; never inspect a different or
  stale local worktree. The aggregate count in the wrapper's blocked report is not sufficient.
  From that verified PR-head checkout, enumerate every `decisions[]` front-matter entry carrying
  `review_finding_ref`, recording its ADR path, decision ID, and grounds. A track-born draft
  candidate must satisfy all of these structural conditions: its source is absent from the PR
  track's ADR-baseline ledger; its ADR path is an added path on this PR (not merely modified);
  and the commit that introduced that path descends from the Phase 0 ADR-baseline commit (the
  first commit on the track that records this ledger). This metadata, ledger, and branch-history
  comparison excludes pre-existing ADRs that happen to carry review grounds. Do not inspect body
  content or infer whether the candidate was admitted. For each candidate that passes all three
  checks, dispatch `adr-diagnoser` with the bytes from that same PR-head checkout to re-judge the current text (the byte-bound
  interim admission procedure of
  `knowledge/conventions/pre-track-adr-authoring.md#In-track 意味変更の裁定権`); only a
  verdict confirming the admitted, judged text routes that block into the adjudication
  recovery below. Every other blocking signal (🔴, a 🟡 outside a track-born draft, or a
  candidate the guardian does not confirm) is an ordinary gate failure: resolve it upstream
  through the normal lanes and re-invoke; do not enter the adjudication recovery for it.
  Once the follow-up machine admission record ships, `bin/sotp pr wait-and-merge` is
  expected to surface the intentional-🟡 case as a distinct machine-readable blocked
  status, replacing this diagnoser round-trip.
- **Adjudication recovery (entered only via the block triage above)**: this block is
  the designed adjudication point — the user is present at this workflow's invocation, so
  obtain their adjudication here. The block triage has already resolved, fetched, and established
  the PR-head checkout, so every recovery edit, stamp, and commit lands in the PR's own track
  context. Then follow
  the merge-stage procedures of
  `knowledge/conventions/pre-track-adr-authoring.md#In-track 意味変更の裁定権`:
  - **Adoption**: dispatch `adr-editor` to promote the draft's grounds to
    `user_decision_ref` (an edit that voids the admission — re-judge and re-admit via
    `adr-diagnoser`), pass the adoption-conformance re-audit, then run
    `bin/sotp adr-baseline snapshot --source <file> --kind new-adr --reason <text>` (the
    reason records the origin-input provenance and the judgment summaries). Commit through
    the guarded review → commit flow, push via `bin/sotp pr push`, re-run the canonical
    `pr-review` workflow to a terminal state, and only then re-invoke this workflow.
  - **Rejection**: dispatch `adr-editor` to delete the draft (or apply the instructed
    revision), then pass the rejection-conformance re-audit. A deletion is final only after
    that re-audit. A revision invalidates admission, so re-run the three-way admission judgment
    before the revised candidate is cited or carried forward; on a bounce, remove it and return
    the resolution to its origin. Rework every downstream artifact that cited or derived from
    the deleted or newly admitted revised draft in SoT-chain order, then follow the same
    commit → push → `pr-review` → re-invoke sequence.
  Do not bypass or weaken the gate.
- **Terminal-audit misclassification adjudication**: when the user adjudicates a presented
  ledger record as a misclassified semantic change, run the corrective-restoration route
  (adr-editor restores the prior valid text, adr-diagnoser confirms the byte match, then a
  reason-less `--kind non-semantic-fix` restamp), complete any required re-processing, and
  re-invoke.
- **Wait timeout**: inspect the pending checks (`bin/sotp pr status <pr_number>`) and either wait
  longer (re-invoke) or diagnose the blocked check.

## Outputs

- Merged PR (or an explicit failure report — the workflow does not merge on any error path).
- A short post-merge summary (PR URL, merge method, resulting commit, next recommended
  command).
- The normal successful merge path creates no local commits. A recovery that follows user
  adjudication of an intentional 🟡 may create a guarded commit before the workflow is
  re-invoked.
