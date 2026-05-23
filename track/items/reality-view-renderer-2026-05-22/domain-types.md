<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BaselineDocument | value_object | — | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BaselineGraphLoaderError | error_type | — | NotFound, ParseFailed, IoError, SymlinkRejected, LayerDiscoveryFailed | 🔵 | 🔵 |
| BaselineGraphRendererError | error_type | — | StyleConfigNotFound, StyleConfigInvalid, RenderFailed | 🔵 | 🔵 |
| BaselineGraphWriterError | error_type | — | IoError, SymlinkRejected, TrackNotFound | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BaselineGraphLoader | secondary_port | — | fn load_all(&self, track_id: &TrackId) -> Result<Vec<BaselineDocument>, BaselineGraphLoaderError> | 🔵 | 🔵 |
| BaselineGraphRenderer | secondary_port | — | fn render_overview(&self, baselines: &[BaselineDocument], layer: &LayerId) -> Result<String, BaselineGraphRendererError>, fn render_clusters(&self, baselines: &[BaselineDocument], layer: &LayerId) -> Result<Vec<ClusterRender>, BaselineGraphRendererError> | 🔵 | 🔵 |
| BaselineGraphWriter | secondary_port | — | fn write_overview(&self, track_id: &TrackId, layer: &LayerId, content: &str) -> Result<(), BaselineGraphWriterError>, fn write_cluster(&self, track_id: &TrackId, layer: &LayerId, cluster_key: &str, content: &str) -> Result<(), BaselineGraphWriterError> | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ClusterRender | dto | — | — | 🔵 | 🔵 |

