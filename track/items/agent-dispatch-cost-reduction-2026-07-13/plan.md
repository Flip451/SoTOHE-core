<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# 外部 agent 呼び出しのコスト削減

## Summary

GO-01 → T001, T002, T004, T007.
GO-02 → T003, T004, T007, T008, T009.
GO-03 → T005, T011, T012, T013, T014, T015, T016, T017, T018.
GO-04 → T001, T002, T003, T004, T005, T007, T008, T009, T011, T012, T013, T014, T015, T016, T017, T018, T019.

## Tasks (15/17 resolved)

### S1 — Explicit dispatch profile resolution

> Run T001 before T002.
> T003, T004, and T007 depend on the resolved profile contract from this section.

- [x] **T001**: Modify `libs/usecase/src/capability_exec.rs` (`ReasoningEffort`, `CapabilityProfile`, `CapabilityProfilePort::resolve`, and `CapabilityExecError`) and its unit-test module; add the corresponding bindings in `track/items/agent-dispatch-cost-reduction-2026-07-13/test-bindings.json`. IN-01, IN-02, CN-01, CN-02, AC-01, AC-02. (`fc39a2ec`)
- [x] **T002**: Modify `libs/infrastructure/src/agent_profiles.rs` (`AgentProfiles`, `AgentProfilesError` error mapping, `CapabilityConfigDto`, `ExecutionModeDto`, `ResolvedExecution`, and routing DTOs), `libs/infrastructure/src/capability_exec/agent_profiles.rs` (`AgentProfilesCapabilityAdapter::resolve`), and `libs/infrastructure/src/ref_verify/process_runner.rs` (the Claude, Codex, and Gemini ref-verifier argument builders); update `.harness/config/agent-profiles.json` and `.harness/config/samples/agent-profiles.default.json`, their fixtures, and `track/items/agent-dispatch-cost-reduction-2026-07-13/test-bindings.json`. IN-01, IN-02, CN-01, AC-01, AC-02. (`fc39a2ec`)

### S2 — Safe reviewer session reuse

> Run T003 after T001 and T002.
> Run T004 after T003.

- [~] **T003**: Extend `libs/usecase/src/capability_exec.rs` with `TargetArtifactPath` and `TargetArtifactSet`; create `libs/usecase/src/provider_session.rs` (`ProviderSessionCacheEntry`, `ProviderSessionCacheError`, `ProviderSessionCacheKey`, `ProviderSessionCachePort`, `ProviderSessionId`, and `ReviewerPrompt`) and `libs/infrastructure/src/provider_session.rs` (`FsProviderSessionCacheAdapter`); export both modules from their `lib.rs` files, consume existing `TrackId`, `ScopeName`, and `RoundType` identity values, and add focused tests plus `track/items/agent-dispatch-cost-reduction-2026-07-13/test-bindings.json` entries. IN-03, IN-04, IN-08, IN-09, CN-04, CN-06, CN-08, AC-03, AC-04, AC-09, AC-10.
- [x] **T004**: Modify `libs/usecase/src/review_v2/ports.rs` (`Reviewer`), `libs/infrastructure/src/review_v2/{claude_reviewer,codex_reviewer}.rs` (`ClaudeReviewer` and `CodexReviewer`), `apps/cli/src/commands/review/{mod,local,claude_local,codex_local}.rs` (`ReviewCommand` and `cli::commands::review::execute`), and `apps/cli-composition/src/review_v2/{mod,run,shared,shim}.rs` (`ReviewCompositionRoot`); add boundary tests and `track/items/agent-dispatch-cost-reduction-2026-07-13/test-bindings.json` entries. IN-01, IN-03, IN-04, CN-02, CN-03, CN-06, CN-09, AC-01, AC-03, AC-04.

### S3 — Conservative type-signal freshness

> Run T005 before the active type-signal tasks T011, T013, T014, T015, T016, and T018.
> Run T011 before T013; run T013 before T015 and T018.
> Run T012 and T016 before T018. Run T017 before freshness-driven signal re-evaluation.

