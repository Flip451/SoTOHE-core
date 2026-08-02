<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# reasoning effort に max 段を追加し、限定レーンを Luna Max へ移行する

## Summary

GO-01 → T001, T005, T006.
GO-02 → T004, T003, T002.

## Tasks (5/6 resolved)

### provider-compatibility — Provider compatibility

> Update profile validation and execution-route coverage. [D1; AC-01; AC-02]

- [x] **T001**: Update `libs/infrastructure/src/agent_profiles/profiles.rs::AgentProfiles::resolve_execution` and `tests.rs::test_resolve_execution_unsupported_provider_effort_returns_error` for profile compatibility validation. [D1; IN-01; IN-02; CN-01; AC-01; AC-02] (`e3dca232`)
- [x] **T005**: Extend `libs/infrastructure/src/capability_exec/codex.rs::{build_codex_args,test_codex_capability_adapter_dispatches_native_skill_with_profile_model_and_prompt}` for capability dispatch. [D1; IN-02; AC-02] (`6d1a506f`)
- [x] **T006**: Extend `apps/cli-composition/src/review_v2/mod.rs::{CliApp::review_run_local,review_run_local_resolves_profile_happy_path_writes_verdict_and_telemetry}` for review dispatch and verdict recording. [D1; IN-02; AC-02] (`17f385a5`)

### completion-observation-preparation — Completion observation preparation

> Install the completion-recording workflow step. [D2; AC-05]

- [x] **T004**: Amend the completion steps in `.harness/workflows/track/implement.md` and `.harness/workflows/track/full-cycle.md`. [D2; IN-06; AC-05] (`c7939d73`)

### pre-rollout-recovery — Pre-rollout recovery

> Install the recovery workflow and reload boundary. [D2; AC-04]

- [x] **T003**: Amend `.harness/workflows/track/implement.md` and `.harness/workflows/track/full-cycle.md` with the pre-rollout recovery procedure and a stop/reload boundary. [D2; IN-05; CN-03; AC-04]

### limited-profile-rollout — Limited profile rollout

> Update the selected capability profiles. [D2; AC-03]

- [ ] **T002**: Update `.harness/config/agent-profiles.json` entries `implementer`, `review-fix-lead`, and `dry-fix-lead`; extend committed-profile resolution coverage. [D2; IN-03; IN-04; CN-02; AC-03]
