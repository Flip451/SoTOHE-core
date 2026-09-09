<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| domain::tddd::test_obligation::pair::ObligationFulfillmentPair | value_object | modify | — | 🟡 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ObligationFulfillmentVerifierPort | secondary_port | modify | fn verify_pair(&self, pair: &ObligationFulfillmentPair, tier: ModelTier) -> Result<ObligationFulfillmentVerdict, SemanticVerifierError> | 🟡 | 🔵 |

