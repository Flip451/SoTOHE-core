<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RustdocTargetKind | enum | add | Library, Binary | 🟡 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CargoTargetName | value_object | add | — | 🟡 | 🔵 |
| RustdocTargetResolution | value_object | add | — | 🟡 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RustdocRootResolutionError | error_type | add | MetadataCommand, MetadataDecode, PackageNotFound, TargetSelection, InvalidTargetName | 🟡 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| SignalEvaluatorV2 | secondary_adapter | modify | impl Debug, impl Clone, impl Default, impl SignalEvaluatorPort | 🟡 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::schema_export::bin_target::resolve_rustdoc_root_name | free_function | add | fn(workspace_root: &std::path::Path, package_name: &domain::tddd::catalogue_v2::CrateName) -> Result<RustdocTargetResolution, RustdocRootResolutionError> | 🟡 | 🔵 |

