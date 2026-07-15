<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ExecutionModeDto | enum | reference | OrchestratorOutput, TypedPipeline | 🔵 | 🔵 |
| ReasoningEffortDto | enum | add | Low, Medium, High, XHigh, Max | 🔵 | 🔵 |
| ResolvedExecution | enum | modify | ProviderCli, HostedService | 🔵 | 🔵 |
| RoundType | enum | reference | Final, Fast | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AgentProfilesError | error_type | modify | Io, Symlink, PathOutsideTrustedRoot, Parse, UnsupportedSchemaVersion, InvalidCapability, CapabilityNotFound, ModelMissing, EffortMissing, UnsupportedEffort | 🔵 | 🔵 |
| EvaluateSignalsError | error_type | reference | — | 🔵 | 🔵 |
| LoadCatalogueSpecSignalsForViewError | error_type | modify | NotFound, NotRegularFile, Io, Decode, StaleHash | 🔵 | 🔵 |
| TypeSignalsCodecError | error_type | modify | Json, UnsupportedSchemaVersion, InvalidSchemaVersion, InvalidTimestamp, InvalidDigest | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityConfigDto | dto | modify | — | 🔵 | 🔵 |
| ModelNameDto | dto | reference | — | 🔵 | 🔵 |
| ProviderNameDto | dto | reference | — | 🔵 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AgentProfiles | secondary_adapter | modify | impl Debug | 🔵 | 🔵 |
| AgentProfilesCapabilityAdapter | secondary_adapter | reference | impl CapabilityProfilePort | 🔵 | 🔵 |
| ClaudeCapabilityAdapter | secondary_adapter | modify | impl CapabilityProviderPort | 🟡 | 🔵 |
| ClaudeReviewer | secondary_adapter | modify | impl Reviewer | 🔵 | 🔵 |
| CodexCapabilityAdapter | secondary_adapter | modify | impl CapabilityProviderPort | 🟡 | 🔵 |
| CodexReviewer | secondary_adapter | modify | impl Reviewer | 🔵 | 🔵 |
| FsProviderSessionCacheAdapter | secondary_adapter | add | impl ProviderSessionCachePort | 🔵 | 🔵 |
| RustdocSchemaExporter | secondary_adapter | modify | impl SchemaExporter, impl SchemaExporterPort | 🔵 | 🔵 |
| TypeSignalsExecutorAdapter | secondary_adapter | modify | impl TypeSignalsExecutorPort, impl TypeSignalsExecutorPort, impl Debug, impl Default | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::ref_verify::process_runner::build_claude_ref_verifier_args | free_function | modify | fn(model: &str, effort: usecase::capability_exec::ReasoningEffort, prompt: &str) -> Vec<std::ffi::OsString> | 🔵 | 🔵 |
| infrastructure::ref_verify::process_runner::build_codex_ref_verifier_args | free_function | modify | fn(model: &str, effort: usecase::capability_exec::ReasoningEffort, prompt: &str, output_schema: &std::path::Path, output_last_message: &std::path::Path) -> Vec<std::ffi::OsString> | 🔵 | 🔵 |
| infrastructure::ref_verify::process_runner::build_gemini_ref_verifier_args | free_function | modify | fn(model: &str, effort: usecase::capability_exec::ReasoningEffort, prompt: &str) -> Result<Vec<std::ffi::OsString>, usecase::ref_verify::RefVerifyError> | 🔵 | 🔵 |
| infrastructure::tddd::type_signals_codec::declaration_hash | free_function | modify | fn(declaration_bytes: &[u8]) -> domain::tddd::type_signals_doc::CatalogueDeclarationHash | 🔵 | 🔵 |
| infrastructure::tddd::type_signals_codec::decode | free_function | reference | fn(json: &str) -> Result<domain::tddd::type_signals_doc::TypeSignalsDocument, TypeSignalsCodecError> | 🔵 | 🔵 |
| infrastructure::tddd::type_signals_codec::encode | free_function | reference | fn(doc: &domain::tddd::type_signals_doc::TypeSignalsDocument) -> Result<String, TypeSignalsCodecError> | 🔵 | 🔵 |
| infrastructure::tddd::type_signals_evaluator::execute_type_signals_for_layer | free_function | modify | fn(items_dir: &std::path::Path, track_id: &domain::ids::TrackId, workspace_root: &std::path::Path, binding: &TdddLayerBinding) -> Result<std::process::ExitCode, EvaluateSignalsError> | 🔵 | 🔵 |
| infrastructure::verify::catalogue_spec_signals::compute_catalogue_declaration_hash | free_function | modify | fn(catalogue_bytes: &[u8]) -> domain::tddd::type_signals_doc::CatalogueDeclarationHash | 🔵 | 🔵 |

