<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# DRY ゲートを利用者設定で切り替え可能にし、既定を無効（opt-in）とする

## Summary

Add `enabled: bool` to the DRY gate configuration path (infrastructure DTO -> usecase config -> interactor fields) and implement the opt-in short-circuit at both evaluation points (DryCheckApprovalInteractor and FixpointResolveInteractor). Migrate dry-check.json schema from v3 to v4. (T001-T012 complete.) Move reasoning_effort fields from dry-check.json to agent-profiles.json capabilities.dry-checker, remove them from DryCheckConfig accessors and fingerprint, and update the composition root to read reasoning effort from AgentProfiles/CapabilityConfigDto (D4 / IN-08 / IN-09 / IN-10).

## Tasks (12/15 resolved)

### S1 — Infrastructure: schema_version 4 migration

> Migrate the infrastructure DryCheckConfig loader from schema_version 3 to 4 as an atomic same-commit pair with the checked-in dry-check.json update.
> Add `enabled` boolean field (serde default false) to the internal DTO.
> Add `enabled()` public accessor.
> Reject schema_version 3 with UnsupportedSchemaVersion.
> Update `.harness/config/dry-check.json` to schema_version 4 with `enabled: false` in the same commit as the loader migration.

- [x] **T001**: Infrastructure DryCheckConfig: migrate schema_version acceptance from 3 to 4, add `enabled: bool` field with `#[serde(default)]` (defaults to false) to DryCheckConfigDto, add `enabled()` accessor to DryCheckConfig, and reject schema_version 3 with UnsupportedSchemaVersion. Atomicity: T001 and T002 must land in the same commit (same commit_hash) so the loader never rejects the repository's checked-in dry-check.json between tasks. (`dc7ea26d73ad3fc803110eb3dcc5303966e04dd0`)
- [x] **T002**: Update `.harness/config/dry-check.json`: bump schema_version from 3 to 4 and add `"enabled": false`. Keep all other fields (threshold, max_parallelism, reasoning efforts, known-bad percents) unchanged. Atomicity: T002 must land in the same commit as T001 so the checked-in config and the loader's accepted schema move together. (`dc7ea26d73ad3fc803110eb3dcc5303966e04dd0`)

### S2 — Usecase: DryCheckConfig enabled field

> Add `enabled: bool` to the usecase-layer DryCheckConfig struct and its constructor.
> Update the composition helper to propagate the enabled flag from the infrastructure DTO.

- [x] **T003**: Usecase DryCheckConfig: add `enabled: bool` public field and update `DryCheckConfig::new()` to accept an `enabled: bool` parameter. The derive attributes (Debug, Clone, PartialEq, Eq) are inherited by the new field. (`f815dd2c7c261fe423be0fac97bc53d1a729585a`)
- [x] **T004**: Composition: update `build_usecase_dry_check_config` in `apps/cli-composition/src/dry.rs` to propagate `infra_config.enabled()` into `DryCheckConfig::new(...)` as the `enabled` argument. (`f815dd2c7c261fe423be0fac97bc53d1a729585a`)

### S3 — Usecase: DryCheckApprovalInteractor enabled short-circuit

> Thread DryCheckConfig into DryCheckApprovalInteractor.
> Implement the `enabled: false` early-return path in `check_approved` as the service-boundary guard.
> Update dry_check_approved composition to return Approved before dry diff/corpus/fragment preparation when the config is disabled, and pass DryCheckConfig in the enabled path.

- [x] **T005**: Usecase DryCheckApprovalInteractor: add `dry_config: DryCheckConfig` field. Update `DryCheckApprovalInteractor::new()` to accept `dry_config` as its first parameter. In `check_approved`, return `Ok(DryCheckApprovalVerdict::Approved)` immediately when `dry_config.enabled` is false, before executing staleness, fingerprint, and all-resolved checks. This is the service-boundary safety net; T006 adds the CLI composition early return that avoids building DRY inputs when the gate is disabled. (`f815dd2c7c261fe423be0fac97bc53d1a729585a`)
- [x] **T006**: Composition wiring for DryCheckApprovalInteractor: in `apps/cli-composition/src/dry.rs` (dry_check_approved), load `.harness/config/dry-check.json` and build the usecase DryCheckConfig before dry diff/corpus preparation. If `enabled` is false, return Approved immediately without resolving the dry diff base, computing corpus fingerprints, building fragment refs, constructing coverage/store adapters, or constructing `DryCheckApprovalInteractor`. If `enabled` is true, preserve the existing preparation flow and pass the DryCheckConfig as the first argument to `DryCheckApprovalInteractor::new`. Update the `apps/cli-composition/src/track/fixpoint_resolve.rs` approval-interactor call site in the enabled path as part of T008. (`f815dd2c7c261fe423be0fac97bc53d1a729585a`)

