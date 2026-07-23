<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| FsAdrBaselineStore | secondary_adapter | reference | impl AdrBaselineStorePort, impl AdrBaselineStoreReadPort, impl From<std::path::PathBuf>, impl Debug, impl Clone | 🔵 | 🔵 |
| FsCatalogAdapter | secondary_adapter | modify | impl Debug, impl Default, impl CatalogPort | 🔵 | 🔵 |
| FsGitAdrBaselineSource | secondary_adapter | reference | impl AdrBaselineSourcePort, impl From<std::path::PathBuf>, impl Debug, impl Clone | 🔵 | 🔵 |
| FsImplCatalogSignalReader | secondary_adapter | modify | impl Debug, impl ImplCatalogSignalReaderPort | 🔵 | 🔵 |
| FsImplPlanReader | secondary_adapter | modify | impl Debug, impl ImplPlanReaderPort | 🔵 | 🔵 |
| FsSelfBinaryTransplantAdapter | secondary_adapter | reference | impl Debug, impl Default, impl SelfBinaryTransplantPort | 🔵 | 🔵 |
| FsTaskContractReader | secondary_adapter | modify | impl Debug, impl TaskContractReaderPort | 🔵 | 🔵 |
| FsTemplateBoundaryManifestAdapter | secondary_adapter | reference | impl Debug, impl Default, impl TemplateBoundaryManifestPort | 🔵 | 🔵 |
| FsTemplateExportAdapter | secondary_adapter | reference | impl Debug, impl TemplateExportPort | 🔵 | 🔵 |

