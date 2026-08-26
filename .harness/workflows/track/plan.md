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

## Summary-first context intake

Once a track is active, use CLI summaries as the orchestrator's primary context for progress,
review necessity, obligation state, and catalogue state:

- `bin/sotp track resolve`, `bin/sotp track task-counts`, and `bin/sotp track next-task` report
  phase, task progress, and the next open task.
- `bin/sotp review results` reports which review scopes are required, approved, or empty.
- `bin/sotp test-obligation results` reports the obligation lanes and cached fulfillment state when
  the enrollment artifacts exist.
- `bin/sotp catalog check` and `bin/sotp ref-verify results --chain 2 --filter all` report
  catalogue completion and catalogue-to-specification verification state.

Use the output of the phase command or delegated capability as the primary result; do not begin
by reading all track artifacts, type catalogues, review or binding JSON, or full sub-workflow
texts. Open an artifact body only for a targeted diff or blocker investigation, and use the
corresponding CLI result instead of opening `review.json` or a cache to determine state. The
orchestrator does not open a `Related Conventions` list: the capability dispatcher places the
resolved convention paths (possibly an empty set) in the delegated briefing, and the capability
reads those paths.

## Sequence

### Preamble: register the phase chain as tasks

Before executing the state machine, register the following items as a task list so progress
stays visible across phases and back-and-forth loops:

1. Phase 0 — invoke `init`, ADR-baseline `review`, and ADR-baseline `commit` in that order,
   following `.harness/policies/pre-track-adr-authoring.md#In-track 意味変更の裁定権`
2. Phase 1 loop — invoke `spec-design` workflow, evaluate spec → ADR signal, escalate on 🔴
   (delta-candidate lane — the input box is frozen after the boundary)
3. Phase 2 loop — invoke `type-design` workflow, evaluate type → spec signal per layer, escalate on 🔴
4. Phase 3 loop — invoke `impl-plan` workflow, evaluate the task-coverage gate AND the batch-plan structural gate, re-invoke while either is ERROR
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
| 0 | `init` → `review` → `commit` | orchestrator (direct) | the governing convention's Phase 0 gates, then ADR-baseline commit |
| 1 | `spec-design` | spec-designer | spec → ADR signal (🔵🟡🔴) |
| 2 | `type-design` | type-designer | type → spec signal, per layer (🔵🟡🔴) |
| 3 | `impl-plan` | impl-planner | task-coverage binary gate AND batch-plan structural gate (each OK / ERROR) |

### Phase 0: init workflow

1. Invoke the `init` workflow (`.harness/workflows/track/init.md`) with the feature name **and**
   the direct ADR source filename to designate. `init` records that exact filename through its
   `--kind init` snapshot step; that ledger record becomes the primary designation. On ERROR,
   stop and report.
2. Invoke the `review` workflow for the ADR baseline, then follow
   `.harness/policies/pre-track-adr-authoring.md#In-track 意味変更の裁定権` for Phase 0
   convergence, user adjudication, and boundary-stamp preparation — up to but excluding the
   ADR-baseline commit, which step 3 performs. That convention is the sole normative source
   for this phase; do not restate or alter its procedure here.
3. Run `bin/sotp git add-all` and invoke the `commit` workflow for the ADR baseline. Its
   success closes the Phase 0 adjudication boundary; the input box is frozen from here to
   track end. Mark Phase 0 `completed` only after that guarded commit succeeds; then
   proceed to Phase 1. The commit workflow owns its message and other commit preconditions.

### Phase 1 loop: spec-design workflow

The phase workflows prepare their configured briefings and enter Phase 1–3 exclusively through
`bin/sotp phase enter spec-design`, `bin/sotp phase enter type-design`, and
`bin/sotp phase enter impl-plan`, respectively. Do not dispatch a phase writer outside its
corresponding phase entry.

1. Invoke the `spec-design` workflow (`.harness/workflows/track/spec-design.md`).
2. Use the capability's signal summary (blue / yellow / red counts + 🔴 element ids with cited
   ADR paths) without reopening `spec.json`.