### S4 — Usecase: FixpointResolveInteractor enabled dry-gate bypass

> Thread DryCheckConfig into FixpointResolveInteractor.
> Implement the `enabled: false` dry-gate bypass in `resolve`.
> Update fixpoint_resolve composition to load DryCheckConfig before dry preparation, skip dry prep when disabled, and supply DryCheckConfig to the enabled path.

- [x] **T007**: Usecase FixpointResolveInteractor: add `dry_config: DryCheckConfig` field. Update `FixpointResolveInteractor::new()` to accept `dry_config` as its first parameter. In `resolve`, when `dry_config.enabled` is false, skip the dry gate call entirely and treat the dry gate as Approved (do not return RunDfp; proceed to review gate evaluation). (`f815dd2c7c261fe423be0fac97bc53d1a729585a`)
- [x] **T008**: Composition wiring for FixpointResolveInteractor: in `apps/cli-composition/src/track/fixpoint_resolve.rs`, load `.harness/config/dry-check.json` and build the usecase DryCheckConfig before dry diff-base resolution, corpus fingerprinting, fragment-ref construction, and dry approval adapter construction. If `enabled` is false, skip that dry-gate preparation entirely, pass an empty `current_fragment_refs` set and a no-op dry approval service, and rely on `FixpointResolveInteractor::resolve` to bypass the dry gate and continue to review/ref-verify evaluation. If `enabled` is true, preserve the existing dry preparation flow, pass DryCheckConfig to `DryCheckApprovalInteractor::new`, and pass the same DryCheckConfig as the first argument to `FixpointResolveInteractor::new`. (`f815dd2c7c261fe423be0fac97bc53d1a729585a`)

### S5 — Tests and ADR cross-reference

> Add and update unit tests for the infrastructure schema v4 migration.
> Add and update unit tests for the two usecase interactors.
> Verify the ADR cross-reference is in place and the full CI suite passes.

- [x] **T009**: Unit tests for infrastructure DryCheckConfig (schema v4): test enabled defaults to false when key is omitted, enabled=true when explicitly set, schema_version 3 now rejected with UnsupportedSchemaVersion (expected: 4), and schema_version 4 with explicit enabled=false accepted. Update existing test fixtures from schema_version 3 to schema_version 4. (`dc7ea26d73ad3fc803110eb3dcc5303966e04dd0`)
- [x] **T010**: Unit tests for usecase DryCheckApprovalInteractor: add tests covering (a) enabled=false returns Approved immediately without touching the coverage port or reader, and (b) enabled=true with existing coverage-absent and violation scenarios behave unchanged. Update `make_interactor` helpers to supply DryCheckConfig with appropriate enabled value. (`f815dd2c7c261fe423be0fac97bc53d1a729585a`)
- [x] **T011**: Unit tests for usecase FixpointResolveInteractor: add tests covering (a) enabled=false with a dry gate that would return Blocked still resolves to RunRfp/RunRefVerify/Commit based on review and ref-verify gate states (dry gate bypassed), and (b) enabled=true behavior identical to current (dry Blocked returns RunDfp). Update `make_interactor` helpers to accept DryCheckConfig. (`f815dd2c7c261fe423be0fac97bc53d1a729585a`)
- [x] **T012**: Verify the ADR cross-reference: confirm the Follow-up section of `knowledge/adr/2026-06-02-0716-dry-checker.md` contains the supersede notice for `2026-06-19-2335-dry-gate-configurable-default-off.md` (add if absent). Run `cargo make ci` to confirm the full test suite (fmt-check + clippy + nextest + deny + check-layers + verify-*) passes with all changes from T001-T011. (`f815dd2c7c261fe423be0fac97bc53d1a729585a`)

### S6 — D4: reasoning_effort migration from dry-check.json to agent-profiles.json

