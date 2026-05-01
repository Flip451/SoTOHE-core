<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| Decision | enum | reference | Allow, Block | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TrackId | value_object | reference | — | 🔵 | 🔵 |
| CommitHash | value_object | reference | — | 🔵 | 🔵 |
| CatalogueLintViolation | value_object | reference | — | 🔵 | 🔵 |
| TypeSignal | value_object | modify | — | 🔵 | 🔵 |
| AdrVerifyReport | value_object | reference | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TrackWriteError | error_type | reference | Domain, Repository | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TrackReader | secondary_port | reference | fn find(&self, id: &TrackId) -> Result<Option<TrackMetadata>, TrackReadError> | 🔵 | 🔵 |

