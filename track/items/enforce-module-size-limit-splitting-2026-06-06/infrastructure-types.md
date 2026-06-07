<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::track::render::collect_track_snapshots | free_function | delete | fn(root: &std::path::Path) -> Result<Vec<TrackSnapshot>, RenderError> | 🔵 | 🔵 |
| infrastructure::track::render::plan::render_plan | free_function | — | fn(track: &domain::TrackMetadata, impl_plan: Option<&domain::ImplPlanDocument>) -> String | 🔵 | 🔵 |
| infrastructure::track::render::registry::render_registry | free_function | — | fn(tracks: &[TrackSnapshot]) -> String | 🔵 | 🔵 |
| infrastructure::track::render::render_plan | free_function | delete | fn(track: &domain::TrackMetadata, impl_plan: Option<&domain::ImplPlanDocument>) -> String | 🔵 | 🔵 |
| infrastructure::track::render::render_registry | free_function | delete | fn(tracks: &[TrackSnapshot]) -> String | 🔵 | 🔵 |
| infrastructure::track::render::snapshot::collect_track_snapshots | free_function | — | fn(root: &std::path::Path) -> Result<Vec<TrackSnapshot>, RenderError> | 🔵 | 🔵 |
| infrastructure::track::render::sync::sync_rendered_views | free_function | — | fn(root: &std::path::Path, track_id: Option<&str>) -> Result<Vec<std::path::PathBuf>, RenderError> | 🔵 | 🔵 |
| infrastructure::track::render::sync_rendered_views | free_function | delete | fn(root: &std::path::Path, track_id: Option<&str>) -> Result<Vec<std::path::PathBuf>, RenderError> | 🔵 | 🔵 |
| infrastructure::track::render::validate::validate_track_snapshots | free_function | — | fn(root: &std::path::Path) -> Result<(), RenderError> | 🔵 | 🔵 |
| infrastructure::track::render::validate_track_snapshots | free_function | delete | fn(root: &std::path::Path) -> Result<(), RenderError> | 🔵 | 🔵 |
| infrastructure::verify::module_size::verify | free_function | modify | fn(root: &std::path::Path) -> domain::verify::VerifyOutcome | 🔵 | 🔵 |
| infrastructure::verify::plan_artifact_refs::build_element_map | free_function | delete | fn(raw: &serde_json::Value) -> std::collections::HashMap<String, String> | 🔵 | 🔵 |
| infrastructure::verify::plan_artifact_refs::canonical_json_sha256 | free_function | delete | fn(json: &str) -> String | 🔵 | 🔵 |

