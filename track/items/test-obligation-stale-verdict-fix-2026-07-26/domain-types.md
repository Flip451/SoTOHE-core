<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| EdgeResolutionOutcome | enum | modify | Fulfillment, Waiver, MissingBinding | 🔵 | 🔵 |
| ObligationFulfillmentCacheEntryState | enum | add | Legacy, Identified | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| EdgeVerdictRecord | value_object | modify | — | 🔵 | 🔵 |
| ObligationFulfillmentCacheDocument | value_object | modify | — | 🔵 | 🔵 |
| ObligationFulfillmentCacheEntry | value_object | modify | — | 🔵 | 🔵 |
| ObligationsDocument | value_object | modify | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| FulfillmentCacheLookupError | error_type | add | AmbiguousCurrentEntries | 🔵 | 🔵 |
| ObligationCheckError | error_type | modify | RulesLoad, ObligationsAbsent, BindingsAbsent, StaleObligationsArtifact, DriftsDetected, UnresolvedEdges, StaleVerdicts, CatalogueLoad, SpecLoad, InvalidCatalogueState, ArtifactCodec, SourceScan, CacheIo, TaskAttribution, FulfillmentCacheLookup, BindingConsistency, FulfillmentCacheRequiresEvaluation | 🔵 | 🔵 |
| ObligationEvaluateError | error_type | modify | TrackNotActive, CatalogueLoad, SpecLoad, ArtifactLoad, TestSourceScan, VerifierPort, CachePersistence, SemanticFailuresConfirmed, HumanEscalationRequired, FulfillmentCacheLookup, BindingConsistency | 🔵 | 🔵 |
| TestBindingConsistencyError | error_type | add | VoluntaryBindingOwnsDerivedObligation | 🔵 | 🔵 |
| VerifyCacheError | error_type | reference | Io, MalformedJson | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| domain::tddd::test_obligation::ids::unavailable_diagnostic_message | free_function | add | fn() -> DiagnosticMessage | 🔵 | 🔵 |

