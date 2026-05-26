<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| NextCommand | enum | modify | Implement, Done, PlanNewFeature, Status | 🔵 | 🔵 |
| TrackPhase | enum | modify | Planning, InProgress, ReadyToShip, Blocked, Cancelled, Archived | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| domain::track_phase::next_command | free_function | modify | fn(track: &TrackMetadata, impl_plan: Option<&ImplPlanDocument>) -> NextCommand | 🔵 | 🔵 |
| domain::track_phase::resolve_phase | free_function | modify | fn(track: &TrackMetadata, impl_plan: Option<&ImplPlanDocument>) -> TrackPhaseInfo | 🔵 | 🔵 |
| domain::track_phase::resolve_phase_from_record | free_function | modify | fn(status: TrackStatus, override_reason: Option<&str>) -> TrackPhaseInfo | 🔵 | 🔵 |

