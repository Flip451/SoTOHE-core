<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RunCatalogueLintError | error_type | — | CatalogueLoad, LintExecution, InvalidLayer | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RunCatalogueLint | application_service | — | fn execute(&self, cmd: RunCatalogueLintCommand) -> Result<Vec<CatalogueLintViolation>, RunCatalogueLintError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RenderContractMapInteractor | interactor | reference | — | 🔵 | 🔵 |
| RunCatalogueLintInteractor | interactor | — | — | 🟡 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RunCatalogueLintCommand | command | — | — | 🔵 | 🔵 |

