<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ConventionShippingVerdict | enum | add | Conforming, UnsuppliedDocumentsShipped | 🟡 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityFailureDetail | value_object | reference | — | 🔵 | 🔵 |
| CapabilityFilePath | value_object | reference | — | 🔵 | 🔵 |
| CapabilityName | value_object | reference | — | 🔵 | 🔵 |
| ConventionDocumentPath | value_object | add | — | 🔵 | 🔵 |
| ConventionRequirement | value_object | add | — | 🟡 | 🔵 |
| ConventionResolution | value_object | add | — | 🟡 | 🔵 |
| DisciplineText | value_object | modify | — | 🟡 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityExecError | error_type | modify | ProfileResolution, ExecutionModeRejected, ModelMissing, EffortMissing, UnsupportedProvider, SourceValidation, AdapterPreflight, DispatchFailed, ConventionResolutionFailed | 🟡 | 🔵 |
| ConventionDocumentPathError | error_type | add | OutsideConventionRoot | 🔵 | 🔵 |
| ConventionResolveError | error_type | add | FrontMatterUnparseable, RequiredForNotStringArray, EmptyCapabilityId, DocumentPathOutsideRoot, DocumentUnreadable | 🔵 | 🔵 |
| ConventionShippingCheckError | error_type | add | ConventionRootMissing, TreeUnreadable, DocumentPathRejected | 🟡 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ConventionInventoryPort | secondary_port | add | fn list_conventions(&self, tree_root: &std::path::Path) -> Result<Vec<ConventionDocumentPath>, ConventionShippingCheckError> | 🟡 | 🔵 |
| ConventionRequirementPort | secondary_port | add | fn scan_requirements(&self, project_root: &std::path::Path) -> Result<Vec<ConventionRequirement>, ConventionResolveError> | 🟡 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ConventionResolveService | application_service | add | fn resolve(&self, query: ResolveConventionsQuery) -> Result<ConventionResolution, ConventionResolveError> | 🟡 | 🔵 |
| ConventionShippingCheckService | application_service | add | fn check(&self, query: CheckConventionShippingQuery) -> Result<ConventionShippingVerdict, ConventionShippingCheckError> | 🟡 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityExecInteractor | interactor | modify | — | 🟡 | 🔵 |
| ConventionResolveInteractor | interactor | add | — | 🟡 | 🔵 |
| ConventionShippingCheckInteractor | interactor | add | — | 🟡 | 🔵 |

## Queries

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CheckConventionShippingQuery | query | add | — | 🟡 | 🔵 |
| ResolveConventionsQuery | query | add | — | 🟡 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| usecase::conventions_resolve::select_required_conventions | free_function | add | fn(requirements: &[ConventionRequirement], capability: &CapabilityName) -> ConventionResolution | 🟡 | 🔵 |
| usecase::template_conventions::select_unsupplied_conventions | free_function | add | fn(shipped: &[ConventionDocumentPath], supplied: &[ConventionDocumentPath]) -> ConventionShippingVerdict | 🟡 | 🔵 |

