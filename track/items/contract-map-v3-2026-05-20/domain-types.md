<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ContractMapContent | value_object | reference | — | 🔵 | 🔵 |
| ContractMapRenderOptions | value_object | reference | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueLoaderError | error_type | reference | CatalogueNotFound, LayerDiscoveryFailed, DecodeFailed, SymlinkRejected, IoError, TopologicalSortFailed | 🔵 | 🔵 |
| ContractMapRendererError | error_type | — | StyleConfigNotFound, StyleConfigInvalid, RenderFailed | 🟡 | 🔵 |
| ContractMapWriterError | error_type | reference | IoError, SymlinkRejected, TrackNotFound | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueLoader | secondary_port | reference | fn load_all(&self, track_id: TrackId) -> Result<(Vec<LayerId>, BTreeMap<LayerId, CatalogueDocument>), CatalogueLoaderError> | 🔵 | 🔵 |
| ContractMapRenderer | secondary_port | — | fn render(&self, catalogues: &[CatalogueDocument], layer_order: &[LayerId], opts: &ContractMapRenderOptions) -> Result<ContractMapContent, ContractMapRendererError> | 🟡 | 🔵 |
| ContractMapWriter | secondary_port | reference | fn write(&self, track_id: TrackId, content: &ContractMapContent) -> Result<(), ContractMapWriterError> | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| domain::tddd::contract_map_render::render_contract_map | free_function | delete | fn(catalogues: &BTreeMap<LayerId, CatalogueDocument>, layer_order: &[LayerId], opts: &ContractMapRenderOptions) -> ContractMapContent | 🟡 | 🔵 |

