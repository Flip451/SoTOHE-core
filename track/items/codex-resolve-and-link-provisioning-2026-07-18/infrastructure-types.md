<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ResolvedCodexRuntime | value_object | add | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CodexRuntimeResolveError | error_type | add | ProjectRootInvalid, RepoLocalLinkInvalid, PathFallbackUnavailable, ProbeFailed | 🔵 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CodexCapabilityAdapter | secondary_adapter | modify | impl CapabilityProviderPort | 🔵 | 🔵 |
| CodexDryChecker | secondary_adapter | modify | impl DryCheckAgentPort, impl Debug | 🔵 | 🔵 |
| CodexDryFixLocalRunner | secondary_adapter | modify | impl Default | 🔵 | 🔵 |
| CodexReviewFixRunner | secondary_adapter | modify | impl ReviewFixRunner | 🔵 | 🔵 |
| CodexReviewer | secondary_adapter | modify | impl Reviewer | 🔵 | 🔵 |
| FsCodexRuntimeProvisioner | secondary_adapter | add | impl CodexRuntimeProvisionPort, impl Default | 🔵 | 🔵 |
| GitCodexRuntimeProjectRootDiscoveryAdapter | secondary_adapter | add | impl CodexRuntimeProjectRootDiscoveryPort, impl Default | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::codex_common::resolve_codex_runtime | free_function | add | fn(project_root: &std::path::Path) -> Result<ResolvedCodexRuntime, CodexRuntimeResolveError> | 🔵 | 🔵 |
| infrastructure::ref_verify::process_runner::make_ref_verifier_process_runner | free_function | modify | fn(project_root: std::path::PathBuf) -> std::sync::Arc<AgentExecutionRunner> | 🔵 | 🔵 |

