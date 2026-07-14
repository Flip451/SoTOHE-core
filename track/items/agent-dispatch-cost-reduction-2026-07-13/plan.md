<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# 外部 agent 呼び出しのコスト削減

## Summary

GO-01 → T001, T002, T004, T007.
GO-02 → T003, T004, T007, T008, T009.
GO-03 → T005, T011, T012, T013, T014, T015, T016, T017, T018.
GO-04 → T001, T002, T003, T004, T005, T007, T008, T009, T011, T012, T013, T014, T015, T016, T017, T018.

## Tasks (11/16 resolved)

### S1 — Explicit dispatch profile resolution

> Run T001 before T002.
> T003, T004, and T007 depend on the resolved profile contract from this section.

- [x] **T001**: Modify `libs/usecase/src/capability_exec.rs` (`ReasoningEffort`, `CapabilityProfile`, `CapabilityProfilePort::resolve`, and `CapabilityExecError`) and its unit-test module; add the corresponding bindings in `track/items/agent-dispatch-cost-reduction-2026-07-13/test-bindings.json`. IN-01, IN-02, CN-01, CN-02, AC-01, AC-02. (`fc39a2ec`)
- [x] **T002**: Modify `libs/infrastructure/src/agent_profiles.rs` (`AgentProfiles`, `AgentProfilesError` error mapping, `CapabilityConfigDto`, `ExecutionModeDto`, `ResolvedExecution`, and routing DTOs), `libs/infrastructure/src/capability_exec/agent_profiles.rs` (`AgentProfilesCapabilityAdapter::resolve`), and `libs/infrastructure/src/ref_verify/process_runner.rs` (the Claude, Codex, and Gemini ref-verifier argument builders); update `.harness/config/agent-profiles.json` and `.harness/config/samples/agent-profiles.default.json`, their fixtures, and `track/items/agent-dispatch-cost-reduction-2026-07-13/test-bindings.json`. IN-01, IN-02, CN-01, AC-01, AC-02. (`fc39a2ec`)

### S2 — Safe reviewer session reuse

> Run T003 after T001 and T002.
> Run T004 after T003.

- [ ] **T003**: Extend `libs/usecase/src/capability_exec.rs` with `TargetArtifactPath` and `TargetArtifactSet`; create `libs/usecase/src/provider_session.rs` (`ExecutionContractFingerprint`, `CapabilityExecutionContractFingerprintPort`, `ReviewerExecutionContractFingerprintPort`, `ProviderSessionCacheEntry`, `ProviderSessionCacheError`, `ProviderSessionCacheKey`, `ProviderSessionCachePort`, `ProviderSessionId`, and `ReviewerPrompt`) and `libs/infrastructure/src/provider_session.rs` (`FsProviderSessionCacheAdapter` and `Sha256ExecutionContractFingerprintAdapter`); export both modules from their `lib.rs` files, consume existing `TrackId`, `ScopeName`, and `RoundType` identity values, and add focused tests plus `track/items/agent-dispatch-cost-reduction-2026-07-13/test-bindings.json` entries. IN-03, IN-04, IN-08, IN-09, CN-04, CN-06, CN-08, AC-03, AC-04, AC-09, AC-10.
- [ ] **T004**: Modify `libs/usecase/src/review_v2/ports.rs` (`Reviewer`), `libs/infrastructure/src/review_v2/{claude_reviewer,codex_reviewer}.rs` (`ClaudeReviewer` and `CodexReviewer`), `apps/cli/src/commands/review/{mod,local,claude_local,codex_local}.rs` (`ReviewCommand` and `cli::commands::review::execute`), and `apps/cli-composition/src/review_v2/{mod,run,shared,shim}.rs` (`ReviewCompositionRoot`); add boundary tests and `track/items/agent-dispatch-cost-reduction-2026-07-13/test-bindings.json` entries. IN-01, IN-03, IN-04, CN-02, CN-03, CN-06, CN-09, AC-01, AC-03, AC-04.

### S3 — Conservative type-signal freshness

> Run T005 before T011 through T018.
> Run T011 before T013; run T013 before T015 and T018.
> Run T012 before T016, run T014 before T017, and run T016 and T017 before T018.

