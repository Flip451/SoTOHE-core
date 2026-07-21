<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ExecutionModeDto | enum | add | OrchestratorOutput, TypedPipeline | 🔵 | 🔵 |
| SandboxMode | enum | add | ReadOnly, WorkspaceWrite | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AgentProfilesError | error_type | modify | Io, Symlink, PathOutsideTrustedRoot, Parse, UnsupportedSchemaVersion, InvalidCapability | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityConfigDto | dto | modify | — | 🔵 | 🔵 |
| ModelNameDto | dto | add | — | 🔵 | 🔵 |
| ProviderNameDto | dto | add | — | 🔵 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AgentProfiles | secondary_adapter | modify | impl Debug | 🔵 | 🔵 |
| AgentProfilesCapabilityAdapter | secondary_adapter | add | impl CapabilityProfilePort | 🔵 | 🔵 |
| ClaudeCapabilityAdapter | secondary_adapter | add | impl CapabilityProviderPort | 🔵 | 🔵 |
| CodexCapabilityAdapter | secondary_adapter | add | impl CapabilityProviderPort | 🔵 | 🔵 |
| FsCapabilitySourceAdapter | secondary_adapter | add | impl CapabilitySourcePort | 🔵 | 🔵 |

