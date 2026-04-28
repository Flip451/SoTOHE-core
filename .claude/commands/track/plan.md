---
description: Plan a feature via the canonical track planning workflow (Phase 0-3 orchestrator).
---

Canonical command for feature planning. `/track:plan` is the state-machine orchestrator that drives Phase 0 → Phase 1 → Phase 2 → Phase 3 through the four independent phase commands, delegating each phase to its writer capability. The pre-track stage must have authored an ADR under `knowledge/adr/` beforehand; back-and-forth escalation is triggered automatically when a downstream gate fails.

Provider routing is resolved via `.harness/config/agent-profiles.json`.

Arguments:

- `$ARGUMENTS`:
  - `<feature>`: feature name / slug for a new track
  - `<integer>`: `max_retry` positional integer (default 5). If `$ARGUMENTS` parses as a bare integer it is treated as `max_retry`; otherwise as `<feature>`. Flag-style names are not used — invoke as `/track:plan 3` for `max_retry=3`.
  - `<feature> <integer>`: both (space-separated)
  - Empty: ask the user for a feature name and stop

## Preamble: create a task list

Before executing the state machine, create a `TaskCreate` task list covering the following items so progress stays visible across phases and back-and-forth loops:

1. Phase 0 — invoke `/track:init <feature>`
2. Phase 1 loop — invoke `/track:spec-design`, evaluate the spec → ADR signal, escalate on 🔴
3. Phase 2 loop — invoke `/track:type-design`, evaluate the type → spec signal per layer, escalate on 🔴
4. Phase 3 loop — invoke `/track:impl-plan`, evaluate the task-coverage gate, re-invoke on ERROR
5. Termination — ADR working-tree diff presentation and user decision

Mark each item `in_progress` before executing and `completed` after it passes. When back-and-forth escalation runs, append a sub-task for each re-invocation (the back-and-forth transition is itself a task). Propagate the gate result sequentially through each task; do not parallelise phases.

## SoT Chain (dependency direction)

Track artifacts form a one-way downstream → upstream dependency chain:

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
| ② | type contract → spec | Phase 2 evaluates each catalogue entry's `spec_refs[]` / `informal_grounds[]` and renders per-layer 🔵/🟡/🔴 counts |
| ③ | implementation → type contract | Phase 4 and later. Evaluated by rustdoc extraction cross-checked against catalogue declarations in CI. |

**Reverse references and layer skipping are forbidden**:

- `spec → type catalogue`, `ADR → track-internal artifact`, `track → another track's internal artifact` are forbidden.
- `type catalogue → ADR` is a layer skip. When an ADR reference is required, propagate through the spec step by step.
- Implementation has no embedded reference mechanism pointing at spec / ADR / convention. Consistency is evaluated solely via the rustdoc signal.

Conventions are cross-track auxiliary information shared by every layer and may be cited from any artifact (outside the SoT Chain).

## Phase invocation table

| Phase | Command | Writer | Gate |
|---|---|---|---|
| 0 | `/track:init` | main (direct) | metadata identity schema (OK / ERROR) |
| 1 | `/track:spec-design` | spec-designer subagent (direct writer) | spec → ADR signal (🔵🟡🔴) |
| 2 | `/track:type-design` | type-designer subagent (direct writer) | type → spec signal, per layer (🔵🟡🔴) |
| 3 | `/track:impl-plan` | impl-planner subagent (direct writer) | task-coverage binary gate (OK / ERROR) |

Each phase command owns its full internal pipeline (Write + render + CLI gate evaluation). `/track:plan` receives only the gate result and decides the next transition — it does not read or manipulate the artifacts itself.

## Phase 0: /track:init

Invoke `/track:init <feature>` with the feature name from `$ARGUMENTS`. Phase 0 creates the track directory, writes `metadata.json`, materializes the branch, and runs the identity-schema binary gate. On ERROR, stop and report to the user. On OK, mark the Phase 0 task `completed` and proceed to Phase 1.

## Phase 1 loop: /track:spec-design

