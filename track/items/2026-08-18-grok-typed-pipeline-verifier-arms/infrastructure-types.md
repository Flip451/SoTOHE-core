<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| GrokOutputEnvelope | enum | reference | Succeeded, Failed | 🔵 | 🔵 |
| GrokSandbox | enum | reference | ReadOnly, Workspace, Strict, ProjectProfile | 🔵 | 🔵 |
| ResolvedExecution | enum | reference | ProviderCli, HostedService | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| GrokSandboxProfileName | value_object | reference | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| GrokEnvelopeError | error_type | reference | ProviderFailure | 🔵 | 🔵 |
| GrokSandboxProfileNameError | error_type | reference | Empty, Reserved | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| GrokCapabilityDefinition | dto | reference | — | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::ref_verify::process_runner::build_grok_ref_verifier_args | free_function | add | fn(model: &usecase::capability_exec::ModelName, effort: usecase::capability_exec::ReasoningEffort, sandbox: &GrokSandbox, resume_id: Option<&usecase::provider_session::ProviderSessionId>, prompt: &str) -> Vec<std::ffi::OsString> | 🟡 | 🔵 |
| infrastructure::ref_verify::process_runner::make_ref_verifier_process_runner | free_function | reference | fn(project_root: std::path::PathBuf) -> std::sync::Arc<AgentExecutionRunner> | 🔵 | 🔵 |

