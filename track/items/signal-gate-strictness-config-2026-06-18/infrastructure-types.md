<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| StrictnessDto | enum | — | Strict, Interim | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| SignalGatesConfigError | error_type | — | FileNotFound, ParseFailed, SchemaVersionUnknown, MissingKey, InvalidValue, BlobFetchError | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| GateRowDto | dto | — | — | 🔵 | 🔵 |
| SignalGatesConfig | dto | — | — | 🔵 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| GitBlobAdrFileAdapter | secondary_adapter | — | impl Debug, impl Clone, impl AdrFilePort | 🔵 | 🔵 |
| LocalSignalLayerReaderAdapter | secondary_adapter | — | impl Debug, impl SignalLayerReader | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::verify::adr_signals::execute_verify_adr_signals | free_function | modify | fn(project_root: &std::path::Path) -> domain::verify::VerifyOutcome | 🔵 | 🔵 |
| infrastructure::verify::adr_signals::execute_verify_adr_signals_with_strict | free_function | — | fn(project_root: &std::path::Path, strict: bool) -> domain::verify::VerifyOutcome | 🔵 | 🔵 |
| infrastructure::verify::catalogue_spec_signals::check_catalog_spec_from_signals_file | free_function | — | fn(signals_path: &std::path::Path, catalog_hash_hex: &str, strict: bool) -> domain::verify::VerifyOutcome | 🔵 | 🔵 |
| infrastructure::verify::catalogue_spec_signals::compute_catalogue_declaration_hash | free_function | — | fn(catalogue_bytes: &[u8]) -> String | 🔵 | 🔵 |
| infrastructure::verify::catalogue_spec_signals::compute_catalogue_entry_hash | free_function | — | fn(catalogue_json: &str, section: &str, entry_key: &str) -> Result<String, String> | 🔵 | 🔵 |
| infrastructure::verify::catalogue_spec_signals::execute_catalogue_spec_signals | free_function | modify | fn(items_dir: std::path::PathBuf, track_id: String, workspace_root: std::path::PathBuf, strict: bool) -> domain::verify::VerifyOutcome | 🔵 | 🔵 |
| infrastructure::verify::catalogue_spec_signals::execute_catalogue_spec_signals_check | free_function | modify | fn(items_dir: std::path::PathBuf, workspace_root: std::path::PathBuf, strict: bool) -> domain::verify::VerifyOutcome | 🔵 | 🔵 |
| infrastructure::verify::signal_gates_config::load_signal_gates_config | free_function | — | fn(config_path: std::path::PathBuf) -> Result<domain::SignalGateMatrix, SignalGatesConfigError> | 🔵 | 🔵 |
| infrastructure::verify::signal_gates_config::load_signal_gates_config_from_branch | free_function | — | fn(repo_root: &std::path::Path, branch: &str) -> Result<domain::SignalGateMatrix, SignalGatesConfigError> | 🔵 | 🔵 |
| infrastructure::verify::spec_states::check_impl_catalog_from_signals_file | free_function | — | fn(signals_path: &std::path::Path, catalog_hash_hex: &str, strict: bool) -> domain::verify::VerifyOutcome | 🔵 | 🔵 |
| infrastructure::verify::spec_states::verify_from_spec_json | free_function | modify | fn(spec_json_path: std::path::PathBuf, strict: bool, trusted_root: std::path::PathBuf) -> domain::verify::VerifyOutcome | 🔵 | 🔵 |
| infrastructure::verify::spec_states::verify_type_signals_from_spec_json | free_function | — | fn(spec_json_path: std::path::PathBuf, strict: bool, trusted_root: std::path::PathBuf) -> domain::verify::VerifyOutcome | 🔵 | 🔵 |