1. Invoke `/track:spec-design` (no arguments).
2. Read the signal result returned by the command. The result includes per-section blue / yellow / red counts **and**, for each 🔴 element, the spec element id and the target ADR path cited by that element — so the orchestrator has enough information to brief `adr-editor` without reading `spec.json` itself.
3. Apply the loop rule:
   - 🔵 (all elements blue): mark the Phase 1 task `completed` and proceed to Phase 2.
   - 🟡 (at least one yellow, no red): log a warning and proceed to Phase 2. Yellow must be resolved before merge but does not block the phase transition.
   - 🔴 (at least one red): escalate per the **ADR auto-edit criteria** below:
     a. Identify the target ADR file path cited by the 🔴 element(s).
     b. Check the ADR file's commit history:
        - Has commit history → invoke the `adr-editor` subagent (Agent tool, `subagent_type: "adr-editor"`). Briefing must include: the 🔴 spec element(s), the target ADR path, and the explicit instruction "edit the working tree only; do not commit inside the loop".
        - No commit history → user pause. Instruct the user to commit the ADR first, then resume.
     c. After the ADR edit, re-invoke `/track:spec-design`.
     d. **max_retry guard**: each 🔴 → adr-editor → re-invoke iteration counts against `max_retry`. On overflow, stop and present options to the user (continue with warnings, abort, or manual edit).

## Phase 2 loop: /track:type-design

1. Invoke `/track:type-design` (no arguments).
2. Read the per-layer signal result.
3. Apply the loop rule:
   - 🔵 across all processed layers: mark the Phase 2 task `completed` and proceed to Phase 3.
   - 🟡: log a warning and proceed. Yellow must be resolved before merge.
   - 🔴:
     a. Re-invoke `/track:spec-design` (Phase 2 🔴 typically indicates the spec needs refinement before the type catalogue can pass).
     b. Re-evaluate the Phase 1 gate. If Phase 1 returns 🔴 as well, escalate via the Phase 1 ADR loop above.
     c. On Phase 1 🔵 or 🟡, re-invoke `/track:type-design`.
     d. **max_retry guard**: the Phase 2 retry counter is independent of Phase 1's. On overflow, stop and present options.

## Phase 3 loop: /track:impl-plan

1. Invoke `/track:impl-plan` (no arguments).
2. Read the binary gate verdict (OK / ERROR).
3. Apply the loop rule:
   - OK: mark the Phase 3 task `completed` and proceed to termination.
   - ERROR: re-invoke `/track:impl-plan` in the same phase (the impl-planner subagent regenerates `impl-plan.json` + `task-coverage.json` on each invocation). **max_retry guard** applies; on overflow, stop and present the latest error message to the user.

## Termination

After Phase 3 OK (or `max_retry` overflow anywhere in the loop):

1. Check whether the ADR working tree differs from HEAD.
2. If the diff is non-empty, present the diff to the user and ask them to decide:
   - **accept**: stage and commit the ADR alongside the other artifacts
   - **revert**: discard the ADR working-tree changes
   - **manual edit**: pause for the user to refine the ADR further
   - **abort**: stop `/track:plan` and leave the tree as-is for the user to inspect
3. Mark the termination task `completed`.

## Back-and-forth (summary table)

| Downstream failure | Upstream writer | Re-invoke command |
|---|---|---|
| Phase 1 🔴 | adr-editor subagent | Invoke adr-editor (working-tree edit only, no commit inside the loop) |
| Phase 2 🔴 | spec-designer subagent | Re-invoke `/track:spec-design`; if Phase 1 also 🔴, escalate to the ADR loop |
| Phase 3 ERROR | impl-planner subagent | Re-invoke `/track:impl-plan` (regenerate in the same phase) |

**ADR auto-edit criteria**:

- ADR file has commit history → auto-invoke adr-editor
- ADR file has no commit history → user pause (user commits the ADR first, then the loop resumes)

**One writer per file**: the orchestrator does not directly Write or Edit `knowledge/adr/*.md`, `spec.json`, `<layer>-types.json`, `impl-plan.json`, or `task-coverage.json`. Each artifact's writer owns its file end-to-end (see Writer ownership below).

## Writer ownership

