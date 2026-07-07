<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# テスト義務ゲートと obligation-fulfillment 意味論検証 — SoT chain 第三リンクの意味論検証の完成

## Tasks (0/23 resolved)

### S1 — Preceding shared foundation: semantic-verdict core & ref-verify migration

- [ ] **T001**: Target libs/usecase tddd::semantic_verdict_core. Add SemanticEscalationDriverPort (generic K/V trait), SemanticEscalationFuture, SemanticEscalationVerdictBridge, SemanticCalibrationProbeConfig at semantic_verdict_core::{driver,verdict,probe} (IN-01/AC-15/OS-02). Add unit tests for bridge variant construction, probe config new(), and a stub SemanticEscalationDriverPort double covering evaluate_with_escalation.
- [ ] **T002**: Target libs/usecase ref_verify chain-1 (spec-adr) and chain-2 (catalog-spec) drivers. Migrate their fast/final/human escalation, hash-frozen cache access, and calibration probe injection to consume SemanticEscalationDriverPort + SemanticCalibrationProbeConfig from tddd::semantic_verdict_core (IN-01/AC-15/OS-02). Keep existing ref-verify public API and existing test surface unchanged; adjust wiring/composition only.

### S2 — Decision-table config surface

- [ ] **T003**: Target libs/domain tddd::test_obligation::{rules,vocab,errors,ports}. Add RoleObligationRules, TestObligationRule, TestObligationRulesDocument, TestObligationBriefTemplate, TestObligationMinimum; TestObligationKind, TestObligationPerAxis, TestObligationPatternKind, TargetEntryRoleKind; TestObligationRulesLoadError, ValidationError; TestObligationRulesLoaderPort (IN-02/IN-04/IN-17/CN-05/CN-10/CN-16). Add unit tests for validating constructors and enum totality.
- [ ] **T004**: Target libs/infrastructure test_obligation::rules_codec. Add DataRoleKey, ContractRoleKey, FunctionRoleKey, PatternKey, TestObligationPerAxisDto, TestObligationRuleDto, RoleObligationRulesDto, TestObligationRulesDocumentDto; JsonTestObligationRulesLoader impl TestObligationRulesLoaderPort (IN-02/IN-04/IN-17/AC-02/AC-16/CN-05/CN-10/OS-05). Add unit tests covering those anchors.
- [ ] **T005**: Target .harness/config/test-obligation-rules.json. Author default test-obligation rules config (IN-02/IN-03/AC-01/CN-10/CN-12/CN-16/OS-03/OS-05). Add decoding smoke test for JsonTestObligationRulesLoader over the default config.

### S3 — Domain foundation: hashes, ids, obligations, bindings, verdicts, drift, errors, ports

- [~] **T006**: Target libs/domain tddd::test_obligation::{hashes,ids}. Add AnchorTextHash, DeclarationHash, BoundTestsSetHash, WaivedReasonHash, TestBodySpanHash; TestObligationId, TestObligationAnchorId, TestObligationEdgeId, TestObligationItemIdentifier, TestObligationBrief, TestModulePath, TestFunctionName, WaivedReason, RoleName, DiagnosticMessage (IN-05/IN-06/CN-01). Add unit tests covering IN-05/IN-06/CN-01.
- [~] **T007**: Target libs/domain tddd::test_obligation::{obligations,binding,scope}. Add TestObligation, ObligationsDocument; TestLocation, NonEmptyTestLocations, TestBindingRecord, TestBindingsDocument; TestObligationScopePresence, UncitedSpecElementFinding (IN-05/IN-06/IN-14/IN-16/CN-02/CN-07/CN-13/CN-15/AC-13). Add unit tests covering those anchors.
- [~] **T008**: Target libs/domain tddd::test_obligation::{verdict,drift}. Add ObligationFulfillmentVerdict, WaiverVerdict; ObligationFulfillmentCacheKey, ObligationFulfillmentCacheEntry, ObligationFulfillmentCacheDocument; WaiverCacheKey, WaiverCacheEntry, WaiverCacheDocument; TestObligationDriftKind, TestObligationDrift, EdgeResolutionOutcome, EdgeVerdictRecord (IN-13/CN-01/CN-04/AC-05). Add unit tests covering those anchors.
- [~] **T009**: Target libs/domain tddd::test_obligation::{errors,ports,vocab}. Add ObligationCheckError, ObligationDeriveError, ObligationEvaluateError, ObligationResultsError, FulfillmentFailCategory, SemanticVerifierError, ArtifactCodecError, TestSourceScanError, VerifyCacheError; ObligationsArtifactPort, TestBindingsArtifactPort, ObligationFulfillmentCachePort, WaiverCachePort, ObligationFulfillmentVerifierPort, WaiverVerifierPort, TestSourceScannerPort (IN-08/IN-11/IN-12/CN-08). Add unit tests for Display/Error over every error variant.

### S4 — Infrastructure codecs

- [ ] **T010**: Target libs/infrastructure test_obligation::obligations_codec. Add CatalogueEntryRefDto, ObligationsDocumentDto, TestObligationDto, TestObligationIdDto, TestObligationAnchorIdDto, TestObligationKindDto, ObligationsCodecError; JsonObligationsCodec impl ObligationsArtifactPort (IN-05/AC-03/CN-01). Add unit tests covering those anchors.
- [ ] **T011**: Target libs/infrastructure test_obligation::bindings_codec. Add TestBindingsDocumentDto, TestBindingRecordDto (three-form union), TestLocationDto, TestObligationEdgeIdDto, TestBindingsCodecError; JsonTestBindingsCodec impl TestBindingsArtifactPort (IN-06/OS-06/AC-04). Add unit tests for each TestBindingRecord form (fulfillment / waiver / voluntary) round-tripping.
- [ ] **T012**: Target libs/infrastructure test_obligation::{fulfillment_cache_codec,waiver_cache_codec}. Add ObligationFulfillmentCacheDocumentDto, ObligationFulfillmentCacheEntryDto, ObligationFulfillmentVerdictDto, FulfillmentFailCategoryDto; WaiverCacheDocumentDto, WaiverCacheEntryDto, WaiverVerdictDto; JsonObligationFulfillmentCacheCodec impl ObligationFulfillmentCachePort; JsonWaiverCacheCodec impl WaiverCachePort (AC-06/CN-04). Add unit tests: hash-triple serialization and decoding.

