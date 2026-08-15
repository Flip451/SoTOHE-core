<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# grok provider binding

## Summary

GO-01 -> T001, T009, T008, T002, T003, T004, T005, T010, T006, T011, T012.
Grok host hook coverage is delivered by T007 and T013.

## Tasks (6/13 resolved)

### provider-foundation — Provider foundation and admission

> Coordinate foundation and admission tasks. [IN-01; IN-02; IN-05; CN-01; CN-02; AC-01; AC-02; AC-03; AC-04; AC-06]

- [x] **T001**: Add `GrokOutputEnvelope` and `GrokEnvelopeError` with focused envelope-extraction tests in `libs/infrastructure/src/grok_common.rs`. [IN-01; OS-02; AC-01] (`28d1b47425930cc287dbb64e83a08ad2d4c1c49f`)
- [x] **T009**: Add `GrokSandbox`, `GrokSandboxProfileName`, and `GrokSandboxProfileNameError` admission validation with invalid-value tests in `libs/infrastructure/src/grok_common.rs`. [IN-05; OS-05; CN-01; AC-04] (`d3995a59f24e6932cf95e27563a5acb3fef12565`)
- [x] **T008**: Add `.harness/config/samples/agent-profiles.grok-heavy.json` and a `grok-sandbox` declaration example in `.agents/skills/impl-planner/SKILL.md`, then validate the sample profile and unchanged default selection. [IN-05; IN-06; OS-01; OS-05; OS-06; CN-01; AC-06] (`28d1b47425930cc287dbb64e83a08ad2d4c1c49f`)
- [x] **T002**: Add `GrokCapabilityDefinition` discovery and admission validation with shared-adapter fixture tests in `libs/infrastructure/src/capability_exec/grok.rs`. [IN-02; IN-04; IN-05; OS-04; OS-05; CN-01; CN-02; AC-02; AC-03; AC-04]

### provider-execution-paths — Provider execution paths

> Coordinate provider execution tasks. [IN-01; IN-02; IN-03; IN-04; OS-01; OS-02; OS-03; OS-04; CN-01; CN-02; AC-01; AC-02; AC-03; AC-04; AC-05; AC-08]

- [ ] **T003**: Implement `GrokCapabilityAdapter` with subprocess, structured-envelope, and resume fallback tests in `libs/infrastructure/src/capability_exec/grok.rs`. [IN-01; IN-02; IN-03; IN-04; IN-05; OS-01; OS-02; OS-03; OS-04; OS-05; CN-01; CN-02; AC-01; AC-02; AC-03; AC-04; AC-05; AC-08]
- [ ] **T004**: Implement `GrokReviewer` with typed-pipeline launch and resume-setting tests in `libs/infrastructure/src/review_v2/grok_reviewer.rs`. [IN-01; IN-02; IN-04; OS-04; CN-02; AC-01; AC-02; AC-03; AC-05]
- [ ] **T005**: Implement `GrokDryChecker` with typed-pipeline launch and resume-setting tests in `libs/infrastructure/src/dry_check/grok_dry_checker.rs`. [IN-01; IN-02; IN-04; OS-04; CN-02; AC-01; AC-02; AC-03; AC-05]
- [ ] **T010**: Update `CodexDryFixLocalRunner` in `libs/infrastructure/src/dry_check/dry_fix_local/mod.rs` and `ReviewFixRunnerAdapter` in `libs/infrastructure/src/review_v2/review_fix_adapter.rs` with Grok launch-path tests. [IN-01; IN-02; IN-04; OS-04; CN-02; AC-01; AC-02; AC-03; AC-05]

### provider-composition — Provider composition wiring

> Coordinate driver and composition wiring tasks. [IN-02; IN-03; IN-04; OS-01; OS-03; OS-04; CN-02; AC-02; AC-03; AC-05; AC-08]

- [ ] **T006**: Wire `CapabilityDriver` in `apps/cli-driver/src/capability.rs` and `CapabilityCompositionRoot` in `apps/cli-composition/src/capability.rs`, with capability-dispatch integration tests. [IN-02; IN-03; IN-04; OS-01; OS-03; OS-04; CN-02; AC-02; AC-03; AC-05; AC-08]
- [ ] **T011**: Wire `ReviewDriver` in `apps/cli-driver/src/review.rs` and `ReviewCompositionRoot::review_driver` in `apps/cli-composition/src/review_v2/shim.rs`, with reviewer-dispatch integration tests. [IN-02; IN-04; OS-04; CN-02; AC-02; AC-03; AC-05]
- [ ] **T012**: Wire `ReviewFixDriver` in `apps/cli-driver/src/review.rs` and `ReviewCompositionRoot::review_fix_driver` in `apps/cli-composition/src/review_v2/shim.rs`, with review-fix dispatch integration tests. [IN-02; IN-04; OS-04; CN-02; AC-02; AC-03; AC-05]

### grok-host-guards — Grok host guards

> Coordinate Grok host guard tasks. [IN-07; OS-06; CN-03; AC-07]

- [x] **T007**: Add Grok host-guard declarations under `.grok/hooks/` and validate their configured handler names. [IN-07; OS-06; CN-03; AC-07] (`28d1b47425930cc287dbb64e83a08ad2d4c1c49f`)
- [x] **T013**: Update `HookInput`, `HookDriver`, and `CommandOutcome` in `apps/cli-driver/src/hook.rs` with Grok envelope and tool-name translation tests. [IN-07; OS-06; CN-03; AC-07] (`28d1b47425930cc287dbb64e83a08ad2d4c1c49f`)
