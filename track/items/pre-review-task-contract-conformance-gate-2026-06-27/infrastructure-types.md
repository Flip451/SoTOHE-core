<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TaskContractCodecError | error_type | add | Json, UnsupportedSchemaVersion, Validation | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ContractedEntryRefDto | dto | add | — | 🔵 | 🔵 |
| TaskContractDocumentDto | dto | add | — | 🔵 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| FsImplCatalogSignalReader | secondary_adapter | add | impl Debug, impl ImplCatalogSignalReaderPort | 🔵 | 🔵 |
| FsTaskContractReader | secondary_adapter | add | impl Debug, impl TaskContractReaderPort | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::task_contract_codec::decode | free_function | add | fn(bytes: &[u8]) -> Result<domain::task_contract::TaskContractDocument, TaskContractCodecError> | 🔵 | 🔵 |
| infrastructure::task_contract_codec::encode | free_function | add | fn(doc: &domain::task_contract::TaskContractDocument) -> Result<Vec<u8>, TaskContractCodecError> | 🔵 | 🔵 |

