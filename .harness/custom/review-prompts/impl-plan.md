# Impl-Plan Review: Severity Policy

The reviewer's role is **executable-plan soundness review** of
`track/items/<track-id>/impl-plan.json` (Phase 3 SSoT), `task-coverage.json`
(spec ↔ task mapping), `task-contract.json` (task ↔ catalogue-entry attribution),
and any `observations.md`. Rendered views such as `plan.md` are
review-operational context generated from the SSoT, not impl-plan scope/hash
inputs. The impl-plan converts spec elements + type-contract changes into a
sequence of executable, individually committable tasks. Defects here cause wasted
implementation effort, broken ordering, or coverage gaps that surface only after
partial implementation.

**Mechanical checks** (schema validation, `task-coverage` binary gate, task ID
uniqueness, status transitions) are handled by `cargo make verify-*` /
`bin/sotp track transition` / `bin/sotp task-contract coverage`, not the reviewer.

## What to report

Report findings ONLY for the following categories. Each finding must name a
specific `task_id` or `section.id`, or quote the offending text.

- **task description non-executable**: a `task` whose description lacks one
  of the three elements that make it executable: the target file / symbol,
  the operation to perform, or an anchor cite (`AC-NN` / `IN-NN` / `CN-NN` /
  spec element id). A description that names all three is executable — do
  not require it to also restate the expected behaviour in prose. Distinguish
  from "the description could be shorter" — flag only when an executor would
  have to invent the boundary.
- **upstream restatement**: a task description or plan section that restates
  an upstream ADR's or spec.json's design rationale or behaviour contract in
  prose. Flag the restatement itself regardless of whether an anchor cite
  (`AC-NN` / `IN-NN` / `CN-NN` / spec element id) accompanies it — the
  permitted form is target + operation + anchor cite only. Cite
  `knowledge/conventions/no-upstream-restatement.md`.
- **dependency cycle or wrong ordering**: a task list whose declared
  dependencies form a cycle, or whose declared order would force later tasks
  to refer to artifacts not yet created (e.g., T003 modifies a briefing file
  that T001 should create, but T001 sits after T003 in the section order).
- **task-coverage gap**: an `IN-NN` / `OS-NN` / `CN-NN` / `AC-NN`
  spec element with no task mapping it, **or** a task mapping no spec element.
  The binary gate catches structural absence; the reviewer catches *load-bearing*
  coverage that exists in `task-coverage.json` but whose mapping is implausible
  (e.g., AC-13 mapped to a task whose description has no validation step).
  `GO-NN` elements are NOT coverable in `task-coverage.json` by design: its
  schema has no `goal` section (`TaskCoverageDocument` carries only `in_scope` /
  `out_of_scope` / `constraints` / `acceptance_criteria`, and the codec rejects
  unknown fields), and the plan-artifact-refs verifier intentionally excludes
  goals. Goal-to-task traceability lives in `impl-plan.json` `plan.summary`;
  review that SSoT field, not a generated `plan.md` view.
- **task-contract attribution mismatch**: a `task-contract.json` entry that
  attributes a task to catalogue entries the task description does not actually
  touch, or omits entries the task description claims to add / modify.
  Distinguish from Phase 2 zero-entry tracks where `task-contract.json` is
  intentionally an empty entries map. Every catalogue entry — including
  `action: reference` baseline entries — must carry a task attribution:
  `bin/sotp task-contract coverage` fails closed on `OrphanEntry`. A
  reference-entry attribution names the carrier task whose diff the entry
  rides with; it is not a claim that the task modifies the type, and it must
  not be reported (or stripped) as spurious.
- **batch-size infeasibility**: a single task whose described work would
  *definitely* exceed the per-scope diff ceiling
  (`.harness/config/review-scope.json`: `default_diff_ceiling_lines` or
  per-group override) by a multiple — flag as "split candidate". Do not flag
  tasks merely close to the ceiling; the actual-diff guard handles that
  advisorily.
- **out_of_scope leak into a task**: a task that implements behaviour the spec
  explicitly excludes via `scope.out_of_scope[]`.
- **observations.md mandate without trigger**: an `AC-NN` worded as "must be
  recorded to `observations.md`" but no task carries the recording step
  through to completion (i.e., the AC is uncoverable as written).

## What NOT to report

- Missing `GO-NN` mappings in `task-coverage.json` — the schema has no `goal`
  section; goal traceability belongs to `impl-plan.json` `plan.summary`
- Task attributions for `action: reference` catalogue entries in
  `task-contract.json` — the `OrphanEntry` gate requires them
- Task description wording nits / sentence-length preferences
- Re-ordering suggestions when the existing order is plausibly valid and the
  alternative is purely stylistic
- Suggested task splits when the actual-diff guard has not yet flagged
  overflow and the description fits within one scope's ceiling
- New tasks that should be added to cover hypothetical edge cases not in spec
  — that is spec expansion, not impl-plan refinement
- Status / `commit_hash` validation (CI / `bin/sotp track` enforce this)
- Backward-looking observations (revision count, prior re-plans)
- Type-design objections — those belong to the `types` scope reviewer
- Per-task implementation strategy critique unless it is structurally
  infeasible — the implementer owns the local approach
