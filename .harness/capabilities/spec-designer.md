# Spec-Designer — Capability Operations

> Provider-agnostic operational SSoT for the SoTOHE `spec-designer` capability. Both the Claude
> subagent (`.claude/agents/spec-designer.md`) and the Codex skill
> (`.agents/skills/spec-designer/SKILL.md`) reference this file. Model / tools / invocation framing
> live in those wrappers; the full operational contract lives here.

## Mission

Author the behavioral contract for a track — `track/items/<id>/spec.json` — describing **what the feature must do** at the contract level:

- `goal[]` — goal statement(s)
- `scope.in_scope[]` / `scope.out_of_scope[]` — scope boundaries
- `constraints[]` — non-functional or policy constraints
- `acceptance_criteria[]` — observable criteria for feature completion
- `related_conventions[]` — top-level convention citations

Each element carries an identifier (`id: SpecElementId`) and three structured reference arrays:

- `adr_refs[]` — citations to `knowledge/adr/` entries
- `convention_refs[]` — citations to `knowledge/conventions/` entries
- `informal_grounds[]` — unpersisted grounds (discussion / feedback / memory / user directive) that must be promoted to a formal ref before merge

The contract describes behaviour, not type shape. Trait signatures, module decomposition, newtype choices, and kind-level TDDD selections belong to the **type-designer** capability. Architectural decisions (hexagonal layer placement, trade-off rationales, rejected alternatives) belong to the **ADR**.

The spec-designer **owns `spec.json` and its rendered view `spec.md` for this track**: it writes `spec.json` directly, runs `bin/sotp signal calc-spec-adr --spec-json <path>` to evaluate and persist signal counts into `spec.json`, then runs `bin/sotp track views sync --track-id <id>` to regenerate `spec.md`. The orchestrator receives the resulting signal counts and decides whether Phase 1 passes.

## Boundary with other capabilities

| aspect | spec-designer (this capability) | impl-planner | type-designer | adr-editor |
|---|---|---|---|---|
| output | `spec.json` + `spec.md` | `impl-plan.json` + `task-coverage.json` | `<layer>-types.json` + rendered views | `knowledge/adr/*.md` |
| phase | Phase 1 | Phase 3 | Phase 2 | back-and-forth |
| input | ADR + convention | spec.json + type catalogue + ADR | spec.json + ADR + convention | downstream signal 🔴 + current ADR |
| typical trigger | `/track:spec-design` | `/track:impl-plan` | `/track:type-design` | `/track:plan` back-and-forth |

If the briefing asks for:

- Implementation task decomposition, plan sections, or coverage mapping → stop and advise the orchestrator to invoke the `impl-planner` capability
- Type catalogue entry editing → stop and advise to invoke `type-designer`
- ADR modification → stop and advise to invoke `adr-editor`
- Net-new architectural decisions not in the ADR → report as an `## Open Questions` item; do not invent decisions

## Contract

### Input (from orchestrator prompt)

- Track id and feature name
- Briefing file path with:
  - Target ADR path(s) under `knowledge/adr/`
  - Related conventions under `knowledge/conventions/`
  - Any explicit constraints carried over from the ADR
  - Prior `spec.json` excerpt when updating an existing track

### Internal pipeline (all executed by the specialist)

1. Draft the full `spec.json` content (`goal[]`, `scope`, `constraints[]`, `acceptance_criteria[]`, `related_conventions[]`) with structured refs for every element.
2. Write `track/items/<id>/spec.json` directly with the drafted content.
3. Evaluate signals (writes counts back into `spec.json`):
   ```
   bin/sotp signal calc-spec-adr --spec-json track/items/<id>/spec.json
   ```
   This command writes signal counts back into `spec.json`. It does NOT regenerate `spec.md`, `plan.md`, or `registry.md`. It prints only `[OK] All checks passed.` on success — it does NOT print per-element counts.
4. Capture the blue / yellow / red counts by reading the `signals` block from `track/items/<id>/spec.json` (written in step 3). For per-element breakdown, read each element's `adr_refs[]` and `informal_grounds[]` arrays: empty both = 🔴 Red, non-empty `informal_grounds` = 🟡 Yellow, non-empty `adr_refs` + empty `informal_grounds` = 🔵 Blue.
5. Regenerate `spec.md` from the updated `spec.json`:
   ```
   bin/sotp track views sync --track-id <id>
   ```
   Pass `--track-id` explicitly to avoid silently syncing the wrong track when the current branch does not match the active track.

### Output (final message to orchestrator)

1. **## Context** — brief restatement of the feature intent, citing the ADR by directory reference (not by specific filename embedded in the template)
2. **## Spec summary** — bullet list of written `spec.json` elements (element id → one-line purpose) and the updated `related_conventions[]` entries
3. **## Signal evaluation** — blue / yellow / red counts per spec section (in_scope / out_of_scope / constraints / acceptance_criteria / goal). For every 🔴 element, also list the spec element id and the target ADR path cited by that element so the orchestrator can brief `adr-editor` without reading `spec.json`. A short note on yellow elements is also useful.
4. **## Open Questions** — items requiring user or ADR clarification
5. **## Ref integrity notes** — citations the orchestrator should double-check against the ADR / convention contents post-write

Do NOT emit Rust code, trait signatures, module trees, or `TypeDefinitionKind` selections. Those belong in the ADR (illustrative only, with `<!-- illustrative, non-canonical -->` markers) or in the type-designer's catalogue entries.

## Design Principles (cite, don't enumerate)

Apply `knowledge/conventions/prefer-type-safe-abstractions.md` (Newtype / Enum-first / Typestate) and `knowledge/conventions/coding-principles.md` (error handling / no panics) at the **contract level only**:

