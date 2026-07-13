<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# 外部 provider 実行基盤の修復

## Summary

T001: modify the capability-exec timeout path and verify its D1 configuration and runbook baselines (GO-01, GO-02; AC-01, AC-02, AC-03).
T002: modify the Phase1Error vocabulary and add and wire the shared cargo-metadata resolver in `schema_export::bin_target` across its listed infrastructure consumers (GO-03; AC-04 through AC-10).

## Tasks (2/2 resolved)

### S1 — S1: capability-exec timeout propagation

> T001 modifies the listed usecase, infrastructure, cli-driver, and CLI targets; verifies the existing configuration and runbook baselines; then adds its focused tests and test-obligation bindings (AC-01, AC-02, AC-03; IN-01, IN-02, IN-03; CN-01, CN-02).

- [x] **T001**: Modify `libs/usecase/src/capability_exec.rs` (`TimeoutSeconds`, `CapabilityInputValidationError`, and `CapabilityExecRequest`), then wire the optional timeout through `libs/infrastructure/src/capability_exec/{claude,codex}.rs` (`ClaudeCapabilityAdapter` and `CodexCapabilityAdapter` `CapabilityProviderPort::dispatch`) and `libs/infrastructure/src/capability_exec/process.rs` (`ProviderProcessRunner::run`, `run_provider_process`, and `wait_for_provider_process`), `apps/cli-driver/src/capability.rs` (`TimeoutSecondsArg` and `CapabilityExecDriverInput`), and `apps/cli/src/commands/capability.rs` (`CapabilityExecArgs`). Verify the existing D1 baselines in `.codex/config.toml` (`unified_exec`) and `.harness/capabilities/review-fix-lead.md` (reviewer timeout handling); add focused regression tests and matching test-obligation bindings. AC-01, AC-02, AC-03; IN-01, IN-02, IN-03; CN-01, CN-02. (`915f6c443c2184e6e348e20f25f76e05c57913c9`)

### S2 — S2: cargo-metadata rustdoc-root resolver and consumers

> T002 modifies the listed Phase1Error vocabulary, including the validated DiagnosticMessage payload migration; adds the resolver function and its typed result and error contracts in `schema_export::bin_target`; and replaces the listed schema-export, catalog-import, and signal-evaluation consumer logic, then adds its focused tests and test-obligation bindings (AC-04 through AC-10; IN-04, IN-05, IN-06, IN-07; CN-03, CN-04).
> T001 and T002 may be applied as one prepared-patch batch; complete focused verification for both before the final Rust CI run.

- [x] **T002**: Modify `libs/domain/src/tddd/signal_evaluator/phase1_error.rs` (`Phase1Error`) to migrate the existing `ActionContradiction`, `UnresolvedTypeRef`, and `DanglingId` payloads to the validated `DiagnosticMessage` newtype and add `RustdocRootResolution(DiagnosticMessage)`, then add the shared `resolve_rustdoc_root_name` function with its `RustdocTargetResolution` and `RustdocRootResolutionError` contracts in `libs/infrastructure/src/schema_export/bin_target.rs`. Replace the local root-resolution logic in that module (`run_rustdoc` and `resolve_bin_target_name`), `libs/infrastructure/src/tddd/catalog_gen/import_shape.rs` (`select_type`), and `libs/infrastructure/src/tddd/signal_evaluator_v2/mod.rs` (`canonical_function_root_segment`). Add focused regression tests and matching test-obligation bindings. AC-04, AC-05, AC-06, AC-07, AC-08, AC-09, AC-10; IN-04, IN-05, IN-06, IN-07; CN-03, CN-04. (`915f6c443c2184e6e348e20f25f76e05c57913c9`)
