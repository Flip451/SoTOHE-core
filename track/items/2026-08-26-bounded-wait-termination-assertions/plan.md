<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# Bounded-wait termination assertions for descendant-process tests

## Summary

GO-01 → T001. The task verifies eventual descendant termination with a bounded re-observation loop and leaves production cleanup behavior unchanged.

## Tasks (1/1 resolved)

### S1 — Bounded descendant-termination observation

> Update test_version_probe_terminates_descendant_after_clean_pipe_drain in libs/infrastructure/src/review_v2/review_fix_runner/launch_context.rs to use bounded descendant-termination re-observation. IN-01; IN-02; OUT-01; OUT-02; CN-01; CN-02; CN-05; AC-01; AC-02; AC-03; AC-04; AC-05.

- [x] **T001**: Update test_version_probe_terminates_descendant_after_clean_pipe_drain in libs/infrastructure/src/review_v2/review_fix_runner/launch_context.rs to replace the immediate descendant-state assertion with bounded re-observation. IN-01; IN-02; OUT-01; OUT-02; CN-01; CN-02; CN-05; AC-01; AC-02; AC-03; AC-04; AC-05.
