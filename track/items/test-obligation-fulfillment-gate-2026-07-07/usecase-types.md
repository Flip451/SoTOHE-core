<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TestObligationChainLabel | enum | add | Fulfillment, Waiver | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RefVerifyCacheKey | value_object | add | — | 🔵 | 🔵 |
| RefVerifyConfig | value_object | modify | — | 🔵 | 🔵 |
| RefVerifyPercent | value_object | modify | — | 🔵 | 🔵 |
| SemanticCalibrationProbeConfig | value_object | add | — | 🔵 | 🔵 |
| TestObligationEvaluateConfig | value_object | add | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TestBindingsSkeletonError | error_type | add | ObligationsAbsent, ArtifactLoad | 🔵 | 🔵 |
| TestObligationEvaluateConfigError | error_type | add | InvalidInjectionRate, InvalidDetectionThreshold, InvalidParallelism | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ContentHasherPort | secondary_port | add | fn sha256(&self, bytes: &[u8]) -> domain::ContentHash | 🔵 | 🔵 |
| SemanticEscalationDriverPort | secondary_port | add | fn evaluate_with_escalation(&self, pair: &'a P, key: &'a K, initial_tier: domain::tddd::semantic_verify::ModelTier) -> SemanticEscalationFuture<'a, V, E> | 🔵 | 🔵 |
| SemanticEscalationVerdictBridge | secondary_port | add | fn project(&self, verdict: &V) -> domain::tddd::semantic_verify::SemanticVerdict | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CheckTestObligationsApplicationService | application_service | add | fn execute(&self, cmd: &CheckTestObligationsCommand) -> Result<CheckTestObligationsOutcome, domain::tddd::test_obligation::errors::ObligationCheckError> | 🔵 | 🔵 |
| DeriveTestObligationsApplicationService | application_service | add | fn execute(&self, cmd: &DeriveTestObligationsCommand) -> Result<(), domain::tddd::test_obligation::errors::ObligationDeriveError> | 🔵 | 🔵 |
| EvaluateTestObligationsApplicationService | application_service | add | fn execute(&self, cmd: &'a EvaluateTestObligationsCommand) -> EvaluateTestObligationsFuture<'a> | 🔵 | 🔵 |
| TestBindingsSkeletonApplicationService | application_service | add | fn execute(&self, cmd: &TestBindingsSkeletonCommand) -> Result<TestBindingsSkeletonOutcome, TestBindingsSkeletonError> | 🔵 | 🔵 |
| TestObligationResultsApplicationService | application_service | add | fn execute(&self, cmd: &TestObligationResultsCommand) -> Result<TestObligationResultsOutput, domain::tddd::test_obligation::errors::ObligationResultsError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CheckTestObligationsInteractor | interactor | add | — | 🔵 | 🔵 |
| DeriveTestObligationsInteractor | interactor | add | — | 🔵 | 🔵 |
| EvaluateTestObligationsInteractor | interactor | add | — | 🔵 | 🔵 |
| TestBindingsSkeletonInteractor | interactor | add | — | 🔵 | 🔵 |
| TestObligationResultsInteractor | interactor | add | — | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CheckTestObligationsOutcome | dto | add | — | 🔵 | 🔵 |
| EvaluateTestObligationsFuture | dto | add | — | 🔵 | 🔵 |
| EvaluateTestObligationsOutcome | dto | add | — | 🔵 | 🔵 |
| SemanticEscalationFuture | dto | add | — | 🔵 | 🔵 |
| TestBindingsSkeletonOutcome | dto | add | — | 🔵 | 🔵 |
| TestBindingsSkeletonRecord | dto | add | — | 🔵 | 🔵 |
| TestObligationLaneSummary | dto | add | — | 🔵 | 🔵 |
| TestObligationResultsOutput | dto | add | — | 🔵 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CheckTestObligationsCommand | command | add | — | 🔵 | 🔵 |
| DeriveTestObligationsCommand | command | add | — | 🔵 | 🔵 |
| EvaluateTestObligationsCommand | command | add | — | 🔵 | 🔵 |
| TestBindingsSkeletonCommand | command | add | — | 🔵 | 🔵 |
| TestObligationCatalogueCommandInput | command | add | — | 🔵 | 🔵 |
| TestObligationResultsCommand | command | add | — | 🔵 | 🔵 |

