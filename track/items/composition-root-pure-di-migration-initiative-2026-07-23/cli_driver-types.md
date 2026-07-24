<!-- Generated from cli_driver-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrBaselineInput | enum | modify | Snapshot, Restore, CheckReview, CheckCommit | 🔵 | 🔵 |
| AdrBaselineSnapshotInput | enum | add | Init, Cite, NewAdr, NonSemanticFix, Escalation | 🔵 | 🔵 |

## Primary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrBaselineDriver | primary_adapter | modify | — | 🔵 | 🔵 |
| CatalogDriver | primary_adapter | modify | — | 🔵 | 🔵 |
| TaskContractDriver | primary_adapter | reference | — | 🔵 | 🔵 |
| TemplateDriver | primary_adapter | reference | — | 🔵 | 🔵 |

