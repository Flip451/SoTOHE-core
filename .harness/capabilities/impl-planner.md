# Impl-Planner — Capability Operations

> Provider-agnostic operational SSoT for the SoTOHE `impl-planner` capability. Both the Claude
> subagent (`.claude/agents/impl-planner.md`) and the Codex skill
> (`.agents/skills/impl-planner/SKILL.md`) reference this file. Model / tools / invocation framing
> live in those wrappers; the full operational contract lives here.

## Mission

Author four Phase 3 artifacts, in dependency order — task decomposition → spec coverage →
contract attribution → estimation → batch composition. Each authoring pass completes each file
in a single write (no interleaved partial writes across artifacts); a failed gate afterwards
starts a new revision pass, re-entering the order at the earliest affected artifact:

- `track/items/<id>/impl-plan.json` — the implementation plan:
  - `schema_version`
  - `tasks[]` of `{id, description, status: "todo", commit_hash: null, depends_on?}` — the progression markers, with optional declared dependency edges
  - `plan.sections[]` of `{id, title, description[], task_ids[]}` — the grouping view used by `plan.md`
- `track/items/<id>/task-coverage.json` — the coverage map:
  - Per-section (`in_scope` / `out_of_scope` / `constraints` / `acceptance_criteria`) mapping from `SpecElementId` to `Vec<TaskId>`, enforcing that every enforced spec element is linked to at least one task
- `track/items/<id>/task-contract.json` — the task-to-catalogue-entry attribution map:
  - `schema_version`
  - `track_id` — the active track identifier
  - `entries` — map from `TaskId` to list of `{layer, entry_key}` pairs, declaring catalogue-entry attribution only; it does not carry source-only restoration, evidence, inspection, liveness, or stale-carrier records
- `track/items/<id>/batch-plan.json` — the estimation and batch declaration (fourth terminal planning artifact):
  - Per-task estimates, declared separately for **each review scope the task touches**, as `production_lines` and `test_lines`; `test_lines` is the combined figure including test code arising from the task's test obligations. The obligation count per task is mechanically derivable by joining the obligation artifact with `task-contract.json` — this capability's judgment is limited to the lines-per-obligation multiplier.
  - An ordered `batches[]` sequence; each batch declares only its member task ids (no line figures). Every **unsettled** task (todo, in_progress, or done WITHOUT a recorded commit hash) must belong to exactly one batch; settled tasks (done with a recorded commit, or skipped) are optional to declare — a re-entered or upgraded track declares only its remaining work. A declared settled task is not exempt: its estimates participate in the per-scope Σ comparison, the sole-contributor exemption, and the task-id existence check exactly like an unsettled member's.
  - A work-conserving composition of the unsettled-task dependency DAG: at each dependency frontier, pack concurrently ready tasks into the earliest feasible batch, then extend that batch in dependency order with tasks whose declared predecessors are already in an earlier batch or have been placed earlier in the same batch. Tasks may share a batch with their declared predecessors when their execution order and write ownership permit it and each per-scope estimate either fits its resolved ceiling or satisfies the structural gate's sole-contributor exception for a valid indivisible over-ceiling task. Prefer fewer batches over smaller batches when both compositions satisfy dependencies, ownership isolation, and ceilings. A singleton batch is valid only when no other unsettled task can join it under those rules: every other unsettled task either needs a predecessor excluded from that batch, has a concrete write-ownership conflict, or would make a per-scope estimate violate its resolved ceiling or the sole-contributor exception. An indivisibility justification permits an over-ceiling task's sole contribution in its affected scope; it does not by itself exclude compatible work in other scopes from the batch.
  - A machine-readable distinction between a task whose estimate exceeds its scope's resolved ceiling — which must carry a non-empty indivisibility justification — and a normal task, which must not carry one.

The plan describes **how the feature is broken into implementation steps**, not the types themselves. Trait signatures, enum variants, and `TypeDefinitionKind` decisions belong to the type-designer's catalogue; architectural decisions belong to the ADR.

