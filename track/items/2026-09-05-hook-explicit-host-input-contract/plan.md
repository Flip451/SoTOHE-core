<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# hook 接続元の host 明示と入力契約の選択

## Tasks (0/6 resolved)

### implementation — Typed host boundary and private cache correction

> Execute T001 independently and T002 before T003. GO-01, GO-03.

- [ ] **T001**: libs/infrastructure/src/tddd/type_signals_evaluator/freshness.rs and libs/infrastructure/src/tddd/type_signals_evaluator_properties.rs: adopt and verify the already-present private fingerprint correction and focused tests. IN-06, IN-07, OS-05, CN-06, CN-07, AC-09, AC-10, AC-11.
- [ ] **T002**: apps/cli-driver/src/hook.rs and its focused tests: add HookHost, modify HookInput, and retain HookName conformance at the hook input boundary. IN-01, IN-03, IN-04, OS-01, OS-02, CN-01, CN-02, CN-03, AC-01, AC-03, AC-04, AC-06.
- [ ] **T003**: apps/cli/src/commands/hook.rs and its focused tests: add CliHookHost, modify HookCommand, and retain CliHookName conformance in CLI parsing and dispatch mapping. IN-01, OS-01, OS-02, CN-01, AC-01.

### migration — Bootstrap-safe connection migration

> Execute T004, T005, and T006 in dependency order. GO-02.

- [ ] **T004**: cargo make build-sotp and the rebuilt bin/sotp hook dispatch: execute the bootstrap build and direct dispatch checks before connection edits. IN-05, CN-05, AC-07.
- [ ] **T005**: Claude settings and Codex/Grok hook wrappers: migrate agent-hook invocations after T004 and update focused connection tests. IN-02, IN-05, OS-03, CN-01, CN-03, CN-04, CN-05, AC-02, AC-05, AC-06, AC-07.
- [ ] **T006**: Existing hook integration and automation tests: extend paired-state coverage after T005 and run the relevant build and test gates. IN-01, IN-02, IN-03, IN-04, IN-05, OS-01, OS-02, OS-03, CN-02, CN-03, CN-04, AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-07, AC-08.
