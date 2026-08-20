<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# 欠ける typed-pipeline 専用経路に grok を割り当て可能にする

## Summary

GO-01 -> T001, T002.

## Tasks (2/2 resolved)

### grok-arm — Shared process-runner grok arm

> Add the grok arm and grok-arm tests under `libs/infrastructure/src/ref_verify/`. [IN-01; IN-02; IN-03; IN-04; OS-01; OS-03; OS-04; OS-05; CN-01; CN-02; CN-03; CN-04; AC-01; AC-02; AC-03; AC-04; AC-05; AC-09]

- [x] **T001**: Add `build_grok_ref_verifier_args` and the grok arm of `run_ref_verifier_agent` in `libs/infrastructure/src/ref_verify/process_runner.rs`, placing the new arm in a sibling module under `libs/infrastructure/src/ref_verify/` so `process_runner.rs` stays within the 700-line production limit. Add grok-arm tests for `build_grok_ref_verifier_args` and `run_ref_verifier_agent` under `libs/infrastructure/src/ref_verify/`. [IN-01; IN-02; IN-03; IN-04; OS-01; OS-03; OS-04; OS-05; CN-01; CN-02; CN-03; CN-04; AC-01; AC-02; AC-03; AC-04; AC-05; AC-09] (`307dfc0b1624b425455db541595cad8e928eed8f`)

### shipped-defaults — Shipped-default lock tests

> Add shipped-default lock tests under `libs/infrastructure/src/agent_profiles/`. [IN-05; OS-02; OS-03; OS-06; OS-07; CN-05; CN-06; AC-06; AC-07; AC-08; AC-10]

- [x] **T002**: Add shipped-default lock tests under `libs/infrastructure/src/agent_profiles/` for `.harness/config/agent-profiles.json`, `.harness/config/samples/`, and `pr-reviewer` hosted resolution. [IN-05; OS-02; OS-03; OS-06; OS-07; CN-05; CN-06; AC-06; AC-07; AC-08; AC-10] (`307dfc0b1624b425455db541595cad8e928eed8f`)
