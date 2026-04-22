---
description: Plan a feature via the canonical track planning workflow (Phase 0-3 orchestrator).
---

Canonical command for feature planning. `/track:plan` orchestrates the track planning phases — ADR pre-check, then Phase 0 (init) → Phase 1 (spec) → Phase 2 (design) → Phase 3 (impl-plan) — delegating each phase to its assigned writer capability. The pre-track stage must have authored an ADR under `knowledge/adr/` beforehand; back-and-forth fixes are triggered automatically when downstream gates fail.

Provider routing is resolved via `.harness/config/agent-profiles.json`. Concrete capabilities depend on the active profile (`spec-designer` / `impl-planner` / `type-designer` / `researcher` / `adr-editor`).

Arguments:

- `$ARGUMENTS` is one of the following:
  - `<feature>`: feature name / slug for a new track.
  - `<integer>`: `max_retry` (back-and-forth loop upper bound, default 5). If the argument parses as an integer it is treated as `max_retry`; otherwise it is treated as `<feature>`.
  - `<feature> <max_retry>` (space-separated) when both are needed.
  - Empty: ask the user for a feature name and stop.

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
| ② | type contract → spec | Phase 2 evaluates each catalogue entry's `spec_refs[]` / `informal_grounds[]` (schema/file existence only until the signal is implemented) |
| ③ | implementation → type contract | Phase 4 and later (existing flow). Evaluated by rustdoc extraction cross-checked against catalogue declarations in CI. |

**Reverse references and layer skipping are forbidden**:

- `spec → type catalogue`, `ADR → track-internal artifact`, `track → another track's internal artifact` are forbidden.
- `type catalogue → ADR` is a layer skip. When an ADR reference is required, propagate through the spec step by step.
- Implementation has no embedded reference mechanism pointing at spec / ADR / convention. Consistency is evaluated solely via the rustdoc signal.

Conventions are cross-track auxiliary information shared by every layer and may be cited from any artifact (outside the SoT Chain).

## Lifecycle at a glance

```
(Pre-track stage)
  Author an ADR: /adr:add or a user + main dialogue that writes to knowledge/adr/
     ↓
/track:plan [max_retry]
  ├─ ADR pre-check: confirm the referenced ADR exists under knowledge/adr/
  │   (strict mode: stop and ask the user to author the ADR when missing;
  │    see knowledge/conventions/pre-track-adr-authoring.md)
  │
  ├─ Phase 0: /track:init
  │   ├─ Writer: main
  │   ├─ Output: track/items/<id>/metadata.json (identity-only)
  │   └─ Gate: file existence + identity schema validation
  │
  ├─ Phase 1: /track:spec
  │   ├─ Writer: spec-designer subagent
  │   ├─ Output: track/items/<id>/spec.json
  │   ├─ Inputs: ADR + convention (SoT Chain ①)
  │   └─ Gate: spec → ADR signal (🔵🟡🔴)
  │         🔴 → escalate to ADR editing (re-invoke adr-editor)
  │         🟡 → log a warning and proceed (commit allowed, must be resolved before merge)
  │         🔵 → proceed to the next phase
  │
  ├─ Phase 2: /track:type-design
  │   ├─ Writer: type-designer subagent
  │   ├─ Output: track/items/<id>/<layer>-types.json (per TDDD-enabled layer)
  │   ├─ Inputs: spec (SoT Chain ②)
  │   └─ Gate: type contract → spec signal
  │         Before the signal is implemented: file/schema existence only
  │         Once implemented: 🔴 → escalate to spec editing (re-invoke spec-designer)
  │
  └─ Phase 3: /track:impl-plan
      ├─ Writer: impl-planner subagent
      ├─ Output: track/items/<id>/impl-plan.json + track/items/<id>/task-coverage.json
      ├─ Inputs: type catalogue + spec
      └─ Gate: task-coverage binary pass/fail
            ERROR → re-invoke /track:impl-plan automatically (same phase, regenerate)
```

## Back-and-forth (exploratory refinement loop)

When a downstream phase reports a 🔴 signal, automatically re-invoke the writer **one layer above**:

| Downstream failure | Upstream writer | Re-invoke command |
|---|---|---|
| Phase 1 signal 🔴 | adr-editor subagent | Invoke adr-editor to edit the ADR in the working tree |
| Phase 2 signal 🔴 | spec-designer subagent | Re-invoke `/track:spec` to edit `spec.json` |
| Phase 3 ERROR | impl-planner subagent | Re-invoke `/track:impl-plan` (regenerate in the same phase) |