- [x] **T005**: Modify `libs/domain/src/tddd/type_signals_doc.rs` for `CatalogueDeclarationHash`, `ImplementationInputHash`, `LayerId`, `Sha256Digest`, `Sha256DigestError`, `TrackBranch`, `TypeSignalsDocument`, `TypeSignalsLoadResult`, `TypeSignalsReuseDecision`, `TypeSignalsSchemaVersion`, `TypeSignalsSchemaVersionError`, and `decide_type_signals_reuse`; add domain tests and `track/items/agent-dispatch-cost-reduction-2026-07-13/test-bindings.json` entries. IN-05, IN-06, IN-07, CN-05, CN-07, AC-05, AC-06, AC-07, AC-08. (`a8d5f6b1`)
- [x] **T011**: Add `TypeSignalsExecutorPort` and `TypeSignalsExecutionError` in `libs/usecase/src/type_signals/ports.rs`; update `TypeSignalsInteractor`, `TypeSignalsRequest`, `TypeSignalsError`, `DiagnosticText`, and `TypeSignalsService` in `libs/usecase/src/type_signals/{interactor,service}.rs`; add focused usecase tests and `track/items/agent-dispatch-cost-reduction-2026-07-13/test-bindings.json` entries. IN-05, IN-06, IN-07, CN-05, CN-07, CN-11, AC-05, AC-06, AC-07. (`a8d5f6b1`)
- [x] **T013**: Remove the legacy domain `TypeSignalsExecutorPort` and `TypeSignalsExecutionError` from `libs/domain/src/tddd/catalogue_v2/catalogue_impl_signals_ports.rs`; implement the declaration digest and codec/adapter boundaries in `libs/infrastructure/src/tddd/{type_signals_codec,type_signals_executor_adapter}.rs` (`declaration_hash`, `decode`, `encode`, `TypeSignalsCodecError`, and `TypeSignalsExecutorAdapter`); rebind `apps/cli-composition/src/signal/mod.rs` (`SignalCompositionRoot`); add focused boundary tests and `track/items/agent-dispatch-cost-reduction-2026-07-13/test-bindings.json` entries. IN-05, IN-06, IN-07, CN-05, CN-07, AC-05, AC-06, AC-07, AC-08. (`a8d5f6b1`)
- [x] **T014**: Wire the existing `SchemaExporter`, `SchemaExporterPort`, and `RustdocSchemaExporter` boundary into type-signal evaluation; add focused infrastructure tests and `track/items/agent-dispatch-cost-reduction-2026-07-13/test-bindings.json` entries. IN-05, IN-06, CN-07, AC-05, AC-06, AC-07. (`a8d5f6b1`)
- [x] **T012**: Modify `libs/infrastructure/src/tddd/type_signals_evaluator/build_inputs.rs` (`hash_implementation_inputs`, `append_component`, sorted traversal, and bounded read helpers); implement/add tests and bindings entries. IN-05, CN-05, AC-05. (`a8d5f6b1`)
- [x] **T016**: Implement per-layer implementation-input hashing in `libs/infrastructure/src/tddd/type_signals_evaluator/build_inputs.rs`; add focused infrastructure tests and `track/items/agent-dispatch-cost-reduction-2026-07-13/test-bindings.json` entries. IN-05, IN-06, CN-05, CN-07, AC-05, AC-06, AC-07. (`a8d5f6b1`)
- [x] **T017**: Modify `libs/infrastructure/src/tddd/type_signals_evaluator/freshness.rs` (`existing_rustdoc_content` and `RustdocJsonPathProvider`); implement/add tests and bindings entries. IN-06, AC-06. (`a8d5f6b1`)
- [x] **T018**: Integrate `EvaluateSignalsError` and `execute_type_signals_for_layer` in `libs/infrastructure/src/tddd/type_signals_evaluator.rs` with the build-input helper; add focused infrastructure tests and `track/items/agent-dispatch-cost-reduction-2026-07-13/test-bindings.json` entries. IN-05, IN-06, IN-07, CN-05, CN-07, AC-05, AC-06, AC-07, AC-08. (`a8d5f6b1`)
- [x] **T015**: Modify the catalogue↔spec evaluation components: `libs/infrastructure/src/verify/merge_gate_adapter.rs` (`GitShowTrackBlobReader::read_spec_element_hashes`); `libs/usecase/src/merge_gate.rs` (`check_strict_merge_gate`) and `libs/usecase/src/merge_gate/chain2_gate.rs` (`check_chain2_for_layer`); `apps/cli-composition/src/track/tddd.rs` (`track_catalogue_spec_signals`); `libs/infrastructure/src/tddd/catalogue_spec_signals_refresher.rs` (`refresh_one_layer`); `libs/infrastructure/src/verify/catalogue_spec_signals.rs` (`compute_catalogue_declaration_hash`); and `libs/infrastructure/src/type_catalogue_render.rs` (`LoadCatalogueSpecSignalsForViewError`). Add focused Chain ②/prelude tests and `track/items/agent-dispatch-cost-reduction-2026-07-13/test-bindings.json` entries. IN-05, AC-05, AC-06. (`a8d5f6b1`)

