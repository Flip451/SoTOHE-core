<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# spec-states commit ゲートを spec 成果物未生成の段階でも通す

## Tasks (0/1 resolved)

### S1 — Skip-gate fix and test coverage

> Single cohesive change: make the track-resolution path of verify spec-states tolerate missing spec artifacts by returning a SKIP outcome instead of an error, and add test coverage for the new skip branch.
> Touch points: build_spec_path_from_track_id (cli-composition/src/verify.rs), verify_spec_states (cli-composition/src/verify.rs), dispatch_spec_states_with_resolver #[cfg(test)] mirror (cli/src/commands/verify.rs).
> Existing evaluation path (spec artifact present) and explicit-path path are both unchanged.

- [ ] **T001**: Fix verify spec-states to skip when spec artifacts are absent on the track-resolution path, and add skip-branch test coverage. Two-site change in non-TDDD layers (cli-composition + cli): (1) In apps/cli-composition/src/verify.rs, change build_spec_path_from_track_id to check both spec.json and spec.md; return Ok(None) (skip-signal) when neither file exists, and Ok(Some(path)) for the existing path when at least one exists. In verify_spec_states, when build_spec_path_from_track_id returns Ok(None), render a SKIP outcome using render_skip("verify spec states", ...) identical in shape to the non-track-branch skip already present in that function, and return Ok(...). When it returns Ok(Some(path)), continue with existing evaluation logic unchanged. (2) In apps/cli/src/commands/verify.rs, apply the same skip-signal logic to the #[cfg(test)] mirror dispatch_spec_states_with_resolver: when the resolver returns Ok(Some(track_id)), check both spec.json and spec.md; if neither exists, call print_skip and return ExitCode::SUCCESS, mirroring the cli-composition change. (3) Add unit tests covering: (a) track-resolution path + both spec artifacts absent => exit 0 + SKIP output; (b) track-resolution path + spec.md present => evaluates normally (delegates to infrastructure::verify::spec_states::verify); (c) track-resolution path + spec.json present (spec.md absent) => evaluates normally (not skipped); (d) explicit-path invocation with a missing file => still returns ExitCode::FAILURE (OS-01 unchanged). All existing tests remain passing. cargo make ci passes (fmt-check + clippy + nextest + deny + check-layers + verify-*).
