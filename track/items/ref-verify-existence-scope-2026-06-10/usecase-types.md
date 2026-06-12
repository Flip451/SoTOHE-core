<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RefVerifyScope | enum | reference | Chain1, Chain2, All | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RefVerifierPort | secondary_port | reference | fn verify_pair(&self, claim: String, evidence: String, cache_scope: &RefVerifyCacheScope, tier: domain::tddd::semantic_verify::ModelTier) -> Result<domain::tddd::semantic_verify::SemanticVerdict, RefVerifyError> | 🔵 | 🔵 |
| RefVerifyCachePort | secondary_port | reference | fn load_entries(&self, cmd: &RefVerifyCommand, cache_scope: &RefVerifyCacheScope) -> Result<Vec<domain::tddd::semantic_verify::SemanticVerifyEntry>, RefVerifyError>, fn save_entries(&self, cmd: &RefVerifyCommand, cache_scope: &RefVerifyCacheScope, entries: Vec<domain::tddd::semantic_verify::SemanticVerifyEntry>) -> Result<(), RefVerifyError> | 🔵 | 🔵 |
| RefVerifyPairSourcePort | secondary_port | reference | fn load_pairs(&self, cmd: &RefVerifyCommand, config: &RefVerifyConfig) -> Result<Vec<RefVerifyPair>, RefVerifyError> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RefVerifyApplicationService | application_service | reference | fn execute(&self, cmd: &RefVerifyCommand) -> Result<(), RefVerifyError> | 🔵 | 🔵 |

