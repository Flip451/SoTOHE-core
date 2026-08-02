# Impl-Plan Review: Severity Policy

The reviewer's role is **executable-plan soundness review** of
`track/items/<track-id>/impl-plan.json` (Phase 3 SSoT), `task-coverage.json`
(spec ↔ task mapping), `task-contract.json` (task ↔ catalogue-entry attribution),
`batch-plan.json` (per-task per-scope estimates + ordered batch declaration),
and any `observations.md`. Rendered views such as `plan.md` are
review-operational context generated from the SSoT, not impl-plan scope/hash
inputs. The impl-plan converts spec elements + type-contract changes into a
sequence of executable, individually committable tasks. Defects here cause wasted
implementation effort, broken ordering, or coverage gaps that surface only after
partial implementation.

**Mechanical checks** (schema validation, `task-coverage` binary gate, task ID
uniqueness, status transitions, batch-plan structural conformance via
`bin/sotp batch-plan check`) are handled by `cargo make verify-*` /
`bin/sotp track transition` / `bin/sotp task-contract coverage` /
`bin/sotp batch-plan check`, not the reviewer. The machinery checks structure
only; the **semantic validity** of indivisibility justifications and batch
composition is deliberately this reviewer's lane — no LLM judgment sits in the
gate path.

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
  `.harness/policies/no-upstream-restatement.md`.
- **dependency cycle or wrong ordering**: a task list whose declared
  dependencies form a cycle, or whose declared order would force later tasks
  to refer to artifacts not yet created (e.g., T003 modifies a briefing file
  that T001 should create, but T001 sits after T003 in the section order).
- **task-coverage gap**: an `IN-NN` / `OS-NN` / `CN-NN` / `AC-NN`
  spec element with no task mapping it, **or** a task mapping no spec element.
  The binary gate catches structural absence; the reviewer catches *load-bearing*
  coverage that exists in `task-coverage.json` but whose mapping is implausible
  (e.g., an acceptance criterion mapped to a task whose description has no validation step).
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
  per-group override) by a multiple — flag as "split candidate". Exception: do
  not flag an over-ceiling task that carries a non-empty indivisibility
  justification you judge valid — that is the designed single-exemption lane;
  review the justification itself under the category below instead. Do not
  flag tasks merely close to the ceiling. **Temporal boundary**: this category
  applies only to tasks whose implementation has NOT started (`todo`). The
  ceiling concept exists in the planning and admission domains only; once a
  task is `in_progress` or done, no review route may apply a line-ceiling lens
  to it.
- **implausible indivisibility justification**: a `batch-plan.json` task whose
  over-ceiling estimate carries a justification that does not actually argue
  indivisibility — e.g., it names convenience or schedule rather than a reason
  the task cannot be split at a behavior boundary, or the described work
  visibly contains independently verifiable behavior units that could stand as
  separate tasks. Quote the justification text. **Same temporal boundary as
  above**: judge justifications only for tasks not yet started. For a task
  whose implementation has started or completed, diff growth from review fixes
  and obligation-mandated tests is the legitimate consequence of correct
  behaviour — never report it as 超過 or demand a retroactive justification.
- **unsound batch composition**: an ordered `batches[]` declaration that
  passes structural checks but remains semantically unsound — e.g., a batch
  grouping tasks with a concrete overlapping write target that prevents safe
  concurrent ownership, or an ordering that defers the sole validation-bearing
  task until after everything it validates has already merged. Do not report
  estimate-sum or declared dependency-order violations here: `bin/sotp
  batch-plan check` owns those structural failures.
- **avoidable batch serialization (P1)**: an ordered `batches[]` declaration
  places tasks in different batches even though they can share a
  dependency-respecting implementation order, have no overlapping write targets,
  and each combined per-scope estimate fits its resolved ceiling or satisfies
  the structural gate's sole-contributor exception for a valid indivisible
  over-ceiling task. A declared
  dependency path does not prevent same-batch membership when its producer runs
  first. Report the specific task ids, the dependency frontier where the first
  task is ready (or the predecessor placement that makes the other ready within
  the batch), their non-overlapping targets and combined estimates, and the
  earlier batch that can contain them. A singleton batch is a finding when
  another task was ready, or became ready through predecessors already in that
  batch, and fit without a concrete write-ownership or per-scope ceiling
  conflict. An indivisibility justification alone does not excuse serialization;
  it only permits the over-ceiling task's sole contribution in its affected
  scope. Do not demand globally optimal bin packing or speculate about unlisted
  conflicts; the evidence must show a concrete removable batch boundary. This
  category applies only while the affected tasks are unstarted (`todo`).
- **pre-implementation split proposal**: when a task bundles multiple
  *independently verifiable behavior units* (each unit testable on its own
  through the spec's acceptance criteria), propose the split **before
  implementation starts**, naming the units and the anchor each would cover.
  This is the one forward-looking category: it exists so ceiling pressure is
  answered by task-boundary splits, not post-hoc justifications.
- **out_of_scope leak into a task**: a task that implements behaviour the spec
  explicitly excludes via `scope.out_of_scope[]`.
- **observations.md mandate without trigger**: an `AC-NN` worded as "must be
  recorded to `observations.md`" but no task carries the recording step
  through to completion (i.e., the AC is uncoverable as written).

## What NOT to report

- Ceiling- or size-derived findings against a task whose implementation has
  started or completed (`in_progress` / any done state) — the ceiling concept
  belongs to the planning and admission domains, and review-fix / DFP diff
  growth is the legitimate consequence of correct behaviour, never 超過. This
  covers batch-size infeasibility, indivisibility-justification validity,
  estimate-sum concerns, and retroactive split demands alike; the one
  forward-looking split lane is the pre-implementation category above, and it
  closes when implementation starts.
- Missing `GO-NN` mappings in `task-coverage.json` — the schema has no `goal`
  section; goal traceability belongs to `impl-plan.json` `plan.summary`
- Task attributions for `action: reference` catalogue entries in
  `task-contract.json` — the `OrphanEntry` gate requires them
- Task description wording nits / sentence-length preferences
- Re-ordering suggestions when the existing order is plausibly valid and the
  alternative is purely stylistic. This exclusion does not apply to an
  `avoidable batch serialization` finding that satisfies its dependency,
  ownership, and ceiling evidence requirements.
- Suggested task splits for a task whose estimates fit within every touched
  scope's ceiling AND whose description does not bundle independently
  verifiable behavior units — splitting there is purely stylistic
- New tasks that should be added to cover hypothetical edge cases not in spec
  — that is spec expansion, not impl-plan refinement
- Status / `commit_hash` validation (CI / `bin/sotp track` enforce this)
- Backward-looking observations (revision count, prior re-plans)
- Type-design objections — those belong to the `types` scope reviewer
- Per-task implementation strategy critique unless it is structurally
  infeasible — the implementer owns the local approach
