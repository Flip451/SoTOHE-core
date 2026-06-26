<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RefVerifyChainFilter | enum | add | Chain1, Chain2, All | 🟡 | 🔵 |
| RefVerifyLayerFilter | enum | add | Specific, All | 🟡 | 🔵 |
| RefVerifyVerdictFilter | enum | add | FailPending, Pass, Fail, Pending, All | 🟡 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RefVerifyPair | value_object | modify | — | 🟡 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RefVerifyAggregateService | application_service | modify | fn run(&self, track_id: &str, items_dir: &std::path::Path) -> Result<RefVerifyRunOutcome, RefVerifyDriverError>, fn check_approved(&self, track_id: &str, items_dir: &std::path::Path) -> Result<RefVerifyCheckApprovedOutcome, RefVerifyDriverError>, fn results(&self, track_id: &str, items_dir: &std::path::Path, chain: RefVerifyChainFilter, layer: RefVerifyLayerFilter, verdict: RefVerifyVerdictFilter) -> Result<RefVerifyResultsOutput, RefVerifyDriverError> | 🟡 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RefVerifyLaneSummary | dto | add | — | 🟡 | 🔵 |
| RefVerifyPairRecord | dto | add | — | 🟡 | 🔵 |
| RefVerifyResultsOutput | dto | add | — | 🟡 | 🔵 |

