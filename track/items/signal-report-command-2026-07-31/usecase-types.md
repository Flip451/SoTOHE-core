<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| SignalReportChain | enum | add | AdrUser, SpecAdr, CatalogSpec, ImplCatalog | 🟡 | 🔵 |
| SignalReportChainSelection | enum | add | All, One | 🟡 | 🔵 |
| SignalReportLevel | enum | add | Yellow, Red | 🟡 | 🔵 |
| SignalReportLevelSelection | enum | add | YellowOnly, RedOnly, YellowAndRed | 🟡 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| SignalReportEntryId | value_object | add | — | 🟡 | 🔵 |
| SignalReportLocation | value_object | add | — | 🟡 | 🔵 |
| SignalReportReason | value_object | add | — | 🟡 | 🔵 |
| SignalReportReference | value_object | add | — | 🟡 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| SignalReportError | error_type | add | SourceUnavailable, InvalidOccurrence | 🟡 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| SignalReportSourcePort | secondary_port | add | fn load(&self, chain: SignalReportChain) -> Result<Vec<SignalReportOccurrence>, SignalReportError> | 🟡 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| SignalReportService | application_service | add | fn report(&self, query: SignalReportQuery) -> Result<SignalReportOutput, SignalReportError> | 🟡 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| SignalReportInteractor | interactor | add | — | 🟡 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| SignalReportOccurrence | dto | add | — | 🟡 | 🔵 |
| SignalReportOutput | dto | add | — | 🟡 | 🔵 |

## Queries

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| SignalReportQuery | query | add | — | 🟡 | 🔵 |

