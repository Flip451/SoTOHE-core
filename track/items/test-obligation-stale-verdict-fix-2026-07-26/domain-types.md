<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| EdgeResolutionOutcome | enum | modify | Fulfillment, Waiver, MissingBinding | 🟡 | 🔵 |
| FulfillmentCacheReevaluationReason | enum | add | Absent, LegacyRowsMissingBoundTests | 🔵 | 🔵 |
| ObligationFulfillmentCacheLoad | enum | add | Current, ReevaluationRequired | 🟡 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| EdgeVerdictRecord | value_object | modify | — | 🟡 | 🔵 |
| ObligationFulfillmentCacheDocument | value_object | modify | — | 🔵 | 🔵 |
| ObligationFulfillmentCacheEntry | value_object | modify | — | 🟡 | 🔵 |
| ObligationsDocument | value_object | modify | — | 🟡 | 🔵 |
| ResolvedBoundTests | value_object | add | — | 🟡 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| FulfillmentCacheLookupError | error_type | add | AmbiguousCurrentEntries | 🔵 | 🔵 |
| ObligationCheckError | error_type | modify | RulesLoad, ObligationsAbsent, BindingsAbsent, StaleObligationsArtifact, DriftsDetected, UnresolvedEdges, StaleVerdicts, CatalogueLoad, SpecLoad, InvalidCatalogueState, ArtifactCodec, SourceScan, CacheIo, TaskAttribution, FulfillmentCacheLookup, BindingConsistency, FulfillmentCacheRequiresEvaluation | 🔵 | 🔵 |
| ObligationEvaluateError | error_type | modify | TrackNotActive, CatalogueLoad, SpecLoad, ArtifactLoad, TestSourceScan, VerifierPort, CachePersistence, SemanticFailuresConfirmed, HumanEscalationRequired, FulfillmentCacheLookup, BindingConsistency, CacheEntry | 🟡 | 🔵 |
| ObligationFulfillmentCacheEntryError | error_type | add | BoundTestsHashMismatch | 🟡 | 🔵 |
| TestBindingConsistencyError | error_type | add | VoluntaryBindingOwnsDerivedObligation | 🔵 | 🔵 |
| VerifyCacheError | error_type | modify | Io, MalformedJson, FulfillmentCacheEntry | 🟡 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ObligationFulfillmentCachePort | secondary_port | modify | fn load(&self, track_id: &TrackId) -> Result<ObligationFulfillmentCacheLoad, VerifyCacheError>, fn save(&self, doc: &ObligationFulfillmentCacheDocument) -> Result<(), DiagnosticMessage> | 🟡 | 🔵 |

