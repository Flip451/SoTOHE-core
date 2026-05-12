<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueImplSignalsError | error_type | — | — | 🔵 | 🔵 |
| RenderContractMapError | error_type | modify | — | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueSpecSignalsWriter | secondary_port | reference | — | 🔵 | 🔵 |
| SchemaExporterPort | secondary_port | reference | — | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueImplSignalsService | application_service | — | — | 🔵 | 🔵 |
| PreCommitTypeSignalsService | application_service | reference | — | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueImplSignalsInteractor | interactor | — | — | 🔵 | 🔵 |
| PreCommitTypeSignalsInteractor | interactor | reference | — | 🔵 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RenderContractMapCommand | command | modify | — | 🔵 | 🔵 |

