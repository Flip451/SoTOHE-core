<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TemplatePathClassificationDto | enum | add | Include, Exclude, Overlay | 🟡 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TemplateBoundaryManifestCodecError | error_type | add | SchemaVersion, Json, Pattern, Manifest | 🟡 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TemplateBoundaryManifestDto | dto | add | — | 🟡 | 🔵 |
| TemplatePathEntryDto | dto | add | — | 🟡 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| FsTemplateBoundaryManifestAdapter | secondary_adapter | add | impl Debug, impl Default, impl TemplateBoundaryManifestPort | 🟡 | 🔵 |
| FsTemplateExportAdapter | secondary_adapter | add | impl Debug, impl Default, impl TemplateExportPort | 🟡 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::template_export::codec::decode_manifest | free_function | add | fn(json: &str) -> Result<domain::template_export::TemplateBoundaryManifest, TemplateBoundaryManifestCodecError> | 🟡 | 🔵 |

