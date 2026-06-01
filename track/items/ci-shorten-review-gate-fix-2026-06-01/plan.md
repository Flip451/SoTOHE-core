<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# CI 所要時間の短縮(キャッシュ戦略)+ review/commit ゲートのカタログ未生成耐性

## Summary

This track fixes two independent problems: (1) CI run time ballooned from ~3-4 min to 14-16 min after heavy native dependencies were added; (2) track-active-gate fails hard when type catalogues are absent, blocking the standard Phase 0/1 review and commit flow. ADR 2026-06-01-1206 further extends the fix by removing the --lenient and --force execution paths entirely, making signal evaluation and baseline capture uniformly simple.
T001 audits the CI timing baseline before any change is made. T002 acts on those findings, modifying only cache-strategy configuration with no Rust source changes. T003 fixes the gate leniency bug by passing lenient: true on the gate path (the mechanism already exists in TypeSignalsRequest). T004 is the final cargo make ci confirmation gate for the T002+T003 work.
T005 removes the --lenient flag entirely so that both the gate path and the direct user-invoked type-signals path share the same unconditional absent-skip behaviour (ADR 2026-06-01-1206 D1). T006 removes the --force flag and the overwrite branch from baseline-capture so that capture is always idempotent (ADR 2026-06-01-1206 D2).
T005 and T006 supersede the partial fix in T003 for their respective concerns: T003 addressed only the gate path, while T005 removes the distinction altogether. T005 and T006 are independent of each other and can be committed separately. T001 must precede T002. T004 depends on T002 and T003. T005 and T006 may be committed after T003.

## Tasks (4/6 resolved)

### S1 — CI cache audit (discovery only)

> Read .github/workflows/ job definitions and Makefile.toml cache configuration. Identify the dominant slow steps (compile, sccache miss, Docker layer pull) and list available cache-strategy levers. Output: observations.md entry. No configuration changes in this task.
> This discovery step is mandatory before T002 to avoid trial-and-error blind changes. The spec deliberately leaves the cache mechanism undecided (OS-03); T001 narrows the candidate mechanisms so T002 can act on evidence.

- [x] **T001**: Audit CI timing baseline: read .github/workflows/ job definitions and Makefile.toml cache configuration to identify which steps dominate the 14-16 min run time and which cache-strategy levers are available (sccache layer, GitHub Actions cache action, Docker layer cache). Produce a written finding in observations.md as input to T002. No file writes beyond observations.md. (`9cef4a08`)

### S2 — CI cache strategy adjustment

> Apply the cache-strategy changes identified in T001. Permitted change surface: .github/workflows/ workflow definitions (cache action configuration, layer cache settings, compose invocation), Docker Compose files/overlays that affect CI cache/target topology, and Makefile.toml cache-related entries. Forbidden: any file under libs/, apps/, Cargo.toml, Cargo.lock, or non-cache application/script logic.
> Acceptance is outcome-level (AC-04): CI duration measurably reduced and cargo make ci passes with no Rust-source diff. The exact mechanism (sccache layer config, GitHub Actions cache action, Docker build cache) is resolved in T001 and applied here; the spec does not prescribe the mechanism (OS-03).

- [x] **T002**: Adjust CI cache strategy: modify only cache-related CI/container configuration in .github/workflows/, Docker Compose files/overlays, and/or Makefile.toml cache settings based on T001 findings. No changes to Rust source (libs/, apps/), Cargo.toml, or Cargo.lock. Acceptance: CI duration measurably shorter than the 14-16 min baseline and cargo make ci passes with zero Rust-source diff. (`d4f767e9`)

### S3 — Gate leniency fix — tolerate absent catalogues in track-active-gate

> The root cause is apps/cli-composition/src/track/tddd.rs line 56: lenient: false is hardcoded for the gate path. The fix sets lenient: true for the gate-path invocation so that TypeSignalsInteractor maps it to MissingCataloguePolicy::SkipSilently, matching the existing views sync behaviour for absent inputs.
> Strict leniency is preserved in this task: the user-facing `sotp track type-signals` command retains lenient: false. New unit tests cover the three required behaviors: (a) absent catalogue skips with warning and exits zero (AC-01, AC-02); (b) present catalogue with red signal blocks (AC-03, CN-02); (c) user-invoked path stays strict. CN-03 (consistency with views sync) and CN-04 (cargo make ci passes) are verified here. Note: T005 later supersedes the gate-vs-direct distinction entirely.

