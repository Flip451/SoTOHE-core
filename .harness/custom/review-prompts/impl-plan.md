# Impl-Plan Review: Severity Policy

The reviewer's role is **executable-plan soundness review** of
`track/items/<track-id>/impl-plan.json` (Phase 3 SSoT), `task-coverage.json`
(spec ↔ task mapping), `task-contract.json` (task ↔ catalogue-entry attribution),
the rendered `plan.md`, and any `observations.md`. The impl-plan converts spec
elements + type-contract changes into a sequence of executable, individually
committable tasks. Defects here cause wasted implementation effort, broken
ordering, or coverage gaps that surface only after partial implementation.

**Mechanical checks** (schema validation, `task-coverage` binary gate, task ID
uniqueness, status transitions) are handled by `cargo make verify-*` /
`bin/sotp track transition` / `bin/sotp task-contract coverage`, not the reviewer.

## What to report

Report findings ONLY for the following categories. Each finding must name a
specific `task_id` or `section.id`, or quote the offending text.

- **task description non-executable**: a `task` whose description does not
  give an implementer enough information to execute it without re-reading the
  ADR / spec from scratch. Concretely missing: which files must change, what
  the expected behaviour is, or what AC-NN the task closes. Distinguish from
  "the description could be shorter" — flag only when an executor would have
  to invent the boundary.
- **dependency cycle or wrong ordering**: a task list whose declared
  dependencies form a cycle, or whose declared order would force later tasks
  to refer to artifacts not yet created (e.g., T003 modifies a briefing file
  that T001 should create, but T001 sits after T003 in the section order).
- **task-coverage gap**: a `GO-NN` / `IN-NN` / `OS-NN` / `CN-NN` / `AC-NN`
  spec element with no task mapping it, **or** a task mapping no spec element.
  The binary gate catches structural absence; the reviewer catches *load-bearing*
  coverage that exists in `task-coverage.json` but whose mapping is implausible
  (e.g., AC-13 mapped to a task whose description has no validation step).
- **task-contract attribution mismatch**: a `task-contract.json` entry that
  attributes a task to catalogue entries the task description does not actually
  touch, or omits entries the task description claims to add / modify.
  Distinguish from Phase 2 zero-entry tracks where `task-contract.json` is
  intentionally an empty entries map.
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