| Phase | Artifact | Writer capability |
|---|---|---|
| Pre-track | `knowledge/adr/*.md` (initial authoring) | user + main dialogue (`/adr:add` is available) |
| Pre-track | `knowledge/adr/*.md` (back-and-forth edits) | adr-editor subagent (auto-invoked) |
| 0 | `track/items/<id>/metadata.json` | main (direct) |
| 1 | `track/items/<id>/spec.json` + `spec.md` | spec-designer subagent (direct writer) |
| 2 | `track/items/<id>/<layer>-types.json` + `<catalogue-stem>-baseline.json` + derived views (type-graph md, contract-map md, `<layer>-types.md`) | type-designer subagent (direct writer) |
| 3 | `track/items/<id>/impl-plan.json` + `task-coverage.json` | impl-planner subagent (direct writer) |

Each writer owns its SSoT files and associated rendered views end-to-end. Rewriting another writer's files during the same phase is forbidden so file hashes stay stable; only back-and-forth escalation may re-invoke an upstream writer.

## Sub-agent briefing rules (no design prescription)

When the `/track:plan` orchestrator composes a briefing for one of the track sub-agents (spec-designer, type-designer, impl-planner, adr-editor), the briefing body MUST contain only:

- Problem statement / trigger (what was observed, what symptom)
- Context references (track state, file paths, relevant ADRs / conventions, prior edits already in the working tree)
- Interaction contract: what the sub-agent should report back **and** any operational constraints on what it may or may not do (e.g., "edit the working tree only; do not commit inside the loop")

The briefing body MUST NOT contain **design prescription** — anything that pre-solves the sub-agent's own domain expert judgment:

- Prescriptive design solutions (e.g., "apply approach X to solve this problem")
- Pre-classified approaches (e.g., "categorize into these 3 buckets")
- Pre-decided outcomes presented as design requirements (e.g., "this must result in Z")
- Priority orderings for design alternatives the sub-agent is supposed to evaluate

Each sub-agent is the domain expert for its owned artifact (ADR / spec / type catalogue / impl-plan). The orchestrator supplies context and problem framing; the sub-agent judges, decides, and reports back the decision path with rationale. Pre-solving in the briefing bypasses the sub-agent's judgment, violates SoT Chain writer ownership, and has produced real design errors in past runs (e.g., type-designer was briefed with "list only new methods for `action: modify`" — a TDDD-framework-semantic decision the orchestrator was not qualified to make, producing Red signals that should have been Yellow).

**Mechanism**: tier 5 (documentation + semantic review) per `knowledge/conventions/enforce-by-mechanism.md` §Exceptions. Harness-policy scope covers `.claude/commands/**`; reviewers must flag changes that weaken or remove this constraint. **Reassess trigger**: recurring Red signals traceable to biased briefings, or a committed briefing-file schema that enables static analysis.

## Gate policy

Each gate uses one of two evaluation styles:

- **SoT Chain signal** (🔵🟡🔴): Phase 1 and Phase 2.
- **Binary check** (OK / ERROR): Phase 0 identity schema / Phase 3 task-coverage.

Artificial states such as `approved` / `status` are not introduced (see `knowledge/conventions/workflow-ceremony-minimization.md`).

**Pre-approval exceptions**: outside the gate system, the user is asked for explicit pre-approval only on irreversible actions:

- `git push` / `git commit` (already guarded)
- External API calls (PR / issue creation)
- Destructive filesystem operations
- Environment-breaking changes (CI configuration, lockfile rewrites)

Artifact generation uses post-hoc review (show the artifact to the user and wait for guidance), not pre-write approval.

## Sub-invocation details

### /track:init (Phase 0, writer = main)

`/track:init` runs the identity-only bootstrap directly from main: ADR pre-check, track directory creation, `metadata.json` write, branch materialization, and identity-schema verification. Refer to `.claude/commands/track/init.md` for the full step list; `/track:plan` passes the feature name and receives the schema verdict.

### /track:spec-design (Phase 1, writer = spec-designer subagent)

Invocation path depends on the active profile (`.harness/config/agent-profiles.json`):

