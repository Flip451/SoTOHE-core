<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RoundType | enum | reference | Final, Fast | 🔵 | 🔵 |
| SemanticVerdictDto | enum | — | Pass, Fail, Pending | 🟡 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AgentExecutionRunner | value_object | — | — | 🟡 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| SemanticVerifyCodecError | error_type | — | Json, UnsupportedSchemaVersion, Validation | 🟡 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueSpecVerifyCacheDocumentDto | dto | — | — | 🟡 | 🔵 |
| SemanticVerifyEntryDto | dto | — | — | 🟡 | 🔵 |
| SpecAdrVerifyCacheDocumentDto | dto | — | — | 🟡 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AgentProfiles | secondary_adapter | reference | — | 🔵 | 🔵 |
| AgentRefVerifierAdapter | secondary_adapter | — | impl Debug, impl RefVerifierPort | 🟡 | 🔵 |
| CatalogueSpecVerifyCacheDocumentCodec | secondary_adapter | — | impl Debug | 🟡 | 🔵 |
| RefVerifyCacheAdapter | secondary_adapter | — | impl Debug, impl RefVerifyCachePort | 🟡 | 🔵 |
| RefVerifyPairSourceAdapter | secondary_adapter | — | impl Debug, impl RefVerifyPairSourcePort | 🟡 | 🔵 |
| SpecAdrVerifyCacheDocumentCodec | secondary_adapter | — | impl Debug | 🟡 | 🔵 |

