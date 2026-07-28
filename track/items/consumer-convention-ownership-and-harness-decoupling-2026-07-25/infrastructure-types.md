<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityIdField | dto | add | — | 🔵 | 🔵 |
| ConventionFrontMatterDto | dto | add | — | 🔵 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| FsConventionInventoryAdapter | secondary_adapter | add | impl ConventionInventoryPort, impl Default | 🟡 | 🔵 |
| FsConventionRequirementAdapter | secondary_adapter | add | impl ConventionRequirementPort, impl Default | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::conventions_resolve::parse_convention_front_matter | free_function | add | fn(document: &usecase::conventions_resolve::ConventionDocumentPath, content: &str) -> Result<ConventionFrontMatterDto, usecase::conventions_resolve::ConventionResolveError> | 🔵 | 🔵 |
| infrastructure::conventions_resolve::scan_convention_requirements | free_function | add | fn(project_root: &std::path::Path) -> Result<Vec<usecase::conventions_resolve::ConventionRequirement>, usecase::conventions_resolve::ConventionResolveError> | 🔵 | 🔵 |
| infrastructure::template_conventions::list_convention_documents | free_function | add | fn(tree_root: &std::path::Path) -> Result<Vec<usecase::conventions_resolve::ConventionDocumentPath>, usecase::template_conventions::ConventionShippingCheckError> | 🔵 | 🔵 |

