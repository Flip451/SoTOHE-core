<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# 外部 agent 呼び出しのコスト削減

## Summary

GO-01 → T001, T002, T004, T007.
GO-02 → T003, T004, T007, T008, T009.
GO-03 → T005, T006.
GO-04 → T001, T002, T003, T004, T005, T006, T007, T008, T009.

## Tasks (2/9 resolved)

### S1 — Explicit dispatch profile resolution

> Run T001 before T002.
> T003, T004, and T007 depend on the resolved profile contract from this section.

- [x] **T001**: Modify `libs/usecase/src/capability_exec.rs` (`ReasoningEffort`, `CapabilityProfile`, `CapabilityProfilePort::resolve`, and `CapabilityExecError`) and its unit-test module; add the corresponding bindings in `track/items/agent-dispatch-cost-reduction-2026-07-13/test-bindings.json`. IN-01, IN-02, CN-01, CN-02, AC-01, AC-02.
- [x] **T002**: Modify `libs/infrastructure/src/agent_profiles.rs` (`AgentProfiles`, `AgentProfilesError` error mapping, `CapabilityConfigDto`, `ExecutionModeDto`, `ResolvedExecution`, and routing DTOs), `libs/infrastructure/src/capability_exec/agent_profiles.rs` (`AgentProfilesCapabilityAdapter::resolve`), and `libs/infrastructure/src/ref_verify/process_runner.rs` (the Claude, Codex, and Gemini ref-verifier argument builders); update `.harness/config/agent-profiles.json` and `.harness/config/samples/agent-profiles.default.json`, their fixtures, and `track/items/agent-dispatch-cost-reduction-2026-07-13/test-bindings.json`. IN-01, IN-02, CN-01, AC-01, AC-02.

### S2 — Safe reviewer session reuse

> Run T003 after T001 and T002.
> Run T004 after T003.

- [ ] **T003**: Extend `libs/usecase/src/capability_exec.rs` with `TargetArtifactPath` and `TargetArtifactSet`; create `libs/usecase/src/provider_session.rs` (`ExecutionContractFingerprint`, `CapabilityExecutionContractFingerprintPort`, `ReviewerExecutionContractFingerprintPort`, `ProviderSessionCacheEntry`, `ProviderSessionCacheError`, `ProviderSessionCacheKey`, `ProviderSessionCachePort`, `ProviderSessionId`, and `ReviewerPrompt`) and `libs/infrastructure/src/provider_session.rs` (`FsProviderSessionCacheAdapter` and `Sha256ExecutionContractFingerprintAdapter`); export both modules from their `lib.rs` files, consume existing `TrackId`, `ScopeName`, and `RoundType` identity values, and add focused tests plus `track/items/agent-dispatch-cost-reduction-2026-07-13/test-bindings.json` entries. IN-03, IN-04, IN-08, IN-09, CN-04, CN-06, CN-08, AC-03, AC-04, AC-09, AC-10.
- [ ] **T004**: Modify `libs/usecase/src/review_v2/ports.rs` (`Reviewer`), `libs/infrastructure/src/review_v2/{claude_reviewer,codex_reviewer}.rs` (`ClaudeReviewer` and `CodexReviewer`), `apps/cli/src/commands/review/{mod,local,claude_local,codex_local}.rs` (`ReviewCommand` and `cli::commands::review::execute`), and `apps/cli-composition/src/review_v2/{mod,run,shared,shim}.rs` (`ReviewCompositionRoot`); add boundary tests and `track/items/agent-dispatch-cost-reduction-2026-07-13/test-bindings.json` entries. IN-01, IN-03, IN-04, CN-02, CN-03, CN-06, CN-09, AC-01, AC-03, AC-04.

### S3 — Conservative type-signal freshness

> Run T005 before T006.
> Keep the domain artifact model and the infrastructure preflight in separate review-sized tasks.

