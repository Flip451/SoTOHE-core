---
name: track-diagnose
description: |
  One-shot diagnostic skill that runs when the impl-phase or later surfaces a structural
  inconsistency the internal signal pipeline cannot localize on its own. Triggers
  automatically when `bin/sotp task-contract check` (PreReviewGate) returns
  `PreReviewGateOutcome::Blocked`, and may also be invoked when step 6
  `/track:review` plan-artifacts findings or external PR-reviewer comments need
  back-and-forth routing. Reads the SoT chain (ADR → spec → catalogue → impl-plan →
  source), judges the most upstream phase where the root cause originates, and
  returns a structured routing decision the calling orchestrator dispatches. This
  skill never invokes a writer subagent itself.
metadata:
  short-description: Phase-rollback routing for impl-phase structural inconsistencies — returns routing_target + reason + recommended_next_action; orchestrator dispatches.
---

# `/track:diagnose` Backing Skill

`/track:diagnose` is a **one-shot** phase-rollback routing skill. It receives diagnostic
input (a PreReviewGate Blocked summary, a plan-artifacts reviewer finding, or any free-form
reviewer comment), reads the SoT chain artifacts, and emits a structured routing decision.
It does **not** apply a fix — the calling orchestrator owns writer dispatch.

The provider used to execute the LLM-semantic judgment is resolved from
`capabilities.rollback-diagnoser` in `.harness/config/agent-profiles.json`
(`provider` and `model` fields, same JSON structure as `spec-designer` / `type-designer` /
other writer / verifier capabilities).

## Trigger conditions

The skill fires on any of the following inputs. Multiple inputs may be combined in a
single invocation; the skill produces one routing decision per call.

1. **PreReviewGate Blocked (primary)**: `bin/sotp task-contract check` returned
   `PreReviewGateOutcome::Blocked` with a list of entries that failed the liveness check
   (catalogue entries declared by some task as "going to be 🔵" but still 🟡 / 🔴 after
   implementation). The Blocked stderr already includes a soft prompt suggesting this
   skill; the orchestrator should pass through the Blocked summary verbatim.
2. **plan-artifacts findings (primary)**: `/track:review` on the `plan-artifacts` scope
   surfaced 🔴 signals or structural mismatch findings that the local reviewer judged
   inconclusive for orchestrator-level classification.
3. **External PR-reviewer comments (manual passthrough)**: any reviewer comment from
   `/track:pr-review` (Codex Cloud or another external reviewer) whose routing target is
   not self-evident. This path is not automatically triggered; the orchestrator decides
   when to delegate.

## Context files (mandatory pre-read)

Before rendering a routing decision, the skill MUST read the following artifacts for the
active track. The track id is taken from the current branch (`track/<id>`).

- `track/items/<track-id>/spec.json` — Phase 1 behavioral contract (spec ↔ ADR grounding).
- `track/items/<track-id>/<layer>-types.json` for **every** TDDD-enabled layer (per
  `architecture-rules.json` order) — Phase 2 type catalogue (catalogue ↔ spec grounding,
  per-entry `action` and `spec_refs[]`).
- `track/items/<track-id>/impl-plan.json` and `track/items/<track-id>/task-coverage.json`
  — Phase 3 implementation plan (task ↔ spec coverage).
- `track/items/<track-id>/task-contract.json` — PreReviewGate attribution map (task ↔
  catalogue entry).
- `track/items/<track-id>/*-signals.json` and `track/items/<track-id>/*-type-signals.json`
  — per-layer Chain ② / Chain ③ signal snapshots.
- Any ADR cited by the failing spec element(s) under `knowledge/adr/`.
- Any source files referenced by Blocked entries or reviewer findings (Rust crates under
  `libs/` and `apps/`).

The mandatory read step exists so the routing decision reflects the full SoT chain, not
just the immediate finding text. A skill invocation that emits a routing decision without
reading the upstream artifacts is incorrect by construction.

## LLM-semantic routing judgment

The judgment is **purely LLM-semantic**. There is no regex / keyword / file-path /
finding-message rule table, and there must not be one — the SoT chain is too rich to be
captured by surface patterns (the same keyword can appear in multiple phases for unrelated
reasons; see ADR Rejected Alternatives for the precedent argument).

Traverse the SoT hierarchy **top-down** (ADR → spec → catalogue → impl-plan → source) and
identify the most upstream phase where the root cause of the finding originates. The
hierarchy reflects the SoT Chain direction (downstream artifacts ground in upstream
artifacts), and the rollback target is whichever upstream artifact is missing, incorrect,
or ambiguous.

### 5-class routing taxonomy

Pick exactly one of the following five classes for `routing_target`:

| target | meaning | typical evidence |
|--------|---------|------------------|
| `adr` | An architectural decision needed to ground the finding is absent from any ADR, or an existing ADR's decision is ambiguous enough to admit the finding as a permitted interpretation. | The finding references a principle (e.g., hexagonal purity, layer placement) that no ADR explicitly decides; or the spec elements citing the relevant ADR all carry `informal_grounds[]` rather than `adr_refs[]`. |
| `spec` | The ADR decides the question, but Phase 1 spec.json did not capture the decision as an actionable acceptance criterion / constraint / in-scope element. | An ADR D-anchor exists for the topic, but no spec element cites it (or the spec element is too vague to drive implementation). |
| `type` | The spec captures the decision correctly, but the per-layer `<layer>-types.json` catalogue has an architectural defect (wrong layer placement, missing entry, wrong `role`, wrong `action`, wrong shape, conflict with `architecture-rules.json`). | A spec acceptance criterion grounds a type concept, but no catalogue entry exists for it, or the entry sits in the wrong layer, or its `role` violates `prefer-type-safe-abstractions.md` / `hexagonal-architecture.md`. |
| `impl_plan` | The ADR, spec, and catalogue all correctly express the design, but the Phase 3 impl-plan task list does not describe the implementation work that would close the finding. | A finding targets a behavior that no `impl-plan.json` task description mentions, or a task's `attributed_entries` map misses an entry whose change is required. |
| `impl` | The entire design chain (ADR → spec → catalogue → impl-plan) is consistent, and the finding is a pure source-side contract violation. | A test fails, a method signature drift, an obviously incorrect branch in source — design documents do not need editing; the source itself must be fixed. |

The `impl` class is not "out_of_scope" / "do nothing". It is the explicit affirmative
diagnosis that no design-side rollback is required and the implementation is the only
target. The calling orchestrator translates this to a source-edit task (no writer
subagent).

### Routing procedure (LLM-semantic, sketch)

1. Read the finding text and the artifacts listed under "Context files".
2. For each candidate root-cause hypothesis the finding admits (typically 1-3), trace it
   up the SoT chain and ask: "Would editing this artifact (ADR / spec / catalogue /
   impl-plan / source) close the finding without creating downstream inconsistency?"
3. The most upstream artifact whose edit closes the finding is the routing target. If two
   candidates tie at the same level (e.g., both ADR and spec could be edited and either
   would close the finding consistently), prefer the upstream one (`adr` over `spec`,
   `spec` over `type`, etc.) — design rollback is cheaper than carrying ambiguity
   downstream.
4. Write the reasoning into `reason` and the concrete recommendation into
   `recommended_next_action`.

The judgment may sometimes need to call `bin/sotp` read-only subcommands
(`signal calc-*`, `ref-verify results`, `task-contract coverage`, `task-contract check`,
`review results`) to inspect the current signal state. These calls are inspection-only;
this skill does not write to any SoT artifact.

## Output contract

Return exactly three top-level fields. The terminal output of the skill is this object;
no surrounding prose, no markdown framing.

| field | type | meaning |
|-------|------|---------|
| `routing_target` | enum string — one of `adr` / `spec` / `type` / `impl_plan` / `impl` | The phase the orchestrator should rollback to. |
| `reason` | non-empty string (Japanese for human-readable diagnostic, English identifiers in code references) | Which signal / finding / artifact inspection led to this routing. Cite specific element ids (e.g., spec `AC-04`, catalogue entry `usecase:PreReviewGateInteractor`, ADR `D-anchor`). |
| `recommended_next_action` | non-empty string (Japanese) | The concrete next step the orchestrator should take (e.g., "adr-editor で `D-anchor` を改訂し ... を明示する", "type-designer で `usecase-types.json` の `X` エントリを `action: add` で追加", "T-XXX の description に Y を追加して /track:impl-plan を再走", "apps/cli/src/foo.rs の Z 関数を修正"). |

All three fields are required on every invocation. Empty `reason` or `recommended_next_action`
is a contract violation.

## Orchestrator dispatch boundary (this skill does NOT dispatch)

This skill is **diagnose-only**. It MUST NOT:

- Launch the `adr-editor` / `spec-designer` / `type-designer` / `impl-planner` subagents
  (writer dispatch belongs to the calling orchestrator — typically `/track:adr2pr` /
  `/track:full-cycle` / a /track:plan back-and-forth loop).
- Edit any SoT artifact (ADR / spec.json / `<layer>-types.json` / impl-plan.json /
  task-coverage.json / task-contract.json).
- Stage or commit any file.
- Run `bin/sotp` write-side subcommands (`signal calc-*` is read-only and allowed for
  signal inspection, but anything that mutates the working tree is forbidden in this
  skill).

The output `routing_target` is a recommendation; the orchestrator may override it and is
expected to read `reason` to validate the diagnosis before dispatching the corresponding
writer or applying a source edit.

## Out-of-scope

- Determining whether the finding is real or noise — by the time this skill runs, the
  caller has already decided the finding is actionable.
- Estimating the size or effort of the recommended fix.
- Sequencing across multiple findings — the skill processes one diagnostic input per
  invocation; chain orchestration is the caller's responsibility.
- Replacing internal signal evaluation — `signal calc-spec-adr` / `calc-catalog-spec` /
  `calc-impl-catalog` and `task-contract check` / `coverage` remain the canonical
  per-phase gates. This skill only fires when those gates have already blocked or when a
  reviewer surfaced something the internal signals could not localize.