> Add `fast_reasoning_effort` / `final_reasoning_effort` to `CapabilityConfigDto` (infrastructure) with serde deserialization and accessors.
> Add those fields to `.harness/config/agent-profiles.json` `capabilities.dry-checker`.
> Remove `fast_reasoning_effort` / `final_reasoning_effort` from `DryCheckConfigDto`, `DryCheckConfig`, fingerprint encoding, and validation in the infrastructure dry_check::config module.
> Remove `DryCheckConfigError::InvalidReasoningEffort` variant.
> Update `dry.rs` composition root to read reasoning effort from `AgentProfiles`/`CapabilityConfigDto` instead of `DryCheckConfig`.
> Remove the two fields from `.harness/config/dry-check.json` (deny_unknown_fields now rejects them).
> T013 and T014 are atomic (same commit) to keep the codebase compilable.
> Update all test fixtures that embed the old reasoning_effort fields, remove accessor tests, and add new tests for the deny_unknown_fields rejection, fingerprint stability, and CapabilityConfigDto accessor coverage.

- [~] **T013**: D4 infrastructure setup: (1) add `fast_reasoning_effort: Option<String>` and `final_reasoning_effort: Option<String>` fields to `CapabilityConfigDto` in `libs/infrastructure/src/agent_profiles.rs` (serde struct, `deny_unknown_fields` update) and add the corresponding `fast_reasoning_effort()` / `final_reasoning_effort()` accessors to the `impl CapabilityConfigDto` block; (2) add `"fast_reasoning_effort"` and `"final_reasoning_effort"` to `.harness/config/agent-profiles.json` under `capabilities.dry-checker`, using the values currently in `.harness/config/dry-check.json` (`"medium"` / `"high"`). Atomicity: T013 must land in the same commit as T014 because T014 removes the reasoning_effort fields from `DryCheckConfig` and updates `dry.rs` to call `CapabilityConfigDto::fast_reasoning_effort()` / `final_reasoning_effort()` — the composition root must compile at every commit.
- [~] **T014**: D4 infrastructure cleanup and composition root migration (must be the same commit as T013): (1) remove `fast_reasoning_effort` and `final_reasoning_effort` fields from `DryCheckConfigDto` and `DryCheckConfig` in `libs/infrastructure/src/dry_check/config.rs`; remove `validate_reasoning_effort` helper, `ALLOWED_REASONING_EFFORTS` constant, `InvalidReasoningEffort` variant from `DryCheckConfigError`, and the two corresponding public accessors `fast_reasoning_effort()` / `final_reasoning_effort()`; update `DryCheckConfig::load` to no longer call `validate_reasoning_effort`; (2) update `fingerprint_with_threshold` canonical encoding in `config.rs` to exclude `fast_reasoning_effort` / `final_reasoning_effort` lines — the new encoding covers only `enabled`, `threshold`, `max_parallelism`, `known_bad_injection_rate_percent`, `known_bad_detection_threshold_percent` (CN-08 / IN-10); (3) in `apps/cli-composition/src/dry.rs` `dry_write`, replace `infra_config.fast_reasoning_effort().to_owned()` and `infra_config.final_reasoning_effort().to_owned()` calls with reads from `AgentProfiles` via `CapabilityConfigDto::fast_reasoning_effort()` / `final_reasoning_effort()` — load `agent-profiles.json` before constructing `CodexDryChecker`; (4) update `.harness/config/dry-check.json` to remove the `fast_reasoning_effort` and `final_reasoning_effort` fields (the loader now has `deny_unknown_fields` and would reject the old file). After this task the codebase must compile without errors.
- [~] **T015**: D4 tests update and CI pass: (1) update all test fixtures in `libs/infrastructure/src/dry_check/config.rs` that include `fast_reasoning_effort` / `final_reasoning_effort` fields — remove those fields from every JSON fixture string so the updated `deny_unknown_fields` DTO can parse them; (2) remove or update tests that assert on `fast_reasoning_effort()` / `final_reasoning_effort()` accessors (these methods no longer exist); (3) add tests covering: (a) that a v4 `dry-check.json` containing `fast_reasoning_effort` or `final_reasoning_effort` fields returns a `Parse` error (unknown field rejected by `deny_unknown_fields`), (b) that `fingerprint` does NOT change when reasoning effort values differ (i.e., the fingerprint is stable across the field move), (c) `fast_reasoning_effort()` / `final_reasoning_effort()` accessors on `CapabilityConfigDto` return `Some` when present and `None` when absent; (4) add tests in `agent_profiles.rs` confirming the new fields are deserialized correctly; (5) run `cargo make ci` to confirm the full suite passes.
