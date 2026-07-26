<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# Support workflow composition roots への純 DI 移行 (RefVerify / SemanticDup / TestObligation / Verify)

## Summary

GO-01: T001, T002.
GO-02: T001, T002.

## Tasks (2/2 resolved)

### S1 — RefVerify and Verify execution-path convergence

> Targets: T001, T002 in apps/cli/src/commands/ref_verify.rs and apps/cli/src/commands/verify_catalogue_spec_refs.rs. Operation: replace live composition execution calls with driver calls. Spec: track/items/composition-root-pure-di-support-workflows-2026-07-26/spec.json#AC-01, #AC-02.

- [x] **T001**: Target: apps/cli/src/commands/ref_verify.rs::{execute_run,execute_check_approved,execute_results} and apps/cli-composition/src/ref_verify.rs tests. Operation: replace RefVerifyCompositionRoot::{ref_verify_run,ref_verify_check_approved,ref_verify_results} calls with ref_verify_driver().handle calls; retain compatibility definitions without live runtime references. Spec: IN-01/IN-02/IN-03/CN-01/CN-02/AC-01/AC-05/AC-06/AC-07. (`61c7169869028c7bc043ae599ea32f0aebd8adb7`)
- [x] **T002**: Target: apps/cli/src/commands/verify_catalogue_spec_refs.rs::execute_verify_catalogue_spec_refs, apps/cli-composition/src/verify.rs::execute_catalogue_spec_refs, and their command-path tests. Operation: replace the composition execution call with VerifyCompositionRoot::verify_driver().handle, then remove the now-dead production execution free function rather than retain it as a compatibility surface. Spec: IN-01/IN-02/IN-03/CN-01/CN-02/CN-03/AC-02/AC-05/AC-06/AC-07. (`f999293ae3fc435eae3142b17537f4a4eaa8f5db`)
