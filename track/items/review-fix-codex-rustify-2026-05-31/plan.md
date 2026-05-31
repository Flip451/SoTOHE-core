<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# Codex review-fix-lead wrapper の Rust 化 — cargo make 流出ロジックを Rust 層へ移設

## Summary

Decompose the review-fix-lead Codex fixer from a ~280-line Makefile.toml bash implementation into a hexagonal Rust structure mirroring the existing reviewer architecture.
T001-T002 build the usecase port and infrastructure adapter (D1 foundation + D2 sandbox fix). T003-T004 wire and expose the new capability through cli-composition and the clap CLI. T005 trims the Makefile task to a thin passthrough. T006 performs self-referential dogfooding to validate D2's nested-session fix under real load (D3).
T006 has a hard dependency on T001-T005: the dogfooding only succeeds after the nested reviewer session fix (D2 sandbox config) is fully wired and the sotp make dispatch is in place.

## Tasks (0/6 resolved)

### S1 — Usecase layer — port, DTOs, interactor, and unit tests

> Adds libs/usecase/src/review_v2/run_review_fix.rs with all types from the usecase catalogue: RunReviewFixCommand, RunReviewFixOutput, RunReviewFixError, ReviewFixRunnerError, ReviewFixRunner (secondary port), RunReviewFixService (primary port), RunReviewFixInteractor (Arc<dyn Fn> pattern). Unit tests cover the three outcome scenarios (completed / blocked_cross_scope / failed) with a mock runner.
> Layer constraint: no std::process::* in usecase; public face exposes only stdlib types (String, PathBuf). Mirrors run_review.rs pattern exactly.

- [~] **T001**: Add usecase layer module: RunReviewFixCommand / RunReviewFixOutput / RunReviewFixError / ReviewFixRunnerError structs+enum, ReviewFixRunner secondary port trait, RunReviewFixService primary port trait, RunReviewFixInteractor concrete struct with Arc<dyn Fn> pattern, trait impls, and unit tests (completed / blocked_cross_scope / failed mock scenarios) in libs/usecase/src/review_v2/run_review_fix.rs

### S2 — Infrastructure layer — CodexReviewFixRunner adapter

> Adds libs/infrastructure/src/review_v2/review_fix_runner.rs implementing ReviewFixRunner. Encapsulates all side-effecting concerns: smoke-test (forbidden flag check + codex version range), credential isolation (GITHUB_TOKEN / SSH_AUTH_SOCK / GIT_SSH / GIT_SSH_COMMAND=/bin/false / SSH_CONNECTION / SSH_CLIENT excluded; HOME replaced with temp dir), D2 sandbox config (sandbox_workspace_write.writable_roots=[CODEX_HOME] + network_access=true), codex exec spawning, REVIEW_FIX_STATUS sentinel parsing (full-line match only), and exit-code mapping.
> Unit tests verify smoke-test rejection on forbidden flags and sentinel parsing correctness (including false-sentinel rejection). No test spawns a real codex process.

- [ ] **T002**: Add infrastructure layer module: CodexReviewFixRunner adapter in libs/infrastructure/src/review_v2/review_fix_runner.rs implementing ReviewFixRunner — (1) smoke-test: forbidden sandbox flag env check and codex CLI version range check (>= 0.115.0, < 1.0.0); (2) credential isolation: GITHUB_TOKEN / SSH_AUTH_SOCK / GIT_SSH / GIT_SSH_COMMAND=/bin/false / SSH_CONNECTION / SSH_CLIENT excluded, HOME replaced with temp dir; (3) D2 sandbox config: sandbox_workspace_write.writable_roots=[CODEX_HOME] and network_access=true; (4) codex exec spawning; (5) sentinel parsing (REVIEW_FIX_STATUS full-line match); (6) exit-code mapping; plus unit tests for smoke-test rejection and sentinel parsing

### S3 — CLI composition wiring — provider resolution and CliApp method

> Adds review-fix-lead wiring to apps/cli-composition/src/review_v2/: a new submodule (e.g. run_review_fix.rs) with CliApp::review_run_fix_local that loads agent-profiles.json, calls profiles.resolve_execution("review-fix-lead", round_type), constructs CodexReviewFixRunner, wraps it in a run_fn closure, injects into RunReviewFixInteractor, and calls service.run(command). Exposes RunReviewFixLocalInput as the input struct, following the ReviewRunLocalInput pattern. Currently only the codex provider path is implemented (the spec scope defines only CodexReviewFixRunner); an unsupported-provider error is emitted for any other value.

- [ ] **T003**: Add cli-composition wiring module: apps/cli-composition/src/review_v2/ submodule for review-fix-lead, CliApp::review_run_fix_local method that resolves 'review-fix-lead' capability via profiles.resolve_execution and wires CodexReviewFixRunner, RunReviewFixInteractor, and the run_fn closure; expose RunReviewFixLocalInput and necessary re-exports from cli-composition lib

### S4 — CLI clap subcommand — sotp review fix-local

> Adds apps/cli/src/commands/review/fix_local.rs with FixLocalArgs (7 flags: --scope, --briefing-file, --track-id, --round-type, --reviewer-model, --model, --scope-files) and execute_fix_local dispatching through cli_composition::CliApp::new().review_run_fix_local. Registers ReviewCommand::FixLocal in the review mod.rs dispatch match. No direct use of usecase:: or infrastructure:: from within apps/cli.

- [ ] **T004**: Add apps/cli clap subcommand: apps/cli/src/commands/review/fix_local.rs with FixLocalArgs accepting 7 flags (--scope / --briefing-file / --track-id / --round-type / --reviewer-model / --model / --scope-files), dispatch function calling cli_composition::CliApp::new().review_run_fix_local; register ReviewCommand::FixLocal variant in apps/cli/src/commands/review/mod.rs; no direct use of usecase:: or infrastructure:: in apps/cli

### S5 — Makefile passthrough and sotp make dispatch

> Replaces the ~280-line bash implementation of track-local-review-fix-codex in Makefile.toml with a single-line thin passthrough: script = ["bin/sotp", "make", "track-local-review-fix-codex", "${@}"]. Adds TrackLocalReviewFixCodex variant to MakeTask in apps/cli/src/commands/make.rs and dispatch_track_local_review_fix_codex forwarding to 'review fix-local', following the build_forwarded_args pattern used by dispatch_track_local_review.

- [ ] **T005**: Replace Makefile.toml track-local-review-fix-codex bash implementation with thin passthrough: script_runner calls bin/sotp make track-local-review-fix-codex "$@"; add TrackLocalReviewFixCodex variant to MakeTask enum in apps/cli/src/commands/make.rs; add dispatch_track_local_review_fix_codex forwarding to 'review fix-local'; remove all inline bash logic from the task

### S6 — Dogfooding and CI gate

> Sets .harness/config/agent-profiles.json review-fix-lead.provider=codex. Runs the new fixer on this track's own change set (self-referential dogfooding). Records outcome in observations.md: if Codex rfl completes stably, provider stays at codex; if unstable, reverts to claude. Final cargo make ci gate confirms all CI checks pass.

- [ ] **T006**: Dogfooding: set review-fix-lead.provider=codex in .harness/config/agent-profiles.json; run the new Codex fixer on this track's own change set (self-referential dogfooding per ADR D3); record outcome in track/items/review-fix-codex-rustify-2026-05-31/observations.md (completed / reverted to claude if unstable); verify cargo make ci passes as final gate
