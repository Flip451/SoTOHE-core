<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RenderError | error_type | reference | Io, InvalidMetadata, OutOfSync, UnsupportedSchemaVersion, InvalidTrackMetadata | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::track::render::sync_rendered_views | free_function | modify | fn(root: &std::path::Path, track_id: Option<&str>) -> Result<Vec<std::path::PathBuf>, RenderError> | 🔵 | 🔵 |

