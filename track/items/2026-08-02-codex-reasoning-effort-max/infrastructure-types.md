<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ReasoningEffortDto | enum | reference | Low, Medium, High, XHigh, Max | 🔵 | 🔵 |
| ResolvedExecution | enum | reference | ProviderCli, HostedService | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AgentProfilesError | error_type | reference | Io, Symlink, PathOutsideTrustedRoot, Parse, UnsupportedSchemaVersion, InvalidCapability, CapabilityNotFound, ModelMissing, EffortMissing, UnsupportedEffort | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityConfigDto | dto | reference | — | 🔵 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AgentProfiles | secondary_adapter | modify | impl Debug | 🔵 | 🔵 |

