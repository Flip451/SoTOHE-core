<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueSectionKeyDto | enum | add | Types, Traits, Functions | 🟡 | 🔵 |
| SemanticVerdictDto | enum | add | Pass, Fail, Pending | 🟡 | 🔵 |
| SpecSectionKindDto | enum | add | Goal, InScope, OutOfScope, Constraint, AcceptanceCriteria | 🟡 | 🔵 |
| VerifyOriginRefDto | enum | add | SpecElement, AdrDecision, CatalogueEntry | 🟡 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrDecisionRefDto | dto | add | — | 🟡 | 🔵 |
| CatalogueEntryRefDto | dto | add | — | 🟡 | 🔵 |
| SemanticVerifyEntryDto | dto | add | — | 🟡 | 🔵 |
| SpecElementRefDto | dto | add | — | 🟡 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| FsRefVerifyAggregateAdapter | secondary_adapter | modify | impl RefVerifyAggregateService, impl Default | 🔵 | 🔵 |

