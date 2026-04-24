<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TdddLayerBinding | value_object | modify | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueSpecSignalsCodecError | error_type | — | Json, UnsupportedSchemaVersion, Validation | 🔵 | 🔵 |
| LoadCatalogueSpecSignalsForViewError | error_type | — | NotFound, NotRegularFile, Io, Decode, StaleHash | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueSpecSignalsDocumentDto | dto | — | — | 🔵 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| GitShowTrackBlobReader | secondary_adapter | modify | impl TrackBlobReader, impl SpecElementHashReader | 🔵 | 🔵 |
| FsCatalogueSpecSignalsStore | secondary_adapter | — | impl CatalogueSpecSignalsWriter | 🔵 | 🔵 |

