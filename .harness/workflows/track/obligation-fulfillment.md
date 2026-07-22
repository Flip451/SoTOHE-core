# Obligation-Fulfillment Workflow SSoT

> Provider-agnostic workflow SSoT for driving a track's test-obligation gate from derived
> obligations to a green `check`. The `implement` workflow (Step 4) and the `full-cycle`
> workflow delegate here for every track holding at least one TDDD catalogue — such a track
> is already enrolled by the `type-design` workflow's mandatory terminal derive step
> (ADR 2026-07-23-0240 D1); enrollment is never decided here. The workflow may also be run
> standalone to close the gate on an existing track. Authoring rules for individual binding
> records live in the `implementer` capability contract
> (`.harness/capabilities/implementer.md` Step 3) — this workflow owns the ORCHESTRATION
> loop, not the per-record discipline, and must not duplicate it.

## Mission

Bring `bin/sotp test-obligation check` to exit 0 for the current track: every derived
obligation bound, every ref edge resolved (tests or waiver), and every resolving verdict
fresh under the current hash triples and verifier fingerprint. The loop is a cooperation
between the **implementer capability** (authors `test-bindings.json` and tests) and the
**orchestrator host** (owns running `evaluate`; the provider-specific sandbox rationale and
launch constraints live in the provider adapters).

## Timing (read first)

- **Canonical point: immediately after Phase 2 (type-design).** `derive`'s inputs
  (catalogues, spec, rules) are fixed there. Deriving early lets implementers write tests
  AGAINST named obligations (briefs in hand), grow bindings incrementally per batch, and pay
  per-batch evaluation costs of minutes. Commit `obligations.json` together with a valid
  (possibly records-empty) `test-bindings.json` so the artifact scope is fully materialized.
- **Retro-fit (post-implementation) is the degenerate mode**: expect a large cold evaluation,
  a high first-pass rejection rate (tests were not written against obligations), and several
  repair loops. Use it only when the gate arrives on an already-implemented track.

## Inputs

- Current `track/<id>` branch.
- `track/items/<id>/obligations.json` and a (possibly records-empty)
  `test-bindings.json` — both materialized by the `type-design` workflow's terminal derive
  step; their absence on a catalogue-bearing track is a fail-closed `check` failure, not a
  state this workflow silently repairs — plus the track's catalogues + `spec.json`
  (triangulation sources).
- **Capability and verifier routing** — `bin/sotp capability exec` resolves the implementer
  profile internally from `.harness/config/agent-profiles.json`; `bin/sotp test-obligation
  evaluate` resolves its verifier profiles internally from the same routing SSoT.

## Sequence

**Step 1: Re-derive on upstream change.** `obligations.json` already exists (type-design
terminal derive). Run `bin/sotp test-obligation derive` (active track branch required) again
only when catalogues / spec / rules changed since the last derivation, so the obligations
reflect the current upstream. Never hand-edit the artifact.

**Step 2: Skeleton (first enrollment only).** The enrollment artifact already exists for a
normal run. Do not regenerate or overwrite it: its existing fulfillment and waiver records
are the batch's incremental authoring history. If a sanctioned recovery has created a missing
bindings artifact, run `bin/sotp test-obligation bindings-skeleton` into a scratch file and
use that schema-pure draft (every obligation id as a `fulfillment` record with TODO test
locations) only as the initial authoring input; validate it before its first materialization.

**Step 3: Author (implementer capability).** Delegate binding authoring to the `implementer`
capability. Its contract owns the record forms, the canonical waiver shapes, and the
**triangulation discipline** (declaration × anchor → entry-relevant intersection). Briefings
must point at the capability contract instead of restating it.

**File-safety discipline (mandatory for every editing round):** copy the current
`test-bindings.json` to a scratch backup before edits; edit a scratch copy; validate JSON;
only then replace the real artifact. A corrupted bindings file is expensive to reconstruct
(it is untracked until the closing commit).

**Step 4: Totality loop (deterministic, sandbox-safe).** Run `bin/sotp test-obligation
check`; it enumerates unresolved edges and existence drifts. The implementer resolves every
edge (`voluntary_binding` with real tests, or a canonical waiver) and repeats until the only
remaining failure class is missing/stale verdicts — that class belongs to Step 5.

