# T017 — acceptance-criteria verification log

## Date: 2026-06-15

## AC-13: full gate pass
- `cargo make ci`: PASSED (fmt-check + clippy + nextest 4361 tests + deny + check-layers + verify-*).
- `bin/sotp dry check-approved --track-id dfp-rfp-loop-cost-reduction-2026-06-13 --items-dir track/items`: APPROVED.
- `bin/sotp review check-approved`: APPROVED (after plan-artifacts re-review).

## AC-01 (dfl zero-violation branch)
- `.claude/agents/dry-fix-lead.md` carries the three-state terminal contract (`completed` / `blocked` / `failed`) used by `/track:dry-check`. Tested indirectly via the per-task DFP runs throughout T010-T016.

## AC-02 (efficient dfl loop)
- D1 (T006) committed: dfl skips `cargo make ci-rust` and `dry write` on no-fix iterations; GateEval telemetry from `dry check-approved` (T007 / IN-07) is emitted with `verdict_str + reason_summary`.

## AC-03 (fixpoint-resolve emits 3-gate composed step)
- T015: `sotp track fixpoint-resolve --track-id <id> --current-branch <branch>` outputs `run-dfp` / `run-rfp scopes=<a,b>` / `run-ref-verify` / `commit` (BTreeSet-ordered, deterministic).

## AC-04 (fixpoint-resolve uses public APIs only)
- `FixpointResolveInteractor` composes `DryCheckApprovalService`, `ReviewGateStatePort`, `RefVerifyGateStatePort` — all are usecase-layer public traits. No infrastructure internals leak into the interactor.

## AC-05 (2-phase dry-write with parallelism + key-ordered records)
- T010: `DryCheckInteractor::run_dry_check` splits into inquiry (collect unverified pairs) + judgment (bounded `std::thread::scope` fan-out via `config.max_parallelism`). Records persist in `DryCheckPairKey` sort order (unit tests in `interactor.rs`).

## AC-06 (max_parallelism reduces wall-clock vs serial baseline)
- Recorded as a known limitation: the codebase has unit tests for the 2-phase / parallel structure; an end-to-end `duration_ms` comparison with a delay-injecting mock agent is NOT included in this track because the integration harness would require additional time-scaling fixtures that risk flakiness in CI. T010 covers the algorithmic correctness; the wall-clock improvement is expected from the bounded fan-out and is exercised in T012's test paths that drive multiple pairs through the agent port.

## AC-07 (2-tier fast/final dry-checker)
- T012: tier-discriminating tests in `libs/usecase/src/dry_check/interactor.rs` prove Final-tier verdicts override provisional Fast verdicts on escalation and Final is invoked on Fast-fail re-run.

## AC-08 (calibration-fail → fail-closed)
- T012: `test_fast_calibration_failure_final_also_fails_returns_calibration_error` confirms `DryCheckCycleError::Agent(Unexpected("calibration failed"))` is returned, coverage manifest is empty (so `check_approved` returns Blocked).

## AC-09 (telemetry round_type + tier model)
- T013: per-tier `ReviewRound` events with `round_type="fast"` / `"final"` and tier-specific model strings. Calibration probes excluded from tier counters.

## AC-10 (check-approved is embed/search-free)
- T003-T005: `DryCheckApprovalInteractor` only reads `DryCheckCoveragePort` + `DryCheckReader`. No `EmbeddingPort`, no `SemanticIndexPort` in its constructor or call graph.

## AC-11 (coverage record write/read)
- T004: `DryCheckInteractor::run_dry_check` writes via `DryCheckCoveragePort` at the end of every run; T003/T005: `check_approved` reads the same record for staleness.

## AC-12 (check-approved is cheap + repeatable)
- AC-10 + AC-11 together: pure-read gate = O(diff fragments) hash compare + O(records) verdict scan. No external subprocess, no LLM, no embedding. Repeated invocations are idempotent.

## Notes
- 4 Yellow type-signals remain (`CodexDryChecker`, `DryCheckConfig`, `DryCheckConfigError`, `FsDryCheckCoverageAdapter`) — these are evaluator-coverage gaps for inherent methods/constructors and do NOT block CI. They are listed for future TDDD evaluator improvements.
