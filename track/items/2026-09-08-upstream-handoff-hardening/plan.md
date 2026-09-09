<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# consumer 運用で見つかった共通ハーネスの責務・復旧・検証契約を整える

## Summary

GO-01 is implemented by T001-T004 and T011-T013; GO-02 is implemented by T005-T010.
T001-T013 cover H01-H15 via IN-01 through IN-06 and AC-01 through AC-12; H16 remains excluded by the Phase 3 briefing.

## Tasks (0/13 resolved)

### S1 — Workflow and provider handoff

> `.harness/workflows/track/`, provider adapters, `.codex/instructions.md`, `.claude/settings.json`, and `.claude/agents/`: align shared-workflow dispatch, resume, distribution, and adapter-list surfaces. IN-01; IN-02; CN-01; AC-01; AC-02; AC-03.

- [ ] **T001**: `.harness/workflows/track/`, `.harness/capabilities/rollback-diagnoser.md`, Codex track/rollback adapters, and `.codex/instructions.md`: align shared-workflow and Codex dispatch references. IN-01; CN-01; AC-01; AC-02.
- [ ] **T002**: `.claude/commands/track/implement.md`: align the Claude dispatcher’s provider-selection and phase/rollback delegation path. IN-01; AC-02.
- [ ] **T011**: `.claude/settings.json` `permissions.allow`: align the Claude distribution allowlist with the existing phase, test-obligation, catalog, and ref-verify command surfaces. IN-02; AC-03.
- [ ] **T012**: `.claude/settings.json` `hooks.PreCompact` and `.claude/agents/README.md`: align compaction context and the Claude capability adapter list. IN-01; CN-01; AC-01.

### S2 — Type-designer guidance

> `.harness/capabilities/type-designer.md` and provider adapters: correct enrollment and inherent-method declaration guidance. IN-02; CN-02; AC-04.

- [ ] **T003**: `.harness/capabilities/type-designer.md` and provider type-designer adapters: correct enrollment and inherent-method declaration guidance. IN-02; CN-02; AC-04.

### S3 — PR-review method and result surfaces

> Framework PR-review prompts: establish the shared prompt reference. IN-03; CN-03; AC-05.
> Automated PR-review workflow and adapters: distinguish completion labels. IN-03; AC-06.

- [ ] **T004**: `.harness/prompts/pr-reviewer.md` and `.harness/custom/review-prompts/pr-review.md`: establish the framework prompt reference. IN-03; CN-03; AC-05.
- [ ] **T013**: `.harness/workflows/track/pr-review.md`, `.claude/commands/track/pr-review.md`, and `.agents/skills/track-pr-review/SKILL.md`: distinguish explicit zero-findings completion from approved deviations in automated PR-review results. IN-03; AC-06.

### S4 — Open-PR residual-work recovery

> `.harness/workflows/track/pr-review.md`, `.harness/workflows/track/full-cycle.md`, `.harness/policies/review-protocol.md`, and `.harness/policies/task-completion.md`: add open-PR recovery and downstream full-cycle re-entry branches. IN-04; CN-04; AC-07; AC-08.

- [ ] **T005**: `.harness/workflows/track/pr-review.md` Step 3, `.harness/workflows/track/full-cycle.md`, `.harness/policies/review-protocol.md`, and `.harness/policies/task-completion.md`: add open-PR residual-work recovery and downstream full-cycle re-entry branches. IN-04; CN-04; AC-07; AC-08.

### S5 — Reviewer failure diagnostics

> `libs/usecase/src/review_v2/error.rs`, existing reviewer/process adapters, CLI error rendering, and reviewer boundary fixtures: implement and propagate catalogue-backed reviewer diagnostics. IN-05; CN-05; AC-09; AC-10.

- [ ] **T006**: `libs/usecase/src/review_v2/error.rs` plus existing `ReviewerError` constructors in `libs/infrastructure/src/review_v2/`, `libs/infrastructure/src/track/gate_state.rs`, `apps/cli-composition/src/review_v2/`, and `libs/usecase/src/review_v2/tests.rs`: implement the catalogue-backed diagnostic types, migrate every existing caller to the new variants, and update the CLI rendering so this API change is independently compiling and tested. IN-05; CN-05; AC-09; AC-10.
- [ ] **T007**: `libs/infrastructure/tests/review_v2_diagnostics.rs` and `apps/cli-composition/tests/review_v2_diagnostics.rs`: add disjoint boundary tests for typed reviewer-process propagation after T006's caller migration. IN-05; CN-05; AC-09; AC-10.

### S6 — Entry-local fulfillment verification

> Obligation-verifier pair/port, callers, prompt/cache path, regression fixtures, and calibration path: implement the catalogue contract and distinct evidence passes. IN-06; CN-06; AC-11; AC-12.

- [ ] **T008**: `libs/domain/src/tddd/test_obligation/{pair.rs,ports.rs}`, `libs/usecase/src/test_obligation/evaluate/{plan.rs,mod.rs}`, `libs/infrastructure/src/test_obligation/{fulfillment_verifier.rs,fulfillment_escalation_driver.rs}`, and `apps/cli-composition/src/test_obligation.rs`: implement the pair/port input-contract change and migrate every existing implementation and caller to compile. IN-06; CN-06; AC-11; AC-12.
- [ ] **T009**: `libs/infrastructure/src/test_obligation/fulfillment_verifier.rs` and `libs/usecase/src/test_obligation/evaluate/{mod.rs,plan.rs,cache.rs,tests.rs}`: apply entry-local responsibility inputs, update prompt/cache identity, and add structural regression coverage. IN-06; CN-06; AC-11; AC-12.
- [ ] **T010**: `libs/usecase/src/test_obligation/evaluate/calibration.rs` and configured-provider evaluation path: add positive/negative calibration cases and separately report actual-provider evidence. IN-06; CN-06; AC-12.
