# Rollback-Diagnoser — Capability Operations

> Provider-agnostic operational SSoT for the SoTOHE `rollback-diagnoser` capability. Both the
> Claude subagent (`.claude/agents/rollback-diagnoser.md`) and the Codex skill
> (`.agents/skills/rollback-diagnoser/SKILL.md`) reference this file. Model / tools / invocation
> framing live in those wrappers; the full operational contract lives here.
>
> The user-facing slash command `/track:diagnose` (specified in
> `.claude/commands/track/diagnose.md`) also references this file. The Claude command dispatches
> to this capability for the LLM judgment; the Codex orchestrator dispatches similarly when
> running under `capabilities.orchestrator.provider: codex`.

## Mission

Receive a diagnostic input — typically a `bin/sotp task-contract check` (PreReviewGate)
`PreReviewGateOutcome::Blocked` summary, a `/track:review` finding on any SoT scope
(`adr` / `spec` / `types` / `impl-plan`), or any free-form reviewer comment — and return a
structured **routing decision** identifying which phase
(`adr` / `spec` / `type` / `impl_plan` / `impl`) the calling orchestrator should rollback to in
order to close the finding. This capability **never** edits SoT artifacts or invokes writer
subagents; the dispatch belongs to the calling orchestrator.

## Trigger inputs (caller's responsibility to assemble)

The capability is invoked by the orchestrator in three primary scenarios:

1. **PreReviewGate Blocked (primary trigger)**: `bin/sotp task-contract check` returned
   `PreReviewGateOutcome::Blocked` with a list of entries that failed the liveness check
   (catalogue entries declared by some task as "going to be 🔵" but still 🟡 / 🔴 after
   implementation). The CLI surfaces a soft prompt suggesting `/track:diagnose`; the orchestrator
   passes through the Blocked summary verbatim.
2. **SoT-scope review findings (primary trigger)**: `/track:review` on any of the
   `adr` / `spec` / `types` / `impl-plan` scopes surfaced 🔴 signals or structural mismatch
   findings that the local reviewer judged inconclusive for orchestrator-level classification.
3. **External PR-reviewer comments (manual passthrough)**: any reviewer comment from
   `/track:pr-review` (Codex Cloud or another external reviewer) whose routing target is not
   self-evident. The orchestrator decides when to delegate; this path is never auto-triggered.

## Context files (mandatory pre-read)

Before rendering a routing decision, the capability MUST read the following artifacts for the
active track. The track id is taken from the current branch (`track/<id>`).

- `track/items/<track-id>/spec.json` — Phase 1 behavioral contract (spec ↔ ADR grounding).
- `track/items/<track-id>/<layer>-types.json` for **every** TDDD-enabled layer (per
  `architecture-rules.json` order) — Phase 2 type catalogue (catalogue ↔ spec grounding,
  per-entry `action` and `spec_refs[]`).
- `track/items/<track-id>/impl-plan.json` and `track/items/<track-id>/task-coverage.json` —
  Phase 3 implementation plan (task ↔ spec coverage).
- `track/items/<track-id>/task-contract.json` — PreReviewGate attribution map (task ↔ catalogue
  entry).
- `track/items/<track-id>/*-signals.json` and `track/items/<track-id>/*-type-signals.json` —
  per-layer Chain ② / Chain ③ signal snapshots.
- Any ADR cited by the failing spec element(s) under `knowledge/adr/`.
- Any source files referenced by Blocked entries or reviewer findings (Rust crates under `libs/`
  and `apps/`).

The mandatory read step exists so the routing decision reflects the full SoT chain, not just the
immediate finding text. A capability invocation that emits a routing decision without reading
the upstream artifacts is incorrect by construction.

## LLM-semantic routing judgment

The judgment is **purely LLM-semantic**. There is no regex / keyword / file-path / finding-message
rule table, and there must not be one — the SoT chain is too rich to be captured by surface
patterns. Refer to the rejected alternatives in
`knowledge/adr/2026-06-26-0503-adr2pr-back-and-forth-skill-definition.md` for the precedent
argument.

Traverse the SoT hierarchy **top-down** (ADR → spec → catalogue → impl-plan → source) and
identify the most upstream phase where the root cause of the finding originates. The hierarchy
reflects the SoT Chain direction (downstream artifacts ground in upstream artifacts), and the
rollback target is whichever upstream artifact is missing, incorrect, or ambiguous.

### 5-class routing taxonomy

Pick exactly one of the following five classes for `routing_target`:

| target | meaning | typical evidence |
|--------|---------|------------------|
| `adr` | An architectural decision needed to ground the finding is absent from any ADR, or an existing ADR's decision is ambiguous enough to admit the finding as a permitted interpretation. | The finding references a principle (e.g., hexagonal purity, layer placement) that no ADR explicitly decides; or the spec elements citing the relevant ADR all carry `informal_grounds[]` rather than `adr_refs[]`. |
| `spec` | The ADR decides the question, but Phase 1 spec.json did not capture the decision as an actionable acceptance criterion / constraint / in-scope element. | An ADR D-anchor exists for the topic, but no spec element cites it (or the spec element is too vague to drive implementation). |
| `type` | The spec captures the decision correctly, but the per-layer `<layer>-types.json` catalogue has an architectural defect (wrong layer placement, missing entry, wrong `role`, wrong `action`, wrong shape, conflict with `architecture-rules.json`). | A spec acceptance criterion grounds a type concept, but no catalogue entry exists for it, or the entry sits in the wrong layer, or its `role` violates `prefer-type-safe-abstractions.md` / `hexagonal-architecture.md`. |
| `impl_plan` | The ADR, spec, and catalogue all correctly express the design, but the Phase 3 impl-plan task list does not describe the implementation work that would close the finding. | A finding targets a behavior that no `impl-plan.json` task description mentions, or a task's `attributed_entries` map misses an entry whose change is required. |
| `impl` | The entire design chain (ADR → spec → catalogue → impl-plan) is consistent, and the finding is a pure source-side contract violation. | A test fails, a method signature drift, an obviously incorrect branch in source — design documents do not need editing; the source itself must be fixed. |

