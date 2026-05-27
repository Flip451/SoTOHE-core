<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# track-id 引数を省略可能にし、省略時は現在ブランチに紐づくアクティブトラックを既定値とする

## Tasks (0/7 resolved)

### S1 — usecase: port + interactor for active-track resolution

> Introduce the BranchReaderPort secondary port and all associated types (BranchReadError, ActiveTrackResolveError, ActiveTrackResolveService, ActiveTrackResolveInteractor) in libs/usecase/src/track_resolution.rs.
> Re-use the existing resolve_track_id_from_branch for the parse rule so no new resolution logic is introduced (IN-04, IN-05, CN-04, CN-05).
> Include unit tests using a stub BranchReaderPort to verify the resolution cases (track branch, non-track branch, detached HEAD via Some("HEAD"), and no branch via None) without git I/O (IN-10, AC-08).

- [ ] **T001**: usecase: add BranchReaderPort, BranchReadError, ActiveTrackResolveError, ActiveTrackResolveService, ActiveTrackResolveInteractor to libs/usecase/src/track_resolution.rs. BranchReaderPort is the secondary port (current_branch() -> Result<Option<String>, BranchReadError>), passing through Some("HEAD") for detached HEAD and using None only when no branch name can be determined. ActiveTrackResolveInteractor holds Arc<dyn BranchReaderPort> and delegates to the existing resolve_track_id_from_branch for the parse rule (IN-04, IN-05). Implement standard trait impls for both error types (Debug, Display, Error, From). Add unit tests for the new types: (a) track/<id> branch resolves correctly, (b) main branch returns NotTrackBranch, (c) detached HEAD (Some("HEAD") from current_branch) returns DetachedHead, and (d) None from current_branch returns NoBranch. These tests use a stub BranchReaderPort — no git I/O (IN-10, AC-08). Keep changes within libs/usecase/src/track_resolution.rs (CN-04, CN-05).

### S2 — infrastructure: SystemGitRepo implements BranchReaderPort

> Add the BranchReaderPort trait impl for SystemGitRepo in libs/infrastructure/src/git_cli/mod.rs by delegating to the existing GitRepository::current_branch() method and mapping the error to BranchReadError::ReadFailed.
> This completes the port inversion: the usecase layer declares the port; the infrastructure layer provides the adapter (IN-04, IN-06, CN-05).

- [ ] **T002**: infrastructure: add BranchReaderPort impl for SystemGitRepo in libs/infrastructure/src/git_cli/mod.rs. SystemGitRepo::current_branch() already exists on GitRepository; wire it as the BranchReaderPort impl by delegating to that method and mapping the error type to BranchReadError::ReadFailed (IN-04, IN-06, CN-05). Ensure the trait impl is exported from the infrastructure crate public API. Keep changes within libs/infrastructure/src/git_cli/ (one impl block). This task is a prerequisite for T003 (CLI wiring) and T004 (write-guard integration).

### S3 — usecase: replace write-guard closure injection with BranchReaderPort

> TaskOperationInteractor currently injects a raw closure that shells out to git rev-parse --abbrev-ref HEAD for branch validation (active-track-write-guard pattern).
> Replace the closure parameter with Arc<dyn BranchReaderPort> and update the CLI call sites (transition.rs, state_ops.rs) to pass SystemGitRepo (IN-07, CN-05).
> This consolidates validation and default resolution onto the same port-based path as T001-T002, eliminating the remaining direct git shelling-out from usecase-facing handlers.

- [ ] **T003**: usecase: replace the BranchReaderFn closure injection in TaskOperationInteractor with Arc<dyn BranchReaderPort>. TaskOperationInteractor::new currently accepts a closure `move |_items_dir| { Command::new("git").args(["rev-parse","--abbrev-ref","HEAD"]) ... }` for the branch guard (IN-07, CN-05). Replace this with Arc<dyn BranchReaderPort> as the second constructor parameter. Update the internal branch-reading call to invoke BranchReaderPort::current_branch() instead. Update all call sites in apps/cli/src/commands/track/ (transition.rs, state_ops.rs) to pass Arc::new(SystemGitRepo::discover_from(...)) as the BranchReaderPort. This removes the last direct git rev-parse shelling-out from usecase-facing command handlers. Keep changes within libs/usecase/src/task_ops.rs plus the CLI call sites that wire the interactor (IN-07, CN-05).

### S4 — CLI: make track-id optional — positional-arg commands

> Change track_id from String to Option<String> for all positional-arg track commands: Transition, AddTask, SetOverride, ClearOverride, NextTask, TaskCounts, Signals, TypeSignals, TypeGraph, BaselineGraph, ContractMap, CatalogueSpecSignals, CatalogueImplSignals, SpecElementHash, and BaselineCapture.
> Wire ActiveTrackResolveInteractor at the composition root; fail-closed on non-track branch with a user-facing error (IN-01, CN-01, CN-02, AC-01, AC-02, AC-03).
> Consolidate the existing bespoke auto-detect in resolve.rs onto the shared interactor path (IN-09).
> Add command/helper coverage for explicit track-id priority when a value is supplied (IN-10, AC-03).