- [x] **T003**: Make track-active-gate lenient on absent catalogues: in apps/cli-composition/src/track/tddd.rs, change the TypeSignalsRequest built for the gate path (called from track-active-gate / track-local-review / track-commit-message) to lenient: true, leaving the user-facing `sotp track type-signals` command at lenient: false. Add unit tests that verify: (a) absent catalogue produces no-op + warning and zero exit; (b) present catalogue with red signal still blocks; (c) user-invoked path remains strict. cargo make ci must pass. (`b28022c3`)

### S4 — Final CI gate (T002+T003)

> Run cargo make ci (fmt-check + clippy + nextest + deny + check-layers + verify-*) across the combined T002 + T003 state. Record timing measurements in observations.md to confirm the CI duration target from AC-04 is met. This task produces no source changes; it is the final acceptance checkpoint for S1-S3.

- [x] **T004**: Final integration gate: run cargo make ci (fmt-check + clippy + nextest + deny + check-layers + verify-*) against the combined T002 + T003 changes and confirm all checks pass. Document any residual timing measurements in observations.md. (`b28022c3`)

### S5 — Remove --lenient flag — unify type-signals absent-skip across all call sites

> ADR 2026-06-01-1206 D1 requires removing the MissingCataloguePolicy enum from domain and the gate-vs-direct branching from type-signals. After T003 the gate path already skips on absent catalogue; T005 removes the distinguishing mechanism (lenient field, policy parameter) so the direct user-invoked path gains the same behaviour without a flag.
> Change surface: domain crate (delete MissingCataloguePolicy enum, update TypeSignalsExecutorPort::evaluate_layer signature), usecase crate (remove lenient field from TypeSignalsRequest), apps/cli-composition (remove --lenient CLI flag and the per-call-site branching). Both call sites emit the same absent-skip warning. Unit tests verify that `sotp track type-signals` on an absent catalogue now skips without error (AC-06).

- [ ] **T005**: Remove the --lenient flag and the gate-vs-direct call-site distinction from type-signals: delete MissingCataloguePolicy enum from domain (libs/), remove the policy parameter from TypeSignalsExecutorPort::evaluate_layer, remove the lenient field from TypeSignalsRequest in usecase (libs/), and update apps/cli-composition to pass no policy argument on either the gate path or the direct user-invoked path. Both call sites now receive the same unconditional absent-skip behaviour. Update or add unit tests to confirm: (a) absent catalogue skips with warning on the direct user-invoked path (AC-06); (b) present catalogue with red signal still blocks (CN-02 preserved); (c) cargo make ci passes.

### S6 — Remove --force flag — make baseline-capture always idempotent

> ADR 2026-06-01-1206 D2 requires removing the --force overwrite path from baseline-capture. Change surface: domain crate (remove force parameter from RustdocBaselineCapturePort::capture), usecase crate (remove force field from BaselineCaptureRequest), infrastructure crate (delete force_capture_rustdoc_baseline_for_layer function and the overwrite branch in the adapter), apps/cli-composition (remove --force CLI flag).
> After this task, re-capture requires deleting the baseline file first, then running capture again (2-step operation). The --source-workspace flag is unaffected and continues to work for main-worktree baseline capture (AC-08). Unit tests verify idempotent behaviour (AC-07) and --source-workspace operation (AC-08).

- [ ] **T006**: Remove the --force flag and the overwrite execution path from baseline-capture: remove the force field from BaselineCaptureRequest in usecase (libs/), remove the force parameter from RustdocBaselineCapturePort::capture in domain (libs/), delete the force_capture_rustdoc_baseline_for_layer free function from infrastructure (libs/), remove the --force CLI flag from apps/cli-composition, and remove the overwrite branch in the infrastructure adapter. Verify that (a) running baseline-capture when a baseline already exists is a no-op (AC-07); (b) baseline-capture --source-workspace continues to work (AC-08); (c) cargo make ci passes.