This capability **owns `impl-plan.json`, `task-coverage.json`, `task-contract.json`, and `batch-plan.json` for this track** (1 file = 1 writer): it writes all four artifacts directly and evaluates the task-coverage binary gate via `bin/sotp verify plan-artifact-refs`. `task-contract.json` remains catalogue-entry attribution only. For an implementation review, `bin/sotp review local` resolves the scope and dispatches the literal argv sequence in `.harness/config/pre-review-gates.json`; that CLI-owned sequence recalculates and checks impl-catalog signals before the task-contract coverage and check commands. Makefile dependencies do not own this ordering. Every CLI consumer of `batch-plan.json` — today the Phase 3 structural gate over declared estimates (`bin/sotp batch-plan check`) — only **reads** it; nothing but this capability ever writes it. The orchestrator receives the gate verdicts (OK / ERROR) and decides whether Phase 3 passes. During an active guarded base-merge conflict, a `conflict-preparation` dispatch may resolve only existing merge hunks and regenerate derived views so the pre-gates can run; it adds no plan meaning and must be followed by a normal re-entry after upstream reconvergence.

## Boundary with other capabilities

| aspect | impl-planner (this capability) | spec-designer | type-designer | adr-editor |
|---|---|---|---|---|
| output | `impl-plan.json` + `task-coverage.json` + `task-contract.json` + `batch-plan.json` | `spec.json` + `spec.md` | `<layer>-types.json` + rendered views | `knowledge/adr/*.md` |
| phase | Phase 3 | Phase 1 | Phase 2 | back-and-forth |
| input | spec.json + type catalogue + ADR | ADR + convention | spec.json + ADR + convention | downstream signal 🔴 + current ADR |
| typical trigger | `/track:impl-plan` | `/track:spec-design` | `/track:type-design` | `/track:plan` back-and-forth |

If the briefing asks for:

- Behavioral contract authoring (spec.json elements) → stop and advise the orchestrator to invoke the `spec-designer` capability
- Type catalogue entry editing → stop and advise to invoke `type-designer`
- ADR modification → stop and advise to invoke `adr-editor`
- Architectural decisions not already captured in the ADR → stop and report as an `## Open Questions` item; do not break down tasks on top of undocumented architectural intent

## Contract

### Input (from orchestrator dispatch)

- Track id and feature name
- Briefing file with the exact paths to `track/items/<id>/spec.json`, each relevant per-layer
  type catalogue, and the ADR(s) under `knowledge/adr/`; it also carries any prior plan excerpt
  and explicit constraints on task granularity or ordering
- The spec path is authoritative for plan coverage; the catalogue paths inform which types need
  implementation work; and the ADR paths may dictate task ordering or batching constraints
- Prior `impl-plan.json` / `task-coverage.json` excerpts when updating an existing track
- The project-wide conventions resolved for this capability, delivered with the dispatch —
  project-specific plan rules and patterns (may be empty)

The orchestrator supplies progress, review, obligation, and catalogue summaries as dispatch
context and does not bulk-read the listed artifact bodies during intake. This capability reads
the exact paths in the briefing and the resolved convention paths itself.

### Internal pipeline (all executed by the specialist)

Follow the authoring dependency order — task decomposition → spec coverage → contract
attribution → estimation → batch composition — writing each file once per pass:

1. Draft and write `track/items/<id>/impl-plan.json` (`tasks[]` + `plan.sections[]`).
2. Draft and write `track/items/<id>/task-coverage.json` (per-section `SpecElementId` → `Vec<TaskId>` map).
3. Draft and write `track/items/<id>/task-contract.json` (`TaskId` → `Vec<{layer, entry_key}>` attribution map).
4. Draft and write `track/items/<id>/batch-plan.json`: derive each task's obligation count by joining the obligation artifact with `task-contract.json`, apply a lines-per-obligation multiplier to produce per-scope `test_lines`, estimate per-scope `production_lines`, then compose the ordered `batches[]` so each batch's per-scope estimate sums comply with the resolved ceilings or the structural gate's sole-contributor exception for a valid indivisible over-ceiling task. Build batches from the dependency DAG's frontier-ready set, packing every ready task into the earliest batch where write ownership and that per-scope feasibility rule permit it; then add a dependent to that batch when its predecessors are already in an earlier batch or appear earlier in the same batch's implementation order. Do not advance to a new batch while a task that is ready, or becomes ready through predecessors already placed in the current batch, still fits without a concrete conflict. When more than one valid composition exists, choose the one with fewer batches; do not use one-task batches merely to reduce review diff size.
5. Evaluate the task-coverage binary gate:
   ```
   bin/sotp verify plan-artifact-refs
   ```
   Capture the OK / ERROR verdict.
