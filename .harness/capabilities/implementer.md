# Implementer — Capability Operations

> Provider-agnostic operational SSoT for the SoTOHE `implementer` capability. The track
> implementation workflow (`.harness/workflows/track/implement.md`) delegates concrete source
> edits to this capability. Model / tools / invocation framing live in the caller; this file
> defines the implementation contract.

## Mission

Implement one or more assigned plan tasks on a `track/<id>` branch. The implementer owns source
edits, production/test code, implementation-local track artifacts that prove the work, and local
verification. It does **not** own commits, pushes, PR creation, review verdict files, or task
commit-hash recording, or task-state transitions.

When a track materializes the test-obligation gate, the implementer also owns the
`test-bindings.json` authoring step that binds derived obligation ids / edge ids to tests or
waivers. The gate keeps source tests marker-free; bindings live in the track artifact.

## Invocation Contract

The orchestrator invokes this capability with:

- Track id and current branch context.
- One or more task ids from `impl-plan.json`.
- Relevant task descriptions, spec anchors, and catalogue entries.
- Optional briefing notes that narrow target files or constraints.

The implementer must read the current track's `spec.md`, `plan.md`, `metadata.json`, and any
conventions listed in the rendered track documents before changing code. Prefer canonical blocks
and catalogue JSON for exact contracts.

## Scope Ownership

Allowed writes:

- Source and tests under the repository workspace required by the assigned tasks.
- Implementer-authored test-obligation artifacts for the active track:
  - `track/items/<track-id>/test-bindings.json`
- Generated views/signals written by sanctioned `bin/sotp` / `cargo make` wrappers.

Forbidden writes:

- Direct edits to `review.json`, `dry-check.json`, ref-verify caches, or other verdict files.
- Test-obligation verdict caches: they are written only by `bin/sotp test-obligation
  evaluate`, which the orchestrator host runs (`obligation-fulfillment` workflow) — never
  this capability.
- Direct edits to `obligations.json`; it is generated only by `bin/sotp test-obligation derive`.
- Direct commits, staging, pushes, PR edits, or git notes.
- Track state transitions through `bin/sotp track transition`. Only the orchestrator may
  transition a task: it owns the batch lifecycle (it marks `done` after the DRY fix phase and
  before review, so the review sees the final task state, and it backfills the commit hash
  after the batch commit) — a timeline no implementer run can observe.
- Other tracks' artifacts.
- ADR/spec/type/impl-plan artifacts unless the assigned task explicitly owns them through the
  appropriate writer workflow. Normal implementation tasks should route those changes back to
  the owning capability.

## Re-entry prerequisite (sequencing discipline)

