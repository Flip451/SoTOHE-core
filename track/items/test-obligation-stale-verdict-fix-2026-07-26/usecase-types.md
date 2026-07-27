<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ResolvedBoundTests | value_object | add | — | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ContentHasherPort | secondary_port | reference | fn sha256(&self, bytes: &[u8]) -> domain::ContentHash | 🔵 | 🔵 |
| ObligationFulfillmentCachePort | secondary_port | add | fn load(&self, track_id: &domain::TrackId) -> Result<Option<domain::tddd::test_obligation::verdict::ObligationFulfillmentCacheDocument>, domain::tddd::test_obligation::errors::VerifyCacheError>, fn save(&self, doc: &domain::tddd::test_obligation::verdict::ObligationFulfillmentCacheDocument) -> Result<(), domain::tddd::test_obligation::ids::DiagnosticMessage> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ResolvedBoundTestsResolverPort | application_service | add | fn resolve(&self, locations: domain::tddd::test_obligation::binding::NonEmptyTestLocations) -> Result<ResolvedBoundTests, domain::tddd::test_obligation::errors::TestSourceScanError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CheckTestObligationsInteractor | interactor | modify | — | 🔵 | 🔵 |
| EvaluateTestObligationsInteractor | interactor | modify | — | 🔵 | 🔵 |
| ResolvedBoundTestsResolver | interactor | add | — | 🔵 | 🔵 |
| TestObligationResultsInteractor | interactor | modify | — | 🔵 | 🔵 |

