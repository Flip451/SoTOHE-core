# Plan Workflow SSoT

> Provider-agnostic workflow SSoT for the `plan` track workflow. Both the Claude adapter
> (`.claude/commands/track/plan.md`) and the Codex skill adapter
> (`.agents/skills/track-plan/SKILL.md`) reference this file. Provider-specific invocation
> framing lives in those adapters; the full workflow contract lives here.

## Mission

Plan a feature via the canonical track planning workflow — a state-machine orchestrator that
drives Phase 0 → Phase 1 → Phase 2 → Phase 3 through the four independent phase workflows,
delegating each phase to its writer capability. The pre-track stage must have authored an ADR
under `knowledge/adr/` beforehand. Back-and-forth escalation is triggered automatically when
a downstream gate fails. Each capability dispatch command resolves provider routing internally
from `.harness/config/agent-profiles.json`.

Sub-workflows used:

- `.harness/workflows/track/init.md` (Phase 0)
- `.harness/workflows/track/review.md` (Phase 0 ADR-baseline review loop)
- `.harness/workflows/track/commit.md` (Phase 0 ADR-baseline commit)
- `.harness/workflows/track/spec-design.md` (Phase 1)
- `.harness/workflows/track/type-design.md` (Phase 2)
- `.harness/workflows/track/impl-plan.md` (Phase 3)

## Inputs

- **Feature name or slug** — supplied as the primary argument. If absent, ask the user for a
  feature name and stop.
- **`max_retry`** — optional integer (default 5). If the argument parses as a bare integer it
  is treated as `max_retry`; otherwise as the feature name. A `<feature> <integer>` pair sets
  both.
- **ADR existence** — at least one relevant ADR must exist under `knowledge/adr/`. If none
  exists, stop and ask the user to author one (the `adr:add` command provides this path).
- **Primary ADR source filename** — before Phase 0, the orchestrator selects the relevant ADR's
  direct Markdown filename under `knowledge/adr/` to create an init designation record. This is a
  required explicit input to `init`; it must not be inferred from the feature name or stored as a
  metadata pointer. After stamping, the ledger init record is the primary designation.
- **Current branch** — must be compatible with the operation: the configured base branch
  (`.harness/config/branch-strategy.json#base_branch`) for a new track, `track/<id>` for a
  track already initialized.

## Sequence

### Preamble: register the phase chain as tasks

Before executing the state machine, register the following items as a task list so progress
stays visible across phases and back-and-forth loops:

1. Phase 0 — invoke `init` workflow, then the ADR-baseline `review` loop (stamp-free
   in-place convergence with per-edit guardian judgment) through user adjudication, the
   boundary review-refinement stamp, staging, and the ADR-baseline commit (closing the
   Phase 0 adjudication boundary)
2. Phase 1 loop — invoke `spec-design` workflow, evaluate spec → ADR signal, escalate on 🔴
   (delta-candidate lane — the input box is frozen after the boundary)
3. Phase 2 loop — invoke `type-design` workflow, evaluate type → spec signal per layer, escalate on 🔴
4. Phase 3 loop — invoke `impl-plan` workflow, evaluate task-coverage gate, re-invoke on ERROR
5. Termination — unexpected-divergence triage only; admitted delta drafts stay 🟡 for the
   merge-stage adjudication

Mark each item `in_progress` before executing and `completed` after it passes. When back-and-forth
escalation runs, append a sub-task for each re-invocation.

### SoT Chain (dependency direction)

```
ADR
  ↑ ①
spec (spec.json)
  ↑ ②
type contract (<layer>-types.json)
  ↑ ③
implementation (Rust code)
```

| # | Reference source → target | Evaluation |
|---|---|---|
| ① | spec → ADR | Phase 1 evaluates each spec element's `adr_refs[]` / `convention_refs[]` / `informal_grounds[]` (🔵🟡🔴) |
| ② | type contract → spec | Phase 2 evaluates each catalogue entry's `spec_refs[]` / `informal_grounds[]` per layer |
| ③ | implementation → type contract | Phase 4+; evaluated by rustdoc extraction cross-checked against catalogue declarations in CI |

Reverse references and layer skipping are forbidden: `spec → type catalogue`,
`ADR → track-internal artifact`, `type catalogue → ADR` are all disallowed.

### Phase invocation table