**ADR auto-edit criteria** (for adr-editor invocation):

- ADR file has commit history → auto-edit (working-tree-only; never commit inside the loop)
- ADR file has no commit history → user pause (user must commit the ADR first, then the loop resumes)

**Termination**: When `/track:plan` finishes (either success or `max_retry` exceeded), if the ADR working tree differs from HEAD, present the diff to the user for a decision (accept / revert / manual edit / abort).

**No unilateral design edits by main**: During back-and-forth, the main orchestrator must not make design decisions and directly apply them to `knowledge/adr/`, `spec.json`, or any type catalogue. Each artifact has one writer:
- `knowledge/adr/*.md`: `adr-editor` edits the file directly (working tree only, no commit during the loop).
- `spec.json` / type catalogues: `spec-designer` / `type-designer` produce **advisory** proposals; the orchestrator transcribes those proposals into the files. The orchestrator does not invent or filter the proposals.

See `knowledge/conventions/pre-track-adr-authoring.md` for the one-file = one-writer principle.

## Writer ownership

| Phase | Artifact | Writer capability |
|---|---|---|
| Pre-track | `knowledge/adr/*.md` (initial authoring) | user + main dialogue (`/adr:add` is also available) |
| Pre-track | `knowledge/adr/*.md` (back-and-forth edits) | adr-editor subagent (auto-invoked) |
| 0 | `track/items/<id>/metadata.json` | main |
| 1 | `track/items/<id>/spec.json` | spec-designer subagent |
| 2 | `track/items/<id>/<layer>-types.json` | type-designer subagent |
| 3 | `track/items/<id>/impl-plan.json` + `track/items/<id>/task-coverage.json` | impl-planner subagent |

Each phase has an independent writer. Rewriting the same file as regular work in another phase is forbidden so the file hash stays stable; only back-and-forth escalation may re-edit upstream artifacts.

## Gate policy

Each gate uses one of two evaluation styles:

- **SoT Chain signal** (🔵🟡🔴): Phase 1 and Phase 2. Each reference field is evaluated independently; the overall signal is the `max` across fields.
- **Binary check** (OK / ERROR): Phase 0 schema validation / Phase 3 task-coverage.

Artificial states such as `approved` / `status` are not introduced (see `knowledge/conventions/workflow-ceremony-minimization.md`).

**Pre-approval exceptions**: outside the gate system, the user is asked for explicit pre-approval only on irreversible actions:

- `git push` / `git commit` (already guarded)
- External API calls (PR / issue creation)
- Destructive filesystem operations
- Environment-breaking changes (CI configuration, lockfile rewrites)

Artifact generation does not belong in that list — every artifact uses post-hoc review (show the real artifact to the user and wait for guidance).

## Interim mode (before independent phase commands exist)

While `/track:init`, `/track:spec`, and `/track:impl-plan` are not yet implemented, `/track:plan` runs the equivalent steps inline:

- Phase 0 equivalent: main writes `track/items/<id>/metadata.json` (identity fields only)
- Phase 1 equivalent: invoke the spec-designer subagent to produce `spec.json`
- Phase 2 equivalent: invoke `/track:type-design` (the existing command dedicated to Phase 2)
- Phase 3 equivalent: invoke the impl-planner subagent to produce `impl-plan.json` + `task-coverage.json`

Once the independent phase commands are in place, `/track:plan` becomes a thin orchestrator that simply invokes the four commands in order.

## Sub-invocation details

### Invoking adr-editor

Resolve the provider through `capabilities.adr-editor` in `.harness/config/agent-profiles.json`. On the Claude profile, invoke through the Agent tool with `subagent_type: "adr-editor"`. The briefing must include:

- The downstream signal evaluation that triggered the 🔴 (which elements fired)
- The target ADR path (and the caller should verify its commit history beforehand)
- An explicit instruction: "edit the working tree only; do not commit inside the loop"

### Invoking spec-designer / impl-planner

The invocation path depends on the active profile:

- **Claude (default profile)**: invoke through the Agent tool with a custom subagent. Use `subagent_type: "spec-designer"` (Phase 1) or `subagent_type: "impl-planner"` (Phase 3); the `model: opus` frontmatter in the corresponding `.claude/agents/<name>.md` file ensures Claude Opus is selected. This path inherits the main-session context, which is better suited to design review than an external process.
- **Codex (codex-heavy profile)**: invoke through `cargo make track-local-plan` (out-of-process, override-first path):

  ```bash
  cargo make track-local-plan -- --model {model} --briefing-file tmp/<capability>-briefing.md
  ```

  The `--briefing-file` path is internally translated to `"Read {path} and perform the task"` so that git keywords do not leak into the Bash command string (maintaining compatibility with the guardrail). The briefing content distinguishes the spec-designer vs impl-planner role; the CLI wrapper is a generic entrypoint that handles both.

