<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RefVerifyCacheScope | enum | — | SpecAdr, CatalogueSpec | 🟡 | 🔵 |
| RefVerifyScope | enum | — | Chain1, Chain2, All | 🟡 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RefVerifyConfig | value_object | — | — | 🟡 | 🔵 |
| RefVerifyPair | value_object | — | — | 🟡 | 🔵 |
| RefVerifyParallelism | value_object | — | — | 🟡 | 🔵 |
| RefVerifyPercent | value_object | — | — | 🟡 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RefVerifyError | error_type | — | InvalidConfig, TrackNotActive, VerifierPort, CachePersistence, SemanticFailuresConfirmed, HumanEscalationRequired | 🟡 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RefVerifierPort | secondary_port | — | fn verify_pair(&self, claim: String, evidence: String, tier: domain::tddd::semantic_verify::ModelTier) -> Result<domain::tddd::semantic_verify::SemanticVerdict, RefVerifyError> | 🟡 | 🔵 |
| RefVerifyCachePort | secondary_port | — | fn load_entries(&self, cmd: &RefVerifyCommand, cache_scope: &RefVerifyCacheScope) -> Result<Vec<domain::tddd::semantic_verify::SemanticVerifyEntry>, RefVerifyError>, fn save_entries(&self, cmd: &RefVerifyCommand, cache_scope: &RefVerifyCacheScope, entries: Vec<domain::tddd::semantic_verify::SemanticVerifyEntry>) -> Result<(), RefVerifyError> | 🟡 | 🔵 |
| RefVerifyPairSourcePort | secondary_port | — | fn load_pairs(&self, cmd: &RefVerifyCommand, config: &RefVerifyConfig) -> Result<Vec<RefVerifyPair>, RefVerifyError> | 🟡 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RefVerifyApplicationService | application_service | — | fn execute(&self, cmd: &RefVerifyCommand) -> Result<(), RefVerifyError> | 🟡 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| VerifySemanticRefsInteractor | interactor | — | — | 🟡 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RefVerifyCommand | command | — | — | 🟡 | 🔵 |

