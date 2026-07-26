<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PrCommand | enum | add | Push, Ensure, Status, WaitAndMerge, TriggerReview, PollReview, ReviewCycle | 🟡 | 🔵 |
| PrReviewCycleMode | enum | add | Start, Resume | 🟡 | 🔵 |
| SignalCommand | enum | add | CalcAdrUser, CheckAdrUser, CalcSpecAdr, CheckSpecAdr, CalcCatalogSpec, CheckCatalogSpec, CalcImplCatalog, CheckImplCatalog, CheckGate | 🟡 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PrIdentifier | value_object | add | — | 🟡 | 🔵 |
| PrPollIntervalSeconds | value_object | add | — | 🟡 | 🔵 |
| PrPollTimeoutSeconds | value_object | add | — | 🟡 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PrCommandPort | secondary_port | add | fn execute(&self, command: PrCommand) -> PrCommandOutput | 🟡 | 🔵 |
| SignalCommandPort | secondary_port | add | fn execute(&self, command: SignalCommand) -> SignalCommandOutput | 🟡 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PrCommandInteractor | interactor | modify | — | 🟡 | 🔵 |
| SignalCommandInteractor | interactor | add | — | 🟡 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PrCommandOutput | dto | reference | — | 🔵 | 🔵 |
| SignalCommandOutput | dto | reference | — | 🔵 | 🔵 |

