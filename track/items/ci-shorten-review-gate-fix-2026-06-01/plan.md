<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# CI 所要時間の短縮(キャッシュ戦略)+ review/commit ゲートのカタログ未生成耐性

## Summary

This track fixes two independent problems: (1) CI run time ballooned from ~3-4 min to 14-16 min after heavy native dependencies were added; (2) track-active-gate fails hard when type catalogues are absent, blocking the standard Phase 0/1 review and commit flow.
T001 audits the CI timing baseline before any change is made. T002 acts on those findings, modifying only cache-strategy configuration with no Rust source changes. T003 fixes the gate leniency bug by passing lenient: true on the gate path (the mechanism already exists in TypeSignalsRequest). T004 is the final cargo make ci confirmation gate.
T002 and T003 are independent and can be committed separately. T001 must precede T002 (discovery first). T004 depends on both T002 and T003 being merged.

## Tasks (4/4 resolved)

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
> Strict leniency is preserved: the user-facing `sotp track type-signals` command retains lenient: false. New unit tests cover the three required behaviors: (a) absent catalogue skips with warning and exits zero (AC-01, AC-02); (b) present catalogue with red signal blocks (AC-03, CN-02); (c) user-invoked path stays strict. CN-03 (consistency with views sync) and CN-04 (cargo make ci passes) are verified here.

- [x] **T003**: Make track-active-gate lenient on absent catalogues: in apps/cli-composition/src/track/tddd.rs, change the TypeSignalsRequest built for the gate path (called from track-active-gate / track-local-review / track-commit-message) to lenient: true, leaving the user-facing `sotp track type-signals` command at lenient: false. Add unit tests that verify: (a) absent catalogue produces no-op + warning and zero exit; (b) present catalogue with red signal still blocks; (c) user-invoked path remains strict. cargo make ci must pass. (`b28022c3`)

### S4 — Final CI gate

> Run cargo make ci (fmt-check + clippy + nextest + deny + check-layers + verify-*) across the combined T002 + T003 state. Record timing measurements in observations.md to confirm the CI duration target from AC-04 is met. This task produces no source changes; it is the final acceptance checkpoint.

- [x] **T004**: Final integration gate: run cargo make ci (fmt-check + clippy + nextest + deny + check-layers + verify-*) against the combined T002 + T003 changes and confirm all checks pass. Document any residual timing measurements in observations.md. (`b28022c3`)
