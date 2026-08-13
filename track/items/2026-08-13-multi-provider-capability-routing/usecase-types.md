<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityProviderBinding | enum | add | Standard, CodexCustom | 🟡 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ModelProviderName | value_object | add | — | 🟡 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityInputValidationError | error_type | modify | EmptyProviderName, EmptyModelName, EmptyModelProviderName, EmptyFilePath, InvalidFilePath, EmptyTargetArtifactSet, EmptyContent, ZeroTimeoutSeconds | 🟡 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityProfilePort | secondary_port | reference | fn resolve(&self, capability: &CapabilityName) -> Result<CapabilityProfile, CapabilityExecError> | 🔵 | 🔵 |
| CapabilityProviderPort | secondary_port | reference | fn provider(&self) -> &ProviderName, fn dispatch(&self, request: &CapabilityDispatchRequest) -> Result<CapabilityDispatchOutcome, CapabilityExecError> | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityProfile | dto | modify | — | 🟡 | 🔵 |