3. Apply the loop rule:
   - **🔵**: run `bin/sotp ref-verify run` (semantic review of Chain ①). On `[BLOCKED]`, treat
     as 🔴 (route to `adr-editor` or re-invoke the `spec-design` workflow depending on which
     side is wrong).
     On `[ESCALATE]`, report to user and stop. On `[OK]`, mark Phase 1 `completed` and proceed
     to Phase 2.
   - **🟡**: re-invoke the `spec-design` workflow to resolve the yellow, then run the Phase 1
     🔵 path.
     Do not enter Phase 2 until that path completes. Count against `max_retry`; on overflow,
     stop and present the latest signal to the user.
   - **🔴**: escalate through the delta-candidate lane (the input box is frozen — no
     in-place semantic edit, regardless of the target ADR's pre/post-merge state):
     a. Identify the target ADR path and decision from the 🔴 element.
     b. Dispatch `adr-editor` to author (or revise) a delta candidate under
        `knowledge/adr/` with non-user grounds, declaring any supersedes / refines targets
        in the draft body per
        `.harness/policies/pre-track-adr-authoring.md#In-track 意味変更の裁定権`.
        Briefing must include the 🔴 element(s), the originating signal verbatim, and the
        constraint "edit working tree only; do not commit, do not snapshot".
     c. Dispatch `adr-diagnoser` for the delta admission judgment (three-way). On admit
        (independent / clarifying) or modification-proposal, the candidate enters the
        delta box and stays 🟡 until the merge-stage user adjudication. On bounce, have
        adr-editor remove the candidate and route the presented resolution back to the
        originating element (typically a spec-side fix).
     d. After admission (which changes the adr scope), re-converge the ADR side before the re-dispatch: run `bin/sotp signal check-adr-user --gate commit`, then run the `review` workflow's single-scope re-entry round for the adr scope only (`.harness/workflows/track/review.md` §Single-scope re-entry round) to `zero_findings`; do not invoke `review-fix-lead` directly or launch the all-required-scopes review wave here. Run the ADR signal check again so it reflects any review-driven repair; repeat until both checks satisfy the re-entry prerequisite in `.harness/policies/sot-reentry-sequencing.md`. Downstream scopes stay halted until their own upstream re-converges. Then re-invoke the `spec-design` workflow so the failing element(s) cite the admitted draft and Chain ① regenerates — what rides to the merge gate is the draft's chain ⓪ 🟡, never a standing Chain ① 🔴. After a bounce, re-invoke the `spec-design` workflow with the resolution instead (a bounce leaves the ADR unchanged, so no adr re-convergence is needed). Count either retry against `max_retry`; on overflow, stop.
     e. Non-semantic fixes to an input-box ADR (typo / reference path) follow the apply-then-classify lane instead: adr-editor applies, adr-diagnoser classifies, and only a non-semantic verdict is retained and restamped kind: non-semantic-fix. After a retained restamp, re-converge the ADR side through the same signal check and adr-scoped `review` workflow lifecycle before re-invoking the `spec-design` workflow, per `.harness/policies/sot-reentry-sequencing.md`.

### Phase 2 loop: type-design workflow

1. Invoke the `type-design` workflow (`.harness/workflows/track/type-design.md`).
2. Use the per-layer signal summary without reopening the catalogue files.
3. Apply the loop rule:
   - **🔵 all layers**: run `bin/sotp ref-verify run` (semantic review covering Chain ① and
     Chain ②). On `[BLOCKED]`, route by owning side: catalogue-side → re-invoke the
     `type-design` workflow,
     re-run semantic review; spec-side → treat as Phase 2 🔴 (see below). On `[ESCALATE]`,
     report to user and stop. On `[OK]`, mark Phase 2 `completed` and proceed to Phase 3.
   - **🟡**: re-invoke the `type-design` workflow to resolve the yellow, then run the Phase 2
     🔵 path.
     Do not enter Phase 3 until that path completes. Count against `max_retry`; on overflow,
     stop and present the latest signal to the user.
   - **🔴**:
     a. Re-invoke the `spec-design` workflow (Phase 2 🔴 typically indicates spec needs refinement).
     b. Re-evaluate Phase 1 gate. If Phase 1 also 🔴, escalate via Phase 1 ADR loop.
     c. Re-converge the spec before the re-dispatch. The re-entry prerequisite requires the `spec_adr` reference signal to satisfy `.harness/config/signal-gates.json`'s applicable specification — under a strict designation a 🟡 also blocks, so the plan workflow's general 🟡-advance rule does not apply to this re-entry: resolve the yellow (typically by re-invoking the `spec-design` workflow) before proceeding. Once the signal satisfies the gate specification, run the `review` workflow's single-scope re-entry round for the spec scope only (`.harness/workflows/track/review.md` §Single-scope re-entry round) to `zero_findings`; do not invoke `review-fix-lead` directly or launch the all-required-scopes review wave here. Re-evaluate the Phase 1 signal gate again so it reflects any review-driven repair; if it no longer satisfies the gate specification, return to the Phase 1 loop and re-run the scoped review after its repair. The spec's semantic-verification element asks only for Chain-①-relevant findings, per `.harness/policies/sot-reentry-sequencing.md`: confirm via the chain-scoped read `bin/sotp ref-verify results --chain 1` that no Chain ① finding remains unresolved — Chain ② findings and enumeration failures caused by the catalogues pending regeneration by the upcoming `type-design` re-run do not participate in this judgment (a full `bin/sotp ref-verify run` may abort on such stale pairs; that abort is not a spec-convergence failure). Then re-invoke the `type-design` workflow, and immediately after its regeneration run the full `bin/sotp ref-verify run` (the Phase 2 🔵 step) before any further descent; any Chain ① finding it surfaces routes back per the immediate bounce-back rule. The types scope otherwise resumes only after the spec re-converges.
     d. The Phase 2 retry counter is independent of Phase 1's. Count against `max_retry`.

### Phase 3 loop: impl-plan workflow

1. Invoke the `impl-plan` workflow (`.harness/workflows/track/impl-plan.md`).
2. Use BOTH binary gate summaries (each OK / ERROR): the task-coverage gate
   (`bin/sotp verify plan-artifact-refs`) and the batch-plan structural gate
   (`bin/sotp batch-plan check`).
3. Apply the loop rule:
   - **Both OK**: mark Phase 3 `completed` and proceed to Termination.
   - **Either ERROR**: re-invoke the `impl-plan` workflow with the failing gate's output.
     Count against `max_retry`; on overflow, stop and present the latest error to the user.
     Coverage OK with batch-plan ERROR does NOT complete Phase 3.

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
| 3 | `track/items/<id>/impl-plan.json` + `task-coverage.json` + `task-contract.json` + `batch-plan.json` | `impl-planner` capability |

The orchestrator does not directly write `knowledge/adr/*.md`, `spec.json`,
`<layer>-types.json`, `impl-plan.json`, `task-coverage.json`, `task-contract.json`, or
`batch-plan.json`. Each artifact's writer capability owns its file end-to-end.

### Sub-workflow briefing rules (no design prescription)

When composing a briefing for a writer capability, the briefing body MUST contain only:

- Problem statement / trigger (what was observed, what symptom)
- Context references (CLI summaries, target file paths, relevant ADRs / convention paths supplied
  in the briefing, and prior edits in the working tree)
- Interaction contract (what the capability should report back, and operational constraints)

The briefing body MUST NOT contain design prescription — anything that pre-solves the
capability's domain expert judgment. Each capability is the domain expert for its owned artifact.

## Gates

| Gate style | Phases | Signals |
|------------|--------|---------|
| ADR-baseline review + user adjudication + commit | Phase 0 after `init` | `.harness/policies/pre-track-adr-authoring.md` の手順に従い → commit |
| SoT Chain signal (🔵🟡🔴) | Phase 1, Phase 2 | Blue = pass, Yellow = recover before downstream entry, Red = escalate |
| Binary check (OK / ERROR) | Phase 0 `init`, Phase 3 | OK = pass, ERROR = re-invoke matching workflow or stop |

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
- `track/items/<id>/impl-plan.json` + `task-coverage.json` + `task-contract.json` + `batch-plan.json` (Phase 3)
- Per-phase gate results (🔵🟡🔴 / OK / ERROR) and final `max_retry` counters
- Back-and-forth edits that occurred (target artifact and its writer)
- Unexpected ADR working-tree divergence triage (if any), and admitted delta drafts left 🟡 for
  merge-stage adjudication
- The guarded Phase 0 ADR-baseline commit result