- [x] **T005**: Modify `libs/domain/src/tddd/type_signals_doc.rs` (`BaselineHash`, `CatalogueDeclarationHash`, `EvaluatorContractHash`, `ImplementationInputHash`, `LayerId`, `LiveRustdocSnapshotHash`, `LiveRustdocSnapshotStatus`, `RustdocExtractionContractHash`, `Sha256Digest`, `Sha256DigestError`, `TrackBranch`, `TypeSignalsDocument`, `TypeSignalsCurrentInputs`, `TypeSignalsFreshness`, `TypeSignalsLoadResult`, `TypeSignalsReuseDecision`, `TypeSignalsSchemaVersion`, `TypeSignalsSchemaVersionError`, and `decide_type_signals_reuse`); add its domain tests and `track/items/agent-dispatch-cost-reduction-2026-07-13/test-bindings.json` entries. IN-05, IN-06, IN-07, CN-05, CN-07, AC-05, AC-06, AC-07, AC-08.
- [x] **T011**: Add the usecase `TypeSignalsExecutorPort` and `TypeSignalsExecutionError` in `libs/usecase/src/type_signals/ports.rs`; update `TypeSignalsInteractor`, `TypeSignalsRequest`, `TypeSignalsError`, `DiagnosticText`, and `TypeSignalsService` in `libs/usecase/src/type_signals/{interactor,service}.rs` to orchestrate layer evaluation and fail closed; add focused usecase tests and `track/items/agent-dispatch-cost-reduction-2026-07-13/test-bindings.json` entries. IN-05, IN-06, IN-07, CN-05, CN-07, AC-05, AC-06, AC-07.
- [x] **T013**: Remove the legacy domain `TypeSignalsExecutorPort` and `TypeSignalsExecutionError` from `libs/domain/src/tddd/catalogue_v2/catalogue_impl_signals_ports.rs`; implement the infallible declaration digest and codec/adapter boundaries in `libs/infrastructure/src/tddd/{type_signals_codec,type_signals_executor_adapter}.rs` (`declaration_hash`, `decode`, `encode`, `TypeSignalsCodecError`, and `TypeSignalsExecutorAdapter`); rebind `apps/cli-composition/src/signal/mod.rs` (`SignalCompositionRoot`); add focused boundary tests and `track/items/agent-dispatch-cost-reduction-2026-07-13/test-bindings.json` entries. IN-05, IN-06, IN-07, CN-05, CN-07, AC-05, AC-06, AC-07, AC-08.
- [x] **T014**: Add `RustdocSchemaExporter::existing_rustdoc_json_path` in `libs/infrastructure/src/schema_export.rs` for validated reuse of the resolved live rustdoc JSON path; add focused infrastructure tests and `track/items/agent-dispatch-cost-reduction-2026-07-13/test-bindings.json` entries. IN-06, CN-07, AC-05, AC-06, AC-07.
- [x] **T012**: Add resolved Cargo metadata package-closure, target, and build-script validation in `libs/infrastructure/src/tddd/type_signals_evaluator/build_inputs.rs` (`cargo_metadata`, `resolved_package_closure`, `resolved_target_triple`, and `reject_unresolved_build_script`); add focused infrastructure tests and `track/items/agent-dispatch-cost-reduction-2026-07-13/test-bindings.json` entries. IN-05, IN-06, CN-05, CN-07, AC-05, AC-06, AC-07.
- [x] **T016**: Add deterministic resolved-closure content hashing in `libs/infrastructure/src/tddd/type_signals_evaluator/build_inputs.rs` (`append_local_package_sources`, `collect_package_files`, `append_file`, `read_regular_file`, `cargo_configs`, `nightly_toolchain_identity`, and `append_environment`); add focused infrastructure tests and `track/items/agent-dispatch-cost-reduction-2026-07-13/test-bindings.json` entries. IN-05, IN-06, CN-05, CN-07, AC-05, AC-06, AC-07.
- [x] **T017**: Add verified live-rustdoc snapshot read, parse, hash-status, and reuse-decision helpers in `libs/infrastructure/src/tddd/type_signals_evaluator.rs` (`snapshot_status_and_content`, `RustdocJsonPathProvider`, and `reuse_decision_for_recorded_document`) using `RustdocSchemaExporter::existing_rustdoc_json_path`; add focused infrastructure tests and `track/items/agent-dispatch-cost-reduction-2026-07-13/test-bindings.json` entries. IN-05, IN-06, CN-07, AC-05, AC-06, AC-07.
- [x] **T018**: Integrate per-layer freshness evaluation in `libs/infrastructure/src/tddd/type_signals_evaluator.rs` through `EvaluateSignalsError`, `execute_type_signals_for_layer`, and `execute_type_signals_for_layer_with_dependencies`, consuming the resolved build-input and snapshot helpers; add focused infrastructure tests and `track/items/agent-dispatch-cost-reduction-2026-07-13/test-bindings.json` entries. IN-05, IN-06, IN-07, CN-05, CN-07, AC-05, AC-06, AC-07, AC-08.
- [x] **T015**: Modify the catalogue↔spec evaluation components: `libs/infrastructure/src/verify/merge_gate_adapter.rs` (`GitShowTrackBlobReader::read_spec_element_hashes`); `libs/usecase/src/merge_gate.rs` (`check_strict_merge_gate`) and `libs/usecase/src/merge_gate/chain2_gate.rs` (`check_chain2_for_layer`); `apps/cli-composition/src/track/tddd.rs` (`track_catalogue_spec_signals`); and `libs/infrastructure/src/tddd/catalogue_spec_signals_refresher.rs` (`refresh_one_layer`). Retain `libs/infrastructure/src/verify/catalogue_spec_signals.rs` (`compute_catalogue_declaration_hash`) and fail-closed `LoadCatalogueSpecSignalsForViewError` handling in `libs/infrastructure/src/type_catalogue_render.rs`; add focused Chain ②/prelude tests and `track/items/agent-dispatch-cost-reduction-2026-07-13/test-bindings.json` entries. IN-05, AC-05, AC-06.

