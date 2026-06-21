# DRY Violation Snapshot — after (9270de33)

Independent AI-based DRY-violation census (intra-unit + thematic finders -> adversarial verification).
Scope: src/ of the 5 first-party crates (incl. inline #[cfg(test)] modules). Excluded: vendor/** and integration tests/ dirs.

## Overview

| metric | value |
|---|---|
| totalLoc | 199423 |
| unitCount | 52 |
| totalFindings | 168 |
| weightedScore (high*3+med*2+low*1) | 307 |
| densityPerKLoc | 0.842 |
| weightedDensityPerKLoc | 1.539 |
| crossLayerFindings | 21 |
| unverifiedKept | 0 |

## Breakdown by severity

| severity | count |
|---|---|
| high | 26 |
| medium | 87 |
| low | 55 |

## Breakdown by category

| category | count |
|---|---|
| near-clone | 67 |
| structural-dup | 50 |
| exact-clone | 18 |
| semantic-dup | 15 |
| data-dup | 14 |
| knowledge-dup | 4 |

## Breakdown by layer (primary location)

| layer | count |
|---|---|
| infrastructure | 80 |
| usecase | 32 |
| domain | 22 |
| cli-composition | 20 |
| cli | 14 |

## Cross-layer findings — DRY gate blind-spot candidates

- **CODEX_BOT_LOGINS constant and is_codex_bot function duplicated across apps** _[high/exact-clone]_ (`after-005`)
  - apps/cli-composition/src/pr/poll.rs:14-20, apps/cli/src/commands/pr.rs:155-161
- **validate_track_id slug validation logic duplicated in 3 usecase submodules (also shadows domain)** _[high/knowledge-dup]_ (`after-118`)
  - libs/usecase/src/baseline_capture/mod.rs:27-65, libs/usecase/src/catalogue_impl_signals/mod.rs:37-75, libs/usecase/src/type_signals/interactor.rs:28-64, libs/domain/src/ids.rs:232-252
- **Shell launcher metadata table duplicated across usecase and domain layers** _[high/knowledge-dup]_ (`after-131`)
  - libs/usecase/src/hook/test_file_deletion.rs:18-58, libs/domain/src/guard/policy.rs:14-84
- **check_reaction_zero_findings and check_comment_zero_findings duplicated across apps** _[high/near-clone]_ (`after-006`)
  - apps/cli-composition/src/pr/poll.rs:153-220, apps/cli/src/commands/pr.rs:315-385
- **poll_review_for_cycle (~160 lines) near-cloned between cli-composition and cli test_helpers** _[high/near-clone]_ (`after-007`)
  - apps/cli-composition/src/pr/poll.rs:227-399, apps/cli/src/commands/pr.rs:388-588
- **validate_track_id slug-validation logic duplicated across usecase modules** _[high/near-clone]_ (`after-146`)
  - libs/usecase/src/type_signals/interactor.rs:28-64, libs/usecase/src/catalogue_impl_signals/mod.rs:37-75, libs/usecase/src/baseline_capture/mod.rs:27-65, libs/domain/src/ids.rs:232-252
- **Codex fixer spawn-and-collect pattern duplicated across review_fix_runner and dry_fix_runner** _[high/near-clone]_ (`after-165`)
  - libs/infrastructure/src/review_v2/review_fix_runner/spawn.rs:35-141, apps/cli-composition/src/dry_fix_runner.rs:426-515
- **codex --version smoke-test (spawn, parse semver, validate minor >= 115) duplicated** _[high/near-clone]_ (`after-166`)
  - libs/infrastructure/src/review_v2/review_fix_runner/mod.rs:60-103, libs/infrastructure/src/review_v2/review_fix_runner/smoke_test.rs:5-24, apps/cli-composition/src/dry_fix_runner.rs:163-219
- **make_fragment constructor duplicated across 5 test modules** _[high/near-clone]_ (`after-174`)
  - libs/usecase/src/dry_check/shared.rs:425-427, libs/usecase/src/dry_check/interactor.rs:847-849, libs/usecase/src/semantic_dup/interactor.rs:760-762, libs/infrastructure/src/semantic_dup/embedding.rs:669-677, libs/infrastructure/src/dry_check/codex_dry_checker.rs:592-594
- **env --split-string payload detection implemented twice with different approaches** _[high/semantic-dup]_ (`after-132`)
  - libs/usecase/src/hook/test_file_deletion.rs:426-556, libs/domain/src/guard/policy.rs:407-447
- **CODEX_BIN_ENV = "SOTP_CODEX_BIN" defined independently in four modules** _[low/data-dup]_ (`after-185`)
  - libs/infrastructure/src/review_v2/codex_reviewer.rs:25, libs/infrastructure/src/review_v2/review_fix_runner/mod.rs:23, apps/cli/src/commands/review/mod.rs:41, apps/cli/src/commands/plan/mod.rs:21
- **`hex_pattern(byte: u8) -> String` duplicated in usecase and infrastructure** _[low/near-clone]_ (`after-180`)
  - libs/usecase/src/catalogue_spec_refs.rs:662-668, libs/infrastructure/src/tddd/mod.rs:59-65
- **SOTP_GUARDED_GIT token string defined in two places** _[medium/data-dup]_ (`after-130`)
  - libs/usecase/src/hook/guard.rs:16, libs/domain/src/guard/policy.rs:136
- **"tmp/reviewer-runtime" runtime directory string duplicated across four const definitions plus one inline literal** _[medium/data-dup]_ (`after-183`)
  - libs/infrastructure/src/review_v2/codex_reviewer.rs:20, libs/infrastructure/src/review_v2/review_fix_runner/spawn.rs:12, libs/infrastructure/src/dry_check/codex_dry_checker.rs:53, apps/cli/src/commands/review/mod.rs:37, apps/cli-composition/src/dry_fix_runner.rs:417
- **POLL_INTERVAL = Duration::from_millis(50) defined in five separate modules** _[medium/data-dup]_ (`after-184`)
  - libs/infrastructure/src/review_v2/codex_reviewer.rs:21, libs/infrastructure/src/review_v2/claude_reviewer.rs:42, libs/infrastructure/src/dry_check/codex_dry_checker.rs:54, apps/cli/src/commands/review/mod.rs:39, apps/cli/src/commands/plan/mod.rs:19
- **MAX_NESTING_DEPTH = 16 defined independently in domain and infrastructure shell parsers** _[medium/data-dup]_ (`after-186`)
  - libs/domain/src/guard/text.rs:13, libs/infrastructure/src/shell/conch.rs:25
- **append_len_prefixed_bytes duplicated across cli-composition and infrastructure** _[medium/exact-clone]_ (`after-003`)
  - apps/cli-composition/src/dry/corpus_root.rs:41-44, libs/infrastructure/src/dry_check/corpus.rs:123-126
- **normalize_check_status and checks_summary duplicated across apps** _[medium/near-clone]_ (`after-008`)
  - apps/cli-composition/src/pr/poll.rs:97-118, apps/cli/src/commands/pr.rs:163-179
- **InMemoryTrackStore (infrastructure) and StubTrackStore (usecase test) implement the same in-memory Mutex store logic** _[medium/near-clone]_ (`after-148`)
  - libs/infrastructure/src/lib.rs:56-104, libs/usecase/src/lib.rs:346-413
- **`make_hash(hex: &str) -> FragmentContentHash` / `make_file_path` / `make_fragment_ref` cluster duplicated in domain and infrastructure dry_check tests** _[medium/near-clone]_ (`after-179`)
  - libs/domain/src/dry_check.rs:113-123, libs/infrastructure/src/dry_check/store.rs:421-431, libs/domain/src/dry_check/coverage.rs:269-272, libs/infrastructure/src/dry_check/coverage.rs:279-282
- **`git rev-parse --show-toplevel` repo-root resolution duplicated in CLI main** _[medium/semantic-dup]_ (`after-170`)
  - apps/cli/src/main.rs:290-308, libs/infrastructure/src/git_cli/mod.rs:170-195

## Full enumeration

### Severity: high

#### [exact-clone] CODEX_BOT_LOGINS constant and is_codex_bot function duplicated across apps (`after-005`)

- Rationale: The same business rule (which GitHub logins count as the Codex bot) is encoded twice. A new bot login added to one copy but not the other would silently break zero-findings detection or recovery logic in whichever copy is executing. The duplication spans two app crates (cli vs cli-composition).
- Locations: apps/cli-composition/src/pr/poll.rs:14-20, apps/cli/src/commands/pr.rs:155-161
- Fix: Move CODEX_BOT_LOGINS and is_codex_bot into a shared location accessible to both crates — either a new `usecase::pr_review` or `usecase::pr_workflow` public item, or a dedicated `pr_polling` module in `usecase`. Remove the test_helpers copy in apps/cli and call the shared function instead.

#### [exact-clone] Mermaid output assembly block (comment+fence+section loops) duplicated across two renderers (`after-172`)

- Rationale: Three copies spanning two different adapter modules. The section order (classDef → subgraph → edge → class-attach) and the fenced-block format are documented as ADR constraints (IN-18). A future change to that order, or to add a new section, must be applied consistently in all three sites — but there is no compile-time guarantee they stay in sync.
- Locations: libs/infrastructure/src/tddd/contract_map_renderer_adapter/render/mod.rs:326-353, libs/infrastructure/src/tddd/baseline_graph_renderer_adapter/render/mod.rs:202-223, libs/infrastructure/src/tddd/baseline_graph_renderer_adapter/render/mod.rs:410-431
- Fix: Extract an `assemble_mermaid_output(header: &str, class_defs, subgraph_lines, edge_lines, class_attach) -> String` helper (in a shared location accessible by both adapters, e.g. an `infrastructure` internal module) that owns the section ordering. The per-cluster case can pass a dynamic header string.

#### [exact-clone] init_git_repo_on_track_branch test helper duplicated in three test modules (`after-027`)

- Rationale: Three copies of a non-trivial test helper that encodes the same knowledge (minimal git repo setup for write-guard testing). The existing `test_support` module in `apps/cli/src/commands/track/mod.rs` already provides `seed_repo` and `run_git` for exactly this class of shared test infrastructure. Divergence risk is real: if a new git config key (e.g. `init.defaultBranch`) becomes required for CI reproducibility, all three copies must be updated.
- Locations: apps/cli/src/commands/track/tddd/baseline.rs:108-142, apps/cli/src/commands/track/tddd/signals.rs:49-65, apps/cli/src/commands/track/tddd/catalogue_spec_signals.rs:54-61
- Fix: Move `init_git_repo_on_track_branch` into `apps/cli/src/commands/track/mod.rs`'s `test_support` module (alongside the existing `seed_repo`/`run_git` helpers) and import it in each tddd test module.

#### [exact-clone] spawn_codex / drain_pipe / tee_stderr_to_file triplicate in codex reviewer and codex dry-checker (`after-163`)

- Rationale: Three non-trivial functions (>30 lines total) are copy-pasted across two infrastructure adapters. A change to the subprocess spawning strategy (e.g. adding a timeout for the drain thread, changing how the log file is flushed) must be applied in both places. The duplication spans two subsystems of the same architectural layer.
- Locations: libs/infrastructure/src/review_v2/codex_reviewer.rs:322-384, libs/infrastructure/src/dry_check/codex_dry_checker.rs:468-522
- Fix: Extract `spawn_codex`, `drain_pipe`, and `tee_stderr_to_file` into a shared module (e.g. `libs/infrastructure/src/process/codex_spawn.rs`). Parameterise only what differs (the error mapping). Both adapters import from the shared module.

#### [knowledge-dup] Shell launcher metadata table duplicated across usecase and domain layers (`after-131`)

- Rationale: The two tables encode the same real-world policy knowledge (which launchers exist and how to skip their arguments to reach the real command). Divergence is already present: policy.rs handles chrt/taskset/ionice/setsid/chronic while test_file_deletion.rs does not. Adding a new launcher to protect the git-write guard in policy.rs leaves the test-file-deletion guard blind to the same launcher, allowing a bypass like `taskset ff rm tests/foo.rs`. The duplication spans architectural layers (usecase imports domain, but defines its own parallel tables rather than reusing domain data).
- Locations: libs/usecase/src/hook/test_file_deletion.rs:18-58, libs/domain/src/guard/policy.rs:14-84
- Fix: Move the canonical launcher metadata (launcher list, positional-arg counts, per-launcher option tables) into `domain::guard` as pub(crate) or pub constants/functions. `test_file_deletion.rs` can import and reuse them instead of maintaining its own parallel table. The two sets of skip-launcher-args traversal logic can then converge on a shared helper.

#### [knowledge-dup] validate_track_id slug validation logic duplicated in 3 usecase submodules (also shadows domain) (`after-118`)

- Rationale: 3 copies of the same non-trivial validation algorithm within the usecase crate, plus the domain origin. A rule change forces simultaneous edits in 3 usecase modules. The comments in all three copies even acknowledge the duplication ('Mirrors the domain TrackId::try_new validation without importing domain'). The domain already owns this rule — the usecase modules should rely on a shared helper or accept a domain dependency.
- Locations: libs/usecase/src/baseline_capture/mod.rs:27-65, libs/usecase/src/catalogue_impl_signals/mod.rs:37-75, libs/usecase/src/type_signals/interactor.rs:28-64, libs/domain/src/ids.rs:232-252
- Fix: Introduce a single `pub(crate) fn validate_track_id_str(id: &str) -> bool` (or similar) in `libs/usecase/src/lib.rs` or a dedicated `libs/usecase/src/shared/track_id.rs`, then have each interactor call it and map the bool to its own error type. Alternatively, add `domain` as a usecase dependency and call `TrackId::try_new` directly, converting the `ValidationError` to the local error type.

#### [near-clone] Codex fixer spawn-and-collect pattern duplicated across review_fix_runner and dry_fix_runner (`after-165`)

- Rationale: Over 90 lines of subprocess management logic are duplicated. The pattern encodes the same knowledge: how to safely spawn Codex with a sanitised environment, how to concurrently drain stdout/stderr, how to handle stdin-write failures, and how to write a session log. A bug in the error-on-kill path or the log-write call would need to be fixed in both places. The duplication also crosses the infrastructure/cli-composition crate boundary.
- Locations: libs/infrastructure/src/review_v2/review_fix_runner/spawn.rs:35-141, apps/cli-composition/src/dry_fix_runner.rs:426-515
- Fix: Move the spawn-and-collect pattern into a shared infrastructure module (or a new `codex_fixer_spawn` crate). Parameterise redaction and error-type mapping via a closure or trait. `dry_fix_runner` in `cli-composition` should delegate to this shared implementation.

#### [near-clone] Codex subprocess management helpers duplicated between dry_check and review_v2 (`after-063`)

- Rationale: Over 130 lines of subprocess management logic are duplicated. A bug fix or improvement (e.g., timeout handling, pipe draining strategy) must be applied in two places. The project already has a `codex_common` module that factored out `build_codex_read_only_invocation`; the subprocess lifecycle helpers are the natural next candidates for consolidation there.
- Locations: libs/infrastructure/src/dry_check/codex_dry_checker.rs:427-578, libs/infrastructure/src/review_v2/codex_reviewer.rs:255-480
- Fix: Extract the shared helpers (unique-path generation, AutoCleanup RAII, spawn_codex, drain_pipe, tee_stderr_to_file, and the polling loop skeleton) into `crate::codex_common`. Parameterise the error-mapping so both callers can map the shared error type to their respective concrete error variants.

#### [near-clone] Duplicate exclusive-lock acquire pattern in WriteGuard (review_v2) and FsDryCheckStore::acquire_write_lock (`after-155`)

- Rationale: The lock-file convention (`<path>.json.lock` + `fs4` exclusive lock) and the directory-creation safety step are a shared protocol. A change to the convention (e.g. switching to a different lock-file suffix, or adding a timeout) would have to be applied in both places. The dry_check store doc comment literally says it 'mirrors FsReviewStore', confirming intentional copy.
- Locations: libs/infrastructure/src/review_v2/persistence/mod.rs:117-161, libs/infrastructure/src/dry_check/store.rs:188-217
- Fix: Move the lock-acquire boilerplate into a shared `acquire_json_write_lock(path: &Path) -> std::io::Result<std::fs::File>` free function in the infrastructure crate (alongside `atomic_write_file`). Both stores call it and map the `io::Error` to their own error variant.

#### [near-clone] Duplicated items_dir canonicalization + containment-guard block in shared.rs and scope.rs (`after-010`)

- Rationale: This is a security-critical path-traversal guard; having two independent implementations means a fix or tightening of the logic must be applied in both places to remain effective. The two copies already differ in one subtle way (shared.rs uses an items_dir-anchored git discovery; scope.rs uses CWD-based `SystemGitRepo::discover()`), showing that divergence has already started. A single shared helper (e.g. `fn validate_items_dir_under_root(items_dir, canonical_root) -> Result<PathBuf, String>`) would be the correct abstraction.
- Locations: apps/cli-composition/src/review_v2/shared.rs:137-163, apps/cli-composition/src/review_v2/scope.rs:23-49
- Fix: Extract the canonicalize + containment check into a shared private function in `shared.rs` (or a dedicated `path_guard` module) and call it from both `build_v2_shared` and `load_scope_config_only`. Also unify git-discovery strategy: `scope.rs` should use `discover_repo_from_items_dir` instead of CWD-based `SystemGitRepo::discover()` so the two paths stay consistent.

#### [near-clone] ImplTrait unsupported-bounds guard duplicated across three formatters (`after-092`)

- Rationale: Three copies of the same D3 policy guard: adding or removing a supported bound kind requires changes in all three files. A divergence would silently break the fail-closed invariant for one formatter while the others remain correct.
- Locations: libs/infrastructure/src/tddd/signal_evaluator_v2/format/ty_canon.rs:106-115, libs/infrastructure/src/tddd/signal_evaluator_v2/format/ty_strip.rs:88-97, libs/infrastructure/src/tddd/signal_evaluator_v2/format/ty_occ.rs:68-76
- Fix: Extract a `fn impl_trait_is_unsupported(bounds: &[GenericBound]) -> bool` (or reuse the already-existing `bounds_supported` in `where_form.rs` after verifying they implement the same logic) in `ty_base.rs` or `canon.rs`, and call it from all three sites.

#### [near-clone] `make_item` / `empty_generics` rustdoc-types builders duplicated across 4 test modules in the baseline_graph_renderer_adapter subtree (`after-175`)

- Rationale: The three renderer sub-modules are siblings and share no test helper module; each has an exact duplicate of the same 15-line block. A production change to the `rustdoc_types::Item` struct (e.g., adding a new required field) would require patching every copy.
- Locations: libs/infrastructure/src/tddd/baseline_graph_renderer_adapter/render/node_extractor.rs:212-229, libs/infrastructure/src/tddd/baseline_graph_renderer_adapter/render/entry_subgraph.rs:406-423, libs/infrastructure/src/tddd/baseline_graph_renderer_adapter/render/impl_processor.rs:658-675, libs/infrastructure/src/tddd/signal_evaluator_v2/tests.rs:34-51, libs/infrastructure/src/tddd/signal_evaluator_v2/structural_eq.rs:453-455
- Fix: Extract a `test_helpers` module in `libs/infrastructure/src/tddd/baseline_graph_renderer_adapter/render/` exposing `empty_generics()`, `make_item()`, and `make_baseline()`. The four test modules import from it.

#### [near-clone] build_full_prompt duplicated verbatim in ClaudeReviewer and CodexReviewer (`after-069`)

- Rationale: The two blocks are byte-identical (20 lines). Any edit to the review scope prompt template must be applied in both files. This is cross-module change-amplification with a real bug risk: if the two diverge silently (e.g. one gets a new context hint) the Claude and Codex reviewers will receive materially different prompts.
- Locations: libs/infrastructure/src/review_v2/claude_reviewer.rs:129-148, libs/infrastructure/src/review_v2/codex_reviewer.rs:82-101
- Fix: Extract `build_full_prompt(base_prompt: &str, target: &ReviewTarget, scope_label: &str) -> String` as a free function in a shared `review_v2` module (e.g. `mod.rs` or a new `common.rs`). Both reviewer structs call the shared function.

#### [near-clone] capture_rustdoc_baseline_for_layer and capture_baseline_inner duplicate the same baseline-capture pipeline (`after-103`)

- Rationale: Two ~120-line implementations of the same security-critical pipeline exist side-by-side. The symlink guard logic (items_dir + workspace_root) is byte-identical across both files. Any security fix or behavioural change (e.g. idempotency check, format validation, error message wording) must be applied to both. The comment in `rustdoc_baseline_capture_adapter.rs` explicitly acknowledges the mirroring, confirming this is intentional but unresolved duplication. The risk is that a security patch to one file is silently missed in the other.
- Locations: libs/infrastructure/src/tddd/baseline_capture.rs:48-177, libs/infrastructure/src/tddd/rustdoc_baseline_capture_adapter.rs:69-191
- Fix: Consolidate both functions into a single generic inner function parameterised on the error type (or accepting `(items_dir, track_id, workspace_root, baseline_filename, layer_id, targets)` as plain values) that both callers delegate to. The infra-layer caller converts its getter-based binding to plain values; the domain-layer caller passes its public fields directly.

#### [near-clone] check_reaction_zero_findings and check_comment_zero_findings duplicated across apps (`after-006`)

- Rationale: The same business logic for detecting zero-findings signals ('+1' reaction from a Codex bot after the trigger, or a 'Didn't find any major issues' comment) is expressed in two separate app crates. A bug fix or behavioral change (e.g., changing the magic phrase or the timestamp comparison) must be replicated in both copies. Divergence would cause tests to pass while production behavior differs.
- Locations: apps/cli-composition/src/pr/poll.rs:153-220, apps/cli/src/commands/pr.rs:315-385
- Fix: Promote check_reaction_zero_findings and check_comment_zero_findings into the usecase::pr_review crate as public functions. Both apps then call the shared implementation. The test_helpers copies in apps/cli can be deleted.

#### [near-clone] codex --version smoke-test (spawn, parse semver, validate minor >= 115) duplicated (`after-166`)

- Rationale: The minimum acceptable codex version (0.115.0) and maximum major (< 1.0.0) are a single business rule encoded in three places. If the minimum version is bumped, both implementations must be updated independently. The slightly different semver parsers also mean the rule could diverge silently on edge-case version strings.
- Locations: libs/infrastructure/src/review_v2/review_fix_runner/mod.rs:60-103, libs/infrastructure/src/review_v2/review_fix_runner/smoke_test.rs:5-24, apps/cli-composition/src/dry_fix_runner.rs:163-219
- Fix: Move `parse_semver_from_text`, `parse_major_minor`, and the bounds-checking logic into a single shared function (e.g. `infrastructure::process::validate_codex_version`). Both `CodexReviewFixRunner` and `dry_fix_smoke_test_codex_version` delegate to it.

#### [near-clone] convert_raw_to_final / convert_raw_to_fast / require_successful_payload / convert_findings_to_domain are exact copies across codex_reviewer and claude_reviewer (`after-070`)

- Rationale: These four functions encode the business rule for mapping usecase ReviewVerdict → domain Verdict/FastVerdict and for translating usecase ReviewFinding → domain ReviewerFinding. Having two copies means a new ReviewVerdict variant (e.g., a future `RateLimited`) must be handled in both places or one reviewer silently mis-classifies it. The duplication spans two infrastructure adapters and directly derives from the usecase types in the scan unit.
- Locations: libs/infrastructure/src/review_v2/codex_reviewer.rs:177-248, libs/infrastructure/src/review_v2/claude_reviewer.rs:200-271
- Fix: Extract a shared `reviewer_common` module (or a trait with a blanket impl) inside `libs/infrastructure/src/review_v2/` that provides `convert_raw_to_final`, `convert_raw_to_fast`, `require_successful_payload`, and `convert_findings_to_domain` as free functions parameterized over a `ReviewOutcomeRaw`-like trait/struct. Both `CodexReviewer` and `ClaudeReviewer` can then delegate to that shared module.

#### [near-clone] make_fragment constructor duplicated across 5 test modules (`after-174`)

- Rationale: Five copies across two crates (usecase and infrastructure). Any change to how `CodeFragment` test fixtures are constructed requires editing all five. The shared.rs site already exists as a consolidation point — the other four should delegate to it or to a shared test utility.
- Locations: libs/usecase/src/dry_check/shared.rs:425-427, libs/usecase/src/dry_check/interactor.rs:847-849, libs/usecase/src/semantic_dup/interactor.rs:760-762, libs/infrastructure/src/semantic_dup/embedding.rs:669-677, libs/infrastructure/src/dry_check/codex_dry_checker.rs:592-594
- Fix: Promote the helper in `libs/usecase/src/dry_check/shared.rs` (or a new `libs/domain/src/semantic_dup/test_support.rs`) to be the single canonical factory, and have the other four sites import it.

#### [near-clone] poll_review_for_cycle (~160 lines) near-cloned between cli-composition and cli test_helpers (`after-007`)

- Rationale: This is the most critical duplication: ~160 lines of polling, timeout-recovery, and zero-findings logic are copy-maintained across two crates. The tests in apps/cli/src/commands/pr_tests.rs exercise the test_helpers copy rather than the production cli-composition copy, so a bug fixed in cli-composition/src/pr/poll.rs would not be caught by those tests. Historical precedent (PR #143, T005 comments in poll.rs) shows this code has already required non-trivial fixes.
- Locations: apps/cli-composition/src/pr/poll.rs:227-399, apps/cli/src/commands/pr.rs:388-588
- Fix: Delete the test_helpers::poll_review_for_cycle copy. The tests in pr_tests.rs that import poll_review_for_cycle via 'use super::poll_review_for_cycle' should be migrated to import and test the cli_composition production function directly, or relocated to apps/cli-composition where they can exercise the real implementation.

#### [near-clone] review_run_codex vs review_run_claude — duplicated composition body (`after-160`)

- Rationale: ~60 lines of composition logic duplicated verbatim across two methods. Any change to the review composition flow (e.g. new validation step, new telemetry field, changed warning text) must be propagated to both. The review_run_local method at line 271 already partially factors this out (for the auto-resolve path) and shows the pattern can be extracted. Bug risk: the WARN message for unsafe briefing path (lines 127-131 and 206-210) is byte-identical — a future fix will need to update both.
- Locations: apps/cli-composition/src/review_v2/mod.rs:115-183, apps/cli-composition/src/review_v2/mod.rs:194-258
- Fix: Extract a private run_reviewer_impl function taking the shared fields (track_id, group, items_dir, briefing_file, prompt, timeout, round_type) plus a ReviewerKind enum or a dyn ReviewerTrait argument. Both review_run_codex and review_run_claude become thin delegators that construct the Reviewer and call the shared impl.

#### [near-clone] validate_track_id duplicated across 3 usecase modules (+ 3 CLI copies) (`after-126`)

- Rationale: Three copies of the same non-trivial validation rule (slug grammar with 5 distinct error conditions) live in the same architectural layer. Any grammar change is a 3-site edit with no compiler help to keep them in sync. Additional copies also exist in the CLI layer (apps/cli/src/commands/track/validate.rs:15-43, apps/cli-composition/src/track/mod.rs:12-40, apps/cli-composition/src/verify.rs:199-227), making the total count at least 6. Severity is high because: (a) there are 3+ copies within the usecase layer alone, and (b) divergence would silently produce different validation behaviour depending on which use case is invoked.
- Locations: libs/usecase/src/catalogue_impl_signals/mod.rs:37-75, libs/usecase/src/baseline_capture/mod.rs:27-65, libs/usecase/src/type_signals/interactor.rs:28-64
- Fix: Extract the slug validation logic into a shared usecase-internal utility (e.g. `libs/usecase/src/shared/track_id_validation.rs`) that accepts a closure or generic error constructor, or — preferably — expose `domain::TrackId::try_new` validation as a pure domain function that can be called from all usecase modules without the full domain dep, so there is a single canonical source of the slug grammar.

#### [near-clone] validate_track_id slug-validation logic duplicated across usecase modules (`after-146`)

- Rationale: Four copies (three in usecase, one in domain) encode the same validation rule with identical character-by-character logic and the same error messages. The copies are self-aware — each contains a comment saying it mirrors domain — but the self-awareness does not prevent drift. Any rule change forces parallel edits. The domain already owns the canonical predicate; usecase modules could delegate validation to domain::ids::TrackId::try_new instead of re-implementing it.
- Locations: libs/usecase/src/type_signals/interactor.rs:28-64, libs/usecase/src/catalogue_impl_signals/mod.rs:37-75, libs/usecase/src/baseline_capture/mod.rs:27-65, libs/domain/src/ids.rs:232-252
- Fix: Expose a domain-level predicate (e.g. `domain::ids::TrackId::is_valid(id: &str) -> bool`) or a fallible constructor that usecase callers can invoke and map to their own error types via a one-liner. Remove the three inline copies in usecase crates.

#### [semantic-dup] Duplicate is_valid_rust_identifier with behavioral divergence (`after-051`)

- Rationale: Different implementations of the same business rule (what constitutes a valid Rust identifier) are scattered across two files in the same crate. A change to the underscore rule requires modifying both locations, and the existing divergence is a latent inconsistency bug (EnumVariantDeclaration rejects `"_"`, but TypeName/MethodName created via catalogue_v2/identifiers.rs do not).
- Locations: libs/domain/src/tddd/catalogue.rs:48-60, libs/domain/src/tddd/catalogue_v2/identifiers.rs:58-67
- Fix: Move the single canonical implementation to `catalogue_v2/identifiers.rs` (or a new `identifier_utils` sub-module), add the bare-`"_"` check there, and re-export or call it from `catalogue.rs`. `EnumVariantDeclaration::try_new` in `catalogue.rs` can then call the shared function and delete its local copy.

#### [semantic-dup] SHA-256 to lowercase hex encoding implemented three times within dry_check (`after-064`)

- Rationale: Three independent reimplementations of the same one-line operation. The `codex_dry_checker.rs` copy uses `{hash_bytes:x}` (the GenericArray Display impl) which may or may not produce per-byte zero-padded output depending on the sha2 version, making it a latent correctness divergence in addition to a DRY violation. `corpus::sha256_hex` is already public within the module; `config.rs` and `codex_dry_checker.rs` should call it.
- Locations: libs/infrastructure/src/dry_check/corpus.rs:117-121, libs/infrastructure/src/dry_check/config.rs:281-283, libs/infrastructure/src/dry_check/codex_dry_checker.rs:259-261
- Fix: Replace the inline SHA-256 hex encoding in `config.rs` and `codex_dry_checker.rs` with a call to the already-public `crate::dry_check::corpus::sha256_hex`. Alternatively, move `sha256_hex` into a shared utility module (e.g., `crate::codex_common` or a new `crate::util`) to make the sharing intent explicit.

#### [semantic-dup] Three-way duplication of the latest-per-pair map derivation algorithm (`after-127`)

- Rationale: Three copies of a non-trivial domain-semantic rule (how to resolve history into current state). Any change to the rule — e.g., a tie-breaker on recorded_at, a filter by config_fingerprint, or a type substitution — must be applied in all three places. Prior round-6 P1 fix commentary in interactor.rs already shows that the rule was changed once (fingerprint-filtered verified_set) and diverged from the approval_interactor's version, demonstrating real-world divergence risk.
- Locations: libs/usecase/src/dry_check/interactor.rs:208-210, libs/usecase/src/dry_check/approval_interactor.rs:156-158, libs/usecase/src/dry_check/results_interactor.rs:65-67
- Fix: Extract a free function `fn build_latest_per_pair(records: Vec<DryCheckRecord>) -> BTreeMap<DryCheckPairKey, DryCheckRecord>` in `shared.rs` and call it from all three interactors. The function body is 3 lines; consolidation is straightforward.

#### [semantic-dup] env --split-string payload detection implemented twice with different approaches (`after-132`)

- Rationale: The same tokenisation rule (env -S/--split-string flag recognition) is encoded twice. If the rule changes (e.g., to handle a new env flag form), both implementations must be updated. A divergence would mean one guard catches a bypass that the other misses. The two implementations already differ subtly in how they handle bundled short flags, creating a surface for undetected bypasses.
- Locations: libs/usecase/src/hook/test_file_deletion.rs:426-556, libs/domain/src/guard/policy.rs:407-447
- Fix: Extract the env split-string flag recognition into a shared function in `domain::guard` (possibly as part of a `ShellParser`-adjacent utility or a new `domain::guard::env_split` module). Both the policy check and the test-file-deletion traversal can then call the single authoritative implementation.

### Severity: medium

#### [data-dup] "tmp/reviewer-runtime" runtime directory string duplicated across four const definitions plus one inline literal (`after-183`)

- Rationale: Five independent occurrences of the same path string. Renaming the scratch directory (e.g. for security or ergonomic reasons) requires hunting down all five sites. The constant should live once, possibly in a shared `infrastructure::runtime_dirs` module or in the usecase layer, and be imported everywhere else.
- Locations: libs/infrastructure/src/review_v2/codex_reviewer.rs:20, libs/infrastructure/src/review_v2/review_fix_runner/spawn.rs:12, libs/infrastructure/src/dry_check/codex_dry_checker.rs:53, apps/cli/src/commands/review/mod.rs:37, apps/cli-composition/src/dry_fix_runner.rs:417
- Fix: Declare one `pub const REVIEWER_RUNTIME_DIR: &str = "tmp/reviewer-runtime"` in a shared location (e.g. `libs/infrastructure/src/runtime_dirs.rs` or `libs/usecase/src/runtime_dirs.rs`) and replace all four private const definitions and the one inline literal.

#### [data-dup] Grounding fields (spec_refs + informal_grounds) repeated on all three entry types (`after-056`)

- Rationale: Three copies of the same two-field grounding group. Adding or renaming a grounding field requires synchronized edits to all three entry structs.
- Locations: libs/domain/src/tddd/catalogue_v2/entries.rs:52-58, libs/domain/src/tddd/catalogue_v2/entries.rs:103-110, libs/domain/src/tddd/catalogue_v2/entries.rs:154-161
- Fix: Extract a `GroundingFields { spec_refs: Vec<SpecRef>, informal_grounds: Vec<InformalGroundRef> }` value object and embed it as `pub grounding: GroundingFields` in each entry type (or use a trait `HasGrounding`). This centralizes the knowledge that grounding is a fixed pair of fields.

#### [data-dup] MAX_NESTING_DEPTH = 16 defined independently in domain and infrastructure shell parsers (`after-186`)

- Rationale: Two implementations of the same guard policy (shell nesting depth limit) embed the same numeric threshold. If the limit is ever changed for security or correctness reasons, both files must be updated in sync. The threshold should be expressed once, ideally as a `pub const` in the domain's `guard` module, and imported by the infrastructure implementation.
- Locations: libs/domain/src/guard/text.rs:13, libs/infrastructure/src/shell/conch.rs:25
- Fix: Promote `MAX_NESTING_DEPTH` to `pub const` in `libs/domain/src/guard/` (e.g. in `libs/domain/src/guard/mod.rs` or `verdict.rs`) and remove the private redefinition in `libs/infrastructure/src/shell/conch.rs`, importing the domain constant instead.

#### [data-dup] POLL_INTERVAL = Duration::from_millis(50) defined in five separate modules (`after-184`)

- Rationale: Five copies of the same tuning parameter. Any latency tuning (e.g. bumping to 100 ms) requires changing all five files. The interval is a shared operational policy for all subprocess-polling loops and belongs in one place.
- Locations: libs/infrastructure/src/review_v2/codex_reviewer.rs:21, libs/infrastructure/src/review_v2/claude_reviewer.rs:42, libs/infrastructure/src/dry_check/codex_dry_checker.rs:54, apps/cli/src/commands/review/mod.rs:39, apps/cli/src/commands/plan/mod.rs:19
- Fix: Extract to a single `pub const SUBPROCESS_POLL_INTERVAL: Duration` in a shared infrastructure or usecase module (e.g. `libs/infrastructure/src/subprocess.rs`) and import it in all five sites.

#### [data-dup] REVIEW_RUNTIME_DIR constant "tmp/reviewer-runtime" defined in two places (`after-071`)

- Rationale: Two definitions of the same path constant in different modules. A directory rename (e.g. during CI layout changes) requires finding both; the compiler gives no guidance since both are string literals of the same type.
- Locations: libs/infrastructure/src/review_v2/codex_reviewer.rs:20, libs/infrastructure/src/review_v2/review_fix_runner/spawn.rs:12
- Fix: Hoist `REVIEW_RUNTIME_DIR` to `libs/infrastructure/src/review_v2/mod.rs` as `pub(super) const` (or `pub(crate) const`), then reference it from both `codex_reviewer` and `review_fix_runner/spawn`.

#### [data-dup] SOTP_GUARDED_GIT token string defined in two places (`after-130`)

- Rationale: A single conceptual datum (the token string) is duplicated across the usecase and domain layers. Divergence would silently break the guard: one layer would scan for the old token while the other scanned for the new one, leaving a bypass window open.
- Locations: libs/usecase/src/hook/guard.rs:16, libs/domain/src/guard/policy.rs:136
- Fix: Promote the constant to `domain::guard` (e.g. `domain::guard::GUARDED_GIT_TOKEN`) and re-export it. Both `guard.rs` and `policy.rs` then reference the single definition.

#### [data-dup] TRACK_ITEMS_DIR / TRACK_ARCHIVE_DIR redefined in three infrastructure submodules (`after-182`)

- Rationale: Four independent definitions of the same filesystem path pair. A directory restructure (e.g. renaming `track/items` to `track/active`) requires four const edits plus all call sites that use the raw string literal instead of the constant. The constants belong in a single `infrastructure::paths` or `infrastructure::track` module and should be re-exported to the verify submodules.
- Locations: libs/infrastructure/src/track/render/mod.rs:33-34, libs/infrastructure/src/verify/latest_track.rs:14-15, libs/infrastructure/src/verify/tech_stack.rs:12-13, libs/infrastructure/src/verify/view_freshness.rs:10
- Fix: Promote both constants to `pub(crate)` in `libs/infrastructure/src/track/render/mod.rs` (or move them to a new `libs/infrastructure/src/paths.rs`), then `use` them from the three verify submodules.

#### [exact-clone] Duplicate ScopeName resolution block in briefing.rs (`after-009`)

- Rationale: Both functions perform exactly the same string-to-ScopeName mapping before calling `scope_config.briefing_file_for_scope(&scope)`. A single private helper (e.g. `parse_scope_name(scope_name: &str) -> Result<ScopeName, String>`) would eliminate the duplication. Divergence risk: a future change to how `"other"` is handled (e.g. case-insensitive) or to the error message format would require updating two sites.
- Locations: apps/cli-composition/src/review_v2/briefing.rs:36-45, apps/cli-composition/src/review_v2/briefing.rs:90-99
- Fix: Extract the shared block into a private `fn parse_scope_name(scope_name: &str) -> Result<ScopeName, String>` in `briefing.rs` (or promote it to `scope.rs` where a similar parse already exists for `validate_scope_for_track_str`), and call it from both functions.

#### [exact-clone] EnvVarGuard + env_lock test helper duplicated between plan/tests.rs and review/tests.rs (`after-021`)

- Rationale: 33-line block of test infrastructure encoding the same 'serialize env-var mutation via a mutex + RAII restore' pattern. Bug fixes or safety comment updates must be applied twice.
- Locations: apps/cli/src/commands/plan/tests.rs:13-45, apps/cli/src/commands/review/tests.rs:19-51
- Fix: Move EnvVarGuard and env_lock into a shared test-utility module (e.g. apps/cli/src/test_utils.rs gated with #[cfg(test)]) and import from both test files.

#### [exact-clone] FastEmbedAdapter + LanceDbSemanticIndexAdapter initialization block duplicated in check.rs and find_similar.rs (`after-014`)

- Rationale: The error message strings ('failed to load embedding model: {e}', 'failed to open index at {}: {e}') are byte-identical. Any change to adapter construction (e.g. adding a config parameter or changing the error type) must be made in two places. Both files import the same two adapter types for the same purpose.
- Locations: apps/cli-composition/src/semantic_dup/check.rs:155-161, apps/cli-composition/src/semantic_dup/find_similar.rs:108-114
- Fix: Extract a shared helper in common.rs such as `pub(super) fn make_embedding_and_index_ports(db_path: &PathBuf) -> Result<(Arc<FastEmbedAdapter>, Arc<LanceDbSemanticIndexAdapter>), String>` and call it from both handlers.

#### [exact-clone] FindSimilarError and DupCheckError are byte-identical enums (`after-143`)

- Rationale: Two copies of an identical composite error definition. Any addition of a new error variant (e.g. an `Io` arm) or change to the error representation model must be applied twice. The duplication signals that both use-cases share the same error knowledge but express it redundantly.
- Locations: libs/usecase/src/semantic_dup/errors.rs:89-98, libs/usecase/src/semantic_dup/errors.rs:100-109
- Fix: Introduce a single `SemanticOpError { Embedding(EmbeddingError), Index(SemanticIndexError) }` composite and type-alias or re-export it as `FindSimilarError` and `DupCheckError`, or unify them into one shared type used by both interactors.

#### [exact-clone] Signal column lookup+render block copy-pasted into two loops in the same function (`after-171`)

- Rationale: The lookup logic (predicate, emoji mapping, default) is identical in both loops. Any change to the lookup semantics — e.g. switching from first-match to a different key — must be applied in both places, with no compiler guard against divergence.
- Locations: libs/infrastructure/src/type_catalogue_render.rs:471-483, libs/infrastructure/src/type_catalogue_render.rs:543-554
- Fix: Extract a free function `render_signal_col(type_signals: Option<&[TypeSignal]>, name: &str, sig_kind_tag: &str) -> String` that encapsulates the lookup and returns the rendered column cell. Call it from both loops.

#### [exact-clone] append_len_prefixed_bytes duplicated across cli-composition and infrastructure (`after-003`)

- Rationale: Byte-identical function body across two crates. cli-composition already imports from infrastructure::dry_check::corpus (sha256_hex is already used via that import path at corpus_root.rs line 13). The duplication spans architectural layers (cli-composition → infrastructure), meaning the encoding rule is expressed in two places.
- Locations: apps/cli-composition/src/dry/corpus_root.rs:41-44, libs/infrastructure/src/dry_check/corpus.rs:123-126
- Fix: Make `append_len_prefixed_bytes` pub(crate) or pub in `libs/infrastructure/src/dry_check/corpus.rs` and remove the copy from `corpus_root.rs`, calling the infrastructure version instead.

#### [exact-clone] build_prompt duplicated between plan and review codex_local (`after-020`)

- Rationale: Identical 11-line function body encoding the same briefing-to-prompt conversion rule. A divergence (e.g. changing the prompt wording in one place) would produce silently inconsistent CLI behaviour between sotp plan and sotp review.
- Locations: apps/cli/src/commands/plan/codex_local.rs:53-64, apps/cli/src/commands/review/codex_local.rs:54-65
- Fix: Extract a free function `build_briefing_prompt(briefing_file: Option<&PathBuf>, prompt: Option<&String>) -> Result<String, String>` in a shared CLI utilities module and call it from both sites.

#### [exact-clone] sibling_spec_json helper duplicated across four verifier modules (`after-115`)

- Rationale: Four exact copies of a small helper. Any bug fix or policy change (e.g., how to handle an empty parent) must be applied four times. The function is a pure path computation with no layer-specific knowledge, making consolidation straightforward.
- Locations: libs/infrastructure/src/verify/spec_attribution.rs:96-101, libs/infrastructure/src/verify/spec_frontmatter.rs:81-86, libs/infrastructure/src/verify/spec_signals.rs:309-314, libs/infrastructure/src/verify/spec_states.rs:466-471
- Fix: Extract `sibling_spec_json` into a shared `verify::helpers` or `verify::frontmatter` module (the existing `super::frontmatter` submodule is a natural home) and import it from all four callers.

#### [exact-clone] validate_track_id_str duplicated in verify.rs (local copy) vs track/mod.rs (canonical) (`after-001`)

- Rationale: Byte-identical 28-line function duplicated in two files of the same crate. Any rule change (valid chars, error messages) must be applied in both places; the second site is already bypassed by every other caller in the module that uses the track-module version.
- Locations: apps/cli-composition/src/track/mod.rs:12-40, apps/cli-composition/src/verify.rs:199-227
- Fix: Delete `validate_track_id_str_local` from `verify.rs` and replace its single call site (line 169) with `crate::track::validate_track_id_str(&track_id).map_err(|e| format!("invalid track ID: {e}"))?;`.

#### [exact-clone] write_architecture_rules test fixture duplicated across two test modules (`after-028`)

- Rationale: The data-shape (test_layer fixture with catalogue_spec_signal) is a shared test contract between the two verify commands. When the architecture-rules schema evolves (e.g. a new required field, a changed key name), both copies must be updated in sync. A test utility module would make the shared contract explicit.
- Locations: apps/cli/src/commands/track/tddd/catalogue_spec_signals.rs:109-133, apps/cli/src/commands/verify_catalogue_spec_refs.rs:62-86
- Fix: Extract this fixture into a shared test utility or a test-only constant/function accessible from both test modules.

#### [knowledge-dup] ADR lifecycle status string set encoded in both `adr_decision/parse.rs` (decode) and `ref_verify/pair_source.rs` (encode) (`after-059`)

- Rationale: The five status strings are a closed set defined by the ADR lifecycle convention (`knowledge/conventions/adr.md`). Both the parse codec and the pair-source serializer must agree on the exact strings; they currently do so by independent hard-coding. A new status (or a rename) would require consistent edits in both files with no compile-time enforcement that they stay in sync. The domain enum `AdrDecisionEntry` is the natural single source of truth for this mapping.
- Locations: libs/infrastructure/src/adr_decision/parse.rs:197-227, libs/infrastructure/src/ref_verify/pair_source.rs:244-252
- Fix: Add a `fn as_status_str(&self) -> &'static str` method (or a `Display` impl) to `AdrDecisionEntry` in `libs/domain/src/adr_decision/entry.rs`. Both `parse.rs` and `pair_source.rs` then delegate to that method, and exhaustiveness is enforced by the compiler at the single definition site. Alternatively, derive `serde::Serialize` / `Deserialize` on `AdrDecisionEntry` using `#[serde(rename = "...")]` annotations on each variant (infrastructure-side newtype wrapper, keeping CN-05 compliance).

#### [near-clone] Active-track guard (CN-07) duplicated in two interactors (`after-120`)

- Rationale: Both guards encode the same business rule: 'branch must be track/<track_id>'. A rule change (e.g. allowing `feature/` branches) requires edits in both files. The error variant payloads are structurally identical. A shared free function `check_active_track_branch(branch, track_id) -> Result<(), ActiveTrackError>` would eliminate the duplication.
- Locations: libs/usecase/src/catalogue_spec_signals.rs:171-180, libs/usecase/src/catalogue_spec_refs.rs:169-178
- Fix: Extract a free function (e.g. `fn check_active_track_branch(branch: &str, track_id: &TrackId) -> Result<(), ActiveTrackBranchError>`) in a shared usecase sub-module and call it from both interactors, converting the returned error into each workflow's own error type.

#### [near-clone] AdrAnchor and ConventionAnchor: identical non-empty-string newtype scaffold (`after-046`)

- Rationale: Both types encode the exact same business rule (anchor must be non-empty after trimming) with the same implementation. A shared macro or a generic validated-string helper could capture the rule once. The distinct newtype names are intentional for type-safety, but the validation logic itself is duplicated knowledge.
- Locations: libs/domain/src/plan_ref/adr_ref.rs:12-38, libs/domain/src/plan_ref/convention_ref.rs:11-41
- Fix: Introduce a private macro (e.g. `validated_anchor_newtype!`) or a generic `NonEmptyStr<E>` newtype parameterised by the error variant, used by both AdrAnchor and ConventionAnchor. The distinct public names and error variants are preserved; only the implementation body is deduplicated.

#### [near-clone] B-side impl re-insertion into S duplicated across orphan-impl pass and Step 5.5 Reference recovery (`after-097`)

- Rationale: A change to the child-insertion protocol (e.g. how `s_actions` is populated, what happens when `b_id_remap` misses a key, or adding a new per-child operation) must be made in both places. The block is ~18 lines of non-trivial stateful logic and has already diverged in one small detail (root insertion uses `insert` in both but the outer id-resolution differs: `b_id_remap.get(&impl_id)` vs the pre-resolved `s_id`). A shared helper such as `reinsert_b_impl_into_s(state, b, b_impl_item, target_s_id)` would eliminate the duplication.
- Locations: libs/infrastructure/src/tddd/signal_evaluator_v2/phase1/builder/main_fn.rs:98-128, libs/infrastructure/src/tddd/signal_evaluator_v2/phase1/builder/step55_impls.rs:155-198
- Fix: Extract a `fn reinsert_b_impl_into_s(state: &mut Phase1State, b: &Crate, b_impl_item: &Item, target_s_id: Id)` helper in `child_items.rs` or `builder/rewrite.rs` that performs: rewrite root → remap root → assign id → insert root into s_index/s_actions → for each child: rewrite child → remap child → assign child id → or_insert child into s_index/s_actions. Both call sites replace their inline block with a call to this helper.

#### [near-clone] Branch-validation preamble duplicated between merge_gate and task_completion (`after-124`)

- Rationale: The two blocks are structurally identical (same three operations, same return type, same error category strings), differing only in the human-readable gate name embedded in the step-2 error message. `track_resolution::resolve_track_id_from_branch` already encodes this rule but is not reused here.
- Locations: libs/usecase/src/merge_gate.rs:241-258, libs/usecase/src/task_completion.rs:42-58
- Fix: Extract a shared helper (e.g. `validate_track_branch(branch: &str) -> Result<&str, VerifyOutcome>`) inside the usecase crate, or reuse `resolve_track_id_from_branch` (mapping its `TrackResolutionError` to `VerifyOutcome`). Both gate functions call the helper and supply only the per-gate name string.

#### [near-clone] BuildIndexError and MeasureQualityError are near-identical three-variant error types (`after-144`)

- Rationale: Two copies of an identical three-variant composite error with four repeated impl blocks. Adding an error variant (e.g. `ParseFailed`) or adjusting the Io format string requires editing both types and all four impls. The duplication is purely nominal — the two types carry the same error knowledge.
- Locations: libs/usecase/src/semantic_dup/errors.rs:112-158, libs/usecase/src/semantic_dup/errors.rs:161-209
- Fix: Introduce a single shared `IndexBuildError { Embedding(EmbeddingError), Index(SemanticIndexError), Io { path: PathBuf, source: String } }` with one set of impls, and type-alias or newtype it for the two use-cases, or collapse them into a single error type shared by both interactors.

#### [near-clone] CN-07 active-track branch guard duplicated across usecase interactors (`after-147`)

- Rationale: Three interactors encode the same CN-07 guard rule. The type_signals copy is self-aware ('mirrors … exactly'). The guard embodies a non-trivial business invariant (branch naming convention for active tracks) and a change to the prefix or matching rule would require three coordinated edits. A shared free function or a domain-level helper that returns a typed result could eliminate all three copies.
- Locations: libs/usecase/src/type_signals/interactor.rs:127-138, libs/usecase/src/catalogue_spec_signals.rs:171-180, libs/usecase/src/catalogue_spec_refs.rs:169-178
- Fix: Extract a shared `fn check_active_track_branch(branch: &str, track_id: &str) -> Result<(), ActiveTrackGuardError>` in a shared usecase utility module (or expose it from the domain). Each interactor maps `ActiveTrackGuardError` variants to its own error type.

#### [near-clone] Codex child-process termination logic duplicated between plan and review (`after-023`)

- Rationale: 50-line block encoding the platform-specific subprocess termination policy. A bug in the Unix ESRCH handling or the Windows taskkill fallback must be fixed in both places; historically such bugs (race conditions in SIGKILL/ESRCH) are subtle and easy to miss in one copy.
- Locations: apps/cli/src/commands/plan/codex_local.rs:205-254, apps/cli/src/commands/review/codex_local.rs:419-465
- Fix: Extract `terminate_child_process(child: &mut Child) -> Result<(), String>` into a shared subprocess utilities module (e.g. a new `apps/cli/src/subprocess.rs` or inside cli-composition) and call it from both sites.

#### [near-clone] CodexLocalArgs vs ClaudeLocalArgs — nearly identical arg structs (`after-159`)

- Rationale: Two copies of the same 8-field arg struct with the same defaults and the same ArgGroup constraint. Adding a shared flag (e.g., --retry-count) or changing a default (e.g., timeout) must be done twice. The arg-group name differs but carries the same semantics.
- Locations: apps/cli/src/commands/review/mod.rs:86-131, apps/cli/src/commands/review/mod.rs:133-172
- Fix: Extract a single SharedReviewArgs struct containing the common 7 fields (everything except output_last_message). Embed it with #[command(flatten)] in both CodexLocalArgs and ClaudeLocalArgs, and keep the test-only field only in CodexLocalArgs. The mutual-exclusion ArgGroup can be expressed on the containing struct.

#### [near-clone] Dual commit-hash store read() implementations with near-identical logic (`after-154`)

- Rationale: The comment in dry_check/commit_hash_store.rs explicitly says 'Behavior mirrors FsCommitHashStore::read()', confirming intentional duplication. Any future change to the algorithm (ancestry logic, error handling, symlink policy) must be applied to both. The only justified difference is the error enum — the remaining ~40 lines of logic are change-amplification.
- Locations: libs/infrastructure/src/dry_check/commit_hash_store.rs:81-130, libs/infrastructure/src/review_v2/persistence/commit_hash_store.rs:38-69
- Fix: Extract a crate-private free function `read_commit_hash_from_file<E>(path, trusted_root, map_err: impl Fn(...) -> E) -> Result<Option<CommitHash>, E>` that encapsulates the common algorithm, then have both adapters call it with their respective error mappers.

#### [near-clone] Duplicate spec-section traversal chain in Chain-1 and Chain-2 pair sources (`after-067`)

- Rationale: The five-part traversal pattern encodes the same structural knowledge about which SpecDocument sections exist and their canonical label names. Adding, removing, or renaming a section in the domain type requires updating both sites. The functions live in sibling files within the same module, so a shared free function `iter_spec_sections(doc: &SpecDocument) -> impl Iterator<Item = (&'static str, &SpecRequirement)>` would eliminate the duplication.
- Locations: libs/infrastructure/src/ref_verify/pair_source.rs:138-146, libs/infrastructure/src/ref_verify/pair_source_chain2.rs:202-210
- Fix: Extract a shared helper (e.g., `fn iter_spec_sections<'a>(doc: &'a domain::SpecDocument) -> impl Iterator<Item = (&'static str, &'a domain::SpecRequirement)>`) in `pair_source.rs` (already the shared-helpers module) and call it from both `enumerate_chain1_pairs` and `find_spec_element_text`.

#### [near-clone] Duplicate test git() helper in show.rs and merge_gate_adapter.rs (`after-065`)

- Rationale: Both copies encode the same knowledge: git test invocations must pin locale to C and set a local committer identity for CI stability. If the panic message format or required env vars change (e.g. adding GIT_CONFIG_GLOBAL to prevent picking up host config), both copies must be updated in sync. A shared test-utilities module (e.g. `libs/infrastructure/src/test_utils/git.rs` behind `#[cfg(test)]`) would eliminate the duplication.
- Locations: libs/infrastructure/src/git_cli/show.rs:231-253, libs/infrastructure/src/verify/merge_gate_adapter.rs:530-551
- Fix: Extract `fn git(cwd: &Path, args: &[&str])` into a shared `#[cfg(test)]` module inside the `infrastructure` crate (e.g. `libs/infrastructure/src/git_cli/test_helpers.rs` re-exported as `pub(crate)` within tests), then replace both copies with a `use` import.

#### [near-clone] Duplicated open-or-create table async logic in `insert` and `insert_batch` (`after-074`)

- Rationale: Two copies of a ~25-line async block encoding the same open-or-create-table-with-race-safety invariant. Any change to the table-open strategy (e.g. using a native `open_or_create` API, changing error types, or adding retry logic) requires identical edits in both places. The block is non-trivial and contains subtle race-handling logic that must stay in sync.
- Locations: libs/infrastructure/src/semantic_dup/index.rs:271-303, libs/infrastructure/src/semantic_dup/index.rs:406-431
- Fix: Extract an `async fn open_or_create_table(connection: &Connection, schema: Arc<Schema>) -> Result<lancedb::Table, SemanticIndexError>` helper (or a closure capturing `connection` and `schema`) and call it from both `insert` and `insert_batch`. The error variant (InsertFailed) can be passed as a parameter or the helper can return a generic storage error that the callers map.

#### [near-clone] DynTrait rendering logic duplicated between ty_base (common arms) and ty_strip (`after-095`)

- Rationale: The DynTrait traversal pattern (HRTB guard → iterate PolyTraits → short name → inner args → sort → join → lifetime suffix) is duplicated across ~40 lines. A bug fix or extension (e.g. handling a new DynTrait field) requires changes in both files.
- Locations: libs/infrastructure/src/tddd/signal_evaluator_v2/format/ty_base.rs:611-645, libs/infrastructure/src/tddd/signal_evaluator_v2/format/ty_strip.rs:129-168
- Fix: Delegate `ty_strip`'s `DynTrait` arm to `format_dyn_trait_with` in `ty_base.rs` (which already exists and accepts callbacks), or let `format_type_strip_type_params` fall through to `format_type_common_arms` for the `DynTrait` case by supplying strip-aware callbacks.

#### [near-clone] Fenced code block state-machine logic repeated across three markdown scanner functions (`after-116`)

- Rationale: The same markdown fenced-code-block detection rule (how to open, how to close, what constitutes a valid fence) is encoded in three files. A bug in the closing-fence match (e.g., allowing extra trailing characters) would need to be fixed in all three locations and is unlikely to be caught consistently across them.
- Locations: libs/infrastructure/src/verify/spec_signals.rs:85-110, libs/infrastructure/src/verify/spec_attribution.rs:153-182, libs/infrastructure/src/verify/spec_states.rs:351-410
- Fix: Extract a reusable `FenceTracker` struct or a `advance_fence_state(fence: &mut Option<(char, usize)>, trimmed: &str) -> bool` helper into the shared `verify::frontmatter` or a new `verify::markdown` module. Each call site replaces its inline block with a call to the helper.

#### [near-clone] ImplTrait TraitBound rendering loop duplicated in ty_canon and ty_strip (`after-093`)

- Rationale: Two copies of the same ~30-line impl-trait rendering scaffold. The shared logic (modifier string, short-name extraction, args wrapping, sort, join) should live in `ty_base::format_impl_trait_with` which already exists and accepts callbacks — these two sites should call it instead of inlining the loop.
- Locations: libs/infrastructure/src/tddd/signal_evaluator_v2/format/ty_canon.rs:116-151, libs/infrastructure/src/tddd/signal_evaluator_v2/format/ty_strip.rs:99-128
- Fix: Both sites should delegate to the existing `format_impl_trait_with` helper in `ty_base.rs` (lines 182-200), supplying appropriate `fmt_args` and `fmt_trait_bound` callbacks for their respective canonicalization strategy, exactly as `format_base_impl_trait` already does.

#### [near-clone] InMemoryTrackStore (infrastructure) and StubTrackStore (usecase test) implement the same in-memory Mutex store logic (`after-148`)

- Rationale: Any change to how Mutex lock failures, TrackNotFound conditions, or TrackWriter::update semantics are handled must be applied in two places. The infrastructure already exports InMemoryTrackStore for testing purposes; the usecase test re-implements it from scratch adding ImplPlanReader/Writer support.
- Locations: libs/infrastructure/src/lib.rs:56-104, libs/usecase/src/lib.rs:346-413
- Fix: Extend InMemoryTrackStore to also implement ImplPlanReader + ImplPlanWriter (or create a combined InMemoryTrackStore in infrastructure that covers all four traits), then reuse it in the usecase tests by importing it from infrastructure.

#### [near-clone] Parallel non-empty-after-trim newtype constructors (`after-158`)

- Rationale: Six copies of the same 3-line 'trim then reject empty' rule. A change to that rule (e.g. switching from is_empty to a more nuanced check) requires edits in all six types. The module comment in ids.rs already notes they were extracted from nutype because of a rustdoc tooling problem, so a shared internal helper is clearly feasible. NonEmptyString already exists and could serve as the canonical implementation that the others reuse.
- Locations: libs/domain/src/ids.rs:178-181, libs/domain/src/ids.rs:214-217, libs/domain/src/plan_ref/adr_ref.rs:22-25, libs/domain/src/plan_ref/convention_ref.rs:21-28, libs/domain/src/adr_decision/grounds.rs:30-36, libs/domain/src/plan_ref/informal_ground_ref.rs:59-63
- Fix: Extract a private or pub(crate) free function `fn trim_and_validate_non_empty(value: String) -> Result<String, ()>` (or reuse NonEmptyString::try_new internally). All five non-empty anchor/ref newtypes can call it, storing the untrimmed or trimmed value per their existing behaviour.

#### [near-clone] Path-safety guard logic duplicated between `resolve_and_guard_path` and `validate_template_path` (`after-068`)

- Rationale: Both functions encode the same security-critical rule: a caller-supplied file path must be relative, non-traversing, non-empty, and must resolve within the project root. Because the rule is safety-relevant, divergence (e.g., one function gaining a new check that the other misses) could create a security gap. The `validate_template_path` variant could delegate to `resolve_and_guard_path` (mapping the error type) or the shared logic could be extracted into a common helper in `guarded_io.rs` parameterised on an error converter.
- Locations: libs/infrastructure/src/ref_verify/guarded_io.rs:24-58, libs/infrastructure/src/ref_verify/pair_source.rs:335-360
- Fix: Have `validate_template_path` delegate to `resolve_and_guard_path` and convert the `RefVerifyError` to `String`, or introduce a lower-level `check_path_safety(project_root, path) -> Result<PathBuf, String>` in `guarded_io.rs` that both callers use.

#### [near-clone] RefactorProposal and Rationale are near-identical non-empty string newtypes (`after-039`)

- Rationale: Two non-empty string types in the same file with byte-level identical implementation modulo names. A conceptual change to the empty-rejection rule (e.g., also trimming whitespace, changing error message format) requires editing both blocks. A general-purpose `NonEmptyString` already exists in the codebase (`ids.rs` line 166) and is unused here.
- Locations: libs/domain/src/dry_check/value_objects.rs:62-96, libs/domain/src/dry_check/value_objects.rs:105-139
- Fix: Replace both `RefactorProposal` and `Rationale` with domain-typed newtypes that wrap `ids::NonEmptyString` (newtype-over-newtype), keeping the distinct types for type safety but eliminating the duplicated constructor/validation logic. Alternatively, introduce a `non_empty_string!` declarative macro in the dry_check module that generates the repetitive boilerplate from a type name and error message.

#### [near-clone] ReviewRunCodexInput vs ReviewRunClaudeInput — identical DTO structs (`after-012`)

- Rationale: Two type aliases that are structurally identical. A field addition or type change (e.g. timeout from u64 to Duration) must be made twice. Because these types appear in the public CLI-composition API surface, divergence could cause callers to use the wrong DTO silently.
- Locations: apps/cli-composition/src/review_v2/inputs.rs:7-16, apps/cli-composition/src/review_v2/inputs.rs:18-29
- Fix: Unify into a single ReviewRunReviewerInput (or generic ReviewRunInput) with an additional provider: String field (or a ReviewerProvider enum). Both review_run_codex and review_run_claude would accept this unified type, and the provider field distinguishes dispatch.

#### [near-clone] Signal computation loop duplicated verbatim in test (`after-087`)

- Rationale: The test does not exercise the production function — it re-implements the logic and validates the re-implementation, not the code under test. A bug in the production loop that is absent in the test copy would pass undetected.
- Locations: libs/infrastructure/src/tddd/catalogue_spec_signals_refresher.rs:164-195, libs/infrastructure/src/tddd/catalogue_spec_signals_refresher.rs:319-350
- Fix: Call `refresh_one_layer` (or extract a pure `compute_signals(v3_doc, raw_json)` helper) in the test rather than re-implementing the loop inline.

#### [near-clone] Symlink guard on trusted-root / items_dir repeated in three files (`after-107`)

- Rationale: All three blocks encode the same security invariant: 'the trusted root must not itself be a symlink because reject_symlinks_below only inspects descendants.' A divergence in one copy (e.g., skipping the Err branch) would silently weaken the security boundary in that call site. There are also 5 more copies of this pattern outside the scan unit (baseline_capture.rs, catalogue_spec_refs.rs, catalogue_spec_signals.rs, catalogue_spec_signals_refresher.rs, rustdoc_baseline_capture_adapter.rs), amplifying the change-propagation cost.
- Locations: libs/infrastructure/src/track/fs_store.rs:339-353, libs/infrastructure/src/track/fs_spec_file_loader.rs:53-67, libs/infrastructure/src/track/spec_element_hash.rs:55-69
- Fix: Add `pub fn guard_root_not_symlink(root: &Path) -> Result<(), std::io::Error>` to `symlink_guard.rs`, returning a typed `io::Error`. Each caller maps the `io::Error` to its own error type via `.map_err(|e| MyError(e.to_string()))`. This keeps symlink_guard.rs as the single home for all root-symlink knowledge.

#### [near-clone] Verdict and FastVerdict are byte-identical enums with byte-identical constructors (`after-048`)

- Rationale: Two types with identical structure and identical constructor logic. Any change to `findings_remain` semantics (e.g. adding a max-findings cap or a new validity check) must be applied twice. The distinct types are justified; the duplicated constructor body is not.
- Locations: libs/domain/src/review_v2/types.rs:296-310, libs/domain/src/review_v2/types.rs:325-339
- Fix: Extract a shared constructor free-function or a sealed trait `VerdictLike` implemented by both enums, so `findings_remain` delegates to a single shared implementation. Alternatively, define a macro that stamps out both types from one template (though this may reduce readability). The simplest option: extract a `fn make_findings_remain(findings: Vec<ReviewerFinding>) -> Result<NonEmptyReviewerFindings, VerdictError>` and call it from both `impl` blocks.

#### [near-clone] `make_baseline(layer_str, crate_name_str, krate)` duplicated in 3 renderer sub-modules (`after-176`)

- Rationale: Three identical 6-line functions. Covered by the same extraction opportunity as the `make_item`/`empty_generics` finding above — best fixed together.
- Locations: libs/infrastructure/src/tddd/baseline_graph_renderer_adapter/render/node_extractor.rs:315-321, libs/infrastructure/src/tddd/baseline_graph_renderer_adapter/render/entry_subgraph.rs:464-470, libs/infrastructure/src/tddd/baseline_graph_renderer_adapter/render/impl_processor.rs:716-722, libs/infrastructure/src/tddd/baseline_graph_renderer_adapter/render/mod.rs:542-551, libs/infrastructure/src/tddd/baseline_graph_renderer_adapter/mod.rs:194-200
- Fix: Consolidate into the shared `test_helpers` module under `baseline_graph_renderer_adapter/render/`.

#### [near-clone] `make_hash(hex: &str) -> FragmentContentHash` / `make_file_path` / `make_fragment_ref` cluster duplicated in domain and infrastructure dry_check tests (`after-179`)

- Rationale: The dry_check domain-layer tests already expose these helpers; the infrastructure adapter tests re-implement them from scratch. Any change to `FragmentRef` or `FragmentContentHash` construction must be applied in both layers.
- Locations: libs/domain/src/dry_check.rs:113-123, libs/infrastructure/src/dry_check/store.rs:421-431, libs/domain/src/dry_check/coverage.rs:269-272, libs/infrastructure/src/dry_check/coverage.rs:279-282
- Fix: Consolidate into a `domain::dry_check::test_support` or a `usecase::dry_check::shared` test-only module (the latter already exists and has related helpers) and re-export for infrastructure use.

#### [near-clone] ensure_target_can_produce_type_ref_checks and ensure_target_can_produce_data_role_field_checks are structural near-clones (`after-052`)

- Rationale: Adding a new field to the validation logic, changing the error message format, or adjusting the role-matching strategy must be done twice in parallel. The two functions are 25 lines each with nearly byte-identical bodies.
- Locations: libs/domain/src/tddd/catalogue_linter_eval.rs:34-59, libs/domain/src/tddd/catalogue_linter_eval.rs:61-90
- Fix: Extract a single generic helper `ensure_target_can_produce_field_checks(rule_kind, target_roles, target_field, default_roles: &[RoleKind], all_label: &str, carries_field: impl Fn(RoleKind, &str) -> bool)` and have both call sites pass their respective constants and method references.

#### [near-clone] execute_branch_create / execute_branch_switch near-identical bodies (`after-026`)

- Rationale: 17-line near-identical blocks where a single validation-message change or structural refactor must be applied twice. The call site (`execute_branch`) already dispatches on `BranchAction`, so a small helper that takes the `CliApp` method as a parameter would eliminate the duplication.
- Locations: apps/cli/src/commands/track/branch_ops.rs:33-51, apps/cli/src/commands/track/branch_ops.rs:61-79
- Fix: Extract a shared `execute_branch_op(args: BranchArgs, op: impl FnOnce(&CliApp, PathBuf, String) -> Result<CommandOutcome, String>) -> Result<ExitCode, CliError>` that contains the shared validation and outcome-printing, and call it from both thin wrappers.

#### [near-clone] is_branch_delete and is_checkout_branch_create are near-identical git-option-skip + flag-scan functions (`after-042`)

- Rationale: Both functions are ~47 lines of structurally identical Rust (same while-loop body, same found_X guard, same per-token for-loop with `--` break, same bundled-flag detection block). A future change to how git global options are skipped (e.g. extending GIT_OPTIONS_WITH_ARG) requires updating both functions independently.
- Locations: libs/domain/src/guard/policy.rs:284-334, libs/domain/src/guard/policy.rs:337-383
- Fix: Extract a private helper such as `fn git_subcommand_has_any_flag(argv: &[String], git_index: usize, subcommand: &str, long_flags: &[&str], short_chars: &[char]) -> bool` that encodes the two phases once. Call it from both `is_branch_delete` and `is_checkout_branch_create`.

#### [near-clone] module_path_from_summary vs module_path_str_from_summary — same algorithm, different join separator (`after-083`)

- Rationale: The extraction algorithm (crate-root short-circuit, middle-segment slice) is a non-trivial rule that must stay consistent. If the path format changes (e.g. the ADR D decision changes how path slices are structured), both sites must be updated identically or they diverge silently, producing inconsistent index keys vs node ids.
- Locations: libs/infrastructure/src/tddd/baseline_graph_renderer_adapter/render/node_id_generator.rs:182-191, libs/infrastructure/src/tddd/baseline_graph_renderer_adapter/render/trait_index.rs:172-179
- Fix: Extract a `module_path_segments(path: &[String]) -> &[String]` (or return an iterator) from `node_id_generator`. Both callers apply their own `.join()`. Place the shared helper in `node_id_generator` (already `pub(super)`) and re-export or inline.

#### [near-clone] normalize_check_status and checks_summary duplicated across apps (`after-008`)

- Rationale: The mapping from raw GitHub check states to PrCheckStatus enum values is a business rule: adding a new terminal state (e.g., 'NEUTRAL') requires changes in both copies. The cli test_helpers copy is used by pr_tests.rs tests, so the rule is effectively maintained twice.
- Locations: apps/cli-composition/src/pr/poll.rs:97-118, apps/cli/src/commands/pr.rs:163-179
- Fix: Move normalize_check_status and summarize logic into usecase::pr_workflow (which already owns PrCheckStatus and summarize_checks). The test_helpers copies in apps/cli can be removed.

#### [near-clone] patch_paths_crate_ids and patch_paths_crate_ids_extra are near-identical loop bodies (`after-096`)

- Rationale: Two copies of the same crate-id patching loop. A change to the patching logic (e.g. error handling, new summary fields) must be applied in both places, and a divergence would silently produce inconsistent crate-id mappings for S-side vs A-side paths entries.
- Locations: libs/infrastructure/src/tddd/signal_evaluator_v2/external_crates.rs:108-128, libs/infrastructure/src/tddd/signal_evaluator_v2/external_crates.rs:138-158
- Fix: Extract a private `fn patch_paths_crate_ids_inner(paths, source_crates, name_to_new_id, id_filter: impl Fn(&Id) -> bool)` and call it from both public functions, passing an appropriate closure for each filter direction.

#### [near-clone] resolve_phase and resolve_phase_from_record duplicate the TrackStatus → TrackPhaseInfo mapping (`after-037`)

- Rationale: All six arms (Planned, InProgress, Done, Blocked, Cancelled, Archived) are structurally duplicated including their string literals ('track is planned', 'track has unresolved tasks', 'all tasks completed', 'track is blocked', 'track has been cancelled', 'track is archived'). A new TrackStatus variant or a change to any arm's reason/next_command must be applied to both functions. The duplication is large (≈47 lines of logic) and entirely avoidable.
- Locations: libs/domain/src/track_phase.rs:164-221, libs/domain/src/track_phase.rs:228-276
- Fix: Rewrite resolve_phase as: let status = derive_track_status(impl_plan, track.status_override()); let override_reason = track.status_override().map(|o| o.reason()); resolve_phase_from_record(status, override_reason). This makes resolve_phase a thin adapter and resolve_phase_from_record the single source of truth.

#### [near-clone] review() and fast_review() in cycle.rs are near-identical (`after-138`)

- Rationale: A single conceptual change to the review cycle (e.g., adding a third diff step, changing the FileChangedDuringReview condition, or altering the Skipped guard) must be applied in both methods. The doc comment on fast_review explicitly says 'Same flow as review()', confirming the duplication is recognised but not yet extracted.
- Locations: libs/usecase/src/review_v2/cycle.rs:55-76, libs/usecase/src/review_v2/cycle.rs:85-108
- Fix: Extract a private generic helper fn run_review_inner<V>(&self, scope: &ScopeName, invoke: impl FnOnce(&R, &ReviewTarget) -> Result<(V, LogInfo), ReviewerError>) -> Result<ReviewOutcome<V>, ReviewCycleError> and delegate both public methods to it.

#### [near-clone] run_codex_child timeout-poll loop duplicated in codex_reviewer and codex_dry_checker (`after-164`)

- Rationale: The poll loop and handle-join logic (~40 lines) encode the same child-process lifecycle management rule. A divergence (e.g. a fix to the kill-then-wait sequence) applied in one place but not the other would produce inconsistent timeout behaviour between reviewer and dry-checker.
- Locations: libs/infrastructure/src/review_v2/codex_reviewer.rs:386-466, libs/infrastructure/src/dry_check/codex_dry_checker.rs:524-579
- Fix: Extract a generic `run_codex_subprocess(child, io_handles, timeout) -> (bool /* timed_out */, bool /* exit_success */)` helper into the shared `process` module. Each caller then handles only its own output-file parsing after the helper returns.

#### [near-clone] track_add_task / track_add_task_resolved duplicate after-task-ID validation + output format (`after-016`)

- Rationale: A rule change (e.g. allowing `T0` or switching the delimiter) would require identical edits in both places. The shared error message string `"invalid --after value {a:?}: expected T<digits> (e.g. T001)"` encodes a user-facing contract that must stay in sync.
- Locations: apps/cli-composition/src/track/mod.rs:424-453, apps/cli-composition/src/track/ops.rs:45-77
- Fix: Extract the `after_task_id` validation into a free function `validate_after_task_id(after: Option<String>) -> Result<Option<String>, String>` in `track/mod.rs` (or a shared submodule). Both methods call it before building `AddTaskCommand`. The output-formatting block can similarly be a helper that accepts `output: AddTaskOutput`.

#### [near-clone] track_set_override / track_set_override_resolved and track_clear_override / track_clear_override_resolved are near-clones (`after-018`)

- Rationale: A change to the override output message or the SetOverrideCommand fields must be applied in both files. The two clear-override counterparts (mod.rs:493-519, ops.rs:124-150) compound this to four callsites for two conceptual operations.
- Locations: apps/cli-composition/src/track/mod.rs:458-489, apps/cli-composition/src/track/ops.rs:84-118
- Fix: Have `track_set_override` call `track_set_override_resolved` after resolving the track_id (and similarly for clear_override). The `_resolved` variant becomes the canonical implementation; the non-resolved wrapper is a thin shim that does ID resolution and delegates.

#### [semantic-dup] 4-source git diff union (merge-base + cached + worktree + untracked) re-implemented for review and dry-check (`after-167`)

- Rationale: The 4-source diff rule (which sources to include, which diff-filter flags, how to handle merge-base) is a single piece of knowledge duplicated across two adapters. Adding a fifth source or changing the diff-filter would require parallel edits. The merge-base lookup block is byte-identical.
- Locations: libs/infrastructure/src/review_v2/diff_getter.rs:21-101, libs/infrastructure/src/dry_check/diff_getter.rs:33-96
- Fix: Extract a shared helper `git_diff_sources(git: &SystemGitRepo, base: &CommitHash) -> Result<impl Iterator<Item=(&str, Output)>, …>` that runs all four git commands and yields labelled outputs. Each adapter then applies its own output-to-domain-type mapping over the shared iterator.

#### [semantic-dup] Duplicate `InformalGroundRefDto` struct and `informal_ground_kind_from_str` mapping (`after-151`)

- Rationale: Both locations encode the same 4-variant string→enum mapping for the same domain type. If a new `InformalGroundKind` variant is added, both `informal_ground_kind_from_str` functions and both `InformalGroundRefDto` definitions need updating. The `spec_ground_codec` module already exists precisely to share these types across the catalogue codec family; `spec/codec.rs` simply does not import from it.
- Locations: libs/infrastructure/src/spec/codec.rs:126-133, 303-323, 429-433, libs/infrastructure/src/tddd/spec_ground_codec.rs:52-62, 101-123, 126-133, 150-159
- Fix: In `spec/codec.rs`, remove the private `InformalGroundRefDto` struct and private `informal_ground_kind_from_str` / `informal_ground_ref_to_dto` functions. Import `InformalGroundRefDto` from `crate::tddd::spec_ground_codec` and reuse `informal_grounds_from_dtos` / `informal_grounds_to_dtos` from there, adapting the error mapping to `SpecCodecError::InvalidField`.

#### [semantic-dup] InformalGroundKind string-to-enum mapping duplicated across two codec modules (`after-078`)

- Rationale: Both functions encode the complete canonical mapping of the InformalGroundKind discriminants. Adding a new variant to the domain enum requires updating both match blocks. The only difference is the return type (Result vs Option), which could be unified into a single shared helper (e.g., returning Option and letting callers map to their respective error types).
- Locations: libs/infrastructure/src/spec/codec.rs:313-323, libs/infrastructure/src/tddd/spec_ground_codec.rs:126-134
- Fix: Extract a single canonical fn informal_ground_kind_from_str(s: &str) -> Option<InformalGroundKind> into a shared private infrastructure helper (e.g., a new libs/infrastructure/src/shared/informal_ground.rs or directly into the InformalGroundKind domain type as a from_str impl). Both codec modules then call the shared helper.

#### [semantic-dup] Three-section spec-refs loop duplicates `iter_catalogue_entries` traversal already in scope (`after-134`)

- Rationale: The module-level doc of catalogue_traversal.rs explicitly states it was created to prevent 'two usecases from drifting independently'. The spec-refs integrity loop in chain2_gate.rs is a third consumer that bypassed the shared helper, creating a third site where the traversal order can drift. If a new section (e.g. `impls`) is added to CatalogueDocument, the three-loop block must be updated manually while the single iter_catalogue_entries call at line 210 would handle it automatically.
- Locations: libs/usecase/src/merge_gate/chain2_gate.rs:110-136, libs/usecase/src/catalogue_traversal.rs:69-93
- Fix: Replace the three for-loops (lines 110–136) with a single loop over `iter_catalogue_entries(&catalogue)`, passing `entry.key.as_str()` as the entry name and `&entry.spec_refs` as the refs — matching the pattern already used at line 210 in the same function.

#### [semantic-dup] `git rev-parse --show-toplevel` repo-root resolution duplicated in CLI main (`after-170`)

- Rationale: The error message strings ('git rev-parse --show-toplevel returned an empty path') and the overall parse logic are nearly identical between the two sites. The main.rs version also bypasses the guarded-git wrapper, which is a correctness concern on top of the duplication. Any change to how toplevel discovery handles errors (e.g. adding a retry, changing error messages) must be done in both places.
- Locations: apps/cli/src/main.rs:290-308, libs/infrastructure/src/git_cli/mod.rs:170-195
- Fix: Replace the body of repo_root_for_items_dir() with SystemGitRepo::discover_from(&project_root).map(|r| r.root().to_path_buf()).map_err(|e| format!("{e}")). This delegates to the canonical infrastructure implementation and restores the guarded-git env guard.

#### [semantic-dup] `git rev-parse main` → CommitHash pipeline implemented twice (`after-169`)

- Rationale: Both sites encode the rule 'git rev-parse main output must be trimmed and parsed as CommitHash; failure is a String error'. The dry/shared.rs version was extracted precisely to serve as a shared helper, but review_v2/shared.rs did not adopt it. The divergence is confirmed by reviewing both functions: the only difference is that resolve_diff_base receives an already-discovered git instance while git_rev_parse_main_at discovers it internally.
- Locations: apps/cli-composition/src/dry/shared.rs:99-115, apps/cli-composition/src/review_v2/shared.rs:254-273
- Fix: Have resolve_diff_base in review_v2/shared.rs call git_rev_parse_main_at (re-exported from dry/shared.rs or moved to a shared location), passing Some(git.root()) as the discovery root. Alternatively, move git_rev_parse_main_at to a shared cli-composition utility module accessible from both dry and review_v2 submodules.

#### [semantic-dup] git merge-base --is-ancestor ancestry check duplicated in two commit-hash stores (`after-168`)

- Rationale: The 'stored hash must be an ancestor of HEAD' invariant is a single rule expressed identically in two stores. If the fail-closed policy changes (e.g. to emit a warning rather than silently return None), both stores must be updated.
- Locations: libs/infrastructure/src/review_v2/persistence/commit_hash_store.rs:59-68, libs/infrastructure/src/dry_check/commit_hash_store.rs:118-130
- Fix: Extract `fn check_is_ancestor_of_head(hash: &CommitHash) -> bool` (or returning `Result<bool, …>`) into `libs/infrastructure/src/git_cli/mod.rs` or a shared commit-hash utility module. Both stores call this helper.

#### [semantic-dup] round_type string validation knowledge encoded in two places (`after-139`)

- Rationale: Both sites encode the same business invariant (valid round types and the error message for an unknown value). run_review_fix.rs already imports nothing from run_review.rs, so it re-implements the check manually. Using ReviewRoundType::parse() (pub in run_review.rs) or extracting a shared validate_round_type() helper would be the minimal fix.
- Locations: libs/usecase/src/review_v2/run_review.rs:34-39, libs/usecase/src/review_v2/run_review_fix.rs:171-178
- Fix: In RunReviewFixInteractor::run(), replace the inline match with ReviewRoundType::parse(&command.round_type).map_err(RunReviewFixError::InvalidRoundType)? — which already exists in run_review.rs and is pub.

#### [semantic-dup] sanitize() function duplicated in baseline and contract_map renderer adapters (`after-085`)

- Rationale: The sanitization rule (which characters are safe in a Mermaid node id) is a cross-adapter policy. If the rule ever needs to change (e.g. to also allow `-` for a future Mermaid version), both copies must be updated together or they diverge and produce inconsistent node ids across the two renderers. The `mermaid_style` module is the established shared location for such cross-adapter helpers.
- Locations: libs/infrastructure/src/tddd/baseline_graph_renderer_adapter/render/style_config.rs:74-76, libs/infrastructure/src/tddd/contract_map_renderer_adapter/render/mod.rs:33-35
- Fix: Move `sanitize` into `libs/infrastructure/src/tddd/mermaid_style.rs` as a `pub(crate)` function and re-export it from both adapter `style_config` modules, exactly as `class_def_line` and `apply_shape` are already shared.

#### [semantic-dup] write_manifest re-implements atomic temp-rename write already provided by infrastructure::track::atomic_write (`after-004`)

- Rationale: Two functions in the same dry/ module encode the same atomic-write protocol differently. The divergence in durability (fsync present in one, absent in the other) is a latent correctness risk on systems where write ordering matters. A single conceptual change to the atomic-write policy would require edits in both places.
- Locations: apps/cli-composition/src/dry/manifest.rs:78-95, apps/cli-composition/src/dry/corpus_root.rs:111-143
- Fix: Replace the manual temp+rename in `write_manifest` with a call to `infrastructure::track::atomic_write::atomic_write_file`, converting the JSON string to bytes first. This matches the pattern already used by `write_dry_corpus_root_manifest` in the same module tree.

#### [structural-dup] Catalogue filename stem extraction repeated 4 times across 3 files (`after-110`)

- Rationale: Five occurrences across four call sites (one file has two). The same knowledge — 'strip -types.json, else use file_stem, else "unknown"' — is encoded every time a catalogue is decoded. A naming convention change or fallback policy change would require touching all sites. The code even self-documents this as 'Mirror the same derivation' (merge_gate_adapter.rs line 316), confirming the duplication is known but not yet extracted.
- Locations: libs/infrastructure/src/verify/catalogue_spec_refs.rs:208-214, libs/infrastructure/src/verify/catalogue_spec_refs.rs:470-476, libs/infrastructure/src/verify/catalogue_spec_signals.rs:264-272, libs/infrastructure/src/verify/merge_gate_adapter.rs:195-207, libs/infrastructure/src/verify/merge_gate_adapter.rs:320-332
- Fix: Extract a private helper `fn catalogue_stem_from_filename(filename: &str) -> String` in a shared location (e.g., the existing `crate::tddd::catalogue_document_codec` module or a new `crate::verify::catalogue_stem` helper) and call it from all 4 call sites.

#### [structural-dup] ConfidenceSignal→emoji match arms duplicated inline despite existing helper (`after-060`)

- Rationale: A change to the emoji values (or addition of a new ConfidenceSignal variant) must be applied in three places. The helper already encodes this rule; the two inline blocks simply forgot to call it.
- Locations: libs/infrastructure/src/type_catalogue_render.rs:234-241, libs/infrastructure/src/type_catalogue_render.rs:477-482, libs/infrastructure/src/type_catalogue_render.rs:548-553
- Fix: Replace both inline match blocks (lines 477-482 and 548-553) with a call to `catalogue_spec_signal_emoji(sig.signal())` and apply the `.unwrap_or_else` fallback to `"\u{2014}"` the same way the cat_spec_col path does. Optionally rename the helper to `confidence_signal_emoji` to clarify it is not cat-spec-specific.

#### [structural-dup] Four inline Stage-2 test mock structs repeat identical TrackBlobReader boilerplate (`after-135`)

- Rationale: All four structs share over 20 lines of identical boilerplate per struct (~80 lines total). Any change to the shared method signatures or their return-type conventions (e.g. changing the empty-BTreeMap idiom or the ZERO_HASH constant) requires editing all four mock impls. The existing `MockTrackBlobReader` struct (lines 536–638) and `MultiLayerMock` (lines 1046–1133) demonstrate that the codebase already uses shared configurable mock types for similar purposes; the Stage-2 structs pre-date or bypassed that pattern.
- Locations: libs/usecase/src/merge_gate.rs:1871-1918, libs/usecase/src/merge_gate.rs:1940-1988, libs/usecase/src/merge_gate.rs:2012-2059, libs/usecase/src/merge_gate.rs:2135-2207
- Fix: Introduce a single configurable `Stage2Mock` struct (similar to the existing `MockTrackBlobReader`) with fields for `type_catalogue_result` and `type_signals_result`, defaulting to Found+ZERO_HASH and the variant under test respectively. The four test functions would construct `Stage2Mock` with the relevant `read_type_signals` variant, eliminating the per-test struct definitions.

#### [structural-dup] Four-way subgraph prologue repetition in entry_emitter.rs (`after-080`)

- Rationale: Four copies of the same ~14-line boilerplate block. Divergence risk: if a new field needs to be derived from the item (e.g. deprecation) or the ID-generator signature changes, all four prologue blocks must be updated in sync. The pattern is clearly warranted for abstraction.
- Locations: libs/infrastructure/src/tddd/baseline_graph_renderer_adapter/render/entry_emitter.rs:63-90, libs/infrastructure/src/tddd/baseline_graph_renderer_adapter/render/entry_emitter.rs:213-236, libs/infrastructure/src/tddd/baseline_graph_renderer_adapter/render/entry_emitter.rs:401-424, libs/infrastructure/src/tddd/baseline_graph_renderer_adapter/render/entry_emitter.rs:527-549
- Fix: Extract a private `resolve_entry_context(doc, id, layer) -> Option<EntryContext>` helper returning `(item, name, module_path, entry_sg_id, rep_node_id, label)`. Each emitter calls it at the top and returns `Ok(())` if `None`. The `type_node_id` vs `trait_node_id` distinction is passed as a parameter or handled by two specialised variants.

#### [structural-dup] Param/return type encoding logic duplicated between encode_function and encode_method_items (`after-091`)

- Rationale: Adding a new case to the generic-name encoding path (e.g., for associated-type projections in return positions) requires modifying both sites independently. The two sites differ mainly in receiver handling and `force_has_body` logic; the param/return encoding kernel is identical.
- Locations: libs/infrastructure/src/tddd/catalogue_to_extended_crate_codec/encoder_state_fn_trait_codec.rs:103-175, libs/infrastructure/src/tddd/catalogue_to_extended_crate_codec/encoder_state_fn_trait_codec.rs:254-346
- Fix: Extract `fn encode_typed_string(raw_str: &str, generic_names: &[&str], state: &mut EncoderState) -> Result<Type, _>` covering the bare-generic-name branch + parse_type_ref_str + rewrite_generic_types, and call it from both loops.

#### [structural-dup] Read-old / compare / atomic-write-if-changed pattern repeated 5 times across render submodules (`after-106`)

- Rationale: Five near-identical blocks encode the same scaffold. Adding a new rendered artifact requires another copy. The only variation between the four sync.rs copies is the path variable and rendered-content variable names — the control flow is byte-for-byte identical. The contract_map.rs copy differs only in its error handler (eprintln+return-Ok vs return-Err), which would map to a parameter or closure in a helper.
- Locations: libs/infrastructure/src/track/render/sync.rs:145-156, libs/infrastructure/src/track/render/sync.rs:190-201, libs/infrastructure/src/track/render/sync.rs:410-420, libs/infrastructure/src/track/render/sync.rs:484-495, libs/infrastructure/src/track/render/contract_map.rs:135-156
- Fix: Extract a `fn write_if_changed(path: &Path, rendered: &str, changed: &mut Vec<PathBuf>) -> Result<(), RenderError>` helper in `render/mod.rs` (or a new `render/write.rs`). The contract_map.rs variant could take a `on_error: ErrorPolicy` parameter or accept a closure.

#### [structural-dup] Redirect-collection block duplicated for two redirect source lists in collect_from_conch_simple (`after-075`)

- Rationale: The same classification-and-collection rule (is_output_redirect → track has_output_redirect → extract word → push to both vecs) is encoded identically in two consecutive blocks. Adding a new redirect kind or changing the output-redirect classification rule requires updates in both places.
- Locations: libs/infrastructure/src/shell/conch.rs:247-260, libs/infrastructure/src/shell/conch.rs:262-276
- Fix: Extract a helper `fn collect_redirect_texts(redirect: &ast::Redirect<...>, redirect_texts: &mut Vec<String>, output_redirect_texts: &mut Vec<String>, has_output_redirect: &mut bool)` and call it from both loops.

#### [structural-dup] Repeated reject_symlinks_below → SymlinkDetected/Io error mapping pattern (`after-156`)

- Rationale: The `InvalidInput` sentinel is an undocumented internal contract of `reject_symlinks_below`. Every call site that interprets this sentinel is brittle: if the sentinel were changed, all four sites would silently misclassify the error. A shared helper would encapsulate the `InvalidInput` knowledge in one place.
- Locations: libs/infrastructure/src/dry_check/store.rs:149-155, libs/infrastructure/src/dry_check/store.rs:237-243, libs/infrastructure/src/dry_check/commit_hash_store.rs:85-91, libs/infrastructure/src/review_v2/persistence/commit_hash_store.rs:26-35
- Fix: Add a typed error enum to `track::symlink_guard` (e.g. `SymlinkGuardError { SymlinkDetected, Io(io::Error) }`) returned by `reject_symlinks_below`. Call sites `match` on it and construct their own domain error — the `InvalidInput` sentinel check disappears from all four sites.

#### [structural-dup] StubReader test double redefined in three test modules (`after-128`)

- Rationale: Three independent definitions of the same test double. If the DryCheckReader trait signature changes, all three stubs need updating. The precedent set by shared.rs::test_mocks (which already centralises MockEmbeddingPort/MockSemanticIndexPort) makes this a clear oversight rather than an intentional design choice.
- Locations: libs/usecase/src/dry_check/interactor.rs:765-773, 839-843, libs/usecase/src/dry_check/approval_interactor.rs:247-255, libs/usecase/src/dry_check/results_interactor.rs:115-129
- Fix: Move a canonical `StubReader` (and the matching `ErrorReader`) into `shared::test_mocks` alongside the existing mock definitions, then replace the three local definitions with imports.

#### [structural-dup] TaskOperationInteractor construction boilerplate repeated 6+ times (`after-017`)

- Rationale: Replacing `TaskOperationInteractor` with a different concrete type or changing the branch-reader wiring would require updating all six callsites. The repetition is pure scaffolding with no variation in logic between instances (only the command struct and service method differ).
- Locations: apps/cli-composition/src/track/mod.rs:229-239, apps/cli-composition/src/track/mod.rs:413-423, apps/cli-composition/src/track/mod.rs:465-475, apps/cli-composition/src/track/mod.rs:498-508, apps/cli-composition/src/track/ops.rs:33-43, apps/cli-composition/src/track/ops.rs:91-101
- Fix: Add a private helper `fn build_task_operation_service(items_dir: &PathBuf, project_root: &Path) -> usecase::task_ops::TaskOperationInteractor<…>` (or returning the service trait object) in `track/mod.rs`. All six callsites reduce to a single call.

#### [structural-dup] TdddLayerBindingsError → usecase error match block repeated identically in two interactors (`after-119`)

- Rationale: Two copies of a non-trivial match conversion block with identical per-arm logic and identical error message strings. Change-amplification is concrete: adding a new TdddLayerBindingsError variant requires touching both interactors.
- Locations: libs/usecase/src/baseline_capture/interactor.rs:113-127, libs/usecase/src/catalogue_impl_signals/interactor.rs:158-175
- Fix: Define a shared helper trait or free function `map_layer_bindings_error<E>(e: TdddLayerBindingsError, ...) -> E` in the usecase crate, or use a blanket From<TdddLayerBindingsError> approach. Alternatively, if the target error types were unified through a common variant, the conversion could live in a single place.

#### [structural-dup] Three copies of the telemetry emission block in mod.rs (review_run_codex / review_run_claude / review_run_local) (`after-011`)

- Rationale: Three copies of a 20-line block with the same control flow. Any future change to telemetry emission (new event type, changed argument order, added guard condition) requires updating all three sites. A helper function `fn emit_review_telemetry(items_dir, track_id, run_result, round_type, provider, model, round_start)` extracting the common pattern would remove the duplication.
- Locations: apps/cli-composition/src/review_v2/mod.rs:156-180, apps/cli-composition/src/review_v2/mod.rs:232-255, apps/cli-composition/src/review_v2/mod.rs:360-384
- Fix: Extract the shared block into a private `fn emit_review_telemetry(items_dir: &Path, track_id: &str, run_result: &Result<CodexReviewOutcome, String>, round_type: &str, provider: &str, model: &str, round_start: Instant)` and call it from all three `review_run_*` methods.

#### [structural-dup] Three-way duplication of `supersede` transition body in state.rs (`after-033`)

- Rationale: Three identical copies of the same invariant guard + constructor encode the same business rule (superseded_by must not be empty). A divergent edit (e.g., switching to trim-based validation consistent with DecisionGroundRef) would require updating all three sites independently, risking inconsistency.
- Locations: libs/domain/src/adr_decision/state.rs:98-106, libs/domain/src/adr_decision/state.rs:163-171, libs/domain/src/adr_decision/state.rs:212-219
- Fix: Extract a private free function `fn make_superseded(common: AdrDecisionCommon, superseded_by: String) -> Result<SupersededDecision, AdrDecisionCommonError>` that performs the guard and constructs the value. Have all three callers delegate to it.

#### [structural-dup] Three-way rep_id look-up block for inherent-method dispatch in entry_subgraph.rs (`after-081`)

- Rationale: Three identical 10-line blocks within the same function. Any change to how the representative node id is computed (e.g. a new parameter in `type_rep_node_id`) must be applied to all three copies. The extraction opportunity is clear and low-risk.
- Locations: libs/infrastructure/src/tddd/baseline_graph_renderer_adapter/render/entry_subgraph.rs:264-274, libs/infrastructure/src/tddd/baseline_graph_renderer_adapter/render/entry_subgraph.rs:289-299, libs/infrastructure/src/tddd/baseline_graph_renderer_adapter/render/entry_subgraph.rs:338-348
- Fix: Extract a local closure or private function `lookup_method_ids(id: &Id, doc: &BaselineDocument, layer: &str, inherent_map: &...) -> Option<&[Id]>` that encapsulates the summary_path/mp/item_name/type_rep_node_id logic and the inherent_map lookup. Call it from each of the three loops.

#### [structural-dup] Timestamped unique-path generation logic duplicated in codex_reviewer and review_fix_runner/spawn (`after-072`)

- Rationale: The uniqueness guarantee (counter + timestamp + pid) is a non-trivial piece of logic. If a bug is found (e.g. the counter wrapping, the nanos conversion, or the create_dir_all error handling), both copies must be fixed. The two static `AtomicU64` counters are also separate, so uniqueness is only guaranteed within each module, not across the two call sites.
- Locations: libs/infrastructure/src/review_v2/codex_reviewer.rs:255-271, libs/infrastructure/src/review_v2/review_fix_runner/spawn.rs:14-33
- Fix: Extract a shared `make_runtime_path(base_dir: &str, prefix: &str, ext: &str) -> Result<PathBuf, String>` free function in the `review_v2` module, with a single `AtomicU64`. Each caller wraps the `String` error into its own error type via `.map_err`.

#### [structural-dup] TraitBoundModifier 3-arm match repeated 4 times across format submodules (`after-094`)

- Rationale: The modifier-to-string mapping is a single piece of knowledge about the `TraitBoundModifier` enum. Four copies means an enum extension (e.g. a new modifier variant) requires four coordinated edits, and a divergence would produce silently inconsistent formatting strings across different code paths.
- Locations: libs/infrastructure/src/tddd/signal_evaluator_v2/format/ty_base.rs:464-469, libs/infrastructure/src/tddd/signal_evaluator_v2/format/ty_canon.rs:135-140, libs/infrastructure/src/tddd/signal_evaluator_v2/format/ty_canon.rs:356-361, libs/infrastructure/src/tddd/signal_evaluator_v2/format/ty_strip.rs:113-118
- Fix: Extract `fn format_trait_bound_modifier(modifier: &TraitBoundModifier) -> &'static str` in `ty_base.rs` and call it from all four sites.

#### [structural-dup] Two-pass external-crate-discovery pattern duplicated across parse_type_ref_str and encode_bound_str (`after-088`)

- Rationale: Any change to the two-pass protocol (e.g., the deduplication check `if !new_crate_names.contains`, the placeholder value `u32::MAX - 1`, or the on-demand registration in pass 2) must be applied identically in both methods. A single extraction function parameterised on the parser callable would eliminate this coupling.
- Locations: libs/infrastructure/src/tddd/catalogue_to_extended_crate_codec/encoder_state_core.rs:268-319, libs/infrastructure/src/tddd/catalogue_to_extended_crate_codec/encoder_state_core.rs:382-422
- Fix: Extract a generic helper `two_pass_parse<F, T>(&mut self, input: &str, parser: F) -> Result<T, _>` where `F` wraps `parse_type_ref` or `parse_generic_bound`, then call it from both methods.

#### [structural-dup] Verdict tally loop repeated three times within execute() (`after-136`)

- Rationale: Three identical match-arm sequences encode the same 'count fails and pendings in a verdict slice' knowledge. A helper fn tally_verdicts(verdicts: &[(_, SemanticVerdict)]) -> (usize, usize) would unify all three sites. Adding a fourth SemanticVerdict variant would require three edits.
- Locations: libs/usecase/src/ref_verify/interactor.rs:161-169, libs/usecase/src/ref_verify/interactor.rs:238-244, libs/usecase/src/ref_verify/interactor.rs:247-253
- Fix: Extract a private helper `fn tally_verdicts(verdicts: &[(impl Any, SemanticVerdict)]) -> (usize, usize)` that returns `(fail_count, pending_count)`, then replace all three loop bodies with a single call site.

#### [structural-dup] `write_file(root, rel, content)` test helper duplicated 5 times inside `libs/infrastructure/src/verify/` (`after-177`)

- Rationale: Six copies of a 5-line helper in the same directory. Any correctness change (e.g., error propagation, symlink handling) must be applied to all six.
- Locations: libs/infrastructure/src/verify/architecture_rules.rs:284-290, libs/infrastructure/src/verify/doc_patterns.rs:129-135, libs/infrastructure/src/verify/doc_links.rs:180-186, libs/infrastructure/src/verify/catalogue_spec_signals.rs:499-505, libs/infrastructure/src/verify/plan_artifact_refs_tests.rs:25-31, libs/infrastructure/src/verify/latest_track_tests.rs:11-17
- Fix: Create a `libs/infrastructure/src/verify/test_support.rs` (or `verify/helpers.rs`) with a single `pub(crate) fn write_file(root: &Path, rel: &str, content: &str)` and import it from all six test modules.

#### [structural-dup] git-global-option-skip loop body duplicated across three functions (`after-043`)

- Rationale: Three copies of the same git-option traversal loop. If GIT_OPTIONS_WITH_ARG grows or the `--` semantics change, all three sites need updating. The two later functions could instead call a variant of `extract_git_subcommand` that also returns the post-subcommand index, removing the duplicated knowledge.
- Locations: libs/domain/src/guard/policy.rs:257-281, libs/domain/src/guard/policy.rs:286-309, libs/domain/src/guard/policy.rs:338-361
- Fix: Extend `extract_git_subcommand` to return `(Option<String>, usize)` (subcommand name + index after it), or add a sibling `find_git_subcommand_index`. Then rewrite `is_branch_delete` and `is_checkout_branch_create` to call that helper instead of re-walking git options themselves.

#### [structural-dup] layer_filter validation-and-application pattern duplicated in two render-workflow interactors (`after-122`)

- Rationale: The layer-filter validation rule (find requested LayerId in loader set, fail with LayerNotFound if absent; deduplicate via re-iteration over the authoritative order) is a single business invariant repeated across two workflows. The code is ~20 lines in each site. A mismatched evolution would silently produce different behaviour between the two render workflows.
- Locations: libs/usecase/src/baseline_graph_workflow.rs:258-281, libs/usecase/src/contract_map_workflow.rs:202-226
- Fix: Extract a free function `fn apply_layer_filter<E>(layer_order: &[LayerId], filter: Option<&[LayerId]>, make_not_found: impl Fn(LayerId) -> E) -> Result<Vec<LayerId>, E>` and call it from both interactors.

#### [structural-dup] resolve_track_id match-arm boilerplate repeated 4 times across command modules (`after-024`)

- Rationale: 4 copies of the same 7-line resolution+error-print pattern. The sites that use the `?` operator (classify.rs, files.rs, results.rs) are already factored; these 4 sites use an inconsistent match-arm style that diverges from the majority idiom. A change to the error reporting format (e.g. adding a prefix) must be applied to all 4 sites.
- Locations: apps/cli/src/commands/dry.rs:190-196, apps/cli/src/commands/ref_verify.rs:85-92, apps/cli/src/commands/ref_verify.rs:100-107, apps/cli/src/commands/review/mod.rs:435-442
- Fix: Define a helper macro or free function `resolve_track_id_or_fail(track_id: Option<String>, items_dir: &Path) -> Result<String, ExitCode>` that encodes the eprintln+FAILURE mapping, and replace the 4 match blocks with a single `?` call on the result.

#### [structural-dup] std_canonical_path and core_canonical_path encode the same trait-to-module mapping knowledge twice (`after-100`)

- Rationale: Both functions encode the same knowledge — which Rust stdlib traits belong to which module — and must be kept in sync. Any new trait added to one must be added to the other, and any module rename would require dual updates. Abstractions: a single const map from `short_name → (std_module, core_module)` would consolidate the lookup, or a macro could generate both match arms from one source table.
- Locations: libs/infrastructure/src/tddd/type_ref_parser/helpers.rs:32-133, libs/infrastructure/src/tddd/type_ref_parser/helpers.rs:145-232
- Fix: Introduce a single const lookup table of `(&str, &str, &str)` tuples (short_name, std_path, core_path) and drive both `std_canonical_path` and `core_canonical_path` from it. Alternatively, a single `canonical_path(short_name, prefix: &str)` function with shared module suffix logic eliminates the duplicate match structure.

### Severity: low

#### [data-dup] ARCH_RULES_FILE constant defined independently in two modules (`after-113`)

- Rationale: Two independent constants with the same value and same purpose. Low risk since the filename is unlikely to change, but they represent the same knowledge split across two files.
- Locations: libs/infrastructure/src/verify/canonical_modules.rs:14, libs/infrastructure/src/verify/module_size.rs:11
- Fix: Promote to a shared constant in `crate::arch` (e.g., `pub const ARCH_RULES_FILE: &str = "architecture-rules.json";`) and reference it from both modules. The `arch` module is already imported in both files.

#### [data-dup] CODEX_BIN_ENV = "SOTP_CODEX_BIN" defined independently in four modules (`after-185`)

- Rationale: Four copies of a test-only env-var name. Renaming the env variable (e.g. to `SOTP_CODEX_PATH`) requires four edits. The constant should live once in a shared test-support or infrastructure module.
- Locations: libs/infrastructure/src/review_v2/codex_reviewer.rs:25, libs/infrastructure/src/review_v2/review_fix_runner/mod.rs:23, apps/cli/src/commands/review/mod.rs:41, apps/cli/src/commands/plan/mod.rs:21
- Fix: Define one `pub const CODEX_BIN_ENV: &str = "SOTP_CODEX_BIN"` in `libs/infrastructure/src/review_v2/codex_reviewer.rs` (already `pub(crate)`) and import it in the other three sites, or move it to a shared `infrastructure::subprocess` or `infrastructure::test_support` module.

#### [data-dup] DOMAIN_SRC_DIR constant defined independently in two modules (`after-114`)

- Rationale: Two independent constants with the same string value encoding the same workspace-layout knowledge. A workspace refactoring of the domain crate path would need updates in both files.
- Locations: libs/infrastructure/src/verify/domain_purity.rs:10, libs/infrastructure/src/verify/domain_strings.rs:10
- Fix: Promote to a shared constant in `crate::verify::mod` or `crate::arch` and import it in both modules.

#### [data-dup] Duplicate `SignalCountsDto` struct for the same JSON/YAML signal-counts shape (`after-153`)

- Rationale: If signal counts gains a fourth field (e.g., `gray`), both structs need updating. The frontmatter verifier's doc comment acknowledges it is a mirror of the same domain type. The duplication is small (3 fields) but encodes the same schema knowledge. The difference in derive set (YAML-only vs JSON+YAML) may make direct reuse awkward without a wrapper, but the data-dup is real.
- Locations: libs/infrastructure/src/spec/codec.rs:153-160, libs/infrastructure/src/verify/spec_frontmatter.rs:12-23
- Fix: Re-export the JSON codec's `SignalCountsDto` from `spec/codec.rs` as `pub(crate)`, then use `#[serde(with = ...)]` or a type alias in `verify/spec_frontmatter.rs`. Alternatively, add `#[derive(serde::Deserialize)]` to the codec's existing `SignalCountsDto` is already present — the frontmatter module can import from the codec module directly.

#### [data-dup] Em-dash fallback literal `\u{2014}` repeated across entry_details.rs and type_catalogue_render.rs without a named constant (`after-109`)

- Rationale: Changing the fallback display character (e.g. from em-dash to a different sentinel) would force edits across 11 production-code sites in two files. The data-dup is real but the risk of dangerous divergence is low — the character is purely cosmetic. Extracting a module-level `const EMPTY_CELL: &str = "\u{2014}";` would consolidate the definition.
- Locations: libs/infrastructure/src/type_catalogue_render/entry_details.rs:115,130,149,154,164, libs/infrastructure/src/type_catalogue_render.rs:312,483,499,554,561
- Fix: Declare `const EMPTY_CELL: &str = "\u{2014}";` (or `DASH_CELL`) at the top of `type_catalogue_render.rs` (making it visible to the `entry_details` submodule via `pub(super)`) and replace all literal occurrences with this constant.

#### [data-dup] POLL_INTERVAL constant duplicated in claude_reviewer and codex_reviewer (`after-073`)

- Rationale: Minor data duplication: two copies of the same constant encoding the same polling policy. Changing the poll interval requires editing both files.
- Locations: libs/infrastructure/src/review_v2/claude_reviewer.rs:42, libs/infrastructure/src/review_v2/codex_reviewer.rs:21
- Fix: Move `POLL_INTERVAL` to `libs/infrastructure/src/review_v2/mod.rs` as a `pub(super) const` and reference it from both reviewer modules.

#### [data-dup] Triple `PathBuf::from("track/items")` construction in `execute_hook_with_telemetry` (`after-031`)

- Rationale: Three identical `PathBuf::from("track/items")` bindings in the same function body encode the same constant path. The fix is a single `let items_dir = PathBuf::from("track/items");` hoisted before the `match`, and each arm can reuse the binding via a shared reference.
- Locations: apps/cli/src/main.rs:350, apps/cli/src/main.rs:357, apps/cli/src/main.rs:378
- Fix: Hoist `let items_dir = std::path::PathBuf::from("track/items");` before the `match &outcome_result { ... }` block (around line 344) and remove the three inner bindings. Each arm then borrows `&items_dir`.

#### [exact-clone] CurrentDirGuard test helper duplicated across git.rs and review/tests.rs (`after-022`)

- Rationale: 17-line RAII helper encoding the same 'save/restore cwd' invariant. Low severity because it is test-only; medium duplication risk if future callers need to add cross-platform handling.
- Locations: apps/cli/src/commands/git.rs:91-107, apps/cli/src/commands/review/tests.rs:72-88
- Fix: Add CurrentDirGuard to the shared test-utility module suggested for EnvVarGuard and import it from both sites.

#### [exact-clone] Duplicate test helper make_baseline_with_module_struct / make_baseline_with_struct in render/mod.rs (`after-084`)

- Rationale: Both helpers are exact clones modulo local `use` statements. They encode the same test-fixture construction logic. The earlier one can be replaced with the later one (or vice versa) throughout the test module.
- Locations: libs/infrastructure/src/tddd/baseline_graph_renderer_adapter/render/mod.rs:628-643, libs/infrastructure/src/tddd/baseline_graph_renderer_adapter/render/mod.rs:1245-1257
- Fix: Remove `make_baseline_with_module_struct` and replace all its call sites with `make_baseline_with_struct`. Both have the identical signature and body.

#### [exact-clone] NonEmptyString::try_new and ReviewGroupName::try_new are byte-identical (`after-035`)

- Rationale: The two implementations are byte-for-byte identical (single-line body). Any future change to the validation rule — such as adding a length limit, a character allowlist, or changing the error type — must be applied in two places or one type silently diverges.
- Locations: libs/domain/src/ids.rs:178-180, libs/domain/src/ids.rs:214-216
- Fix: Extract a private free function `validated_non_empty(value: impl Into<String>) -> Result<String, ValidationError>` that trims and rejects empty. Both try_new methods delegate to it.

#### [exact-clone] SpecAttribution|SpecFrontmatter items_dir derivation duplicated for SpecSignals in verify.rs (`after-029`)

- Rationale: Six-line exact clone within the same match expression. Both `SpecVerifyArgs.spec_path` and the spec path field for SpecSignals are the same type (`PathBuf`), so the arms could be collapsed with a `|` pattern if both arms carried the same struct type, or extracted to a small named helper `derive_items_dir_from_spec_path(spec_path: &Path) -> PathBuf`.
- Locations: apps/cli/src/commands/verify.rs:201-207, apps/cli/src/commands/verify.rs:208-214
- Fix: Extract the shared derivation into a free function `fn items_dir_from_spec_path(spec_path: &Path) -> PathBuf` and call it from both arms. Alternatively, if `SpecSignals` can reuse `SpecVerifyArgs`, collapse the arms with `|`.

#### [knowledge-dup] TrackStatus string-to-enum mapping duplicated in registry.rs and track_status_reader_adapter.rs (`after-108`)

- Rationale: Both encode the same set of valid status strings and the same variant mapping. If a new variant is added to domain::TrackStatus (e.g., Paused) and only one file is updated, the registry renderer silently falls back to Planned while the adapter raises an error — creating a behaviour split that could hide the missing update. The fallback semantics differ intentionally, but the variant-to-string mapping knowledge is shared and should live once.
- Locations: libs/infrastructure/src/track/render/registry.rs:7-17, libs/infrastructure/src/track/track_status_reader_adapter.rs:55-66
- Fix: Add a `fn track_status_from_str(s: &str) -> Option<TrackStatus>` function to the domain crate (or to a shared infrastructure helper). Both sites then call this function: registry.rs uses `.unwrap_or(TrackStatus::Planned)`, the adapter uses `.ok_or_else(|| unrecognised error)`.

#### [near-clone] Branch-id consistency check duplicated in TrackMetadata::with_branch and set_branch (`after-038`)

- Rationale: The check is 8-9 lines of duplicated logic including the format string and error construction. If the branch prefix policy ever changes (e.g., a different separator or an additional validation), both sites must be updated. The duplication is contained within the same struct impl block, making extraction straightforward.
- Locations: libs/domain/src/track.rs:324-332, libs/domain/src/track.rs:371-381
- Fix: Extract a private free function `check_branch_id_consistency(id: &TrackId, branch: &TrackBranch) -> Result<(), DomainError>` and call it from both with_branch and set_branch.

#### [near-clone] Catalogue hash hex-parse step duplicated in two interactors (`after-121`)

- Rationale: Same knowledge (how to convert a raw-bytes hex string from the port into a ContentHash and surface the failure with a layer_id tag) expressed in two places. Medium-severity if the port contract evolves, but currently small (4 lines each).
- Locations: libs/usecase/src/catalogue_spec_signals.rs:235-242, libs/usecase/src/catalogue_spec_refs.rs:203-212
- Fix: Extract a helper such as `fn parse_catalogue_hash(hex: &str, layer_id: &str) -> Result<ContentHash, InvalidCatalogueHashReason>` in a shared usecase utility module and map it to each error type at the call site.

#### [near-clone] D-types vs C and D-functions vs C evaluation loops are structurally identical in phase2 (`after-099`)

- Rationale: A change to the D-vs-C signal logic (e.g. adding metadata to the signal or changing the signal region name) requires editing two places. The D-impls loop (lines 205-219) diverges with the stripped-key fallback and is excluded.
- Locations: libs/infrastructure/src/tddd/signal_evaluator_v2/phase2.rs:173-181, libs/infrastructure/src/tddd/signal_evaluator_v2/phase2.rs:184-190
- Fix: Extract a helper `fn evaluate_d_vs_c_for_map(d_map: &BTreeMap<String,Id>, c_map: &BTreeMap<String,Id>, signals: &mut Vec<ThreeWaySignal>)` and call it for types and functions.

#### [near-clone] Duplicate O_NOFOLLOW open helpers in report.rs and writer.rs (`after-104`)

- Rationale: Both functions encode the same security invariant (never follow a symlink on open) using the same platform-specific idiom. If the invariant changes (e.g. adding O_PATH, switching to openat, or supporting non-Unix) both sites need updating in lock-step. Two copies, ~9 lines each, with a clean abstraction opportunity.
- Locations: libs/infrastructure/src/telemetry/report.rs:319-328, libs/infrastructure/src/telemetry/writer.rs:273-282
- Fix: Extract a shared `open_no_follow(path: &Path, options: &OpenOptions) -> io::Result<File>` helper (or a simple mode enum) into `crate::track::symlink_guard` or a new `telemetry::io` submodule, and have both callers delegate to it.

#### [near-clone] Four gh-API paginated-list functions share identical structure (endpoint, paginate, stdout/stderr handling) (`after-061`)

- Rationale: All four functions encode the same rule: gh api <endpoint> --paginate → return stdout string or CommandFailed. A change to gh CLI flag names, stdout/stderr extraction, or error formatting would require touching all four bodies. The duplication is mechanical; a shared helper `gh_api_paginated(endpoint, run_gh)` would eliminate it.
- Locations: libs/infrastructure/src/gh_cli.rs:370-381, libs/infrastructure/src/gh_cli.rs:383-394, libs/infrastructure/src/gh_cli.rs:396-407, libs/infrastructure/src/gh_cli.rs:409-425
- Fix: Extract a private helper `fn gh_api_paginated_with<F>(endpoint: &str, run_gh: &F) -> Result<String, GhError>` that performs the common run/success/stderr logic, then call it from each of the four public-facing functions with the appropriate endpoint string.

#### [near-clone] ImplPlanCodecError and TaskCoverageCodecError are byte-identical except for the type name (`after-062`)

- Rationale: Any change to the error message format (e.g. the UnsupportedSchemaVersion message, or the Validation delegation) must be applied in both codecs identically. A shared `JsonSchemaCodecError` type or a macro could unify them.
- Locations: libs/infrastructure/src/impl_plan_codec.rs:17-40, libs/infrastructure/src/task_coverage_codec.rs:17-40
- Fix: Define a single `JsonSchemaCodecError` (or rename to something domain-neutral) in a shared location within the infrastructure crate and type-alias or re-export it in each codec module. Alternatively, a `macro_rules!` can generate the boilerplate with the codec-specific name.

#### [near-clone] Missing-index guard block duplicated in check.rs and find_similar.rs (`after-015`)

- Rationale: The same business rule ('a missing/unrecognizable index is a hard error, not a silent empty result') is encoded in two places with nearly identical format strings. If the guard logic changes (e.g. adding a symlink check analogous to build.rs) or the error message template changes, both files must be updated in sync.
- Locations: apps/cli-composition/src/semantic_dup/check.rs:145-153, apps/cli-composition/src/semantic_dup/find_similar.rs:99-106
- Fix: Extract a shared helper in common.rs such as `pub(super) fn require_recognizable_index(db_path: &Path, command_name: &str) -> Result<(), String>` that performs the `is_recognizable_lancedb_index` check and returns the formatted error with the command_name prefix, then call it from both handlers.

#### [near-clone] MockDiffGetter and FailingDiffGetter mock structs duplicated across test modules (`after-140`)

- Rationale: Test helpers for the same port trait (DiffGetter) are copy-pasted across two test scopes within the same crate. A change to DiffGetter's signature would require updating both copies. Extractable to a shared #[cfg(test)] test_support submodule or test utilities module within the crate.
- Locations: libs/usecase/src/review_v2/tests.rs:58-84, libs/usecase/src/review_v2/scope_query.rs:311-334
- Fix: Move the two mock types to a crate-internal #[cfg(test)] mod test_support in review_v2/mod.rs (or a dedicated test_utils.rs) and re-use them from both test modules.

#### [near-clone] S-types vs C and S-functions vs C evaluation loops are structurally identical in phase2 (`after-098`)

- Rationale: Any change to the S-vs-C evaluation logic (e.g. adding a new `structurally_equal` branch, changing the sentinel action fallback, or augmenting the signal payload) must be replicated in both loops. The S-impls loop (lines 122-170) has additional stripped-key fallback logic so it is intentionally different and is excluded from this finding.
- Locations: libs/infrastructure/src/tddd/signal_evaluator_v2/phase2.rs:78-98, libs/infrastructure/src/tddd/signal_evaluator_v2/phase2.rs:101-119
- Fix: Extract a generic helper `fn evaluate_s_vs_c_for_map(s: &ExtendedCrate, s_map: &BTreeMap<String,Id>, c_krate: &Crate, c_map: &BTreeMap<String,Id>, crate_name: &str, signals: &mut Vec<ThreeWaySignal>)` and call it twice, once for types/traits and once for functions.

#### [near-clone] SimilarityScore::new and SimilarityThreshold::new encode the same [0.0, 1.0] range check (`after-036`)

- Rationale: The range rule [0.0, 1.0] is duplicated. If the valid range or the validation approach changes (e.g., excluding 0.0 or clamping rather than rejecting), both sites must be updated in sync.
- Locations: libs/domain/src/semantic_dup.rs:108-113, libs/domain/src/semantic_dup.rs:162-167
- Fix: Extract a private free function `validate_unit_f32(value: f32) -> bool` (or returning `Result`) and share it from both constructors. Alternatively, introduce a single macro or generic newtype for unit-interval f32 values.

#### [near-clone] `hex_pattern(byte: u8) -> String` duplicated in usecase and infrastructure (`after-180`)

- Rationale: Two copies encoding the same deterministic hash-generation rule. Used to build `ContentHash` test fixtures in tests for different components of the same pipeline.
- Locations: libs/usecase/src/catalogue_spec_refs.rs:662-668, libs/infrastructure/src/tddd/mod.rs:59-65
- Fix: Move `hex_pattern` to a shared domain-level test support module (e.g., `domain::ContentHash::test_support`) and import from both call sites.

#### [near-clone] `implemented` and `superseded` match arms in `decision_dto_to_entry` are structurally identical (`after-058`)

- Rationale: Both arms are within the same function and file; the repetition is ~10 lines each. Extraction into a shared closure or generic helper (taking the field name, status literal, `Option<String>` value, and a constructor `fn(AdrDecisionCommon, String) -> Result<T, E>`) would remove the duplication. The risk is low: the two arms are adjacent and the current code is easy to read, but a future third status with the same pattern (e.g., a hypothetical `replaced_by` field) would add a third copy.
- Locations: libs/infrastructure/src/adr_decision/parse.rs:200-211, libs/infrastructure/src/adr_decision/parse.rs:212-223
- Fix: Extract a private helper `fn require_typestate_field(id: &str, status: &str, field_name: &str, value: Option<String>) -> Result<String, AdrFrontMatterCodecError>` that performs the `ok_or_else` check with the standard message. The `.map` / `.map_err` wrapping at the constructor call site remains per-arm since the constructors are distinct types, but the `ok_or_else` boilerplate is eliminated.

#### [near-clone] read_enabled_layers and read_catalogue_spec_signal_opted_in_layers are near-clones (`after-111`)

- Rationale: Both methods share ~12 lines of identical fetch + parse + error-handling logic. The only difference is a filter predicate in the collection step. A bug in the shared prefix (e.g., wrong error message for architecture-rules.json parse errors) must be fixed in both. The comment on line 244 even says 'Mirrors read_enabled_layers', confirming the duplication is intentional but unfactored.
- Locations: libs/infrastructure/src/verify/merge_gate_adapter.rs:218-238, libs/infrastructure/src/verify/merge_gate_adapter.rs:240-268
- Fix: Extract a private helper `fn fetch_and_parse_tddd_bindings(&self, branch: &str) -> BlobFetchResult<Vec<TdddLayerBinding>>` and call it from both methods, passing only the filter/collect lambda as a closure or by making the helper return the full binding list.

#### [near-clone] spec.json read-and-decode boilerplate duplicated in spec_attribution and spec_signals verify_from_spec_json (`after-117`)

- Rationale: Two copies of a ~15-line read+decode boilerplate. Error message wording is intentionally consistent, but maintaining that consistency across copies requires discipline. The pattern is not architectural separation — both files call the same codec and produce the same error shape.
- Locations: libs/infrastructure/src/verify/spec_attribution.rs:43-61, libs/infrastructure/src/verify/spec_signals.rs:175-193
- Fix: Extract a `load_spec_doc(path: &Path) -> Result<SpecDocument, VerifyOutcome>` helper in the shared verify helpers module. Both callers replace their boilerplate with a single `match load_spec_doc(spec_json_path) { Ok(doc) => doc, Err(o) => return o }`.

#### [near-clone] temp_build_path and backup_path_for are structurally identical functions (`after-013`)

- Rationale: Identical 8-line bodies differing only in a suffix string literal. Changing the error message wording or the extraction logic requires two edits. Clear extraction opportunity: a private `fn hidden_sibling(db_path: &Path, suffix: &str) -> Result<PathBuf, String>` shared by both.
- Locations: apps/cli-composition/src/semantic_dup/build.rs:143-151, apps/cli-composition/src/semantic_dup/build.rs:160-168
- Fix: Extract a private `fn hidden_sibling_path(db_path: &Path, suffix: &str) -> Result<PathBuf, String>` that does the file_name/parent extraction, then implement `temp_build_path` and `backup_path_for` as one-liner wrappers calling it with `"tmp-build"` and `"old"` respectively.

#### [semantic-dup] SHA-256 hex validation logic duplicated between coverage.rs and value_objects.rs (`after-040`)

- Rationale: Same validation rule expressed twice. If the rule changed (e.g., supporting SHA-3 with a different length, or restricting to uppercase), both sites must be updated. The fix is straightforward: move `is_valid_sha256_hex` to a shared module-level location (e.g., a `dry_check::hash_util` submodule or a pub(super) fn in `dry_check/mod.rs`) so all fingerprint and hash types can import it.
- Locations: libs/domain/src/dry_check/coverage.rs:7-8, libs/domain/src/dry_check/value_objects.rs:28
- Fix: Promote `is_valid_sha256_hex` to a `pub(super)` function at the `dry_check` module level (e.g., in `mod.rs` or a new `dry_check/util.rs`) so both `coverage.rs` and `value_objects.rs` can import and call it, eliminating the inline duplicate.

#### [structural-dup] Catalogue table header/separator lines duplicated inside a single render function (`after-173`)

- Rationale: Two copies within one function. Adding a column requires editing both blocks; a typo in one will cause inconsistent table formatting between canonical and unknown-role sections.
- Locations: libs/infrastructure/src/type_catalogue_render.rs:457-462, libs/infrastructure/src/type_catalogue_render.rs:535-540
- Fix: Extract an `emit_table_header(out: &mut String, has_spec_signals: bool)` inline helper (or a macro) and call it from both sites.

#### [structural-dup] Check-and-raise pattern for SemanticFailuresConfirmed / HumanEscalationRequired appears twice (`after-137`)

- Rationale: The two check-and-raise blocks are short (4 lines each) but encode the identical error-priority rule: Fail surfaces before Pending. Divergence here (e.g., reordering the checks in one place) would produce inconsistent gate behaviour between the D12 skip path and the main path. A shared helper such as `fn raise_gate_errors(fails: usize, pending: usize) -> Result<(), RefVerifyError>` would unify both.
- Locations: libs/usecase/src/ref_verify/interactor.rs:170-175, libs/usecase/src/ref_verify/interactor.rs:328-333
- Fix: Extract `fn raise_verdict_gate(fails: usize, pending: usize) -> Result<(), RefVerifyError>` that checks fails then pending and returns the appropriate error or Ok(()), then call it from both the D12 early-return block and the end of the main path.

#### [structural-dup] DataRole and ContractRole duplicate Display/TryFrom<&str> scaffolding (`after-055`)

- Rationale: Two copies of the same three-block pattern. A change to the pattern (e.g., adding a new Display format convention or changing TryFrom error type) must be replicated in both places.
- Locations: libs/domain/src/tddd/catalogue_v2/roles.rs:128-198, libs/domain/src/tddd/catalogue_v2/roles.rs:225-269
- Fix: Introduce a private trait `VariantNamed { fn variant_name(&self) -> &'static str; }` and a single blanket `impl<T: VariantNamed + FromStr<Err=strum::ParseError>> Display for T` and `impl<T: FromStr<Err=strum::ParseError>> TryFrom<&str> for T`. Alternatively, a macro_rules! that emits variant_name, Display, and TryFrom for a given enum.

#### [structural-dup] Duplicated `implemented_in` non-empty guard in AcceptedDecision::implement and ImplementedDecision::new (`after-034`)

- Rationale: Two copies of the same guard encode the same invariant. A change to validation logic (e.g., trim-based like DecisionGroundRef) would need to be applied in both places. The duplication is mild (two sites, small block) but structurally redundant given that implement delegates to new conceptually.
- Locations: libs/domain/src/adr_decision/state.rs:79-87, libs/domain/src/adr_decision/state.rs:145-153
- Fix: Have `AcceptedDecision::implement` delegate to `ImplementedDecision::new` directly: `Ok(ImplementedDecision::new(self.common, implemented_in)?)`. This eliminates the duplicated guard while keeping the transition API intact.

#### [structural-dup] ErrorReader test double duplicated across two test modules (`after-129`)

- Rationale: Two identical test doubles for the same failure scenario. Low severity because test doubles rarely diverge in ways that cause bugs, but they add friction when the DryCheckReader trait or DryCheckReaderError type changes.
- Locations: libs/usecase/src/dry_check/approval_interactor.rs:257-265, libs/usecase/src/dry_check/results_interactor.rs:131-139
- Fix: Consolidate into a single `ErrorReader` in `shared::test_mocks` alongside the existing mock helpers.

#### [structural-dup] Four interactor structs repeat the same two-field layout, Debug impl, and new() constructor (`after-145`)

- Rationale: Four copies of identical struct scaffolding (fields + Debug + new). Adding a third port dependency (e.g. a cache port) would require touching all four structs. The repeated Debug impl with static string literals is boilerplate that a shared inner struct or a derive macro would eliminate.
- Locations: libs/usecase/src/semantic_dup/interactor.rs:86-108, libs/usecase/src/semantic_dup/interactor.rs:134-156, libs/usecase/src/semantic_dup/interactor.rs:238-261, libs/usecase/src/semantic_dup/interactor.rs:290-316
- Fix: Extract a shared `SemanticPorts { embedding_port: Arc<dyn EmbeddingPort>, index_port: Arc<dyn SemanticIndexPort> }` struct (which can derive Debug via a wrapper or implement it once), and embed it in each interactor as a single field. Each interactor's `new()` can delegate to `SemanticPorts::new()`.

#### [structural-dup] InformalGroundRefDto wire struct defined identically in two codec modules (`after-079`)

- Rationale: The two structs are wire-format identical — they represent the same JSON shape and map to the same domain type (InformalGroundRef). The only difference is visibility (private vs pub(crate)). A single pub(crate) struct in a shared module would serve both codecs, eliminating the risk of field-name drift between the two wire representations.
- Locations: libs/infrastructure/src/spec/codec.rs:126-132, libs/infrastructure/src/tddd/spec_ground_codec.rs:55-62
- Fix: Move InformalGroundRefDto (and its encode/decode helpers) to a shared module (e.g., libs/infrastructure/src/shared/informal_ground_dto.rs or reuse the existing tddd/spec_ground_codec.rs pub(crate) definition by making it accessible to spec/codec.rs), then delete the private copy in spec/codec.rs.

#### [structural-dup] Io/SymlinkDetected error-variant pattern repeated across three error enums (`after-049`)

- Rationale: Three port error enums independently encode the same I/O-failure shape. The repetition is minor (2 variants, 2-3 lines each) and the enums are otherwise distinct (different unique variants), so the change-amplification risk is limited. However, a systemic I/O error shape change still requires three edits.
- Locations: libs/domain/src/review_v2/error.rs:48-61, libs/domain/src/review_v2/error.rs:64-81, libs/domain/src/review_v2/error.rs:84-95
- Fix: Extract a shared `PortIoError { path: String, detail: String }` and `SymlinkError { path: String }` newtype or struct, then wrap them in each port's enum: `ReviewReaderError::Io(PortIoError)`. Alternatively, a macro_rules! that stamps out the common I/O variants can reduce the three copies to one declaration site.

#### [structural-dup] Near-identical codec error enum shape in `ImplPlanCodecError` and `TaskCoverageCodecError` (`after-152`)

- Rationale: The two enums are structurally identical 10-line blocks including error message strings and From impls. Any change to the error message format or the From impl logic would need to be applied in both places. The variant names and behavior are identical; only the type name differs.
- Locations: libs/infrastructure/src/impl_plan_codec.rs:19-40, libs/infrastructure/src/task_coverage_codec.rs:19-40
- Fix: Extract a shared `TrackDocumentCodecError` enum (or a macro) with the three variants. Each codec module can either re-export it as a type alias or convert via `From`. Alternatively, use a macro to stamp out the boilerplate while keeping separate public types for API stability.

#### [structural-dup] Paired "routes" + "parses" test scaffolding repeated for every Dry/RefVerify subcommand variant (`after-032`)

- Rationale: Each "parses" test duplicates the parse invocation and field assertion already present in its paired "routes" test. If a subcommand arg name changes, both tests in each pair must be updated. The "parses" tests provide no additional test coverage that the "routes" tests do not already exercise (successful routing implies successful parsing). The repeated scaffolding across five pairs is the structural-dup.
- Locations: apps/cli/src/main.rs:595-619, apps/cli/src/main.rs:622-649, apps/cli/src/main.rs:651-681, apps/cli/src/main.rs:693-727, apps/cli/src/main.rs:729-765
- Fix: Remove the five "_parses_" tests. The "_routes_" tests already exercise the parse path (try_parse_from must succeed and the correct variant must be matched for the closure to fire). If explicit parse-shape assertions are desired as documentation, consolidate them into a single table-driven test that iterates over (args, expected-variant-label) pairs.

#### [structural-dup] Parallel AST word-tree traversal chains for flatten vs collect-substitutions in flatten.rs (`after-077`)

- Rationale: Both traversal families encode the knowledge of how the conch-parser word AST is structured. A change to how a new Word variant should be handled (e.g., a future conch-parser update) requires parallel edits in both families. The duplication is a consequence of the two different accumulation strategies (String vs Vec), which is the intentional divergence, but the scaffolding structure is fully replicated.
- Locations: libs/infrastructure/src/shell/flatten.rs:19-73, libs/infrastructure/src/shell/flatten.rs:242-278
- Fix: A visitor abstraction or a single generic `walk_word` function parameterised over a mutable context type could unify the structural traversal. However, given the different return types, a macro or a trait with associated methods may be more ergonomic than a closure-based visitor.

#### [structural-dup] Repeated CommandOutcome stdout-print + exit-code pattern across all execute_* wrappers (`after-030`)

- Rationale: 17 repetitions of the same 3-line terminal pattern. Abstraction is clearly warranted and already exists in sibling `verify.rs` (`print_outcome`). However, the existing `print_outcome` in `verify.rs` is `pub(super)` and scoped to that module; a shared utility in the `track` module or a crate-internal helper would avoid the repetition without cross-module visibility issues.
- Locations: apps/cli/src/commands/track/state_ops.rs:21-24, apps/cli/src/commands/track/tddd/baseline_graph.rs:26-29
- Fix: Introduce a small helper `fn emit_outcome(outcome: CommandOutcome) -> Result<ExitCode, CliError>` in `apps/cli/src/commands/track/mod.rs` (or a shared `outcome_util.rs`) that prints stdout and returns the exit code, then replace all 17 boilerplate tails with a single call.

#### [structural-dup] Repeated expand-then-compile_glob pattern for three glob groups in ReviewScopeConfig::new (`after-050`)

- Rationale: Three structurally identical expand+compile blocks. Adding a new glob group (e.g. a fourth category of pattern) would require copy-pasting the same boilerplate again. The blocks are short (~8 lines each) and differ enough (expansion fn, error variant) that extraction is non-trivial but feasible with a closure parameter.
- Locations: libs/domain/src/review_v2/scope_config.rs:66-75, libs/domain/src/review_v2/scope_config.rs:80-88, libs/domain/src/review_v2/scope_config.rs:91-99
- Fix: Extract a helper `fn compile_patterns<F>(patterns: &[String], expand: impl Fn(&str) -> String, make_err: F) -> Result<Vec<GlobMatcher>, ScopeConfigError> where F: Fn(String, globset::Error) -> ScopeConfigError`, then call it three times with the appropriate expansion function and error constructor. This reduces each call site to ~3 lines.

#### [structural-dup] Repeated full TrackBlobReader panic-stub impls across test helpers in two modules (`after-123`)

- Rationale: If TrackBlobReader gains a new method, every one of these test structs must be updated. A blanket default-panic impl in a test utility module would reduce future maintenance burden. Currently this is borderline but concrete: the 3 identical panic-body stub impls appear verbatim in both FixedReader structs.
- Locations: libs/usecase/src/catalogue_spec_refs.rs:385-435, libs/usecase/src/catalogue_spec_signals.rs:411-457
- Fix: Introduce a `PanicTrackBlobReader` base struct in a shared test utility module with blanket panic impls for all trait methods, then compose it in each concrete reader.

#### [structural-dup] Repeated read/decode/write/mkdir RepositoryError::Message format! blocks in FsTrackStore for metadata.json and impl-plan.json (`after-149`)

- Rationale: The file paths differ (metadata.json vs impl-plan.json) but the error-wrapping logic is the same knowledge: 'I/O failure on this path → RepositoryError::Message with a human-readable string'. Adding a new I/O step or changing the error format requires edits in both blocks. A small helper fn fs_read / fs_write / fs_decode that returns RepositoryError would unify them.
- Locations: libs/infrastructure/src/track/fs_store.rs:40-100, libs/infrastructure/src/track/fs_store.rs:260-295
- Fix: Extract helper functions such as fs_read_to_string(path) -> Result<String, RepositoryError> and atomic_write_json(path, content) -> Result<(), RepositoryError> to eliminate the repeated map_err to RepositoryError::Message pattern.

#### [structural-dup] Repeated symlink-metadata guard match block in baseline_capture.rs (`after-082`)

- Rationale: Two copies of a 14-line match block within the same function. Divergence would cause inconsistent error messages or missed guard cases. Severity is low because the two roots serve distinct security purposes, the duplication is local and contained, and the existing `reject_symlinks_below` utility in the same crate (called at line 98) already factors out the general guard logic for sub-paths.
- Locations: libs/infrastructure/src/tddd/baseline_capture.rs:57-71, libs/infrastructure/src/tddd/baseline_capture.rs:72-86
- Fix: Extract a private `guard_root_against_symlink(path: &Path, name: &str, err_fn: impl Fn(String) -> CaptureBaselineError) -> Result<(), CaptureBaselineError>` helper and call it twice with the respective path and label string.

#### [structural-dup] Repeated symlink_metadata directory-check match in TelemetryReport::aggregate (`after-105`)

- Rationale: Both blocks encode the same knowledge: 'check that a path is a real (non-symlink) directory, distinguishing not-found from wrong-type from other I/O errors'. A change to how non-directory paths are handled (e.g. returning a different error variant) would require updating both blocks. The blocks are ~9 lines each and appear in the same function.
- Locations: libs/infrastructure/src/telemetry/report.rs:178-195, libs/infrastructure/src/telemetry/report.rs:212-227
- Fix: Extract a private `require_dir(path: &Path, io_err_path: &str) -> Result<bool, TelemetryReportError>` helper that returns `Ok(false)` for NotFound and `Err(Io)` otherwise, replacing both match blocks with a two-line guard call.

#### [structural-dup] StubStore test helper duplicated in track_phase and task_ops test modules (`after-125`)

- Rationale: The struct definition and the two read-only trait impls (TrackReader, ImplPlanReader) are byte-identical across the two modules. Because both modules live in the same crate, a shared `#[cfg(test)]` helper module (e.g. `libs/usecase/src/test_helpers.rs`) could eliminate the duplication.
- Locations: libs/usecase/src/track_phase.rs:161-180, libs/usecase/src/task_ops.rs:672-721
- Fix: Move the common `StubStore` + read-only trait impls into a crate-private `#[cfg(test)] mod test_helpers` module. `task_ops` extends it with the `TrackWriter`/`ImplPlanWriter` impls locally, or adds those to the shared definition.

#### [structural-dup] Three identical reject_symlinks_below guard match blocks in execute_type_signals_for_layer (`after-102`)

- Rationale: Three copies of the same 7-line pattern within one function. A single `guard_path(path, root, label)` helper that returns `Result<(), EvaluateSignalsError>` would replace all three blocks and reduce the risk of one copy being updated while others diverge.
- Locations: libs/infrastructure/src/tddd/type_signals_evaluator.rs:165-175, libs/infrastructure/src/tddd/type_signals_evaluator.rs:183-191, libs/infrastructure/src/tddd/type_signals_evaluator.rs:312-320
- Fix: Extract a `guard_symlink(path: &Path, root: &Path, label: &str) -> Result<(), EvaluateSignalsError>` closure or local helper inside `execute_type_signals_for_layer`, or reuse the existing `reject_symlinked_type_signals_anchor` helper by passing `canonical_items` as the anchor.

#### [structural-dup] Three near-identical nullable-non-blank guards repeated in validate_review_payload (`after-142`)

- Rationale: Three copies of the same predicate mean that adding a new optional string field (e.g., `rule_id`) requires adding a fourth copy. The pattern is clear enough to extract into a small closure or helper such as `fn nullable_non_blank(opt: Option<&str>) -> bool`. The current code is ~9 lines of near-identical boilerplate.
- Locations: libs/usecase/src/review_workflow/verdict.rs:217-222, libs/usecase/src/review_workflow/verdict.rs:224-229, libs/usecase/src/review_workflow/verdict.rs:235-239
- Fix: Replace the three guards with a helper closure: `let nullable_non_blank = |opt: Option<&str>| opt.is_some_and(|v| v.trim().is_empty());` then call `findings.iter().any(|f| nullable_non_blank(f.severity.as_deref()))`, etc. This makes adding a new field a one-liner.

#### [structural-dup] Three newtype-String error structs with identical boilerplate in catalogue_impl_signals_ports.rs (`after-053`)

- Rationale: Three copies of the same boilerplate mean that if the Display format or derive set is ever changed (e.g. adding `thiserror::Error`), all three sites must be updated simultaneously. The pattern is repeated at 3+ locations within a single file.
- Locations: libs/domain/src/tddd/catalogue_v2/catalogue_impl_signals_ports.rs:334-343, libs/domain/src/tddd/catalogue_v2/catalogue_impl_signals_ports.rs:387-396, libs/domain/src/tddd/catalogue_v2/catalogue_impl_signals_ports.rs:425-434
- Fix: Consider using a single generic `OpaqueStringError<Tag>` newtype or a shared macro. Alternatively, convert all three to use `thiserror::Error` with a single `#[error("{0}")]` derive, eliminating the manual `Display` impl entirely.

#### [structural-dup] Track-directory guard block duplicated in write_overview and write_cluster (`after-086`)

- Rationale: The guard block is not complex (8 lines), but it encodes a security-critical invariant (symlink-then-existence check order). The file comment at line 132 even says 'Same symlink pre-check as write_overview', acknowledging the repetition. Extracting a `validate_track_dir(track_dir, track_id, trusted_root) -> Result<PathBuf, BaselineGraphWriterError>` helper would factor it out and make any future change to the guard logic apply in one place.
- Locations: libs/infrastructure/src/tddd/baseline_graph_writer_adapter.rs:93-106, libs/infrastructure/src/tddd/baseline_graph_writer_adapter.rs:131-141
- Fix: Extract a private `validate_track_dir(&self, track_id: &TrackId) -> Result<PathBuf, BaselineGraphWriterError>` method that performs the symlink guard and existence check, returning the `track_dir` path. Both `write_overview` and `write_cluster` call it first.

#### [structural-dup] dispatch_subst_commands pattern repeated four times across conch.rs (`after-076`)

- Rationale: Four identical double-nested loops encode the same knowledge: 'walk a Vec<Vec<TopLevelCommand>> and recurse into each command at depth+1'. Any change to this dispatch logic (e.g., error handling or depth increment strategy) must be replicated four times.
- Locations: libs/infrastructure/src/shell/conch.rs:319-323, libs/infrastructure/src/shell/conch.rs:382-386, libs/infrastructure/src/shell/conch.rs:437-441, libs/infrastructure/src/shell/conch.rs:457-460
- Fix: Extract `fn dispatch_subst_commands(subst_commands: Vec<Vec<ast::TopLevelCommand<String>>>, out: &mut Vec<SimpleCommand>, depth: usize) -> Result<(), ParseError>` and call it at each of the four sites.

#### [structural-dup] impl From<TrackReadError> for usecase error types duplicates the same RepositoryError matching pattern (`after-150`)

- Rationale: This is borderline — the match is short (7 lines each) and the target variant names differ. However it encodes the same knowledge: TrackReadError carries only a Repository arm, TrackNotFound is always mapped to a same-named variant, and any other RepositoryError is stringified. If TrackReadError gains a second arm (e.g., a permission error), both From impls must be updated.
- Locations: libs/usecase/src/task_ops.rs:113-122, libs/usecase/src/track_phase.rs:49-58
- Fix: Consider a macro or a shared helper trait that maps TrackReadError to a pair of (TrackNotFound, Generic) variants. Alternatively, accept the duplication given the small block size, but document the shared convention so future arms are added consistently.

#### [structural-dup] items_dir + workspace_root symlink guard pair duplicated across two entry-point functions (`after-112`)

- Rationale: The guard encodes a security invariant (refuse to use a symlinked trusted root). If the error message format or the guard semantics ever change, both files must be updated. The comment on catalogue_spec_refs.rs line 358 ('Mirrors execute_catalogue_spec_signals') confirms the copy is known. Return-type difference makes a direct extraction slightly non-trivial but not difficult.
- Locations: libs/infrastructure/src/verify/catalogue_spec_refs.rs:357-390, libs/infrastructure/src/verify/catalogue_spec_signals.rs:34-66
- Fix: Extract a free function `fn guard_path_not_symlink(path: &Path, label: &str) -> Result<(), String>` in `crate::verify::trusted_root` (the module already exists) and call it from both entry points, mapping the Err differently per their return types.

#### [structural-dup] make_input test helper duplicated in guard.rs and hooks_path_setup.rs test modules (`after-133`)

- Rationale: Test-only boilerplate, but the existing `crate::hook::test_support` module in `mod.rs` already centralises the `simple_command` helper for the same purpose. Adding `make_bash_input` there would be consistent with the established pattern and eliminate the duplicate.
- Locations: libs/usecase/src/hook/guard.rs:118-125, libs/usecase/src/hook/hooks_path_setup.rs:106-113
- Fix: Add a `make_bash_input(command: &str) -> HookInput` function to `crate::hook::test_support` (mod.rs lines 31-42) and replace the two local definitions with a use of that shared helper.

#### [structural-dup] track_baseline_graph and track_contract_map share identical resolve→validate→layer-filter→loader/writer/renderer→execute scaffold (`after-019`)

- Rationale: The shared preamble (ID resolution, TrackId construction, layer filter parsing, rules_path) would need to be changed in both methods if that logic changes. However, the concrete adapter types differ substantially so full deduplication would require a generic/trait-based interactor factory, which may not be warranted.
- Locations: apps/cli-composition/src/track/tddd.rs:91-136, apps/cli-composition/src/track/tddd.rs:144-187
- Fix: Extract the common preamble (resolve_id, typed_track_id, layer_filter_parsed, rules_path) into a private helper `fn resolve_render_inputs(track_id: Option<String>, items_dir: &PathBuf, workspace_root: &Path, layers: Option<String>) -> Result<(String, TrackId, Option<Vec<LayerId>>, PathBuf), String>`. Each method calls this helper then constructs its own adapter set.