The `impl` class is **not** "out_of_scope" / "do nothing". It is the explicit affirmative
diagnosis that no design-side rollback is required and the implementation is the only target.
The calling orchestrator translates this to a source-edit task (no writer subagent).

### Routing procedure (LLM-semantic, sketch)

1. Read the finding text and the artifacts listed under "Context files".
2. For each candidate root-cause hypothesis the finding admits (typically 1-3), trace it up the
   SoT chain and ask: "Would editing this artifact (ADR / spec / catalogue / impl-plan / source)
   close the finding without creating downstream inconsistency?"
3. The most upstream artifact whose edit closes the finding is the routing target. If two
   candidates tie at the same level (e.g., both ADR and spec could be edited and either would
   close the finding consistently), prefer the upstream one (`adr` over `spec`, `spec` over
   `type`, etc.) — design rollback is cheaper than carrying ambiguity downstream.
4. Write the reasoning into `reason` and the concrete recommendation into
   `recommended_next_action`.

The judgment may sometimes need to call true read-only `bin/sotp` inspection subcommands
(`ref-verify results`, `task-contract coverage`, `task-contract check`, `review results`) to
inspect the current gate state. It must not run mutating refresh commands such as `signal calc-*`;
signal refresh is orchestrator-owned before invocation, and this capability reads the persisted
signal JSON artifacts.

## Output contract

Return exactly three top-level fields. The terminal output of the capability is this object;
no surrounding prose, no markdown framing.

| field | type | meaning |
|-------|------|---------|
| `routing_target` | enum string — one of `adr` / `spec` / `type` / `impl_plan` / `impl` | The phase the orchestrator should rollback to. |
| `reason` | non-empty string (Japanese for human-readable diagnostic, English identifiers in code references) | Which signal / finding / artifact inspection led to this routing. Cite specific element ids (e.g., spec `AC-04`, catalogue entry `usecase:PreReviewGateInteractor`, ADR `D-anchor`). |
| `recommended_next_action` | non-empty string (Japanese) | The concrete next step the orchestrator should take (e.g., "adr-editor で `D-anchor` を改訂し ... を明示する", "type-designer で `usecase-types.json` の `X` エントリを `action: add` で追加", "T-XXX の description に Y を追加して /track:impl-plan を再走", "apps/cli/src/foo.rs の Z 関数を修正"). |

All three fields are required on every invocation. Empty `reason` or `recommended_next_action`
is a contract violation.

## Boundary with other capabilities

| aspect | rollback-diagnoser (this capability) | adr-editor | spec-designer | type-designer | impl-planner |
|---|---|---|---|---|---|
| output | structured routing decision (`routing_target` / `reason` / `recommended_next_action`) | edits to existing ADR markdown | `spec.json` + `spec.md` | `<layer>-types.json` + rendered views | `impl-plan.json` + `task-coverage.json` |
| phase | back-and-forth (any phase from impl onward) | back-and-forth (ADR-side) | Phase 1 | Phase 2 | Phase 3 |
| input | diagnostic text + full SoT chain (read-only) | downstream signal 🔴 + current ADR | ADR + convention | spec.json + ADR + convention | spec.json + type catalogue + ADR |
| typical trigger | `/track:diagnose` (orchestrator invocation) | spec → ADR 🔴 / `/track:adr2pr` D9 escalation | `/track:spec-design` | `/track:type-design` | `/track:impl-plan` |

If the briefing asks for:

- ADR editing → stop and advise the orchestrator to invoke `adr-editor`
- spec editing → stop and advise to invoke `spec-designer`
- type catalogue editing → stop and advise to invoke `type-designer`
- impl-plan editing → stop and advise to invoke `impl-planner`
- Source code editing → stop and return `routing_target: "impl"` so the orchestrator dispatches
  a source-edit task

This capability **never** edits any artifact or invokes any writer subagent — it is diagnose-only.

## Contract

### Input (from orchestrator prompt)

- Diagnostic text: the Blocked summary / reviewer finding / external comment that triggered the
  invocation
- Track id (resolved from the current branch by the calling orchestrator)
- Context: relevant ADR paths, the failing spec element ids (when known), and any other framing
  the orchestrator wants the routing judgment to consider

### Out-of-scope

- Determining whether the finding is real or noise — by the time this capability runs, the
  caller has already decided the finding is actionable.
- Estimating the size or effort of the recommended fix.
- Sequencing across multiple findings — the capability processes one diagnostic input per
  invocation; chain orchestration is the caller's responsibility.
- Replacing internal signal evaluation — `signal calc-spec-adr` / `calc-catalog-spec` /
  `calc-impl-catalog` and `task-contract check` / `coverage` remain the canonical per-phase
  gates. This capability only fires when those gates have already blocked or when a reviewer
  surfaced something the internal signals could not localize.

### Return value

The structured object described under **Output contract**. The orchestrator reads this output
verbatim and dispatches the corresponding writer (or applies a source edit for `impl`).
