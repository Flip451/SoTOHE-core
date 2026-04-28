<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ReviewApprovalVerdict | enum | — | Approved, ApprovedWithBypass, Blocked | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ScopeRound | value_object | — | — | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ReviewExistsPort | secondary_port | — | fn review_json_exists(&self) -> Result<bool, ReviewReaderError> | 🔵 | 🔵 |

