<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| FrozenTrackStatus | enum | add | Done, Archived | 🔵 | 🔵 |
| TrackStatus | enum | modify | Planned, InProgress, Done, Blocked, Cancelled, Archived | 🔵 | 🔵 |
| TrackStatusReadFailureKind | enum | add | Unavailable | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ObligationDeriveError | error_type | modify | RulesLoad, TrackNotActive, TrackFrozen, TrackStatusRead, CatalogueLoad, SpecLoad, InvalidCatalogueState, ArtifactWrite | 🔵 | 🔵 |

