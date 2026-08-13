<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityProviderBindingDto | enum | add | Standard, CodexCustom | 🟡 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityConfigDto | dto | modify | — | 🟡 | 🔵 |
| ModelProviderNameDto | dto | add | — | 🟡 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AgentProfilesCapabilityAdapter | secondary_adapter | reference | impl CapabilityProfilePort | 🔵 | 🔵 |
| CodexCapabilityAdapter | secondary_adapter | reference | impl CapabilityProviderPort | 🔵 | 🔵 |

