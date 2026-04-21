<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal |
|------|------|--------|---------|--------|
| InformalGroundKind | enum | — | Discussion, Feedback, Memory, UserDirective | 🔵 |
| SpecStatus | enum | delete | Draft, Approved | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal |
|------|------|--------|---------|--------|
| SpecElementId | value_object | — | — | 🔵 |
| AdrAnchor | value_object | — | — | 🔵 |
| ConventionAnchor | value_object | — | — | 🔵 |
| ContentHash | value_object | — | — | 🔵 |
| InformalGroundSummary | value_object | — | — | 🔵 |
| AdrRef | value_object | — | — | 🔵 |
| ConventionRef | value_object | — | — | 🔵 |
| SpecRef | value_object | — | — | 🔵 |
| InformalGroundRef | value_object | — | — | 🔵 |
| ImplPlanDocument | value_object | — | — | 🔵 |
| TaskCoverageDocument | value_object | — | — | 🔵 |
| SpecDocument | value_object | modify | — | 🔵 |
| SpecRequirement | value_object | modify | — | 🔵 |
| TrackMetadata | value_object | modify | — | 🔵 |
| TypeCatalogueEntry | value_object | modify | — | 🔵 |
| CoverageResult | value_object | delete | — | 🟡 |

## Error Types

| Name | Kind | Action | Details | Signal |
|------|------|--------|---------|--------|
| SpecValidationError | error_type | modify | EmptyTitle, EmptyVersion, EmptyRequirementText, EmptyDomainStateName, EmptySectionTitle | 🔵 |
| ValidationError | error_type | modify | InvalidSpecElementId, EmptyAdrAnchor, EmptyConventionAnchor, InvalidContentHash, EmptyInformalGroundSummary, MultiLineInformalGroundSummary | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal |
|------|------|--------|---------|--------|
| ImplPlanReader | secondary_port | — | fn load_impl_plan(&self, id: &TrackId) -> Result<Option<ImplPlanDocument>, RepositoryError> | 🔵 |
| ImplPlanWriter | secondary_port | — | fn save_impl_plan(&self, id: &TrackId, doc: &ImplPlanDocument) -> Result<(), RepositoryError> | 🔵 |

