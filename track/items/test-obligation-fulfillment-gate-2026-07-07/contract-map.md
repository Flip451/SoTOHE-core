<!-- Generated contract-map-renderer — DO NOT EDIT DIRECTLY -->
```mermaid
---
config:
  layout: elk
---
flowchart LR
classDef aggregate_root fill:#ede9fe,stroke:#4c1d95,stroke-width:2px
classDef app_service fill:#ecfdf5,stroke:#059669,stroke-width:2px
classDef command fill:#fff7ed,stroke:#c2410c,stroke-width:1px
classDef domain_service fill:#fee2e2,stroke:#991b1b,stroke-width:1px
classDef dto fill:#f8fafc,stroke:#64748b,stroke-width:1px
classDef entity fill:#dbeafe,stroke:#1e40af,stroke-width:2px
classDef error_type fill:#fef2f2,stroke:#b91c1c,stroke-width:1px,stroke-dasharray:4 2
classDef factory fill:#e0f2fe,stroke:#0369a1,stroke-width:1px
classDef free_function fill:#f5f3ff,stroke:#7c3aed,stroke-width:1px
classDef function_node fill:#f5f3ff,stroke:#a78bfa,stroke-width:1px
classDef interactor fill:#f0fdfa,stroke:#0d9488,stroke-width:1px
classDef method_node fill:#f8fafc,stroke:#cbd5e1,stroke-width:1px
classDef query fill:#f0f9ff,stroke:#0369a1,stroke-width:1px
classDef secondary_adapter fill:#fafaf9,stroke:#57534e,stroke-width:1px
classDef secondary_port fill:#fafaf9,stroke:#78716c,stroke-width:1px,stroke-dasharray:4 2
classDef specification fill:#fdf4ff,stroke:#6b21a8,stroke-width:1px
classDef specification_port fill:#fdf4ff,stroke:#9333ea,stroke-width:1px,stroke-dasharray:4 2
classDef typestate_overlay stroke:#dc2626,stroke-width:3px
classDef use_case fill:#ecfeff,stroke:#0e7490,stroke-width:1px
classDef use_case_function fill:#eef2ff,stroke:#4338ca,stroke-width:1px
classDef value_object fill:#d1fae5,stroke:#065f46,stroke-width:1px
classDef variant_node fill:#fafaf9,stroke:#d6d3d1,stroke-width:1px
subgraph domain["domain"]
  direction TB
  subgraph domain_domain_module_tddd["domain::tddd"]
    direction TB
  subgraph T28_domain_domain_AnchorTextHash["tddd::test_obligation::hashes::AnchorTextHash"]
    direction TB
    T28_domain_domain_AnchorTextHash__self[AnchorTextHash]
    T28_domain_domain_AnchorTextHash_new([new])
    T28_domain_domain_AnchorTextHash_as_hash([as_hash])
  end
  subgraph T32_domain_domain_ArtifactCodecError["tddd::test_obligation::errors::ArtifactCodecError"]
    direction TB
    T32_domain_domain_ArtifactCodecError__self[ArtifactCodecError]
    T32_domain_domain_ArtifactCodecError_Io[Io]
    T32_domain_domain_ArtifactCodecError_MalformedJson[MalformedJson]
    T32_domain_domain_ArtifactCodecError_DomainInvariant[DomainInvariant]
  end
  subgraph T31_domain_domain_BoundTestsSetHash["tddd::test_obligation::hashes::BoundTestsSetHash"]
    direction TB
    T31_domain_domain_BoundTestsSetHash__self[BoundTestsSetHash]
    T31_domain_domain_BoundTestsSetHash_new([new])
    T31_domain_domain_BoundTestsSetHash_as_hash([as_hash])
  end
  subgraph T29_domain_domain_DeclarationHash["tddd::test_obligation::hashes::DeclarationHash"]
    direction TB
    T29_domain_domain_DeclarationHash__self[DeclarationHash]
    T29_domain_domain_DeclarationHash_new([new])
    T29_domain_domain_DeclarationHash_as_hash([as_hash])
  end
  subgraph T31_domain_domain_DiagnosticMessage["tddd::test_obligation::ids::DiagnosticMessage"]
    direction TB
    T31_domain_domain_DiagnosticMessage__self[DiagnosticMessage]
    T31_domain_domain_DiagnosticMessage_try_new([try_new])
    T31_domain_domain_DiagnosticMessage_as_str([as_str])
  end
  subgraph T35_domain_domain_EdgeResolutionOutcome["tddd::test_obligation::drift::EdgeResolutionOutcome"]
    direction TB
    T35_domain_domain_EdgeResolutionOutcome__self[EdgeResolutionOutcome]
    T35_domain_domain_EdgeResolutionOutcome_Fulfilled[Fulfilled]
    T35_domain_domain_EdgeResolutionOutcome_Waived[Waived]
    T35_domain_domain_EdgeResolutionOutcome_Fail[Fail]
    T35_domain_domain_EdgeResolutionOutcome_Pending[Pending]
    T35_domain_domain_EdgeResolutionOutcome_MissingBinding[MissingBinding]
  end
  subgraph T31_domain_domain_EdgeVerdictRecord["tddd::test_obligation::drift::EdgeVerdictRecord"]
    direction TB
    T31_domain_domain_EdgeVerdictRecord__self[EdgeVerdictRecord]
    T31_domain_domain_EdgeVerdictRecord_new([new])
  end
  subgraph T37_domain_domain_FulfillmentFailCategory["tddd::test_obligation::vocab::FulfillmentFailCategory"]
    direction TB
    T37_domain_domain_FulfillmentFailCategory__self[FulfillmentFailCategory]
    T37_domain_domain_FulfillmentFailCategory_Contradiction[Contradiction]
    T37_domain_domain_FulfillmentFailCategory_Substitution[Substitution]
    T37_domain_domain_FulfillmentFailCategory_CentralUnverified[CentralUnverified]
    T37_domain_domain_FulfillmentFailCategory_as_kebab([as_kebab])
  end
  subgraph T34_domain_domain_ObligationCheckError["tddd::test_obligation::errors::ObligationCheckError"]
    direction TB
    T34_domain_domain_ObligationCheckError__self[ObligationCheckError]
    T34_domain_domain_ObligationCheckError_ObligationsOnly[ObligationsOnly]
    T34_domain_domain_ObligationCheckError_BindingsOnly[BindingsOnly]
    T34_domain_domain_ObligationCheckError_TrackNotActive[TrackNotActive]
    T34_domain_domain_ObligationCheckError_DriftsDetected[DriftsDetected]
    T34_domain_domain_ObligationCheckError_UnresolvedEdges[UnresolvedEdges]
    T34_domain_domain_ObligationCheckError_StaleVerdicts[StaleVerdicts]
    T34_domain_domain_ObligationCheckError_ArtifactCodec[ArtifactCodec]
    T34_domain_domain_ObligationCheckError_SourceScan[SourceScan]
    T34_domain_domain_ObligationCheckError_CacheIo[CacheIo]
  end
  subgraph T35_domain_domain_ObligationDeriveError["tddd::test_obligation::errors::ObligationDeriveError"]
    direction TB
    T35_domain_domain_ObligationDeriveError__self[ObligationDeriveError]
    T35_domain_domain_ObligationDeriveError_RulesLoad[RulesLoad]
    T35_domain_domain_ObligationDeriveError_TrackNotActive[TrackNotActive]
    T35_domain_domain_ObligationDeriveError_CatalogueLoad[CatalogueLoad]
    T35_domain_domain_ObligationDeriveError_SpecLoad[SpecLoad]
    T35_domain_domain_ObligationDeriveError_InvalidCatalogueState[InvalidCatalogueState]
    T35_domain_domain_ObligationDeriveError_ArtifactCodec[ArtifactCodec]
    T35_domain_domain_ObligationDeriveError_ArtifactWrite[ArtifactWrite]
  end
  subgraph T37_domain_domain_ObligationEvaluateError["tddd::test_obligation::errors::ObligationEvaluateError"]
    direction TB
    T37_domain_domain_ObligationEvaluateError__self[ObligationEvaluateError]
    T37_domain_domain_ObligationEvaluateError_InvalidConfig[InvalidConfig]
    T37_domain_domain_ObligationEvaluateError_TrackNotActive[TrackNotActive]
    T37_domain_domain_ObligationEvaluateError_VerifierPort[VerifierPort]
    T37_domain_domain_ObligationEvaluateError_CachePersistence[CachePersistence]
    T37_domain_domain_ObligationEvaluateError_CacheWrite[CacheWrite]
    T37_domain_domain_ObligationEvaluateError_SemanticFailuresConfirmed[SemanticFailuresConfirmed]
    T37_domain_domain_ObligationEvaluateError_HumanEscalationRequired[HumanEscalationRequired]
  end
  subgraph T48_domain_domain_ObligationFulfillmentCacheDocument["tddd::test_obligation::verdict::ObligationFulfillmentCacheDocument"]
    direction TB
    T48_domain_domain_ObligationFulfillmentCacheDocument__self[ObligationFulfillmentCacheDocument]
    T48_domain_domain_ObligationFulfillmentCacheDocument_new([new])
    T48_domain_domain_ObligationFulfillmentCacheDocument_entries([entries])
  end
  subgraph T45_domain_domain_ObligationFulfillmentCacheEntry["tddd::test_obligation::verdict::ObligationFulfillmentCacheEntry"]
    direction TB
    T45_domain_domain_ObligationFulfillmentCacheEntry__self[ObligationFulfillmentCacheEntry]
    T45_domain_domain_ObligationFulfillmentCacheEntry_new([new])
  end
  subgraph T43_domain_domain_ObligationFulfillmentCacheKey["tddd::test_obligation::verdict::ObligationFulfillmentCacheKey"]
    direction TB
    T43_domain_domain_ObligationFulfillmentCacheKey__self[ObligationFulfillmentCacheKey]
    T43_domain_domain_ObligationFulfillmentCacheKey_new([new])
  end
  subgraph T42_domain_domain_ObligationFulfillmentVerdict["tddd::test_obligation::verdict::ObligationFulfillmentVerdict"]
    direction TB
    T42_domain_domain_ObligationFulfillmentVerdict__self[ObligationFulfillmentVerdict]
    T42_domain_domain_ObligationFulfillmentVerdict_Fulfilled[Fulfilled]
    T42_domain_domain_ObligationFulfillmentVerdict_Fail[Fail]
    T42_domain_domain_ObligationFulfillmentVerdict_Pending[Pending]
  end
  subgraph T36_domain_domain_ObligationResultsError["tddd::test_obligation::errors::ObligationResultsError"]
    direction TB
    T36_domain_domain_ObligationResultsError__self[ObligationResultsError]
    T36_domain_domain_ObligationResultsError_IoError[IoError]
    T36_domain_domain_ObligationResultsError_MalformedArtifact[MalformedArtifact]
  end
  subgraph T33_domain_domain_ObligationsDocument["tddd::test_obligation::obligations::ObligationsDocument"]
    direction TB
    T33_domain_domain_ObligationsDocument__self[ObligationsDocument]
    T33_domain_domain_ObligationsDocument_new([new])
    T33_domain_domain_ObligationsDocument_track_id([track_id])
    T33_domain_domain_ObligationsDocument_obligations([obligations])
  end
  subgraph T22_domain_domain_RoleName["tddd::test_obligation::ids::RoleName"]
    direction TB
    T22_domain_domain_RoleName__self[RoleName]
    T22_domain_domain_RoleName_try_new([try_new])
    T22_domain_domain_RoleName_as_str([as_str])
  end
  subgraph T33_domain_domain_RoleObligationRules["tddd::test_obligation::rules::RoleObligationRules"]
    direction TB
    T33_domain_domain_RoleObligationRules__self[RoleObligationRules]
    T33_domain_domain_RoleObligationRules_new([new])
    T33_domain_domain_RoleObligationRules_obligations([obligations])
    T33_domain_domain_RoleObligationRules_is_empty_explicitly([is_empty_explicitly])
  end
  subgraph T35_domain_domain_SemanticVerifierError["tddd::test_obligation::errors::SemanticVerifierError"]
    direction TB
    T35_domain_domain_SemanticVerifierError__self[SemanticVerifierError]
    T35_domain_domain_SemanticVerifierError_VerifierPort[VerifierPort]
  end
  subgraph T33_domain_domain_TargetEntryRoleKind["tddd::test_obligation::vocab::TargetEntryRoleKind"]
    direction TB
    T33_domain_domain_TargetEntryRoleKind__self[TargetEntryRoleKind]
    T33_domain_domain_TargetEntryRoleKind_DataRole[DataRole]
    T33_domain_domain_TargetEntryRoleKind_ContractRole[ContractRole]
    T33_domain_domain_TargetEntryRoleKind_FunctionRole[FunctionRole]
    T33_domain_domain_TargetEntryRoleKind_TraitImpl[TraitImpl]
    T33_domain_domain_TargetEntryRoleKind_Pattern[Pattern]
  end
  subgraph T31_domain_domain_TestBindingRecord["tddd::test_obligation::binding::TestBindingRecord"]
    direction TB
    T31_domain_domain_TestBindingRecord__self[TestBindingRecord]
    T31_domain_domain_TestBindingRecord_Fulfillment[Fulfillment]
    T31_domain_domain_TestBindingRecord_Waiver[Waiver]
    T31_domain_domain_TestBindingRecord_VoluntaryBinding[VoluntaryBinding]
  end
  subgraph T34_domain_domain_TestBindingsDocument["tddd::test_obligation::binding::TestBindingsDocument"]
    direction TB
    T34_domain_domain_TestBindingsDocument__self[TestBindingsDocument]
    T34_domain_domain_TestBindingsDocument_new([new])
    T34_domain_domain_TestBindingsDocument_track_id([track_id])
    T34_domain_domain_TestBindingsDocument_records([records])
  end
  subgraph T30_domain_domain_TestBodySpanHash["tddd::test_obligation::hashes::TestBodySpanHash"]
    direction TB
    T30_domain_domain_TestBodySpanHash__self[TestBodySpanHash]
    T30_domain_domain_TestBodySpanHash_new([new])
    T30_domain_domain_TestBodySpanHash_as_hash([as_hash])
  end
  subgraph T30_domain_domain_TestFunctionName["tddd::test_obligation::ids::TestFunctionName"]
    direction TB
    T30_domain_domain_TestFunctionName__self[TestFunctionName]
    T30_domain_domain_TestFunctionName_try_new([try_new])
    T30_domain_domain_TestFunctionName_as_str([as_str])
  end
  subgraph T26_domain_domain_TestLocation["tddd::test_obligation::binding::TestLocation"]
    direction TB
    T26_domain_domain_TestLocation__self[TestLocation]
    T26_domain_domain_TestLocation_new([new])
    T26_domain_domain_TestLocation_layer([layer])
    T26_domain_domain_TestLocation_module_path([module_path])
    T26_domain_domain_TestLocation_test_name([test_name])
  end
  subgraph T28_domain_domain_TestModulePath["tddd::test_obligation::ids::TestModulePath"]
    direction TB
    T28_domain_domain_TestModulePath__self[TestModulePath]
    T28_domain_domain_TestModulePath_try_new([try_new])
    T28_domain_domain_TestModulePath_as_str([as_str])
  end
  subgraph T28_domain_domain_TestObligation["tddd::test_obligation::obligations::TestObligation"]
    direction TB
    T28_domain_domain_TestObligation__self[TestObligation]
    T28_domain_domain_TestObligation_new([new])
    T28_domain_domain_TestObligation_id([id])
    T28_domain_domain_TestObligation_target_entry([target_entry])
    T28_domain_domain_TestObligation_target_role([target_role])
    T28_domain_domain_TestObligation_brief([brief])
    T28_domain_domain_TestObligation_declaration_hash([declaration_hash])
    T28_domain_domain_TestObligation_spec_refs([spec_refs])
  end
  subgraph T36_domain_domain_TestObligationAnchorId["tddd::test_obligation::ids::TestObligationAnchorId"]
    direction TB
    T36_domain_domain_TestObligationAnchorId__self[TestObligationAnchorId]
    T36_domain_domain_TestObligationAnchorId_try_new([try_new])
    T36_domain_domain_TestObligationAnchorId_file_path([file_path])
    T36_domain_domain_TestObligationAnchorId_element_id([element_id])
  end
  subgraph T33_domain_domain_TestObligationBrief["tddd::test_obligation::ids::TestObligationBrief"]
    direction TB
    T33_domain_domain_TestObligationBrief__self[TestObligationBrief]
    T33_domain_domain_TestObligationBrief_try_new([try_new])
    T33_domain_domain_TestObligationBrief_as_str([as_str])
  end
  subgraph T41_domain_domain_TestObligationBriefTemplate["tddd::test_obligation::rules::TestObligationBriefTemplate"]
    direction TB
    T41_domain_domain_TestObligationBriefTemplate__self[TestObligationBriefTemplate]
    T41_domain_domain_TestObligationBriefTemplate_try_new([try_new])
    T41_domain_domain_TestObligationBriefTemplate_as_str([as_str])
  end
  subgraph T33_domain_domain_TestObligationDrift["tddd::test_obligation::drift::TestObligationDrift"]
    direction TB
    T33_domain_domain_TestObligationDrift__self[TestObligationDrift]
    T33_domain_domain_TestObligationDrift_missing_obligation([missing_obligation])
    T33_domain_domain_TestObligationDrift_orphaned_edge([orphaned_edge])
    T33_domain_domain_TestObligationDrift_spec_changed_edge([spec_changed_edge])
    T33_domain_domain_TestObligationDrift_decl_changed_edge([decl_changed_edge])
    T33_domain_domain_TestObligationDrift_test_changed_edge([test_changed_edge])
    T33_domain_domain_TestObligationDrift_reason_changed_edge([reason_changed_edge])
  end
  subgraph T37_domain_domain_TestObligationDriftKind["tddd::test_obligation::vocab::TestObligationDriftKind"]
    direction TB
    T37_domain_domain_TestObligationDriftKind__self[TestObligationDriftKind]
    T37_domain_domain_TestObligationDriftKind_Missing[Missing]
    T37_domain_domain_TestObligationDriftKind_Orphaned[Orphaned]
    T37_domain_domain_TestObligationDriftKind_SpecChanged[SpecChanged]
    T37_domain_domain_TestObligationDriftKind_DeclChanged[DeclChanged]
    T37_domain_domain_TestObligationDriftKind_TestChanged[TestChanged]
    T37_domain_domain_TestObligationDriftKind_ReasonChanged[ReasonChanged]
    T37_domain_domain_TestObligationDriftKind_as_kebab([as_kebab])
    T37_domain_domain_TestObligationDriftKind_is_existence([is_existence])
    T37_domain_domain_TestObligationDriftKind_is_freshness([is_freshness])
  end
  subgraph T34_domain_domain_TestObligationEdgeId["tddd::test_obligation::ids::TestObligationEdgeId"]
    direction TB
    T34_domain_domain_TestObligationEdgeId__self[TestObligationEdgeId]
    T34_domain_domain_TestObligationEdgeId_new([new])
    T34_domain_domain_TestObligationEdgeId_entry_key([entry_key])
    T34_domain_domain_TestObligationEdgeId_anchor_id([anchor_id])
  end
  subgraph T30_domain_domain_TestObligationId["tddd::test_obligation::ids::TestObligationId"]
    direction TB
    T30_domain_domain_TestObligationId__self[TestObligationId]
    T30_domain_domain_TestObligationId_new([new])
    T30_domain_domain_TestObligationId_entry_key([entry_key])
    T30_domain_domain_TestObligationId_obligation_kind([obligation_kind])
    T30_domain_domain_TestObligationId_item_identifier([item_identifier])
  end
  subgraph T42_domain_domain_TestObligationItemIdentifier["tddd::test_obligation::ids::TestObligationItemIdentifier"]
    direction TB
    T42_domain_domain_TestObligationItemIdentifier__self[TestObligationItemIdentifier]
    T42_domain_domain_TestObligationItemIdentifier_try_new([try_new])
    T42_domain_domain_TestObligationItemIdentifier_as_str([as_str])
  end
  subgraph T32_domain_domain_TestObligationKind["tddd::test_obligation::vocab::TestObligationKind"]
    direction TB
    T32_domain_domain_TestObligationKind__self[TestObligationKind]
    T32_domain_domain_TestObligationKind_Boundary[Boundary]
    T32_domain_domain_TestObligationKind_InvariantPreservation[InvariantPreservation]
    T32_domain_domain_TestObligationKind_EventEmission[EventEmission]
    T32_domain_domain_TestObligationKind_LogicResult[LogicResult]
    T32_domain_domain_TestObligationKind_PredicateBothBranches[PredicateBothBranches]
    T32_domain_domain_TestObligationKind_ConstructionResult[ConstructionResult]
    T32_domain_domain_TestObligationKind_Result[Result]
    T32_domain_domain_TestObligationKind_Reaction[Reaction]
    T32_domain_domain_TestObligationKind_Transition[Transition]
    T32_domain_domain_TestObligationKind_Contract[Contract]
    T32_domain_domain_TestObligationKind_ContractConformance[ContractConformance]
    T32_domain_domain_TestObligationKind_Logic[Logic]
    T32_domain_domain_TestObligationKind_as_kebab([as_kebab])
  end
  subgraph T35_domain_domain_TestObligationMinimum["tddd::test_obligation::rules::TestObligationMinimum"]
    direction TB
    T35_domain_domain_TestObligationMinimum__self[TestObligationMinimum]
    T35_domain_domain_TestObligationMinimum_try_new([try_new])
    T35_domain_domain_TestObligationMinimum_as_usize([as_usize])
  end
  subgraph T39_domain_domain_TestObligationPatternKind["tddd::test_obligation::vocab::TestObligationPatternKind"]
    direction TB
    T39_domain_domain_TestObligationPatternKind__self[TestObligationPatternKind]
    T39_domain_domain_TestObligationPatternKind_Typestate[Typestate]
  end
  subgraph T35_domain_domain_TestObligationPerAxis["tddd::test_obligation::vocab::TestObligationPerAxis"]
    direction TB
    T35_domain_domain_TestObligationPerAxis__self[TestObligationPerAxis]
    T35_domain_domain_TestObligationPerAxis_Invariant[Invariant]
    T35_domain_domain_TestObligationPerAxis_Method[Method]
    T35_domain_domain_TestObligationPerAxis_Handles[Handles]
    T35_domain_domain_TestObligationPerAxis_ReactsTo[ReactsTo]
    T35_domain_domain_TestObligationPerAxis_Transition[Transition]
    T35_domain_domain_TestObligationPerAxis_TraitMethod[TraitMethod]
    T35_domain_domain_TestObligationPerAxis_Entry[Entry]
    T35_domain_domain_TestObligationPerAxis_Emits[Emits]
    T35_domain_domain_TestObligationPerAxis_TraitImpl[TraitImpl]
    T35_domain_domain_TestObligationPerAxis_as_kebab([as_kebab])
  end
  subgraph T32_domain_domain_TestObligationRule["tddd::test_obligation::rules::TestObligationRule"]
    direction TB
    T32_domain_domain_TestObligationRule__self[TestObligationRule]
    T32_domain_domain_TestObligationRule_new([new])
    T32_domain_domain_TestObligationRule_kind([kind])
    T32_domain_domain_TestObligationRule_per_axis([per_axis])
    T32_domain_domain_TestObligationRule_minimum([minimum])
  end
  subgraph T41_domain_domain_TestObligationRulesDocument["tddd::test_obligation::rules::TestObligationRulesDocument"]
    direction TB
    T41_domain_domain_TestObligationRulesDocument__self[TestObligationRulesDocument]
    T41_domain_domain_TestObligationRulesDocument_try_new([try_new])
    T41_domain_domain_TestObligationRulesDocument_data_roles([data_roles])
    T41_domain_domain_TestObligationRulesDocument_contract_roles([contract_roles])
    T41_domain_domain_TestObligationRulesDocument_function_roles([function_roles])
    T41_domain_domain_TestObligationRulesDocument_patterns([patterns])
    T41_domain_domain_TestObligationRulesDocument_trait_impls([trait_impls])
  end
  subgraph T42_domain_domain_TestObligationRulesLoadError["tddd::test_obligation::errors::TestObligationRulesLoadError"]
    direction TB
    T42_domain_domain_TestObligationRulesLoadError__self[TestObligationRulesLoadError]
    T42_domain_domain_TestObligationRulesLoadError_RoleNotCovered[RoleNotCovered]
    T42_domain_domain_TestObligationRulesLoadError_ObligationsFieldOmitted[ObligationsFieldOmitted]
    T42_domain_domain_TestObligationRulesLoadError_UnknownRoleName[UnknownRoleName]
    T42_domain_domain_TestObligationRulesLoadError_InvalidRuleValue[InvalidRuleValue]
    T42_domain_domain_TestObligationRulesLoadError_IoError[IoError]
    T42_domain_domain_TestObligationRulesLoadError_MalformedJson[MalformedJson]
  end
  subgraph T41_domain_domain_TestObligationScopePresence["tddd::test_obligation::scope::TestObligationScopePresence"]
    direction TB
    T41_domain_domain_TestObligationScopePresence__self[TestObligationScopePresence]
    T41_domain_domain_TestObligationScopePresence_Both[Both]
    T41_domain_domain_TestObligationScopePresence_Neither[Neither]
    T41_domain_domain_TestObligationScopePresence_ObligationsOnly[ObligationsOnly]
    T41_domain_domain_TestObligationScopePresence_BindingsOnly[BindingsOnly]
  end
  subgraph T33_domain_domain_TestSourceScanError["tddd::test_obligation::errors::TestSourceScanError"]
    direction TB
    T33_domain_domain_TestSourceScanError__self[TestSourceScanError]
    T33_domain_domain_TestSourceScanError_Io[Io]
    T33_domain_domain_TestSourceScanError_Parse[Parse]
  end
  subgraph T39_domain_domain_UncitedSpecElementFinding["tddd::test_obligation::scope::UncitedSpecElementFinding"]
    direction TB
    T39_domain_domain_UncitedSpecElementFinding__self[UncitedSpecElementFinding]
    T39_domain_domain_UncitedSpecElementFinding_new([new])
  end
  subgraph T30_domain_domain_VerifyCacheError["tddd::test_obligation::errors::VerifyCacheError"]
    direction TB
    T30_domain_domain_VerifyCacheError__self[VerifyCacheError]
    T30_domain_domain_VerifyCacheError_Io[Io]
    T30_domain_domain_VerifyCacheError_MalformedJson[MalformedJson]
  end
  subgraph T26_domain_domain_WaivedReason["tddd::test_obligation::ids::WaivedReason"]
    direction TB
    T26_domain_domain_WaivedReason__self[WaivedReason]
    T26_domain_domain_WaivedReason_try_new([try_new])
    T26_domain_domain_WaivedReason_as_str([as_str])
  end
  subgraph T30_domain_domain_WaivedReasonHash["tddd::test_obligation::hashes::WaivedReasonHash"]
    direction TB
    T30_domain_domain_WaivedReasonHash__self[WaivedReasonHash]
    T30_domain_domain_WaivedReasonHash_new([new])
    T30_domain_domain_WaivedReasonHash_as_hash([as_hash])
  end
  subgraph T33_domain_domain_WaiverCacheDocument["tddd::test_obligation::verdict::WaiverCacheDocument"]
    direction TB
    T33_domain_domain_WaiverCacheDocument__self[WaiverCacheDocument]
    T33_domain_domain_WaiverCacheDocument_new([new])
    T33_domain_domain_WaiverCacheDocument_entries([entries])
  end
  subgraph T30_domain_domain_WaiverCacheEntry["tddd::test_obligation::verdict::WaiverCacheEntry"]
    direction TB
    T30_domain_domain_WaiverCacheEntry__self[WaiverCacheEntry]
    T30_domain_domain_WaiverCacheEntry_new([new])
  end
  subgraph T28_domain_domain_WaiverCacheKey["tddd::test_obligation::verdict::WaiverCacheKey"]
    direction TB
    T28_domain_domain_WaiverCacheKey__self[WaiverCacheKey]
    T28_domain_domain_WaiverCacheKey_new([new])
  end
  subgraph T27_domain_domain_WaiverVerdict["tddd::test_obligation::verdict::WaiverVerdict"]
    direction TB
    T27_domain_domain_WaiverVerdict__self[WaiverVerdict]
    T27_domain_domain_WaiverVerdict_Waived[Waived]
    T27_domain_domain_WaiverVerdict_Fail[Fail]
    T27_domain_domain_WaiverVerdict_Pending[Pending]
  end
  subgraph R44_domain_domain_ObligationFulfillmentCachePort["tddd::test_obligation::ports::ObligationFulfillmentCachePort"]
    direction TB
    R44_domain_domain_ObligationFulfillmentCachePort__self[ObligationFulfillmentCachePort]
    R44_domain_domain_ObligationFulfillmentCachePort_load([load])
    R44_domain_domain_ObligationFulfillmentCachePort_save([save])
  end
  subgraph R47_domain_domain_ObligationFulfillmentVerifierPort["tddd::test_obligation::ports::ObligationFulfillmentVerifierPort"]
    direction TB
    R47_domain_domain_ObligationFulfillmentVerifierPort__self[ObligationFulfillmentVerifierPort]
    R47_domain_domain_ObligationFulfillmentVerifierPort_verify_pair([verify_pair])
  end
  subgraph R37_domain_domain_ObligationsArtifactPort["tddd::test_obligation::ports::ObligationsArtifactPort"]
    direction TB
    R37_domain_domain_ObligationsArtifactPort__self[ObligationsArtifactPort]
    R37_domain_domain_ObligationsArtifactPort_load([load])
    R37_domain_domain_ObligationsArtifactPort_save([save])
  end
  subgraph R38_domain_domain_TestBindingsArtifactPort["tddd::test_obligation::ports::TestBindingsArtifactPort"]
    direction TB
    R38_domain_domain_TestBindingsArtifactPort__self[TestBindingsArtifactPort]
    R38_domain_domain_TestBindingsArtifactPort_load([load])
    R38_domain_domain_TestBindingsArtifactPort_save([save])
  end
  subgraph R43_domain_domain_TestObligationRulesLoaderPort["tddd::test_obligation::ports::TestObligationRulesLoaderPort"]
    direction TB
    R43_domain_domain_TestObligationRulesLoaderPort__self[TestObligationRulesLoaderPort]
    R43_domain_domain_TestObligationRulesLoaderPort_load([load])
  end
  subgraph R35_domain_domain_TestSourceScannerPort["tddd::test_obligation::ports::TestSourceScannerPort"]
    direction TB
    R35_domain_domain_TestSourceScannerPort__self[TestSourceScannerPort]
    R35_domain_domain_TestSourceScannerPort_scan_test_body([scan_test_body])
    R35_domain_domain_TestSourceScannerPort_hash_test_body([hash_test_body])
  end
  subgraph R29_domain_domain_WaiverCachePort["tddd::test_obligation::ports::WaiverCachePort"]
    direction TB
    R29_domain_domain_WaiverCachePort__self[WaiverCachePort]
    R29_domain_domain_WaiverCachePort_load([load])
    R29_domain_domain_WaiverCachePort_save([save])
  end
  subgraph R32_domain_domain_WaiverVerifierPort["tddd::test_obligation::ports::WaiverVerifierPort"]
    direction TB
    R32_domain_domain_WaiverVerifierPort__self[WaiverVerifierPort]
    R32_domain_domain_WaiverVerifierPort_verify_pair([verify_pair])
  end
  end
end
subgraph usecase["usecase"]
  direction TB
  subgraph usecase_usecase_module_semantic_verdict_core["usecase::semantic_verdict_core"]
    direction TB
  subgraph T46_usecase_usecase_SemanticCalibrationProbeConfig["semantic_verdict_core::probe::SemanticCalibrationProbeConfig"]
    direction TB
    T46_usecase_usecase_SemanticCalibrationProbeConfig__self[SemanticCalibrationProbeConfig]
    T46_usecase_usecase_SemanticCalibrationProbeConfig_new([new])
  end
  subgraph R44_usecase_usecase_SemanticEscalationDriverPort["semantic_verdict_core::driver::SemanticEscalationDriverPort"]
    direction TB
    R44_usecase_usecase_SemanticEscalationDriverPort__self[SemanticEscalationDriverPort]
    R44_usecase_usecase_SemanticEscalationDriverPort_evaluate_with_escalation([evaluate_with_escalation])
  end
  subgraph R47_usecase_usecase_SemanticEscalationVerdictBridge["semantic_verdict_core::verdict::SemanticEscalationVerdictBridge"]
    direction TB
    R47_usecase_usecase_SemanticEscalationVerdictBridge__self[SemanticEscalationVerdictBridge]
    R47_usecase_usecase_SemanticEscalationVerdictBridge_project([project])
  end
  end
  subgraph usecase_usecase_module_test_obligation["usecase::test_obligation"]
    direction TB
  subgraph T43_usecase_usecase_CheckTestObligationsCommand["test_obligation::check::CheckTestObligationsCommand"]
    direction TB
    T43_usecase_usecase_CheckTestObligationsCommand__self[CheckTestObligationsCommand]
    T43_usecase_usecase_CheckTestObligationsCommand_new([new])
  end
  subgraph T46_usecase_usecase_CheckTestObligationsInteractor["test_obligation::check::CheckTestObligationsInteractor"]
    direction TB
    T46_usecase_usecase_CheckTestObligationsInteractor__self[CheckTestObligationsInteractor]
    T46_usecase_usecase_CheckTestObligationsInteractor_new([new])
  end
  subgraph T43_usecase_usecase_CheckTestObligationsOutcome["test_obligation::check::CheckTestObligationsOutcome"]
    direction TB
    T43_usecase_usecase_CheckTestObligationsOutcome__self[CheckTestObligationsOutcome]
    T43_usecase_usecase_CheckTestObligationsOutcome_new_verified_scope([new_verified_scope])
    T43_usecase_usecase_CheckTestObligationsOutcome_new_empty_scope([new_empty_scope])
  end
  subgraph T44_usecase_usecase_DeriveTestObligationsCommand["test_obligation::derive::DeriveTestObligationsCommand"]
    direction TB
    T44_usecase_usecase_DeriveTestObligationsCommand__self[DeriveTestObligationsCommand]
    T44_usecase_usecase_DeriveTestObligationsCommand_new([new])
  end
  subgraph T47_usecase_usecase_DeriveTestObligationsInteractor["test_obligation::derive::DeriveTestObligationsInteractor"]
    direction TB
    T47_usecase_usecase_DeriveTestObligationsInteractor__self[DeriveTestObligationsInteractor]
    T47_usecase_usecase_DeriveTestObligationsInteractor_new([new])
  end
  subgraph T46_usecase_usecase_EvaluateTestObligationsCommand["test_obligation::evaluate::EvaluateTestObligationsCommand"]
    direction TB
    T46_usecase_usecase_EvaluateTestObligationsCommand__self[EvaluateTestObligationsCommand]
    T46_usecase_usecase_EvaluateTestObligationsCommand_new([new])
  end
  subgraph T49_usecase_usecase_EvaluateTestObligationsInteractor["test_obligation::evaluate::EvaluateTestObligationsInteractor"]
    direction TB
    T49_usecase_usecase_EvaluateTestObligationsInteractor__self[EvaluateTestObligationsInteractor]
    T49_usecase_usecase_EvaluateTestObligationsInteractor_new([new])
  end
  subgraph T46_usecase_usecase_EvaluateTestObligationsOutcome["test_obligation::evaluate::EvaluateTestObligationsOutcome"]
    direction TB
    T46_usecase_usecase_EvaluateTestObligationsOutcome__self[EvaluateTestObligationsOutcome]
    T46_usecase_usecase_EvaluateTestObligationsOutcome_new([new])
  end
  subgraph T40_usecase_usecase_TestObligationChainLabel["test_obligation::results::TestObligationChainLabel"]
    direction TB
    T40_usecase_usecase_TestObligationChainLabel__self[TestObligationChainLabel]
    T40_usecase_usecase_TestObligationChainLabel_Fulfillment[Fulfillment]
    T40_usecase_usecase_TestObligationChainLabel_Waiver[Waiver]
  end
  subgraph T44_usecase_usecase_TestObligationEvaluateConfig["test_obligation::evaluate::TestObligationEvaluateConfig"]
    direction TB
    T44_usecase_usecase_TestObligationEvaluateConfig__self[TestObligationEvaluateConfig]
    T44_usecase_usecase_TestObligationEvaluateConfig_try_new([try_new])
  end
  subgraph T41_usecase_usecase_TestObligationLaneSummary["test_obligation::results::TestObligationLaneSummary"]
    direction TB
    T41_usecase_usecase_TestObligationLaneSummary__self[TestObligationLaneSummary]
    T41_usecase_usecase_TestObligationLaneSummary_new([new])
  end
  subgraph T44_usecase_usecase_TestObligationResultsCommand["test_obligation::results::TestObligationResultsCommand"]
    direction TB
    T44_usecase_usecase_TestObligationResultsCommand__self[TestObligationResultsCommand]
    T44_usecase_usecase_TestObligationResultsCommand_new([new])
  end
  subgraph T47_usecase_usecase_TestObligationResultsInteractor["test_obligation::results::TestObligationResultsInteractor"]
    direction TB
    T47_usecase_usecase_TestObligationResultsInteractor__self[TestObligationResultsInteractor]
    T47_usecase_usecase_TestObligationResultsInteractor_new([new])
  end
  subgraph T43_usecase_usecase_TestObligationResultsOutput["test_obligation::results::TestObligationResultsOutput"]
    direction TB
    T43_usecase_usecase_TestObligationResultsOutput__self[TestObligationResultsOutput]
    T43_usecase_usecase_TestObligationResultsOutput_new([new])
  end
  subgraph R54_usecase_usecase_CheckTestObligationsApplicationService["test_obligation::check::CheckTestObligationsApplicationService"]
    direction TB
    R54_usecase_usecase_CheckTestObligationsApplicationService__self[CheckTestObligationsApplicationService]
    R54_usecase_usecase_CheckTestObligationsApplicationService_execute([execute])
  end
  subgraph R55_usecase_usecase_DeriveTestObligationsApplicationService["test_obligation::derive::DeriveTestObligationsApplicationService"]
    direction TB
    R55_usecase_usecase_DeriveTestObligationsApplicationService__self[DeriveTestObligationsApplicationService]
    R55_usecase_usecase_DeriveTestObligationsApplicationService_execute([execute])
  end
  subgraph R57_usecase_usecase_EvaluateTestObligationsApplicationService["test_obligation::evaluate::EvaluateTestObligationsApplicationService"]
    direction TB
    R57_usecase_usecase_EvaluateTestObligationsApplicationService__self[EvaluateTestObligationsApplicationService]
    R57_usecase_usecase_EvaluateTestObligationsApplicationService_execute([execute])
  end
  subgraph R55_usecase_usecase_TestObligationResultsApplicationService["test_obligation::results::TestObligationResultsApplicationService"]
    direction TB
    R55_usecase_usecase_TestObligationResultsApplicationService__self[TestObligationResultsApplicationService]
    R55_usecase_usecase_TestObligationResultsApplicationService_execute([execute])
  end
  end
end
subgraph infrastructure["infrastructure"]
  direction TB
  subgraph infrastructure_infrastructure_module_test_obligation["infrastructure::test_obligation"]
    direction TB
  subgraph T50_infrastructure_infrastructure_CatalogueEntryRefDto["test_obligation::obligations_codec::CatalogueEntryRefDto"]
    direction TB
    T50_infrastructure_infrastructure_CatalogueEntryRefDto__self[CatalogueEntryRefDto]
  end
  subgraph T45_infrastructure_infrastructure_ContractRoleKey["test_obligation::rules_codec::ContractRoleKey"]
    direction TB
    T45_infrastructure_infrastructure_ContractRoleKey__self[ContractRoleKey]
    T45_infrastructure_infrastructure_ContractRoleKey_SecondaryPort[SecondaryPort]
    T45_infrastructure_infrastructure_ContractRoleKey_SpecificationPort[SpecificationPort]
    T45_infrastructure_infrastructure_ContractRoleKey_Repository[Repository]
    T45_infrastructure_infrastructure_ContractRoleKey_ApplicationService[ApplicationService]
  end
  subgraph T41_infrastructure_infrastructure_DataRoleKey["test_obligation::rules_codec::DataRoleKey"]
    direction TB
    T41_infrastructure_infrastructure_DataRoleKey__self[DataRoleKey]
    T41_infrastructure_infrastructure_DataRoleKey_ValueObject[ValueObject]
    T41_infrastructure_infrastructure_DataRoleKey_Entity[Entity]
    T41_infrastructure_infrastructure_DataRoleKey_AggregateRoot[AggregateRoot]
    T41_infrastructure_infrastructure_DataRoleKey_DomainService[DomainService]
    T41_infrastructure_infrastructure_DataRoleKey_UseCase[UseCase]
    T41_infrastructure_infrastructure_DataRoleKey_EventPolicy[EventPolicy]
    T41_infrastructure_infrastructure_DataRoleKey_DomainEvent[DomainEvent]
    T41_infrastructure_infrastructure_DataRoleKey_Specification[Specification]
    T41_infrastructure_infrastructure_DataRoleKey_Factory[Factory]
    T41_infrastructure_infrastructure_DataRoleKey_Interactor[Interactor]
    T41_infrastructure_infrastructure_DataRoleKey_Command[Command]
    T41_infrastructure_infrastructure_DataRoleKey_Query[Query]
    T41_infrastructure_infrastructure_DataRoleKey_Dto[Dto]
    T41_infrastructure_infrastructure_DataRoleKey_ErrorType[ErrorType]
    T41_infrastructure_infrastructure_DataRoleKey_SecondaryAdapter[SecondaryAdapter]
    T41_infrastructure_infrastructure_DataRoleKey_CompositionRoot[CompositionRoot]
    T41_infrastructure_infrastructure_DataRoleKey_PrimaryAdapter[PrimaryAdapter]
  end
  subgraph T56_infrastructure_infrastructure_FulfillmentFailCategoryDto["test_obligation::fulfillment_cache_codec::FulfillmentFailCategoryDto"]
    direction TB
    T56_infrastructure_infrastructure_FulfillmentFailCategoryDto__self[FulfillmentFailCategoryDto]
    T56_infrastructure_infrastructure_FulfillmentFailCategoryDto_Contradiction[Contradiction]
    T56_infrastructure_infrastructure_FulfillmentFailCategoryDto_Substitution[Substitution]
    T56_infrastructure_infrastructure_FulfillmentFailCategoryDto_CentralUnverified[CentralUnverified]
  end
  subgraph T45_infrastructure_infrastructure_FunctionRoleKey["test_obligation::rules_codec::FunctionRoleKey"]
    direction TB
    T45_infrastructure_infrastructure_FunctionRoleKey__self[FunctionRoleKey]
    T45_infrastructure_infrastructure_FunctionRoleKey_UseCaseFunction[UseCaseFunction]
    T45_infrastructure_infrastructure_FunctionRoleKey_FreeFunction[FreeFunction]
  end
  subgraph T65_infrastructure_infrastructure_JsonObligationFulfillmentCacheCodec["test_obligation::fulfillment_cache_codec::JsonObligationFulfillmentCacheCodec"]
    direction TB
    T65_infrastructure_infrastructure_JsonObligationFulfillmentCacheCodec__self[JsonObligationFulfillmentCacheCodec]
    T65_infrastructure_infrastructure_JsonObligationFulfillmentCacheCodec_new([new])
  end
  subgraph T50_infrastructure_infrastructure_JsonObligationsCodec["test_obligation::obligations_codec::JsonObligationsCodec"]
    direction TB
    T50_infrastructure_infrastructure_JsonObligationsCodec__self[JsonObligationsCodec]
    T50_infrastructure_infrastructure_JsonObligationsCodec_new([new])
  end
  subgraph T51_infrastructure_infrastructure_JsonTestBindingsCodec["test_obligation::bindings_codec::JsonTestBindingsCodec"]
    direction TB
    T51_infrastructure_infrastructure_JsonTestBindingsCodec__self[JsonTestBindingsCodec]
    T51_infrastructure_infrastructure_JsonTestBindingsCodec_new([new])
  end
  subgraph T59_infrastructure_infrastructure_JsonTestObligationRulesLoader["test_obligation::rules_codec::JsonTestObligationRulesLoader"]
    direction TB
    T59_infrastructure_infrastructure_JsonTestObligationRulesLoader__self[JsonTestObligationRulesLoader]
    T59_infrastructure_infrastructure_JsonTestObligationRulesLoader_new([new])
  end
  subgraph T50_infrastructure_infrastructure_JsonWaiverCacheCodec["test_obligation::waiver_cache_codec::JsonWaiverCacheCodec"]
    direction TB
    T50_infrastructure_infrastructure_JsonWaiverCacheCodec__self[JsonWaiverCacheCodec]
    T50_infrastructure_infrastructure_JsonWaiverCacheCodec_new([new])
  end
  subgraph T67_infrastructure_infrastructure_ObligationFulfillmentCacheDocumentDto["test_obligation::fulfillment_cache_codec::ObligationFulfillmentCacheDocumentDto"]
    direction TB
    T67_infrastructure_infrastructure_ObligationFulfillmentCacheDocumentDto__self[ObligationFulfillmentCacheDocumentDto]
  end
  subgraph T64_infrastructure_infrastructure_ObligationFulfillmentCacheEntryDto["test_obligation::fulfillment_cache_codec::ObligationFulfillmentCacheEntryDto"]
    direction TB
    T64_infrastructure_infrastructure_ObligationFulfillmentCacheEntryDto__self[ObligationFulfillmentCacheEntryDto]
  end
  subgraph T61_infrastructure_infrastructure_ObligationFulfillmentVerdictDto["test_obligation::fulfillment_cache_codec::ObligationFulfillmentVerdictDto"]
    direction TB
    T61_infrastructure_infrastructure_ObligationFulfillmentVerdictDto__self[ObligationFulfillmentVerdictDto]
    T61_infrastructure_infrastructure_ObligationFulfillmentVerdictDto_Fulfilled[Fulfilled]
    T61_infrastructure_infrastructure_ObligationFulfillmentVerdictDto_Fail[Fail]
    T61_infrastructure_infrastructure_ObligationFulfillmentVerdictDto_Pending[Pending]
  end
  subgraph T66_infrastructure_infrastructure_ObligationFulfillmentVerifierAdapter["test_obligation::fulfillment_verifier::ObligationFulfillmentVerifierAdapter"]
    direction TB
    T66_infrastructure_infrastructure_ObligationFulfillmentVerifierAdapter__self[ObligationFulfillmentVerifierAdapter]
    T66_infrastructure_infrastructure_ObligationFulfillmentVerifierAdapter_new([new])
  end
  subgraph T51_infrastructure_infrastructure_ObligationsCodecError["test_obligation::obligations_codec::ObligationsCodecError"]
    direction TB
    T51_infrastructure_infrastructure_ObligationsCodecError__self[ObligationsCodecError]
  end
  subgraph T52_infrastructure_infrastructure_ObligationsDocumentDto["test_obligation::obligations_codec::ObligationsDocumentDto"]
    direction TB
    T52_infrastructure_infrastructure_ObligationsDocumentDto__self[ObligationsDocumentDto]
    T52_infrastructure_infrastructure_ObligationsDocumentDto_from_domain([from_domain])
    T52_infrastructure_infrastructure_ObligationsDocumentDto_into_domain([into_domain])
  end
  subgraph T40_infrastructure_infrastructure_PatternKey["test_obligation::rules_codec::PatternKey"]
    direction TB
    T40_infrastructure_infrastructure_PatternKey__self[PatternKey]
    T40_infrastructure_infrastructure_PatternKey_Typestate[Typestate]
  end
  subgraph T52_infrastructure_infrastructure_RoleObligationRulesDto["test_obligation::rules_codec::RoleObligationRulesDto"]
    direction TB
    T52_infrastructure_infrastructure_RoleObligationRulesDto__self[RoleObligationRulesDto]
  end
  subgraph T50_infrastructure_infrastructure_SynTestSourceScanner["test_obligation::source_scanner::SynTestSourceScanner"]
    direction TB
    T50_infrastructure_infrastructure_SynTestSourceScanner__self[SynTestSourceScanner]
    T50_infrastructure_infrastructure_SynTestSourceScanner_new([new])
  end
  subgraph T50_infrastructure_infrastructure_TestBindingRecordDto["test_obligation::bindings_codec::TestBindingRecordDto"]
    direction TB
    T50_infrastructure_infrastructure_TestBindingRecordDto__self[TestBindingRecordDto]
    T50_infrastructure_infrastructure_TestBindingRecordDto_Fulfillment[Fulfillment]
    T50_infrastructure_infrastructure_TestBindingRecordDto_Waiver[Waiver]
    T50_infrastructure_infrastructure_TestBindingRecordDto_VoluntaryBinding[VoluntaryBinding]
  end
  subgraph T52_infrastructure_infrastructure_TestBindingsCodecError["test_obligation::bindings_codec::TestBindingsCodecError"]
    direction TB
    T52_infrastructure_infrastructure_TestBindingsCodecError__self[TestBindingsCodecError]
  end
  subgraph T53_infrastructure_infrastructure_TestBindingsDocumentDto["test_obligation::bindings_codec::TestBindingsDocumentDto"]
    direction TB
    T53_infrastructure_infrastructure_TestBindingsDocumentDto__self[TestBindingsDocumentDto]
    T53_infrastructure_infrastructure_TestBindingsDocumentDto_from_domain([from_domain])
    T53_infrastructure_infrastructure_TestBindingsDocumentDto_into_domain([into_domain])
  end
  subgraph T45_infrastructure_infrastructure_TestLocationDto["test_obligation::bindings_codec::TestLocationDto"]
    direction TB
    T45_infrastructure_infrastructure_TestLocationDto__self[TestLocationDto]
  end
  subgraph T55_infrastructure_infrastructure_TestObligationAnchorIdDto["test_obligation::obligations_codec::TestObligationAnchorIdDto"]
    direction TB
    T55_infrastructure_infrastructure_TestObligationAnchorIdDto__self[TestObligationAnchorIdDto]
  end
  subgraph T47_infrastructure_infrastructure_TestObligationDto["test_obligation::obligations_codec::TestObligationDto"]
    direction TB
    T47_infrastructure_infrastructure_TestObligationDto__self[TestObligationDto]
  end
  subgraph T53_infrastructure_infrastructure_TestObligationEdgeIdDto["test_obligation::bindings_codec::TestObligationEdgeIdDto"]
    direction TB
    T53_infrastructure_infrastructure_TestObligationEdgeIdDto__self[TestObligationEdgeIdDto]
  end
  subgraph T49_infrastructure_infrastructure_TestObligationIdDto["test_obligation::obligations_codec::TestObligationIdDto"]
    direction TB
    T49_infrastructure_infrastructure_TestObligationIdDto__self[TestObligationIdDto]
  end
  subgraph T51_infrastructure_infrastructure_TestObligationKindDto["test_obligation::obligations_codec::TestObligationKindDto"]
    direction TB
    T51_infrastructure_infrastructure_TestObligationKindDto__self[TestObligationKindDto]
    T51_infrastructure_infrastructure_TestObligationKindDto_Boundary[Boundary]
    T51_infrastructure_infrastructure_TestObligationKindDto_InvariantPreservation[InvariantPreservation]
    T51_infrastructure_infrastructure_TestObligationKindDto_EventEmission[EventEmission]
    T51_infrastructure_infrastructure_TestObligationKindDto_LogicResult[LogicResult]
    T51_infrastructure_infrastructure_TestObligationKindDto_PredicateBothBranches[PredicateBothBranches]
    T51_infrastructure_infrastructure_TestObligationKindDto_ConstructionResult[ConstructionResult]
    T51_infrastructure_infrastructure_TestObligationKindDto_Result[Result]
    T51_infrastructure_infrastructure_TestObligationKindDto_Reaction[Reaction]
    T51_infrastructure_infrastructure_TestObligationKindDto_Transition[Transition]
    T51_infrastructure_infrastructure_TestObligationKindDto_Contract[Contract]
    T51_infrastructure_infrastructure_TestObligationKindDto_ContractConformance[ContractConformance]
    T51_infrastructure_infrastructure_TestObligationKindDto_Logic[Logic]
  end
  subgraph T54_infrastructure_infrastructure_TestObligationPerAxisDto["test_obligation::rules_codec::TestObligationPerAxisDto"]
    direction TB
    T54_infrastructure_infrastructure_TestObligationPerAxisDto__self[TestObligationPerAxisDto]
    T54_infrastructure_infrastructure_TestObligationPerAxisDto_Invariant[Invariant]
    T54_infrastructure_infrastructure_TestObligationPerAxisDto_Method[Method]
    T54_infrastructure_infrastructure_TestObligationPerAxisDto_Handles[Handles]
    T54_infrastructure_infrastructure_TestObligationPerAxisDto_ReactsTo[ReactsTo]
    T54_infrastructure_infrastructure_TestObligationPerAxisDto_Transition[Transition]
    T54_infrastructure_infrastructure_TestObligationPerAxisDto_TraitMethod[TraitMethod]
    T54_infrastructure_infrastructure_TestObligationPerAxisDto_Entry[Entry]
    T54_infrastructure_infrastructure_TestObligationPerAxisDto_Emits[Emits]
    T54_infrastructure_infrastructure_TestObligationPerAxisDto_TraitImpl[TraitImpl]
  end
  subgraph T51_infrastructure_infrastructure_TestObligationRuleDto["test_obligation::rules_codec::TestObligationRuleDto"]
    direction TB
    T51_infrastructure_infrastructure_TestObligationRuleDto__self[TestObligationRuleDto]
  end
  subgraph T60_infrastructure_infrastructure_TestObligationRulesDocumentDto["test_obligation::rules_codec::TestObligationRulesDocumentDto"]
    direction TB
    T60_infrastructure_infrastructure_TestObligationRulesDocumentDto__self[TestObligationRulesDocumentDto]
    T60_infrastructure_infrastructure_TestObligationRulesDocumentDto_into_domain([into_domain])
  end
  subgraph T52_infrastructure_infrastructure_WaiverCacheDocumentDto["test_obligation::waiver_cache_codec::WaiverCacheDocumentDto"]
    direction TB
    T52_infrastructure_infrastructure_WaiverCacheDocumentDto__self[WaiverCacheDocumentDto]
  end
  subgraph T49_infrastructure_infrastructure_WaiverCacheEntryDto["test_obligation::waiver_cache_codec::WaiverCacheEntryDto"]
    direction TB
    T49_infrastructure_infrastructure_WaiverCacheEntryDto__self[WaiverCacheEntryDto]
  end
  subgraph T46_infrastructure_infrastructure_WaiverVerdictDto["test_obligation::waiver_cache_codec::WaiverVerdictDto"]
    direction TB
    T46_infrastructure_infrastructure_WaiverVerdictDto__self[WaiverVerdictDto]
    T46_infrastructure_infrastructure_WaiverVerdictDto_Waived[Waived]
    T46_infrastructure_infrastructure_WaiverVerdictDto_Fail[Fail]
    T46_infrastructure_infrastructure_WaiverVerdictDto_Pending[Pending]
  end
  subgraph T51_infrastructure_infrastructure_WaiverVerifierAdapter["test_obligation::waiver_verifier::WaiverVerifierAdapter"]
    direction TB
    T51_infrastructure_infrastructure_WaiverVerifierAdapter__self[WaiverVerifierAdapter]
    T51_infrastructure_infrastructure_WaiverVerifierAdapter_new([new])
  end
  end
end
subgraph cli_driver["cli_driver"]
  direction TB
  subgraph cli_driver_cli_driver_module_test_obligation["cli_driver::test_obligation"]
    direction TB
  subgraph T48_cli_driver_cli_driver_TestObligationCheckHandler["test_obligation::check::TestObligationCheckHandler"]
    direction TB
    T48_cli_driver_cli_driver_TestObligationCheckHandler__self[TestObligationCheckHandler]
    T48_cli_driver_cli_driver_TestObligationCheckHandler_new([new])
    T48_cli_driver_cli_driver_TestObligationCheckHandler_handle([handle])
  end
  subgraph T46_cli_driver_cli_driver_TestObligationCheckInput["test_obligation::check::TestObligationCheckInput"]
    direction TB
    T46_cli_driver_cli_driver_TestObligationCheckInput__self[TestObligationCheckInput]
    T46_cli_driver_cli_driver_TestObligationCheckInput_new([new])
    T46_cli_driver_cli_driver_TestObligationCheckInput_track_id([track_id])
    T46_cli_driver_cli_driver_TestObligationCheckInput_current_branch([current_branch])
  end
  subgraph T49_cli_driver_cli_driver_TestObligationDeriveHandler["test_obligation::derive::TestObligationDeriveHandler"]
    direction TB
    T49_cli_driver_cli_driver_TestObligationDeriveHandler__self[TestObligationDeriveHandler]
    T49_cli_driver_cli_driver_TestObligationDeriveHandler_new([new])
    T49_cli_driver_cli_driver_TestObligationDeriveHandler_handle([handle])
  end
  subgraph T47_cli_driver_cli_driver_TestObligationDeriveInput["test_obligation::derive::TestObligationDeriveInput"]
    direction TB
    T47_cli_driver_cli_driver_TestObligationDeriveInput__self[TestObligationDeriveInput]
    T47_cli_driver_cli_driver_TestObligationDeriveInput_new([new])
    T47_cli_driver_cli_driver_TestObligationDeriveInput_track_id([track_id])
    T47_cli_driver_cli_driver_TestObligationDeriveInput_current_branch([current_branch])
  end
  subgraph T51_cli_driver_cli_driver_TestObligationEvaluateHandler["test_obligation::evaluate::TestObligationEvaluateHandler"]
    direction TB
    T51_cli_driver_cli_driver_TestObligationEvaluateHandler__self[TestObligationEvaluateHandler]
    T51_cli_driver_cli_driver_TestObligationEvaluateHandler_new([new])
    T51_cli_driver_cli_driver_TestObligationEvaluateHandler_handle([handle])
  end
  subgraph T49_cli_driver_cli_driver_TestObligationEvaluateInput["test_obligation::evaluate::TestObligationEvaluateInput"]
    direction TB
    T49_cli_driver_cli_driver_TestObligationEvaluateInput__self[TestObligationEvaluateInput]
    T49_cli_driver_cli_driver_TestObligationEvaluateInput_new([new])
    T49_cli_driver_cli_driver_TestObligationEvaluateInput_track_id([track_id])
    T49_cli_driver_cli_driver_TestObligationEvaluateInput_current_branch([current_branch])
  end
  subgraph T50_cli_driver_cli_driver_TestObligationResultsHandler["test_obligation::results::TestObligationResultsHandler"]
    direction TB
    T50_cli_driver_cli_driver_TestObligationResultsHandler__self[TestObligationResultsHandler]
    T50_cli_driver_cli_driver_TestObligationResultsHandler_new([new])
    T50_cli_driver_cli_driver_TestObligationResultsHandler_handle([handle])
  end
  subgraph T48_cli_driver_cli_driver_TestObligationResultsInput["test_obligation::results::TestObligationResultsInput"]
    direction TB
    T48_cli_driver_cli_driver_TestObligationResultsInput__self[TestObligationResultsInput]
    T48_cli_driver_cli_driver_TestObligationResultsInput_new([new])
    T48_cli_driver_cli_driver_TestObligationResultsInput_track_id([track_id])
  end
  end
end
subgraph cli_composition["cli_composition"]
  direction TB
  subgraph cli_composition_cli_composition_module_test_obligation["cli_composition::test_obligation"]
    direction TB
  subgraph T61_cli_composition_cli_composition_TestObligationCompositionRoot["test_obligation::TestObligationCompositionRoot"]
    direction TB
    T61_cli_composition_cli_composition_TestObligationCompositionRoot__self[TestObligationCompositionRoot]
    T61_cli_composition_cli_composition_TestObligationCompositionRoot_new([new])
    T61_cli_composition_cli_composition_TestObligationCompositionRoot_derive_handler([derive_handler])
    T61_cli_composition_cli_composition_TestObligationCompositionRoot_check_handler([check_handler])
    T61_cli_composition_cli_composition_TestObligationCompositionRoot_evaluate_handler([evaluate_handler])
    T61_cli_composition_cli_composition_TestObligationCompositionRoot_results_handler([results_handler])
  end
  end
end
subgraph cli["cli"]
  direction TB
  subgraph cli_cli_module_test_obligation["cli::test_obligation"]
    direction TB
  subgraph T26_cli_cli_TestObligationArgs["test_obligation::TestObligationArgs"]
    direction TB
    T26_cli_cli_TestObligationArgs__self[TestObligationArgs]
    T26_cli_cli_TestObligationArgs_new([new])
  end
  subgraph T31_cli_cli_TestObligationCheckArgs["test_obligation::TestObligationCheckArgs"]
    direction TB
    T31_cli_cli_TestObligationCheckArgs__self[TestObligationCheckArgs]
  end
  subgraph T32_cli_cli_TestObligationDeriveArgs["test_obligation::TestObligationDeriveArgs"]
    direction TB
    T32_cli_cli_TestObligationDeriveArgs__self[TestObligationDeriveArgs]
  end
  subgraph T34_cli_cli_TestObligationEvaluateArgs["test_obligation::TestObligationEvaluateArgs"]
    direction TB
    T34_cli_cli_TestObligationEvaluateArgs__self[TestObligationEvaluateArgs]
  end
  subgraph T33_cli_cli_TestObligationResultsArgs["test_obligation::TestObligationResultsArgs"]
    direction TB
    T33_cli_cli_TestObligationResultsArgs__self[TestObligationResultsArgs]
  end
  subgraph T32_cli_cli_TestObligationSubcommand["test_obligation::TestObligationSubcommand"]
    direction TB
    T32_cli_cli_TestObligationSubcommand__self[TestObligationSubcommand]
    T32_cli_cli_TestObligationSubcommand_Derive[Derive]
    T32_cli_cli_TestObligationSubcommand_Check[Check]
    T32_cli_cli_TestObligationSubcommand_Evaluate[Evaluate]
    T32_cli_cli_TestObligationSubcommand_Results[Results]
  end
  end
end
T28_domain_domain_AnchorTextHash_new --> T28_domain_domain_AnchorTextHash__self
T32_domain_domain_ArtifactCodecError_Io --o T31_domain_domain_DiagnosticMessage__self
T32_domain_domain_ArtifactCodecError_MalformedJson --o T31_domain_domain_DiagnosticMessage__self
T32_domain_domain_ArtifactCodecError_DomainInvariant --o T31_domain_domain_DiagnosticMessage__self
T31_domain_domain_BoundTestsSetHash_new --> T31_domain_domain_BoundTestsSetHash__self
T29_domain_domain_DeclarationHash_new --> T29_domain_domain_DeclarationHash__self
T31_domain_domain_DiagnosticMessage_try_new --> T31_domain_domain_DiagnosticMessage__self
T35_domain_domain_EdgeResolutionOutcome_Fail --o T37_domain_domain_FulfillmentFailCategory__self
T31_domain_domain_EdgeVerdictRecord_new --o T34_domain_domain_TestObligationEdgeId__self
T31_domain_domain_EdgeVerdictRecord_new --o T35_domain_domain_EdgeResolutionOutcome__self
T31_domain_domain_EdgeVerdictRecord_new --o T33_domain_domain_TestObligationDrift__self
T31_domain_domain_EdgeVerdictRecord_new --> T31_domain_domain_EdgeVerdictRecord__self
T34_domain_domain_ObligationCheckError_TrackNotActive --o|branch| T31_domain_domain_DiagnosticMessage__self
T34_domain_domain_ObligationCheckError_DriftsDetected --o|drifts| T33_domain_domain_TestObligationDrift__self
T34_domain_domain_ObligationCheckError_UnresolvedEdges --o|edges| T34_domain_domain_TestObligationEdgeId__self
T34_domain_domain_ObligationCheckError_StaleVerdicts --o|edges| T34_domain_domain_TestObligationEdgeId__self
T34_domain_domain_ObligationCheckError_ArtifactCodec --o T32_domain_domain_ArtifactCodecError__self
T34_domain_domain_ObligationCheckError_SourceScan --o T33_domain_domain_TestSourceScanError__self
T34_domain_domain_ObligationCheckError_CacheIo --o T30_domain_domain_VerifyCacheError__self
T35_domain_domain_ObligationDeriveError_RulesLoad --o T42_domain_domain_TestObligationRulesLoadError__self
T35_domain_domain_ObligationDeriveError_TrackNotActive --o|branch| T31_domain_domain_DiagnosticMessage__self
T35_domain_domain_ObligationDeriveError_CatalogueLoad --o T31_domain_domain_DiagnosticMessage__self
T35_domain_domain_ObligationDeriveError_SpecLoad --o T31_domain_domain_DiagnosticMessage__self
T35_domain_domain_ObligationDeriveError_InvalidCatalogueState --o T31_domain_domain_DiagnosticMessage__self
T35_domain_domain_ObligationDeriveError_ArtifactCodec --o T32_domain_domain_ArtifactCodecError__self
T35_domain_domain_ObligationDeriveError_ArtifactWrite --o T31_domain_domain_DiagnosticMessage__self
T37_domain_domain_ObligationEvaluateError_InvalidConfig --o|message| T31_domain_domain_DiagnosticMessage__self
T37_domain_domain_ObligationEvaluateError_TrackNotActive --o|branch| T31_domain_domain_DiagnosticMessage__self
T37_domain_domain_ObligationEvaluateError_VerifierPort --o T35_domain_domain_SemanticVerifierError__self
T37_domain_domain_ObligationEvaluateError_CachePersistence --o T30_domain_domain_VerifyCacheError__self
T37_domain_domain_ObligationEvaluateError_CacheWrite --o T31_domain_domain_DiagnosticMessage__self
T37_domain_domain_ObligationEvaluateError_SemanticFailuresConfirmed --o|records| T31_domain_domain_EdgeVerdictRecord__self
T37_domain_domain_ObligationEvaluateError_HumanEscalationRequired --o|records| T31_domain_domain_EdgeVerdictRecord__self
T48_domain_domain_ObligationFulfillmentCacheDocument_new --o T45_domain_domain_ObligationFulfillmentCacheEntry__self
T48_domain_domain_ObligationFulfillmentCacheDocument_new --> T48_domain_domain_ObligationFulfillmentCacheDocument__self
T48_domain_domain_ObligationFulfillmentCacheDocument_entries --> T45_domain_domain_ObligationFulfillmentCacheEntry__self
T45_domain_domain_ObligationFulfillmentCacheEntry_new --o T34_domain_domain_TestObligationEdgeId__self
T45_domain_domain_ObligationFulfillmentCacheEntry_new --o T30_domain_domain_TestObligationId__self
T45_domain_domain_ObligationFulfillmentCacheEntry_new --o T43_domain_domain_ObligationFulfillmentCacheKey__self
T45_domain_domain_ObligationFulfillmentCacheEntry_new --o T42_domain_domain_ObligationFulfillmentVerdict__self
T45_domain_domain_ObligationFulfillmentCacheEntry_new --> T45_domain_domain_ObligationFulfillmentCacheEntry__self
T43_domain_domain_ObligationFulfillmentCacheKey_new --o T31_domain_domain_BoundTestsSetHash__self
T43_domain_domain_ObligationFulfillmentCacheKey_new --o T29_domain_domain_DeclarationHash__self
T43_domain_domain_ObligationFulfillmentCacheKey_new --o T28_domain_domain_AnchorTextHash__self
T43_domain_domain_ObligationFulfillmentCacheKey_new --> T43_domain_domain_ObligationFulfillmentCacheKey__self
T42_domain_domain_ObligationFulfillmentVerdict_Fail --o|category| T37_domain_domain_FulfillmentFailCategory__self
T42_domain_domain_ObligationFulfillmentVerdict_Fail --o|reason| T31_domain_domain_DiagnosticMessage__self
T36_domain_domain_ObligationResultsError_IoError --o T31_domain_domain_DiagnosticMessage__self
T36_domain_domain_ObligationResultsError_MalformedArtifact --o T31_domain_domain_DiagnosticMessage__self
T33_domain_domain_ObligationsDocument_new --o T28_domain_domain_TestObligation__self
T33_domain_domain_ObligationsDocument_new --> T33_domain_domain_ObligationsDocument__self
T33_domain_domain_ObligationsDocument_obligations --> T28_domain_domain_TestObligation__self
T22_domain_domain_RoleName_try_new --> T22_domain_domain_RoleName__self
T33_domain_domain_RoleObligationRules_new --o T32_domain_domain_TestObligationRule__self
T33_domain_domain_RoleObligationRules_new --> T33_domain_domain_RoleObligationRules__self
T33_domain_domain_RoleObligationRules_obligations --> T32_domain_domain_TestObligationRule__self
T35_domain_domain_SemanticVerifierError_VerifierPort --o T31_domain_domain_DiagnosticMessage__self
T33_domain_domain_TargetEntryRoleKind_Pattern --o T39_domain_domain_TestObligationPatternKind__self
T31_domain_domain_TestBindingRecord_Fulfillment --o|obligation_id| T30_domain_domain_TestObligationId__self
T31_domain_domain_TestBindingRecord_Fulfillment --o|tests| T26_domain_domain_TestLocation__self
T31_domain_domain_TestBindingRecord_Waiver --o|edge_id| T34_domain_domain_TestObligationEdgeId__self
T31_domain_domain_TestBindingRecord_Waiver --o|reason| T26_domain_domain_WaivedReason__self
T31_domain_domain_TestBindingRecord_VoluntaryBinding --o|edge_id| T34_domain_domain_TestObligationEdgeId__self
T31_domain_domain_TestBindingRecord_VoluntaryBinding --o|tests| T26_domain_domain_TestLocation__self
T34_domain_domain_TestBindingsDocument_new --o T31_domain_domain_TestBindingRecord__self
T34_domain_domain_TestBindingsDocument_new --> T34_domain_domain_TestBindingsDocument__self
T34_domain_domain_TestBindingsDocument_records --> T31_domain_domain_TestBindingRecord__self
T30_domain_domain_TestBodySpanHash_new --> T30_domain_domain_TestBodySpanHash__self
T30_domain_domain_TestFunctionName_try_new --> T30_domain_domain_TestFunctionName__self
T26_domain_domain_TestLocation_new --o T28_domain_domain_TestModulePath__self
T26_domain_domain_TestLocation_new --o T30_domain_domain_TestFunctionName__self
T26_domain_domain_TestLocation_new --> T26_domain_domain_TestLocation__self
T26_domain_domain_TestLocation_module_path --> T28_domain_domain_TestModulePath__self
T26_domain_domain_TestLocation_test_name --> T30_domain_domain_TestFunctionName__self
T28_domain_domain_TestModulePath_try_new --> T28_domain_domain_TestModulePath__self
T28_domain_domain_TestObligation_new --o T30_domain_domain_TestObligationId__self
T28_domain_domain_TestObligation_new --o T33_domain_domain_TargetEntryRoleKind__self
T28_domain_domain_TestObligation_new --o T33_domain_domain_TestObligationBrief__self
T28_domain_domain_TestObligation_new --o T29_domain_domain_DeclarationHash__self
T28_domain_domain_TestObligation_new --o T36_domain_domain_TestObligationAnchorId__self
T28_domain_domain_TestObligation_new --> T28_domain_domain_TestObligation__self
T28_domain_domain_TestObligation_id --> T30_domain_domain_TestObligationId__self
T28_domain_domain_TestObligation_target_role --> T33_domain_domain_TargetEntryRoleKind__self
T28_domain_domain_TestObligation_brief --> T33_domain_domain_TestObligationBrief__self
T28_domain_domain_TestObligation_declaration_hash --> T29_domain_domain_DeclarationHash__self
T28_domain_domain_TestObligation_spec_refs --> T36_domain_domain_TestObligationAnchorId__self
T36_domain_domain_TestObligationAnchorId_try_new --> T36_domain_domain_TestObligationAnchorId__self
T33_domain_domain_TestObligationBrief_try_new --> T33_domain_domain_TestObligationBrief__self
T41_domain_domain_TestObligationBriefTemplate_try_new --> T41_domain_domain_TestObligationBriefTemplate__self
T33_domain_domain_TestObligationDrift_missing_obligation --o T30_domain_domain_TestObligationId__self
T33_domain_domain_TestObligationDrift_missing_obligation --o T31_domain_domain_DiagnosticMessage__self
T33_domain_domain_TestObligationDrift_missing_obligation --> T33_domain_domain_TestObligationDrift__self
T33_domain_domain_TestObligationDrift_orphaned_edge --o T34_domain_domain_TestObligationEdgeId__self
T33_domain_domain_TestObligationDrift_orphaned_edge --o T31_domain_domain_DiagnosticMessage__self
T33_domain_domain_TestObligationDrift_orphaned_edge --> T33_domain_domain_TestObligationDrift__self
T33_domain_domain_TestObligationDrift_spec_changed_edge --o T34_domain_domain_TestObligationEdgeId__self
T33_domain_domain_TestObligationDrift_spec_changed_edge --o T31_domain_domain_DiagnosticMessage__self
T33_domain_domain_TestObligationDrift_spec_changed_edge --> T33_domain_domain_TestObligationDrift__self
T33_domain_domain_TestObligationDrift_decl_changed_edge --o T34_domain_domain_TestObligationEdgeId__self
T33_domain_domain_TestObligationDrift_decl_changed_edge --o T31_domain_domain_DiagnosticMessage__self
T33_domain_domain_TestObligationDrift_decl_changed_edge --> T33_domain_domain_TestObligationDrift__self
T33_domain_domain_TestObligationDrift_test_changed_edge --o T34_domain_domain_TestObligationEdgeId__self
T33_domain_domain_TestObligationDrift_test_changed_edge --o T31_domain_domain_DiagnosticMessage__self
T33_domain_domain_TestObligationDrift_test_changed_edge --> T33_domain_domain_TestObligationDrift__self
T33_domain_domain_TestObligationDrift_reason_changed_edge --o T34_domain_domain_TestObligationEdgeId__self
T33_domain_domain_TestObligationDrift_reason_changed_edge --o T31_domain_domain_DiagnosticMessage__self
T33_domain_domain_TestObligationDrift_reason_changed_edge --> T33_domain_domain_TestObligationDrift__self
T34_domain_domain_TestObligationEdgeId_new --o T36_domain_domain_TestObligationAnchorId__self
T34_domain_domain_TestObligationEdgeId_new --> T34_domain_domain_TestObligationEdgeId__self
T34_domain_domain_TestObligationEdgeId_anchor_id --> T36_domain_domain_TestObligationAnchorId__self
T30_domain_domain_TestObligationId_new --o T32_domain_domain_TestObligationKind__self
T30_domain_domain_TestObligationId_new --o T42_domain_domain_TestObligationItemIdentifier__self
T30_domain_domain_TestObligationId_new --> T30_domain_domain_TestObligationId__self
T30_domain_domain_TestObligationId_obligation_kind --> T32_domain_domain_TestObligationKind__self
T30_domain_domain_TestObligationId_item_identifier --> T42_domain_domain_TestObligationItemIdentifier__self
T42_domain_domain_TestObligationItemIdentifier_try_new --> T42_domain_domain_TestObligationItemIdentifier__self
T35_domain_domain_TestObligationMinimum_try_new --> T35_domain_domain_TestObligationMinimum__self
T32_domain_domain_TestObligationRule_new --o T32_domain_domain_TestObligationKind__self
T32_domain_domain_TestObligationRule_new --o T35_domain_domain_TestObligationPerAxis__self
T32_domain_domain_TestObligationRule_new --o T35_domain_domain_TestObligationMinimum__self
T32_domain_domain_TestObligationRule_new --o T41_domain_domain_TestObligationBriefTemplate__self
T32_domain_domain_TestObligationRule_new --> T32_domain_domain_TestObligationRule__self
T32_domain_domain_TestObligationRule_kind --> T32_domain_domain_TestObligationKind__self
T32_domain_domain_TestObligationRule_per_axis --> T35_domain_domain_TestObligationPerAxis__self
T32_domain_domain_TestObligationRule_minimum --> T35_domain_domain_TestObligationMinimum__self
T41_domain_domain_TestObligationRulesDocument_try_new --o T33_domain_domain_RoleObligationRules__self
T41_domain_domain_TestObligationRulesDocument_try_new --o T33_domain_domain_RoleObligationRules__self
T41_domain_domain_TestObligationRulesDocument_try_new --o T33_domain_domain_RoleObligationRules__self
T41_domain_domain_TestObligationRulesDocument_try_new --o T33_domain_domain_RoleObligationRules__self
T41_domain_domain_TestObligationRulesDocument_try_new --o T39_domain_domain_TestObligationPatternKind__self
T41_domain_domain_TestObligationRulesDocument_try_new --o T33_domain_domain_RoleObligationRules__self
T41_domain_domain_TestObligationRulesDocument_try_new --> T41_domain_domain_TestObligationRulesDocument__self
T41_domain_domain_TestObligationRulesDocument_try_new --> T42_domain_domain_TestObligationRulesLoadError__self
T41_domain_domain_TestObligationRulesDocument_data_roles --> T33_domain_domain_RoleObligationRules__self
T41_domain_domain_TestObligationRulesDocument_contract_roles --> T33_domain_domain_RoleObligationRules__self
T41_domain_domain_TestObligationRulesDocument_function_roles --> T33_domain_domain_RoleObligationRules__self
T41_domain_domain_TestObligationRulesDocument_patterns --> T33_domain_domain_RoleObligationRules__self
T41_domain_domain_TestObligationRulesDocument_patterns --> T39_domain_domain_TestObligationPatternKind__self
T41_domain_domain_TestObligationRulesDocument_trait_impls --> T33_domain_domain_RoleObligationRules__self
T42_domain_domain_TestObligationRulesLoadError_RoleNotCovered --o|role_name| T22_domain_domain_RoleName__self
T42_domain_domain_TestObligationRulesLoadError_ObligationsFieldOmitted --o|role_name| T22_domain_domain_RoleName__self
T42_domain_domain_TestObligationRulesLoadError_UnknownRoleName --o|role_name| T22_domain_domain_RoleName__self
T42_domain_domain_TestObligationRulesLoadError_InvalidRuleValue --o|role_name| T22_domain_domain_RoleName__self
T42_domain_domain_TestObligationRulesLoadError_InvalidRuleValue --o|message| T31_domain_domain_DiagnosticMessage__self
T42_domain_domain_TestObligationRulesLoadError_IoError --o T31_domain_domain_DiagnosticMessage__self
T42_domain_domain_TestObligationRulesLoadError_MalformedJson --o T31_domain_domain_DiagnosticMessage__self
T33_domain_domain_TestSourceScanError_Io --o T31_domain_domain_DiagnosticMessage__self
T33_domain_domain_TestSourceScanError_Parse --o T31_domain_domain_DiagnosticMessage__self
T39_domain_domain_UncitedSpecElementFinding_new --> T39_domain_domain_UncitedSpecElementFinding__self
T30_domain_domain_VerifyCacheError_Io --o T31_domain_domain_DiagnosticMessage__self
T30_domain_domain_VerifyCacheError_MalformedJson --o T31_domain_domain_DiagnosticMessage__self
T26_domain_domain_WaivedReason_try_new --> T26_domain_domain_WaivedReason__self
T30_domain_domain_WaivedReasonHash_new --> T30_domain_domain_WaivedReasonHash__self
T33_domain_domain_WaiverCacheDocument_new --o T30_domain_domain_WaiverCacheEntry__self
T33_domain_domain_WaiverCacheDocument_new --> T33_domain_domain_WaiverCacheDocument__self
T33_domain_domain_WaiverCacheDocument_entries --> T30_domain_domain_WaiverCacheEntry__self
T30_domain_domain_WaiverCacheEntry_new --o T34_domain_domain_TestObligationEdgeId__self
T30_domain_domain_WaiverCacheEntry_new --o T28_domain_domain_WaiverCacheKey__self
T30_domain_domain_WaiverCacheEntry_new --o T27_domain_domain_WaiverVerdict__self
T30_domain_domain_WaiverCacheEntry_new --> T30_domain_domain_WaiverCacheEntry__self
T28_domain_domain_WaiverCacheKey_new --o T30_domain_domain_WaivedReasonHash__self
T28_domain_domain_WaiverCacheKey_new --o T29_domain_domain_DeclarationHash__self
T28_domain_domain_WaiverCacheKey_new --o T28_domain_domain_AnchorTextHash__self
T28_domain_domain_WaiverCacheKey_new --> T28_domain_domain_WaiverCacheKey__self
T27_domain_domain_WaiverVerdict_Fail --o|reason| T31_domain_domain_DiagnosticMessage__self
R44_domain_domain_ObligationFulfillmentCachePort_load --> T48_domain_domain_ObligationFulfillmentCacheDocument__self
R44_domain_domain_ObligationFulfillmentCachePort_load --> T30_domain_domain_VerifyCacheError__self
R44_domain_domain_ObligationFulfillmentCachePort_save --o T48_domain_domain_ObligationFulfillmentCacheDocument__self
R44_domain_domain_ObligationFulfillmentCachePort_save --> T31_domain_domain_DiagnosticMessage__self
R47_domain_domain_ObligationFulfillmentVerifierPort_verify_pair --> T42_domain_domain_ObligationFulfillmentVerdict__self
R47_domain_domain_ObligationFulfillmentVerifierPort_verify_pair --> T35_domain_domain_SemanticVerifierError__self
R37_domain_domain_ObligationsArtifactPort_load --> T32_domain_domain_ArtifactCodecError__self
R37_domain_domain_ObligationsArtifactPort_load --> T33_domain_domain_ObligationsDocument__self
R37_domain_domain_ObligationsArtifactPort_save --o T33_domain_domain_ObligationsDocument__self
R37_domain_domain_ObligationsArtifactPort_save --> T31_domain_domain_DiagnosticMessage__self
R38_domain_domain_TestBindingsArtifactPort_load --> T32_domain_domain_ArtifactCodecError__self
R38_domain_domain_TestBindingsArtifactPort_load --> T34_domain_domain_TestBindingsDocument__self
R38_domain_domain_TestBindingsArtifactPort_save --o T34_domain_domain_TestBindingsDocument__self
R38_domain_domain_TestBindingsArtifactPort_save --> T31_domain_domain_DiagnosticMessage__self
R43_domain_domain_TestObligationRulesLoaderPort_load --> T41_domain_domain_TestObligationRulesDocument__self
R43_domain_domain_TestObligationRulesLoaderPort_load --> T42_domain_domain_TestObligationRulesLoadError__self
R35_domain_domain_TestSourceScannerPort_scan_test_body --o T26_domain_domain_TestLocation__self
R35_domain_domain_TestSourceScannerPort_scan_test_body --> T33_domain_domain_TestSourceScanError__self
R35_domain_domain_TestSourceScannerPort_hash_test_body --> T30_domain_domain_TestBodySpanHash__self
R29_domain_domain_WaiverCachePort_load --> T30_domain_domain_VerifyCacheError__self
R29_domain_domain_WaiverCachePort_load --> T33_domain_domain_WaiverCacheDocument__self
R29_domain_domain_WaiverCachePort_save --o T33_domain_domain_WaiverCacheDocument__self
R29_domain_domain_WaiverCachePort_save --> T31_domain_domain_DiagnosticMessage__self
R32_domain_domain_WaiverVerifierPort_verify_pair --> T35_domain_domain_SemanticVerifierError__self
R32_domain_domain_WaiverVerifierPort_verify_pair --> T27_domain_domain_WaiverVerdict__self
T46_usecase_usecase_SemanticCalibrationProbeConfig_new --> T46_usecase_usecase_SemanticCalibrationProbeConfig__self
T43_usecase_usecase_CheckTestObligationsCommand_new --> T43_usecase_usecase_CheckTestObligationsCommand__self
T46_usecase_usecase_CheckTestObligationsInteractor_new --> T46_usecase_usecase_CheckTestObligationsInteractor__self
T43_usecase_usecase_CheckTestObligationsOutcome_new_verified_scope --o T34_domain_domain_TestObligationEdgeId__self
T43_usecase_usecase_CheckTestObligationsOutcome_new_verified_scope --o T39_domain_domain_UncitedSpecElementFinding__self
T43_usecase_usecase_CheckTestObligationsOutcome_new_verified_scope --> T43_usecase_usecase_CheckTestObligationsOutcome__self
T43_usecase_usecase_CheckTestObligationsOutcome_new_empty_scope --o T39_domain_domain_UncitedSpecElementFinding__self
T43_usecase_usecase_CheckTestObligationsOutcome_new_empty_scope --> T43_usecase_usecase_CheckTestObligationsOutcome__self
T44_usecase_usecase_DeriveTestObligationsCommand_new --> T44_usecase_usecase_DeriveTestObligationsCommand__self
T47_usecase_usecase_DeriveTestObligationsInteractor_new --> T47_usecase_usecase_DeriveTestObligationsInteractor__self
T46_usecase_usecase_EvaluateTestObligationsCommand_new --> T46_usecase_usecase_EvaluateTestObligationsCommand__self
T49_usecase_usecase_EvaluateTestObligationsInteractor_new --o T44_usecase_usecase_TestObligationEvaluateConfig__self
T49_usecase_usecase_EvaluateTestObligationsInteractor_new --> T49_usecase_usecase_EvaluateTestObligationsInteractor__self
T46_usecase_usecase_EvaluateTestObligationsOutcome_new --> T46_usecase_usecase_EvaluateTestObligationsOutcome__self
T44_usecase_usecase_TestObligationEvaluateConfig_try_new --> T44_usecase_usecase_TestObligationEvaluateConfig__self
T44_usecase_usecase_TestObligationEvaluateConfig_try_new --> T37_domain_domain_ObligationEvaluateError__self
T41_usecase_usecase_TestObligationLaneSummary_new --o T40_usecase_usecase_TestObligationChainLabel__self
T41_usecase_usecase_TestObligationLaneSummary_new --> T41_usecase_usecase_TestObligationLaneSummary__self
T44_usecase_usecase_TestObligationResultsCommand_new --> T44_usecase_usecase_TestObligationResultsCommand__self
T47_usecase_usecase_TestObligationResultsInteractor_new --> T47_usecase_usecase_TestObligationResultsInteractor__self
T43_usecase_usecase_TestObligationResultsOutput_new --o T41_usecase_usecase_TestObligationLaneSummary__self
T43_usecase_usecase_TestObligationResultsOutput_new --o T31_domain_domain_EdgeVerdictRecord__self
T43_usecase_usecase_TestObligationResultsOutput_new --o T39_domain_domain_UncitedSpecElementFinding__self
T43_usecase_usecase_TestObligationResultsOutput_new --> T43_usecase_usecase_TestObligationResultsOutput__self
R54_usecase_usecase_CheckTestObligationsApplicationService_execute --o T43_usecase_usecase_CheckTestObligationsCommand__self
R54_usecase_usecase_CheckTestObligationsApplicationService_execute --> T43_usecase_usecase_CheckTestObligationsOutcome__self
R54_usecase_usecase_CheckTestObligationsApplicationService_execute --> T34_domain_domain_ObligationCheckError__self
R55_usecase_usecase_DeriveTestObligationsApplicationService_execute --o T44_usecase_usecase_DeriveTestObligationsCommand__self
R55_usecase_usecase_DeriveTestObligationsApplicationService_execute --> T35_domain_domain_ObligationDeriveError__self
R57_usecase_usecase_EvaluateTestObligationsApplicationService_execute --o T46_usecase_usecase_EvaluateTestObligationsCommand__self
R57_usecase_usecase_EvaluateTestObligationsApplicationService_execute --> T46_usecase_usecase_EvaluateTestObligationsOutcome__self
R57_usecase_usecase_EvaluateTestObligationsApplicationService_execute --> T37_domain_domain_ObligationEvaluateError__self
R55_usecase_usecase_TestObligationResultsApplicationService_execute --o T44_usecase_usecase_TestObligationResultsCommand__self
R55_usecase_usecase_TestObligationResultsApplicationService_execute --> T43_usecase_usecase_TestObligationResultsOutput__self
R55_usecase_usecase_TestObligationResultsApplicationService_execute --> T36_domain_domain_ObligationResultsError__self
T50_infrastructure_infrastructure_CatalogueEntryRefDto__self ---|alias_of| T50_infrastructure_infrastructure_CatalogueEntryRefDto__self
T65_infrastructure_infrastructure_JsonObligationFulfillmentCacheCodec_new --> T65_infrastructure_infrastructure_JsonObligationFulfillmentCacheCodec__self
T50_infrastructure_infrastructure_JsonObligationsCodec_new --> T50_infrastructure_infrastructure_JsonObligationsCodec__self
T51_infrastructure_infrastructure_JsonTestBindingsCodec_new --> T51_infrastructure_infrastructure_JsonTestBindingsCodec__self
T59_infrastructure_infrastructure_JsonTestObligationRulesLoader_new --> T59_infrastructure_infrastructure_JsonTestObligationRulesLoader__self
T50_infrastructure_infrastructure_JsonWaiverCacheCodec_new --> T50_infrastructure_infrastructure_JsonWaiverCacheCodec__self
T61_infrastructure_infrastructure_ObligationFulfillmentVerdictDto_Fail --o|category| T56_infrastructure_infrastructure_FulfillmentFailCategoryDto__self
T66_infrastructure_infrastructure_ObligationFulfillmentVerifierAdapter_new --> T66_infrastructure_infrastructure_ObligationFulfillmentVerifierAdapter__self
T51_infrastructure_infrastructure_ObligationsCodecError__self ---|alias_of| T32_domain_domain_ArtifactCodecError__self
T52_infrastructure_infrastructure_ObligationsDocumentDto_from_domain --o T33_domain_domain_ObligationsDocument__self
T52_infrastructure_infrastructure_ObligationsDocumentDto_from_domain --> T52_infrastructure_infrastructure_ObligationsDocumentDto__self
T52_infrastructure_infrastructure_ObligationsDocumentDto_into_domain --> T51_infrastructure_infrastructure_ObligationsCodecError__self
T52_infrastructure_infrastructure_ObligationsDocumentDto_into_domain --> T33_domain_domain_ObligationsDocument__self
T50_infrastructure_infrastructure_SynTestSourceScanner_new --> T50_infrastructure_infrastructure_SynTestSourceScanner__self
T50_infrastructure_infrastructure_TestBindingRecordDto_Fulfillment --o|obligation_id| T49_infrastructure_infrastructure_TestObligationIdDto__self
T50_infrastructure_infrastructure_TestBindingRecordDto_Fulfillment --o|tests| T45_infrastructure_infrastructure_TestLocationDto__self
T50_infrastructure_infrastructure_TestBindingRecordDto_Waiver --o|edge_id| T53_infrastructure_infrastructure_TestObligationEdgeIdDto__self
T50_infrastructure_infrastructure_TestBindingRecordDto_VoluntaryBinding --o|edge_id| T53_infrastructure_infrastructure_TestObligationEdgeIdDto__self
T50_infrastructure_infrastructure_TestBindingRecordDto_VoluntaryBinding --o|tests| T45_infrastructure_infrastructure_TestLocationDto__self
T52_infrastructure_infrastructure_TestBindingsCodecError__self ---|alias_of| T32_domain_domain_ArtifactCodecError__self
T53_infrastructure_infrastructure_TestBindingsDocumentDto_from_domain --o T34_domain_domain_TestBindingsDocument__self
T53_infrastructure_infrastructure_TestBindingsDocumentDto_from_domain --> T53_infrastructure_infrastructure_TestBindingsDocumentDto__self
T53_infrastructure_infrastructure_TestBindingsDocumentDto_into_domain --> T52_infrastructure_infrastructure_TestBindingsCodecError__self
T53_infrastructure_infrastructure_TestBindingsDocumentDto_into_domain --> T34_domain_domain_TestBindingsDocument__self
T60_infrastructure_infrastructure_TestObligationRulesDocumentDto_into_domain --> T42_domain_domain_TestObligationRulesLoadError__self
T60_infrastructure_infrastructure_TestObligationRulesDocumentDto_into_domain --> T41_domain_domain_TestObligationRulesDocument__self
T51_infrastructure_infrastructure_WaiverVerifierAdapter_new --> T51_infrastructure_infrastructure_WaiverVerifierAdapter__self
T59_infrastructure_infrastructure_JsonTestObligationRulesLoader__self -.impl.-> R43_domain_domain_TestObligationRulesLoaderPort__self
T50_infrastructure_infrastructure_JsonObligationsCodec__self -.impl.-> R37_domain_domain_ObligationsArtifactPort__self
T51_infrastructure_infrastructure_JsonTestBindingsCodec__self -.impl.-> R38_domain_domain_TestBindingsArtifactPort__self
T65_infrastructure_infrastructure_JsonObligationFulfillmentCacheCodec__self -.impl.-> R44_domain_domain_ObligationFulfillmentCachePort__self
T50_infrastructure_infrastructure_JsonWaiverCacheCodec__self -.impl.-> R29_domain_domain_WaiverCachePort__self
T50_infrastructure_infrastructure_SynTestSourceScanner__self -.impl.-> R35_domain_domain_TestSourceScannerPort__self
T66_infrastructure_infrastructure_ObligationFulfillmentVerifierAdapter__self -.impl.-> R47_domain_domain_ObligationFulfillmentVerifierPort__self
T51_infrastructure_infrastructure_WaiverVerifierAdapter__self -.impl.-> R32_domain_domain_WaiverVerifierPort__self
T48_cli_driver_cli_driver_TestObligationCheckHandler_new --> T48_cli_driver_cli_driver_TestObligationCheckHandler__self
T48_cli_driver_cli_driver_TestObligationCheckHandler_handle --o T46_cli_driver_cli_driver_TestObligationCheckInput__self
T46_cli_driver_cli_driver_TestObligationCheckInput_new --o T31_domain_domain_DiagnosticMessage__self
T46_cli_driver_cli_driver_TestObligationCheckInput_new --> T46_cli_driver_cli_driver_TestObligationCheckInput__self
T46_cli_driver_cli_driver_TestObligationCheckInput_current_branch --> T31_domain_domain_DiagnosticMessage__self
T49_cli_driver_cli_driver_TestObligationDeriveHandler_new --> T49_cli_driver_cli_driver_TestObligationDeriveHandler__self
T49_cli_driver_cli_driver_TestObligationDeriveHandler_handle --o T47_cli_driver_cli_driver_TestObligationDeriveInput__self
T47_cli_driver_cli_driver_TestObligationDeriveInput_new --o T31_domain_domain_DiagnosticMessage__self
T47_cli_driver_cli_driver_TestObligationDeriveInput_new --> T47_cli_driver_cli_driver_TestObligationDeriveInput__self
T47_cli_driver_cli_driver_TestObligationDeriveInput_current_branch --> T31_domain_domain_DiagnosticMessage__self
T51_cli_driver_cli_driver_TestObligationEvaluateHandler_new --> T51_cli_driver_cli_driver_TestObligationEvaluateHandler__self
T51_cli_driver_cli_driver_TestObligationEvaluateHandler_handle --o T49_cli_driver_cli_driver_TestObligationEvaluateInput__self
T49_cli_driver_cli_driver_TestObligationEvaluateInput_new --o T31_domain_domain_DiagnosticMessage__self
T49_cli_driver_cli_driver_TestObligationEvaluateInput_new --> T49_cli_driver_cli_driver_TestObligationEvaluateInput__self
T49_cli_driver_cli_driver_TestObligationEvaluateInput_current_branch --> T31_domain_domain_DiagnosticMessage__self
T50_cli_driver_cli_driver_TestObligationResultsHandler_new --> T50_cli_driver_cli_driver_TestObligationResultsHandler__self
T50_cli_driver_cli_driver_TestObligationResultsHandler_handle --o T48_cli_driver_cli_driver_TestObligationResultsInput__self
T48_cli_driver_cli_driver_TestObligationResultsInput_new --> T48_cli_driver_cli_driver_TestObligationResultsInput__self
T61_cli_composition_cli_composition_TestObligationCompositionRoot_new --> T61_cli_composition_cli_composition_TestObligationCompositionRoot__self
T61_cli_composition_cli_composition_TestObligationCompositionRoot_derive_handler --> T49_cli_driver_cli_driver_TestObligationDeriveHandler__self
T61_cli_composition_cli_composition_TestObligationCompositionRoot_check_handler --> T48_cli_driver_cli_driver_TestObligationCheckHandler__self
T61_cli_composition_cli_composition_TestObligationCompositionRoot_evaluate_handler --> T51_cli_driver_cli_driver_TestObligationEvaluateHandler__self
T61_cli_composition_cli_composition_TestObligationCompositionRoot_results_handler --> T50_cli_driver_cli_driver_TestObligationResultsHandler__self
T26_cli_cli_TestObligationArgs_new --o T32_cli_cli_TestObligationSubcommand__self
T26_cli_cli_TestObligationArgs_new --> T26_cli_cli_TestObligationArgs__self
T26_cli_cli_TestObligationArgs__self --o|subcommand| T32_cli_cli_TestObligationSubcommand__self
T32_cli_cli_TestObligationSubcommand_Derive --o T32_cli_cli_TestObligationDeriveArgs__self
T32_cli_cli_TestObligationSubcommand_Check --o T31_cli_cli_TestObligationCheckArgs__self
T32_cli_cli_TestObligationSubcommand_Evaluate --o T34_cli_cli_TestObligationEvaluateArgs__self
T32_cli_cli_TestObligationSubcommand_Results --o T33_cli_cli_TestObligationResultsArgs__self
class T28_domain_domain_AnchorTextHash_new method_node
class T28_domain_domain_AnchorTextHash_as_hash method_node
class T28_domain_domain_AnchorTextHash__self value_object
class T32_domain_domain_ArtifactCodecError_Io variant_node
class T32_domain_domain_ArtifactCodecError_MalformedJson variant_node
class T32_domain_domain_ArtifactCodecError_DomainInvariant variant_node
class T32_domain_domain_ArtifactCodecError__self error_type
class T31_domain_domain_BoundTestsSetHash_new method_node
class T31_domain_domain_BoundTestsSetHash_as_hash method_node
class T31_domain_domain_BoundTestsSetHash__self value_object
class T29_domain_domain_DeclarationHash_new method_node
class T29_domain_domain_DeclarationHash_as_hash method_node
class T29_domain_domain_DeclarationHash__self value_object
class T31_domain_domain_DiagnosticMessage_try_new method_node
class T31_domain_domain_DiagnosticMessage_as_str method_node
class T31_domain_domain_DiagnosticMessage__self value_object
class T35_domain_domain_EdgeResolutionOutcome_Fulfilled variant_node
class T35_domain_domain_EdgeResolutionOutcome_Waived variant_node
class T35_domain_domain_EdgeResolutionOutcome_Fail variant_node
class T35_domain_domain_EdgeResolutionOutcome_Pending variant_node
class T35_domain_domain_EdgeResolutionOutcome_MissingBinding variant_node
class T35_domain_domain_EdgeResolutionOutcome__self value_object
class T31_domain_domain_EdgeVerdictRecord_new method_node
class T31_domain_domain_EdgeVerdictRecord__self value_object
class T37_domain_domain_FulfillmentFailCategory_Contradiction variant_node
class T37_domain_domain_FulfillmentFailCategory_Substitution variant_node
class T37_domain_domain_FulfillmentFailCategory_CentralUnverified variant_node
class T37_domain_domain_FulfillmentFailCategory_as_kebab method_node
class T37_domain_domain_FulfillmentFailCategory__self value_object
class T34_domain_domain_ObligationCheckError_ObligationsOnly variant_node
class T34_domain_domain_ObligationCheckError_BindingsOnly variant_node
class T34_domain_domain_ObligationCheckError_TrackNotActive variant_node
class T34_domain_domain_ObligationCheckError_DriftsDetected variant_node
class T34_domain_domain_ObligationCheckError_UnresolvedEdges variant_node
class T34_domain_domain_ObligationCheckError_StaleVerdicts variant_node
class T34_domain_domain_ObligationCheckError_ArtifactCodec variant_node
class T34_domain_domain_ObligationCheckError_SourceScan variant_node
class T34_domain_domain_ObligationCheckError_CacheIo variant_node
class T34_domain_domain_ObligationCheckError__self error_type
class T35_domain_domain_ObligationDeriveError_RulesLoad variant_node
class T35_domain_domain_ObligationDeriveError_TrackNotActive variant_node
class T35_domain_domain_ObligationDeriveError_CatalogueLoad variant_node
class T35_domain_domain_ObligationDeriveError_SpecLoad variant_node
class T35_domain_domain_ObligationDeriveError_InvalidCatalogueState variant_node
class T35_domain_domain_ObligationDeriveError_ArtifactCodec variant_node
class T35_domain_domain_ObligationDeriveError_ArtifactWrite variant_node
class T35_domain_domain_ObligationDeriveError__self error_type
class T37_domain_domain_ObligationEvaluateError_InvalidConfig variant_node
class T37_domain_domain_ObligationEvaluateError_TrackNotActive variant_node
class T37_domain_domain_ObligationEvaluateError_VerifierPort variant_node
class T37_domain_domain_ObligationEvaluateError_CachePersistence variant_node
class T37_domain_domain_ObligationEvaluateError_CacheWrite variant_node
class T37_domain_domain_ObligationEvaluateError_SemanticFailuresConfirmed variant_node
class T37_domain_domain_ObligationEvaluateError_HumanEscalationRequired variant_node
class T37_domain_domain_ObligationEvaluateError__self error_type
class T48_domain_domain_ObligationFulfillmentCacheDocument_new method_node
class T48_domain_domain_ObligationFulfillmentCacheDocument_entries method_node
class T48_domain_domain_ObligationFulfillmentCacheDocument__self value_object
class T45_domain_domain_ObligationFulfillmentCacheEntry_new method_node
class T45_domain_domain_ObligationFulfillmentCacheEntry__self value_object
class T43_domain_domain_ObligationFulfillmentCacheKey_new method_node
class T43_domain_domain_ObligationFulfillmentCacheKey__self value_object
class T42_domain_domain_ObligationFulfillmentVerdict_Fulfilled variant_node
class T42_domain_domain_ObligationFulfillmentVerdict_Fail variant_node
class T42_domain_domain_ObligationFulfillmentVerdict_Pending variant_node
class T42_domain_domain_ObligationFulfillmentVerdict__self value_object
class T36_domain_domain_ObligationResultsError_IoError variant_node
class T36_domain_domain_ObligationResultsError_MalformedArtifact variant_node
class T36_domain_domain_ObligationResultsError__self error_type
class T33_domain_domain_ObligationsDocument_new method_node
class T33_domain_domain_ObligationsDocument_track_id method_node
class T33_domain_domain_ObligationsDocument_obligations method_node
class T33_domain_domain_ObligationsDocument__self value_object
class T22_domain_domain_RoleName_try_new method_node
class T22_domain_domain_RoleName_as_str method_node
class T22_domain_domain_RoleName__self value_object
class T33_domain_domain_RoleObligationRules_new method_node
class T33_domain_domain_RoleObligationRules_obligations method_node
class T33_domain_domain_RoleObligationRules_is_empty_explicitly method_node
class T33_domain_domain_RoleObligationRules__self value_object
class T35_domain_domain_SemanticVerifierError_VerifierPort variant_node
class T35_domain_domain_SemanticVerifierError__self error_type
class T33_domain_domain_TargetEntryRoleKind_DataRole variant_node
class T33_domain_domain_TargetEntryRoleKind_ContractRole variant_node
class T33_domain_domain_TargetEntryRoleKind_FunctionRole variant_node
class T33_domain_domain_TargetEntryRoleKind_TraitImpl variant_node
class T33_domain_domain_TargetEntryRoleKind_Pattern variant_node
class T33_domain_domain_TargetEntryRoleKind__self value_object
class T31_domain_domain_TestBindingRecord_Fulfillment variant_node
class T31_domain_domain_TestBindingRecord_Waiver variant_node
class T31_domain_domain_TestBindingRecord_VoluntaryBinding variant_node
class T31_domain_domain_TestBindingRecord__self value_object
class T34_domain_domain_TestBindingsDocument_new method_node
class T34_domain_domain_TestBindingsDocument_track_id method_node
class T34_domain_domain_TestBindingsDocument_records method_node
class T34_domain_domain_TestBindingsDocument__self value_object
class T30_domain_domain_TestBodySpanHash_new method_node
class T30_domain_domain_TestBodySpanHash_as_hash method_node
class T30_domain_domain_TestBodySpanHash__self value_object
class T30_domain_domain_TestFunctionName_try_new method_node
class T30_domain_domain_TestFunctionName_as_str method_node
class T30_domain_domain_TestFunctionName__self value_object
class T26_domain_domain_TestLocation_new method_node
class T26_domain_domain_TestLocation_layer method_node
class T26_domain_domain_TestLocation_module_path method_node
class T26_domain_domain_TestLocation_test_name method_node
class T26_domain_domain_TestLocation__self value_object
class T28_domain_domain_TestModulePath_try_new method_node
class T28_domain_domain_TestModulePath_as_str method_node
class T28_domain_domain_TestModulePath__self value_object
class T28_domain_domain_TestObligation_new method_node
class T28_domain_domain_TestObligation_id method_node
class T28_domain_domain_TestObligation_target_entry method_node
class T28_domain_domain_TestObligation_target_role method_node
class T28_domain_domain_TestObligation_brief method_node
class T28_domain_domain_TestObligation_declaration_hash method_node
class T28_domain_domain_TestObligation_spec_refs method_node
class T28_domain_domain_TestObligation__self value_object
class T36_domain_domain_TestObligationAnchorId_try_new method_node
class T36_domain_domain_TestObligationAnchorId_file_path method_node
class T36_domain_domain_TestObligationAnchorId_element_id method_node
class T36_domain_domain_TestObligationAnchorId__self value_object
class T33_domain_domain_TestObligationBrief_try_new method_node
class T33_domain_domain_TestObligationBrief_as_str method_node
class T33_domain_domain_TestObligationBrief__self value_object
class T41_domain_domain_TestObligationBriefTemplate_try_new method_node
class T41_domain_domain_TestObligationBriefTemplate_as_str method_node
class T41_domain_domain_TestObligationBriefTemplate__self value_object
class T33_domain_domain_TestObligationDrift_missing_obligation method_node
class T33_domain_domain_TestObligationDrift_orphaned_edge method_node
class T33_domain_domain_TestObligationDrift_spec_changed_edge method_node
class T33_domain_domain_TestObligationDrift_decl_changed_edge method_node
class T33_domain_domain_TestObligationDrift_test_changed_edge method_node
class T33_domain_domain_TestObligationDrift_reason_changed_edge method_node
class T33_domain_domain_TestObligationDrift__self value_object
class T37_domain_domain_TestObligationDriftKind_Missing variant_node
class T37_domain_domain_TestObligationDriftKind_Orphaned variant_node
class T37_domain_domain_TestObligationDriftKind_SpecChanged variant_node
class T37_domain_domain_TestObligationDriftKind_DeclChanged variant_node
class T37_domain_domain_TestObligationDriftKind_TestChanged variant_node
class T37_domain_domain_TestObligationDriftKind_ReasonChanged variant_node
class T37_domain_domain_TestObligationDriftKind_as_kebab method_node
class T37_domain_domain_TestObligationDriftKind_is_existence method_node
class T37_domain_domain_TestObligationDriftKind_is_freshness method_node
class T37_domain_domain_TestObligationDriftKind__self value_object
class T34_domain_domain_TestObligationEdgeId_new method_node
class T34_domain_domain_TestObligationEdgeId_entry_key method_node
class T34_domain_domain_TestObligationEdgeId_anchor_id method_node
class T34_domain_domain_TestObligationEdgeId__self value_object
class T30_domain_domain_TestObligationId_new method_node
class T30_domain_domain_TestObligationId_entry_key method_node
class T30_domain_domain_TestObligationId_obligation_kind method_node
class T30_domain_domain_TestObligationId_item_identifier method_node
class T30_domain_domain_TestObligationId__self value_object
class T42_domain_domain_TestObligationItemIdentifier_try_new method_node
class T42_domain_domain_TestObligationItemIdentifier_as_str method_node
class T42_domain_domain_TestObligationItemIdentifier__self value_object
class T32_domain_domain_TestObligationKind_Boundary variant_node
class T32_domain_domain_TestObligationKind_InvariantPreservation variant_node
class T32_domain_domain_TestObligationKind_EventEmission variant_node
class T32_domain_domain_TestObligationKind_LogicResult variant_node
class T32_domain_domain_TestObligationKind_PredicateBothBranches variant_node
class T32_domain_domain_TestObligationKind_ConstructionResult variant_node
class T32_domain_domain_TestObligationKind_Result variant_node
class T32_domain_domain_TestObligationKind_Reaction variant_node
class T32_domain_domain_TestObligationKind_Transition variant_node
class T32_domain_domain_TestObligationKind_Contract variant_node
class T32_domain_domain_TestObligationKind_ContractConformance variant_node
class T32_domain_domain_TestObligationKind_Logic variant_node
class T32_domain_domain_TestObligationKind_as_kebab method_node
class T32_domain_domain_TestObligationKind__self value_object
class T35_domain_domain_TestObligationMinimum_try_new method_node
class T35_domain_domain_TestObligationMinimum_as_usize method_node
class T35_domain_domain_TestObligationMinimum__self value_object
class T39_domain_domain_TestObligationPatternKind_Typestate variant_node
class T39_domain_domain_TestObligationPatternKind__self value_object
class T35_domain_domain_TestObligationPerAxis_Invariant variant_node
class T35_domain_domain_TestObligationPerAxis_Method variant_node
class T35_domain_domain_TestObligationPerAxis_Handles variant_node
class T35_domain_domain_TestObligationPerAxis_ReactsTo variant_node
class T35_domain_domain_TestObligationPerAxis_Transition variant_node
class T35_domain_domain_TestObligationPerAxis_TraitMethod variant_node
class T35_domain_domain_TestObligationPerAxis_Entry variant_node
class T35_domain_domain_TestObligationPerAxis_Emits variant_node
class T35_domain_domain_TestObligationPerAxis_TraitImpl variant_node
class T35_domain_domain_TestObligationPerAxis_as_kebab method_node
class T35_domain_domain_TestObligationPerAxis__self value_object
class T32_domain_domain_TestObligationRule_new method_node
class T32_domain_domain_TestObligationRule_kind method_node
class T32_domain_domain_TestObligationRule_per_axis method_node
class T32_domain_domain_TestObligationRule_minimum method_node
class T32_domain_domain_TestObligationRule__self value_object
class T41_domain_domain_TestObligationRulesDocument_try_new method_node
class T41_domain_domain_TestObligationRulesDocument_data_roles method_node
class T41_domain_domain_TestObligationRulesDocument_contract_roles method_node
class T41_domain_domain_TestObligationRulesDocument_function_roles method_node
class T41_domain_domain_TestObligationRulesDocument_patterns method_node
class T41_domain_domain_TestObligationRulesDocument_trait_impls method_node
class T41_domain_domain_TestObligationRulesDocument__self value_object
class T42_domain_domain_TestObligationRulesLoadError_RoleNotCovered variant_node
class T42_domain_domain_TestObligationRulesLoadError_ObligationsFieldOmitted variant_node
class T42_domain_domain_TestObligationRulesLoadError_UnknownRoleName variant_node
class T42_domain_domain_TestObligationRulesLoadError_InvalidRuleValue variant_node
class T42_domain_domain_TestObligationRulesLoadError_IoError variant_node
class T42_domain_domain_TestObligationRulesLoadError_MalformedJson variant_node
class T42_domain_domain_TestObligationRulesLoadError__self error_type
class T41_domain_domain_TestObligationScopePresence_Both variant_node
class T41_domain_domain_TestObligationScopePresence_Neither variant_node
class T41_domain_domain_TestObligationScopePresence_ObligationsOnly variant_node
class T41_domain_domain_TestObligationScopePresence_BindingsOnly variant_node
class T41_domain_domain_TestObligationScopePresence__self value_object
class T33_domain_domain_TestSourceScanError_Io variant_node
class T33_domain_domain_TestSourceScanError_Parse variant_node
class T33_domain_domain_TestSourceScanError__self error_type
class T39_domain_domain_UncitedSpecElementFinding_new method_node
class T39_domain_domain_UncitedSpecElementFinding__self value_object
class T30_domain_domain_VerifyCacheError_Io variant_node
class T30_domain_domain_VerifyCacheError_MalformedJson variant_node
class T30_domain_domain_VerifyCacheError__self error_type
class T26_domain_domain_WaivedReason_try_new method_node
class T26_domain_domain_WaivedReason_as_str method_node
class T26_domain_domain_WaivedReason__self value_object
class T30_domain_domain_WaivedReasonHash_new method_node
class T30_domain_domain_WaivedReasonHash_as_hash method_node
class T30_domain_domain_WaivedReasonHash__self value_object
class T33_domain_domain_WaiverCacheDocument_new method_node
class T33_domain_domain_WaiverCacheDocument_entries method_node
class T33_domain_domain_WaiverCacheDocument__self value_object
class T30_domain_domain_WaiverCacheEntry_new method_node
class T30_domain_domain_WaiverCacheEntry__self value_object
class T28_domain_domain_WaiverCacheKey_new method_node
class T28_domain_domain_WaiverCacheKey__self value_object
class T27_domain_domain_WaiverVerdict_Waived variant_node
class T27_domain_domain_WaiverVerdict_Fail variant_node
class T27_domain_domain_WaiverVerdict_Pending variant_node
class T27_domain_domain_WaiverVerdict__self value_object
class R44_domain_domain_ObligationFulfillmentCachePort_load method_node
class R44_domain_domain_ObligationFulfillmentCachePort_save method_node
class R44_domain_domain_ObligationFulfillmentCachePort__self secondary_port
class R47_domain_domain_ObligationFulfillmentVerifierPort_verify_pair method_node
class R47_domain_domain_ObligationFulfillmentVerifierPort__self secondary_port
class R37_domain_domain_ObligationsArtifactPort_load method_node
class R37_domain_domain_ObligationsArtifactPort_save method_node
class R37_domain_domain_ObligationsArtifactPort__self secondary_port
class R38_domain_domain_TestBindingsArtifactPort_load method_node
class R38_domain_domain_TestBindingsArtifactPort_save method_node
class R38_domain_domain_TestBindingsArtifactPort__self secondary_port
class R43_domain_domain_TestObligationRulesLoaderPort_load method_node
class R43_domain_domain_TestObligationRulesLoaderPort__self secondary_port
class R35_domain_domain_TestSourceScannerPort_scan_test_body method_node
class R35_domain_domain_TestSourceScannerPort_hash_test_body method_node
class R35_domain_domain_TestSourceScannerPort__self secondary_port
class R29_domain_domain_WaiverCachePort_load method_node
class R29_domain_domain_WaiverCachePort_save method_node
class R29_domain_domain_WaiverCachePort__self secondary_port
class R32_domain_domain_WaiverVerifierPort_verify_pair method_node
class R32_domain_domain_WaiverVerifierPort__self secondary_port
class T46_usecase_usecase_SemanticCalibrationProbeConfig_new method_node
class T46_usecase_usecase_SemanticCalibrationProbeConfig__self value_object
class R44_usecase_usecase_SemanticEscalationDriverPort_evaluate_with_escalation method_node
class R44_usecase_usecase_SemanticEscalationDriverPort__self secondary_port
class R47_usecase_usecase_SemanticEscalationVerdictBridge_project method_node
class R47_usecase_usecase_SemanticEscalationVerdictBridge__self secondary_port
class T43_usecase_usecase_CheckTestObligationsCommand_new method_node
class T43_usecase_usecase_CheckTestObligationsCommand__self command
class T46_usecase_usecase_CheckTestObligationsInteractor_new method_node
class T46_usecase_usecase_CheckTestObligationsInteractor__self interactor
class T43_usecase_usecase_CheckTestObligationsOutcome_new_verified_scope method_node
class T43_usecase_usecase_CheckTestObligationsOutcome_new_empty_scope method_node
class T43_usecase_usecase_CheckTestObligationsOutcome__self dto
class T44_usecase_usecase_DeriveTestObligationsCommand_new method_node
class T44_usecase_usecase_DeriveTestObligationsCommand__self command
class T47_usecase_usecase_DeriveTestObligationsInteractor_new method_node
class T47_usecase_usecase_DeriveTestObligationsInteractor__self interactor
class T46_usecase_usecase_EvaluateTestObligationsCommand_new method_node
class T46_usecase_usecase_EvaluateTestObligationsCommand__self command
class T49_usecase_usecase_EvaluateTestObligationsInteractor_new method_node
class T49_usecase_usecase_EvaluateTestObligationsInteractor__self interactor
class T46_usecase_usecase_EvaluateTestObligationsOutcome_new method_node
class T46_usecase_usecase_EvaluateTestObligationsOutcome__self dto
class T40_usecase_usecase_TestObligationChainLabel_Fulfillment variant_node
class T40_usecase_usecase_TestObligationChainLabel_Waiver variant_node
class T40_usecase_usecase_TestObligationChainLabel__self dto
class T44_usecase_usecase_TestObligationEvaluateConfig_try_new method_node
class T44_usecase_usecase_TestObligationEvaluateConfig__self value_object
class T41_usecase_usecase_TestObligationLaneSummary_new method_node
class T41_usecase_usecase_TestObligationLaneSummary__self dto
class T44_usecase_usecase_TestObligationResultsCommand_new method_node
class T44_usecase_usecase_TestObligationResultsCommand__self command
class T47_usecase_usecase_TestObligationResultsInteractor_new method_node
class T47_usecase_usecase_TestObligationResultsInteractor__self interactor
class T43_usecase_usecase_TestObligationResultsOutput_new method_node
class T43_usecase_usecase_TestObligationResultsOutput__self dto
class R54_usecase_usecase_CheckTestObligationsApplicationService_execute method_node
class R54_usecase_usecase_CheckTestObligationsApplicationService__self app_service
class R55_usecase_usecase_DeriveTestObligationsApplicationService_execute method_node
class R55_usecase_usecase_DeriveTestObligationsApplicationService__self app_service
class R57_usecase_usecase_EvaluateTestObligationsApplicationService_execute method_node
class R57_usecase_usecase_EvaluateTestObligationsApplicationService__self app_service
class R55_usecase_usecase_TestObligationResultsApplicationService_execute method_node
class R55_usecase_usecase_TestObligationResultsApplicationService__self app_service
class T50_infrastructure_infrastructure_CatalogueEntryRefDto__self dto
class T45_infrastructure_infrastructure_ContractRoleKey_SecondaryPort variant_node
class T45_infrastructure_infrastructure_ContractRoleKey_SpecificationPort variant_node
class T45_infrastructure_infrastructure_ContractRoleKey_Repository variant_node
class T45_infrastructure_infrastructure_ContractRoleKey_ApplicationService variant_node
class T45_infrastructure_infrastructure_ContractRoleKey__self dto
class T41_infrastructure_infrastructure_DataRoleKey_ValueObject variant_node
class T41_infrastructure_infrastructure_DataRoleKey_Entity variant_node
class T41_infrastructure_infrastructure_DataRoleKey_AggregateRoot variant_node
class T41_infrastructure_infrastructure_DataRoleKey_DomainService variant_node
class T41_infrastructure_infrastructure_DataRoleKey_UseCase variant_node
class T41_infrastructure_infrastructure_DataRoleKey_EventPolicy variant_node
class T41_infrastructure_infrastructure_DataRoleKey_DomainEvent variant_node
class T41_infrastructure_infrastructure_DataRoleKey_Specification variant_node
class T41_infrastructure_infrastructure_DataRoleKey_Factory variant_node
class T41_infrastructure_infrastructure_DataRoleKey_Interactor variant_node
class T41_infrastructure_infrastructure_DataRoleKey_Command variant_node
class T41_infrastructure_infrastructure_DataRoleKey_Query variant_node
class T41_infrastructure_infrastructure_DataRoleKey_Dto variant_node
class T41_infrastructure_infrastructure_DataRoleKey_ErrorType variant_node
class T41_infrastructure_infrastructure_DataRoleKey_SecondaryAdapter variant_node
class T41_infrastructure_infrastructure_DataRoleKey_CompositionRoot variant_node
class T41_infrastructure_infrastructure_DataRoleKey_PrimaryAdapter variant_node
class T41_infrastructure_infrastructure_DataRoleKey__self dto
class T56_infrastructure_infrastructure_FulfillmentFailCategoryDto_Contradiction variant_node
class T56_infrastructure_infrastructure_FulfillmentFailCategoryDto_Substitution variant_node
class T56_infrastructure_infrastructure_FulfillmentFailCategoryDto_CentralUnverified variant_node
class T56_infrastructure_infrastructure_FulfillmentFailCategoryDto__self dto
class T45_infrastructure_infrastructure_FunctionRoleKey_UseCaseFunction variant_node
class T45_infrastructure_infrastructure_FunctionRoleKey_FreeFunction variant_node
class T45_infrastructure_infrastructure_FunctionRoleKey__self dto
class T65_infrastructure_infrastructure_JsonObligationFulfillmentCacheCodec_new method_node
class T65_infrastructure_infrastructure_JsonObligationFulfillmentCacheCodec__self secondary_adapter
class T50_infrastructure_infrastructure_JsonObligationsCodec_new method_node
class T50_infrastructure_infrastructure_JsonObligationsCodec__self secondary_adapter
class T51_infrastructure_infrastructure_JsonTestBindingsCodec_new method_node
class T51_infrastructure_infrastructure_JsonTestBindingsCodec__self secondary_adapter
class T59_infrastructure_infrastructure_JsonTestObligationRulesLoader_new method_node
class T59_infrastructure_infrastructure_JsonTestObligationRulesLoader__self secondary_adapter
class T50_infrastructure_infrastructure_JsonWaiverCacheCodec_new method_node
class T50_infrastructure_infrastructure_JsonWaiverCacheCodec__self secondary_adapter
class T67_infrastructure_infrastructure_ObligationFulfillmentCacheDocumentDto__self dto
class T64_infrastructure_infrastructure_ObligationFulfillmentCacheEntryDto__self dto
class T61_infrastructure_infrastructure_ObligationFulfillmentVerdictDto_Fulfilled variant_node
class T61_infrastructure_infrastructure_ObligationFulfillmentVerdictDto_Fail variant_node
class T61_infrastructure_infrastructure_ObligationFulfillmentVerdictDto_Pending variant_node
class T61_infrastructure_infrastructure_ObligationFulfillmentVerdictDto__self dto
class T66_infrastructure_infrastructure_ObligationFulfillmentVerifierAdapter_new method_node
class T66_infrastructure_infrastructure_ObligationFulfillmentVerifierAdapter__self secondary_adapter
class T51_infrastructure_infrastructure_ObligationsCodecError__self error_type
class T52_infrastructure_infrastructure_ObligationsDocumentDto_from_domain method_node
class T52_infrastructure_infrastructure_ObligationsDocumentDto_into_domain method_node
class T52_infrastructure_infrastructure_ObligationsDocumentDto__self dto
class T40_infrastructure_infrastructure_PatternKey_Typestate variant_node
class T40_infrastructure_infrastructure_PatternKey__self dto
class T52_infrastructure_infrastructure_RoleObligationRulesDto__self dto
class T50_infrastructure_infrastructure_SynTestSourceScanner_new method_node
class T50_infrastructure_infrastructure_SynTestSourceScanner__self secondary_adapter
class T50_infrastructure_infrastructure_TestBindingRecordDto_Fulfillment variant_node
class T50_infrastructure_infrastructure_TestBindingRecordDto_Waiver variant_node
class T50_infrastructure_infrastructure_TestBindingRecordDto_VoluntaryBinding variant_node
class T50_infrastructure_infrastructure_TestBindingRecordDto__self dto
class T52_infrastructure_infrastructure_TestBindingsCodecError__self error_type
class T53_infrastructure_infrastructure_TestBindingsDocumentDto_from_domain method_node
class T53_infrastructure_infrastructure_TestBindingsDocumentDto_into_domain method_node
class T53_infrastructure_infrastructure_TestBindingsDocumentDto__self dto
class T45_infrastructure_infrastructure_TestLocationDto__self dto
class T55_infrastructure_infrastructure_TestObligationAnchorIdDto__self dto
class T47_infrastructure_infrastructure_TestObligationDto__self dto
class T53_infrastructure_infrastructure_TestObligationEdgeIdDto__self dto
class T49_infrastructure_infrastructure_TestObligationIdDto__self dto
class T51_infrastructure_infrastructure_TestObligationKindDto_Boundary variant_node
class T51_infrastructure_infrastructure_TestObligationKindDto_InvariantPreservation variant_node
class T51_infrastructure_infrastructure_TestObligationKindDto_EventEmission variant_node
class T51_infrastructure_infrastructure_TestObligationKindDto_LogicResult variant_node
class T51_infrastructure_infrastructure_TestObligationKindDto_PredicateBothBranches variant_node
class T51_infrastructure_infrastructure_TestObligationKindDto_ConstructionResult variant_node
class T51_infrastructure_infrastructure_TestObligationKindDto_Result variant_node
class T51_infrastructure_infrastructure_TestObligationKindDto_Reaction variant_node
class T51_infrastructure_infrastructure_TestObligationKindDto_Transition variant_node
class T51_infrastructure_infrastructure_TestObligationKindDto_Contract variant_node
class T51_infrastructure_infrastructure_TestObligationKindDto_ContractConformance variant_node
class T51_infrastructure_infrastructure_TestObligationKindDto_Logic variant_node
class T51_infrastructure_infrastructure_TestObligationKindDto__self dto
class T54_infrastructure_infrastructure_TestObligationPerAxisDto_Invariant variant_node
class T54_infrastructure_infrastructure_TestObligationPerAxisDto_Method variant_node
class T54_infrastructure_infrastructure_TestObligationPerAxisDto_Handles variant_node
class T54_infrastructure_infrastructure_TestObligationPerAxisDto_ReactsTo variant_node
class T54_infrastructure_infrastructure_TestObligationPerAxisDto_Transition variant_node
class T54_infrastructure_infrastructure_TestObligationPerAxisDto_TraitMethod variant_node
class T54_infrastructure_infrastructure_TestObligationPerAxisDto_Entry variant_node
class T54_infrastructure_infrastructure_TestObligationPerAxisDto_Emits variant_node
class T54_infrastructure_infrastructure_TestObligationPerAxisDto_TraitImpl variant_node
class T54_infrastructure_infrastructure_TestObligationPerAxisDto__self dto
class T51_infrastructure_infrastructure_TestObligationRuleDto__self dto
class T60_infrastructure_infrastructure_TestObligationRulesDocumentDto_into_domain method_node
class T60_infrastructure_infrastructure_TestObligationRulesDocumentDto__self dto
class T52_infrastructure_infrastructure_WaiverCacheDocumentDto__self dto
class T49_infrastructure_infrastructure_WaiverCacheEntryDto__self dto
class T46_infrastructure_infrastructure_WaiverVerdictDto_Waived variant_node
class T46_infrastructure_infrastructure_WaiverVerdictDto_Fail variant_node
class T46_infrastructure_infrastructure_WaiverVerdictDto_Pending variant_node
class T46_infrastructure_infrastructure_WaiverVerdictDto__self dto
class T51_infrastructure_infrastructure_WaiverVerifierAdapter_new method_node
class T51_infrastructure_infrastructure_WaiverVerifierAdapter__self secondary_adapter
class T48_cli_driver_cli_driver_TestObligationCheckHandler_new method_node
class T48_cli_driver_cli_driver_TestObligationCheckHandler_handle method_node
class T46_cli_driver_cli_driver_TestObligationCheckInput_new method_node
class T46_cli_driver_cli_driver_TestObligationCheckInput_track_id method_node
class T46_cli_driver_cli_driver_TestObligationCheckInput_current_branch method_node
class T46_cli_driver_cli_driver_TestObligationCheckInput__self dto
class T49_cli_driver_cli_driver_TestObligationDeriveHandler_new method_node
class T49_cli_driver_cli_driver_TestObligationDeriveHandler_handle method_node
class T47_cli_driver_cli_driver_TestObligationDeriveInput_new method_node
class T47_cli_driver_cli_driver_TestObligationDeriveInput_track_id method_node
class T47_cli_driver_cli_driver_TestObligationDeriveInput_current_branch method_node
class T47_cli_driver_cli_driver_TestObligationDeriveInput__self dto
class T51_cli_driver_cli_driver_TestObligationEvaluateHandler_new method_node
class T51_cli_driver_cli_driver_TestObligationEvaluateHandler_handle method_node
class T49_cli_driver_cli_driver_TestObligationEvaluateInput_new method_node
class T49_cli_driver_cli_driver_TestObligationEvaluateInput_track_id method_node
class T49_cli_driver_cli_driver_TestObligationEvaluateInput_current_branch method_node
class T49_cli_driver_cli_driver_TestObligationEvaluateInput__self dto
class T50_cli_driver_cli_driver_TestObligationResultsHandler_new method_node
class T50_cli_driver_cli_driver_TestObligationResultsHandler_handle method_node
class T48_cli_driver_cli_driver_TestObligationResultsInput_new method_node
class T48_cli_driver_cli_driver_TestObligationResultsInput_track_id method_node
class T48_cli_driver_cli_driver_TestObligationResultsInput__self dto
class T61_cli_composition_cli_composition_TestObligationCompositionRoot_new method_node
class T61_cli_composition_cli_composition_TestObligationCompositionRoot_derive_handler method_node
class T61_cli_composition_cli_composition_TestObligationCompositionRoot_check_handler method_node
class T61_cli_composition_cli_composition_TestObligationCompositionRoot_evaluate_handler method_node
class T61_cli_composition_cli_composition_TestObligationCompositionRoot_results_handler method_node
class T26_cli_cli_TestObligationArgs_new method_node
class T26_cli_cli_TestObligationArgs__self dto
class T31_cli_cli_TestObligationCheckArgs__self dto
class T32_cli_cli_TestObligationDeriveArgs__self dto
class T34_cli_cli_TestObligationEvaluateArgs__self dto
class T33_cli_cli_TestObligationResultsArgs__self dto
class T32_cli_cli_TestObligationSubcommand_Derive variant_node
class T32_cli_cli_TestObligationSubcommand_Check variant_node
class T32_cli_cli_TestObligationSubcommand_Evaluate variant_node
class T32_cli_cli_TestObligationSubcommand_Results variant_node
class T32_cli_cli_TestObligationSubcommand__self dto
```
