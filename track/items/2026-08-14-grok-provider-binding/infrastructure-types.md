<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| GrokOutputEnvelope | enum | add | Succeeded, Failed | 🔵 | 🔵 |
| GrokSandbox | enum | add | ReadOnly, Workspace, Strict, ProjectProfile | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| GrokSandboxProfileName | value_object | add | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| GrokEnvelopeError | error_type | add | ProviderFailure | 🔵 | 🔵 |
| GrokSandboxProfileNameError | error_type | add | Empty, Reserved | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| GrokCapabilityDefinition | dto | add | — | 🟡 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CodexDryFixLocalRunner | secondary_adapter | reference | impl Default | 🔵 | 🔵 |
| GrokCapabilityAdapter | secondary_adapter | add | impl CapabilityProviderPort | 🟡 | 🔵 |
| GrokDryChecker | secondary_adapter | add | impl DryCheckAgentPort, impl Debug | 🟡 | 🔵 |
| GrokReviewer | secondary_adapter | add | impl Reviewer | 🟡 | 🔵 |
| ReviewFixRunnerAdapter | secondary_adapter | reference | impl ReviewFixRunner | 🔵 | 🔵 |

