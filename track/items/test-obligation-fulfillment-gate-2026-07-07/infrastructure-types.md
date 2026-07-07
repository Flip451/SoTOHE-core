<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ContractRoleKey | enum | add | SecondaryPort, SpecificationPort, Repository, ApplicationService | 🟡 | 🔵 |
| DataRoleKey | enum | add | ValueObject, Entity, AggregateRoot, DomainService, UseCase, EventPolicy, DomainEvent, Specification, Factory, Interactor, Command, Query, Dto, ErrorType, SecondaryAdapter, CompositionRoot, PrimaryAdapter | 🟡 | 🔵 |
| FulfillmentFailCategoryDto | enum | add | Contradiction, Substitution, CentralUnverified | 🟡 | 🔵 |
| FunctionRoleKey | enum | add | UseCaseFunction, FreeFunction | 🟡 | 🔵 |
| ObligationFulfillmentVerdictDto | enum | add | Fulfilled, Fail, Pending | 🟡 | 🔵 |
| PatternKey | enum | add | Typestate | 🟡 | 🔵 |
| TestBindingRecordDto | enum | add | Fulfillment, Waiver, VoluntaryBinding | 🟡 | 🔵 |
| TestObligationKindDto | enum | add | Boundary, InvariantPreservation, EventEmission, LogicResult, PredicateBothBranches, ConstructionResult, Result, Reaction, Transition, Contract, ContractConformance, Logic | 🟡 | 🔵 |
| TestObligationPerAxisDto | enum | add | Invariant, Method, Handles, ReactsTo, Transition, TraitMethod, Entry, Emits, TraitImpl | 🟡 | 🔵 |
| WaiverVerdictDto | enum | add | Waived, Fail, Pending | 🟡 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ObligationsCodecError | error_type | add | — | 🟡 | 🔵 |
| TestBindingsCodecError | error_type | add | — | 🟡 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueEntryRefDto | dto | add | — | 🟡 | 🔵 |
| ObligationFulfillmentCacheDocumentDto | dto | add | — | 🟡 | 🔵 |
| ObligationFulfillmentCacheEntryDto | dto | add | — | 🟡 | 🔵 |
| ObligationsDocumentDto | dto | add | — | 🟡 | 🔵 |
| RoleObligationRulesDto | dto | add | — | 🟡 | 🔵 |
| TestBindingsDocumentDto | dto | add | — | 🟡 | 🔵 |
| TestLocationDto | dto | add | — | 🟡 | 🔵 |
| TestObligationAnchorIdDto | dto | add | — | 🟡 | 🔵 |
| TestObligationDto | dto | add | — | 🟡 | 🔵 |
| TestObligationEdgeIdDto | dto | add | — | 🟡 | 🔵 |
| TestObligationIdDto | dto | add | — | 🟡 | 🔵 |
| TestObligationRuleDto | dto | add | — | 🟡 | 🔵 |
| TestObligationRulesDocumentDto | dto | add | — | 🟡 | 🔵 |
| WaiverCacheDocumentDto | dto | add | — | 🟡 | 🔵 |
| WaiverCacheEntryDto | dto | add | — | 🟡 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| JsonObligationFulfillmentCacheCodec | secondary_adapter | add | impl ObligationFulfillmentCachePort | 🟡 | 🔵 |
| JsonObligationsCodec | secondary_adapter | add | impl ObligationsArtifactPort | 🟡 | 🔵 |
| JsonTestBindingsCodec | secondary_adapter | add | impl TestBindingsArtifactPort | 🟡 | 🔵 |
| JsonTestObligationRulesLoader | secondary_adapter | add | impl TestObligationRulesLoaderPort | 🟡 | 🔵 |
| JsonWaiverCacheCodec | secondary_adapter | add | impl WaiverCachePort | 🟡 | 🔵 |
| ObligationFulfillmentVerifierAdapter | secondary_adapter | add | impl ObligationFulfillmentVerifierPort | 🟡 | 🔵 |
| SynTestSourceScanner | secondary_adapter | add | impl TestSourceScannerPort | 🟡 | 🔵 |
| WaiverVerifierAdapter | secondary_adapter | add | impl WaiverVerifierPort | 🟡 | 🔵 |