### S4 — Opt-in capability session reuse

> Run T007 after T001, T002, and T003.
> Run T008 after T007.

- [x] **T007**: Modify `libs/usecase/src/capability_exec.rs` (`CapabilityDispatchOutcome`, `CapabilityDispatchRequest`, `CapabilityExecRequest`, `CapabilityProviderPort`, `CapabilityFailureDetail`, `CapabilityFilePath`, `CapabilityInputValidationError`, `CapabilityResumeRequest`, and `TimeoutSeconds`) and `libs/infrastructure/src/capability_exec/{claude,codex}.rs` (`ClaudeCapabilityAdapter` and `CodexCapabilityAdapter`); add usecase and process-boundary tests plus `track/items/agent-dispatch-cost-reduction-2026-07-13/test-bindings.json` entries. IN-01, IN-02, IN-08, IN-09, CN-01, CN-02, CN-04, CN-08, CN-10, AC-01, AC-02, AC-09, AC-10.
- [x] **T008**: Modify `apps/cli-driver/src/capability.rs` (`CapabilityDriver`, `CapabilityExecDriverInput`, `CapabilityResumeArg`, and `TargetArtifactPathArg`), `apps/cli/src/commands/capability.rs` (`CapabilityCommand`, `CapabilityExecArgs`, `cli::commands::capability::execute`, and `cli::commands::capability::into_driver_input`), and `apps/cli-composition/src/capability.rs` (`CapabilityCompositionRoot`); add CLI fixtures and `track/items/agent-dispatch-cost-reduction-2026-07-13/test-bindings.json` entries. IN-08, IN-09, CN-04, CN-10, AC-09, AC-10.

### S5 — Operational contract cutover

> Run T009 after the reviewer and capability resume paths in T004 and T008 are available.

- [ ] **T009**: Modify `.harness/prompts/capability-exec-discipline.md`, `.harness/workflows/track/review.md`, `.harness/capabilities/{review-fix-lead,adr-editor,spec-designer,type-designer,impl-planner,implementer}.md`, `.claude/agents/{adr-editor,spec-designer,type-designer,impl-planner,implementer,review-fix-lead}.md`, `.agents/skills/{adr-editor,spec-designer,type-designer,impl-planner,implementer,review-fix-lead}/SKILL.md`, `.codex/agents/{adr-editor,spec-designer,type-designer,impl-planner,review-fix-lead}.toml`, and `.codex/instructions.md`; add the resume explicit-flag re-specification and upstream re-read conformance check (IN-03, IN-08, CN-03, CN-10, AC-03, AC-10).

### S6 — Test-obligation machinery repair

> Run T019 after its domain catalogue declarations. Anchors: IN-10, AC-11.

- [x] **T019**: Modify `libs/domain/src/tddd/test_obligation/{obligations,binding}.rs` (`EdgeOwnership`, `TestObligation::{target_entry,target_role,brief,declaration_hash,spec_refs,owns_edge}`, `ObligationsDocument::{obligations,edge_ownership,owners_of_edge}`, and `TestBindingsDocument::{records,waived_edge_ids,is_edge_waived}`) and `libs/usecase/src/test_obligation/evaluate/{plan,edges,mod}.rs` (`find_obligation`, `resolve_anchor_text`, `synthetic_obligation_id`, `PlannedAction`, `ImmediateOutcome`, and `EvaluateTestObligationsInteractor::{plan_binding_records,apply_planned,fulfillment_llm_future,waiver_llm_future,bound_tests}`); expose the required domain methods and update evaluation imports and call sites to use them; add focused regression tests and `track/items/agent-dispatch-cost-reduction-2026-07-13/test-bindings.json` entries. (IN-10, AC-11).