- **Claude (default profile)**: invoke via the Agent tool with `subagent_type: "spec-designer"`. The `model: opus` frontmatter in `.claude/agents/spec-designer.md` guarantees Opus selection.
- **Codex (codex-heavy profile)**: invoke out-of-process through the repo-owned wrapper:
  ```bash
  cargo make track-local-plan -- --model {model} --briefing-file tmp/spec-designer-briefing.md
  ```
  The wrapper translates `--briefing-file` internally so git keywords do not leak into the command string (compatible with the `block-direct-git-ops` guardrail).

The subagent owns `spec.json` and `spec.md`, runs the CLI signal evaluation internally, and returns the blue / yellow / red counts to `/track:plan`. See `.claude/agents/spec-designer.md` for its internal pipeline.

### /track:type-design (Phase 2, writer = type-designer subagent)

Invoke via the Agent tool with `subagent_type: "type-designer"`. The subagent owns every `<layer>-types.json`, captures baselines, renders per-layer views, and evaluates the type → spec signal internally. `/track:plan` receives per-layer signal counts. See `.claude/agents/type-designer.md`.

### /track:impl-plan (Phase 3, writer = impl-planner subagent)

Invocation path depends on the active profile:

- **Claude (default profile)**: invoke via the Agent tool with `subagent_type: "impl-planner"`.
- **Codex (codex-heavy profile)**: invoke out-of-process through the same wrapper used for Phase 1:
  ```bash
  cargo make track-local-plan -- --model {model} --briefing-file tmp/impl-planner-briefing.md
  ```

The subagent owns `impl-plan.json` and `task-coverage.json`, and evaluates the task-coverage binary gate internally. `/track:plan` receives the OK / ERROR verdict. See `.claude/agents/impl-planner.md`.

### Invoking adr-editor (Phase 1 back-and-forth)

Invoke via the Agent tool with `subagent_type: "adr-editor"`. Briefing must include the Phase 1 🔴 spec element(s), the target ADR path (caller verifies commit history beforehand), and the explicit instruction "edit the working tree only; do not commit inside the loop". See `.claude/agents/adr-editor.md`.

## Pre-flight checks (before `/track:plan` runs)

1. ADR existence check (`knowledge/adr/`). If no relevant ADR exists, stop and ask the user to author one (`/adr:add <slug>` is a suggested path).
2. `track/tech-stack.md` has zero `TODO:` markers (precondition for implementation).
3. Current branch is compatible with the operation (`main` / `plan/<id>` / `track/<id>`).
4. `.harness/config/agent-profiles.json` capability mapping is readable.

## Timestamps

Obtain `created_at` / `updated_at` / research-note prefixes via:

```bash
date -u +"%Y-%m-%dT%H:%M:%SZ"  # ISO 8601 UTC (metadata fields)
date -u +"%Y-%m-%d-%H%M"       # research-note prefix
```

Manual input / guessing is forbidden.

## Report format

On completion, present:

1. Per-phase gate results (🔵🟡🔴 / OK / ERROR) and the final `max_retry` counters per loop
2. Generated track artifact paths (`metadata.json` / `spec.json` / `<layer>-types.json` / `impl-plan.json` / `task-coverage.json`)
3. Back-and-forth edits that occurred (target artifact and its writer)
4. ADR working-tree diff against HEAD (if any) and the user's termination decision
5. Suggested next commands:
   - Standard lane: `/track:implement` → `/track:review` → `/track:commit`, or `/track:full-cycle`
   - Planning-review-first: `/track:review` → `/track:commit` to review planning artifacts before implementation
   - Plan-only lane: `/track:plan-only <feature>` + `/track:activate <track-id>`

## References

- `knowledge/conventions/pre-track-adr-authoring.md` — ADR pre-track authoring, strict mode, adr-editor operation
- `knowledge/conventions/workflow-ceremony-minimization.md` — post-hoc review, pre-approval limited to irreversible actions
- `knowledge/conventions/source-attribution.md` — source attribution for spec.json elements
- `knowledge/conventions/adr.md` — ADR format rules
- `.claude/rules/04-coding-principles.md` — enum-first / typestate / newtype principles
