<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TelemetryEvent | enum | reference | TrackSubcommand, GateEval, ReviewRound, ExternalSubprocess, HookBlock, AdvisoryHookFired, NonZeroExit | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DryCheckConfigError | error_type | modify | Io, Parse, UnsupportedSchemaVersion, InvalidThreshold, InvalidParallelism, InvalidReasoningEffort | 🟡 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DryCheckConfig | dto | modify | — | 🟡 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CodexDryChecker | secondary_adapter | modify | impl Debug, impl DryCheckAgentPort | 🟡 | 🔵 |
| FsDryCheckCoverageAdapter | secondary_adapter | — | impl DryCheckCoveragePort, impl Debug | 🟡 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::codex_common::build_codex_read_only_invocation | free_function | — | fn(model: &str, reasoning_effort: &str, prompt: &str, output_last_message: &std::path::Path, output_schema: &std::path::Path) -> Vec<std::ffi::OsString> | 🔵 | 🔵 |
| infrastructure::dry_check::corpus::compute_corpus_fingerprint | free_function | — | fn(workspace_root: &std::path::Path) -> domain::dry_check::DryCheckCorpusFingerprint | 🔵 | 🔵 |
| infrastructure::dry_check::corpus::sha256_hex | free_function | — | fn(data: &[u8]) -> String | 🔵 | 🔵 |

