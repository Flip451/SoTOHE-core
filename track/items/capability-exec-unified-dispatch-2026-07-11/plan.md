<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# capability exec: profile 駆動の汎用 capability dispatch コマンド

## Summary

T001: establish the usecase validated values, errors, profile model, and ports.
T002: implement generic fail-closed orchestration and remove the usecase planner stack.
T003: prepare profile execution-mode metadata, the canonical discipline, and provider-native adapter definitions.
T004: implement profile and filesystem source infrastructure adapters.
T005: implement provider-native infrastructure adapters and remove the Codex planner adapter.
T006: add the CLI-driver dispatch boundary and remove its plan driver.
T007: add the CLI command surface and remove plan codex-local parsing.
T008: wire composition, remove the legacy root, rebuild bin/sotp, and exercise end-to-end behavior.
T009: retarget live operational references after the new route is available.

## Tasks (8/9 resolved)

### S1 — Usecase: typed generic dispatch contract and orchestration

> Run T001 before T002.

- [x] **T001**: Usecase capability_exec values, errors, and profile entries — implement the T001 task-contract catalogue entries and reuse CapabilityName. Add independent unit tests and matching test-obligation bindings (IN-01, IN-02, IN-03, IN-05, IN-06, CN-01, CN-02, CN-03, OUT-01, OUT-03, AC-03). (`ff7e095b3a78d0f528bfde2b652a02e62008643f`)
- [x] **T002**: Usecase capability_exec orchestration — implement the T002 task-contract catalogue entries and remove the obsolete planner usecase stack. Add mock-port tests and matching test-obligation bindings (GO-01, GO-02, IN-01 through IN-09, CN-01 through CN-04, OUT-01 through OUT-03, AC-01 through AC-08).

### S2 — Runtime policy and infrastructure adapters

> Run T003 before T004 and T005.
> Run T004 and T005 after T001.

- [x] **T003**: Runtime dispatch configuration and agent-definition surfaces — update agent-profiles, the capability-exec discipline template, Codex skills, and Claude agents, including the implementer and researcher definition surfaces. Add fixture or conformance tests and matching test-obligation bindings (IN-02, IN-03, IN-04, IN-05, IN-07, CN-01, CN-02, CN-03, OUT-04, AC-04, AC-09).
- [x] **T004**: Infrastructure profile and source adapters — implement the T004 task-contract catalogue entries. Add adapter tests and matching test-obligation bindings (IN-02, IN-03, IN-05, CN-01, CN-02, CN-03, AC-01, AC-02, AC-03).
- [x] **T005**: Infrastructure provider adapters — implement the T005 task-contract catalogue entries and delete CodexPlannerAdapter. Add process-boundary tests and matching test-obligation bindings (IN-03, IN-04, IN-05, IN-06, IN-07, IN-08, CN-01, CN-02, CN-03, CN-04, OUT-02, OUT-04, AC-02, AC-04 through AC-07, AC-09).

### S3 — CLI layers and composition

> Run T006, then T007, then T008.

- [x] **T006**: CLI-driver capability boundary — implement the T006 task-contract catalogue entries and remove PlanDriver and PlanInput. Add driver tests and matching test-obligation bindings (IN-01, IN-06, IN-08, IN-09, CN-01, OUT-03, OUT-04, AC-01 through AC-03, AC-05, AC-07).
- [x] **T007**: CLI primary adapter — implement the T007 task-contract catalogue entries and remove PlanCommand and PlanCodexLocalArgs. Add CLI-level tests and matching test-obligation bindings (IN-01, IN-06, IN-08, CN-01, OUT-03, OUT-04, AC-01, AC-03, AC-05, AC-07).
- [x] **T008**: CLI composition and executable integration — implement the T008 task-contract catalogue entries and remove PlanCompositionRoot. Rebuild with cargo make build-sotp before binary-facing checks; add end-to-end fixtures and matching test-obligation bindings (GO-01, GO-02, IN-01 through IN-09, CN-01 through CN-04, OUT-01 through OUT-04, AC-01 through AC-09).

### S4 — Operational reference cutover

> Run T009 after T008.

- [ ] **T009**: Live policy and workflow reference migration — retarget D8 reference surfaces in Claude rules, track commands, settings hook allowlist text, Codex rules, and Codex-system guidance from plan codex-local to the briefing-file route. Add a dead-reference regression check and matching test-obligation bindings (IN-08, CN-04, AC-07).
