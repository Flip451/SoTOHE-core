<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CommitHash | value_object | reference | — | 🔵 | 🔵 |
| TrackId | value_object | reference | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| WorktreeError | error_type | reference | StatusFailed | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| WorktreeReader | secondary_port | reference | fn porcelain_status(&self) -> Result<String, WorktreeError> | 🔵 | 🔵 |

