<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CanonicalTypeIdentity | value_object | add | — | 🔵 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::tddd::type_ref_parser::SynTypeRefPathExtractorAdapter | secondary_adapter | add | — | 🔵 | 🔵 |
| infrastructure::track_lifecycle::tddd::catalogue_lint_active::SystemTrackCatalogueLintActiveAdapter | secondary_adapter | modify | — | 🔵 | 🔵 |
| infrastructure::track_lifecycle::tddd::lint::SystemTrackLintAdapter | secondary_adapter | modify | — | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::tddd::canonical_type_identity::canonicalize_catalogue_type_ref | free_function | add | fn(type_ref: &domain::tddd::catalogue_v2::identifiers::TypeRef, catalogue_crate: &domain::tddd::catalogue_v2::identifiers::CrateName, rustdoc_paths: &std::collections::HashMap<rustdoc_types::Id, rustdoc_types::ItemSummary>, generic_params: &[domain::tddd::catalogue_v2::identifiers::ParamName]) -> Result<CanonicalTypeIdentity, domain::tddd::new_typegraph_codec_error::NewTypeGraphCodecError> | 🔵 | 🔵 |