### S5 — Test source scanning + LLM verifier adapters

- [ ] **T013**: Target libs/infrastructure test_obligation::source_scanner. Add SynTestSourceScanner impl TestSourceScannerPort (IN-06/OS-06/CN-14/AC-04). Add unit tests covering those anchors.
- [ ] **T014**: Target libs/infrastructure test_obligation::fulfillment_verifier. Add ObligationFulfillmentVerifierAdapter impl ObligationFulfillmentVerifierPort with obligation-fulfillment-verifier provider wiring (IN-09/IN-12/CN-03/CN-08/AC-06/AC-08/OS-01). Add unit tests with a stubbed provider covering those anchors.
- [ ] **T015**: Target libs/infrastructure test_obligation::waiver_verifier. Add WaiverVerifierAdapter impl WaiverVerifierPort with waiver-verifier provider wiring (IN-09/IN-15/CN-03/CN-08/OS-01/OS-04). Add unit tests with a stubbed provider covering those anchors.

### S6 — Usecase interactors: derive, check, evaluate, results

- [ ] **T016**: Target libs/usecase test_obligation::derive. Add DeriveTestObligationsCommand, DeriveTestObligationsApplicationService (trait), DeriveTestObligationsInteractor using catalogue snapshot, spec.json, and TestObligationRulesDocument inputs (IN-07/IN-17/CN-06/CN-11/CN-13/CN-15/CN-16/AC-03/AC-12/AC-16/AC-17/OS-05/OS-09). Add unit tests covering those anchors.
- [ ] **T017**: Target libs/usecase test_obligation::check. Add CheckTestObligationsCommand, CheckTestObligationsOutcome, CheckTestObligationsApplicationService (trait), CheckTestObligationsInteractor using ObligationsDocument and TestBindingsDocument inputs (IN-08/IN-13/IN-14/IN-16/OS-08/OS-10/CN-02/CN-04/CN-07/CN-09/AC-04/AC-05/AC-10/AC-12/AC-13/AC-17). Add unit tests covering those anchors.
- [ ] **T018**: Target libs/usecase test_obligation::evaluate. Add EvaluateTestObligationsCommand, EvaluateTestObligationsOutcome, TestObligationEvaluateConfig, EvaluateTestObligationsApplicationService (trait), EvaluateTestObligationsInteractor using SemanticEscalationDriverPort, ObligationFulfillmentVerifierPort, and WaiverVerifierPort (IN-09/IN-11/IN-12/IN-15/OS-01/OS-04/OS-07/CN-03/CN-04/CN-08/CN-12/AC-06/AC-11/AC-12/AC-16). Add unit tests covering those anchors.
- [ ] **T019**: Target libs/usecase test_obligation::results. Add TestObligationResultsCommand, TestObligationResultsOutput, TestObligationLaneSummary, TestObligationChainLabel, TestObligationResultsApplicationService (trait), TestObligationResultsInteractor using ObligationFulfillmentCacheDocument, WaiverCacheDocument, and drift inputs (IN-10/CN-09/AC-09). Add unit tests covering those anchors.

### S7 — Presentation wiring: cli-driver, cli-composition, cli

- [ ] **T020**: Target apps/cli-driver test_obligation::{derive,check,evaluate,results}. Add TestObligationDeriveInput, TestObligationCheckInput, TestObligationEvaluateInput, TestObligationResultsInput; add TestObligationDeriveHandler, TestObligationCheckHandler, TestObligationEvaluateHandler, TestObligationResultsHandler as thin primary adapters translating parsed args to the four application-service commands and mapping outcomes to CommandOutcome (IN-07/IN-08/IN-09/IN-10). Add unit tests against application-service doubles.
- [ ] **T021**: Target apps/cli-composition test_obligation. Add TestObligationCompositionRoot wiring JsonTestObligationRulesLoader, JsonObligationsCodec, JsonTestBindingsCodec, JsonObligationFulfillmentCacheCodec, JsonWaiverCacheCodec, SynTestSourceScanner, ObligationFulfillmentVerifierAdapter, WaiverVerifierAdapter → four Interactors → four Handlers (IN-07/IN-08/IN-09/IN-10). Add integration test exercising the composition root with test doubles for the LLM verifiers.
- [ ] **T022**: Target apps/cli commands::test_obligation and apps/cli/src/main.rs. Add TestObligationSubcommand (Derive/Check/Evaluate/Results), TestObligationArgs, TestObligationDeriveArgs, TestObligationCheckArgs, TestObligationEvaluateArgs, TestObligationResultsArgs, dispatch through TestObligationCompositionRoot, and top-level `sotp test-obligation` registration (IN-07/IN-08/IN-09/IN-10/OS-10/CN-09/AC-09/AC-14). Add CLI integration tests covering those anchors.

### S8 — agent-profiles capability registration

- [ ] **T023**: Target .harness/config/agent-profiles.json and the Rust-side capability registry used by resolve_execution. Register `obligation-fulfillment-verifier` and `waiver-verifier` capabilities (IN-11/CN-08/AC-07). Add unit tests covering those anchors.
