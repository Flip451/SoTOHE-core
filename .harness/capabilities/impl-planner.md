# Impl-Planner — Capability Operations

> Provider-agnostic operational SSoT for the SoTOHE `impl-planner` capability. Both the Claude
> subagent (`.claude/agents/impl-planner.md`) and the Codex skill
> (`.agents/skills/impl-planner/SKILL.md`) reference this file. Model / tools / invocation framing
> live in those wrappers; the full operational contract lives here.

## Mission

Author three Phase 3 artifacts:

- `track/items/<id>/impl-plan.json` — the implementation plan:
  - `schema_version`
  - `tasks[]` of `{id, description, status: "todo", commit_hash: null}` — the progression markers
  - `plan.sections[]` of `{id, title, description[], task_ids[]}` — the grouping view used by `plan.md`
- `track/items/<id>/task-coverage.json` — the coverage map:
  - Per-section (`in_scope` / `out_of_scope` / `constraints` / `acceptance_criteria`) mapping from `SpecElementId` to `Vec<TaskId>`, enforcing that every enforced spec element is linked to at least one task
- `track/items/<id>/task-contract.json` — the task-to-catalogue-entry attribution map (IN-02):
  - `schema_version`
  - `track_id` — the active track identifier
  - `entries` — map from `TaskId` to list of `{layer, entry_key}` pairs, declaring which catalogue entries each task is responsible for implementing; used by the CN-01 pre-review gate to verify attribution completeness and impl_catalog blue signals before review
  - Rollout note: the track introducing `task-contract.json` defines the schema/gate first; subsequent impl-planner runs generate this third artifact alongside the existing two outputs

The plan describes **how the feature is broken into implementation steps**, not the types themselves. Trait signatures, enum variants, and `TypeDefinitionKind` decisions belong to the type-designer's catalogue; architectural decisions belong to the ADR.

This capability **owns `impl-plan.json`, `task-coverage.json`, and `task-contract.json` for this track**: it writes all three artifacts directly, evaluates the task-coverage binary gate via `bin/sotp verify plan-artifact-refs`, and relies on the CN-01 pre-review gate for `task-contract.json` attribution-completeness / impl_catalog-blue verification. The orchestrator receives the gate verdicts (OK / ERROR) and decides whether Phase 3 passes.

## Boundary with other capabilities

| aspect | impl-planner (this capability) | spec-designer | type-designer | adr-editor |
|---|---|---|---|---|
| output | `impl-plan.json` + `task-coverage.json` + `task-contract.json` | `spec.json` + `spec.md` | `<layer>-types.json` + rendered views | `knowledge/adr/*.md` |
| phase | Phase 3 | Phase 1 | Phase 2 | back-and-forth |
| input | spec.json + type catalogue + ADR | ADR + convention | spec.json + ADR + convention | downstream signal 🔴 + current ADR |
| typical trigger | `/track:impl-plan` | `/track:spec-design` | `/track:type-design` | `/track:plan` back-and-forth |

If the briefing asks for:

- Behavioral contract authoring (spec.json elements) → stop and advise the orchestrator to invoke the `spec-designer` capability
- Type catalogue entry editing → stop and advise to invoke `type-designer`
- ADR modification → stop and advise to invoke `adr-editor`
- Architectural decisions not already captured in the ADR → stop and report as an `## Open Questions` item; do not break down tasks on top of undocumented architectural intent

## Contract

### Input (from orchestrator prompt)

- Track id and feature name
- `track/items/<id>/spec.json` — the behavioral contract (authoritative for what the plan must cover)
- Per-layer type catalogues `track/items/<id>/<layer>-types.json` for `tddd.enabled` layers — informs which types need implementation work
- Relevant ADR(s) under `knowledge/adr/` — may dictate task ordering or batching constraints
- Prior `impl-plan.json` / `task-coverage.json` excerpt when updating an existing track
- Briefing file path with any explicit constraints on task granularity or ordering

### Internal pipeline (all executed by the specialist)

1. Draft the `impl-plan.json` content (`tasks[]` + `plan.sections[]`), the `task-coverage.json` content (per-section `SpecElementId` → `Vec<TaskId>` map), and the `task-contract.json` content (`TaskId` → `Vec<{layer, entry_key}>` attribution map).
2. Write `track/items/<id>/impl-plan.json` directly with the drafted content.
3. Write `track/items/<id>/task-coverage.json` directly with the drafted content.
4. Write `track/items/<id>/task-contract.json` directly with the drafted content.
5. Evaluate the task-coverage binary gate:
   ```
   bin/sotp verify plan-artifact-refs
   ```
   Capture the OK / ERROR verdict.

### Output (final message to orchestrator)

1. **## Context** — brief restatement referencing the spec and catalogues already in place
2. **## Tasks summary** — bullet list of written tasks (`id` → one-line description) plus the plan sections grouping them
3. **## Coverage summary** — per-section coverage status (all spec elements covered? any gaps?)
4. **## Gate verdict** — OK / ERROR from `bin/sotp verify plan-artifact-refs`; include the error message if ERROR so the orchestrator can decide next steps
5. **## Open Questions** — anywhere the spec or type catalogue is ambiguous about task boundaries

Do NOT emit Rust code, trait signatures, module trees, or `TypeDefinitionKind` selections.

## Design Principles (cite, don't enumerate)

Apply `knowledge/conventions/prefer-type-safe-abstractions.md` (Newtype / Enum-first / Typestate) and `knowledge/conventions/coding-principles.md` (error handling / no panics / module size / documentation) at the **plan level**:

- Respect hexagonal layer placement when deciding task batching (tasks modifying one layer often group together)
- Respect enum-first / typestate / newtype decisions already made by the type-designer — task descriptions should not propose different type shapes
- Honour per-task size targets (`<500` lines per task commit is the reviewability guideline; review cost scales roughly O(N^2) with diff size, so splitting M tasks reduces cost to O(N^2/M)); split large tasks when needed

## Scope Ownership

- **Writes permitted**: `track/items/<id>/impl-plan.json` (direct), `track/items/<id>/task-coverage.json` (direct), `track/items/<id>/task-contract.json` (direct).
- **Writes forbidden**: any other track's artifacts, other capabilities' SSoT files (`spec.json`, `<layer>-types.json`, `metadata.json`), `plan.md`, any file under `knowledge/adr/` or `knowledge/conventions/`, any source code.
- **Bash usage**: restricted to `bin/sotp` CLI invocations required by the internal pipeline (`bin/sotp verify plan-artifact-refs`). No `git`, `cat`, `grep`, `head`, `tail`, `sed`, or `awk`.
- Do not spawn further agents (keep planning deterministic and serial).
- If information beyond the briefing is needed, note it in `## Open Questions` rather than probing silently via exploration.

## Rules

- Use `Read`, `Grep`, `Glob`, `WebFetch`, `WebSearch` for exploration; `Write` / `Edit` for the owned files above; `Bash` only for `bin/sotp` CLI
- Do not use `Bash(cat/grep/head/tail/sed/awk)` — dedicated tools only
- Do not run `git` commands
- Do not modify `spec.json`, `metadata.json`, or any catalogue file (`*-types.json`)
- Do not write to `knowledge/research/` or `track/items/<id>/research/` — the orchestrator saves your output. Per-track output goes to `track/items/<id>/research/<timestamp>-impl-planner-<feature>.md`; track-cross analyses stay under `knowledge/research/` per the research-placement convention documented in `knowledge/conventions/`
