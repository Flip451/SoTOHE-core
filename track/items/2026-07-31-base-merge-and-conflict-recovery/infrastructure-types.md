<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| FsBaseMergeCleanupAdapter | secondary_adapter | add | impl BaseMergeCleanupPort | 🟡 | 🔵 |
| FsBaseMergeContextAdapter | secondary_adapter | add | impl BaseMergeContextPort | 🟡 | 🔵 |
| FsBaseMergeGitAdapter | secondary_adapter | add | impl BaseMergeGitPort | 🟡 | 🔵 |
| FsGitStashAdapter | secondary_adapter | add | impl GitStashPort | 🟡 | 🔵 |
| TypeSignalsExecutorAdapter | secondary_adapter | reference | impl Debug, impl Default, impl TypeSignalsExecutorPort | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::tddd::type_signals_codec::baseline_hash | free_function | add | fn(baseline_bytes: &[u8]) -> domain::tddd::type_signals_doc::BaselineHash | 🟡 | 🔵 |
| infrastructure::tddd::type_signals_codec::decode | free_function | reference | fn(json: &str) -> Result<domain::tddd::type_signals_doc::TypeSignalsDocument, TypeSignalsCodecError> | 🔵 | 🔵 |
| infrastructure::tddd::type_signals_codec::encode | free_function | reference | fn(doc: &domain::tddd::type_signals_doc::TypeSignalsDocument) -> Result<String, TypeSignalsCodecError> | 🔵 | 🔵 |