- [ ] **T005**: Modify `libs/domain/src/tddd/type_signals_doc.rs` (`BaselineHash`, `CatalogueDeclarationHash`, `EvaluatorContractHash`, `ImplementationInputHash`, `LayerId`, `LiveRustdocSnapshotHash`, `LiveRustdocSnapshotStatus`, `RustdocExtractionContractHash`, `Sha256Digest`, `Sha256DigestError`, `TrackBranch`, `TypeSignalsDocument`, `TypeSignalsCurrentInputs`, `TypeSignalsFreshness`, `TypeSignalsLoadResult`, `TypeSignalsReuseDecision`, `TypeSignalsSchemaVersion`, `TypeSignalsSchemaVersionError`, and `decide_type_signals_reuse`); add its domain tests and `track/items/agent-dispatch-cost-reduction-2026-07-13/test-bindings.json` entries. IN-05, IN-06, IN-07, CN-05, CN-07, AC-05, AC-06, AC-07, AC-08.
- [ ] **T006**: Modify `libs/domain/src/tddd/catalogue_v2/catalogue_impl_signals_ports.rs` to remove the legacy `TypeSignalsExecutorPort` and `TypeSignalsExecutionError`; add their usecase replacements in `libs/usecase/src/type_signals/ports.rs`, and modify `libs/usecase/src/type_signals/{interactor,service}.rs` (`TypeSignalsInteractor`, `TypeSignalsRequest`, and `TypeSignalsService`); modify `libs/infrastructure/src/tddd/{type_signals_codec,type_signals_evaluator,type_signals_executor_adapter}.rs` (codec functions, `execute_type_signals_for_layer`, `EvaluateSignalsError`, `TypeSignalsCodecError`, and `TypeSignalsExecutorAdapter`), including its port rebinding; modify `apps/cli-composition/src/signal/mod.rs` (`SignalCompositionRoot`); add tests and `track/items/agent-dispatch-cost-reduction-2026-07-13/test-bindings.json` entries. IN-05, IN-06, IN-07, CN-05, CN-07, AC-05, AC-06, AC-07, AC-08.

### S4 — Opt-in capability session reuse

> Run T007 after T001, T002, and T003.
> Run T008 after T007.

- [ ] **T007**: Modify `libs/usecase/src/capability_exec.rs` (`CapabilityDispatchOutcome`, `CapabilityDispatchRequest`, `CapabilityExecRequest`, `CapabilityProviderPort`, `CapabilityFailureDetail`, `CapabilityFilePath`, `CapabilityInputValidationError`, `CapabilityResumeRequest`, and `TimeoutSeconds`) and `libs/infrastructure/src/capability_exec/{claude,codex}.rs` (`ClaudeCapabilityAdapter` and `CodexCapabilityAdapter`); add usecase and process-boundary tests plus `track/items/agent-dispatch-cost-reduction-2026-07-13/test-bindings.json` entries. IN-01, IN-02, IN-08, IN-09, CN-01, CN-02, CN-04, CN-08, CN-10, AC-01, AC-02, AC-09, AC-10.
- [ ] **T008**: Modify `apps/cli-driver/src/capability.rs` (`CapabilityDriver`, `CapabilityExecDriverInput`, `CapabilityResumeArg`, and `TargetArtifactPathArg`), `apps/cli/src/commands/capability.rs` (`CapabilityCommand`, `CapabilityExecArgs`, and `cli::commands::capability::execute`), and `apps/cli-composition/src/capability.rs` (`CapabilityCompositionRoot`); add CLI fixtures and `track/items/agent-dispatch-cost-reduction-2026-07-13/test-bindings.json` entries. IN-08, IN-09, CN-04, CN-10, AC-09, AC-10.

### S5 — Operational contract cutover

> Run T009 after the reviewer and capability resume paths in T004 and T008 are available.

- [ ] **T009**: Modify `.harness/prompts/capability-exec-discipline.md`, `.harness/capabilities/review-fix-lead.md`, and `.harness/capabilities/{adr-editor,spec-designer,type-designer,impl-planner,implementer}.md` to add the operational-contract clauses and their conformance checks. IN-03, IN-08, CN-03, CN-10, AC-03, AC-10.