The briefing should include:

- The target phase (Phase 1 spec authoring vs Phase 3 impl-plan authoring) and the responsible capability name
- Paths to the ADRs, conventions, and type catalogue (directory path or explicit file path)
- A reference to `.claude/rules/04-coding-principles.md` (enum-first / typestate / newtype principles — spec-designer cites them as contract constraints; impl-planner uses them for task-decomposition consistency)

Save subagent output under (per-track research):

- spec-designer: `track/items/<id>/research/<YYYY-MM-DD-HHMM>-spec-designer-<feature>.md`
- impl-planner: `track/items/<id>/research/<YYYY-MM-DD-HHMM>-impl-planner-<feature>.md`

Cross-track analyses (version baselines, ecosystem surveys) continue to live under `knowledge/research/`. Obtain the timestamp prefix with `date -u +"%Y-%m-%d-%H%M"`.

### Invoking type-designer

`/track:type-design` invokes the type-designer subagent internally. Do not invoke type-designer directly from `/track:plan`; route through `/track:type-design`.

## Pre-flight checks (before `/track:plan` runs)

1. ADR existence check (`knowledge/adr/`)
2. `track/tech-stack.md` has zero `TODO:` markers (precondition for starting implementation)
3. Current branch (decide how to handle `main` / `plan/<id>` until `track/<id>` is materialized)
4. `.harness/config/agent-profiles.json` capability mapping is readable

If check 1 fails, stop and ask the user to author the ADR (`/adr:add <slug>` is a suggested path).

## Timestamps

Obtain `created_at` / `updated_at` / research-note prefixes as follows:

```bash
date -u +"%Y-%m-%dT%H:%M:%SZ"  # ISO 8601 UTC (metadata fields)
date -u +"%Y-%m-%d-%H%M"       # research-note prefix
```

Manual input / guessing is forbidden. The `sotp` CLI `now_iso8601()` uses UTC, so all timestamps are UTC-aligned.

## Guide injection

`$ARGUMENTS` (the feature name) and the latest track's `spec.md` / `plan.md` are scanned. Any external guide in `knowledge/external/guides.json` whose `trigger_keywords` match is auto-injected into context as a summary (raw cached text is opened only when needed).

## Rendered views

After `/track:plan` completes, `track/items/<id>/plan.md` and `track/items/<id>/spec.md` are regenerated as read-only views via `cargo make track-sync-views` (direct edits are forbidden). Refresh `track/items/<id>/verification.md` and `track/registry.md` as needed.

`/track:plan` does not write implementation code. Implementation is delegated to `/track:implement` or `/track:full-cycle`.

## Report format

On completion, present:

1. Per-phase gate results (🔵🟡🔴 / OK / ERROR)
2. Generated track artifact paths (`metadata.json` / `spec.json` / `<layer>-types.json` / `impl-plan.json` / `task-coverage.json`)
3. Back-and-forth edits that occurred (the target artifact and its original writer)
4. ADR working-tree diff against HEAD, if any (ask the user to decide: accept / revert / manual edit / abort)
5. Suggested next commands:
   - Standard lane: `/track:implement` → `/track:review` → `/track:commit`, or `/track:full-cycle <task>`
   - Planning-review-first: `/track:review` → `/track:commit` to review the planning artifacts before implementation
   - Plan-only lane: `/track:plan-only <feature>` creates a `plan/<id>` branch, merge the PR into main, then `/track:activate <track-id>` to start implementation

## References

- `knowledge/conventions/pre-track-adr-authoring.md` — ADR pre-track authoring, strict mode, adr-editor operation
- `knowledge/conventions/workflow-ceremony-minimization.md` — post-hoc review, removal of the `approved` state, pre-approval restricted to irreversible actions
- `knowledge/conventions/source-attribution.md` — source attribution for spec.json elements
- `knowledge/conventions/adr.md` — ADR format rules
- `knowledge/external/guides.json` — external guide registry (auto-injected via `trigger_keywords`)
- `.claude/rules/04-coding-principles.md` — enum-first / typestate / newtype principles (required reading for spec-designer / impl-planner / type-designer briefings)