### S4 — Opt-in capability session reuse

> Run T007 after T001, T002, and T003.
> Run T008 after T007.

- [ ] **T007**: Modify `libs/usecase/src/capability_exec.rs` (`CapabilityDispatchOutcome`, `CapabilityDispatchRequest`, `CapabilityExecRequest`, `CapabilityProviderPort`, `CapabilityFailureDetail`, `CapabilityFilePath`, `CapabilityInputValidationError`, `CapabilityResumeRequest`, and `TimeoutSeconds`) and `libs/infrastructure/src/capability_exec/{claude,codex}.rs` (`ClaudeCapabilityAdapter` and `CodexCapabilityAdapter`); add usecase and process-boundary tests plus `track/items/agent-dispatch-cost-reduction-2026-07-13/test-bindings.json` entries. IN-01, IN-02, IN-08, IN-09, CN-01, CN-02, CN-04, CN-08, CN-10, AC-01, AC-02, AC-09, AC-10.
- [ ] **T008**: Modify `apps/cli-driver/src/capability.rs` (`CapabilityDriver`, `CapabilityExecDriverInput`, `CapabilityResumeArg`, and `TargetArtifactPathArg`), `apps/cli/src/commands/capability.rs` (`CapabilityCommand`, `CapabilityExecArgs`, and `cli::commands::capability::execute`), and `apps/cli-composition/src/capability.rs` (`CapabilityCompositionRoot`); add CLI fixtures and `track/items/agent-dispatch-cost-reduction-2026-07-13/test-bindings.json` entries. IN-08, IN-09, CN-04, CN-10, AC-09, AC-10.

### S5 — Operational contract cutover

> Run T009 after the reviewer and capability resume paths in T004 and T008 are available.

- [ ] **T009**: Modify `.harness/prompts/capability-exec-discipline.md`, `.harness/capabilities/review-fix-lead.md`, and `.harness/capabilities/{adr-editor,spec-designer,type-designer,impl-planner,implementer}.md` to add the operational-contract clauses and their conformance checks. IN-03, IN-08, CN-03, CN-10, AC-03, AC-10.
