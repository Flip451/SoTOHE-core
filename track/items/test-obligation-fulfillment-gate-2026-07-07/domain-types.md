<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| EdgeResolutionOutcome | enum | add | Fulfilled, Waived, Fail, Pending, MissingBinding | 🔵 | 🔵 |
| FulfillmentFailCategory | enum | add | Contradiction, Substitution, CentralUnverified | 🔵 | 🔵 |
| ObligationFulfillmentVerdict | enum | add | Fulfilled, Fail, Pending | 🔵 | 🔵 |
| TargetEntryRoleKind | enum | add | DataRole, ContractRole, FunctionRole, TraitImpl, Pattern | 🔵 | 🔵 |
| TestBindingRecord | enum | add | Fulfillment, Waiver, VoluntaryBinding | 🔵 | 🔵 |
| TestObligationDriftKind | enum | add | Missing, Orphaned, SpecChanged, DeclChanged, TestChanged, ReasonChanged | 🔵 | 🔵 |
| TestObligationKind | enum | add | Boundary, InvariantPreservation, EventEmission, LogicResult, PredicateBothBranches, ConstructionResult, Result, Reaction, Transition, Contract, ContractConformance, Logic | 🔵 | 🔵 |
| TestObligationPatternKind | enum | add | Typestate | 🔵 | 🔵 |
| TestObligationPerAxis | enum | add | Invariant, Method, Handles, ReactsTo, Transition, TraitMethod, Entry, Emits, TraitImpl | 🔵 | 🔵 |
| TraitRefScope | enum | add | SelfCrate, Workspace, External | 🔵 | 🔵 |
| WaiverVerdict | enum | add | Waived, Fail, Pending | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AnchorText | value_object | add | — | 🔵 | 🔵 |
| AnchorTextHash | value_object | add | — | 🔵 | 🔵 |
| BoundTestsSetHash | value_object | add | — | 🔵 | 🔵 |
| CrateName | value_object | reference | — | 🔵 | 🔵 |
| DeclarationHash | value_object | add | — | 🔵 | 🔵 |
| DetectionRatePercent | value_object | add | — | 🔵 | 🔵 |
| DiagnosticMessage | value_object | add | — | 🔵 | 🔵 |
| EdgeVerdictRecord | value_object | add | — | 🔵 | 🔵 |
| EntryDeclaration | value_object | add | — | 🔵 | 🔵 |
| NonEmptyDrifts | value_object | add | — | 🔵 | 🔵 |
| NonEmptyEdgeIds | value_object | add | — | 🔵 | 🔵 |
| NonEmptyEdgeVerdictRecords | value_object | add | — | 🔵 | 🔵 |
| NonEmptyTestLocations | value_object | add | — | 🔵 | 🔵 |
| ObligationFulfillmentCacheDocument | value_object | add | — | 🔵 | 🔵 |
| ObligationFulfillmentCacheEntry | value_object | add | — | 🔵 | 🔵 |
| ObligationFulfillmentCacheKey | value_object | add | — | 🔵 | 🔵 |
| ObligationFulfillmentPair | value_object | add | — | 🔵 | 🔵 |
| ObligationsDocument | value_object | add | — | 🔵 | 🔵 |
| RoleName | value_object | add | — | 🔵 | 🔵 |
| RoleObligationRules | value_object | add | — | 🔵 | 🔵 |
| TestBindingsDocument | value_object | add | — | 🔵 | 🔵 |
| TestBodySpanHash | value_object | add | — | 🔵 | 🔵 |
| TestFunctionName | value_object | add | — | 🔵 | 🔵 |
| TestLocation | value_object | add | — | 🔵 | 🔵 |
| TestModulePath | value_object | add | — | 🔵 | 🔵 |
| TestObligation | value_object | add | — | 🔵 | 🔵 |
| TestObligationAnchorId | value_object | add | — | 🔵 | 🔵 |
| TestObligationBrief | value_object | add | — | 🔵 | 🔵 |
| TestObligationBriefTemplate | value_object | add | — | 🔵 | 🔵 |
| TestObligationDrift | value_object | add | — | 🔵 | 🔵 |
| TestObligationEdgeId | value_object | add | — | 🔵 | 🔵 |
| TestObligationId | value_object | add | — | 🔵 | 🔵 |
| TestObligationItemIdentifier | value_object | add | — | 🔵 | 🔵 |
| TestObligationMinimum | value_object | add | — | 🔵 | 🔵 |
| TestObligationRule | value_object | add | — | 🔵 | 🔵 |
| TestObligationRulesDocument | value_object | add | — | 🔵 | 🔵 |
| TestsSource | value_object | add | — | 🔵 | 🔵 |
| TraitImplDeclV2 | value_object | modify | — | 🔵 | 🔵 |
| TraitName | value_object | reference | — | 🔵 | 🔵 |
| UncitedSpecElementFinding | value_object | add | — | 🔵 | 🔵 |
| WaivedReason | value_object | add | — | 🔵 | 🔵 |
| WaivedReasonHash | value_object | add | — | 🔵 | 🔵 |
| WaiverCacheDocument | value_object | add | — | 🔵 | 🔵 |
| WaiverCacheEntry | value_object | add | — | 🔵 | 🔵 |
| WaiverCacheKey | value_object | add | — | 🔵 | 🔵 |
| WaiverPair | value_object | add | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ArtifactCodecError | error_type | add | Io, MalformedJson, DomainInvariant | 🔵 | 🔵 |
| CatalogueDocumentLoaderError | error_type | reference | — | 🔵 | 🔵 |
| ObligationCheckError | error_type | add | ObligationsAbsent, BindingsAbsent, DriftsDetected, UnresolvedEdges, StaleVerdicts, CatalogueLoad, SpecLoad, ArtifactCodec, SourceScan, CacheIo | 🔵 | 🔵 |
| ObligationDeriveError | error_type | add | RulesLoad, TrackNotActive, CatalogueLoad, SpecLoad, InvalidCatalogueState, ArtifactCodec, ArtifactWrite | 🔵 | 🔵 |
| ObligationEvaluateError | error_type | add | TrackNotActive, CatalogueLoad, SpecLoad, ArtifactLoad, TestSourceScan, VerifierPort, CachePersistence, SemanticFailuresConfirmed, HumanEscalationRequired | 🔵 | 🔵 |
| ObligationResultsError | error_type | add | IoError, MalformedArtifact | 🔵 | 🔵 |
| SemanticVerifierError | error_type | add | VerifierPort | 🔵 | 🔵 |
| SpecDocumentLoadError | error_type | add | NotFound, Io, JsonParse, Invariant | 🔵 | 🔵 |
| TestObligationRulesLoadError | error_type | add | RoleNotCovered, ObligationsFieldOmitted, UnknownRoleName, InvalidRuleValue, IoError, MalformedJson | 🔵 | 🔵 |
| TestSourceScanError | error_type | add | Io, Parse | 🔵 | 🔵 |
| ValidationError | error_type | modify | EmptyString, InvalidTrackId, InvalidTaskId, InvalidCommitHash, InvalidTimestamp, InvalidTrackBranch, BranchIdMismatch, StatusOverrideMismatch, EmptyTrackTitle, EmptyTaskDescription, EmptyPlanSectionId, EmptyPlanSectionTitle, DuplicateTaskId, DuplicatePlanSectionId, UnknownTaskReference, DuplicateTaskReference, UnreferencedTask, OverrideIncompatibleWithResolvedTasks, TrackActivationRequiresPlanningOnly, TrackActivationRequiresSchemaV3, TrackAlreadyMaterialized, UnsupportedTargetStatus, SectionNotFound, NoSectionsAvailable, TaskDescriptionMutated, TaskRemoved, DuplicateElementId, InvalidLayerId, InvalidSpecElementId, EmptyAdrAnchor, EmptyConventionAnchor, InvalidContentHash, EmptyInformalGroundSummary, MultiLineInformalGroundSummary, EmptyDecisionGroundRef, InvalidObligationMinimum, InvalidDetectionRate | 🔵 | 🔵 |
| VerifyCacheError | error_type | add | Io, MalformedJson | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueDocumentLoaderPort | secondary_port | reference | — | 🔵 | 🔵 |
| ObligationFulfillmentCachePort | secondary_port | add | fn load(&self, track_id: &TrackId) -> Result<Option<ObligationFulfillmentCacheDocument>, VerifyCacheError>, fn save(&self, doc: &ObligationFulfillmentCacheDocument) -> Result<(), DiagnosticMessage> | 🔵 | 🔵 |
| ObligationFulfillmentVerifierPort | secondary_port | add | fn verify_pair(&self, tests_source: &str, entry_declaration: &str, anchor_text: &str, tier: ModelTier) -> Result<ObligationFulfillmentVerdict, SemanticVerifierError> | 🔵 | 🔵 |
| ObligationsArtifactPort | secondary_port | add | fn load(&self, track_id: &TrackId) -> Result<Option<ObligationsDocument>, ArtifactCodecError>, fn save(&self, doc: &ObligationsDocument) -> Result<(), DiagnosticMessage> | 🔵 | 🔵 |
| SpecDocumentLoaderPort | secondary_port | add | fn load(&self, spec_path: &std::path::Path) -> Result<SpecDocument, SpecDocumentLoadError> | 🔵 | 🔵 |
| TestBindingsArtifactPort | secondary_port | add | fn load(&self, track_id: &TrackId) -> Result<Option<TestBindingsDocument>, ArtifactCodecError>, fn save(&self, doc: &TestBindingsDocument) -> Result<(), DiagnosticMessage> | 🔵 | 🔵 |
| TestObligationRulesLoaderPort | secondary_port | add | fn load(&self) -> Result<TestObligationRulesDocument, TestObligationRulesLoadError> | 🔵 | 🔵 |
| TestSourceScannerPort | secondary_port | add | fn scan_test_body(&self, location: &TestLocation) -> Result<Option<String>, TestSourceScanError>, fn hash_test_body(&self, source: &str) -> TestBodySpanHash | 🔵 | 🔵 |
| WaiverCachePort | secondary_port | add | fn load(&self, track_id: &TrackId) -> Result<Option<WaiverCacheDocument>, VerifyCacheError>, fn save(&self, doc: &WaiverCacheDocument) -> Result<(), DiagnosticMessage> | 🔵 | 🔵 |
| WaiverVerifierPort | secondary_port | add | fn verify_pair(&self, waived_reason: &str, entry_declaration: &str, anchor_text: &str, tier: ModelTier) -> Result<WaiverVerdict, SemanticVerifierError> | 🔵 | 🔵 |

## Domain Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RoleObligationItemsProjector | domain_service | add | — | 🔵 | 🔵 |

