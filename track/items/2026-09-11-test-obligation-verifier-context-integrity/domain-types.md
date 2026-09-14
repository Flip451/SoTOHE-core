<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ObligationResponsibilityHash | value_object | add | — | 🔵 | 🔵 |
| SpecElementHash | value_object | add | — | 🔵 | 🔵 |
| domain::tddd::semantic_verify::SpecElementRef | value_object | reference | — | 🔵 | 🔵 |
| domain::tddd::test_obligation::pair::ObligationFulfillmentPair | value_object | modify | — | 🔵 | 🔵 |
| domain::tddd::test_obligation::pair::WaiverPair | value_object | modify | — | 🔵 | 🔵 |
| domain::tddd::test_obligation::verdict::ObligationFulfillmentCacheKey | value_object | modify | — | 🔵 | 🔵 |
| domain::tddd::test_obligation::verdict::WaiverCacheDocument | value_object | modify | — | 🔵 | 🔵 |
| domain::tddd::test_obligation::verdict::WaiverCacheKey | value_object | modify | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| domain::tddd::test_obligation::errors::ObligationCheckError | error_type | modify | RulesLoad, ObligationsAbsent, BindingsAbsent, StaleObligationsArtifact, DriftsDetected, UnresolvedEdges, StaleVerdicts, CatalogueLoad, SpecLoad, InvalidCatalogueState, ArtifactCodec, SourceScan, CacheIo, TaskAttribution, FulfillmentCacheLookup, WaiverCacheLookup, BindingConsistency, FulfillmentCacheRequiresEvaluation | 🔵 | 🔵 |
| domain::tddd::test_obligation::errors::ObligationEvaluateError | error_type | modify | TrackNotActive, CatalogueLoad, SpecLoad, ArtifactLoad, BindingConsistency, TestSourceScan, VerifierPort, CachePersistence, FulfillmentCacheLookup, WaiverCacheLookup, SemanticFailuresConfirmed, HumanEscalationRequired | 🔵 | 🔵 |
| domain::tddd::test_obligation::verdict::WaiverCacheLookupError | error_type | add | AmbiguousCurrentEntries | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ObligationFulfillmentVerifierPort | secondary_port | reference | fn verify_pair(&self, pair: &ObligationFulfillmentPair, tier: ModelTier) -> Result<ObligationFulfillmentVerdict, SemanticVerifierError> | 🔵 | 🔵 |
| WaiverVerifierPort | secondary_port | modify | fn verify_pair(&self, pair: &WaiverPair, tier: ModelTier) -> Result<WaiverVerdict, SemanticVerifierError> | 🔵 | 🔵 |

