<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal |
|------|------|--------|---------|--------|
| CatalogueSpecSignalsCodecError | error_type | — | Json, UnsupportedSchemaVersion, Validation | 🟡 |

## DTOs

| Name | Kind | Action | Details | Signal |
|------|------|--------|---------|--------|
| CatalogueSpecSignalsDocumentDto | dto | — | — | 🟡 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal |
|------|------|--------|---------|--------|
| GitShowTrackBlobReader | secondary_adapter | modify | impl TrackBlobReader | 🟡 |
| FsCatalogueSpecSignalsStore | secondary_adapter | — | impl CatalogueSpecSignalsWriter | 🟡 |

