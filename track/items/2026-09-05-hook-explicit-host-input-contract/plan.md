<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# hook 接続元の host 明示と入力契約の選択

## Summary

GO-01 -> T002, T003, T006, T008, T009, T010, T011, T013.
GO-02 -> T004, T005, T006, T009, T011, T012, T014.
GO-03 -> T001.

## Tasks (9/13 resolved)

### implementation — Typed host boundary and private cache correction

> Execute T001 independently and T002 before T003. GO-01, GO-03.

- [x] **T001**: libs/infrastructure/src/tddd/type_signals_evaluator/freshness.rs and libs/infrastructure/src/tddd/type_signals_evaluator_properties.rs: adopt and verify the already-present private fingerprint correction and focused tests. IN-06, IN-07, OS-05, CN-06, CN-07, AC-09, AC-10, AC-11. (`c0c38aa93102882ed8ef954d6e665e11c37d4231`)
- [x] **T002**: apps/cli-driver/src/hook.rs: implement HookHost and its trait implementations, modify HookInput, and update co-located tests for HookHost, HookInput, and HookName. IN-01, IN-03, IN-04, OS-01, OS-02, CN-01, CN-02, CN-03, AC-01, AC-03, AC-04, AC-06. (`c0c38aa93102882ed8ef954d6e665e11c37d4231`)
- [x] **T003**: apps/cli/src/commands/hook.rs: implement CliHookHost and From<CliHookHost> for cli_driver::hook::HookHost, modify HookCommand, and update co-located tests for CliHookHost, HookCommand, and CliHookName. IN-01, OS-01, OS-02, CN-01, AC-01. (`c0c38aa93102882ed8ef954d6e665e11c37d4231`)

### migration — Bootstrap-safe connection migration

> Execute T004, T005, and T006 in dependency order. GO-02.

- [x] **T004**: cargo make build-sotp and the rebuilt bin/sotp hook dispatch: execute the bootstrap build and direct dispatch checks before connection edits. IN-05, CN-05, AC-07. (`c0c38aa93102882ed8ef954d6e665e11c37d4231`)
- [x] **T005**: .claude/settings.json, .codex/hooks/sotp-hook.sh, .grok/hooks/sotp-hook.sh, and apps/cli/tests/hook_connections.rs: update provider hook invocations and connection assertions after T004. IN-02, IN-05, OS-03, CN-01, CN-03, CN-04, CN-05, AC-02, AC-05, AC-06, AC-07. (`c0c38aa93102882ed8ef954d6e665e11c37d4231`)
- [x] **T006**: apps/cli/tests/hook_dispatch.rs and cargo make ci/build-sotp: extend hook-dispatch integration cases after T005 and execute both gates. IN-01, IN-02, IN-03, IN-04, IN-05, OS-01, OS-02, OS-03, CN-02, CN-03, CN-04, AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-07, AC-08. (`c0c38aa93102882ed8ef954d6e665e11c37d4231`)

### review-corrections — Typed execution and integration corrections

> Execute T008 through T014 in declared dependency order. GO-01, GO-02.

- [x] **T008**: apps/cli-driver/src/hook.rs::HookInput and HookDriver::handle: implement selected-host input compatibility, envelope normalization, and focused tests. IN-01, IN-03, IN-04, OS-01, OS-02, CN-02, CN-03, AC-01, AC-03, AC-04, AC-06.
- [ ] **T009**: apps/cli-driver/src/hook.rs::HookExecution and HookDriver::handle: implement execution-result and advisory classification with focused tests. IN-01, CN-03, CN-04, AC-01, AC-05.
- [x] **T010**: apps/cli/src/commands/hook.rs::HookCommand and From<HookCommand> for cli_driver::hook::HookInput: implement CLI ingress parsing and mechanical conversion with focused tests. IN-01, OS-01, OS-02, CN-01, CN-02, AC-01.
- [ ] **T011**: apps/cli/src/commands/hook.rs::CliHookExecution, HookExecutionDisposition, execute_inner, and is_hook_block_outcome plus apps/cli/src/main.rs telemetry: implement typed execution/emission and remove is_advisory_hook_command with focused tests. IN-01, CN-03, CN-04, AC-01, AC-05.
- [ ] **T012**: apps/cli/tests/hook_connections.rs: execute shipped provider-connection argv and advisory branches. IN-02, IN-05, CN-01, CN-04, CN-05, AC-02, AC-05, AC-07.
- [x] **T013**: apps/cli/tests/hook_dispatch.rs: add direct host-option parser and rejection validation with CARGO_BIN_EXE. IN-01, CN-01, AC-01.
- [ ] **T014**: apps/cli/tests/hook_dispatch.rs: add cross-host skill-compliance result-parity validation. CN-04, AC-05, AC-08.