| Phase | Workflow | Writer capability | Gate |
|-------|----------|-------------------|------|
| 0 | `init` → `review` → `commit` | orchestrator (direct) | init identity, ADR-baseline `zero_findings` or adjudication-ready → user adjudication → final `zero_findings`, and ADR-baseline commit |
| 1 | `spec-design` | spec-designer | spec → ADR signal (🔵🟡🔴) |
| 2 | `type-design` | type-designer | type → spec signal, per layer (🔵🟡🔴) |
| 3 | `impl-plan` | impl-planner | task-coverage binary gate (OK / ERROR) |

### Phase 0: init workflow

1. Invoke the `init` workflow (`.harness/workflows/track/init.md`) with the feature name **and**
   the direct ADR source filename to designate. `init` records that exact filename through its
   `--kind init` snapshot step; that ledger record becomes the primary designation. On ERROR,
   stop and report.
2. Invoke the `review` workflow for the ADR baseline. During the loop, input-box ADRs are
   converged in place: adr-editor applies each fix, adr-diagnoser judges the applied edit
   immediately (decision-preserved → retained; decision-breaking → reverted, with the
   alternative / no-change rationale relayed verbatim to the reviewer). Do not stamp during
   the loop. On `zero_findings` or the Phase 0 `adjudication-ready` exception, present the init-stamp diff, every guardian-withheld
   proposal, and every hearing-required proposal with its grounds to the user for the Phase 0 adjudication required by
   `knowledge/conventions/pre-track-adr-authoring.md#In-track 意味変更の裁定権`. Present the
   diff content itself in the user-visible chat body (changed hunks verbatim or faithfully
   summarized hunk by hunk); tool output or file references alone do not satisfy the
   presentation requirement.
3. After approval: when the adjudication decided a NEW decision in the hearing (including
   a semantic need on a post-merge input ADR), first have adr-editor implement the hearing
   (append to the pre-merge input ADR, or author the new ADR file with hearing-grounded
   `user_decision_ref`), obtain `hearing-conformant` from its conformance re-audit, and only
   then init-stamp any new file so it joins the input box. A `deviating` verdict reverts the
   edit and returns to the user; it never stamps. Then, for every input-box source whose
   converged text differs from that source's own init record, have
   adr-editor apply the approval `user_decision_ref` to the affected decisions, pass the
   adr-diagnoser re-audit, reconverge the current hash through a fresh review (findings
   that would change the adjudicated text semantically return to the user), then record one
   boundary review-refinement stamp for each such source — its required reason carries only
   the self-contained refinement explanation and the guardian verdict summary (transitional measure until the
   review-refinement kind is implemented: use the existing escalation kind with the reason
   opening declaring a review-refinement record). When a source's converged text equals its
   own init record, no extra stamp is made for that source. The boundary stamps change the protected baseline ledger,
   so re-run the review workflow to `zero_findings` after the stamp (or after confirming no
   stamp was needed) before staging; this refreshes the review hash against the final Phase 0
   operational artifacts.
4. Run `bin/sotp git add-all` and invoke the `commit` workflow for the ADR baseline. The
   commit gate's byte comparison uses the just-recorded boundary stamp (or init). Its
   success closes the Phase 0 adjudication boundary; the input box is frozen from here to
   track end. Mark Phase 0 `completed` only after that guarded commit succeeds; then
   proceed to Phase 1. The commit workflow owns its message and other commit preconditions.

### Phase 1 loop: spec-design workflow

