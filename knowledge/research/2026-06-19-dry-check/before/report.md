# DRY Violation Snapshot — before (c4da67a4)

Independent AI-based DRY-violation census (intra-unit + thematic finders -> adversarial verification).
Scope: src/ of the 5 first-party crates (incl. inline #[cfg(test)] modules). Excluded: vendor/** and integration tests/ dirs.

## Overview

| metric | value |
|---|---|
| totalLoc | 157104 |
| unitCount | 40 |
| totalFindings | 148 |
| weightedScore (high*3+med*2+low*1) | 279 |
| densityPerKLoc | 0.942 |
| weightedDensityPerKLoc | 1.776 |
| crossLayerFindings | 26 |
| unverifiedKept | 0 |

## Breakdown by severity

| severity | count |
|---|---|
| high | 23 |
| medium | 85 |
| low | 40 |

## Breakdown by category

| category | count |
|---|---|
| near-clone | 56 |
| structural-dup | 41 |
| exact-clone | 17 |
| semantic-dup | 15 |
| data-dup | 12 |
| knowledge-dup | 7 |

## Breakdown by layer (primary location)

| layer | count |
|---|---|
| infrastructure | 64 |
| cli-composition | 25 |
| cli | 22 |
| usecase | 20 |
| domain | 17 |

## Cross-layer findings — DRY gate blind-spot candidates

- **validate_track_id_str duplicated verbatim in three modules** _[high/exact-clone]_ (`before-019`)
  - apps/cli-composition/src/track/mod.rs:17-45, apps/cli-composition/src/verify.rs:148-176, apps/cli/src/commands/track/validate.rs:15-43
- **Track ID validation logic duplicated between verify.rs and domain ids.rs** _[high/knowledge-dup]_ (`before-001`)
  - apps/cli-composition/src/verify.rs:148-176, libs/domain/src/ids.rs:232-252
- **Track-ID slug validation rule duplicated across 3 usecase modules and the domain layer** _[high/knowledge-dup]_ (`before-109`)
  - libs/usecase/src/catalogue_impl_signals/mod.rs:37-75, libs/usecase/src/type_signals/interactor.rs:28-64, libs/usecase/src/baseline_capture/mod.rs:27-65, libs/domain/src/ids.rs:232-252
- **CODEX_BOT_LOGINS constant and is_codex_bot() duplicated across cli and cli-composition** _[high/near-clone]_ (`before-005`)
  - apps/cli-composition/src/pr/poll.rs:14-20, apps/cli/src/commands/pr.rs:155-161
- **check_reaction_zero_findings() duplicated across cli and cli-composition** _[high/near-clone]_ (`before-007`)
  - apps/cli-composition/src/pr/poll.rs:153-187, apps/cli/src/commands/pr.rs:315-349
- **check_comment_zero_findings() duplicated across cli and cli-composition** _[high/near-clone]_ (`before-008`)
  - apps/cli-composition/src/pr/poll.rs:189-220, apps/cli/src/commands/pr.rs:351-385
- **poll_review_for_cycle() entire algorithm duplicated across cli and cli-composition** _[high/near-clone]_ (`before-009`)
  - apps/cli-composition/src/pr/poll.rs:227-399, apps/cli/src/commands/pr.rs:388-588
- **init_git_repo_on_track_branch test helper duplicated in 6 locations** _[high/near-clone]_ (`before-145`)
  - apps/cli/tests/transition_integration.rs:25-64, apps/cli/src/commands/track/tddd/baseline.rs:108-142, apps/cli/src/commands/track/tddd/signals.rs:49-65, apps/cli/src/commands/track/tddd/catalogue_spec_signals.rs:54-61, apps/cli-composition/src/track/tddd.rs:614-630, libs/infrastructure/src/track/render_tests.rs:42-44
- **`is_safe_briefing_path` defined independently in two crates** _[high/semantic-dup]_ (`before-136`)
  - apps/cli/src/commands/review/codex_local.rs:119-142, apps/cli-composition/src/review_v2/mod.rs:623-642
- **MAX_NESTING_DEPTH constant defined independently in domain (text.rs) and infrastructure (conch.rs)** _[low/data-dup]_ (`before-042`)
  - libs/domain/src/guard/text.rs:13, libs/infrastructure/src/shell/conch.rs:25
- **Magic string "track/items" path repeated across layers instead of using the existing constant** _[low/data-dup]_ (`before-129`)
  - libs/infrastructure/src/track/render.rs:54, libs/usecase/src/catalogue_impl_signals/interactor.rs:151, libs/usecase/src/type_signals/interactor.rs:146, libs/usecase/src/baseline_capture/interactor.rs:107, apps/cli-composition/src/review_v2/commit_hash.rs:66, apps/cli-composition/src/track/tddd.rs:35
- **prepare_timestamped_path vs fixer_runtime_path — duplicate runtime-path generation** _[low/near-clone]_ (`before-126`)
  - libs/infrastructure/src/review_v2/review_fix_runner/spawn.rs:14-33, apps/cli/src/commands/review/codex_local.rs:240-252
- **`run_command` / `run_sotp` helpers independently defined in both `make` modules** _[low/structural-dup]_ (`before-143`)
  - apps/cli/src/commands/make.rs:195-204, apps/cli-composition/src/make.rs:677-690
- **SOTP_CODEX_BIN env-var name defined four times as CODEX_BIN_ENV** _[medium/data-dup]_ (`before-156`)
  - libs/infrastructure/src/review_v2/codex_reviewer.rs:25, libs/infrastructure/src/review_v2/review_fix_runner/mod.rs:23, apps/cli/src/commands/plan/mod.rs:21, apps/cli/src/commands/review/mod.rs:41
- **POLL_INTERVAL = Duration::from_millis(50) defined four times** _[medium/data-dup]_ (`before-157`)
  - libs/infrastructure/src/review_v2/codex_reviewer.rs:21, libs/infrastructure/src/review_v2/claude_reviewer.rs:42, apps/cli/src/commands/plan/mod.rs:19, apps/cli/src/commands/review/mod.rs:39
- **"tmp/reviewer-runtime" path defined as REVIEW_RUNTIME_DIR in three separate modules** _[medium/data-dup]_ (`before-158`)
  - libs/infrastructure/src/review_v2/codex_reviewer.rs:20, libs/infrastructure/src/review_v2/review_fix_runner/spawn.rs:12, apps/cli/src/commands/review/mod.rs:37
- **resolve_project_root duplicated across cli-composition and cli crates** _[medium/exact-clone]_ (`before-020`)
  - apps/cli-composition/src/track/mod.rs:48-66, apps/cli/src/commands/track/validate.rs:122-147
- **`tee_stderr_to_file` function duplicated verbatim in planner and reviewer codex spawners** _[medium/exact-clone]_ (`before-139`)
  - apps/cli/src/commands/plan/codex_local.rs:134-146, libs/infrastructure/src/review_v2/codex_reviewer.rs:389-401
- **validate_track_id slug-validation also duplicated in CLI layer (apps/cli and apps/cli-composition)** _[medium/knowledge-dup]_ (`before-117`)
  - apps/cli/src/commands/track/validate.rs:15-43, apps/cli-composition/src/verify.rs:148-176
- **normalize_check_status() and checks_summary() duplicated across cli and cli-composition** _[medium/near-clone]_ (`before-006`)
  - apps/cli-composition/src/pr/poll.rs:97-118, apps/cli/src/commands/pr.rs:163-179
- **Timestamped runtime-path builder re-implemented three times across codex/planner/fixer modules** _[medium/near-clone]_ (`before-140`)
  - libs/infrastructure/src/review_v2/codex_reviewer.rs:250-267, libs/infrastructure/src/review_v2/review_fix_runner/spawn.rs:14-33, apps/cli/src/commands/review/codex_local.rs:240-252
- **`cargo make ci` subprocess with log-file tee and last-20-lines error tail duplicated across two `make` modules** _[medium/near-clone]_ (`before-141`)
  - apps/cli/src/commands/make.rs:524-550, apps/cli-composition/src/make.rs:238-265
- **run_git test helper duplicated across 4 test modules** _[medium/near-clone]_ (`before-144`)
  - libs/infrastructure/src/git_cli/mod.rs:414-416, apps/cli/src/commands/git.rs:109-112, apps/cli-composition/src/review_v2/mod.rs:741-744, apps/cli/src/commands/track/tddd/catalogue_spec_signals.rs:43-52
- **Briefing-file-to-prompt conversion logic duplicated across three locations** _[medium/semantic-dup]_ (`before-137`)
  - apps/cli/src/commands/plan/codex_local.rs:53-64, apps/cli/src/commands/review/codex_local.rs:54-65, apps/cli-composition/src/review_v2/mod.rs:579-591
- **Parallel serde structs encoding the same review finding JSON shape: ReviewFinding (usecase) and FindingEntry (infrastructure)** _[medium/structural-dup]_ (`before-124`)
  - libs/usecase/src/review_workflow/verdict.rs:100-112, libs/infrastructure/src/review_v2/persistence/mod.rs:47-55
- **CwdGuard / CurrentDirGuard RAII struct duplicated in 7 test modules** _[medium/structural-dup]_ (`before-146`)
  - apps/cli/src/commands/git.rs:91-107, apps/cli/src/commands/review/tests.rs:72-88, libs/infrastructure/src/git_cli/mod.rs:396-412, apps/cli-composition/src/review_v2/commit_hash.rs:91-107, apps/cli-composition/src/review_v2/run.rs:292-308, apps/cli-composition/src/review_v2/mod.rs:695-700, apps/cli-composition/src/make.rs:706-711

## Full enumeration

### Severity: high

#### [exact-clone] build_full_prompt duplicated verbatim in ClaudeReviewer and CodexReviewer (`before-060`)

- Rationale: The prompt template text (scope header, file-list format, git-diff instruction) is a business-level knowledge item — how the review scope is communicated to the model. Both copies must change together for any wording update. Divergence would silently produce different reviewer behavior across providers, which is a behavioral bug. The duplication also triggers duplicate test coverage for the same logic.
- Locations: libs/infrastructure/src/review_v2/claude_reviewer.rs:129-148, libs/infrastructure/src/review_v2/codex_reviewer.rs:82-101
- Fix: Extract `build_full_prompt` into a free function in a shared `review_v2::prompt` sub-module (or the existing `usecase::review_workflow` layer), taking `(&str, &ReviewTarget, &str) -> String`. Both reviewer structs delegate to it. Tests can be consolidated to the shared location.

#### [exact-clone] sibling_spec_json helper duplicated verbatim in 4 files (`before-099`)

- Rationale: 4 copies, non-trivial logic (empty-parent edge-case handling). Shared `super::frontmatter` already proves the pattern of extracting shared helpers into a sibling module. This function belongs in `super::frontmatter` or a new `super::spec_path` helper and should be re-exported to the four callers.
- Locations: libs/infrastructure/src/verify/spec_attribution.rs:96-101, libs/infrastructure/src/verify/spec_frontmatter.rs:81-86, libs/infrastructure/src/verify/spec_signals.rs:309-314, libs/infrastructure/src/verify/spec_states.rs:466-471
- Fix: Move `sibling_spec_json` into `libs/infrastructure/src/verify/frontmatter.rs` (or a new `libs/infrastructure/src/verify/spec_path.rs` module), make it `pub(super)`, and replace each copy with `use super::frontmatter::sibling_spec_json;`.

#### [exact-clone] stub_binding() for TdddLayerBinding triplicated across three usecase test modules (`before-152`)

- Rationale: Three copies; any change to the canonical TdddLayerBinding field naming convention (catalogue_file / baseline_file suffix) would require three edits. These three test modules are all inside libs/usecase/src and could share a common test-support module.
- Locations: libs/usecase/src/catalogue_impl_signals/interactor_tests.rs:58-65, libs/usecase/src/type_signals/interactor_tests.rs:70-77, libs/usecase/src/baseline_capture/interactor_tests.rs:114-121
- Fix: Extract to a single location, e.g. libs/usecase/src/test_support.rs (or a test_helpers submodule), and re-export via `use crate::test_support::stub_binding;` in each test module.

#### [exact-clone] validate_track_id_str duplicated verbatim in three modules (`before-019`)

- Rationale: Three copies of non-trivial validation logic spanning two crates (cli-composition and cli). Divergence between them would silently produce different validation outcomes for the same input, which is a correctness risk. The mod.rs comment itself says '(mirrors apps/cli/src/commands/track/mod.rs)', confirming awareness of the duplication.
- Locations: apps/cli-composition/src/track/mod.rs:17-45, apps/cli-composition/src/verify.rs:148-176, apps/cli/src/commands/track/validate.rs:15-43
- Fix: Consolidate into the single public function already exposed through cli-composition's resolution façade (track/resolution.rs → CliApp::track_validate_id). The verify.rs call site can replace validate_track_id_str_local with super::track::validate_track_id_str or CliApp::new().track_validate_id. The cli crate can use CliApp::new().track_validate_id (already the thin-client convention). The cli-composition internal copy in track/mod.rs remains as the single implementation.

#### [knowledge-dup] Track ID validation logic duplicated between verify.rs and domain ids.rs (`before-001`)

- Rationale: Encodes a domain business rule (the exact character set and structural constraints for track IDs) in two places across architectural layers. A grammar change (e.g. allowing uppercase) would require updating both; a discrepancy would cause verify.rs to accept or reject IDs that TrackId itself would not.
- Locations: apps/cli-composition/src/verify.rs:148-176, libs/domain/src/ids.rs:232-252
- Fix: In `verify.rs`, call `domain::TrackId::try_new(value).map_err(|e| e.to_string()).map(|_| ())` instead of `validate_track_id_str_local`. Since `cli-composition` already has access to the domain crate (other methods use domain types indirectly via usecase), this eliminates the local copy while keeping the CN-02 public boundary clean.

#### [knowledge-dup] Track-ID slug validation rule duplicated across 3 usecase modules and the domain layer (`before-109`)

- Rationale: Three copies in the same usecase layer plus the canonical source in domain. The rule is non-trivial (edge cases around double-hyphen, trailing hyphen, leading digit). Divergence between copies would produce silently inconsistent validation for different commands.
- Locations: libs/usecase/src/catalogue_impl_signals/mod.rs:37-75, libs/usecase/src/type_signals/interactor.rs:28-64, libs/usecase/src/baseline_capture/mod.rs:27-65, libs/domain/src/ids.rs:232-252
- Fix: Expose the validation as a public or pub(crate) function from the usecase crate root (e.g. `usecase::shared::validate_track_id`) that wraps a call to the domain-level `TrackId::try_new` (which already encodes the same rule), or alternatively expose a pure `is_valid_track_id(&str) -> bool` helper from the domain crate and call it from all three usecase sites. This eliminates 3 redundant copies without changing the hexagonal-architecture boundary.

#### [near-clone] Baseline capture symlink guard + entire capture sequence duplicated across two adapters (`before-070`)

- Rationale: Any security tightening (e.g. adding a check, fixing error messages) must be applied in both places. The duplication was acknowledged inline with the "mirrors" comment but never resolved. The two files differ only in their type wrappers, making this a clear near-clone.
- Locations: libs/infrastructure/src/tddd/baseline_capture.rs:54-176, libs/infrastructure/src/tddd/rustdoc_baseline_capture_adapter.rs:69-191
- Fix: Extract the inner capture logic into a single private helper (e.g. in a shared module) parameterised over the binding fields (`baseline_filename`, `layer_id`, `target_crate`) and a common error closure. Both `capture_rustdoc_baseline_for_layer` and `capture_baseline_inner` become thin wrappers that adapt their binding types and delegate to the helper.

#### [near-clone] CODEX_BOT_LOGINS constant and is_codex_bot() duplicated across cli and cli-composition (`before-005`)

- Rationale: The business rule 'which GitHub logins count as the Codex bot' is encoded in two separate files. Divergence (e.g. one file updated, the other not) would cause tests to accept bot activity the production path ignores, or vice versa.
- Locations: apps/cli-composition/src/pr/poll.rs:14-20, apps/cli/src/commands/pr.rs:155-161
- Fix: Move CODEX_BOT_LOGINS and is_codex_bot into infrastructure::gh_cli (or a shared usecase helper) and have both consumers reference it. The infrastructure layer already knows about GhClient and is the natural home for this list.

#### [near-clone] CommonMark fence-tracking block repeated 4 times across 3 files (`before-100`)

- Rationale: 4 copies spanning 3 files. `plan_artifact_refs.rs` already extracted this same logic into proper helper functions (`detect_fence_open`, `is_fence_close`) that implement the identical rule; the other three files did not adopt those helpers, creating a knowledge divergence. A fix to the closing-fence rule in `plan_artifact_refs.rs` helpers would not propagate to the other sites.
- Locations: libs/infrastructure/src/verify/spec_attribution.rs:163-181, libs/infrastructure/src/verify/spec_signals.rs:91-109, libs/infrastructure/src/verify/spec_states.rs:354-371, libs/infrastructure/src/verify/spec_states.rs:393-409
- Fix: Promote the `detect_fence_open`, `is_fence_close` helpers from `plan_artifact_refs.rs` (or equivalents) into `libs/infrastructure/src/verify/frontmatter.rs` as `pub(super)` utilities, and replace the four inline blocks with calls to those helpers.

#### [near-clone] Four verdict-conversion helpers duplicated across claude_reviewer and codex_reviewer (`before-061`)

- Rationale: The verdict-parsing and findings-conversion logic encodes the mapping between `usecase::review_workflow` types and domain verdict types. Any change to `ReviewPayloadVerdict` arms, `VerdictError` handling, or `ReviewerFinding` constructor signature must be applied in both files. Divergence (e.g. handling a new verdict variant in one reviewer but not the other) would cause split behavior across providers. The duplicated test cases for `test_convert_findings_to_domain_skips_empty_message` and `test_convert_findings_to_domain_converts_valid_finding` confirm the duplication.
- Locations: libs/infrastructure/src/review_v2/claude_reviewer.rs:200-271, libs/infrastructure/src/review_v2/codex_reviewer.rs:172-243
- Fix: Introduce a `ReviewOutcomeLog` enum (or trait) that both `ReviewOutcomeRaw` types implement to yield a `LogInfo`. Extract the four functions into a shared `review_v2::verdict_convert` module. Each reviewer's `ReviewOutcomeRaw` provides a `log_info()` method; the shared `convert_raw_to_final` accepts `impl IntoLogInfo` or a concrete common type.

#### [near-clone] StubLayerBindings struct + TdddLayerBindingsPort impl triplicated across three usecase test modules (`before-153`)

- Rationale: Three copies spanning the same crate; any change to the TdddLayerBindingsPort signature propagates to all three. Collocated with the stub_binding() duplication — a single shared test-support module would fix both.
- Locations: libs/usecase/src/catalogue_impl_signals/interactor_tests.rs:206-218, libs/usecase/src/type_signals/interactor_tests.rs:16-28, libs/usecase/src/baseline_capture/interactor_tests.rs:57-69
- Fix: Move to libs/usecase/src/test_support.rs together with stub_binding(), exposing as pub(crate) for cfg(test) use.

#### [near-clone] `CliApp::review_run_codex` and `CliApp::review_run_claude` share the same implementation body (`before-135`)

- Rationale: Three copies of the same validation + prompt-build + briefing-safety-check logic. The warning message `[WARN] briefing_file for scope '{group}' contains unsafe characters — scope-specific severity policy injection skipped` appears three times. Any change to this business rule (e.g., adding a new validation step, changing the warning text) requires three edits. This is a classic structural-dup that a shared private function or trait could eliminate.
- Locations: apps/cli-composition/src/review_v2/mod.rs:52-88, apps/cli-composition/src/review_v2/mod.rs:99-140, apps/cli-composition/src/review_v2/mod.rs:152-253
- Fix: Extract the common pre-reviewer setup into a private helper `prepare_reviewer_prompt(group, track_id, items_dir, briefing_file, prompt) -> Result<(String, String), String>` returning `(group, base_prompt)`. Both `review_run_codex` and `review_run_claude` become a 3-line wrapper. `review_run_local` then calls the same helper before its provider dispatch.

#### [near-clone] check_comment_zero_findings() duplicated across cli and cli-composition (`before-008`)

- Rationale: The sentinel string 'Didn't find any major issues' is a knowledge rule about the Codex bot's output format. Encoding it in two locations means the test harness can diverge from the production path when the sentinel changes.
- Locations: apps/cli-composition/src/pr/poll.rs:189-220, apps/cli/src/commands/pr.rs:351-385
- Fix: Remove the duplicate from test_helpers; the tests in pr_tests.rs should exercise the composition layer directly rather than re-implementing it.

#### [near-clone] check_reaction_zero_findings() duplicated across cli and cli-composition (`before-007`)

- Rationale: The rule 'a post-trigger +1 reaction from a Codex bot means zero findings' is expressed in two places. A change to the detection rule (e.g. supporting a different reaction type) requires coordinated edits in both files.
- Locations: apps/cli-composition/src/pr/poll.rs:153-187, apps/cli/src/commands/pr.rs:315-349
- Fix: Consolidate into cli_composition::pr::poll (already the canonical location) and have the test_helpers module call it, or expose it from usecase::pr_review.

#### [near-clone] init_git_repo_on_track_branch test helper defined three times with structurally identical logic (`before-031`)

- Rationale: Three independent copies encode the same bootstrapping knowledge. Any invariant change (e.g. adding `--initial-branch=main` for deterministic CI behaviour, or switching from `branch -m` to `switch -c`) must be applied to all three. The `catalogue_spec_signals.rs` version already diverges (different git commands), which could cause subtly different test environment behaviour across modules.
- Locations: apps/cli/src/commands/track/tddd/baseline.rs:108-142, apps/cli/src/commands/track/tddd/signals.rs:49-65, apps/cli/src/commands/track/tddd/catalogue_spec_signals.rs:54-61
- Fix: Extract a shared `tests::git_fixtures::init_git_repo_on_track_branch` helper into a test-support crate or a `#[cfg(test)]` module inside `tddd/mod.rs` and call it from all three test modules.

#### [near-clone] init_git_repo_on_track_branch test helper duplicated in 6 locations (`before-145`)

- Rationale: Six copies across all layers (infrastructure, cli, cli-composition). The rule encoded here — "a valid track requires a git branch named track/<id> with at least one commit and git identity configured" — is a project invariant. Divergence already exists: some copies include `commit.gpgsign=false` and `--no-gpg-sign` (needed in CI environments without global gpg config) while others omit them, meaning some tests may silently fail in different environments.
- Locations: apps/cli/tests/transition_integration.rs:25-64, apps/cli/src/commands/track/tddd/baseline.rs:108-142, apps/cli/src/commands/track/tddd/signals.rs:49-65, apps/cli/src/commands/track/tddd/catalogue_spec_signals.rs:54-61, apps/cli-composition/src/track/tddd.rs:614-630, libs/infrastructure/src/track/render_tests.rs:42-44
- Fix: Provide a single `track_test_support::init_git_repo_on_track_branch(root, track_id)` function in a shared test-support crate (or as a public `#[cfg(test)]` export in `infrastructure`). Canonicalize the flags (include gpgsign=false) to fix the latent divergence.

#### [near-clone] poll_review_for_cycle() entire algorithm duplicated across cli and cli-composition (`before-009`)

- Rationale: This is the core review-polling algorithm. Having two implementations means bug fixes or behavior changes (e.g. adding a new zero-findings detection path) must be applied in both files. The T005 track comments in the production file describe a behavior change that the test_helpers copy does not fully reflect (find_latest_bot_review_in is extracted in prod but inlined in the test copy, allowing future divergence).
- Locations: apps/cli-composition/src/pr/poll.rs:227-399, apps/cli/src/commands/pr.rs:388-588
- Fix: The test_helpers functions exist so pr_tests.rs can call them with a fake GhClient. The correct fix is to make pr_tests.rs import and test cli_composition::pr::poll functions directly (or via the public CliApp surface), removing the duplicate implementation from apps/cli entirely.

#### [near-clone] run_codex_review_str and run_claude_review_str are near-identical functions (`before-011`)

- Rationale: The three nested helpers (finding_to_payload, render_verdict_final, render_verdict_fast) plus the outer round_type_str match block are byte-for-byte identical between the two functions (~80 lines duplicated). The divergence is only in one type parameter: CodexReviewer vs ClaudeReviewer. A bug fix or output format change must be applied in two places. Abstraction is clearly warranted: a generic fn run_review_str<R: Reviewer>(…, reviewer: R, builder: fn(…) -> …) or a single function accepting an enum/trait object would eliminate the duplication.
- Locations: apps/cli-composition/src/review_v2/run.rs:28-139, apps/cli-composition/src/review_v2/run.rs:158-269
- Fix: Extract the shared body into a generic function parameterized over the reviewer type (or accept a pre-built ReviewV2Composition via a trait), keeping run_codex_review_str and run_claude_review_str as thin wrappers that construct the reviewer and delegate.

#### [near-clone] validate_track_id slug-validation function triplicated across usecase modules (`before-102`)

- Rationale: Three copies of non-trivial validation logic (slug grammar with multiple distinct rules) live within the same crate. A grammar change requires touching all three files, and a divergence (e.g. one copy accepts uppercase, another does not) would cause inconsistent behaviour between use cases. The copies also triplicate the associated tests. The fix is straightforward: extract a generic helper (e.g. a free function returning `bool` or `Option<&str>` describing the failure reason) into a shared `usecase` internal module and call it from each interactor.
- Locations: libs/usecase/src/baseline_capture/mod.rs:27-65, libs/usecase/src/catalogue_impl_signals/mod.rs:37-75, libs/usecase/src/type_signals/interactor.rs:28-64
- Fix: Extract the core slug-validation predicate into a shared internal module (e.g. `libs/usecase/src/common/track_id_validation.rs`) as `fn validate_track_id_slug(id: &str) -> Result<(), String>` returning the failure reason as a `String`. Each module's typed `validate_track_id` wrapper becomes a one-liner: `validate_track_id_slug(id).map_err(|reason| XxxError::InvalidTrackId { reason })`.

#### [semantic-dup] ConfidenceSignal ↔ JSON string mapping duplicated across two signal codecs (`before-123`)

- Rationale: The bidirectional string-to-enum mapping for ConfidenceSignal encodes a core domain invariant (the canonical wire representation of the three signal colours). It is already expressed in three places: the two codecs above and indirectly in `domain::tddd::catalogue::TypeSignal::signal_as_str` (lines 303-308). The fallback arms have already diverged (`"unknown"` vs `"red"`), which is exactly the kind of accidental divergence DRY exists to prevent. If a fourth signal colour is ever added to `ConfidenceSignal`, three codec files must be updated consistently.
- Locations: libs/infrastructure/src/tddd/type_signals_codec.rs:178-211, libs/infrastructure/src/tddd/catalogue_spec_signals_codec.rs:139-163
- Fix: Add `as_str(&self) -> &'static str` and `from_str(s: &str) -> Result<Self, …>` methods to `ConfidenceSignal` in `libs/domain/src/signal.rs`, then replace the duplicated match arms in both codecs with calls to those methods. The existing `TypeSignal::signal_as_str` helper in catalogue.rs can then also delegate to `ConfidenceSignal::as_str`.

#### [semantic-dup] `is_safe_briefing_path` defined independently in two crates (`before-136`)

- Rationale: Any change to the security rule (e.g., adding a new rejection class) must be applied to both files. A divergence between the test-gated copy and the production copy would mean tests are validating a different predicate than what runs in production.
- Locations: apps/cli/src/commands/review/codex_local.rs:119-142, apps/cli-composition/src/review_v2/mod.rs:623-642
- Fix: Move `is_safe_briefing_path` into `cli_composition::review_v2` as a `pub(crate)` or `pub` function and remove the duplicate in codex_local.rs. The `#[cfg(test)]` guard on the cli copy suggests it was added to support tests that directly exercised the old codex_local path — those tests can be updated to call the composition-layer function.

#### [semantic-dup] scope_name string → ScopeName conversion repeated 5 times across 3 files (`before-012`)

- Rationale: Five independent re-encodings of the same conversion rule. scope.rs already has the validation helper validate_scope_for_track_str; the string-to-ScopeName conversion itself should be a single private helper (e.g., parse_scope_name_str) in scope.rs or shared.rs, called by all five sites.
- Locations: apps/cli-composition/src/review_v2/briefing.rs:36-44, apps/cli-composition/src/review_v2/briefing.rs:90-98, apps/cli-composition/src/review_v2/run.rs:46-52, apps/cli-composition/src/review_v2/run.rs:176-182, apps/cli-composition/src/review_v2/scope.rs:76-83
- Fix: Add a crate-private helper fn parse_scope_name(scope_name: &str) -> Result<ScopeName, String> in scope.rs (or shared.rs) and replace all five inline conversions with a call to it.

#### [semantic-dup] status_str → TrackStatus mapping encoded twice (`before-093`)

- Rationale: Both blocks express the same knowledge: which string token maps to which domain enum variant. They already diverge in their default arm — `render.rs` silently falls through to `Planned` while the adapter returns an error — which means a new variant would have different effects depending on the call path. Divergence in the completeness of status coverage is a latent bug risk.
- Locations: libs/infrastructure/src/track/render.rs:444-453, libs/infrastructure/src/track/track_status_reader_adapter.rs:55-67
- Fix: Add a `TrackStatus::from_str` (or `impl FromStr for TrackStatus`) to the domain crate. Both sites then delegate to it, eliminating the duplicated arm list and ensuring domain-level exhaustiveness. The render.rs `parse_track_status_str` can remain as a thin wrapper over the domain method.

### Severity: medium

#### [data-dup] "tmp/reviewer-runtime" path defined as REVIEW_RUNTIME_DIR in three separate modules (`before-158`)

- Rationale: The runtime directory path is a shared contract between the reviewer infrastructure adapters and the CLI layer. Three distinct `const` definitions plus inline `PathBuf::from("tmp/reviewer-runtime/...")` usages in review_fix_runner/mod.rs mean a path rename requires hunting down multiple sites across architectural layers.
- Locations: libs/infrastructure/src/review_v2/codex_reviewer.rs:20, libs/infrastructure/src/review_v2/review_fix_runner/spawn.rs:12, apps/cli/src/commands/review/mod.rs:37
- Fix: Define a single `pub const REVIEWER_RUNTIME_DIR: &str = "tmp/reviewer-runtime";` in the infrastructure crate (e.g., review_v2 module root) and import it everywhere, including in the CLI layer.

#### [data-dup] ConfidenceSignal <-> "blue"/"yellow"/"red" string mapping duplicated across two codec modules (`before-080`)

- Rationale: The string representations of ConfidenceSignal are a shared invariant pinned by ADR ("the canonical signal string values"). Both codecs must agree on this mapping. A new variant (e.g. Gray) or a renamed string requires coordinated edits. A shared confidence_signal_to_str / confidence_signal_from_str pair in a shared codec utility module would be the single authoritative definition.
- Locations: libs/infrastructure/src/tddd/catalogue_spec_signals_codec.rs:142-163, libs/infrastructure/src/tddd/type_signals_codec.rs:178-200
- Fix: Add two free functions (e.g. in a shared codec_utils module or directly on ConfidenceSignal via a codec-layer trait): confidence_signal_to_str(s: ConfidenceSignal) -> &'static str and confidence_signal_from_str(s: &str) -> Option<ConfidenceSignal>. Both codecs delegate to these.

#### [data-dup] Magic string "track/items" repeated 9+ times across make.rs and verify.rs (`before-004`)

- Rationale: More than 3 copies of a structural path constant. A rename would require grep-and-replace across two files with no compile-time safety net (the string is assembled at runtime). A single `const ITEMS_DIR: &str = "track/items"` shared between the two files eliminates the risk.
- Locations: apps/cli-composition/src/make.rs:177,186,275,321,334,534,545,556,567, apps/cli-composition/src/verify.rs:83,103
- Fix: Declare `const ITEMS_DIR: &str = "track/items";` in `lib.rs` (or a shared constants module) and replace all literal occurrences in `make.rs` and `verify.rs`.

#### [data-dup] POLL_INTERVAL = Duration::from_millis(50) defined four times (`before-157`)

- Rationale: The same numeric threshold (50 ms) is repeated four times across two crates. The polling pattern (sleep → check → repeat) is the same across all four sites. Tuning the poll interval requires four edits, and divergence would create inconsistent latency behaviour between reviewer and planner loops.
- Locations: libs/infrastructure/src/review_v2/codex_reviewer.rs:21, libs/infrastructure/src/review_v2/claude_reviewer.rs:42, apps/cli/src/commands/plan/mod.rs:19, apps/cli/src/commands/review/mod.rs:39
- Fix: Extract a single `pub const SUBPROCESS_POLL_INTERVAL: Duration = Duration::from_millis(50);` into a shared constants module (e.g., infrastructure) and re-export or import it from all consumer sites.

#### [data-dup] SOTP_CODEX_BIN env-var name defined four times as CODEX_BIN_ENV (`before-156`)

- Rationale: Four separate const definitions encoding the exact same env-var name string. A rename of the env var (e.g. renaming `SOTP_CODEX_BIN`) would require four edits, and missing any one would silently break the binary override in that code path. Spans two crates (infrastructure and cli).
- Locations: libs/infrastructure/src/review_v2/codex_reviewer.rs:25, libs/infrastructure/src/review_v2/review_fix_runner/mod.rs:23, apps/cli/src/commands/plan/mod.rs:21, apps/cli/src/commands/review/mod.rs:41
- Fix: Define a single `pub const CODEX_BIN_ENV: &str = "SOTP_CODEX_BIN";` in one shared location (e.g., a `constants` module in infrastructure or a shared crate), then import it everywhere else.

#### [exact-clone] FindSimilarError and DupCheckError are byte-identical enums (`before-118`)

- Rationale: The two types encode the same knowledge (embedding or index failure) in two identical shapes. A single type alias or a unified error type would remove the duplication without loss of expressiveness at call sites.
- Locations: libs/usecase/src/semantic_dup/errors.rs:82-90, libs/usecase/src/semantic_dup/errors.rs:92-101
- Fix: Unify into a single `SemanticOpError { Embedding(EmbeddingError), Index(SemanticIndexError) }` and type-alias or replace `FindSimilarError` / `DupCheckError` with it, or keep distinct names but derive one from the other via `type DupCheckError = FindSimilarError`.

#### [exact-clone] Identical `supersede` transition methods on AcceptedDecision and ImplementedDecision (`before-035`)

- Rationale: Both methods are 9 lines long, structurally and textually identical. They encode the same business rule in duplicate. If the invariant for `superseded_by` changes, both must change in sync.
- Locations: libs/domain/src/adr_decision/state.rs:98-106, libs/domain/src/adr_decision/state.rs:163-171
- Fix: Factor the shared validation + construction into a free function such as `fn make_superseded(common: AdrDecisionCommon, superseded_by: String) -> Result<SupersededDecision, AdrDecisionCommonError>` and call it from both transition methods, or delegate construction entirely to `SupersededDecision::new` (which already performs the same check) and remove the redundant guard in the transition methods.

#### [exact-clone] NonEmptyString and ReviewGroupName are byte-identical newtypes (`before-131`)

- Rationale: Both types encode exactly the same invariant (trim → non-empty) with the same error variant. The only distinction is the type name. Either ReviewGroupName should alias NonEmptyString, or a shared macro/generic should generate both.
- Locations: libs/domain/src/ids.rs:166-193, libs/domain/src/ids.rs:201-230
- Fix: Make ReviewGroupName a newtype wrapping NonEmptyString (delegating try_new), or generate both types from a declarative macro that captures the shared structure. The distinct type names remain useful for type safety at API boundaries.

#### [exact-clone] Test git() helper function cloned in show.rs and merge_gate_adapter.rs (`before-058`)

- Rationale: The two blocks are 23 lines and byte-for-byte identical. A divergence (e.g. one adds GIT_CONFIG_NOSYSTEM and the other doesn't) would silently produce different fixture behavior across the two test suites. Extraction to a crate-internal test utility module (e.g. `infrastructure::test_support::git`) would remove the duplication.
- Locations: libs/infrastructure/src/git_cli/show.rs:229-251, libs/infrastructure/src/verify/merge_gate_adapter.rs:467-488
- Fix: Extract the helper into a `#[cfg(test)]` module in `libs/infrastructure/src/test_support.rs` (or `git_cli/test_helpers.rs`) and re-export it for both test modules to use.

#### [exact-clone] `tee_stderr_to_file` function duplicated verbatim in planner and reviewer codex spawners (`before-139`)

- Rationale: Both bodies are structurally identical line-for-line: `BufReader::new(pipe)` → `reader.lines()` loop → `writeln!(log_file, ...)` + `eprintln!(...)` on `Ok` → `break` on `Err` → `log_file.flush()`. A divergence (e.g., one site adds credential redaction while the other does not) would be a silent behavioural bug.
- Locations: apps/cli/src/commands/plan/codex_local.rs:134-146, libs/infrastructure/src/review_v2/codex_reviewer.rs:389-401
- Fix: Extract a shared `tee_pipe_to_file<R: Read>(pipe: R, log_file: &mut File)` helper into a crate shared by both sites (e.g., `libs/infrastructure/src/process_util.rs` or a dedicated `subprocess` helper module). Both call sites become a one-liner call to that helper.

#### [exact-clone] items_dir canonicalization + containment guard block duplicated in shared.rs and scope.rs (`before-013`)

- Rationale: The block is a security guard (path traversal prevention). Having two independent copies means a patch to one site may silently leave the other vulnerable. A single helper fn verify_items_dir_containment(root: &Path, items_dir: &Path) -> Result<PathBuf, String> would give one auditable location for this invariant.
- Locations: apps/cli-composition/src/review_v2/shared.rs:102-131, apps/cli-composition/src/review_v2/scope.rs:16-49
- Fix: Extract the canonicalize + containment check into a private fn verify_items_dir_containment(root: &Path, items_dir: &Path) -> Result<PathBuf, String> in shared.rs. Call it from both build_v2_shared and load_scope_config_only.

#### [exact-clone] items_dir symlink guard duplicated across evaluator and adapter (`before-088`)

- Rationale: A change to the guard policy (e.g. adding a log message, adjusting the error message format, or handling a new OS error kind) must be applied in both files. The two blocks are functionally identical security checks; the only difference is the error type wrapper.
- Locations: libs/infrastructure/src/tddd/type_signals_evaluator.rs:90-104, libs/infrastructure/src/tddd/type_signals_executor_adapter.rs:119-133
- Fix: Extract a free function `guard_items_dir_not_symlink(items_dir: &Path) -> Result<(), String>` that returns an error string; each caller wraps the string into its local error type. This keeps the guard logic in one place.

#### [exact-clone] resolve_project_root duplicated across cli-composition and cli crates (`before-020`)

- Rationale: Two copies of the structural path rule '<root>/track/items'. A change to the expected directory layout would require updating both. The cli copy has a longer comment explaining the empty-root normalisation that the cli-composition copy lacks, indicating the two have already started drifting. cli-composition already exposes CliApp::track_resolve_project_root (resolution.rs:81) as a public façade that the cli crate could use instead.
- Locations: apps/cli-composition/src/track/mod.rs:48-66, apps/cli/src/commands/track/validate.rs:122-147
- Fix: Replace the cli copy with a call to CliApp::new().track_resolve_project_root(items_dir.to_path_buf()), which delegates to the cli-composition implementation. This is consistent with the thin-client convention already used for resolve_track_id and validate_track_id_str in cli/src/commands/track/validate.rs.

#### [exact-clone] resolve_track_id_for_write_with_reader and resolve_track_id_from_root_for_write_with_reader have identical bodies in the same test module (`before-032`)

- Rationale: Both helpers are in the same file and are byte-identical in their bodies. They exist to cover two production functions (`resolve_track_id_for_write` vs `resolve_track_id_from_root_for_write`) whose write-semantics are identical (same interactor call), so a single parameterised helper would suffice. The duplicated test suite (5+5 tests) means any AC-18 contract change requires edits in two places.
- Locations: apps/cli/src/commands/track/mod.rs:806-812, apps/cli/src/commands/track/mod.rs:879-885
- Fix: Merge the two helpers into one (e.g. `resolve_for_write_with_reader`) and run the shared five-test suite once. The production distinction between `items_dir` and `workspace_root` anchoring is already covered at the integration/CliApp level.

#### [exact-clone] track_dir symlink guard block appears twice within type_signals_executor_adapter.rs (`before-089`)

- Rationale: Any policy change to how a symlinked track directory is rejected (error message wording, log addition, new error variant) must be applied to both copies. The duplication is within a single function that has grown to 170+ lines of guard logic.
- Locations: libs/infrastructure/src/tddd/type_signals_executor_adapter.rs:164-181, libs/infrastructure/src/tddd/type_signals_executor_adapter.rs:244-259
- Fix: Extract a `guard_track_dir(items_dir: &Path, valid_track_id: &str) -> Result<PathBuf, TypeSignalsExecutionError>` helper that handles the symlink check and catalogue-presence check, returning the resolved `track_dir`.

#### [exact-clone] write_architecture_rules test helper copied verbatim across two test modules (`before-030`)

- Rationale: Byte-for-byte body clone (parameter name aside). Schema-version bumps or field additions must be applied in both places; divergence would silently produce different test scenarios. The files are in different modules (track/tddd vs. commands), so there is no natural sharing today.
- Locations: apps/cli/src/commands/track/tddd/catalogue_spec_signals.rs:109-133, apps/cli/src/commands/verify_catalogue_spec_refs.rs:70-94
- Fix: Extract a shared test fixture helper (e.g. in a `tests::fixtures` module or a dedicated `test_support` crate) and call it from both test modules.

#### [knowledge-dup] Non-empty trimmed string invariant re-implemented inline across domain types instead of delegating to NonEmptyString (`before-132`)

- Rationale: The same business invariant ('a field must not be empty or whitespace-only') is encoded 8+ times across 4 files. If the rule changes (e.g., also rejecting strings of only punctuation), every site must be found and updated. This is high-severity because the duplication spans multiple domain aggregates and because NonEmptyString already exists precisely to encode this invariant — the scattered re-implementations are working around it.
- Locations: libs/domain/src/ids.rs:178-181, libs/domain/src/plan.rs:24-30, libs/domain/src/spec.rs:52-53, libs/domain/src/spec.rs:149-151, libs/domain/src/spec.rs:212-216, libs/domain/src/impl_plan.rs:196-198, libs/domain/src/review_v2/types.rs:233-235
- Fix: In each constructor, replace the inline guard with a field stored as NonEmptyString (or a local type alias) and call NonEmptyString::try_new. Map the resulting ValidationError::EmptyString to the context-specific error variant at the call site. Storing as String internally is still fine; the change is to centralise the validation call.

#### [knowledge-dup] Round-type validation rule (`"fast" | "final"`) encoded in two separate places (`before-112`)

- Rationale: If a new round type (e.g. `"full"`) were added, both sites must be updated. The divergence risk is real: a developer adding the variant to `ReviewRoundType::parse()` may miss the inline match in `run_review_fix.rs`. The canonical parser already exists and is public.
- Locations: libs/usecase/src/review_v2/run_review.rs:34-40, libs/usecase/src/review_v2/run_review_fix.rs:171-178
- Fix: In `RunReviewFixInteractor::run()`, replace the inline match with a call to `ReviewRoundType::parse(&command.round_type).map_err(RunReviewFixError::InvalidRoundType)?;`. This reuses the canonical validation logic and eliminates the duplication.

#### [knowledge-dup] `superseded_by` non-empty invariant enforced in three separate sites (`before-036`)

- Rationale: Three copies of the same domain invariant exist in the same file. This is a textbook DRY violation: a single conceptual rule expressed in triplicate. `SupersededDecision::new` is the canonical home for this invariant; the two `supersede` transition methods could delegate to it, removing the redundant early returns.
- Locations: libs/domain/src/adr_decision/state.rs:102-103, libs/domain/src/adr_decision/state.rs:167-168, libs/domain/src/adr_decision/state.rs:216-217
- Fix: Have `AcceptedDecision::supersede` and `ImplementedDecision::supersede` delegate to `SupersededDecision::new` directly: `SupersededDecision::new(self.common, superseded_by)`. The validation guard then lives only in `SupersededDecision::new`, making it the single source of truth for the `superseded_by` invariant.

#### [knowledge-dup] validate_track_id slug-validation also duplicated in CLI layer (apps/cli and apps/cli-composition) (`before-117`)

- Rationale: Two further copies of the same business rule in the apps layer. Coupled with the three usecase copies, the codebase has five independent expressions of the same slug invariant. The canonical implementation lives in libs/domain/src/ids.rs (is_valid_track_id, lines 232-252); all five copies should delegate to it via TrackId::try_new.
- Locations: apps/cli/src/commands/track/validate.rs:15-43, apps/cli-composition/src/verify.rs:148-176
- Fix: Replace fn validate_track_id_str_local in apps/cli-composition/src/verify.rs and fn validate_track_id_str in apps/cli/src/commands/track/validate.rs with thin wrappers that call cli_composition::review_v2::validate_track_id_str (which already delegates to domain::TrackId::try_new).

#### [near-clone] AdrAnchor and ConventionAnchor are near-identical non-empty string newtypes (`before-044`)

- Rationale: Both types encode the same invariant ('anchor string must be non-empty') and have identical trait-impl bodies. A single conceptual change to the anchor validation policy forces edits in two files. The duplication is 8+ lines of structurally identical logic.
- Locations: libs/domain/src/plan_ref/adr_ref.rs:13-38, libs/domain/src/plan_ref/convention_ref.rs:12-41
- Fix: Introduce a private macro (e.g., `macro_rules! non_empty_string_newtype!`) or a generic `NonEmptyStr<E>` newtype parameterised on the error variant, and derive both AdrAnchor and ConventionAnchor from it. Given the ADR explicitly notes that strict validation is deferred to Q15, a shared base is the natural extraction point.

#### [near-clone] Branch-validation prolog duplicated in merge_gate and task_completion (`before-107`)

- Rationale: Both functions share ~14 lines of gate-opening logic that encodes the same business rule: a branch must pass ref-safety checks, must be a `track/<id>` branch, and must carry a valid TrackId slug. Any future change to this rule (e.g. adding a new forbidden character, changing error wording) requires two edits. The only meaningful difference is the caller-name string embedded in one error message, which is precisely the kind of parameterisation a shared helper accepts.
- Locations: libs/usecase/src/merge_gate.rs:244-262, libs/usecase/src/task_completion.rs:42-58
- Fix: Extract a private `validate_track_branch(branch: &str) -> Result<&str, VerifyOutcome>` helper (or similar) that returns the stripped track_id on success or a `VerifyOutcome` with a parameterised caller label on failure. Both `check_strict_merge_gate` and `check_tasks_resolved_from_git_ref` call this helper at their entry points.

#### [near-clone] BuildIndexError and MeasureQualityError are structurally identical error enums (`before-115`)

- Rationale: The two error types are identical knowledge: same variant set, same Display rules, same source-chain logic. A future structural change (e.g., adding a new variant or changing the Io message) must be applied twice. The Io variant and its Display arm (lines 124-126 and 174-176) are exact clones including the format string. This is a clear near-clone warranting extraction to a shared base error type or a macro.
- Locations: libs/usecase/src/semantic_dup/errors.rs:104-151, libs/usecase/src/semantic_dup/errors.rs:154-201
- Fix: Introduce a shared IndexingError (or similar) enum carrying the three variants and implement Display/Error/From on it, then type-alias or newtype it for BuildIndexError and MeasureQualityError, or use a macro to stamp out the two identical impls.

#### [near-clone] Child process termination logic (terminate_planner_child / terminate_child) duplicated (`before-026`)

- Rationale: The same OS-level process-group termination knowledge is duplicated. A bug fix in one (e.g., handling ESRCH on the Unix path) must be mirrored to the other. The difference in function name (`terminate_planner_child` vs `terminate_child`) obscures the shared intent.
- Locations: apps/cli/src/commands/plan/codex_local.rs:206-254, apps/cli/src/commands/review/codex_local.rs:423-469
- Fix: Extract `terminate_process_group(child: &mut Child) -> Result<(), String>` into a shared internal module (e.g., `cli::subprocess`) used by both plan and review codex-local executors.

#### [near-clone] CodexLocalArgs and ClaudeLocalArgs are near-identical arg structs (`before-133`)

- Rationale: A single change to any shared arg (adding `--verbose`, changing `default_value_t = DEFAULT_TIMEOUT_SECONDS`, renaming `--group` to `--scope`) forces two edits. The structs are large (≥8 fields each) and structurally identical except for provider-specific naming. Divergence in defaults or validation attributes would cause silent inconsistency between `codex-local` and `claude-local` subcommand behaviour.
- Locations: apps/cli/src/commands/review/mod.rs:86-130, apps/cli/src/commands/review/mod.rs:133-172
- Fix: Extract a shared `SharedReviewArgs` struct with `#[command(flatten)]` and compose it into both `CodexLocalArgs` and `ClaudeLocalArgs`. Alternatively, remove `ClaudeLocalArgs` entirely and make the provider selection an explicit enum flag on a single unified struct, since both subcommands already share the same `CodexRoundTypeArg` type and the same `validate_auto_record_args_raw` logic.

#### [near-clone] CurrentDirGuard test helper duplicated across git.rs and review/tests.rs (`before-024`)

- Rationale: The RAII guard for cwd changes is an exact duplicate across two modules. Any divergence (e.g., adding panic-on-restore-fail, making it Send) must be replicated.
- Locations: apps/cli/src/commands/git.rs:91-108, apps/cli/src/commands/review/tests.rs:72-89
- Fix: Move `CurrentDirGuard` into the shared test helpers module alongside `EnvVarGuard`.

#### [near-clone] Duplicate TrackStatus → TrackPhaseInfo match in resolve_phase and resolve_phase_from_record (`before-039`)

- Rationale: Four of six match arms are exact duplicates including hard-coded reason strings. Adding a new TrackStatus variant (or changing a reason string) requires edits in both functions. resolve_phase can be a two-line wrapper that calls resolve_phase_from_record after extracting the override reason from TrackMetadata.
- Locations: libs/domain/src/track_phase.rs:82-131, libs/domain/src/track_phase.rs:143-186
- Fix: Implement resolve_phase as: `let override_reason = track.status_override().map(|o| o.reason()); resolve_phase_from_record(derive_track_status(impl_plan, track.status_override()), override_reason)`, then remove the duplicated match arms.

#### [near-clone] Duplicate next_task and task_counts JSON shape construction in track/mod.rs and track/ops.rs (`before-125`)

- Rationale: Both files hardcode the same JSON key names (`task_id`, `description`, `status`, `total`, `todo`, `in_progress`, `done`, `skipped`) and the same `in_progress > 0` heuristic. A change to the output contract (e.g. renaming `task_id` to `id`, or adding a new count field) requires two edits that can easily diverge.
- Locations: apps/cli-composition/src/track/mod.rs:575-625, apps/cli-composition/src/track/ops.rs:167-220
- Fix: Extract `build_next_task_payload` and `build_task_counts_json` as shared free functions (or add typed response structs with `serde::Serialize`) in a shared ops utility module, then call them from both `TrackCommandHandler` and `TrackOps`.

#### [near-clone] Duplicated `is_valid_rust_identifier` with divergent semantics (`before-049`)

- Rationale: The duplicated logic encodes the same domain knowledge (what counts as a valid Rust identifier) in two places. The semantics diverge silently, making it easy for future edits to the two files to drift further apart. A shared crate-internal helper (or a re-export from `identifiers`) should encode the invariant once.
- Locations: libs/domain/src/tddd/catalogue.rs:49-61, libs/domain/src/tddd/catalogue_v2/identifiers.rs:58-67
- Fix: Extract a single private `is_valid_rust_identifier(s: &str, reject_bare_underscore: bool) -> bool` (or two named helpers) into `catalogue_v2::identifiers` and have `catalogue.rs` call it with the underscore-rejection flag set to `true`.

#### [near-clone] Embedding + index adapter construction block duplicated in `check.rs` and `find_similar.rs` (`before-017`)

- Rationale: The adapter-construction logic and its error messages are identical in both files. If the error message wording or construction strategy changes (e.g. adding a retry or a different adapter), both files must be updated. This is a classic structural duplication caused by the absence of a shared factory helper.
- Locations: apps/cli-composition/src/semantic_dup/check.rs:151-157, apps/cli-composition/src/semantic_dup/find_similar.rs:105-111
- Fix: Add a helper in `common.rs` such as `pub(super) fn build_embedding_and_index_ports(db_path: PathBuf) -> Result<(Arc<FastEmbedAdapter>, Arc<LanceDbSemanticIndexAdapter>), String>` and call it from both handlers.

#### [near-clone] EnvVarGuard test helper duplicated across plan/tests.rs and review/tests.rs (`before-023`)

- Rationale: Identical test scaffolding (`OnceLock<Mutex<()>>` lock function + RAII env-var guard) is copy-pasted between the two sibling test modules. Any change to the guard semantics (e.g., different SAFETY comment, panic-on-fail vs. silent) must be made in two places.
- Locations: apps/cli/src/commands/plan/tests.rs:13-45, apps/cli/src/commands/review/tests.rs:19-51
- Fix: Extract `env_lock()` and `EnvVarGuard` into a shared `apps/cli/src/commands/test_helpers.rs` (cfg(test)) module and re-export it into both plan::tests and review::tests.

#### [near-clone] Four paginated GitHub API functions share identical body structure (`before-056`)

- Rationale: Four copies of the same 8-line paginated-API call pattern. Any change to error representation or paginate behavior would require four separate edits. A helper fn like `gh_api_paginate(endpoint, run_gh) -> Result<String, GhError>` would consolidate to a single site.
- Locations: libs/infrastructure/src/gh_cli.rs:370-381, libs/infrastructure/src/gh_cli.rs:383-394, libs/infrastructure/src/gh_cli.rs:396-407, libs/infrastructure/src/gh_cli.rs:409-425
- Fix: Extract a private helper `fn gh_api_paginate_with<F>(endpoint: &str, run_gh: &F) -> Result<String, GhError>` that does the call and error handling, then have each list_*_with build the endpoint URL and delegate to it.

#### [near-clone] Identical convert_findings_to_domain in codex_reviewer and claude_reviewer (`before-122`)

- Rationale: Two copies of the same conversion function inside the same crate. A field rename in ReviewFinding (e.g. `file` → `file_path`) requires two edits that could diverge silently. The function should be factored into a shared helper module (e.g. `review_v2/conversion.rs`) and called from both reviewer adapters.
- Locations: libs/infrastructure/src/review_v2/codex_reviewer.rs:226-243, libs/infrastructure/src/review_v2/claude_reviewer.rs:254-271
- Fix: Extract `convert_findings_to_domain` into `libs/infrastructure/src/review_v2/conversion.rs` (or a `shared.rs`) and import it from both `codex_reviewer.rs` and `claude_reviewer.rs`.

#### [near-clone] Identical match-arm structure for `implemented` and `superseded` status dispatch (`before-054`)

- Rationale: Both arms are ~11 lines and share the same logic template. A single generic helper `fn build_decision_entry<T, F>(field: Option<String>, status: &str, field_name: &str, id: &str, ctor: F) -> Result<AdrDecisionEntry, AdrFrontMatterCodecError>` (or equivalent closure) would eliminate the duplication. Divergence risk: if the error format string or `map_err` pattern changes, only one arm may be updated.
- Locations: libs/infrastructure/src/adr_decision/parse.rs:156-167, libs/infrastructure/src/adr_decision/parse.rs:168-178
- Fix: Extract a private helper that takes the `Option<String>` field, the required-status label, the id, and a constructor closure, then reuse it for both `implemented` and `superseded` arms.

#### [near-clone] Identical trim-then-non-empty validation in NonEmptyString and ReviewGroupName (`before-038`)

- Rationale: Two copies of the same invariant (trim, reject empty) with the same error type. A future change to the validation logic (e.g., normalising Unicode whitespace) must be applied in both places or one copy silently diverges.
- Locations: libs/domain/src/ids.rs:178-181, libs/domain/src/ids.rs:214-217
- Fix: Extract a private `fn validate_trimmed_non_empty(value: impl Into<String>) -> Result<String, ValidationError>` in ids.rs and call it from both try_new implementations.

#### [near-clone] ImplPlanCodecError and TaskCoverageCodecError are structurally identical codec error enums (`before-119`)

- Rationale: A single change to the validation-error formatting convention (e.g. adding a prefix) requires edits in both files. The From conversion bodies are byte-for-byte identical (`Self::Validation(e.to_string())`). A shared macro or a common `JsonCodecError` base type could eliminate the duplication.
- Locations: libs/infrastructure/src/impl_plan_codec.rs:18-40, libs/infrastructure/src/task_coverage_codec.rs:18-40
- Fix: Define a shared `define_json_codec_error!` macro that emits the common three variants and the two From impls, then invoke it in each codec file. Alternatively, promote a shared `JsonSchemaV1CodecError` type to `infrastructure::shared` that both codecs wrap.

#### [near-clone] Layer-filter validation and application duplicated between baseline_graph_workflow and contract_map_workflow (`before-105`)

- Rationale: The same 'validate requested layers against loaded set, then produce a filtered ordered list' rule is encoded twice. A bug fix (e.g. deduplication edge case) or a policy change (e.g. case-insensitive comparison) would need to be applied to both interactors independently. Both files already call this out as intentionally symmetric in their module-level doc comments.
- Locations: libs/usecase/src/baseline_graph_workflow.rs:244-281, libs/usecase/src/contract_map_workflow.rs:199-226
- Fix: Extract a generic helper such as `fn resolve_layer_filter<E>(loaded_layers: &[LayerId], requested: Option<&[LayerId]>, make_not_found_err: impl Fn(LayerId) -> E) -> Result<Vec<LayerId>, E>` in a shared usecase utility module. Both interactors call the helper and map the error variant.

#### [near-clone] NoLayersBindings struct + impl duplicated across two usecase test modules (`before-154`)

- Rationale: Two copies in the same crate; the error variant name is encoded twice. A shared test-support module would eliminate this alongside StubLayerBindings and stub_binding().
- Locations: libs/usecase/src/baseline_capture/interactor_tests.rs:72-82, libs/usecase/src/type_signals/interactor_tests.rs:30-40
- Fix: Add to the shared test_support module described above.

#### [near-clone] Signal computation loop (types+traits+functions) copied into test (`before-076`)

- Rationale: The test replicates ~25 lines of production logic verbatim. Adding a new catalogue collection (e.g. inherent_impls) to the production loop requires the same edit in the test copy to keep coverage accurate.
- Locations: libs/infrastructure/src/tddd/catalogue_spec_signals_refresher.rs:151-175, libs/infrastructure/src/tddd/catalogue_spec_signals_refresher.rs:276-300
- Fix: Extract a free function (e.g. fn compute_signals(doc: &CatalogueDocument) -> Vec<CatalogueSpecSignal>) from the production triple-loop and call it from both refresh_one_layer and the test. The test then calls count_signals on the returned vec.

#### [near-clone] Structurally identical `WriterError` enums for baseline-graph and contract-map ports (`before-050`)

- Rationale: Both error types encode the same three failure modes (I/O, symlink rejection, missing track directory) with identical field shapes and message formats. Any new failure mode or field rename must be applied twice. A shared type (or a shared macro) would eliminate the duplication while preserving separate public names if needed.
- Locations: libs/domain/src/tddd/baseline_graph_ports.rs:187-231, libs/domain/src/tddd/catalogue_ports.rs:108-123
- Fix: Define a single parameterised error type (e.g. `WriterError<TrackId>`) in a shared module, or use `thiserror` with a common base. If the public API must expose distinct names, type-alias or wrap the common type.

#### [near-clone] Timestamped runtime-path builder re-implemented three times across codex/planner/fixer modules (`before-140`)

- Rationale: All three functions encode the same knowledge: 'a unique, time-stamped, per-process path under a known runtime directory'. The only intentional differences are the milliseconds vs. nanoseconds precision and the presence/absence of an atomic sequence counter — both of which represent independent decisions made at copy time rather than a deliberate API split. A centralised helper with configurable precision and an optional counter would cover all three call sites.
- Locations: libs/infrastructure/src/review_v2/codex_reviewer.rs:250-267, libs/infrastructure/src/review_v2/review_fix_runner/spawn.rs:14-33, apps/cli/src/commands/review/codex_local.rs:240-252
- Fix: Add a `runtime_path(dir: &str, prefix: &str, ext: &str) -> Result<PathBuf, String>` utility (e.g., in `libs/infrastructure/src/process_util.rs`) that uses nanoseconds + an atomic counter (the strictest variant). All three call sites replace their local implementations with a call to this utility, passing their respective `tmp/…-runtime/` directory constant.

#### [near-clone] Trusted-root symlink guard duplicated in three places (`before-092`)

- Rationale: All three blocks encode the same security knowledge: 'the trusted root passed to reject_symlinks_below must itself not be a symlink.' The logic is structurally identical (same three-arm match, same pattern); only the variable name (items_dir vs trusted_root) and error wrapper type differ. A future hardening — e.g., adding a TOCTOU note, surfacing a distinct error kind, or checking that the path is also absolute — must be applied in all three places to remain consistent. The comment in spec_element_hash.rs ("Mirrors execute_catalogue_spec_signals") acknowledges a fourth external copy exists.
- Locations: libs/infrastructure/src/track/fs_store.rs:339-353, libs/infrastructure/src/track/spec_element_hash.rs:55-69, libs/infrastructure/src/track/fs_spec_file_loader.rs:53-67
- Fix: Extract a free function `fn reject_symlinked_root(path: &Path, label: &str) -> Result<(), String>` (or a generic version parameterised on the error constructor) into `symlink_guard.rs` and call it from all three sites. The existing `symlink_guard.rs` module is the natural home.

#### [near-clone] `ReviewRunCodexInput` and `ReviewRunClaudeInput` are identical DTOs (`before-134`)

- Rationale: The two DTOs represent the same conceptual input data (a local reviewer invocation). All eight fields are identical in name and type. The only thing distinguishing them at call sites is which `CliApp` method they go to. A single new field (e.g., `max_retries`) would require two changes here plus two in the CLI args structs above.
- Locations: apps/cli-composition/src/review_v2/inputs.rs:7-16, apps/cli-composition/src/review_v2/inputs.rs:19-29
- Fix: Merge into a single `ReviewRunLocalProviderInput` that adds an explicit `provider: ReviewProvider` enum field (or pass the provider as a separate parameter to a unified `review_run_provider` method). This eliminates the parallel struct families.

#### [near-clone] `cargo make ci` subprocess with log-file tee and last-20-lines error tail duplicated across two `make` modules (`before-141`)

- Rationale: The `tmp/ci-output.log` file name, `saturating_sub(20)` tail logic, and `[track-commit-message]` diagnostics are all duplicated. Any change to the CI invocation (e.g., different log path, different tail line count, adding a spinner) requires edits in both files.
- Locations: apps/cli/src/commands/make.rs:524-550, apps/cli-composition/src/make.rs:238-265
- Fix: Extract `fn run_ci_with_log(log_path: &Path) -> Result<ExitCode, E>` into a shared subprocess helper. Both call sites delegate to it. The `[track-commit-message]` prefix can be passed as a parameter if needed.

#### [near-clone] `derive_filename_stem` implemented twice with the same `-types.json`-stripping logic (`before-074`)

- Rationale: Both functions encode the same naming convention knowledge: catalogue files follow the `<crate>-types.json` pattern. If that convention changes, both sites must be updated. The `catalogue_bulk_loader` copy even calls `CatalogueDocumentCodec::decode` (defined in the codec module), yet keeps its own copy of the same stem-derivation rule instead of reusing the codec module's helper.
- Locations: libs/infrastructure/src/tddd/catalogue_document_codec/mod.rs:272-281, libs/infrastructure/src/tddd/catalogue_bulk_loader.rs:384-390
- Fix: Promote one canonical `derive_filename_stem` function (returning `Option<String>`) into a shared location accessible to both callers (e.g., a small `filename_utils` module inside `crate::tddd`, or expose it from `CatalogueDocumentCodec`), and have both sites call it.

#### [near-clone] `review()` and `fast_review()` share near-identical orchestration bodies in `cycle.rs` (`before-110`)

- Rationale: Any future change to the orchestration flow (e.g., adding a retry, changing the hash-change detection strategy, adding tracing) must be applied to both branches. Current duplication is ~20 lines in each arm.
- Locations: libs/usecase/src/review_v2/cycle.rs:55-76, libs/usecase/src/review_v2/cycle.rs:85-108
- Fix: Extract a private generic helper `fn run_review_inner<V, F>(&self, scope: &ScopeName, invoke: F) -> Result<ReviewOutcome<V>, ReviewCycleError> where F: FnOnce(&ReviewTarget) -> Result<(V, LogInfo), ReviewerError>` that captures all shared steps. Both `review()` and `fast_review()` pass the appropriate closure.

#### [near-clone] build_prompt / build_base_prompt duplicated between plan and review Codex local modules (`before-025`)

- Rationale: The briefing-file-to-prompt conversion string `"Read {} and perform the task described there."` is a shared knowledge rule. If the wording changes it must be updated in both production paths.
- Locations: apps/cli/src/commands/plan/codex_local.rs:53-64, apps/cli/src/commands/review/codex_local.rs:54-65
- Fix: Extract a shared `build_briefing_prompt(briefing_file: Option<&Path>, prompt: Option<String>) -> Result<String, String>` free function in a shared module (e.g., `cli_composition::briefing`) and call it from both plan and review codex-local modules.

#### [near-clone] is_branch_delete and is_checkout_branch_create share an identical git-option-skipping preamble (`before-041`)

- Rationale: The git-global-option skipping loop (checking `GIT_OPTIONS_WITH_ARG`, `--` sentinel, and `starts_with('-')`) is written three times. The two checker functions could instead call `extract_git_subcommand` to obtain the subcommand index and then proceed with their specific flag scan, eliminating the duplicated traversal.
- Locations: libs/domain/src/guard/policy.rs:305-354, libs/domain/src/guard/policy.rs:358-405, libs/domain/src/guard/policy.rs:277-302
- Fix: Refactor `is_branch_delete` and `is_checkout_branch_create` to reuse `extract_git_subcommand` for finding the subcommand position, then start their flag-scan from the index returned. Alternatively, introduce a helper `find_git_subcommand_index(argv, git_index) -> Option<usize>` that returns the index of the subcommand token, and use it in all three callsites.

#### [near-clone] make_track_pr_push and make_track_pr_ensure are near-identical forwarders in make.rs (`before-002`)

- Rationale: Any change to the positional-track-id promotion guard must be made twice. The two functions are already composed in `make_track_pr` (line 401–408), showing the design anticipates them being called together — making a shared helper especially natural.
- Locations: apps/cli-composition/src/make.rs:351-370, apps/cli-composition/src/make.rs:376-395
- Fix: Extract a private helper `fn build_pr_forward_args(subcommand: &str, raw_args: &[String]) -> Vec<String>` and call it from both methods.

#### [near-clone] normalize_check_status() and checks_summary() duplicated across cli and cli-composition (`before-006`)

- Rationale: The mapping from raw gh-cli check state strings to the PrCheckStatus domain enum is a single business rule. Two separate implementations means a new terminal state (e.g. 'SKIPPED') could be added to one without updating the other.
- Locations: apps/cli-composition/src/pr/poll.rs:97-118, apps/cli/src/commands/pr.rs:163-179
- Fix: Promote these into usecase::pr_workflow (alongside PrCheckStatus and summarize_checks) or into a shared cli_composition helper that the test module can call directly.

#### [near-clone] patch_paths_crate_ids and patch_paths_crate_ids_extra are near-identical loop bodies (`before-081`)

- Rationale: A single conceptual change to the crate-id patching logic (e.g. adding a new guard, changing the lookup strategy) would require editing both functions. The only structural difference is the filter direction (exclude vs include) and the crate source; a unified helper accepting `source_crate`, `filter_mode: ExcludeOrInclude`, and `filter_ids` would eliminate the duplication.
- Locations: libs/infrastructure/src/tddd/signal_evaluator_v2/external_crates.rs:108-128, libs/infrastructure/src/tddd/signal_evaluator_v2/external_crates.rs:138-158
- Fix: Extract a private helper `fn patch_paths_crate_ids_impl(paths, source_crate, name_to_new_id, filter: Option<(&HashSet<Id>, bool /* is_include */)>)` and delegate both public functions to it.

#### [near-clone] run_git test helper duplicated across 4 test modules (`before-144`)

- Rationale: Four near-identical copies of the same test utility exist across different crates and layers. If the git invocation pattern (e.g., env isolation, error formatting) needs to change, all four must be updated in lockstep. This is 3+ copies spanning architectural layers (infrastructure, cli, cli-composition).
- Locations: libs/infrastructure/src/git_cli/mod.rs:414-416, apps/cli/src/commands/git.rs:109-112, apps/cli-composition/src/review_v2/mod.rs:741-744, apps/cli/src/commands/track/tddd/catalogue_spec_signals.rs:43-52
- Fix: Extract a shared `test_utils::git` module (e.g., in a `test-support` crate or as a `#[cfg(test)]` module in `infrastructure`) that exports `run_git`, `run_git_output`, and `init_minimal_repo`. All test modules import it instead of redefining it.

#### [near-clone] track_add_task and track_add_task_resolved share an identical --after validation block (`before-021`)

- Rationale: The '--after' argument parsing rule (must start with 'T', remaining chars all ASCII digits, parseable as u64) is an invariant that must be maintained consistently. If the rule changes (e.g. add 'S' prefix support), both files require edits. The duplicated body beyond the validation block also amplifies change cost.
- Locations: apps/cli-composition/src/track/mod.rs:437-469, apps/cli-composition/src/track/ops.rs:45-77
- Fix: Extract a private fn parse_after_task_id(after: Option<String>) -> Result<Option<String>, String> helper in track/mod.rs (or a shared submodule), and call it from both track_add_task and track_add_task_resolved.

#### [near-clone] verify_override_first_model_resolution and verify_reviewer_wrapper_guidance are near-identical iteration bodies (`before-095`)

- Rationale: A future change to how missing-snippet or forbidden-snippet findings are emitted (e.g. listing all missing snippets, not just one) would require updating both bodies in lockstep. The only substantive difference between the two functions is the error-message noun ('canonical override-first guidance' vs 'reviewer wrapper guidance' / 'stale default_model-only guidance' vs 'stale reviewer command guidance') and the input slice.
- Locations: libs/infrastructure/src/verify/orchestra.rs:873-907, libs/infrastructure/src/verify/orchestra.rs:910-944
- Fix: Extract a generic `fn verify_snippet_targets(root: &Path, targets: &[(&str, &str, &[&str], &[&str])], missing_label: &str, stale_label: &str, outcome: &mut VerifyOutcome)` and call it from both sites with appropriate label strings.

#### [semantic-dup] Active-track guard (CN-07) inlined in two separate interactors (`before-104`)

- Rationale: Both sites implement the same CN-07 guard rule with identical branch-validation logic and nearly identical error-message strings. Divergence (e.g. one site added a 'plan/' branch check and the other did not) would silently produce inconsistent policy enforcement — a correctness risk. The external file libs/usecase/src/type_signals/interactor.rs lines 130-135 is a third copy.
- Locations: libs/usecase/src/catalogue_spec_refs.rs:75-82, libs/usecase/src/catalogue_spec_refs.rs:167-175, libs/usecase/src/catalogue_spec_signals.rs:55-63, libs/usecase/src/catalogue_spec_signals.rs:159-168
- Fix: Extract a free function (e.g. `fn assert_active_track_branch(branch: &str, track_id: &TrackId) -> Result<(), ActiveTrackGuardError>`) in a shared module (e.g. `usecase::active_track_guard`). Define a canonical `ActiveTrackGuardError { NonActiveTrack, BranchTrackMismatch }` and map it to each interactor's error type via a `From` impl or a simple `.map_err` closure.

#### [semantic-dup] Briefing-file-to-prompt conversion logic duplicated across three locations (`before-137`)

- Rationale: The string `"Read {} and perform the task described there."` is a knowledge constant — the canonical instruction pattern sent to Codex/Claude. If this phrasing changes (e.g., for a model that needs different wording), it must be changed in three places. The same applies to the error string `"either --briefing-file or --prompt is required"`. Cross-crate duplication (plan vs review vs cli-composition) makes divergence likely over time.
- Locations: apps/cli/src/commands/plan/codex_local.rs:53-64, apps/cli/src/commands/review/codex_local.rs:54-65, apps/cli-composition/src/review_v2/mod.rs:579-591
- Fix: Define a single `fn build_prompt_from_briefing_or_inline(briefing_file: Option<PathBuf>, prompt: Option<String>) -> Result<String, String>` in `cli_composition` and have both the planner and reviewer CLI modules delegate to it. The phrasing constant `"Read {} and perform the task described there."` can also be a `pub const BRIEFING_INSTRUCTION_TEMPLATE: &str`.

#### [semantic-dup] ConfidenceSignal → emoji match expression duplicated across three sites in type_catalogue_render.rs (`before-057`)

- Rationale: The same ConfidenceSignal → emoji encoding rule appears in 3 places. catalogue_spec_signal_emoji already exists but is only called for the Cat-Spec column; the Signal column inlines the identical logic. Renaming the function or changing the fallback requires three edits. The fix is trivial: rename catalogue_spec_signal_emoji to confidence_signal_emoji (or add a separate signal_col helper) and call it at the two inline sites.
- Locations: libs/infrastructure/src/type_catalogue_render.rs:226-233, libs/infrastructure/src/type_catalogue_render.rs:628-639, libs/infrastructure/src/type_catalogue_render.rs:699-710
- Fix: Rename catalogue_spec_signal_emoji to a generic confidence_signal_emoji helper and replace the two inline match expressions at lines 633-638 and 704-709 with .map(|sig| confidence_signal_emoji(sig.signal())).

#### [semantic-dup] InformalGroundRefDto and informal_ground_kind_from_str duplicated between spec/codec.rs and tddd/spec_ground_codec.rs (`before-069`)

- Rationale: The `tddd/spec_ground_codec.rs` module exists precisely to centralise the `InformalGroundRef` wire-format knowledge. Having a second private copy in `spec/codec.rs` violates the intent of that extraction: any new variant or renamed string representation forces a two-site update, and the two implementations can silently diverge (they already differ in return type: `Result` vs `Option`). The diverged return type shows the copies have already started drifting.
- Locations: libs/infrastructure/src/spec/codec.rs:119-132, libs/infrastructure/src/tddd/spec_ground_codec.rs:56-63, libs/infrastructure/src/spec/codec.rs:313-323, libs/infrastructure/src/tddd/spec_ground_codec.rs:135-143
- Fix: In `spec/codec.rs`, remove the private `InformalGroundRefDto` struct and `informal_ground_kind_from_str` function. Import `InformalGroundRefDto` from `crate::tddd::spec_ground_codec` and delegate `informal_ground_ref_from_dto` / `informal_ground_ref_to_dto` to the `informal_grounds_from_dtos` / `informal_grounds_to_dtos` helpers already provided by that module.

#### [semantic-dup] Inline `rev-parse HEAD` pattern for reading current HEAD SHA duplicated 3 times (`before-147`)

- Rationale: Each caller independently encodes the knowledge that HEAD SHA is obtained via `rev-parse HEAD` and the output must be trimmed. The `GitRepository` trait (in `libs/infrastructure/src/git_cli/mod.rs`) already houses `current_branch()` and `index_tree_hash()` as conveniences — a `head_sha()` method following the same pattern would centralize this rule. A divergence in error handling is already present: `commit_hash.rs` returns a hard error on failure, while `pr/poll.rs` silently treats failure as `None`.
- Locations: apps/cli-composition/src/review_v2/commit_hash.rs:52-58, apps/cli-composition/src/pr/poll.rs:565-570, apps/cli-composition/src/pr/poll.rs:622-626
- Fix: Add a `head_sha(&self) -> Result<Option<String>, GitError>` method to the `GitRepository` trait that encapsulates `rev-parse HEAD` + success check + trim. Callers choose their error-handling policy (map to None vs propagate error) but stop re-encoding the git invocation.

#### [semantic-dup] Timestamped unique runtime-path generation duplicated in codex_reviewer and review_fix_runner/spawn (`before-062`)

- Rationale: The uniqueness algorithm (counter + nanos + pid) is a single piece of knowledge. If the algorithm needs to change — e.g., to use UUID, or to switch from nanos to a monotonic clock — both copies must be updated. The `REVIEW_RUNTIME_DIR` constant is also duplicated (see separate finding). An error in one copy but not the other (e.g., forgetting `create_dir_all`) would silently cause failures in only one code path.
- Locations: libs/infrastructure/src/review_v2/codex_reviewer.rs:249-267, libs/infrastructure/src/review_v2/review_fix_runner/spawn.rs:14-33
- Fix: Extract a shared `review_runtime_path(prefix, ext) -> Result<PathBuf, String>` helper into a common module (e.g. `review_v2::runtime`). `fixer_runtime_path` maps the `String` error to `ReviewFixRunnerError::Unexpected`.

#### [semantic-dup] Two module-path-extraction functions with the same algorithm but different join separators (`before-073`)

- Rationale: Both functions encode the same knowledge about ItemSummary.path structure (how to strip the leading crate name and trailing item name to produce module middle segments). A change to that path layout (e.g., path length threshold) would need to be applied in both places. The only difference — the separator — could be a parameter.
- Locations: libs/infrastructure/src/tddd/baseline_graph_renderer_adapter/render/node_id_generator.rs:182-191, libs/infrastructure/src/tddd/baseline_graph_renderer_adapter/render/trait_index.rs:172-179
- Fix: Introduce a single `module_path_middle_segments(path: &[String]) -> impl Iterator<Item = &String>` private helper that returns the middle-segment iterator, then build both `module_path_from_summary` (joins with `_`) and `module_path_str_from_summary` (joins with `::`) on top of it. Alternatively, parameterise the join separator.

#### [semantic-dup] reject_symlinks_below error classification duplicated in contract_map_adapter vs baseline_graph_writer_adapter (`before-130`)

- Rationale: The classification rule — `InvalidInput` from `reject_symlinks_below` means a symlink was found, anything else is a generic IO error — is a piece of knowledge about the symlink-guard API. It is encoded once in a helper in one file and re-encoded inline in another file in the same module directory. If the guard API changes, both sites need updating.
- Locations: libs/infrastructure/src/tddd/baseline_graph_writer_adapter.rs:188-205, libs/infrastructure/src/tddd/contract_map_adapter.rs:181-202
- Fix: Move `map_symlink_guard` to the shared `crate::track::symlink_guard` module as a generic helper (parameterized on the SymlinkRejected / IoError constructors or as a macro), then use it in `FsContractMapWriter::write` to replace the two inline classification blocks.

#### [semantic-dup] status_override string → StatusOverride parsing logic duplicated (`before-094`)

- Rationale: A new override kind (e.g., `"on-hold"`) or a change to the error message template requires editing both files. The render.rs copy is slightly less strict (it uses `unwrap_or("")` for the missing reason field instead of requiring it), which means the two implementations already diverge in their handling of malformed input.
- Locations: libs/infrastructure/src/track/codec.rs:163-171, libs/infrastructure/src/track/render.rs:200-211
- Fix: Make `parse_status_override` in `codec.rs` `pub(crate)` (or move it to a shared internal utility) and call it from `decode_legacy_metadata` in `render.rs` instead of reimplementing the match.

#### [structural-dup] "read existing file or None on NotFound" + conditional atomic-write pattern repeated 4 times within sync_rendered_views (`before-151`)

- Rationale: Four copies of the same "write-if-changed" logic, each ~6–8 lines. The pattern is load-bearing: a bug or change in the idempotency condition (`rendered_matches`) must be applied uniformly to all four sites or plan.md, spec.md, types.md, and registry.md will diverge in freshness semantics. A helper `fn write_if_changed(path: &Path, content: &str, changed: &mut Vec<PathBuf>) -> Result<(), RenderError>` would unify them.
- Locations: libs/infrastructure/src/track/render.rs:865-872, libs/infrastructure/src/track/render.rs:907-916, libs/infrastructure/src/track/render.rs:1127-1137, libs/infrastructure/src/track/render.rs:1202-1210
- Fix: Extract a private helper `fn write_if_changed(path: &Path, rendered: &str, changed: &mut Vec<PathBuf>) -> Result<(), RenderError>` that encapsulates the read-existing / compare / atomic-write / push-to-changed logic, and replace all four inline repetitions with calls to it.

#### [structural-dup] Agent-profiles loading + validate_reviewer_provider block duplicated in pr.rs (`before-003`)

- Rationale: If the profiles key (`"pr-reviewer"`), round type, or error message changes, two sites need updating. The block is self-contained and would be a clean private helper extraction.
- Locations: apps/cli-composition/src/pr.rs:221-233, apps/cli-composition/src/pr.rs:305-318
- Fix: Extract a private free function `fn load_and_validate_pr_reviewer_profile(repo_root: &Path) -> Result<ResolvedExecution, String>` and call it from both methods.

#### [structural-dup] Async-bridge OS-thread spawn logic duplicated between `new` and `block_on_thread` (`before-064`)

- Rationale: A single conceptual change to how OS-thread bridging works (e.g. adding a timeout, changing thread naming, or altering panic handling) requires edits in two places. The duplication spans ~10 lines of non-trivial async-bridging logic and would cause divergence if either copy is updated without the other.
- Locations: libs/infrastructure/src/semantic_dup/index.rs:107-117, libs/infrastructure/src/semantic_dup/index.rs:159-179
- Fix: Extract a free function `fn block_on_with_handle<F, T, E>(handle: tokio::runtime::Handle, future: F, thread_err: E) -> Result<T, E>` that performs the spawn+join+error-map sequence. Call it from both `new` (passing `runtime.handle().clone()`) and from `block_on_thread` (passing `self.runtime.as_ref().map(|rt| rt.handle().clone())`). This makes `block_on_thread` a thin wrapper around the free function and eliminates the inline copy in `new`.

#### [structural-dup] ConfidenceSignal → emoji match-arm duplicated inline vs existing helper (`before-148`)

- Rationale: Three copies of the same knowledge (`ConfidenceSignal` → display emoji). The function `catalogue_spec_signal_emoji` was explicitly created to hold this knowledge, but two call-sites bypass it. A new `ConfidenceSignal` variant would silently fall through to `"?"` only in the two uncovered copies, causing inconsistent column rendering between Cat-Spec and Signal columns.
- Locations: libs/infrastructure/src/type_catalogue_render.rs:226-232, libs/infrastructure/src/type_catalogue_render.rs:633-638, libs/infrastructure/src/type_catalogue_render.rs:704-709
- Fix: Replace the two inline `match sig.signal()` blocks (lines 633–638 and 704–709) with a call to `catalogue_spec_signal_emoji(sig.signal())`, matching the already-correct pattern used for the Cat-Spec column.

#### [structural-dup] CwdGuard / CurrentDirGuard RAII struct duplicated in 7 test modules (`before-146`)

- Rationale: Seven independent definitions of the same RAII guard pattern span all three first-party crates. The divergence in Drop error handling (some panic, some ignore) is a latent inconsistency. A single shared test utility would enforce a uniform policy.
- Locations: apps/cli/src/commands/git.rs:91-107, apps/cli/src/commands/review/tests.rs:72-88, libs/infrastructure/src/git_cli/mod.rs:396-412, apps/cli-composition/src/review_v2/commit_hash.rs:91-107, apps/cli-composition/src/review_v2/run.rs:292-308, apps/cli-composition/src/review_v2/mod.rs:695-700, apps/cli-composition/src/make.rs:706-711
- Fix: Define `CwdGuard` once in a shared test-support module or in `infrastructure`'s `#[cfg(test)]` utilities and re-export it. All seven sites import it.

#### [structural-dup] Inherent impl block encoding block repeated 5 times across type-encoder methods (`before-078`)

- Rationale: Five copies of the same impl-block insertion logic. If the Impl construction changes (e.g. adding impl-level generics to the inherent impl, or changing how make_impl is called), all five sites must be updated. The pattern is clear and a helper would eliminate the copies.
- Locations: libs/infrastructure/src/tddd/catalogue_to_extended_crate_codec.rs:1229-1235, libs/infrastructure/src/tddd/catalogue_to_extended_crate_codec.rs:1295-1301, libs/infrastructure/src/tddd/catalogue_to_extended_crate_codec.rs:1357-1363, libs/infrastructure/src/tddd/catalogue_to_extended_crate_codec.rs:1415-1421, libs/infrastructure/src/tddd/catalogue_to_extended_crate_codec.rs:1456-1462
- Fix: Extract a helper fn insert_inherent_impl(&mut self, type_id: Id, type_name: &str, method_ids: Vec<Id>) -> Id that performs the alloc + resolved_path_type + index.insert and returns the impl_id. All five encoder methods call this helper.

#### [structural-dup] Parallel AST-traversal stacks for CommandList → ListableCommand → PipeableCommand in conch.rs and flatten.rs (`before-067`)

- Rationale: The three-level traversal structure is the same knowledge (how the conch AST is shaped at the list/pipeline/pipeable level). Divergence would occur if a new AST node type is added to one walker but not the other. A generic visitor or a shared traversal helper parameterised on the leaf action would eliminate the duplication.
- Locations: libs/infrastructure/src/shell/conch.rs:146-204, libs/infrastructure/src/shell/flatten.rs:138-197
- Fix: Extract a generic `walk_command_list<F>(list, f)` visitor (where `F` is called on each `SimpleCommand`) and use it from both `collect_from_and_or_list` and `walk_and_or_for_flatten`. The `depth`-tracking argument is only needed for the collecting variant and can remain local to `conch.rs`.

#### [structural-dup] Parallel serde structs encoding the same review finding JSON shape: ReviewFinding (usecase) and FindingEntry (infrastructure) (`before-124`)

- Rationale: Both structs carry the same five fields with the same types and the same serde derives. A field addition (e.g. adding `suggestion: Option<String>`) requires edits in two separate crates plus the manual field-copy in review_store.rs. Note: the domain `ReviewerFinding` is intentionally separate (validated newtype) but the two serde DTOs are redundant serialization representations of the same wire shape.
- Locations: libs/usecase/src/review_workflow/verdict.rs:100-112, libs/infrastructure/src/review_v2/persistence/mod.rs:47-55
- Fix: Consider whether `FindingEntry` can be replaced with `ReviewFinding` directly (since usecase is already a dependency of infrastructure), or factor a shared `FindingDto` in a lower layer that both reference.

#### [structural-dup] Parallel word-tree traversal in flatten.rs: flatten_complex_word/flatten_word vs collect_subst_from_complex_word/collect_subst_from_word (`before-068`)

- Rationale: Both stacks mirror the same word-tree shape. A new word variant (e.g. a hypothetical `Word::Backtick`) or a structural change in `ComplexWord` would require symmetric edits in both stacks. The duplication is purely structural — the leaf actions differ — so a generic fold/visitor over the word tree would eliminate it.
- Locations: libs/infrastructure/src/shell/flatten.rs:19-58, libs/infrastructure/src/shell/flatten.rs:242-266
- Fix: Introduce a generic `visit_word<F>(word, f)` / `visit_complex_word<F>(cw, f)` pair where `F` receives each `SimpleWord`. Both `flatten_word` and `collect_subst_from_word` become callers of this visitor with different `F` implementations.

#### [structural-dup] QualifiedPath and DynTrait formatting arms duplicated across four type-formatter functions in format.rs (`before-084`)

- Rationale: The DynTrait and QualifiedPath formatting rules (HRTB guard, sort order, lifetime suffix, `as Trait>::assoc` projection format) encode the same structural knowledge in four places. A change to how DynTrait lifetime bounds are rendered or how QualifiedPath projections are formatted would require editing all four arms. The primary barrier to factoring is that each formatter uses a different recursive call; a trait-object or closure-based inner-formatter abstraction would allow sharing the structural skeleton.
- Locations: libs/infrastructure/src/tddd/signal_evaluator_v2/format.rs:918-953, libs/infrastructure/src/tddd/signal_evaluator_v2/format.rs:477-538, libs/infrastructure/src/tddd/signal_evaluator_v2/format.rs:1231-1263, libs/infrastructure/src/tddd/signal_evaluator_v2/format.rs:1471-1512
- Fix: Introduce a `FormatInner` trait or a recursive closure parameter that abstracts the inner-type call, then implement the DynTrait and QualifiedPath arms once in terms of that abstraction. Alternatively, add a macro that generates the boilerplate arm body with a pluggable recursive-call expression.

#### [structural-dup] Rep-node-ID computation block copy-pasted 3+ times in the entry-emission loop (`before-071`)

- Rationale: The rep-node-id derivation algorithm (path lookup → module_path extraction → type_rep_node_id call) is the same conceptual operation repeated at least 6 times. If the derivation logic changes (e.g. the fallback for missing paths) all copies must be updated consistently.
- Locations: libs/infrastructure/src/tddd/baseline_graph_renderer_adapter/render/entry_subgraph.rs:262-360, libs/infrastructure/src/tddd/baseline_graph_renderer_adapter/render/entry_emitter.rs:76-81
- Fix: Extract a helper function `fn compute_type_rep_node_id(doc: &BaselineDocument, id: Id, layer: &str) -> Option<String>` (or returning String with fallback) that encapsulates the krate.paths + module_path_from_summary + type_rep_node_id pattern. Call it from all loop bodies and emit_* functions.

#### [structural-dup] Repeated CliApp-call + print-stdout + Ok(ExitCode) pattern across all thin-adapter track execute functions (`before-033`)

- Rationale: A change to how stdout is flushed, how exit codes are converted, or whether stderr is also emitted would require touching 15+ sites. The pattern is already factored for the `semantic_dup` commands via `outcome_to_exit`; the track adapters diverge by returning `Result<ExitCode, CliError>` instead of bare `ExitCode`, but a parallel helper (`outcome_to_cli_result`) could absorb the pattern.
- Locations: apps/cli/src/commands/track/state_ops.rs:17-24, apps/cli/src/commands/track/tddd/signals.rs:26-33, apps/cli/src/commands/track/tddd/baseline.rs:26-32, apps/cli/src/commands/track/tddd/catalogue_spec_signals.rs:23-29
- Fix: Add an `fn outcome_to_cli_result(outcome: CommandOutcome) -> Result<ExitCode, CliError>` (or similar) helper to `track/mod.rs` or `commands/mod.rs` and replace the repeated 3-line pattern in every thin-adapter function with a single call.

#### [structural-dup] Subgraph open/rep-node/class/close boilerplate repeated across four emit_* functions (`before-072`)

- Rationale: The four functions share an ~10-line scaffold open block and a ~4-line close block. A change to how the subgraph header is formatted, how direction is emitted, or how class attach works must be replicated in four places.
- Locations: libs/infrastructure/src/tddd/baseline_graph_renderer_adapter/render/entry_emitter.rs:63-100, libs/infrastructure/src/tddd/baseline_graph_renderer_adapter/render/entry_emitter.rs:212-248, libs/infrastructure/src/tddd/baseline_graph_renderer_adapter/render/entry_emitter.rs:400-435, libs/infrastructure/src/tddd/baseline_graph_renderer_adapter/render/entry_emitter.rs:524-560
- Fix: Extract `fn open_entry_subgraph(...)` and `fn close_entry_subgraph(...)` helpers that take the node-kind key and ID generator as parameters (or an enum), encapsulating the open/close scaffold. The four emit_* functions retain only their kind-specific body logic.

#### [structural-dup] TdddLayerBindingsError → local error mapping triplicated across usecase interactors (`before-103`)

- Rationale: Three copies of the same 10-line match block exist within the same crate. The business rule encoded here — what 'layer not found' means and how to phrase the error — is duplicated, so updating the message or adding a new `TdddLayerBindingsError` variant requires editing all three files. The identical `format!` string for `LayerNotFound` is particularly fragile.
- Locations: libs/usecase/src/baseline_capture/interactor.rs:113-127, libs/usecase/src/catalogue_impl_signals/interactor.rs:158-174, libs/usecase/src/type_signals/interactor.rs:150-163
- Fix: Each of the three error types (`BaselineCaptureError`, `CatalogueImplSignalsError`, `TypeSignalsError`) shares the same variants (`LayerBindingsLoad { reason }`, `NoLayers`). Introduce a shared trait or a generic free function `fn map_layer_bindings_error<E>(e: TdddLayerBindingsError, wrap: impl Fn(String) -> E, no_layers: E) -> E` in the shared usecase internal module, or — if the error enums unify — implement `From<TdddLayerBindingsError>` once on a common enum and delegate.

#### [structural-dup] Three near-identical B-impl reconstruction-and-insertion blocks in builder.rs (`before-085`)

- Rationale: Each block is 20–35 lines encoding the exact same knowledge about how to reconstruct a B-side Impl from `b.index` using `b_id_remap`. Divergence between the three (e.g., one block forgetting to call `remap_child_ids_in_item` on children) would silently produce incorrect id references in S or D. The blocks are already partially cross-referenced via comments ('Mirrors the Reference eviction path').
- Locations: libs/infrastructure/src/tddd/signal_evaluator_v2/phase1/builder.rs:131-164, libs/infrastructure/src/tddd/signal_evaluator_v2/phase1/builder.rs:574-616, libs/infrastructure/src/tddd/signal_evaluator_v2/phase1/builder.rs:682-717
- Fix: Extract a function `fn reinsert_b_impl<F>(b_id, b_impl_map, b, b_id_remap, state, insert_root: F, insert_child: F)` (or a simpler `fn reconstruct_b_impl_tree`) that handles the rewrite→remap→child-iterate pipeline, and call it from all three sites with closures or enum-discriminated insertion targets.

#### [structural-dup] Two-pass snapshot/discover strategy duplicated across parse_type_ref_str and encode_bound_str (`before-077`)

- Rationale: The snapshot-clone + placeholder-discovery + re-register + re-parse scaffolding is identical in structure and intent. A bug fix or optimization to the two-pass mechanism (e.g. how new_crate_names deduplication works) must be applied to both sites. The comment in encode_bound_str even says 'same two-pass strategy as parse_type_ref_str'.
- Locations: libs/infrastructure/src/tddd/catalogue_to_extended_crate_codec.rs:824-882, libs/infrastructure/src/tddd/catalogue_to_extended_crate_codec.rs:939-984
- Fix: Extract a generic two-pass helper, e.g. fn two_pass_parse<T>(&mut self, input: &str, parse_fn: impl Fn(...) -> Result<T, String>) -> Result<T, CatalogueToExtendedCrateCodecError>, and call it from both methods. The post-process step (resolve_external_type_ids) stays in parse_type_ref_str.

#### [structural-dup] `FastEmbedAdapter` construction expression duplicated across all four subcommand handlers (`before-018`)

- Rationale: Four copies of the same adapter-construction expression with the same error string. A change to the error message wording or to the adapter's constructor signature must be applied in four separate files. A single `pub(super) fn load_embedding_port() -> Result<Arc<FastEmbedAdapter>, String>` in `common.rs` would eliminate all four copies.
- Locations: apps/cli-composition/src/semantic_dup/build.rs:461-463, apps/cli-composition/src/semantic_dup/check.rs:151-153, apps/cli-composition/src/semantic_dup/find_similar.rs:105-107, apps/cli-composition/src/semantic_dup/measure_quality.rs:70-72
- Fix: Add `pub(super) fn load_embedding_port() -> Result<Arc<FastEmbedAdapter>, String>` to `common.rs` and replace all four instantiation sites.

#### [structural-dup] build_generic_canon_map and build_combined_canon_map share near-identical double-loop bodies over GenericParamDefKind (`before-083`)

- Rationale: The loop body encoding the canonical-map construction rule is the same knowledge in two places. Any change to how synthetic params are ordered or how const/type params are assigned placeholders requires editing both functions. `build_combined_canon_map` could be trivially expressed as two sequential calls to a shared helper that processes one `&[GenericParamDef]` slice.
- Locations: libs/infrastructure/src/tddd/signal_evaluator_v2/format.rs:226-271, libs/infrastructure/src/tddd/signal_evaluator_v2/generics_eq.rs:349-428
- Fix: Extract a private helper `fn extend_canon_map_from_params(params: &[GenericParamDef], map: &mut HashMap<..>, synthetic_order: &mut Vec<..>, idx: &mut usize)` and call it from both `build_generic_canon_map` (once) and `build_combined_canon_map` (twice: parent then method).

#### [structural-dup] filename_stem derivation block duplicated inside merge_gate_adapter.rs (used by git_cli::show consumers) (`before-059`)

- Rationale: 13-line near-identical block in the same file. The comment at line 192 self-documents the duplication. A private helper method `fn derive_filename_stem(filename: &str) -> String` on `GitShowTrackBlobReader` or as a free function would collapse both into a single-line call. Note: `catalogue_bulk_loader.rs` already defines a crate-internal `pub(crate) fn derive_filename_stem(path: &Path)` at line 384 that encodes the same rule; the adapter could import and reuse it instead of re-implementing it twice.
- Locations: libs/infrastructure/src/verify/merge_gate_adapter.rs:195-207, libs/infrastructure/src/verify/merge_gate_adapter.rs:313-325
- Fix: Import and call the existing `crate::tddd::catalogue_bulk_loader::derive_filename_stem` (or extract a shared utility) from both sites in merge_gate_adapter.rs to eliminate the inline duplication.

#### [structural-dup] items_dir + workspace_root symlink guard blocks duplicated across catalogue_spec_signals and catalogue_spec_refs (`before-097`)

- Rationale: The comment 'Mirrors execute_catalogue_spec_signals (catalogue_spec_signals.rs)' at line 387 is itself evidence that the authors recognised the duplication. A security change to this guard (e.g. adding `is_dir()` checking or a `canonicalize()` step) would need to be applied to both copies.
- Locations: libs/infrastructure/src/verify/catalogue_spec_signals.rs:34-66, libs/infrastructure/src/verify/catalogue_spec_refs.rs:387-420
- Fix: Extract a shared helper in `crate::verify::trusted_root` (already exists in the module list) or `catalogue_spec_signals` that returns `Result<(), String>` and is called from both sites, with each caller converting the `Err` to its own return type.

#### [structural-dup] parse_layers (architecture_rules.rs) and layer_rules (layers.rs) duplicate JSON-to-LayerRule parsing logic (`before-096`)

- Rationale: The deduplication check, self-dep check, and unknown-dep validation blocks are verbatim copies across ~70 lines. If the validation rule changes (e.g. stricter path format validation), both files must be updated. The structs are both private and differ only in one optional field.
- Locations: libs/infrastructure/src/verify/architecture_rules.rs:53-147, libs/infrastructure/src/verify/layers.rs:123-198
- Fix: Define a single shared internal `LayerEntry` type that includes an optional `deny_reason` and expose a shared `parse_layer_entries(rules: &serde_json::Value) -> Result<Vec<LayerEntry>, String>` in the `tddd_layers` or a new `arch_rules_parser` submodule. Both consumers then project onto their own private view.

#### [structural-dup] spec.json read + codec-decode boilerplate repeated in 3 verify_from_spec_json functions (`before-101`)

- Rationale: 3 copies of ~12 lines each, with identical error message templates. The boilerplate is non-trivial (I/O + decode + two distinct error variants). A shared `load_spec_doc(path) -> Result<SpecDocument, VerifyOutcome>` helper would remove all three copies.
- Locations: libs/infrastructure/src/verify/spec_attribution.rs:43-61, libs/infrastructure/src/verify/spec_signals.rs:175-193, libs/infrastructure/src/verify/spec_states.rs:82-99
- Fix: Extract a `pub(super) fn load_spec_doc(path: &Path) -> Result<domain::SpecDocument, VerifyOutcome>` helper (or returning `Result<_, Vec<VerifyFinding>>`) into the shared `frontmatter.rs` or a new `spec_path.rs` module, and replace the three identical prologues with calls to it.

### Severity: low

#### [data-dup] "lock error" magic string literal repeated five times in test stubs (`before-121`)

- Rationale: Five copies of the same magic string inside one test module. A `const LOCK_ERROR: &str = "lock error"` at module top would make the intent explicit and remove the repetition.
- Locations: libs/usecase/src/lib.rs:353, libs/usecase/src/lib.rs:363,373,391,405
- Fix: Add `const LOCK_ERROR: &str = "lock error";` at the top of the test module and replace all five string literals with `LOCK_ERROR.to_owned()`.

#### [data-dup] Duplicate `empty_generics()` test helper in structural_eq.rs and tests.rs (`before-087`)

- Rationale: The function is trivially small and divergence would be innocuous. However it is a minor data-duplication: `Generics { params: vec![], where_predicates: vec![] }` is repeated. A shared test-utils module or moving the helper to `tests.rs` and making `structural_eq.rs`'s tests use it would eliminate the duplication.
- Locations: libs/infrastructure/src/tddd/signal_evaluator_v2/structural_eq.rs:452-454, libs/infrastructure/src/tddd/signal_evaluator_v2/tests.rs:34-36
- Fix: Move `empty_generics()` to the `tests.rs` module (the canonical test file) and make `structural_eq.rs`'s test block import it from there, or extract a `test_helpers` submodule accessible to both test modules.

#### [data-dup] Duplicate nesting-depth guard in split_shell_inner and collect_from_top_level_command (`before-066`)

- Rationale: Two copies of the same guard condition in the same file; a change to the depth-limit rule (e.g. the error variant or the comparison operator) must be applied to both sites.
- Locations: libs/infrastructure/src/shell/conch.rs:105-108, libs/infrastructure/src/shell/conch.rs:134-136
- Fix: Remove the guard from `collect_from_top_level_command`. The entry-point guard in `split_shell_inner` is sufficient because the initial depth is 0 and every recursive call increments depth before calling `collect_from_top_level_command` again through `split_shell_inner` (or through direct `depth + 1` increments whose callers already carry the accumulated depth).

#### [data-dup] MAX_NESTING_DEPTH constant defined independently in domain (text.rs) and infrastructure (conch.rs) (`before-042`)

- Rationale: Two independent magic-number definitions for the same policy limit. A change to one without the other silently creates inconsistent behavior between the hand-written parser path and the conch-parser path.
- Locations: libs/domain/src/guard/text.rs:13, libs/infrastructure/src/shell/conch.rs:25
- Fix: Expose `MAX_NESTING_DEPTH` as a `pub(crate)` or `pub` constant from the domain guard module (e.g., `domain::guard::MAX_NESTING_DEPTH`) and import it in the infrastructure crate, or move it to a shared constants module that both layers can reference.

#### [data-dup] Magic string "track/items" path repeated across layers instead of using the existing constant (`before-129`)

- Rationale: The path `track/items` is a workspace layout invariant. Spelling it in six separate production sites means a future rename would require hunting through all layers. The constant already exists in `render.rs` but is not pub or placed in a shared location.
- Locations: libs/infrastructure/src/track/render.rs:54, libs/usecase/src/catalogue_impl_signals/interactor.rs:151, libs/usecase/src/type_signals/interactor.rs:146, libs/usecase/src/baseline_capture/interactor.rs:107, apps/cli-composition/src/review_v2/commit_hash.rs:66, apps/cli-composition/src/track/tddd.rs:35
- Fix: Move `TRACK_ITEMS_DIR` (and the companion `TRACK_ARCHIVE_DIR`) to a shared infrastructure constants module (e.g., `libs/infrastructure/src/track/mod.rs` or a new `paths.rs`), make it `pub`, and replace all literal spellings with the constant.

#### [data-dup] REVIEW_RUNTIME_DIR constant "tmp/reviewer-runtime" declared in two modules (`before-063`)

- Rationale: Duplicated path constants are a low-severity but clear data-dup: a rename of the runtime directory requires two edits, and if only one is updated the two subsystems would write artifacts to different directories, breaking cross-module diagnostics.
- Locations: libs/infrastructure/src/review_v2/codex_reviewer.rs:20, libs/infrastructure/src/review_v2/review_fix_runner/spawn.rs:12
- Fix: Declare a single `pub(crate) const REVIEW_RUNTIME_DIR` in the `review_v2` crate root (`mod.rs`) and reference it from both modules.

#### [data-dup] TRACK_ITEMS_DIR and TRACK_ARCHIVE_DIR constants defined three times within infrastructure/verify (`before-159`)

- Rationale: All four definitions are within the same `infrastructure` crate, so the duplication is intra-crate. The path strings are workspace layout conventions — renaming `track/items` to `tracks/items` would require edits in four separate files within the same crate. However, the impact is low since the crate boundary keeps the blast radius contained.
- Locations: libs/infrastructure/src/track/render.rs:54-55, libs/infrastructure/src/verify/tech_stack.rs:12-13, libs/infrastructure/src/verify/latest_track.rs:14-15, libs/infrastructure/src/verify/view_freshness.rs:10
- Fix: Define `pub(crate) const TRACK_ITEMS_DIR: &str = "track/items";` and `pub(crate) const TRACK_ARCHIVE_DIR: &str = "track/archive";` once in `libs/infrastructure/src/lib.rs` or a dedicated `paths.rs` constants module, and import them in the verify and track submodules.

#### [exact-clone] Two identical test helper functions `make_baseline_with_module_struct` and `make_baseline_with_struct` in the same test module (`before-075`)

- Rationale: Two functions with different names but identical observable behaviour co-exist in the same test module. Tests using one helper could equally use the other. This creates maintenance confusion — a change to baseline construction must be applied to both.
- Locations: libs/infrastructure/src/tddd/baseline_graph_renderer_adapter/render/mod.rs:628-643, libs/infrastructure/src/tddd/baseline_graph_renderer_adapter/render/mod.rs:1245-1257
- Fix: Delete `make_baseline_with_module_struct` (the earlier copy with redundant imports) and replace all its call sites with the canonical `make_baseline_with_struct`.

#### [exact-clone] `MockDiffGetter` and `FailingDiffGetter` test helpers duplicated in two test modules (`before-111`)

- Rationale: Both copies encode the same trivial contract. Any change to `DiffGetter`'s method signature or error type would require updating both locations. The structs are identical apart from the `empty()` method in `tests.rs`.
- Locations: libs/usecase/src/review_v2/tests.rs:58-84, libs/usecase/src/review_v2/scope_query.rs:311-333
- Fix: Create a `#[cfg(test)] mod test_helpers` submodule (or a `tests/helpers.rs` file in the crate) exporting `MockDiffGetter` and `FailingDiffGetter`. Both test modules import from there.

#### [knowledge-dup] `implemented_in` non-empty invariant enforced in two separate sites (`before-037`)

- Rationale: Two copies of the same domain invariant. `ImplementedDecision::new` is the canonical constructor; `AcceptedDecision::implement` could delegate to it rather than duplicating the guard and direct field construction.
- Locations: libs/domain/src/adr_decision/state.rs:83-86, libs/domain/src/adr_decision/state.rs:149-152
- Fix: Replace the body of `AcceptedDecision::implement` with `ImplementedDecision::new(self.common, implemented_in)`, so the non-empty invariant is owned solely by `ImplementedDecision::new`.

#### [near-clone] Constraint-bound arm of generic-argument iteration duplicated between convert_generic_args and angle_bracketed_to_generic_args (`before-091`)

- Rationale: Any change to how `Iterator<Item: Clone>` style constraint bounds are encoded (e.g. supporting `AssocConst` constraints, changing the modifier, adding HRTB params) requires updating both sites. The two functions exist because `angle_bracketed_to_generic_args` handles recursive `AssocType.generics`, not because the constraint logic differs.
- Locations: libs/infrastructure/src/tddd/type_ref_parser.rs:547-573, libs/infrastructure/src/tddd/type_ref_parser.rs:656-680
- Fix: Extract a `build_constraint_bounds(ctx, bounds_iter) -> Vec<GenericBound>` helper and a `push_constraint(ctx, constraints, ident, generics, bounds_iter)` helper that both callers delegate to.

#### [near-clone] Minimal git repo initialisation sequence copy-pasted across three test helpers in `apps/cli/src/commands/review/tests.rs` (`before-142`)

- Rationale: Three copies of the same git-init sequence within one file. Changing the configured email address or branch name requires three edits. Not cross-layer, hence low severity.
- Locations: apps/cli/src/commands/review/tests.rs:94-112, apps/cli/src/commands/review/tests.rs:679-694, apps/cli/src/commands/review/tests.rs:945-961
- Fix: Extract `fn init_git_repo_bare(root: &Path)` that only runs the five invariant git commands. Each helper calls it and then applies its own tree setup before the final `add`+`commit`.

#### [near-clone] Missing-index guard block duplicated in `check.rs` and `find_similar.rs` (`before-016`)

- Rationale: The same business rule — "fail loudly when the index is missing so the tool doesn't silently return empty results" — is expressed in two places with the same structure and near-identical error message templates. A wording change to the message template (e.g. changing `"missing '{}' marker"`) requires edits in both files.
- Locations: apps/cli-composition/src/semantic_dup/check.rs:141-149, apps/cli-composition/src/semantic_dup/find_similar.rs:96-103
- Fix: Add a helper in `common.rs` such as `pub(super) fn require_recognizable_index(db_path: &Path, cmd_label: &str) -> Result<(), String>` that encodes the check and error message template. Call it from both handlers.

#### [near-clone] Structurally identical `RendererError` enums for baseline-graph and contract-map ports (`before-051`)

- Rationale: Both enums model the same three renderer failure modes with the same field shapes. The only variation is a context label in the Display strings. Adding a new variant or field requires editing two enums and two Display impls in sync.
- Locations: libs/domain/src/tddd/baseline_graph_ports.rs:130-174, libs/domain/src/tddd/contract_map_renderer.rs:62-96
- Fix: Introduce a shared `RendererError` type parameterised on a context label string (compile-time or runtime), and re-export under the existing public names. Alternatively, unify the two enums into a single `RenderError { context: &'static str, kind: RenderErrorKind }` style.

#### [near-clone] Tee-stderr-to-file thread logic duplicated between plan and review Codex spawners (`before-027`)

- Rationale: The identical tee-to-file pattern is duplicated. The plan side extracted it into a named function (`tee_stderr_to_file`) which is cleaner, but the review side inlines the same logic. A change to buffering or error handling would need two edits.
- Locations: apps/cli/src/commands/plan/codex_local.rs:122-146, apps/cli/src/commands/review/codex_local.rs:297-313
- Fix: Reuse the named `tee_stderr_to_file` function or lift it to a shared subprocess helper module.

#### [near-clone] Verdict and FastVerdict are structurally identical enums with identical constructor bodies (`before-046`)

- Rationale: Two copies of non-trivial constructor logic. The distinct types are intentional (type-safety to prevent misuse of fast vs. final verdicts), but the constructor body is not type-specific and could be extracted into a helper on `NonEmptyReviewerFindings` or factored via a shared trait/macro to eliminate the duplication while retaining the distinct types.
- Locations: libs/domain/src/review_v2/types.rs:296-310, libs/domain/src/review_v2/types.rs:325-339
- Fix: Extract the shared constructor logic into a private helper, e.g. `fn make_findings_remain<T, F>(findings: Vec<ReviewerFinding>, wrap: F) -> Result<T, VerdictError> where F: FnOnce(NonEmptyReviewerFindings) -> T`. Both `Verdict::findings_remain` and `FastVerdict::findings_remain` delegate to it. Alternatively, implement a private `VerdictLike` trait with a blanket impl and a blanket `findings_remain` method.

#### [near-clone] `temp_build_path` and `backup_path_for` are near-identical functions (`before-015`)

- Rationale: Both functions encode the same path-derivation knowledge (how to compute a hidden sibling path from `db_path`) and share byte-for-byte identical error messages. A rename of the `--db-path` flag error text must be made in two places. The suffix string is the only variable; a single `fn sibling_path(db_path: &Path, suffix: &str) -> Result<PathBuf, String>` would factor both.
- Locations: apps/cli-composition/src/semantic_dup/build.rs:143-152, apps/cli-composition/src/semantic_dup/build.rs:160-169
- Fix: Extract a private `fn sibling_path(db_path: &Path, suffix: &str) -> Result<PathBuf, String>` in `build.rs`. Replace `temp_build_path` with `sibling_path(db_path, "tmp-build")` and `backup_path_for` with `sibling_path(db_path, "old")`.

#### [near-clone] load_impl_plan_opt and load_task_coverage_opt are structurally identical (`before-127`)

- Rationale: A generic `load_json_opt<T, F>(dir, filename, decode_fn)` helper would replace both. The duplication is 12 nearly byte-identical lines and the error message format string is also identical in shape. Adding a third optional JSON artifact would require writing a third copy.
- Locations: libs/infrastructure/src/track/render.rs:24-36, libs/infrastructure/src/track/render.rs:40-52
- Fix: Introduce a private generic helper `fn load_json_opt<T>(track_dir: &Path, filename: &str, decode: impl Fn(&str) -> Result<T, impl Display>) -> Result<Option<T>, RenderError>` and replace both functions with calls to it.

#### [near-clone] prepare_timestamped_path vs fixer_runtime_path — duplicate runtime-path generation (`before-126`)

- Rationale: Both encode the same knowledge: 'a unique timestamped path under REVIEW_RUNTIME_DIR is generated by combining timestamp + pid + prefix/ext, and the parent dir must be created'. A divergence in precision or the missing counter is a latent bug surface. Both live in the same logical subsystem and share the `REVIEW_RUNTIME_DIR` constant.
- Locations: libs/infrastructure/src/review_v2/review_fix_runner/spawn.rs:14-33, apps/cli/src/commands/review/codex_local.rs:240-252
- Fix: Expose `fixer_runtime_path` (or a parameterized variant) from `spawn.rs` and replace `prepare_timestamped_path` in `codex_local.rs` with a call to the shared helper.

#### [semantic-dup] Demo outcome handler in main.rs duplicates outcome_to_exit helper (`before-034`)

- Rationale: Both sites encode the same knowledge — how to convert a Result<CommandOutcome, String> into an ExitCode with user-visible output. The Demo handler's omission of stderr is an accidental divergence, not an intentional difference. Replacing the inline block with outcome_to_exit(CliApp::new().demo()) removes the duplication and restores stderr handling.
- Locations: apps/cli/src/main.rs:106-117, apps/cli/src/commands/mod.rs:27-43
- Fix: Replace the inline Ok/Err match in the Demo/None arm with a direct call to the already-imported outcome_to_exit helper: `Some(CliCommand::Demo) | None => commands::outcome_to_exit(CliApp::new().demo())`.

#### [semantic-dup] Three repeated `item_is_a_sourced` inline predicate in builder.rs (`before-086`)

- Rationale: The predicate is only 2 lines but encodes a business rule (which actions constitute A-sourced provenance). Divergence is unlikely to cause bugs in isolation, but changing the action set (e.g., adding a new ItemAction variant for A-sourced items) would require finding all three sites. A small inline helper `fn is_a_sourced(actions: &BTreeMap<Id, ItemAction>, id: Id) -> bool` on `Phase1State` would centralize the rule.
- Locations: libs/infrastructure/src/tddd/signal_evaluator_v2/phase1/builder.rs:832-835, libs/infrastructure/src/tddd/signal_evaluator_v2/phase1/builder.rs:926-929, libs/infrastructure/src/tddd/signal_evaluator_v2/phase1/builder.rs:1029-1032
- Fix: Add `fn is_a_sourced(&self, id: Id) -> bool` to `Phase1State` that implements the `matches!(self.s_actions.get(&id), Some(&ItemAction::Add) | Some(&ItemAction::Modify))` check, and replace all three inline occurrences.

#### [structural-dup] All four interactor structs repeat identical field declarations, Debug impl, and new() constructor (`before-116`)

- Rationale: Four copies of the same two-field struct skeleton with an identical Debug impl pattern. The repetition is mechanical boilerplate that a shared base struct or a macro could factor. However, since the business logic in each Service impl is distinct and the structs serve different traits, the duplication is limited to scaffolding rather than knowledge. Severity is low because no single conceptual change is likely to diverge silently, but the 4x repetition crosses the threshold for a structural-dup finding.
- Locations: libs/usecase/src/semantic_dup/interactor.rs:86-109, libs/usecase/src/semantic_dup/interactor.rs:134-157, libs/usecase/src/semantic_dup/interactor.rs:238-261, libs/usecase/src/semantic_dup/interactor.rs:290-317
- Fix: Extract a shared InteractorPorts struct (or a ports! macro) holding the two Arc fields and the Debug impl. Each interactor embeds or wraps it, reducing the field and Debug boilerplate to a single definition. Alternatively, derive Debug via a wrapper struct to eliminate the four manual impls.

#### [structural-dup] Catalogue table row formatting duplicated between main-section and "Other" section loops (`before-150`)

- Rationale: Each site encodes the same knowledge: how many columns to emit and in what order, depending on `has_spec_signals`. Extracting `fn push_catalogue_row(out, name, kind_tag, action, details, signal_col, cat_spec_col: Option<String>)` or similar would unify both.
- Locations: libs/infrastructure/src/type_catalogue_render.rs:656-664, libs/infrastructure/src/type_catalogue_render.rs:718-727
- Fix: Extract a small private helper that accepts the name, kind_tag, action, details, signal_col, and optional cat_spec_col and emits the row line. Call it from both loops.

#### [structural-dup] Duplicated `Parsed(payload) + !exit_success → ProcessFailed` arms in `classify_review_verdict` (`before-114`)

- Rationale: The rule 'if the process did not exit successfully the verdict is ProcessFailed regardless of payload content' is expressed twice. If a new `ReviewPayloadVerdict` variant is added, the author must remember to add both the success arm and the ProcessFailed fallthrough arm, risking the latter being omitted. Collapsing via `if !exit_success { return ReviewVerdict::ProcessFailed; }` inside the Parsed arm would make the invariant explicit once.
- Locations: libs/usecase/src/review_workflow/verdict.rs:152-159, libs/usecase/src/review_workflow/verdict.rs:160-167
- Fix: Restructure the Parsed branch: match on `payload.verdict` inside a single outer arm for `ReviewFinalMessageState::Parsed(_) if !exit_success => ReviewVerdict::ProcessFailed`, followed by a single arm `ReviewFinalMessageState::Parsed(p) => match p.verdict { ZeroFindings => ReviewVerdict::ZeroFindings, FindingsRemain => ReviewVerdict::FindingsRemain }`.

#### [structural-dup] Expand-then-compile-glob pattern repeated three times in ReviewScopeConfig::new (`before-048`)

- Rationale: Three structurally-identical iterator chains that differ only in expander function and error variant. A small private helper `compile_patterns<F, E>(patterns: &[String], expand: F, wrap_err: E) -> Result<Vec<GlobMatcher>, ScopeConfigError>` would collapse all three into single call-sites and make future changes (e.g. enabling `literal_separator`) apply uniformly.
- Locations: libs/domain/src/review_v2/scope_config.rs:66-75, libs/domain/src/review_v2/scope_config.rs:80-88, libs/domain/src/review_v2/scope_config.rs:91-99
- Fix: Extract a private function: `fn compile_patterns(patterns: &[String], expand: impl Fn(&str) -> String, wrap_err: impl Fn(String, globset::Error) -> ScopeConfigError) -> Result<Vec<GlobMatcher>, ScopeConfigError>`. Each of the three call-sites passes its own expand closure and error-constructor closure.

#### [structural-dup] In-memory stub store defined three times across test modules (`before-108`)

- Rationale: All three copies encode the same knowledge: an in-memory HashMap store satisfying the domain storage trait bounds. Adding a new trait method to `TrackReader` or `ImplPlanReader` would require updating all three stubs independently. The abstraction opportunity is clear (a shared test-helper module or a `test_support` crate), but the duplication is confined to test code and carries no runtime bug risk, making severity low.
- Locations: libs/usecase/src/lib.rs:343-409, libs/usecase/src/task_ops.rs:673-720, libs/usecase/src/track_phase.rs:162-179
- Fix: Move the full-featured stub (TrackReader + TrackWriter + ImplPlanReader + ImplPlanWriter) to a `#[cfg(test)]` module at the crate root (e.g. `usecase::tests::support`) or to a dedicated `test_support` crate, then import it from all three test modules.

#### [structural-dup] Markdown table header emission duplicated for main-section and "Other" section loops (`before-149`)

- Rationale: Two copies of a 4-line table header decision differing only in surrounding context. Abstraction is clearly warranted — a `fn emit_section_header(out: &mut String, has_spec_signals: bool)` helper would unify both sites. Divergence risk: if only one site is updated when the column schema evolves, rendered markdown will be inconsistent between known sections and the "Other" catch-all.
- Locations: libs/infrastructure/src/type_catalogue_render.rs:613-618, libs/infrastructure/src/type_catalogue_render.rs:691-696
- Fix: Extract a small private helper `fn push_catalogue_table_header(out: &mut String, has_spec_signals: bool)` and call it from both sites.

#### [structural-dup] Repeated "words → push each word → run_sotp" forwarding boilerplate in make.rs dispatch helpers (`before-028`)

- Rationale: The `build_forwarded_args` helper was introduced to eliminate exactly this pattern (see dispatch_track_next_task, dispatch_track_task_counts), but four older functions were not refactored to use it. Each is a 5-line near-copy with only the sotp subcommand prefix differing.
- Locations: apps/cli/src/commands/make.rs:245-270, apps/cli/src/commands/make.rs:307-323
- Fix: Replace these four functions with calls to the existing `build_forwarded_args` helper, matching the pattern used by dispatch_track_review_results and dispatch_track_check_approved.

#### [structural-dup] Repeated `AdrFilePortError::ListPaths` error-mapping closure in `list_adr_paths` (`before-055`)

- Rationale: Three identical closures map I/O errors to the same `ListPaths` variant with the same format string. Changing the error message or variant requires editing three sites. A one-line local closure `let list_err = |e| AdrFilePortError::ListPaths(format!(…));` would consolidate them.
- Locations: libs/infrastructure/src/adr_decision/adapter.rs:39, libs/infrastructure/src/adr_decision/adapter.rs:43-45, libs/infrastructure/src/adr_decision/adapter.rs:46-48
- Fix: Bind the closure once: `let list_err = |e: std::io::Error| AdrFilePortError::ListPaths(format!("{}: {e}", self.adr_dir.display()));` and pass `list_err` to all three `.map_err` calls.

#### [structural-dup] Repeated `Option<String>` blank-check iterator in `validate_review_payload` (`before-113`)

- Rationale: Three copies of the same 4-line guard inside a single function. A conceptual change to the blank-detection rule (e.g., switching from `trim().is_empty()` to `is_blank()`) must be applied in three places. The pattern is local and the duplication is minor, but it is clear structural repetition warranting a small helper.
- Locations: libs/usecase/src/review_workflow/verdict.rs:217-223, libs/usecase/src/review_workflow/verdict.rs:224-230, libs/usecase/src/review_workflow/verdict.rs:234-240
- Fix: Extract a closure or inline helper `fn has_blank_optional(findings: &[ReviewFinding], get: impl Fn(&ReviewFinding) -> Option<&str>) -> bool { findings.iter().any(|f| get(f).is_some_and(|v| v.trim().is_empty())) }` and call it for severity, file, and category, passing `|f| f.severity.as_deref()` etc.

#### [structural-dup] Repeated column-fetch + type-downcast boilerplate in `extract_similar_fragments` (`before-065`)

- Rationale: The same two-step fetch+cast idiom appears three consecutive times with only the column name and array type varying. Changing error message wording or adding a new column requires touching each block individually. A macro or typed helper closure would factor the pattern.
- Locations: libs/infrastructure/src/semantic_dup/index.rs:423-444, libs/infrastructure/src/semantic_dup/index.rs:423-426
- Fix: Introduce a local macro or closure `get_col_as!(batch, NAME, TYPE, ERROR_VARIANT)` that performs column_by_name + downcast_ref in one expression, used consistently for all three column accesses.

#### [structural-dup] Repeated store + branch_reader + service construction boilerplate across five TaskOperation methods (`before-022`)

- Rationale: Seven identical three-line scaffolding blocks. While each is short, the construction order and the use of Arc::clone are a convention that must stay consistent across all sites. A refactor (e.g. changing TaskOperationInteractor's constructor signature) would require edits in seven places.
- Locations: apps/cli-composition/src/track/mod.rs:211-214, apps/cli-composition/src/track/mod.rs:432-435, apps/cli-composition/src/track/mod.rs:493-496, apps/cli-composition/src/track/mod.rs:534-537, apps/cli-composition/src/track/ops.rs:40-43, apps/cli-composition/src/track/ops.rs:98-101, apps/cli-composition/src/track/ops.rs:135-138
- Fix: Extract a private fn build_task_operation_service(items_dir: &PathBuf, project_root: &Path) -> usecase::task_ops::TaskOperationInteractor helper that encapsulates the three lines, and call it from all seven sites.

#### [structural-dup] ReviewRunCodexInput and ReviewRunClaudeInput are identical structs (`before-014`)

- Rationale: The two structs carry identical knowledge (the set of inputs for a reviewer run). Separate types give the call sites a nominally type-safe API, but any schema change (e.g., adding a new flag) must be applied twice. A single ReviewRunInput struct (or a generic ReviewRunInput<Provider>) would eliminate the duplication while keeping type safety.
- Locations: apps/cli-composition/src/review_v2/inputs.rs:6-16, apps/cli-composition/src/review_v2/inputs.rs:18-29
- Fix: Merge into a single ReviewRunInput struct (or add a provider: ReviewProvider field). The CliApp methods review_run_codex and review_run_claude can keep their separate entry points but accept the unified input type.

#### [structural-dup] Single-segment identifier resolution chain duplicated in convert_type_path and resolve_trait_bound_path (`before-090`)

- Rationale: If the resolution policy changes — e.g. adding a new prelude tier, changing how `std_canonical_path` is invoked, or adding `Self` handling to trait bounds — both branches must be updated. The duplication is acknowledged by the developer comment.
- Locations: libs/infrastructure/src/tddd/type_ref_parser.rs:216-255, libs/infrastructure/src/tddd/type_ref_parser.rs:377-395
- Fix: Extract a `resolve_single_segment_name(ctx, name, args, allow_primitives, allow_self) -> Path` helper that centralises the chain, with boolean flags for the two steps that only apply to type paths.

#### [structural-dup] Sorting logic duplicated across `TypeBaselineEntry::new` and `TypeBaselineEntry::with_trait_impls` (`before-053`)

- Rationale: The sort order is a structural invariant of `TypeBaselineEntry`. Encoding it in two constructors means a future change to the sort key must be applied twice. The canonical fix is for `with_trait_impls` to call `new(kind, members, methods)` and then set `trait_impls`, or to factor the sort into a private `fn sort_members_methods(members: &mut Vec<MemberDeclaration>, methods: &mut Vec<MethodDeclaration>)`.
- Locations: libs/domain/src/tddd/baseline.rs:127-135, libs/domain/src/tddd/baseline.rs:139-148
- Fix: Implement `with_trait_impls` as `let mut entry = Self::new(kind, members, methods); entry.trait_impls = trait_impls; entry` — or extract a private sort helper and call it from both constructors.

#### [structural-dup] Three identical single-string error newtypes in `catalogue_impl_signals_ports.rs` (`before-052`)

- Rationale: All three newtypes provide no more structure than a raw `String`. Repeating the same three-line boilerplate three times in the same file adds noise without adding safety. A macro or a shared trait implementation would halve the line count.
- Locations: libs/domain/src/tddd/catalogue_v2/catalogue_impl_signals_ports.rs:335-343, libs/domain/src/tddd/catalogue_v2/catalogue_impl_signals_ports.rs:388-396, libs/domain/src/tddd/catalogue_v2/catalogue_impl_signals_ports.rs:426-434
- Fix: Define a crate-internal macro `string_error_newtype!(Name)` that emits the struct + Display + Error impls. Alternatively, use `thiserror` with `#[error("{0}")]` on a tuple-struct variant, which eliminates the manual Display impl. If the three types must remain distinct newtypes (for type safety at call sites), the macro approach is preferable.

#### [structural-dup] Unresolved Path id-resolution logic duplicated between resolve_external_type_ids and resolve_external_type_ids_in_path (`before-079`)

- Rationale: Duplicated id-resolution rule: if the unresolved-id lookup policy changes (e.g. accepting single-segment known externals), both sites must be updated. The duplication is modest (~12 lines) but the logic is non-trivial.
- Locations: libs/infrastructure/src/tddd/catalogue_to_extended_crate_codec.rs:632-657, libs/infrastructure/src/tddd/catalogue_to_extended_crate_codec.rs:725-743
- Fix: In the Type::ResolvedPath arm of resolve_external_type_ids, replace the inlined id-lookup with a call to self.resolve_external_type_ids_in_path(p) and re-wrap the result as Type::ResolvedPath. The generic-args recursion is already handled inside resolve_external_type_ids_in_path.

#### [structural-dup] `CodexRoundTypeArg` → `&str` conversion repeated three times without a `From`/`Display` impl (`before-138`)

- Rationale: Three identical match arms encoding the canonical string form of the round type enum. Adding a new variant or renaming a string constant requires three edits. This is a small but avoidable structural dup — standard Rust practice is to implement `Display` or `From<CodexRoundTypeArg> for &'static str` on the type once.
- Locations: apps/cli/src/commands/review/mod.rs:212-215, apps/cli/src/commands/review/local.rs:70-73, apps/cli/src/commands/review/fix_local.rs:69-72
- Fix: Implement `impl CodexRoundTypeArg { pub fn as_str(self) -> &'static str { match self { Self::Fast => "fast", Self::Final => "final" } } }` in mod.rs where the type is defined. All three call sites become a single `args.round_type.as_str().to_owned()` call.

#### [structural-dup] `run_command` / `run_sotp` helpers independently defined in both `make` modules (`before-143`)

- Rationale: The `bin/sotp` invocation pattern and exit-code conversion are duplicated. The return-type difference is real (one returns `ExitCode`, the other `CommandOutcome`), so full unification may require adapter work, but the core subprocess invocation knowledge is the same.
- Locations: apps/cli/src/commands/make.rs:195-204, apps/cli-composition/src/make.rs:677-690
- Fix: Share a low-level `fn run_external(program: &str, args: &[&str]) -> Result<u8, String>` that returns the raw exit code. Each module wraps it in its own return-type adaptor. The `bin/sotp` constant can be defined once and imported.

#### [structural-dup] ensure_parent_dir + create_dir_all boilerplate repeated 3× in FsTrackStore (`before-128`)

- Rationale: Three copies of the same ensure-parent logic in the same file. A single private method `ensure_parent(path) -> Result<(), RepositoryError>` would eliminate two of the three copies (the TrackWriteError site wraps RepositoryError, so callers can just map).
- Locations: libs/infrastructure/src/track/fs_store.rs:69-75, libs/infrastructure/src/track/fs_store.rs:129-135, libs/infrastructure/src/track/fs_store.rs:279-285
- Fix: Add a private `fn ensure_parent_dir(path: &Path) -> Result<(), RepositoryError>` helper to `FsTrackStore` and call it from all three write methods.
