<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# consumer 運用で見つかった共通ハーネスの責務・復旧・検証契約を整える

## Summary

GO-01 is implemented by T001-T004 and T011-T013; GO-02 is implemented by T005-T010.
T001-T013 cover all 35 enforced elements through IN-01-IN-07, OUT-01-OUT-06, CN-01-CN-06, and AC-01-AC-12/AC-14-AC-15; AC-13 is absent from the current spec.

## Tasks (13/13 resolved)

### S1 — Workflow and provider handoff

> `.harness/workflows/track/`, provider adapters, `.codex/instructions.md`, `.claude/settings.json`, and `.claude/agents/`: align shared-workflow dispatch, resume, distribution, and adapter-list surfaces. IN-01; IN-02; CN-01; AC-01; AC-02; AC-03.

- [x] **T001**: `.harness/workflows/track/`, `.harness/capabilities/rollback-diagnoser.md`, Codex track/rollback adapters, and `.codex/instructions.md`: align shared-workflow and Codex dispatch references. IN-01; CN-01; AC-01; AC-02. (`55d050fa2b2f51e068bcd3536503b711873ae3c2`)
- [x] **T002**: `.claude/commands/track/implement.md`: align the Claude dispatcher’s provider-selection and phase/rollback delegation path. IN-01; AC-02. (`55d050fa2b2f51e068bcd3536503b711873ae3c2`)
- [x] **T011**: `.claude/settings.json` `permissions.allow`: align the Claude distribution allowlist with the existing phase, test-obligation, catalog, and ref-verify command surfaces. IN-02; AC-03. (`55d050fa2b2f51e068bcd3536503b711873ae3c2`)
- [x] **T012**: `.claude/settings.json` `hooks.PreCompact` and `.claude/agents/README.md`: align compaction context and the Claude capability adapter list. IN-01; CN-01; AC-01. (`55d050fa2b2f51e068bcd3536503b711873ae3c2`)

### S2 — Type-designer guidance

> `.harness/capabilities/type-designer.md` and provider adapters: correct enrollment and inherent-method declaration guidance. IN-02; CN-02; AC-04.

- [x] **T003**: `.harness/capabilities/type-designer.md` and provider type-designer adapters: correct enrollment and inherent-method declaration guidance. IN-02; CN-02; AC-04. (`55d050fa2b2f51e068bcd3536503b711873ae3c2`)

### S3 — PR-review method and result surfaces

> Framework PR-review prompts: establish the shared prompt reference. IN-03; CN-03; AC-05.
> Automated PR-review workflow and adapters: distinguish completion labels. IN-03; AC-06.

- [x] **T004**: `.harness/prompts/pr-reviewer.md` and `.harness/custom/review-prompts/pr-review.md`: establish the framework prompt reference. IN-03; CN-03; AC-05. (`b27f2726e50b8def151aa38286b24f5e2894bab0`)
- [x] **T013**: `.harness/workflows/track/pr-review.md`, `.claude/commands/track/pr-review.md`, and `.agents/skills/track-pr-review/SKILL.md`: distinguish explicit zero-findings completion from approved deviations in automated PR-review results. IN-03; AC-06. (`b27f2726e50b8def151aa38286b24f5e2894bab0`)

### S4 — Open-PR residual-work recovery

> `.harness/workflows/track/pr-review.md`, `.harness/workflows/track/full-cycle.md`, `.harness/policies/review-protocol.md`, and `.harness/policies/task-completion.md`: add open-PR recovery and downstream full-cycle re-entry branches. IN-04; CN-04; AC-07; AC-08.

- [x] **T005**: `.harness/workflows/track/pr-review.md` Step 3, `.harness/workflows/track/full-cycle.md`, `.harness/policies/review-protocol.md`, and `.harness/policies/task-completion.md`: add open-PR residual-work recovery and downstream full-cycle re-entry branches. IN-04; CN-04; AC-07; AC-08. (`b27f2726e50b8def151aa38286b24f5e2894bab0`)

### S5 — Reviewer diagnostics and query-only adapter

> `libs/usecase/src/review_v2/`, `libs/infrastructure/src/review_v2/`, `libs/infrastructure/src/track/gate_state.rs`, `apps/cli-composition/src/review_v2/`, and reviewer boundary fixtures: finish diagnostic propagation and query-only adapter ownership/call-site corrections. IN-05; IN-07; CN-05; AC-09; AC-10; AC-14; AC-15.

- [x] **T006**: `libs/usecase/src/review_v2/`, `libs/infrastructure/src/review_v2/`, `libs/infrastructure/src/track/gate_state.rs`, and `apps/cli-composition/src/review_v2/`: finish reviewer diagnostic/call-site correction, relocate and export `NullReviewer` in infrastructure, remove its production implementation from composition while retaining construction/injection there, and preserve the existing `Reviewer` port declaration. IN-05; IN-07; CN-05; AC-09; AC-10; AC-14; AC-15. (`55d050fa2b2f51e068bcd3536503b711873ae3c2`)
- [x] **T007**: `libs/usecase/src/review_v2/tests.rs`, `libs/infrastructure/tests/review_v2_diagnostics.rs`, `apps/cli-composition/tests/review_v2_diagnostics.rs`, and query-path tests: extend boundary coverage for Codex process-failure conversion, diagnostic redaction/byte bounds, a genuine setup/spawn failure through an actual runner/adapter, fail-closed behavior of both `NullReviewer` methods, Reviewer non-invocation in state-summary/check-approved reads, and the unchanged `Reviewer` port contract. IN-05; IN-07; CN-05; AC-09; AC-10; AC-14; AC-15. (`55d050fa2b2f51e068bcd3536503b711873ae3c2`)

### S6 — Entry-local fulfillment verification

> Obligation-verifier pair/port, callers, prompt/cache path, regression fixtures, and calibration path: implement the catalogue contract and distinct evidence passes. IN-06; CN-06; AC-11; AC-12.

- [x] **T008**: `libs/domain/src/tddd/test_obligation/{pair.rs,ports.rs}`, `libs/usecase/src/test_obligation/evaluate/{plan.rs,mod.rs}`, `libs/infrastructure/src/test_obligation/{fulfillment_verifier.rs,fulfillment_escalation_driver.rs}`, and `apps/cli-composition/src/test_obligation.rs`: implement the pair/port input-contract change and migrate every existing implementation and caller to compile. IN-06; CN-06; AC-11; AC-12. (`55d050fa2b2f51e068bcd3536503b711873ae3c2`)
- [x] **T009**: `libs/infrastructure/src/test_obligation/fulfillment_verifier.rs` and `libs/usecase/src/test_obligation/evaluate/{mod.rs,plan.rs,cache.rs,calibration.rs,tests.rs}`: apply entry-local responsibility inputs, update prompt/cache identity, add structural regression coverage, and execute configured-provider positive/negative semantic calibration evidence. IN-06; CN-06; AC-11; AC-12. (`55d050fa2b2f51e068bcd3536503b711873ae3c2`)
- [x] **T010**: `libs/usecase/src/test_obligation/evaluate/`, affected evaluation call sites, and `apps/cli-driver/`: implement `ConfiguredProviderCalibrationOutcome`, `NonZeroProductionVerdictCount`, `ProductionVerdictCounts`, and the modified `EvaluateTestObligationsOutcome`; migrate evaluation/reporting and add zero/`usize::MAX` category-boundary plus typed-outcome rendering tests. IN-06; AC-12. (`b27f2726e50b8def151aa38286b24f5e2894bab0`)