**Step 5: Evaluate (orchestrator host).**
`bin/sotp test-obligation evaluate` verifies fulfillment/waiver pairs via the routed LLM
verifiers and freezes verdicts into the caches. Cache semantics governing the loop:

- Verdicts freeze per hash triple; only pairs whose content changed re-verify (cost scales
  with the edit set, not the pair universe).
- Each verdict carries the verifier-prompt fingerprint; a judge/prompt update automatically
  invalidates mismatched entries — never delete or hand-edit the cache files.
- The run includes calibration probes across the three fulfillment-fail categories; a
  calibration failure means the VERIFIER is unhealthy (stop and fix routing/prompt — do not
  "repair" bindings against a broken judge).

**Step 6: Repair loop (reason-driven, triangulation-first).** While `check` still fails:

1. Read each failing pair's cached rejection reason (fulfillment / waiver cache entries).
2. Re-triangulate the edge (capability contract discipline); the reason is a delta signal
   against the entry-relevant intersection, not a substitute for reading both sides.
3. Repair: bind additional existing tests (multi-test records are the norm for multi-branch
   promises), write a focused test where nothing covers the intersection, or convert the
   record to the honest form (canonical waiver ⇄ fulfillment/voluntary) when the prior form
   was wrong for the edge.
4. Re-run Step 5 (diff-only), then `check`. Converge to exit 0.

Keep exactly ONE writer editing `test-bindings.json` per round (no parallel binding editors);
parallelism belongs inside `evaluate`, not in authoring.

## Gates

| Step | Gate | Verdict |
|------|------|---------|
| 4 | `check` reports only the missing/stale-verdict class | proceed to 5 / keep resolving |
| 5 | `evaluate` completes (calibration healthy) | proceed / stop: verifier unhealthy |
| 6 | `bin/sotp test-obligation check` exits 0 | done / loop |

## Failure / recovery

- **Transient provider errors during evaluate** (capacity / rate-limit / launcher races):
  the subprocess runner retries with backoff internally; on a run-level abort, simply re-run
  `evaluate` — frozen verdicts make retries cheap.
- **Calibration failure**: verifier health problem. Check provider/model routing and the
  verifier prompts; after a judge fix, re-run `evaluate` — fingerprint mismatch re-opens
  affected verdicts automatically.
- **Suspected judge misbehavior** (e.g. demands crossing edge-locality): treat as a
  production defect of the verifier prompt, fix via the normal implementation path, and let
  the fingerprint invalidation propagate it. Do not game bindings to satisfy a broken judge.
- **Corrupted `test-bindings.json`**: restore from the round backup (file-safety discipline).
  Last resort: regenerate via Step 2 and re-author — verdict caches survive and any pair
  whose content matches re-attaches without re-verification.
- **Missing enrollment artifacts on a catalogue-bearing existing track**: stop before
  authoring and route the track through the `type-design` workflow's mandatory terminal derive
  step (or its sanctioned migration route). Do not create the artifacts ad hoc in this
  workflow; after that route completes, resume this workflow with both artifacts present.
- **Genuine upstream gaps** (obligation impossible to fulfill or waive honestly): route to
  the owning writer capability (catalogue → type-designer, spec → spec-designer, ADR →
  adr-editor) instead of forcing a binding.

## Constraints

1. `evaluate` is host-owned: only the orchestrator host runs it, never a delegated
   authoring round. The provider-specific sandbox rationale and launch mechanics live in
   the adapter documents (`.agents/skills/track-obligation-fulfillment/SKILL.md`,
   `.claude/commands/track/obligation-fulfillment.md`). All other gate subcommands
   (`derive` on-branch, `check`, `results`, `bindings-skeleton`) may run in delegated
   rounds.
2. Verdict caches are never hand-edited or deleted; validity is governed by hash triples +
   verifier fingerprints (fail-closed).
3. `results` is informational (exit 0); the only pass/fail authority is `check`.

## Outputs

- `track/items/<id>/test-bindings.json` (implementer-authored, committed with the batch)
- `track/items/<id>/obligation-fulfillment-cache.json` / `waiver-cache.json` (frozen verdicts)
- `check` exit 0 — the commit gate's test-obligation precondition holds
- Round metrics worth reporting: per-lane pass/fail counts per round, edit-set sizes,
  evaluation wall-clock (cache-hit ratio evidence)