1. Invoke the `spec-design` workflow (`.harness/workflows/track/spec-design.md`).
2. Read the signal result (blue / yellow / red counts + 🔴 element ids with cited ADR paths).
3. Apply the loop rule:
   - **🔵**: run `bin/sotp ref-verify run` (semantic review of Chain ①). On `[BLOCKED]`, treat
     as 🔴 (route to `adr-editor` or re-invoke `spec-design` depending on which side is wrong).
     On `[ESCALATE]`, report to user and stop. On `[OK]`, mark Phase 1 `completed` and proceed
     to Phase 2.
   - **🟡**: log warning and proceed to Phase 2. Yellow must be resolved before merge.
   - **🔴**: escalate through the delta-candidate lane (the input box is frozen — no
     in-place semantic edit, regardless of the target ADR's pre/post-merge state):
     a. Identify the target ADR path and decision from the 🔴 element.
     b. Dispatch `adr-editor` to author (or revise) a delta candidate under
        `knowledge/adr/` with non-user grounds, declaring any supersedes / refines targets
        in the draft body per
        `knowledge/conventions/pre-track-adr-authoring.md#In-track 意味変更の裁定権`.
        Briefing must include the 🔴 element(s), the originating signal verbatim, and the
        constraint "edit working tree only; do not commit, do not snapshot".
     c. Dispatch `adr-diagnoser` for the delta admission judgment (three-way). On admit
        (independent / clarifying) or modification-proposal, the candidate enters the
        delta box and stays 🟡 until the merge-stage user adjudication. On bounce, have
        adr-editor remove the candidate and route the presented resolution back to the
        originating element (typically a spec-side fix).
     d. After admission (which changes the adr scope), re-converge the ADR side before the re-dispatch: run `bin/sotp signal check-adr-user --gate commit`, then run the `review` workflow's single-scope re-entry round for the adr scope only (`.harness/workflows/track/review.md` §Single-scope re-entry round) to `zero_findings`; do not invoke `review-fix-lead` directly or launch the all-required-scopes review wave here. Run the ADR signal check again so it reflects any review-driven repair; repeat until both checks satisfy the re-entry prerequisite in `knowledge/conventions/sot-reentry-sequencing.md`. Downstream scopes stay halted until their own upstream re-converges. Then re-invoke `spec-design` so the failing element(s) cite the admitted draft and Chain ① regenerates — what rides to the merge gate is the draft's chain ⓪ 🟡, never a standing Chain ① 🔴. After a bounce, re-invoke `spec-design` with the resolution instead (a bounce leaves the ADR unchanged, so no adr re-convergence is needed). Count either retry against `max_retry`; on overflow, stop.
     e. Non-semantic fixes to an input-box ADR (typo / reference path) follow the apply-then-classify lane instead: adr-editor applies, adr-diagnoser classifies, and only a non-semantic verdict is retained and restamped kind: non-semantic-fix. After a retained restamp, re-converge the ADR side through the same signal check and adr-scoped `review` workflow lifecycle before re-invoking `spec-design`, per `knowledge/conventions/sot-reentry-sequencing.md`.

### Phase 2 loop: type-design workflow

1. Invoke the `type-design` workflow (`.harness/workflows/track/type-design.md`).
2. Read the per-layer signal result.
3. Apply the loop rule:
   - **🔵 all layers**: run `bin/sotp ref-verify run` (semantic review covering Chain ① and
     Chain ②). On `[BLOCKED]`, route by owning side: catalogue-side → invoke `type-designer`,
     re-run semantic review; spec-side → treat as Phase 2 🔴 (see below). On `[ESCALATE]`,
     report to user and stop. On `[OK]`, mark Phase 2 `completed` and proceed to Phase 3.
   - **🟡**: log warning and proceed. Yellow must be resolved before merge.
   - **🔴**:
     a. Re-invoke `spec-design` workflow (Phase 2 🔴 typically indicates spec needs refinement).
     b. Re-evaluate Phase 1 gate. If Phase 1 also 🔴, escalate via Phase 1 ADR loop.
     c. On Phase 1 🔵 or 🟡, re-converge the spec before the re-dispatch: the spec edit staled the spec scope, so re-evaluate the Phase 1 signal gate and run the `review` workflow's single-scope re-entry round for the spec scope only (`.harness/workflows/track/review.md` §Single-scope re-entry round) to `zero_findings`; do not invoke `review-fix-lead` directly or launch the all-required-scopes review wave here. Re-evaluate the Phase 1 signal gate again so it reflects any review-driven repair; if it is 🔴, return to the Phase 1 loop and re-run the scoped review after its repair. The spec's semantic-verification element is **deferred** here per `knowledge/conventions/sot-reentry-sequencing.md`: the verification surface cannot evaluate Chain ① in isolation while the catalogues are pending regeneration by the upcoming `type-design` re-run (existence-based scope resolution would evaluate — and can abort on — stale Chain ② pairs). State the deferral explicitly in the `type-design` dispatch briefing, then re-invoke `type-design`, and immediately after its regeneration run the mandatory full `bin/sotp ref-verify run` (the Phase 2 🔵 step) before any further descent; a failure there routes back per the immediate bounce-back rule. The signal and review elements above are not deferrable. The types scope otherwise resumes only after the spec re-converges.
     d. The Phase 2 retry counter is independent of Phase 1's. Count against `max_retry`.

### Phase 3 loop: impl-plan workflow

1. Invoke the `impl-plan` workflow (`.harness/workflows/track/impl-plan.md`).
2. Read the binary gate verdict (OK / ERROR).
3. Apply the loop rule:
   - **OK**: mark Phase 3 `completed` and proceed to Termination.
   - **ERROR**: re-invoke `impl-plan`. Count against `max_retry`; on overflow, stop and
     present the latest error to the user.

### Termination

After Phase 3 OK:

1. Triage only unexpected ADR working-tree divergence. Do not stage, commit, revert, or
   request a user decision here; route a suspected deviation through the established guardian /
   recovery lane.
2. Leave every admitted delta candidate in the delta box as intentional 🟡 for the merge-stage
   user adjudication.
3. Mark the Termination task `completed`.

### Writer ownership

| Phase | Artifact | Writer |
|-------|----------|--------|
| Pre-track | `knowledge/adr/*.md` (initial) | user + `adr:add` |
| Pre-track | `knowledge/adr/*.md` (back-and-forth) | `adr-editor` capability (auto-invoked) |
| 0 | `track/items/<id>/metadata.json` | orchestrator (direct via `init` workflow) |
| 1 | `track/items/<id>/spec.json` + `spec.md` | `spec-designer` capability |
| 2 | `track/items/<id>/<layer>-types.json` + baselines + views | `type-designer` capability |
| 3 | `track/items/<id>/impl-plan.json` + `task-coverage.json` | `impl-planner` capability |

The orchestrator does not directly write `knowledge/adr/*.md`, `spec.json`,
`<layer>-types.json`, `impl-plan.json`, or `task-coverage.json`. Each artifact's writer
capability owns its file end-to-end.

### Sub-workflow briefing rules (no design prescription)

When composing a briefing for a writer capability, the briefing body MUST contain only:

- Problem statement / trigger (what was observed, what symptom)
- Context references (track state, file paths, relevant ADRs / conventions, prior edits in
  the working tree)
- Interaction contract (what the capability should report back, and operational constraints)

The briefing body MUST NOT contain design prescription — anything that pre-solves the
capability's domain expert judgment. Each capability is the domain expert for its owned artifact.

## Gates

| Gate style | Phases | Signals |
|------------|--------|---------|
| ADR-baseline review + user adjudication + commit | Phase 0 after `init` | `zero_findings` → adjudicate → stamp as required → commit |
| SoT Chain signal (🔵🟡🔴) | Phase 1, Phase 2 | Blue = pass, Yellow = warn + proceed, Red = escalate |
| Binary check (OK / ERROR) | Phase 0 `init`, Phase 3 | OK = pass, ERROR = re-invoke or stop |

Pre-approval exceptions (outside the gate system — user is asked only on irreversible actions):
`git push` / `git commit`, external API calls (PR / issue creation), destructive filesystem
operations, environment-breaking changes. Artifact generation uses post-hoc review.

## Failure / recovery

- **No ADR**: stop and ask the user to author one before running this workflow.
- **Non-compatible branch**: report the branch and available options.
- **Phase N 🔴 after max_retry overflows**: stop and present options (continue with warnings,
  abort, manual edit).
- **Phase 1 ADR-side semantic need (any lifecycle)**: route through the delta-candidate
  lane (author → admission judgment → re-ground the failing element on the admitted
  draft); never edit the input box in place after the Phase 0 boundary.
- **Delta admission bounce**: remove the candidate, apply the presented resolution on the
  originating side, and retry; a resubmission with new grounds re-enters the same
  judgment.
- **`[ESCALATE]` from `ref-verify`**: report to user and stop. Do not retry.

## Outputs

- `track/items/<id>/metadata.json` (Phase 0)
- `track/items/<id>/spec.json` + `spec.md` (Phase 1)
- `track/items/<id>/<layer>-types.json` + views (Phase 2, per TDDD-enabled layer)
- `track/items/<id>/impl-plan.json` + `task-coverage.json` (Phase 3)
- Per-phase gate results (🔵🟡🔴 / OK / ERROR) and final `max_retry` counters
- Back-and-forth edits that occurred (target artifact and its writer)
- Unexpected ADR working-tree divergence triage (if any), and admitted delta drafts left 🟡 for
  merge-stage adjudication
- The guarded Phase 0 ADR-baseline commit result
