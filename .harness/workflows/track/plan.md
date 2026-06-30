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
a downstream gate fails. Provider routing is resolved via `.harness/config/agent-profiles.json`.

Sub-workflows used:

- `.harness/workflows/track/init.md` (Phase 0)
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
- **`track/tech-stack.md`** — must be free of blocking `TODO:` markers before implementation.
- **Current branch** — must be compatible with the operation: `main` for a new track,
  `track/<id>` for a track already initialized.
- **`.harness/config/agent-profiles.json`** — must be readable for capability routing.

## Sequence

### Preamble: register the phase chain as tasks

Before executing the state machine, register the following items as a task list so progress
stays visible across phases and back-and-forth loops:

1. Phase 0 — invoke `init` workflow
2. Phase 1 loop — invoke `spec-design` workflow, evaluate spec → ADR signal, escalate on 🔴
3. Phase 2 loop — invoke `type-design` workflow, evaluate type → spec signal per layer, escalate on 🔴
4. Phase 3 loop — invoke `impl-plan` workflow, evaluate task-coverage gate, re-invoke on ERROR
5. Termination — ADR working-tree diff presentation and user decision

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
| 0 | `init` | main (direct) | metadata identity schema (OK / ERROR) |
| 1 | `spec-design` | spec-designer | spec → ADR signal (🔵🟡🔴) |
| 2 | `type-design` | type-designer | type → spec signal, per layer (🔵🟡🔴) |
| 3 | `impl-plan` | impl-planner | task-coverage binary gate (OK / ERROR) |

### Phase 0: init workflow

Invoke the `init` workflow (`.harness/workflows/track/init.md`) with the feature name.
On ERROR, stop and report. On OK, mark Phase 0 `completed` and proceed to Phase 1.

### Phase 1 loop: spec-design workflow

1. Invoke the `spec-design` workflow (`.harness/workflows/track/spec-design.md`).
2. Read the signal result (blue / yellow / red counts + 🔴 element ids with cited ADR paths).
3. Apply the loop rule:
   - **🔵**: run `bin/sotp ref-verify run` (semantic review of Chain ①). On `[BLOCKED]`, treat
     as 🔴 (route to `adr-editor` or re-invoke `spec-design` depending on which side is wrong).
     On `[ESCALATE]`, report to user and stop. On `[OK]`, mark Phase 1 `completed` and proceed
     to Phase 2.
   - **🟡**: log warning and proceed to Phase 2. Yellow must be resolved before merge.
   - **🔴**: escalate per ADR auto-edit criteria:
     a. Identify the target ADR path from the 🔴 element.
     b. If the ADR has commit history: invoke the `adr-editor` capability. Briefing must
        include the 🔴 element(s), ADR path, and the constraint "edit working tree only; do not
        commit inside the loop".
     c. If no commit history: pause for user — instruct them to commit the ADR first.
     d. After ADR edit, re-invoke `spec-design`. Count against `max_retry`; on overflow, stop.

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
     c. On Phase 1 🔵 or 🟡, re-invoke `type-design`.
     d. The Phase 2 retry counter is independent of Phase 1's. Count against `max_retry`.

### Phase 3 loop: impl-plan workflow

1. Invoke the `impl-plan` workflow (`.harness/workflows/track/impl-plan.md`).
2. Read the binary gate verdict (OK / ERROR).
3. Apply the loop rule:
   - **OK**: mark Phase 3 `completed` and proceed to Termination.
   - **ERROR**: re-invoke `impl-plan`. Count against `max_retry`; on overflow, stop and
     present the latest error to the user.

### Termination

After Phase 3 OK (or `max_retry` overflow anywhere):

1. Check whether the ADR working tree differs from HEAD.
2. If the diff is non-empty, present it to the user and ask for a decision:
   - **accept**: stage and commit the ADR alongside other artifacts
   - **revert**: discard the ADR working-tree changes
   - **manual edit**: pause for the user to refine further
   - **abort**: stop the workflow and leave the tree as-is
3. Mark the Termination task `completed`.

### Writer ownership

| Phase | Artifact | Writer |
|-------|----------|--------|
| Pre-track | `knowledge/adr/*.md` (initial) | user + `adr:add` |
| Pre-track | `knowledge/adr/*.md` (back-and-forth) | `adr-editor` capability (auto-invoked) |
| 0 | `track/items/<id>/metadata.json` | main (direct via `init` workflow) |
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
| SoT Chain signal (🔵🟡🔴) | Phase 1, Phase 2 | Blue = pass, Yellow = warn + proceed, Red = escalate |
| Binary check (OK / ERROR) | Phase 0, Phase 3 | OK = pass, ERROR = re-invoke or stop |

Pre-approval exceptions (outside the gate system — user is asked only on irreversible actions):
`git push` / `git commit`, external API calls (PR / issue creation), destructive filesystem
operations, environment-breaking changes. Artifact generation uses post-hoc review.

## Failure / recovery

- **No ADR**: stop and ask the user to author one before running this workflow.
- **Non-compatible branch**: report the branch and available options.
- **Phase N 🔴 after max_retry overflows**: stop and present options (continue with warnings,
  abort, manual edit).
- **adr-editor invoked on ADR without commit history**: pause for user to commit the ADR first,
  then resume.
- **`[ESCALATE]` from `ref-verify`**: report to user and stop. Do not retry.

## Outputs

- `track/items/<id>/metadata.json` (Phase 0)
- `track/items/<id>/spec.json` + `spec.md` (Phase 1)
- `track/items/<id>/<layer>-types.json` + views (Phase 2, per TDDD-enabled layer)
- `track/items/<id>/impl-plan.json` + `task-coverage.json` (Phase 3)
- Per-phase gate results (🔵🟡🔴 / OK / ERROR) and final `max_retry` counters
- Back-and-forth edits that occurred (target artifact and its writer)
- ADR working-tree diff against HEAD (if any) and user termination decision
- No commit is created by this workflow (commit is a separate caller decision)
