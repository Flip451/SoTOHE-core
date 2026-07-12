<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityDispatchOutcome | enum | add | Executed, DelegateInHost | 🟡 | 🔵 |
| ExecutionMode | enum | add | OrchestratorOutput, TypedPipeline | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BriefingText | value_object | add | — | 🔵 | 🔵 |
| CapabilityFailureDetail | value_object | add | — | 🔵 | 🔵 |
| CapabilityFilePath | value_object | add | — | 🔵 | 🔵 |
| CapabilityName | value_object | reference | — | 🔵 | 🔵 |
| DisciplineText | value_object | add | — | 🔵 | 🔵 |
| ModelName | value_object | add | — | 🔵 | 🔵 |
| ProviderName | value_object | add | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityExecError | error_type | add | ProfileResolution, ExecutionModeRejected, ModelMissing, UnsupportedProvider, SourceValidation, AdapterPreflight, DispatchFailed | 🔵 | 🔵 |
| CapabilityInputValidationError | error_type | add | EmptyCapabilityName, EmptyProviderName, EmptyModelName, EmptyFilePath, EmptyContent | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityProfilePort | secondary_port | add | fn resolve(&self, capability: &CapabilityName) -> Result<CapabilityProfile, CapabilityExecError> | 🟡 | 🔵 |
| CapabilityProviderPort | secondary_port | add | fn provider(&self) -> &ProviderName, fn dispatch(&self, request: &CapabilityDispatchRequest) -> Result<CapabilityDispatchOutcome, CapabilityExecError> | 🟡 | 🔵 |
| CapabilitySourcePort | secondary_port | add | fn load_briefing(&self, path: &CapabilityFilePath) -> Result<BriefingText, CapabilityExecError>, fn load_discipline(&self) -> Result<DisciplineText, CapabilityExecError> | 🟡 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityExecService | application_service | add | fn execute(&self, request: CapabilityExecRequest) -> Result<CapabilityDispatchOutcome, CapabilityExecError> | 🟡 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityExecInteractor | interactor | add | — | 🟡 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityDispatchRequest | dto | add | — | 🟡 | 🔵 |
| CapabilityProfile | dto | add | — | 🔵 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityExecRequest | command | add | — | 🔵 | 🔵 |