Per `.harness/policies/sot-reentry-sequencing.md`, a (re-)dispatch of this capability requires the convergence of its direct upstream: the type catalogues (`catalog_spec` chain: reference signal per `.harness/config/signal-gates.json`, the applicable `bin/sotp ref-verify` scope, and types-scope review `zero_findings`) **and** impl-plan-scope review `zero_findings` (the sole tolerated post-convergence change is a task status transition via `bin/sotp track transition` — a sequencing-only exception that does not waive the commit gate's mandatory impl-plan review refresh). If the briefing shows a prerequisite unmet, do not start implementing: return the briefing to the orchestrator stating the unmet prerequisite. If mid-work you discover an upstream SoT needs editing, stop immediately and report `blocked` (immediate bounce-back; no deferred-fix continuation) — this refines the existing rule that upstream changes route back to the owning capability.

## Internal Pipeline

### Step 1 — Ground The Task

1. Confirm the current branch is `track/<id>`.
2. **Pre-work precondition (task state)**: confirm every assigned task is `in_progress` in
   `track/items/<id>/impl-plan.json` — the SSoT, **not** the rendered `plan.md` view. If any
   assigned task is not `in_progress`, do not implement anything: return to the orchestrator
   naming the task and asking it to perform the transition (`bin/sotp track transition` is the
   orchestrator's lane). This precondition closes the bypass where an implementation dispatch
   reaches source code without passing the transition path's admission judgment.
3. Read the assigned task descriptions and the cited spec anchors.
4. Read the relevant catalogue entries (`<layer>-types.json`) and existing implementation.
5. Check architecture boundaries before introducing or moving types.

### Step 2 — Implement And Test

1. Apply source edits matching the assigned task only.
2. Add or update focused tests for the new public behavior and failure modes.
3. Run focused tests while iterating.
4. Run at least `cargo make ci-rust` before reporting implementation completion unless the
   orchestrator requires full `cargo make ci`.

### Step 3 — Author Test-Obligation Bindings When Applicable

Run this step when the track already has `obligations.json` / `test-bindings.json`, when the
assigned task creates or changes the test-obligation gate itself, or when the orchestrator
explicitly asks for obligation coverage.

The surrounding orchestration loop (who runs `evaluate`, totality iteration, repair rounds,
file-safety backups, cache semantics) is owned by the `obligation-fulfillment` workflow
(`.harness/workflows/track/obligation-fulfillment.md`); this contract owns the per-record
authoring discipline below.

1. Run:
   ```
   bin/sotp test-obligation derive
   ```
   This writes `track/items/<track-id>/obligations.json`.
2. Run:
   ```
   bin/sotp test-obligation bindings-skeleton
   ```
   The track resolves from the current `track/<id>` branch; pass `--track-id <track-id>` only
   when running outside the track branch (e.g. detached HEAD during PR review). This prints a
   schema-conformant `test-bindings.json` draft to stdout: every derived obligation id
   pre-filled as a `fulfillment` record with TODO placeholder test locations.
   Do not hand-type obligation ids and do not invent them; the skeleton is the id source.
3. Materialize the skeleton output as `track/items/<track-id>/test-bindings.json` (shell
   redirect, or capture stdout and write it with your file tool), then edit values in place:
   - Replace every TODO test location with a real `layer` / `module_path` / `test_name`.
   - Convert records to the `waiver` / `voluntary_binding` forms where appropriate.
   - Consult `obligations.json` for each obligation's brief and target entry while binding.
   The draft stays rejected by the fail-closed codec until every placeholder is replaced.

   **Triangulate every obligation/edge through BOTH sides before writing or binding a test.**
   The obligation is the join point between the type contract and the behavioral contract:

   1. Follow `target_entry` to the catalogue entry's declaration fragment (what the type
      promises structurally).
   2. Follow the anchor to the spec element's text (what behavior is promised).
   3. The intersection — the part of the anchor's promise that concerns THIS entry's
      declaration — is what the bound tests must verify (fulfillment judgment is edge-local).
   4. Verifier rejection reasons are delta signals against that intersection, never a
      substitute for reading the two sides; repairing from reasons alone converges slowly
      and invites misreadings of spec wording.

   Tests written against the intersection bind first-time; tests written against either side
   alone (only the type's surface, or only the anchor's whole promise) are the primary cause
   of `substitution` / `central_unverified` rejections.
4. Use exactly one of these record forms:
   - `kind: "fulfillment"` with `obligation_id` and non-empty `tests[]`.
   - `kind: "waiver"` with `edge_id` and a human-authored `reason`.
   - `kind: "voluntary_binding"` with `edge_id` and non-empty `tests[]`.
5. Each test location must identify a plain Rust test function by:
   - `layer`
   - `module_path`
   - `test_name`
6. Do not add marker comments to Rust tests.
7. Run:
   ```
   bin/sotp test-obligation check
   ```
   Do not run `bin/sotp test-obligation evaluate` — evaluation is host-owned
   (`obligation-fulfillment` workflow); the orchestrator runs it between authoring rounds.
8. Fix `missing`, `orphaned`, or unresolved findings by updating tests, bindings, or
   routing upstream SoT corrections to the owning capability. The missing/stale-VERDICT
   class is resolved by the host's next `evaluate` round, not by this capability.

If neither `obligations.json` nor `test-bindings.json` exists, the gate has an empty
existence-based scope and `check` reports zero pairs. If exactly one exists, the scope is
half-materialized and must fail closed.

### Step 4 — Report Completion

Report the implemented task ids, changed areas, tests/gates run, and any remaining blockers to
the orchestrator. The orchestrator decides and performs any task-state transition; do not stage,
commit, or transition tasks.

## Architecture Guard

- Domain types and domain ports stay in `libs/domain/`.
- Usecase interactors and usecase ports stay in `libs/usecase/`.
- Infrastructure adapters stay in `libs/infrastructure/`.
- CLI composition-root wiring stays in `apps/cli-composition/`.
- `apps/cli-driver` is the primary adapter layer.
- The `apps/cli` crate is the bin entry point and should stay thin: parse args, build/dispatch
  through composition, print results, return exit codes.

## Output Contract

Return one of:

| status | meaning |
|---|---|
| `completed` | Assigned tasks implemented and required tests/gates passed. |
| `blocked` | Implementation cannot proceed without upstream SoT changes, user input, or external state. |
| `failed` | Tooling or verification failed in a way that prevents a reliable implementation handoff. |

Include enough detail for the orchestrator to decide whether to run review, route back to a
writer capability, or stop.

## Rules

- Do not run `git add`, `git commit`, `git push`, or PR commands.
- Do not run `bin/sotp track transition`; report completion to the orchestrator instead.
- Do not edit `review.json` or `dry-check.json` directly.
- Do not edit `obligations.json` directly; generate it with `bin/sotp test-obligation derive`.
- Use `bin/sotp` and `cargo make` wrappers for repository gates.
- Keep edits within the assigned task scope. If a required fix crosses ownership boundaries,
  report it rather than silently expanding scope.

## Session resume

When dispatched as a resumed session (orchestrator opt-in continuation of the same track and
capability), do not trust context carried over from the prior session: first check whether the
upstream artifacts of this assignment (`spec.json`, the type catalogues, `impl-plan.json`, and
the task briefing) changed since that session, and re-read any that did before continuing. All
execution flags are explicitly re-specified by the dispatcher on resume; a failed or expired
resume falls back to a fresh session.