- [ ] **T004**: CLI batch 1: make track-id optional for positional-arg commands and wire ActiveTrackResolveInteractor. Affected commands in apps/cli/src/commands/track/mod.rs and submodules: TrackCommand::Transition, AddTask, SetOverride, ClearOverride, NextTask, TaskCounts, Signals, TypeSignals, TypeGraph, BaselineGraph, ContractMap, CatalogueSpecSignals, CatalogueImplSignals, SpecElementHash, and BaselineCapture. Change `track_id: String` to `track_id: Option<String>` for each. In the command handler, resolve None via ActiveTrackResolveInteractor (wired at composition root with Arc<SystemGitRepo::discover()>). When Some, use the explicit value directly (CN-02). Add command/helper tests that prove explicit track-id priority is preserved when a value is supplied, alongside the active-branch default tests from T001 (IN-10, AC-03). Also remove the now-redundant individual auto-detect implementation in resolve.rs (ResolveArgs already uses Option<String>) by replacing its bespoke auto-detect with the shared interactor path (IN-09). Fail-closed on non-track branch: error message must prompt the user to provide an explicit track-id (CN-01, AC-01, AC-02, AC-03). Keep under 500 lines total diff; split submodule changes per file if needed for reviewability.

### S5 — CLI: make track-id optional — flag-arg commands and verify subcommands

> Change --track-id and --track flags to Option<String> for all remaining affected commands: Lint, review CodexLocal / ClaudeLocal / Local / CheckApproved / Results / Classify / Files, and verify CatalogueSpecRefs (IN-02, IN-03, CN-01, CN-02).
> Consolidate the bespoke auto-detect implementations in views.rs and verify PlanArtifactRefs onto the shared interactor path (IN-09, AC-04), while preserving track views sync's registry-only fallback when no active track is present.

- [ ] **T005**: CLI batch 2: make track-id optional for the remaining flag-based commands. Affected: (a) flag --track-id commands in apps/cli/src/commands/track/mod.rs and apps/cli/src/commands/review/: Lint (track_id field in the clap struct), review CodexLocal / ClaudeLocal / Local / CheckApproved / Results / Classify / Files (track_id / --track-id fields). Change to Option<String> and add ActiveTrackResolveInteractor wiring at each composition root. (b) flag --track command in apps/cli/src/commands/verify.rs: CatalogueSpecRefsArgs.track field. Change to Option<String>; resolve None via ActiveTrackResolveInteractor. (c) Remove the individual auto-detect implementations in views.rs (ViewAction::Sync --track-id is already Option<String> with bespoke impl) and in verify.rs PlanArtifactRefsArgs (already Option<PathBuf> with bespoke impl), replacing them with the shared interactor path (IN-09). Fail-closed on non-track branch for track-targeted commands and verify plan-artifact-refs (CN-01, AC-01, AC-04); preserve track views sync's registry-only mode when no explicit --track-id is provided and no active track is resolvable, because that path has no track target to fail closed against. Keep under 500 lines per commit; this task may be split across two commits if the diff exceeds that threshold.

### S6 — Makefile: strip shell boilerplate from *-local tasks

> Remove BRANCH=$(git ...); TRACK_ID=${BRANCH#track/}, SPEC_PATH construction, and dead plan/* case arms from verify-plan-artifact-refs-local, verify-catalogue-spec-refs-local, check-catalogue-spec-signals-local, verify-spec-states-current-local.
> Add the missing no-argument active-track path for verify spec-states, then replace the wrappers with bare cargo run calls that let the CLI resolve the track from the branch (IN-08, AC-04, AC-10).

- [ ] **T006**: Makefile + verify CLI cleanup: remove shell boilerplate from *-local tasks. Remove the BRANCH=$(git ...); TRACK_ID="${BRANCH#track/}" pattern, TRACK_DIR / SPEC_PATH construction, and the now-dead plan/* case arms from: verify-plan-artifact-refs-local, verify-catalogue-spec-refs-local, check-catalogue-spec-signals-local, verify-spec-states-current-local. Replace each shell branch-resolve block with a direct `cargo run --quiet -p cli -- <subcommand>` call that passes no pre-resolved track argument/path. For verify-plan-artifact-refs-local, omit --track-dir and rely on the shared active-track resolver path from IN-09; for verify-catalogue-spec-refs-local, omit --track and rely on the optional --track flow from IN-03; for check-catalogue-spec-signals-local, rely on its current-branch lookup after consolidating that lookup onto the shared ActiveTrackResolveInteractor path. For verify-spec-states-current-local, first change `verify spec-states` so its spec path argument is optional; when omitted, resolve the active track via ActiveTrackResolveInteractor and verify `track/items/<id>/spec.md`. On non-track branches, do not preserve skip behavior: let the CLI return a clear fail-closed error and propagate the non-zero exit code (CN-01). Verify this by testing a non-track branch dry-run on main and confirming failure rather than [SKIP]. This satisfies IN-08, AC-04, AC-10 (IN-08, AC-04, AC-10).

### S7 — Integration gate: cargo make ci pass

> Compile, lint, test, and run the full verify-* suite against the changes from T001-T006.
> Fix any issues and confirm all acceptance criteria (AC-01 through AC-10) are satisfied (AC-09).

- [ ] **T007**: Final integration and CI gate. Run cargo make ci (fmt-check + clippy + nextest + deny + check-layers + verify-*) and fix any compilation, lint, or test failures introduced by T001-T006. Verify acceptance criteria: (a) sotp track signals with no track-id on track/<id> branch uses the branch id (AC-01); (b) on main branch without explicit id, the command exits with a clear NotTrackBranch error (AC-02); (c) explicit track-id overrides branch resolution (AC-03); (d) cargo run -p cli -- verify catalogue-spec-refs works without --track on track/<id> branch (AC-04); (e) git rev-parse shelling-out removed from the usecase / CLI path (AC-05, AC-06); (f) track branch create / switch still require explicit track-id — clap reports missing arg (AC-07); (g) unit tests for ActiveTrackResolveInteractor pass (AC-08); (h) cargo make ci passes (AC-09); (i) Makefile plan/* dead case arms are gone (AC-10). No source code changes beyond fixes — this task is gate-only except for trivial fixups (AC-09).
