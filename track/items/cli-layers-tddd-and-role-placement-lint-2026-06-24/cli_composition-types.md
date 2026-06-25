<!-- Generated from cli_composition-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CompositionError | error_type | reference | ConfigLoad, AdapterInit, WiringFailed, Usecase, Infrastructure | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CommandOutcome | dto | delete | — | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| cli_composition::build_codex_read_only_invocation | free_function | delete | fn() -> Vec<std::ffi::OsString> | 🔵 | 🔵 |
| cli_composition::tee_stderr_to_file | free_function | delete | fn(pipe: std::process::ChildStderr, log_file: std::fs::File) -> () | 🔵 | 🔵 |

## Composition Roots

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ArchCompositionRoot | composition_root | modify | — | 🔵 | 🔵 |
| ConventionsCompositionRoot | composition_root | modify | — | 🔵 | 🔵 |
| DemoCompositionRoot | composition_root | reference | — | 🔵 | 🔵 |
| DomainCompositionRoot | composition_root | reference | — | 🔵 | 🔵 |
| DryCompositionRoot | composition_root | reference | — | 🔵 | 🔵 |
| DryFixRunnerCompositionRoot | composition_root | reference | — | 🔵 | 🔵 |
| FileCompositionRoot | composition_root | modify | — | 🔵 | 🔵 |
| GitCompositionRoot | composition_root | reference | — | 🔵 | 🔵 |
| GuardCompositionRoot | composition_root | reference | — | 🔵 | 🔵 |
| HookCompositionRoot | composition_root | reference | — | 🔵 | 🔵 |
| PlanCompositionRoot | composition_root | add | — | 🔵 | 🔵 |
| PrCompositionRoot | composition_root | reference | — | 🔵 | 🔵 |
| RefVerifyCompositionRoot | composition_root | reference | — | 🔵 | 🔵 |
| ReviewCompositionRoot | composition_root | reference | — | 🔵 | 🔵 |
| SemanticDupCompositionRoot | composition_root | reference | — | 🔵 | 🔵 |
| SignalCompositionRoot | composition_root | reference | — | 🔵 | 🔵 |
| TelemetryCompositionRoot | composition_root | reference | — | 🔵 | 🔵 |
| TrackCompositionRoot | composition_root | reference | — | 🔵 | 🔵 |
| VerifyCompositionRoot | composition_root | modify | — | 🔵 | 🔵 |

