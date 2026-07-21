<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrBaselineCodecError | error_type | add | Json, Domain | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrBaselineLedgerRecordDto | dto | add | — | 🔵 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| FsAdrBaselineStore | secondary_adapter | add | impl AdrBaselineStorePort, impl AdrBaselineStoreReadPort, impl From<std::path::PathBuf>, impl Debug, impl Clone | 🔵 | 🔵 |
| FsGitAdrBaselineSource | secondary_adapter | add | impl AdrBaselineSourcePort, impl From<std::path::PathBuf>, impl Debug, impl Clone | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::adr_baseline::decode_ledger_line | free_function | add | fn(line: &str) -> Result<domain::adr_baseline::AdrBaselineLedgerEntry, AdrBaselineCodecError> | 🔵 | 🔵 |
| infrastructure::adr_baseline::encode_ledger_entry | free_function | add | fn(entry: &domain::adr_baseline::AdrBaselineLedgerEntry) -> Result<String, AdrBaselineCodecError> | 🔵 | 🔵 |

