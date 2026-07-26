<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# Support workflow composition roots への純 DI 移行 (RefVerify / SemanticDup / TestObligation / Verify)

## Summary

GO-01: T001, T002, T003, T004.
GO-02: T001, T002, T003, T004.

## Tasks (0/4 resolved)

### S1 — RefVerify and Verify execution-path convergence

> Targets: T001, T002 in apps/cli/src/commands/ref_verify.rs and apps/cli/src/commands/verify_catalogue_spec_refs.rs. Operation: replace live composition execution calls with driver calls. Spec: track/items/composition-root-pure-di-support-workflows-2026-07-26/spec.json#AC-01, #AC-02.

- [ ] **T001**: Target: apps/cli/src/commands/ref_verify.rs::{execute_run,execute_check_approved,execute_results} and apps/cli-composition/src/ref_verify.rs tests. Operation: replace RefVerifyCompositionRoot::{ref_verify_run,ref_verify_check_approved,ref_verify_results} calls with ref_verify_driver().handle calls; retain compatibility definitions without live runtime references. Spec: IN-01/IN-02/IN-03/CN-01/CN-02/AC-01/AC-05/AC-06/AC-07.
- [ ] **T002**: Target: apps/cli/src/commands/verify_catalogue_spec_refs.rs::execute_verify_catalogue_spec_refs, apps/cli-composition/src/verify.rs::execute_catalogue_spec_refs, and their command-path tests. Operation: replace the composition execution call with VerifyCompositionRoot::verify_driver().handle; retain the compatibility definition without live runtime references. Spec: IN-01/IN-02/IN-03/CN-01/CN-02/CN-03/AC-02/AC-05/AC-06/AC-07.

### S2 — SemanticDup additive route and feature-gated cutover

> Targets: T003, T004 in libs/infrastructure/src/semantic_dup/driver_adapter.rs and apps/cli/src/commands/semantic_dup.rs. Operation: add the driver adapter and replace CLI composition execution calls. Spec: track/items/composition-root-pure-di-support-workflows-2026-07-26/spec.json#AC-03.

- [ ] **T003**: Target: apps/cli-composition/src/semantic_dup_driver_adapter.rs::SemanticDupDriverAdapter, libs/infrastructure/src/semantic_dup/driver_adapter.rs::SemanticDupDriverAdapter, libs/usecase/src/semantic_dup_driver.rs::{SemanticDupDriverInteractor,SemanticDupDriverService,SemanticDupDriverPort}, and apps/cli-composition/src/semantic_dup/mod.rs::SemanticDupCompositionRoot::semantic_dup_driver. Operation: relocate the port implementation, wire the root to it, and add adapter/interactor tests. Spec: IN-01/IN-02/CN-01/CN-02/CN-03/CN-04/AC-03/AC-05/AC-07.
- [ ] **T004**: Target: apps/cli/src/commands/semantic_dup.rs::{execute_find_similar,execute_dup_index,execute_dup_check} and apps/cli-composition/src/semantic_dup/{find_similar,build,measure_quality,check}.rs. Operation: route the four command paths through SemanticDupCompositionRoot::semantic_dup_driver().handle, remove live composition execution calls while retaining compatibility definitions, and add semantic-dup-feature CLI integration tests. Spec: IN-01/IN-02/IN-03/CN-01/CN-02/CN-04/AC-03/AC-05/AC-06/AC-07.