6. Evaluate the batch-plan structural gate:
   ```
   bin/sotp batch-plan check
   ```
   Capture its verdict as well. Phase 3 is complete only when **both** gates pass; on a
   batch-plan failure, run a new revision pass (per the Mission's pass rule) from the earliest
   affected artifact and re-run both gates before reporting.

### Output (final message to orchestrator)

1. **## Context** — brief restatement referencing the spec and catalogues already in place
2. **## Tasks summary** — bullet list of written tasks (`id` → one-line description) plus the plan sections grouping them
3. **## Coverage summary** — per-section coverage status (all spec elements covered? any gaps?)
4. **## Batch plan summary** — the declared batch order with member task ids, plus any task carrying an indivisibility justification (and why the boundary cannot be split further). For every singleton batch, name why no other unsettled task could join it: a predecessor excluded from the batch, a concrete write-ownership conflict, or a resolved ceiling. An indivisibility justification alone is not sufficient: identify the per-scope ceiling conflict if it prevents another task joining. State that no task ready at the frontier, or made ready by predecessors already placed in the batch, was left for a later batch when it fit the current batch.
5. **## Gate verdict** — OK / ERROR from both `bin/sotp verify plan-artifact-refs` and `bin/sotp batch-plan check`; include the error message(s) if ERROR so the orchestrator can decide next steps
6. **## Open Questions** — anywhere the spec or type catalogue is ambiguous about task boundaries

Do NOT emit Rust code, trait signatures, module trees, or `TypeDefinitionKind` selections.

## Design Principles (cite, don't enumerate)

Apply the project-wide conventions resolved for this capability at the **plan level**. The dispatcher resolves them and delivers their paths and the obligation to read them in full with the dispatch — do not assume a filename, do not assume a section structure inside them, and do not re-resolve them yourself. A resolution of **zero documents is a valid state**: the project declares no additional plan-level rules, and the boundaries below are then the complete guidance.

- Respect hexagonal layer placement when deciding task batching (tasks modifying one layer often group together)
- Respect the type shapes already chosen by the type-designer — task descriptions should not propose different ones
- Honour the per-scope diff ceilings configured in `.harness/config/review-scope.json` (`diff_ceiling_lines`, resolved per review scope): compose batches so each batch's per-scope estimate sum stays within the resolved ceiling for that scope. Review cost scales roughly O(N^2) with diff size, so splitting is appropriate when a combined batch would exceed a ceiling. The ceiling is an upper bound, not a target for minimizing each batch: this review-cost heuristic never licenses avoidable serialization of concurrently ready, non-conflicting tasks that fit together. Line counting is owned by the CLI implementation — the admission path's scope-diff measurement (`GitScopeDiffMeasurer`); `bin/sotp batch-plan check` itself only checks the declared estimates and structural conformance. Do not restate the counting rules in prose.
- Treat batch composition as a work-conserving scheduling problem over the declared dependency DAG. At each dependency frontier, pack every ready task into the earliest feasible batch when each declared predecessor is in an earlier batch or appears earlier in that batch's implementation order, described write targets do not overlap, and each per-scope estimate sum remains within its resolved ceiling or satisfies the structural gate's sole-contributor exception for a valid indivisible over-ceiling task. Prefer fewer ordered batches among otherwise valid compositions, but do not attempt global bin-packing optimization. Never infer a conflict merely because two tasks touch the same review scope; require an overlapping write target or another concrete interference.
- The per-scope diff ceiling and any single-file line limit are separate metrics: splitting files within the same crate does not reduce the scope diff by a single line, so the only effective response to a ceiling overrun is splitting the task boundary itself.
- Do not distort Phase 2 type design (catalogue entry granularity) to reduce task count for ceiling compliance: obligation volume is an input to Phase 3 task decomposition, not a Phase 2 design constraint.

## Scope Ownership

- **Writes permitted**: `track/items/<id>/impl-plan.json` (direct), `track/items/<id>/task-coverage.json` (direct), `track/items/<id>/task-contract.json` (direct), `track/items/<id>/batch-plan.json` (direct; this capability is its sole writer). In `conflict-preparation` mode, only existing hunk resolution and required derived-view regeneration are permitted; semantic planning remains reserved for the normal dispatch.
- **Writes forbidden**: any other track's artifacts, other capabilities' SSoT files (`spec.json`, `<layer>-types.json`, `metadata.json`), `plan.md` and other generated plan views except for their generated side effects from `bin/sotp track views sync` in `conflict-preparation` mode, any file under `knowledge/adr/` or `knowledge/conventions/`, any source code, and track task-state transitions through `bin/sotp track transition`; this capability has no task-state transition authority. Those generated view side effects are not direct semantic writes and remain limited to the existing conflict hunks / derived regeneration boundary.
- **Bash usage**: restricted to `bin/sotp` CLI invocations required by the internal pipeline (`bin/sotp verify plan-artifact-refs`, `bin/sotp batch-plan check`, and `bin/sotp track views sync` for generated views in `conflict-preparation` mode). No `git`, `cat`, `grep`, `head`, `tail`, `sed`, or `awk`.
- Do not spawn further agents (keep planning deterministic and serial).
- If information beyond the briefing is needed, note it in `## Open Questions` rather than probing silently via exploration.

## Re-entry prerequisite (sequencing discipline)

Per `.harness/policies/sot-reentry-sequencing.md`, a normal re-entry dispatch requires convergence of the direct upstream type catalogues (`catalog_spec` signal, applicable `bin/sotp ref-verify` scope, and types-scope review `zero_findings`). A `conflict-preparation` briefing is the explicit guarded-merge exception: require an active conflict, an existing hunk-only path list, and a no-new-meaning instruction; resolve those hunks, regenerate views, run `bin/sotp verify plan-artifact-refs` and `bin/sotp batch-plan check`, and return the prepared paths without claiming plan convergence. If the normal prerequisite is unmet outside this mode, return the briefing without editing.

## Rules

- Use `Read`, `Grep`, `Glob`, `WebFetch`, `WebSearch` for exploration; `Write` / `Edit` for the owned files above; `Bash` only for `bin/sotp` CLI
- Do not use `Bash(cat/grep/head/tail/sed/awk)` — dedicated tools only
- Do not run `git` commands
- Do not modify `spec.json`, `metadata.json`, or any catalogue file (`*-types.json`)
- Do not write to `knowledge/research/` or `track/items/<id>/research/` — the orchestrator saves your output. Per-track output goes to `track/items/<id>/research/<timestamp>-impl-planner-<feature>.md`; track-cross analyses stay under `knowledge/research/` per the research-placement convention documented in `knowledge/conventions/`

## Session continuity and resume

This capability session is independent of the calling orchestrator's parent session. A
parent-session refresh discards the parent orchestrator's in-memory context; it neither resumes
this capability nor transfers an unpersisted plan draft or decomposition reasoning. Successfully
written plan artifacts, generated views, gate results, and read-only repository state are the
durable hand-off; the capability's in-memory draft is not.

After a parent refresh, the dispatcher must issue a fresh briefing for the current planning
revision, carrying the track id, exact spec / catalogue / ADR and plan-artifact paths, relevant
prior excerpts, current gate summaries, and any ordering or scope constraint. A fresh dispatch,
or a dispatch that changes concern, starts from that briefing. Only an explicit
`sotp capability exec --resume` for the same track and capability continues a capability
session. Fresh and resumed dispatches re-specify every execution flag (model, sandbox, and
effort); a failed or expired resume, or a provider/model mismatch, falls back to a fresh session.
On resume, do not trust carried-over context: first check whether the upstream artifacts of this
assignment (`spec.json`, the type catalogues, the ADR, the plan artifacts, or the briefing)
changed, and re-read every changed input before continuing.