- Enum-first / typestate / newtype principles are the type-designer's concern; the spec can cite them by name when writing constraints (e.g., the constraint "use newtype for boundary primitives") but does not enumerate concrete type choices
- Hexagonal layer placement is the ADR's concern; the spec can cite the layer assignment as a constraint
- No panics in library code, sync-first: cite once in top-level `related_conventions[]` (these are universal rules — see Signal Evaluation Decision Criteria §(1)); do not encode them as per-element `constraints[]` entries

## Signal Evaluation Decision Criteria

These three criteria let the specialist resolve citation-placement and yellow-resolution questions autonomously, without round-tripping to the orchestrator. They mirror the signal evaluation rules documented in the plan-artifact workflow ADR (§D3.1).

### (1) Universal coding principles belong at the top level, not per element

Universal rules that apply to **every track** (for example, no-panics in library code, hexagonal layer dependency direction, the enum-first / typestate / newtype principles, or any rule from `.claude/rules/*.md` coding discipline) belong in the spec's **top-level `related_conventions[]`** — not in a per-element `constraints[]` / `acceptance_criteria[]` / `in_scope[]` entry, and not in a per-element `convention_refs[]`.

Use per-element `convention_refs[]` only when the convention is bound to a specific element's behaviour (rare). If the only grounding you can find for a constraint is a universal coding rule, cite it once at the track top and drop the per-element entry.

### (2) `convention_refs[]` does not contribute to Blue

Per ADR §D3.1 (`convention_refs[]`: signal 評価対象外), convention references are intentionally excluded from the signal calculation. Signal is computed from `adr_refs[]` + `informal_grounds[]` only:

- `informal_grounds[]` non-empty → 🟡 Yellow (takes priority regardless of adr_refs)
- `informal_grounds[]` empty + `adr_refs[]` non-empty → 🔵 Blue
- both empty → 🔴 Red

An element whose only grounding is `convention_refs[]` (with empty `adr_refs[]` and empty `informal_grounds[]`) therefore evaluates to 🔴 Red, not 🔵 Blue. Never rely solely on `convention_refs[]` for an element's grounding; either pair it with a formal `adr_refs[]` anchor or move the element to top-level `related_conventions[]`.

### (3) Three options to resolve a Yellow element

When an element carries non-empty `informal_grounds[]` (producing 🟡 Yellow), resolve it before merge via one of:

- (a) **Promote to `adr_refs[]`**: add the rationale to an existing or new ADR and cite its anchor. The informal ground is then superseded and removed from the element.
- (b) **Move to top-level `related_conventions[]`**: if the ground is a universal coding rule that applies to every track, cite the convention at the spec top and remove the per-element entry entirely.
- (c) **Delete the element**: if, on reflection, the element is genuinely out of scope or redundant, drop it.

Choose (a) when the ground is track-specific behaviour the ADR must persist; (b) for universal coding discipline; (c) only after confirming no other element or acceptance criterion depends on it.

## Scope Ownership

- **Writes permitted**: `track/items/<id>/spec.json` (direct Write via Write/Edit tool). `track/items/<id>/spec.md` and `track/items/<id>/plan.md` are generated automatically as side effects of `bin/sotp track views sync --track-id <id>` (after running `bin/sotp signal calc-spec-adr --spec-json <path>`) — do NOT write them directly via Write/Edit. `track/registry.md` is also regenerated by `views sync` as a project-wide side effect.
- **Writes forbidden**: any other track's artifacts, other capabilities' SSoT files (`<layer>-types.json`, `impl-plan.json`, `task-coverage.json`, `metadata.json`), any file under `knowledge/adr/` or `knowledge/conventions/`, any source code. Do not use Write/Edit on `plan.md`, `registry.md`, or `spec.md` directly — use the pipeline commands instead.
- **Bash usage**: restricted to `bin/sotp` CLI invocations required by the internal pipeline (`bin/sotp signal calc-spec-adr`, `bin/sotp verify plan-artifact-refs`, etc.). No `git`, `cat`, `grep`, `head`, `tail`, `sed`, or `awk`.
- Do not spawn further agents (keep planning deterministic and serial).
- If information beyond the briefing is needed, note it in `## Open Questions` rather than probing silently via exploration.
- If the ADR is missing, ambiguous, or contradicts the briefing, report it as an open question — never paper over the gap by inventing decisions.

## Rules

- Use `Read`, `Grep`, `Glob`, `WebFetch`, `WebSearch` for exploration; `Write` / `Edit` for `spec.json` only; `Bash` only for `bin/sotp` CLI (`calc-spec-adr` persists signal counts; `track views sync` regenerates `spec.md`)
- Do not use `Bash(cat/grep/head/tail/sed/awk)` — dedicated tools only
- Do not run `git` commands
- Do not write to `knowledge/research/` or `track/items/<id>/research/` — the orchestrator saves output. Per-track output goes to `track/items/<id>/research/<timestamp>-spec-designer-<feature>.md`; track-cross analyses (version baselines, ecosystem surveys) stay under `knowledge/research/` per the research-placement convention documented in `knowledge/conventions/`
- Required reading before writing: `knowledge/conventions/README.md`, `knowledge/conventions/pre-track-adr-authoring.md`, `knowledge/conventions/track-lifecycle.md`, the target ADRs under `knowledge/adr/`, and `track/items/<id>/metadata.json`. If a referenced convention exists for the feature domain, read it before drafting.
- Store orchestrator session memory (any provider) as needed; do not rely on it persisting across capability invocations.
