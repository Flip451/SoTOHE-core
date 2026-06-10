<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RefVerifyInvocationContext | enum | — | SpecDesign, TypeDesign, CommitGate, Standalone | 🔵 | 🔵 |
| RoundType | enum | reference | Final, Fast | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AgentExecutionRunner | value_object | — | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RefVerifyScopeResolverError | error_type | — | Io, InvalidLayerId, PartialCatalogues | 🔵 | 🔵 |
| SemanticVerifyCodecError | error_type | — | Json, UnsupportedSchemaVersion, Validation | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityConfigDto | dto | modify | — | 🔵 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AgentProfiles | secondary_adapter | modify | impl Debug | 🔵 | 🔵 |
| AgentRefVerifierAdapter | secondary_adapter | — | impl Debug, impl RefVerifierPort | 🔵 | 🔵 |
| CatalogueSpecVerifyCacheDocumentCodec | secondary_adapter | — | impl Debug | 🔵 | 🔵 |
| RefVerifyCacheAdapter | secondary_adapter | — | impl Debug, impl RefVerifyCachePort | 🔵 | 🔵 |
| RefVerifyPairSourceAdapter | secondary_adapter | — | impl Debug, impl RefVerifyPairSourcePort | 🔵 | 🔵 |
| RefVerifyScopeResolver | secondary_adapter | — | impl Debug | 🔵 | 🔵 |
| SpecAdrVerifyCacheDocumentCodec | secondary_adapter | — | impl Debug | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::ref_verify::process_runner::build_claude_ref_verifier_args | free_function | — | fn(model: &str, prompt: &str) -> Vec<std::ffi::OsString> | 🔵 | 🔵 |
| infrastructure::ref_verify::process_runner::build_codex_ref_verifier_args | free_function | — | fn(model: &str, prompt: &str, output_schema: &std::path::Path, output_last_message: &std::path::Path) -> Vec<std::ffi::OsString> | 🔵 | 🔵 |
| infrastructure::ref_verify::process_runner::build_gemini_ref_verifier_args | free_function | — | fn(model: &str, prompt: &str) -> Vec<std::ffi::OsString> | 🔵 | 🔵 |
| infrastructure::ref_verify::process_runner::make_ref_verifier_process_runner | free_function | — | fn(project_root: std::path::PathBuf) -> std::sync::Arc<AgentExecutionRunner> | 🔵 | 🔵 |

