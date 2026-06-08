<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RoundType | enum | reference | Final, Fast | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AgentExecutionRunner | value_object | — | — | 🟡 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| SemanticVerifyCodecError | error_type | — | Json, UnsupportedSchemaVersion, Validation | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityConfigDto | dto | modify | — | 🔵 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AgentProfiles | secondary_adapter | modify | impl Debug | 🔵 | 🔵 |
| AgentRefVerifierAdapter | secondary_adapter | — | impl Debug, impl RefVerifierPort | 🟡 | 🔵 |
| CatalogueSpecVerifyCacheDocumentCodec | secondary_adapter | — | impl Debug | 🟡 | 🔵 |
| RefVerifyCacheAdapter | secondary_adapter | — | impl Debug, impl RefVerifyCachePort | 🟡 | 🔵 |
| RefVerifyPairSourceAdapter | secondary_adapter | — | impl Debug, impl RefVerifyPairSourcePort | 🟡 | 🔵 |
| SpecAdrVerifyCacheDocumentCodec | secondary_adapter | — | impl Debug | 🟡 | 🔵 |

