# Track Observations

## 2026-07-25 — types re-entry task-contract preflight exception

User adjudication authorizes a track-local exception for the single-scope types
re-convergence after removal of the optional `AdrBaselineSourcePort` catalogue reference entry.

The existing Phase 3 `task-contract.json` still attributes a task to that removed entry.
Consequently, the generic review wrapper's task-contract coverage dependency blocks before the
upstream types reviewer can run, while impl-planner re-entry requires that same types scope to
reach fresh `zero_findings`.

For this re-convergence only:

- retain ADR-baseline, catalogue, catalogue-to-spec signal, semantic-reference, view-freshness,
  and fast/final reviewer checks;
- invoke the canonical reviewer directly, omitting only the stale task-contract coverage/check
  preflight;
- route any type finding back through type-designer rather than editing the catalogue directly;
- regenerate Phase 3 artifacts immediately after types reaches final `zero_findings`;
- end the exception once regenerated `task-contract coverage` and `task-contract check` pass.

The exception does not apply to implementation review, commit, CI, PR review, or merge.
Permanent review/task-contract preflight separation is deferred to a dedicated ADR and track.
